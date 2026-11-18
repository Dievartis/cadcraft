//! Risers (colonnes): where a network goes up or down through the floors. A riser is its shaft
//! outline (the trémie), a circle that shows the way (a dot: the air comes up to this level; a
//! cross: it goes down), a leader arrow and a label "name-Ø0200-240 m³/h". Each floor is its own
//! drawing, so the risers of two floors meet by their name. Like the other ventilation objects it
//! is a block reference, its parameters in extended data and its text written out for other programs.

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_4};

use cadcraft_geom::{PolyVertex, Vec2, polygon_centroid};
use serde::{Deserialize, Serialize};

use super::terminal::air_layer;
use super::{AirDirection, CableUsage, DUCT_DIAMETER_MM, HvacPiece, HvacUnits, check_label_height, check_name, invalid, v3, xdata};
use crate::{Circle, EntityKind, HAlign, Line, LwPolyline, RawTag, Result, Solid, Text, VAlign};

/// The registered application name of a riser's extended data.
pub const RISER_APP: &str = "CADCRAFT_RISER";
/// The layer of a riser's shaft outline; its symbol and text are on the layer of its air direction.
pub const RISER_LAYER: &str = "M-HVAC-RISER";
/// Most vertices of a riser's shaft outline.
pub const MAX_RISER_VERTICES: usize = 256;
/// Most flow a riser may carry, m³/h.
pub const MAX_RISER_FLOW_M3H: f64 = 1_000_000.0;
const XDATA_FORMAT: &str = "RISER1";

fn default_label_mm() -> f64 {
    100.0
}

fn default_up() -> bool {
    true
}

/// The section the riser is given at this level.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "camelCase")]
pub enum RiserSize {
    #[serde(rename_all = "camelCase")]
    Round { diameter_mm: f64 },
    #[serde(rename_all = "camelCase")]
    Rectangular { width_mm: f64, height_mm: f64 },
}

impl RiserSize {
    /// The width a plan shows, mm.
    pub fn width_mm(self) -> f64 {
        match self {
            RiserSize::Round { diameter_mm } => diameter_mm,
            RiserSize::Rectangular { width_mm, .. } => width_mm,
        }
    }

    /// The size as written on the plan: "Ø200" or "400×250".
    pub fn label(self) -> String {
        match self {
            RiserSize::Round { diameter_mm } => format!("Ø{diameter_mm:.0}"),
            RiserSize::Rectangular { width_mm, height_mm } => format!("{width_mm:.0}×{height_mm:.0}"),
        }
    }

    /// The size as the label of a riser writes it, on four digits so that the labels line up: "Ø0200" or "0400×0250".
    pub fn label_padded(self) -> String {
        match self {
            RiserSize::Round { diameter_mm } => format!("Ø{diameter_mm:04.0}"),
            RiserSize::Rectangular { width_mm, height_mm } => format!("{width_mm:04.0}×{height_mm:04.0}"),
        }
    }

    pub(super) fn validate(self) -> Result<()> {
        let ok = |v: f64| v.is_finite() && (DUCT_DIAMETER_MM.0..=DUCT_DIAMETER_MM.1).contains(&v);
        let fits = match self {
            RiserSize::Round { diameter_mm } => ok(diameter_mm),
            RiserSize::Rectangular { width_mm, height_mm } => ok(width_mm) && ok(height_mm),
        };
        if fits { Ok(()) } else { Err(invalid(format!("A riser's size is {} to {} mm.", DUCT_DIAMETER_MM.0, DUCT_DIAMETER_MM.1))) }
    }
}

/// What a riser carries between the floors: air, or an electrical cable or a control bus (the same symbol and arrows, the same shaft).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RiserMedium {
    #[default]
    Air,
    Power,
    Lighting,
    Data,
    Safety,
    Bus,
}

impl RiserMedium {
    pub const ALL: [RiserMedium; 6] =
        [RiserMedium::Air, RiserMedium::Power, RiserMedium::Lighting, RiserMedium::Data, RiserMedium::Safety, RiserMedium::Bus];

    /// The use of the cable, for the cable mediums.
    pub fn usage(self) -> Option<CableUsage> {
        match self {
            RiserMedium::Air => None,
            RiserMedium::Power => Some(CableUsage::Power),
            RiserMedium::Lighting => Some(CableUsage::Lighting),
            RiserMedium::Data => Some(CableUsage::Data),
            RiserMedium::Safety => Some(CableUsage::Safety),
            RiserMedium::Bus => Some(CableUsage::Bus),
        }
    }

    pub fn name(self) -> &'static str {
        self.usage().map_or("air", CableUsage::name)
    }

    pub fn parse(text: &str) -> Option<RiserMedium> {
        if text.trim().eq_ignore_ascii_case("air") {
            return Some(RiserMedium::Air);
        }
        match CableUsage::parse(text)? {
            CableUsage::Power => Some(RiserMedium::Power),
            CableUsage::Lighting => Some(RiserMedium::Lighting),
            CableUsage::Data => Some(RiserMedium::Data),
            CableUsage::Safety => Some(RiserMedium::Safety),
            CableUsage::Bus => Some(RiserMedium::Bus),
        }
    }
}

/// What a riser of cables has besides what every riser has: the circuit (or the bus) it carries, its section and the load, kW.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableData {
    #[serde(default)]
    pub medium: RiserMedium,
    /// The circuit (or, for a bus, the bus) the riser carries: the vertical run is counted in its length.
    #[serde(default)]
    pub circuit: String,
    #[serde(default)]
    pub section_mm2: Option<f64>,
    /// The load the riser carries at this level, kW, and the part of it that goes on to, or comes from, the other floors.
    #[serde(default)]
    pub load_kw: Option<f64>,
    #[serde(default)]
    pub incoming_load_kw: Option<f64>,
}

/// The most power a riser may carry, kW.
pub const MAX_RISER_KW: f64 = 100_000.0;

/// A riser: what its block reference keeps to regenerate it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiserParams {
    /// The name the risers of the other floors share ("VMC 3").
    pub name: String,
    #[serde(default)]
    pub air: AirDirection,
    /// Whether the network goes up from this level (`true`) or down (`false`).
    #[serde(default = "default_up")]
    pub up: bool,
    /// The shaft outline in the block's coordinates (drawing units), without repeating the first point.
    pub outline: Vec<Vec2>,
    /// The flow that comes from the other floors, m³/h, as typed.
    #[serde(default)]
    pub incoming_flow_m3h: Option<f64>,
    /// The flow the riser carries at this level, m³/h, once it is set or sized.
    #[serde(default)]
    pub flow_m3h: Option<f64>,
    /// The flow was typed, not found by sizing: sizing keeps it and sizes the riser for it.
    #[serde(default)]
    pub flow_set: bool,
    #[serde(default)]
    pub size: Option<RiserSize>,
    /// The section of the part that comes from, or goes to, the other floors (the flow typed as `incoming_flow_m3h`), when it is typed; else it is
    /// sized for that flow in the shaft. Shown at the lower end of the leader.
    #[serde(default)]
    pub incoming_size: Option<RiserSize>,
    /// Whether the size was typed: sizing leaves it as it is.
    #[serde(default)]
    pub size_set: bool,
    /// The vertical run of this level's riser, mm (floor to floor): counted in the takeoff.
    #[serde(default)]
    pub height_mm: Option<f64>,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
    /// A riser of cables or of a bus: what it carries (the air fields are then left alone).
    #[serde(default)]
    pub cable: CableData,
}

fn kw_text(kw: f64) -> String {
    if (kw - kw.round()).abs() < 0.05 { format!("{kw:.0} kW") } else { format!("{kw:.1} kW") }
}

impl RiserParams {
    /// Whether the riser carries cables (or a bus), not air.
    pub fn is_cable(&self) -> bool {
        self.cable.medium != RiserMedium::Air
    }

    /// The layer of the symbol and its texts: the air direction's, or the cable's use.
    fn symbol_layer(&self) -> &'static str {
        self.cable.medium.usage().map_or_else(|| air_layer(self.air), CableUsage::layer)
    }

    /// Whether the end of the leader that has the whole load is the upper one. Air that leaves the unit comes from the side of the unit, air that goes to it
    /// goes there; a current comes from the source: below it when it goes up.
    fn total_above(&self) -> bool {
        if self.is_cable() { !self.up } else { self.air.leaves_the_unit() != self.up }
    }

    /// The text of an end of a cable riser's leader: "CP 1-35 mm²-18 kW". A bus or a data cable carries no load: its name only.
    fn cable_text(&self, section: bool, kw: Option<f64>) -> String {
        let mut t = self.name.clone();
        if let (true, Some(s)) = (section, self.cable.section_mm2) {
            t.push_str(&format!("-{s:.0} mm²"));
        }
        if let Some(k) = kw.filter(|_| self.cable.medium.usage().is_some_and(|u| !u.is_signal())) {
            t.push_str(&format!("-{}", kw_text(k)));
        }
        t
    }

    pub fn validate(&self) -> Result<()> {
        check_name("riser", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        for kw in [self.cable.load_kw, self.cable.incoming_load_kw].into_iter().flatten() {
            if !(kw.is_finite() && kw > 0.0 && kw <= MAX_RISER_KW) {
                return Err(invalid(format!("A riser's load is more than 0 and at most {MAX_RISER_KW} kW.")));
            }
        }
        if self.cable.section_mm2.is_some_and(|s| !(s.is_finite() && (0.5..=2_500.0).contains(&s))) {
            return Err(invalid("A cable riser's section is 0.5 to 2500 mm²."));
        }
        if self.cable.circuit.chars().count() > 64 {
            return Err(invalid("A circuit name has at most 64 characters."));
        }
        if !(3..=MAX_RISER_VERTICES).contains(&self.outline.len()) || self.outline.iter().any(|p| !p.is_finite()) {
            return Err(invalid(format!("A riser's shaft outline has 3 to {MAX_RISER_VERTICES} points.")));
        }
        let (w, d) = self.shaft_mm();
        if !(w > 0.0 && d > 0.0) {
            return Err(invalid("A riser's shaft outline has no area."));
        }
        if self.height_mm.is_some_and(|h| !(h.is_finite() && h > 0.0 && h <= 100_000.0)) {
            return Err(invalid("A riser's height is between 0 and 100 m."));
        }
        for f in [self.incoming_flow_m3h, self.flow_m3h].into_iter().flatten() {
            if !(f.is_finite() && f > 0.0 && f <= MAX_RISER_FLOW_M3H) {
                return Err(invalid(format!("A riser's flow is more than 0 and at most {MAX_RISER_FLOW_M3H} m³/h.")));
            }
        }
        for s in [self.size, self.incoming_size].into_iter().flatten() {
            s.validate()?;
        }
        Ok(())
    }

    /// The width and the depth of the shaft (its bounding box), mm.
    pub fn shaft_mm(&self) -> (f64, f64) {
        let (mut lo, mut hi) = (Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN));
        for p in &self.outline {
            (lo, hi) = (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y)));
        }
        let m = self.units.mm_per_unit;
        (((hi.x - lo.x) * m).max(0.0), ((hi.y - lo.y) * m).max(0.0))
    }

    /// Redraws the shaft as a rectangle of `width_mm` by `depth_mm` around the centre it has now.
    pub fn set_shaft_mm(&mut self, width_mm: f64, depth_mm: f64) -> Result<()> {
        let ok = |v: f64| v.is_finite() && (10.0..=1.0e6).contains(&v);
        if !(ok(width_mm) && ok(depth_mm)) {
            return Err(invalid("A shaft is 10 mm to 1000 m on each side."));
        }
        let c = self.centre();
        let (hx, hy) = (self.units.to_units(width_mm) / 2.0, self.units.to_units(depth_mm) / 2.0);
        self.outline = vec![c + Vec2::new(-hx, -hy), c + Vec2::new(hx, -hy), c + Vec2::new(hx, hy), c + Vec2::new(-hx, hy)];
        Ok(())
    }

    fn centre(&self) -> Vec2 {
        polygon_centroid(&self.outline).or_else(|| self.outline.first().copied()).unwrap_or(Vec2::ZERO)
    }

    /// The text written at the end of the arrow, readable by any program: "VMC 3-Ø0200-240 m³/h" (the name, the section on four digits, the flow).
    pub fn text(&self) -> String {
        if self.is_cable() {
            return self.cable_text(true, self.cable.load_kw.or(self.cable.incoming_load_kw));
        }
        self.end_text(self.size, self.flow_m3h.or(self.incoming_flow_m3h))
    }

    /// The text of an end of the leader: "VMC 3-Ø0200-240 m³/h". Missing parts are left out, with their hyphen.
    fn end_text(&self, size: Option<RiserSize>, flow: Option<f64>) -> String {
        let mut t = self.name.clone();
        match (size.map(RiserSize::label_padded), flow.map(|f| format!("{f:.0}"))) {
            (Some(s), Some(f)) => t.push_str(&format!("-{s}-{f} m³/h")),
            (Some(s), None) => t.push_str(&format!("-{s}")),
            (None, Some(f)) => t.push_str(&format!("-{f} m³/h")),
            (None, None) => {}
        }
        t
    }

    /// What the end of the leader away from the unit says, when the riser goes on to (or comes from) another floor: the part of the flow that is not this
    /// floor's, with its section. `None` when the riser has no such part, or when all of its flow is the one it carries here.
    pub fn part_text(&self) -> Option<String> {
        if self.is_cable() {
            let (total, part) = (self.cable.load_kw?, self.cable.incoming_load_kw?);
            return (part < total - 0.05).then(|| self.cable_text(false, Some(part)));
        }
        let (total, part) = (self.flow_m3h?, self.incoming_flow_m3h?);
        if part >= total - 0.5 {
            return None;
        }
        let size = self.incoming_size.or_else(|| super::network::size_riser(self, part));
        Some(self.end_text(size, Some(part)))
    }

    /// The shaft outline, the symbol with its leader and arrow heads (the upper end for the floor above, the lower for the floor below), and the texts.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let h = self.units.to_units(self.label_height_mm);
        let vertices: Vec<PolyVertex> = self.outline.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        let outline = LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false };
        let c = self.centre();
        let (w, d) = self.shaft_mm();
        let small = self.units.to_units(w.min(d));
        // The symbol is the riser's section: a circle of its diameter, a rectangle of its two sides, else
        // a circle of a quarter of the shaft.
        let r = self.size.map_or(0.25 * small, |s| self.units.to_units(s.width_mm() / 2.0)).max(0.8 * h);
        let rect_half = match self.size {
            Some(RiserSize::Rectangular { width_mm, height_mm }) => {
                Some(Vec2::new(self.units.to_units(width_mm / 2.0).max(0.8 * h), self.units.to_units(height_mm / 2.0).max(0.8 * h)))
            }
            _ => None,
        };
        // The dot or the cross inside is sized to the nearer side; the leader leaves at 45° from the edge.
        let (inner, edge) = match rect_half {
            Some(hf) => (hf.x.min(hf.y), hf.x.min(hf.y) * std::f64::consts::SQRT_2),
            None => (r, r),
        };
        let layer = self.symbol_layer();
        let line = |a: Vec2, b: Vec2| HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(a), b: v3(b) }) };
        let circle = |radius: f64| HvacPiece { layer, kind: EntityKind::Circle(Circle { center: v3(c), radius }) };
        let symbol = match rect_half {
            Some(hf) => {
                let corners = [Vec2::new(-hf.x, -hf.y), Vec2::new(hf.x, -hf.y), Vec2::new(hf.x, hf.y), Vec2::new(-hf.x, hf.y)];
                let poly = LwPolyline {
                    vertices: corners.iter().map(|k| PolyVertex { p: c + *k, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect(),
                    closed: true,
                    const_width: 0.0,
                    elevation: 0.0,
                    plinegen: false,
                };
                HvacPiece { layer, kind: EntityKind::LwPolyline(poly) }
            }
            None => circle(r),
        };
        let mut pieces = vec![HvacPiece { layer: RISER_LAYER, kind: EntityKind::LwPolyline(outline) }, symbol];
        if self.up {
            pieces.push(circle(0.18 * inner));
        } else {
            let k = inner * FRAC_1_SQRT_2;
            pieces.push(line(c + Vec2::new(-k, -k), c + Vec2::new(k, k)));
            pieces.push(line(c + Vec2::new(-k, k), c + Vec2::new(k, -k)));
        }
        // One leader through the symbol at 45°: its two ends speak of the floor above and the floor below. Each end has a filled arrow head that points the
        // way the air goes (up for a riser that goes up, down for one that goes down), and the text where the leader ends. The end on the side of the unit
        // has the whole flow of this level; the other, the part that goes on to (or comes from) the other floor, when the riser has one.
        let dir = Vec2::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2);
        let (len, head) = (self.units.to_units(0.6 * (w * w + d * d).sqrt()).max(6.0 * h), 1.2 * h);
        let flow_dir = if self.up { dir } else { -dir };
        let side = dir.perp();
        let triangle = |apex: Vec2| {
            let base = apex - flow_dir * head;
            let corner = |k: f64| v3(base + side * (k * 0.4 * head));
            HvacPiece { layer, kind: EntityKind::Solid(Solid { corners: [v3(apex), corner(1.0), corner(-1.0), corner(-1.0)] }) }
        };
        let total_above = self.total_above();
        let part = self.part_text();
        for (above, text) in
            [(true, if total_above { Some(self.text()) } else { part.clone() }), (false, if total_above { part.clone() } else { Some(self.text()) })]
        {
            let Some(text) = text else { continue };
            let sign = if above { 1.0 } else { -1.0 };
            let (start, end) = (c + dir * (sign * edge), c + dir * (sign * (edge + len)));
            pieces.push(line(start, end));
            // The air goes toward this end (and the head's point is on it), or comes from it (and the head's base is on it).
            pieces.push(triangle(if (sign > 0.0) == self.up { end } else { end + flow_dir * head }));
            let at = end + dir * (sign * 0.6 * h);
            pieces.push(HvacPiece {
                layer,
                kind: EntityKind::Text(Text {
                    insert: v3(at),
                    align_pt: Some(v3(at)),
                    height: h,
                    value: text,
                    rotation: FRAC_PI_4,
                    width_factor: 1.0,
                    oblique: 0.0,
                    style: "Standard".into(),
                    halign: if above { HAlign::Left } else { HAlign::Right },
                    valign: VAlign::Middle,
                }),
            });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(RISER_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<RiserParams> {
        let p: RiserParams = xdata::from_xdata(RISER_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The riser of air an object is, when it is a block reference carrying usable riser data: the networks, the takeoff and the sizing see these only.
pub fn riser_of(e: &crate::Entity) -> Option<RiserParams> {
    any_riser_of(e).filter(|r| !r.is_cable())
}

/// The riser an object is, of air or of cables or a bus.
pub fn any_riser_of(e: &crate::Entity) -> Option<RiserParams> {
    match &e.kind {
        EntityKind::Insert(_) => RiserParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// Adds riser `p`, its shaft outline given in drawing coordinates: an anonymous block with the
    /// outline, symbol and text, referenced on [`RISER_LAYER`] at the outline's first point.
    pub fn add_riser(&mut self, space: &crate::Space, mut common: crate::Common, mut p: RiserParams) -> Result<crate::Handle> {
        let origin = p.outline.first().copied().ok_or_else(|| invalid("A riser needs a shaft outline."))?;
        for v in &mut p.outline {
            *v -= origin;
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(RISER_APP, &common.xdata, p.to_xdata()?);
        common.layer = RISER_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(RISER_LAYER);
        let insert = crate::Insert { block, insert: v3(origin), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives riser `h` the parameters `p` (outline in the block's coordinates), redrawing it in place.
    pub fn regenerate_riser(&mut self, h: crate::Handle, p: &RiserParams) -> Result<()> {
        let pieces = p.generate()?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(RISER_APP, &old, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }
}
