//! Pipe risers (colonnes de tuyauterie): where the pipes of a floor go up or down through the floors. One riser is one shaft (the
//! trémie) and holds several lines (chilled water supply and return, hot water, change-over, a condensate drain, refrigerant
//! links), each with its service, its size and the load it carries to the other floors. Each floor is its own drawing, so the
//! risers of two floors meet by their name. The pipes of a floor that end in a shaft, or start in one, are read through the
//! riser: a water network that starts there takes it as its source, and one that ends there carries what the other floors need.
//! Like the other plan objects it is a block reference with its parameters in extended data.

use cadcraft_geom::{PolyVertex, Vec2, point_in_polygon, polygon_centroid, shoelace};
use serde::{Deserialize, Serialize};

use super::{HvacPiece, HvacUnits, PipeLeg, PipeParams, PipeService, check_label_height, check_name, invalid, label_left, v3, xdata};
use crate::{Circle, EntityKind, Line, LwPolyline, RawTag, Result};

/// The registered application name of a pipe riser's extended data.
pub const PIPE_RISER_APP: &str = "CADCRAFT_PIPERISER";
/// The layer of a pipe riser's shaft outline; each line's symbol and text are on the layer of its service.
pub const PIPE_RISER_LAYER: &str = "M-PIPE-RISER";
/// Most vertices of a shaft outline, and most lines in a riser.
pub const MAX_PIPE_RISER_VERTICES: usize = 256;
pub const MAX_RISER_LINES: usize = 32;
const XDATA_FORMAT: &str = "PIPERISER1";
/// The gap between two line symbols in a shaft, mm.
const SYMBOL_GAP_MM: f64 = 30.0;

fn default_label_mm() -> f64 {
    100.0
}

fn default_up() -> bool {
    true
}

/// One line of a riser: a pipe of a service that goes through the shaft.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiserLine {
    pub service: PipeService,
    #[serde(default)]
    pub leg: PipeLeg,
    /// The nominal diameter of a steel pipe, mm; the outer diameter of a refrigerant tube (the liquid tube of a link).
    pub dn_mm: f64,
    /// The gas tubes of a refrigerant link, mm.
    #[serde(default)]
    pub gas_dn_mm: f64,
    #[serde(default)]
    pub gas2_dn_mm: f64,
    /// Whether the size was typed: sizing leaves it.
    #[serde(default)]
    pub imposed: bool,
    #[serde(default)]
    pub insulation_mm: f64,
    /// What the line carries to the other floors, as typed: kW for water, l/h for a condensate drain, capacity index for refrigerant.
    #[serde(default)]
    pub incoming: Option<f64>,
    /// What it carries at this level once sized (the load of the floor's network that starts here, and `incoming`), and the flow and velocity.
    #[serde(default)]
    pub load: Option<f64>,
    #[serde(default)]
    pub flow_m3h: Option<f64>,
    #[serde(default)]
    pub velocity: Option<f64>,
    /// The name of the network the line is on: its symbol and its text are on the network's layer.
    #[serde(default)]
    pub network: String,
}

impl RiserLine {
    /// A line of `service` and `leg`, `dn_mm` across.
    pub fn new(service: PipeService, leg: PipeLeg, dn_mm: f64) -> RiserLine {
        let mut l = RiserLine {
            service,
            leg: if service.has_legs() { leg } else { PipeLeg::Supply },
            dn_mm,
            gas_dn_mm: 0.0,
            gas2_dn_mm: 0.0,
            imposed: false,
            insulation_mm: 0.0,
            incoming: None,
            load: None,
            flow_m3h: None,
            velocity: None,
            network: String::new(),
        };
        if service.has_gas_tube() {
            l.gas_dn_mm = 15.88;
        }
        if service == PipeService::RefrigerantTriple {
            l.gas2_dn_mm = 19.05;
        }
        l
    }

    /// The pipe the line is made of, one unit long, for its size, its colour and its text.
    pub fn pipe(&self, units: HvacUnits) -> PipeParams {
        PipeParams {
            leg: self.leg,
            gas_dn_mm: self.gas_dn_mm,
            gas2_dn_mm: self.gas2_dn_mm,
            imposed: self.imposed,
            insulation_mm: self.insulation_mm,
            network: self.network.clone(),
            ..PipeParams::new(self.service, self.dn_mm, units)
        }
    }

    /// Whether the line is the same network as a pipe of `service` and `leg`.
    pub fn is(&self, service: PipeService, leg: PipeLeg) -> bool {
        self.service == service && (!service.has_legs() || self.leg == leg)
    }

    /// The text written for the line: "CHW-S DN65 120 kW".
    pub fn text(&self, units: HvacUnits) -> String {
        let p = self.pipe(units);
        let mut t = format!("{} {}", p.tag(), p.size_text());
        if !self.network.trim().is_empty() {
            t = format!("{} {t}", self.network.trim());
        }
        if let Some(l) = self.load.or(self.incoming) {
            t.push_str(&match self.service {
                PipeService::Condensate => format!(" {l:.0} l/h"),
                s if s.is_refrigerant() => format!(" i{l:.0}"),
                _ => format!(" {l:.0} kW"),
            });
        }
        t
    }
}

/// A pipe riser: what its block reference keeps to regenerate it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipeRiserParams {
    /// The name the risers of the other floors share ("CT 1").
    pub name: String,
    /// Whether the pipes go up from this level (`true`) or down (`false`).
    #[serde(default = "default_up")]
    pub up: bool,
    /// The shaft outline in the block's coordinates (drawing units), without repeating the first point.
    pub outline: Vec<Vec2>,
    pub lines: Vec<RiserLine>,
    /// The vertical run of this level's riser, mm (floor to floor): counted in the takeoff for each line.
    #[serde(default)]
    pub height_mm: Option<f64>,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl PipeRiserParams {
    pub fn validate(&self) -> Result<()> {
        check_name("riser", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if !(3..=MAX_PIPE_RISER_VERTICES).contains(&self.outline.len())
            || self.outline.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12)
        {
            return Err(invalid(format!("A riser's shaft outline has 3 to {MAX_PIPE_RISER_VERTICES} points.")));
        }
        if shoelace(&self.outline).abs() <= 1e-12 {
            return Err(invalid("A riser's shaft outline has no area."));
        }
        if self.height_mm.is_some_and(|h| !(h.is_finite() && h > 0.0 && h <= 100_000.0)) {
            return Err(invalid("A riser's height is between 0 and 100 m."));
        }
        if self.lines.len() > MAX_RISER_LINES {
            return Err(invalid(format!("A riser has at most {MAX_RISER_LINES} lines.")));
        }
        for l in &self.lines {
            l.pipe(self.units).validate()?;
            if [l.incoming, l.load, l.flow_m3h, l.velocity].into_iter().flatten().any(|v| !v.is_finite() || !(0.0..=1.0e9).contains(&v)) {
                return Err(invalid("The load of a riser line is 0 or more."));
            }
        }
        Ok(())
    }

    /// The centre of the shaft.
    pub fn centre(&self) -> Vec2 {
        polygon_centroid(&self.outline).or_else(|| self.outline.first().copied()).unwrap_or(Vec2::ZERO)
    }

    /// The width and depth of the shaft (its bounding box), mm.
    pub fn shaft_mm(&self) -> (f64, f64) {
        let (mut lo, mut hi) = (Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN));
        for p in &self.outline {
            (lo, hi) = (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y)));
        }
        let m = self.units.mm_per_unit;
        (((hi.x - lo.x) * m).max(0.0), ((hi.y - lo.y) * m).max(0.0))
    }

    /// The line of the riser that is the network of `service` and `leg`, when it has one.
    pub fn line_for(&self, service: PipeService, leg: PipeLeg) -> Option<&RiserLine> {
        self.lines.iter().find(|l| l.is(service, leg))
    }

    /// Where each line's symbol is, relative to the block's origin: rows along the longer side of the shaft, centred.
    pub(super) fn symbol_centres(&self) -> Vec<Vec2> {
        let n = self.lines.len();
        if n == 0 {
            return Vec::new();
        }
        let m = self.units.mm_per_unit;
        let widest = self.lines.iter().map(|l| l.pipe(self.units).plan_width_mm()).fold(1.0, f64::max);
        let pitch = (widest + SYMBOL_GAP_MM) / m;
        let (w, d) = self.shaft_mm();
        let along_x = w >= d;
        let side = (if along_x { w } else { d }) / m;
        let per_row = ((side / pitch).floor() as usize).clamp(1, n);
        let rows = n.div_ceil(per_row);
        let c = self.centre();
        (0..n)
            .map(|i| {
                let (col, row) = (i % per_row, i / per_row);
                let in_row = if row + 1 == rows && !n.is_multiple_of(per_row) { n % per_row } else { per_row };
                let u = (col as f64 - (in_row as f64 - 1.0) / 2.0) * pitch;
                let v = (row as f64 - (rows as f64 - 1.0) / 2.0) * pitch;
                if along_x { c + Vec2::new(u, v) } else { c + Vec2::new(v, u) }
            })
            .collect()
    }

    /// The shaft outline, a symbol for each line (the section of its tubes, with a dot when it comes up here and a cross when it
    /// goes down) and the name and the lines written under the shaft.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let u = self.units;
        let h = u.to_units(self.label_height_mm);
        let vertices: Vec<PolyVertex> = self.outline.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        let mut pieces = vec![HvacPiece {
            layer: PIPE_RISER_LAYER,
            kind: EntityKind::LwPolyline(LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false }),
        }];
        for (line, c) in self.lines.iter().zip(self.symbol_centres()) {
            let pipe = line.pipe(u);
            let tubes = pipe.tubes();
            let layer = pipe.layer();
            if tubes.is_empty() {
                let r = (u.to_units(pipe.plan_width_mm()) / 2.0).max(0.3 * h);
                pieces.push(HvacPiece { layer, kind: EntityKind::Circle(Circle { center: v3(c), radius: r }) });
                pieces.extend(self.direction_mark(layer, c, r));
            } else {
                // A link: one circle for each tube, side by side.
                let total: f64 = tubes.iter().map(|(_, w)| u.to_units(*w)).sum::<f64>() + u.to_units(20.0) * (tubes.len() as f64 - 1.0);
                let mut x = c.x - total / 2.0;
                for (tube_layer, w) in &tubes {
                    let r = (u.to_units(*w) / 2.0).max(0.2 * h);
                    pieces.push(HvacPiece { layer: tube_layer, kind: EntityKind::Circle(Circle { center: v3(Vec2::new(x + r, c.y)), radius: r }) });
                    x += 2.0 * r + u.to_units(20.0);
                }
                pieces.extend(self.direction_mark(layer, c, u.to_units(pipe.plan_width_mm()) / 4.0));
            }
        }
        // The name, then a row for each line in the colour of its service, under the shaft.
        let (_, d) = self.shaft_mm();
        let c = self.centre();
        let mut y = c.y - u.to_units(d) / 2.0 - 1.4 * h;
        let left = self.outline.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let head = format!("{} {}", self.name, if self.up { "UP" } else { "DOWN" });
        pieces.push(HvacPiece { layer: PIPE_RISER_LAYER, kind: label_left(Vec2::new(left, y), h, head) });
        for line in &self.lines {
            y -= 1.4 * h;
            pieces.push(HvacPiece { layer: line.pipe(u).layer(), kind: label_left(Vec2::new(left, y), h, line.text(u)) });
        }
        Ok(pieces)
    }

    /// A dot when the line comes up to this level, a cross when it goes down, as the air riser draws it.
    fn direction_mark(&self, layer: &'static str, c: Vec2, r: f64) -> Vec<HvacPiece> {
        let line = |a: Vec2, b: Vec2| HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(a), b: v3(b) }) };
        let k = 0.5 * r;
        if self.up {
            vec![HvacPiece { layer, kind: EntityKind::Circle(Circle { center: v3(c), radius: 0.12 * r }) }]
        } else {
            vec![line(c + Vec2::new(-k, -k), c + Vec2::new(k, k)), line(c + Vec2::new(-k, k), c + Vec2::new(k, -k))]
        }
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(PIPE_RISER_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<PipeRiserParams> {
        let p: PipeRiserParams = xdata::from_xdata(PIPE_RISER_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The pipe riser an object is, when it is a block reference carrying usable riser data.
pub fn pipe_riser_of(e: &crate::Entity) -> Option<PipeRiserParams> {
    match &e.kind {
        EntityKind::Insert(_) => PipeRiserParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// Adds riser `p`, its shaft outline given in drawing coordinates: an anonymous block, referenced at the outline's first point.
    pub fn add_pipe_riser(&mut self, space: &crate::Space, mut common: crate::Common, mut p: PipeRiserParams) -> Result<crate::Handle> {
        let origin = p.outline.first().copied().ok_or_else(|| invalid("A riser needs a shaft outline."))?;
        for v in &mut p.outline {
            *v -= origin;
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(PIPE_RISER_APP, &common.xdata, p.to_xdata()?);
        common.layer = PIPE_RISER_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(PIPE_RISER_LAYER);
        let insert = crate::Insert { block, insert: v3(origin), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives riser `h` the parameters `p` (outline in the block's coordinates), redrawing it in place.
    pub fn regenerate_pipe_riser(&mut self, h: crate::Handle, p: &PipeRiserParams) -> Result<()> {
        let pieces = p.generate()?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(PIPE_RISER_APP, &old, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// The shaft of riser `h` in drawing coordinates, with its data: the reference's position, rotation and scale applied to the outline.
    pub fn pipe_riser_outline(&self, h: crate::Handle) -> Option<(PipeRiserParams, Vec<Vec2>)> {
        let e = self.entity(h)?;
        let EntityKind::Insert(i) = &e.kind else { return None };
        let p = pipe_riser_of(e)?;
        let (s, c) = i.rotation.sin_cos();
        let world = p
            .outline
            .iter()
            .map(|v| {
                let (x, y) = (v.x * i.scale.x, v.y * i.scale.y);
                Vec2::new(i.insert.x + x * c - y * s, i.insert.y + x * s + y * c)
            })
            .collect();
        Some((p, world))
    }

    /// Every pipe riser of model space with its shaft in drawing coordinates.
    pub fn pipe_risers(&self) -> Vec<(crate::Handle, PipeRiserParams, Vec<Vec2>)> {
        self.model.iter().filter_map(|e| self.pipe_riser_outline(e.handle).map(|(p, o)| (e.handle, p, o))).collect()
    }
}

/// Whether point `p` is in the shaft `outline` (or within `margin` drawing units of it).
pub(super) fn in_shaft(outline: &[Vec2], p: Vec2, margin: f64) -> bool {
    point_in_polygon(outline, p) || outline.iter().zip(outline.iter().cycle().skip(1)).any(|(a, b)| distance_to_segment(p, *a, *b) <= margin)
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= 1e-18 {
        return p.dist(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.dist(a + ab * t)
}
