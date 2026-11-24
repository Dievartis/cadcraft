//! Risers (colonnes): where a network goes up or down through the floors. A riser is its shaft
//! outline (the trémie), a circle that shows the way (a dot: the air comes up to this level; a
//! cross: it goes down), a leader arrow and a label "name-Ø200-240 m³/h". Each floor is its own
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
    Remote,
}

impl RiserMedium {
    pub const ALL: [RiserMedium; 7] =
        [RiserMedium::Air, RiserMedium::Power, RiserMedium::Lighting, RiserMedium::Data, RiserMedium::Safety, RiserMedium::Bus, RiserMedium::Remote];

    /// The use of the cable, for the cable mediums.
    pub fn usage(self) -> Option<CableUsage> {
        match self {
            RiserMedium::Air => None,
            RiserMedium::Power => Some(CableUsage::Power),
            RiserMedium::Lighting => Some(CableUsage::Lighting),
            RiserMedium::Data => Some(CableUsage::Data),
            RiserMedium::Safety => Some(CableUsage::Safety),
            RiserMedium::Bus => Some(CableUsage::Bus),
            RiserMedium::Remote => Some(CableUsage::Remote),
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
            CableUsage::Remote => Some(RiserMedium::Remote),
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// `"#rrggbb"` as red, green and blue.
pub fn parse_hex_colour(text: &str) -> Option<(u8, u8, u8)> {
    let h = text.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let part = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    Some((part(0)?, part(2)?, part(4)?))
}

/// One line of a riser of cables: a circuit (or a bus) that goes through the shaft, with its section and its load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableLine {
    pub usage: CableUsage,
    /// The circuit (or, for a bus, the bus) the line carries: its vertical run is counted in the length of that circuit.
    #[serde(default)]
    pub circuit: String,
    #[serde(default)]
    pub section_mm2: Option<f64>,
    /// The conductors, the earth and the shield of the cable ("3G1.5", "2x0.75 blindé"), when it was given as such: then the line is written with them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub earth: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub shielded: bool,
    /// A colour of its own for the line, "#rrggbb", instead of the colour of its circuit's wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// The load the line carries at this level, kW, and the part of it that goes on to, or comes from, the other floors.
    #[serde(default)]
    pub load_kw: Option<f64>,
    #[serde(default)]
    pub incoming_load_kw: Option<f64>,
}

impl CableLine {
    pub fn new(usage: CableUsage) -> CableLine {
        CableLine {
            usage,
            circuit: String::new(),
            section_mm2: None,
            cores: None,
            earth: false,
            shielded: false,
            colour: None,
            load_kw: None,
            incoming_load_kw: None,
        }
    }

    /// The cable of the line as it is written: "3G1.5", "2x0.75 blindé", or just the section "35 mm²" when it was given alone.
    pub fn cable_text(&self) -> Option<String> {
        let section = self.section_mm2?;
        if self.cores.is_none() && !self.shielded {
            return Some(format!("{section:.0} mm²"));
        }
        Some(super::CableSpec { cores: self.cores, earth: self.earth, shielded: self.shielded, section_mm2: section }.text())
    }

    /// The colour of the line when it has one of its own and it is a colour.
    pub fn own_colour(&self) -> Option<&str> {
        self.colour.as_deref().filter(|c| parse_hex_colour(c).is_some())
    }

    /// The layer the pieces of the line are on: its circuit's cable layer, with the colour of its own after the line type when it has one.
    pub fn piece_layer(&self) -> &'static str {
        let base = super::cable_layer_of(self.usage, &self.circuit);
        match self.own_colour() {
            Some(c) => super::pipe_networks::intern(format!("{base}|ByLayer|{c}")).unwrap_or(base),
            None => base,
        }
    }

    /// The part of the load that goes on to, or comes from, the other floors, when the line has one apart from what it carries here.
    pub fn part(&self) -> Option<f64> {
        match (self.load_kw, self.incoming_load_kw) {
            (Some(total), Some(part)) if part < total - 0.05 => Some(part),
            _ => None,
        }
    }

    /// The text written for the line: "TD1-C1 35 mm² 18 kW". A bus or a data cable carries no load: no kW.
    pub fn text(&self) -> String {
        self.text_of(self.load_kw.or(self.incoming_load_kw))
    }

    /// The text of the line for a load of `kw`.
    pub fn text_of(&self, kw: Option<f64>) -> String {
        let mut t = if self.circuit.trim().is_empty() { self.usage.name().to_string() } else { self.circuit.trim().to_string() };
        if let Some(c) = self.cable_text() {
            t.push(' ');
            t.push_str(&c);
        }
        if let Some(k) = kw.filter(|_| !self.usage.is_signal()) {
            t.push(' ');
            t.push_str(&kw_text(k));
        }
        t
    }
}

/// What a riser of cables has besides what every riser has: its lines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableData {
    /// What the riser is for: `Air` for a riser of air, else the use of the cable it was drawn for (its lines say their own).
    #[serde(default)]
    pub medium: RiserMedium,
    #[serde(default)]
    pub lines: Vec<CableLine>,
    /// What a riser had before it held lines (one circuit): read to give the line it was, never written.
    #[serde(default, skip_serializing, rename = "circuit")]
    pub legacy_circuit: String,
    #[serde(default, skip_serializing, rename = "sectionMm2")]
    pub legacy_section_mm2: Option<f64>,
    #[serde(default, skip_serializing, rename = "loadKw")]
    pub legacy_load_kw: Option<f64>,
    #[serde(default, skip_serializing, rename = "incomingLoadKw")]
    pub legacy_incoming_load_kw: Option<f64>,
}

impl CableData {
    /// A riser of `medium` with one line of its use.
    pub fn of(medium: RiserMedium) -> CableData {
        let lines = medium.usage().map(|u| vec![CableLine::new(u)]).unwrap_or_default();
        CableData { medium, lines, ..CableData::default() }
    }

    #[cfg(test)]
    pub(super) fn migrate_for_test(&mut self) {
        self.migrate();
    }

    /// Turns what an older riser kept (one circuit) into its line.
    fn migrate(&mut self) {
        let Some(usage) = self.medium.usage().filter(|_| self.lines.is_empty()) else { return };
        self.lines.push(CableLine {
            usage,
            circuit: std::mem::take(&mut self.legacy_circuit),
            section_mm2: self.legacy_section_mm2.take(),
            cores: None,
            earth: false,
            shielded: false,
            colour: None,
            load_kw: self.legacy_load_kw.take(),
            incoming_load_kw: self.legacy_incoming_load_kw.take(),
        });
    }
}

/// Most lines of a riser of cables.
pub const MAX_CABLE_LINES: usize = 32;
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

    /// The layer of the symbol and its texts: the air direction's.
    fn symbol_layer(&self) -> &'static str {
        air_layer(self.air)
    }

    /// Whether the end of the leader that has the whole flow is the upper one: air that leaves the unit comes from the side of the unit, air that goes to
    /// it goes there.
    fn total_above(&self) -> bool {
        self.air.leaves_the_unit() != self.up
    }

    pub fn validate(&self) -> Result<()> {
        check_name("riser", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.cable.lines.len() > MAX_CABLE_LINES {
            return Err(invalid(format!("A riser has at most {MAX_CABLE_LINES} lines.")));
        }
        for l in &self.cable.lines {
            for kw in [l.load_kw, l.incoming_load_kw].into_iter().flatten() {
                if !(kw.is_finite() && kw > 0.0 && kw <= MAX_RISER_KW) {
                    return Err(invalid(format!("A riser's load is more than 0 and at most {MAX_RISER_KW} kW.")));
                }
            }
            if l.section_mm2.is_some_and(|s| !(s.is_finite() && (0.5..=2_500.0).contains(&s))) {
                return Err(invalid("A cable riser's section is 0.5 to 2500 mm²."));
            }
            if l.cores.is_some_and(|n| !(1..=100).contains(&n)) {
                return Err(invalid("A cable riser's cable has 1 to 100 conductors."));
            }
            if l.colour.as_deref().is_some_and(|c| parse_hex_colour(c).is_none()) {
                return Err(invalid("The colour of a riser's line is written #rrggbb."));
            }
            if l.circuit.chars().count() > 64 {
                return Err(invalid("A circuit name has at most 64 characters."));
            }
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

    /// The text written at the end of the arrow, readable by any program: "VMC 3-Ø200-240 m³/h" (the name, the section, the flow).
    pub fn text(&self) -> String {
        if self.is_cable() {
            let rows: Vec<String> = self.cable.lines.iter().map(CableLine::text).collect();
            return format!("{} {}", self.name, rows.join("; ")).trim().to_string();
        }
        self.end_text(self.size, self.flow_m3h.or(self.incoming_flow_m3h))
    }

    /// The text of an end of the leader: "VMC 3-Ø200-240 m³/h". Missing parts are left out, with their hyphen.
    fn end_text(&self, size: Option<RiserSize>, flow: Option<f64>) -> String {
        let mut t = self.name.clone();
        match (size.map(RiserSize::label), flow.map(|f| format!("{f:.0}"))) {
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
            return None;
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
        if self.is_cable() {
            return Ok(self.generate_cable());
        }
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

    /// A riser of cables: the shaft and, for each line, a circle in the colour of its use (a dot when the current comes up to this level, a cross when it goes
    /// down), with its leader and arrows like the air's; the name at the lower right of the shaft. The lines are side by side across the leaders, so that nothing overlaps.
    fn generate_cable(&self) -> Vec<HvacPiece> {
        let u = self.units;
        let h = u.to_units(self.label_height_mm);
        let vertices: Vec<PolyVertex> = self.outline.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        let mut pieces = vec![HvacPiece {
            layer: RISER_LAYER,
            kind: EntityKind::LwPolyline(LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false }),
        }];
        let r = 0.35 * h;
        let leaders: Vec<Leader> = self
            .cable
            .lines
            .iter()
            .map(|l| Leader {
                layer: l.piece_layer(),
                width: 2.0 * r,
                flows_up: self.up,
                // A current comes from the source: below it when it goes up.
                total_above: !self.up,
                total: l.text(),
                part: l.part().map(|kw| l.text_of(Some(kw))),
            })
            .collect();
        let (w, d) = self.shaft_mm();
        let centres = grid_centres(self.centre(), u.to_units(w), u.to_units(d), &vec![2.0 * r; self.cable.lines.len()], u.to_units(30.0));
        let up = self.up;
        let lines = &self.cable.lines;
        pieces.extend(leader_pieces(h, u.to_units((w * w + d * d).sqrt()), &leaders, &centres, |i, c| {
            let layer = lines.get(i).map_or(RISER_LAYER, |l| l.piece_layer());
            let mut out = vec![HvacPiece { layer, kind: EntityKind::Circle(Circle { center: v3(c), radius: r }) }];
            if up {
                out.push(HvacPiece { layer, kind: EntityKind::Circle(Circle { center: v3(c), radius: 0.15 * r }) });
            } else {
                let k = 0.5 * r;
                for (a, b) in [((-k, -k), (k, k)), ((-k, k), (k, -k))] {
                    out.push(HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(c + Vec2::new(a.0, a.1)), b: v3(c + Vec2::new(b.0, b.1)) }) });
                }
            }
            out
        }));
        pieces.push(HvacPiece {
            layer: RISER_LAYER,
            kind: super::riser_name_label(&self.outline, h, &format!("{} {}", self.name, if up { "UP" } else { "DOWN" })),
        });
        pieces
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(RISER_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<RiserParams> {
        let mut p: RiserParams = xdata::from_xdata(RISER_APP, XDATA_FORMAT, tags)?;
        p.cable.migrate();
        p.validate().is_ok().then_some(p)
    }
}

/// One line of a riser drawn with its own leader and arrows (the lines of a riser of cables or of pipes): how wide its symbol is, which way it flows and what each
/// end of the leader says.
pub(super) struct Leader {
    pub layer: &'static str,
    /// The width of its symbol, drawing units.
    pub width: f64,
    /// Whether it flows up (its arrow heads point up).
    pub flows_up: bool,
    /// Whether the end with the whole load is the upper one.
    pub total_above: bool,
    pub total: String,
    /// What goes on to, or comes from, the other floor: the other end of the leader, when the line has such a part.
    pub part: Option<String>,
}

/// A filled arrow head with its point at `apex`, pointing along `flow_dir`.
pub(super) fn head_piece(layer: &'static str, apex: Vec2, flow_dir: Vec2, head: f64) -> HvacPiece {
    let base = apex - flow_dir * head;
    let side = flow_dir.perp();
    let corner = |k: f64| v3(base + side * (k * 0.4 * head));
    HvacPiece { layer, kind: EntityKind::Solid(Solid { corners: [v3(apex), corner(1.0), corner(-1.0), corner(-1.0)] }) }
}

/// Where the symbols of the lines of a riser are: in rows along the longer side of the shaft, centred, a gap between two symbols, so that they spread over the
/// shaft. `centre` and the shaft's `width` and `depth` are in drawing units.
pub(super) fn grid_centres(centre: Vec2, width: f64, depth: f64, widths: &[f64], gap: f64) -> Vec<Vec2> {
    let n = widths.len();
    if n == 0 {
        return Vec::new();
    }
    let pitch = widths.iter().copied().fold(0.0, f64::max) + gap;
    let along_x = width >= depth;
    let side = if along_x { width } else { depth };
    let per_row = ((side / pitch).floor() as usize).clamp(1, n);
    let rows = n.div_ceil(per_row);
    (0..n)
        .map(|i| {
            let (col, row) = (i % per_row, i / per_row);
            let in_row = if row + 1 == rows && !n.is_multiple_of(per_row) { n % per_row } else { per_row };
            let u = (col as f64 - (in_row as f64 - 1.0) / 2.0) * pitch;
            let v = (row as f64 - (rows as f64 - 1.0) / 2.0) * pitch;
            if along_x { centre + Vec2::new(u, v) } else { centre + Vec2::new(v, u) }
        })
        .collect()
}

/// The places across (along the other diagonal) of the arrows of lines whose symbols are at `across` (sorted): as near as they can be to those, and at least
/// `step` apart (the closest sequence in the least squares sense: the arrows of symbols that are close are spread out around them).
fn spread_across(across: &[f64], step: f64) -> Vec<f64> {
    // With q = across - rank * step the arrows are `step` apart when q does not decrease: pool the adjacent values that decrease.
    let mut blocks: Vec<(f64, usize)> = Vec::new();
    for (k, a) in across.iter().enumerate() {
        blocks.push((a - k as f64 * step, 1));
        while blocks.len() > 1 {
            let (last, before) = (blocks[blocks.len() - 1], blocks[blocks.len() - 2]);
            if before.0 <= last.0 {
                break;
            }
            let n = before.1 + last.1;
            let mean = (before.0 * before.1 as f64 + last.0 * last.1 as f64) / n as f64;
            blocks.truncate(blocks.len() - 2);
            blocks.push((mean, n));
        }
    }
    let mut out = Vec::with_capacity(across.len());
    for (mean, n) in blocks {
        for _ in 0..n {
            out.push(mean + out.len() as f64 * step);
        }
    }
    out
}

/// The leaders of the lines of a riser. Each line has its symbol at `centres[i]` (drawn by `symbol`) and an arrow at 45° whose head points the way the line
/// flows, with its text at the end: the end on the side of `total_above` says the whole load, the other the part for the other floor, when there is one.
/// The arrows are parallel and at least a label and a half apart across (so that neither they nor their texts ever touch), whatever the symbols' places:
/// an arrow is where its symbol is, or, when two symbols are too close across, a little aside, joined to its symbol by a thin line.
pub(super) fn leader_pieces(
    h: f64,
    diag: f64,
    leaders: &[Leader],
    centres: &[Vec2],
    mut symbol: impl FnMut(usize, Vec2) -> Vec<HvacPiece>,
) -> Vec<HvacPiece> {
    let n = leaders.len().min(centres.len());
    let (dir, anti) = (Vec2::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2), Vec2::new(FRAC_1_SQRT_2, -FRAC_1_SQRT_2));
    let (len, head, step) = ((0.6 * diag).max(6.0 * h), 1.2 * h, 1.5 * h);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| centres[*a].dot(anti).total_cmp(&centres[*b].dot(anti)));
    let across: Vec<f64> = order.iter().map(|i| centres[*i].dot(anti)).collect();
    let places = spread_across(&across, step);
    let mut pieces = Vec::new();
    for (&i, &u) in order.iter().zip(&places) {
        let (Some(l), Some(&c)) = (leaders.get(i), centres.get(i)) else { continue };
        pieces.extend(symbol(i, c));
        let flow_dir = if l.flows_up { dir } else { -dir };
        let r = l.width / 2.0;
        // Where the arrow is: on its line across, at the symbol's height along it; a thin line joins it to the symbol when it is not on it.
        let foot = anti * u + dir * c.dot(dir);
        let aside = u - c.dot(anti);
        if aside.abs() > r {
            let from = c + anti * (r * aside.signum());
            pieces.push(HvacPiece { layer: l.layer, kind: EntityKind::Line(Line { a: v3(from), b: v3(foot) }) });
        }
        for above in [true, false] {
            let text = if above == l.total_above { Some(l.total.clone()) } else { l.part.clone() };
            let Some(text) = text else { continue };
            let sign = if above { 1.0 } else { -1.0 };
            let start = if aside.abs() > r { foot } else { c + dir * (sign * r) };
            let end = foot + dir * (sign * len);
            pieces.push(HvacPiece { layer: l.layer, kind: EntityKind::Line(Line { a: v3(start), b: v3(end) }) });
            // The point is on the end the line flows toward, else the head's base is on it.
            pieces.push(head_piece(l.layer, if above == l.flows_up { end } else { end + flow_dir * head }, flow_dir, head));
            let at = end + dir * (sign * 0.6 * h);
            pieces.push(HvacPiece {
                layer: l.layer,
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
    }
    pieces
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
        self.ensure_cable_line_layers(&p);
        let insert = crate::Insert { block, insert: v3(origin), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives riser `h` the parameters `p` (outline in the block's coordinates), redrawing it in place.
    pub fn regenerate_riser(&mut self, h: crate::Handle, p: &RiserParams) -> Result<()> {
        let pieces = p.generate()?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(RISER_APP, &old, p.to_xdata()?);
        self.ensure_cable_line_layers(p);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// The layers of the lines of a riser of cables: the layer of the cables of each line's circuit, which is in the colour of that circuit's wire.
    fn ensure_cable_line_layers(&mut self, p: &RiserParams) {
        for l in &p.cable.lines {
            self.ensure_layer(super::cable_layer_of(l.usage, &l.circuit));
        }
    }
}
