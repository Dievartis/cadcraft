//! Structure: columns and beams in rolled steel sections (IPN, IPE, HEA, HEB, UPN), drawn in plan to scale.
//!
//! A column is a block reference at its centre, turned by the reference's rotation, drawn as the section's shape and
//! filled; it has a height and the elevation of its foot. A beam is a block reference at one end of its axis, along
//! +x in its own coordinates, drawn as a band of the flange's width with the lines of the web; it has the elevation
//! of its underside, its top being that plus the section's height. Where a beam ends on a column or on another
//! beam it stops at their face ([`Drawing::cleanup_structure`]). The data travels as extended data of one
//! registered application, like the other plan objects.

use cadcraft_geom::{PolyVertex, Vec2};
use cadcraft_hvac::{SteelSection, parse_section};
use serde::{Deserialize, Serialize};

use super::part::{line, poly};
use super::{HvacPiece, HvacUnits, SheetCell, Workbook, check_label_height, check_name, invalid, label, v3, xdata};
use crate::{EntityKind, Hatch, HatchLoop, RawTag, Result};

/// The registered application name of a structural object's extended data.
pub const STRUCT_APP: &str = "CADCRAFT_STRUCT";
/// The layer of the columns and the layer of the beams.
pub const COLUMN_LAYER: &str = "S-COLS";
pub const BEAM_LAYER: &str = "S-BEAM";
const XDATA_FORMAT: &str = "STRUCT1";
/// Largest height of a column, length of a beam and elevation, mm.
pub const MAX_STRUCT_MM: f64 = 100_000.0;
/// The tolerance when a beam's end is taken to be on a column or on another beam, mm.
const TOUCH_MM: f64 = 1.0;
/// Most structural objects one cleanup looks at.
const MAX_JOINED: usize = 50_000;

fn default_label_mm() -> f64 {
    150.0
}

/// A column or a beam.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StructKind {
    #[default]
    Column,
    Beam,
}

impl StructKind {
    pub fn name(self) -> &'static str {
        match self {
            StructKind::Column => "column",
            StructKind::Beam => "beam",
        }
    }

    pub fn parse(text: &str) -> Option<StructKind> {
        let t: String = text.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphabetic()).collect();
        match t.as_str() {
            "column" | "poteau" | "post" => Some(StructKind::Column),
            "beam" | "poutre" => Some(StructKind::Beam),
            _ => None,
        }
    }

    pub fn layer(self) -> &'static str {
        match self {
            StructKind::Column => COLUMN_LAYER,
            StructKind::Beam => BEAM_LAYER,
        }
    }

    /// The prefix of the tag a new object gets ("C1", "B2").
    pub fn tag_prefix(self) -> &'static str {
        match self {
            StructKind::Column => "C",
            StructKind::Beam => "B",
        }
    }

    /// The height a new column starts with, mm (a storey), and for a beam, nothing: its length is its two points.
    pub fn default_height_mm(self) -> f64 {
        match self {
            StructKind::Column => 3000.0,
            StructKind::Beam => 0.0,
        }
    }
}

/// A structural object: what its block reference keeps to regenerate it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructParams {
    pub kind: StructKind,
    /// The tag written beside it ("C1", "B2").
    #[serde(default)]
    pub name: String,
    /// The section's designation, as the table has it ("IPN 200").
    pub section: String,
    /// A column's foot, a beam's underside: the elevation, mm from the drawing's zero.
    #[serde(default)]
    pub elevation_mm: f64,
    /// A column's height, mm; unused by a beam.
    #[serde(default)]
    pub height_mm: f64,
    /// A beam's length along +x from the block's origin, in drawing units; 0 for a column.
    #[serde(default)]
    pub length: f64,
    /// How much of a beam's length is left out at its start and at its end, mm: where it stops at a column or a beam.
    #[serde(default)]
    pub trim_start_mm: f64,
    #[serde(default)]
    pub trim_end_mm: f64,
    #[serde(default = "yes")]
    pub show_label: bool,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

fn yes() -> bool {
    true
}

impl StructParams {
    /// A column of section `section`, with a storey's height.
    pub fn column(name: &str, section: &SteelSection, units: HvacUnits) -> StructParams {
        StructParams {
            kind: StructKind::Column,
            name: name.trim().to_string(),
            section: section.designation(),
            elevation_mm: 0.0,
            height_mm: StructKind::Column.default_height_mm(),
            length: 0.0,
            trim_start_mm: 0.0,
            trim_end_mm: 0.0,
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    /// A beam of section `section`, as long as `length` drawing units, its underside at 3 m.
    pub fn beam(name: &str, section: &SteelSection, length: f64, units: HvacUnits) -> StructParams {
        StructParams {
            kind: StructKind::Beam,
            name: name.trim().to_string(),
            section: section.designation(),
            elevation_mm: 3000.0,
            height_mm: 0.0,
            length,
            trim_start_mm: 0.0,
            trim_end_mm: 0.0,
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    /// The section; `None` when the table has no such one.
    pub fn section(&self) -> Option<&'static SteelSection> {
        parse_section(&self.section)
    }

    pub fn validate(&self) -> Result<()> {
        check_name("structural object", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.section().is_none() {
            return Err(invalid(format!("There is no section \"{}\" in the table.", self.section.chars().take(40).collect::<String>())));
        }
        if !(self.elevation_mm.is_finite() && self.elevation_mm.abs() <= MAX_STRUCT_MM) {
            return Err(invalid(format!("An elevation is within {MAX_STRUCT_MM} mm of the drawing's zero.")));
        }
        match self.kind {
            StructKind::Column => {
                if !(self.height_mm.is_finite() && (10.0..=MAX_STRUCT_MM).contains(&self.height_mm)) {
                    return Err(invalid(format!("A column's height is 10 to {MAX_STRUCT_MM} mm.")));
                }
            }
            StructKind::Beam => {
                let mm = self.length_mm();
                if !(self.length.is_finite() && mm > 0.0 && mm <= MAX_STRUCT_MM) {
                    return Err(invalid(format!("A beam is more than 0 and at most {MAX_STRUCT_MM} mm long.")));
                }
                let trims = [self.trim_start_mm, self.trim_end_mm];
                if !trims.iter().all(|t| t.is_finite() && *t >= 0.0) || trims.iter().sum::<f64>() >= mm {
                    return Err(invalid("A beam's two cut ends leave something of it."));
                }
            }
        }
        Ok(())
    }

    /// A beam's length, mm.
    pub fn length_mm(&self) -> f64 {
        self.length * self.units.mm_per_unit
    }

    /// The top of a beam, mm (its underside and the section's height); a column's top (its foot and its height).
    pub fn top_mm(&self) -> f64 {
        match self.kind {
            StructKind::Column => self.elevation_mm + self.height_mm,
            StructKind::Beam => self.elevation_mm + self.section().map_or(0.0, |s| s.h_mm),
        }
    }

    /// The mass, kg: the section's mass per metre by the column's height or the beam's length.
    pub fn mass_kg(&self) -> f64 {
        let metres = match self.kind {
            StructKind::Column => self.height_mm / 1000.0,
            StructKind::Beam => self.length_mm() / 1000.0,
        };
        self.section().map_or(0.0, |s| s.kg_m * metres)
    }

    /// The text beside it: the tag and the section, then the height and the foot of a column, or the underside of a beam.
    pub fn label_text(&self) -> String {
        let section = self.section().map(SteelSection::designation).unwrap_or_default();
        let head = [self.name.trim(), section.as_str()].iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        match self.kind {
            StructKind::Column => format!("{head} h={:.2} m ({:+.2} m)", self.height_mm / 1000.0, self.elevation_mm / 1000.0),
            StructKind::Beam => format!("{head} ({:+.2} m)", self.elevation_mm / 1000.0),
        }
    }

    /// The outline of the section in plan, mm, centred on the origin, the flanges along x: an I (or an H), or a channel for a UPN.
    pub fn section_outline_mm(s: &SteelSection) -> Vec<(f64, f64)> {
        let (h, b, tw, tf) = (s.h_mm / 2.0, s.b_mm / 2.0, s.tw_mm / 2.0, s.tf_mm);
        if s.family.is_channel() {
            // The web on the left, the flanges reaching right.
            let web = -b + s.tw_mm;
            vec![(-b, -h), (b, -h), (b, -h + tf), (web, -h + tf), (web, h - tf), (b, h - tf), (b, h), (-b, h)]
        } else {
            vec![
                (-b, -h),
                (b, -h),
                (b, -h + tf),
                (tw, -h + tf),
                (tw, h - tf),
                (b, h - tf),
                (b, h),
                (-b, h),
                (-b, h - tf),
                (-tw, h - tf),
                (-tw, -h + tf),
                (-b, -h + tf),
            ]
        }
    }

    /// The footprint of a column or the band of a beam, as a rectangle `(half length along x, half width along y)`, mm.
    pub fn half_extent_mm(&self) -> Option<(f64, f64)> {
        let s = self.section()?;
        match self.kind {
            StructKind::Column => Some((s.b_mm / 2.0, s.h_mm / 2.0)),
            StructKind::Beam => Some((self.length_mm() / 2.0, s.b_mm / 2.0)),
        }
    }

    /// The drawing: a column's section, filled; a beam's band with the lines of its web, and the label.
    pub fn generate(&self, flip_label: bool) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let s = self.section().ok_or_else(|| invalid("The section is not in the table."))?;
        let layer = self.kind.layer();
        let u = |mm: f64| self.units.to_units(mm);
        let mut pieces = Vec::new();
        let label_h = u(self.label_height_mm);
        let text = if self.show_label { self.label_text() } else { String::new() };
        match self.kind {
            StructKind::Column => {
                let pts: Vec<(f64, f64)> = Self::section_outline_mm(s).into_iter().map(|(x, y)| (u(x), u(y))).collect();
                pieces.push(HvacPiece { layer, kind: poly(&pts, true) });
                pieces.push(HvacPiece { layer, kind: EntityKind::Hatch(solid_hatch(&pts)) });
                if !text.is_empty() {
                    pieces.push(HvacPiece { layer, kind: label(Vec2::new(0.0, -(u(s.h_mm / 2.0) + 1.1 * label_h)), label_h, text) });
                }
            }
            StructKind::Beam => {
                let (x0, x1) = (u(self.trim_start_mm), self.length - u(self.trim_end_mm));
                let half = u(s.b_mm / 2.0);
                pieces.push(HvacPiece { layer, kind: poly(&[(x0, -half), (x1, -half), (x1, half), (x0, half)], true) });
                // The web seen from above, two lines along the beam, when the flange is wider than it.
                let web = u(s.tw_mm / 2.0);
                if web < half * 0.9 {
                    for y in [-web, web] {
                        pieces.push(HvacPiece { layer, kind: line((x0, y), (x1, y)) });
                    }
                }
                if !text.is_empty() {
                    let up = if flip_label { -1.0 } else { 1.0 };
                    let mut t = label(Vec2::new(self.length / 2.0, up * (half + 1.1 * label_h)), label_h, text);
                    if let EntityKind::Text(x) = &mut t {
                        x.rotation = if flip_label { std::f64::consts::PI } else { 0.0 };
                    }
                    pieces.push(HvacPiece { layer, kind: t });
                }
            }
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(STRUCT_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<StructParams> {
        let p: StructParams = xdata::from_xdata(STRUCT_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

fn solid_hatch(pts: &[(f64, f64)]) -> Hatch {
    Hatch {
        pattern: "SOLID".into(),
        solid: true,
        loops: vec![HatchLoop { vertices: pts.iter().map(|&(x, y)| PolyVertex::new(Vec2::new(x, y))).collect(), outer: true, sources: Vec::new() }],
        scale: 1.0,
        angle: 0.0,
        associative: false,
        style: 0,
        elevation: 0.0,
        gradient: None,
        origin: Vec2::ZERO,
        background: None,
        pattern_lines: Vec::new(),
    }
}

/// The structural object an entity is, when it is a block reference carrying usable data.
pub fn struct_of(e: &crate::Entity) -> Option<StructParams> {
    match &e.kind {
        EntityKind::Insert(_) => StructParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour of a structural layer: a warm red for the columns, a blue for the beams.
pub(super) fn structure_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        COLUMN_LAYER => Some(Color::True(Rgb(0xd0, 0x50, 0x40))),
        BEAM_LAYER => Some(Color::True(Rgb(0x40, 0x80, 0xd0))),
        _ => None,
    }
}

/// A structural object placed in the drawing, with what [`Drawing::cleanup_structure`] needs: its handle, its parameters, its place
/// and its direction in drawing units and radians.
struct Placed {
    handle: crate::Handle,
    p: StructParams,
    at: Vec2,
    rotation: f64,
}

impl Placed {
    /// A beam's two ends in the drawing.
    fn ends(&self) -> (Vec2, Vec2) {
        let d = Vec2::new(self.rotation.cos(), self.rotation.sin());
        (self.at, self.at + d * self.p.length)
    }
}

/// How far, along `dir` from `from` (a unit vector), a ray stays inside the column's footprint: 0 when `from` is not inside it.
/// All in mm.
fn run_inside_column(col: &Placed, from_mm: Vec2, dir: Vec2) -> f64 {
    let Some((hx, hy)) = col.p.half_extent_mm() else { return 0.0 };
    let m = col.p.units.mm_per_unit;
    let (c, s) = (col.rotation.cos(), col.rotation.sin());
    // To the column's own axes.
    let rel = from_mm - col.at * m;
    let local = |v: Vec2| Vec2::new(v.x * c + v.y * s, -v.x * s + v.y * c);
    let (p, d) = (local(rel), local(dir));
    if p.x.abs() > hx + TOUCH_MM || p.y.abs() > hy + TOUCH_MM {
        return 0.0;
    }
    // The ray leaves the rectangle at the nearest of its far sides in the direction of travel.
    let exit = |p: f64, d: f64, h: f64| if d.abs() < 1e-12 { f64::INFINITY } else { ((if d > 0.0 { h } else { -h }) - p) / d };
    exit(p.x, d.x, hx).min(exit(p.y, d.y, hy)).max(0.0)
}

/// How far a beam end at `from_mm` going along `dir` is inside the band of `other` (a beam it meets side-on), mm: 0 when it is not on the
/// other's span or runs along it.
fn run_inside_beam(other: &Placed, from_mm: Vec2, dir: Vec2) -> f64 {
    let Some(s) = other.p.section() else { return 0.0 };
    let m = other.p.units.mm_per_unit;
    let (a, b) = other.ends();
    let axis = (b - a) * m;
    let len = axis.len();
    if len < 1e-9 {
        return 0.0;
    }
    let u = axis / len;
    let n = Vec2::new(-u.y, u.x);
    let rel = from_mm - a * m;
    let (along, side) = (rel.dot(u), rel.dot(n));
    let half = s.b_mm / 2.0;
    // On the other's span, within its band, and not along it.
    if along < -TOUCH_MM || along > len + TOUCH_MM || side.abs() > half + TOUCH_MM {
        return 0.0;
    }
    let slope = dir.dot(n);
    if slope.abs() < 0.2 {
        return 0.0;
    }
    ((if slope > 0.0 { half } else { -half }) - side) / slope
}

/// The point where two segments cross, or `None` when they are parallel or miss each other (a touch within `slack` of an end counts).
fn crossing(a: Vec2, b: Vec2, c: Vec2, d: Vec2, slack: f64) -> Option<Vec2> {
    let (r, q) = (b - a, d - c);
    let den = r.x * q.y - r.y * q.x;
    if den.abs() < 1e-12 {
        return None;
    }
    let w = c - a;
    let t = (w.x * q.y - w.y * q.x) / den;
    let u = (w.x * r.y - w.y * r.x) / den;
    let (lt, lu) = (slack / r.len().max(1e-12), slack / q.len().max(1e-12));
    ((-lt..=1.0 + lt).contains(&t) && (-lu..=1.0 + lu).contains(&u)).then(|| a + r * t)
}

/// How far, drawing units, `p` is from the segment `a`-`b`.
fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.dot(ab);
    let t = if l2 < 1e-24 { 0.0 } else { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) };
    p.dist(a + ab * t)
}

impl crate::Drawing {
    /// The corner of the walls of `space` nearest to `p` within `tol` drawing units: a vertex of a wall's axis, or the point where the axes of
    /// two walls cross (an L, a T or an X). Only straight segments count. `None` when no wall has a corner that close. A column placed
    /// there sits on the junction of the walls.
    pub fn wall_corner_near(&self, space: &crate::Space, p: Vec2, tol: f64) -> Option<Vec2> {
        if !p.is_finite() || !tol.is_finite() || tol <= 0.0 {
            return None;
        }
        // The axes of the walls that pass within `tol` of `p`, in the drawing's coordinates.
        let mut axes: Vec<Vec<Vec2>> = Vec::new();
        for e in self.space(space)?.iter() {
            let (Some(w), EntityKind::Insert(i)) = (crate::wall_of(e), &e.kind) else { continue };
            let (c, s) = (i.rotation.cos(), i.rotation.sin());
            let world = |v: &Vec2| {
                let (x, y) = (v.x * i.scale.x, v.y * i.scale.y);
                Vec2::new(i.insert.x + x * c - y * s, i.insert.y + x * s + y * c)
            };
            let path: Vec<Vec2> = w.path.iter().map(world).chain(w.closed.then(|| w.path.first().map(world)).flatten()).collect();
            if path.len() >= 2 && path.windows(2).any(|s| distance_to_segment(p, s[0], s[1]) <= tol) {
                axes.push(path);
            }
            if axes.len() >= 64 {
                break;
            }
        }
        let mut candidates: Vec<Vec2> = axes.iter().flatten().copied().collect();
        for (k, a) in axes.iter().enumerate() {
            for b in axes.iter().skip(k + 1) {
                for sa in a.windows(2) {
                    for sb in b.windows(2) {
                        candidates.extend(crossing(sa[0], sa[1], sb[0], sb[1], 0.0));
                    }
                }
            }
        }
        candidates.into_iter().filter(|c| c.is_finite() && c.dist(p) <= tol).min_by(|a, b| a.dist(p).total_cmp(&b.dist(p)))
    }

    /// The point of the structure nearest to `p` within `tol` drawing units: the centre of a column, an end of a beam, or a corner of the
    /// walls ([`Self::wall_corner_near`]). Where a column or a beam is placed so that it lands on what is already there.
    pub fn structure_point_near(&self, space: &crate::Space, p: Vec2, tol: f64) -> Option<Vec2> {
        if !p.is_finite() || !tol.is_finite() || tol <= 0.0 {
            return None;
        }
        let mut best: Option<Vec2> = None;
        let mut consider = |c: Vec2| {
            if c.is_finite() && c.dist(p) <= tol && best.is_none_or(|b| c.dist(p) < b.dist(p)) {
                best = Some(c);
            }
        };
        for (n, e) in self.space(space)?.iter().enumerate() {
            if n >= MAX_JOINED * 4 {
                break;
            }
            let (Some(o), EntityKind::Insert(i)) = (struct_of(e), &e.kind) else { continue };
            consider(i.insert.xy());
            if o.kind == StructKind::Beam {
                let d = Vec2::new(i.rotation.cos(), i.rotation.sin());
                consider(i.insert.xy() + d * o.length);
            }
        }
        // A corner of the walls wins over nothing, never over a column or a beam end that is closer.
        match (best, self.wall_corner_near(space, p, tol)) {
            (Some(a), Some(b)) => Some(if b.dist(p) < a.dist(p) { b } else { a }),
            (a, b) => a.or(b),
        }
    }

    /// Adds a column at `at`, its section's x axis turned by `rotation` (radians).
    pub fn add_column(&mut self, space: &crate::Space, mut common: crate::Common, p: StructParams, at: Vec2, rotation: f64) -> Result<crate::Handle> {
        if p.kind != StructKind::Column || !at.is_finite() || !rotation.is_finite() {
            return Err(invalid("A column needs a usable place."));
        }
        let pieces = p.generate(false)?;
        common.xdata = xdata::replace(STRUCT_APP, &common.xdata, p.to_xdata()?);
        common.layer = p.kind.layer().into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(p.kind.layer());
        let insert = crate::Insert { block, insert: v3(at), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Adds a beam from `a` to `b`; its `length` is set from the two points.
    pub fn add_beam(&mut self, space: &crate::Space, mut common: crate::Common, mut p: StructParams, a: Vec2, b: Vec2) -> Result<crate::Handle> {
        if p.kind != StructKind::Beam || !a.is_finite() || !b.is_finite() {
            return Err(invalid("A beam needs usable end points."));
        }
        p.length = a.dist(b);
        let rotation = (b - a).angle();
        let pieces = p.generate(super::duct::reads_upside_down(rotation))?;
        common.xdata = xdata::replace(STRUCT_APP, &common.xdata, p.to_xdata()?);
        common.layer = p.kind.layer().into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(p.kind.layer());
        let insert = crate::Insert { block, insert: v3(a), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives structural object `h` the parameters `p` (a beam's length as it is now), redrawing it in place.
    pub fn regenerate_struct(&mut self, h: crate::Handle, p: &StructParams) -> Result<()> {
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let EntityKind::Insert(i) = &e.kind else { return Err(invalid("The object is not a structural one.")) };
        let flip = p.kind == StructKind::Beam && super::duct::reads_upside_down(i.rotation);
        let pieces = p.generate(flip)?;
        let tags = xdata::replace(STRUCT_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        let layer = p.kind.layer();
        self.ensure_layer(layer);
        self.modify_entity(h, |e| e.common.layer = layer.into())?;
        Ok(())
    }

    /// Cuts each beam's ends where they meet a column (at the column's face) or another beam (at its band's edge), in `space`, and redraws the
    /// beams that changed. Returns how many were redrawn. Run after drawing or moving structural objects; it leaves alone what it cannot
    /// read.
    pub fn cleanup_structure(&mut self, space: &crate::Space) -> Result<usize> {
        let mut placed: Vec<Placed> = Vec::new();
        if let Some(store) = self.space(space) {
            for e in store.iter() {
                if let (Some(p), EntityKind::Insert(i)) = (struct_of(e), &e.kind) {
                    if placed.len() >= MAX_JOINED {
                        break;
                    }
                    placed.push(Placed { handle: e.handle, p, at: i.insert.xy(), rotation: i.rotation });
                }
            }
        }
        let columns: Vec<&Placed> = placed.iter().filter(|o| o.p.kind == StructKind::Column).collect();
        let beams: Vec<&Placed> = placed.iter().filter(|o| o.p.kind == StructKind::Beam).collect();
        let mut changes: Vec<(crate::Handle, StructParams)> = Vec::new();
        for beam in &beams {
            let m = beam.p.units.mm_per_unit;
            let (a, b) = beam.ends();
            let len_mm = beam.p.length_mm();
            if len_mm < 1e-9 {
                continue;
            }
            let dir = (b - a) * m / len_mm;
            let end_trim = |from: Vec2, toward: Vec2| -> f64 {
                let mut best = 0.0f64;
                for col in &columns {
                    best = best.max(run_inside_column(col, from * m, toward));
                }
                for other in &beams {
                    if other.handle != beam.handle {
                        best = best.max(run_inside_beam(other, from * m, toward));
                    }
                }
                // Never more than half of the beam, whichever end it is.
                best.min(len_mm / 2.0 - TOUCH_MM).max(0.0)
            };
            let mut next = beam.p.clone();
            next.trim_start_mm = end_trim(a, dir);
            next.trim_end_mm = end_trim(b, -dir);
            if (next.trim_start_mm - beam.p.trim_start_mm).abs() > 1e-6 || (next.trim_end_mm - beam.p.trim_end_mm).abs() > 1e-6 {
                changes.push((beam.handle, next));
            }
        }
        let n = changes.len();
        for (h, p) in changes {
            self.regenerate_struct(h, &p)?;
        }
        Ok(n)
    }
}

/// What one section is used for: how many pieces, the length they add up to and their mass.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionQuantity {
    pub section: String,
    pub count: usize,
    pub length_m: f64,
    pub mass_kg: f64,
}

/// What the slabs of one kind add up to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlabQuantity {
    pub kind: String,
    pub count: usize,
    pub area_m2: f64,
    pub volume_m3: f64,
    /// The openings (trémies) cut through them, and their area.
    pub holes: usize,
    pub holes_area_m2: f64,
}

/// A false ceiling's tiles.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeilingQuantity {
    pub name: String,
    pub tile: String,
    pub whole: usize,
    pub cut: usize,
    pub area_m2: f64,
}

/// The quantities of the structure of a space: the steel columns and beams by section, and the slabs by kind.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructTakeoff {
    pub columns: Vec<SectionQuantity>,
    pub beams: Vec<SectionQuantity>,
    pub slabs: Vec<SlabQuantity>,
    /// The false ceilings, the stairs (how many, and how many risers they have) and the railings (how many, and how long they are, m).
    #[serde(default)]
    pub ceilings: Vec<CeilingQuantity>,
    #[serde(default)]
    pub stairs: usize,
    #[serde(default)]
    pub risers: usize,
    #[serde(default)]
    pub railings: usize,
    #[serde(default)]
    pub railing_length_m: f64,
    /// The mass of all the steel, kg.
    pub steel_kg: f64,
    pub slab_area_m2: f64,
    pub slab_volume_m3: f64,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl StructTakeoff {
    /// The columns, then the beams, then the slabs, as the lines of a table: type, section or kind, count, length (m), mass (kg), area (m²), volume (m³).
    pub fn rows(&self) -> Vec<[String; 7]> {
        let num = |v: f64| format!("{}", round2(v));
        let mut rows = Vec::new();
        for (what, list) in [("Column", &self.columns), ("Beam", &self.beams)] {
            for q in list {
                rows.push([what.into(), q.section.clone(), q.count.to_string(), num(q.length_m), num(q.mass_kg), String::new(), String::new()]);
            }
        }
        for q in &self.slabs {
            rows.push(["Slab".into(), q.kind.clone(), q.count.to_string(), String::new(), String::new(), num(q.area_m2), num(q.volume_m3)]);
        }
        for q in &self.ceilings {
            rows.push([
                "Ceiling".into(),
                format!("{} {} ({}+{} tiles)", q.name, q.tile, q.whole, q.cut),
                "1".into(),
                String::new(),
                String::new(),
                num(q.area_m2),
                String::new(),
            ]);
        }
        if self.stairs > 0 {
            rows.push([
                "Stair".into(),
                format!("{} risers", self.risers),
                self.stairs.to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ]);
        }
        if self.railings > 0 {
            rows.push([
                "Railing".into(),
                String::new(),
                self.railings.to_string(),
                num(self.railing_length_m),
                String::new(),
                String::new(),
                String::new(),
            ]);
        }
        rows
    }

    /// The table as a text file's lines, `;` between the columns; with `comma` the decimals are written with a comma.
    pub fn csv(&self, comma: bool) -> String {
        let fix = |t: String| if comma { t.replace('.', ",") } else { t };
        let mut out = String::from("Type;Section;Count;Length (m);Mass (kg);Area (m²);Volume (m³)\r\n");
        for r in self.rows() {
            out.push_str(&r.iter().map(|c| fix(c.clone())).collect::<Vec<_>>().join(";"));
            out.push_str("\r\n");
        }
        out
    }

    /// The quantities as an Excel workbook: the summary, the steel by section and the slabs.
    pub fn to_xlsx(&self) -> Result<Vec<u8>> {
        use SheetCell as C;
        let head = |cols: &[&str]| cols.iter().map(|c| C::head(*c)).collect::<Vec<_>>();
        let num = |v: f64| C::Num(round2(v));
        let mut wb = Workbook::new();
        wb.sheet(
            "Summary",
            vec![
                vec![C::head("Columns"), C::Num(self.columns.iter().map(|q| q.count).sum::<usize>() as f64)],
                vec![C::head("Beams"), C::Num(self.beams.iter().map(|q| q.count).sum::<usize>() as f64)],
                vec![C::head("Steel (kg)"), num(self.steel_kg)],
                vec![C::head("Slabs"), C::Num(self.slabs.iter().map(|q| q.count).sum::<usize>() as f64)],
                vec![C::head("Slab area (m²)"), num(self.slab_area_m2)],
                vec![C::head("Slab volume (m³)"), num(self.slab_volume_m3)],
            ],
        )?;
        for (name, list) in [("Columns", &self.columns), ("Beams", &self.beams)] {
            let mut rows = vec![head(&["Section", "Count", "Length (m)", "Mass (kg)"])];
            rows.extend(list.iter().map(|q| vec![C::text(q.section.clone()), C::Num(q.count as f64), num(q.length_m), num(q.mass_kg)]));
            wb.sheet(name, rows)?;
        }
        let mut rows = vec![head(&["Kind", "Count", "Area (m²)", "Volume (m³)", "Openings", "Openings area (m²)"])];
        rows.extend(self.slabs.iter().map(|q| {
            vec![C::text(q.kind.clone()), C::Num(q.count as f64), num(q.area_m2), num(q.volume_m3), C::Num(q.holes as f64), num(q.holes_area_m2)]
        }));
        wb.sheet("Slabs", rows)?;
        let mut rows = vec![head(&["Ceiling", "Tile", "Whole tiles", "Cut tiles", "Area (m²)"])];
        rows.extend(
            self.ceilings
                .iter()
                .map(|q| vec![C::text(q.name.clone()), C::text(q.tile.clone()), C::Num(q.whole as f64), C::Num(q.cut as f64), num(q.area_m2)]),
        );
        wb.sheet("Ceilings", rows)?;
        wb.sheet(
            "Stairs and railings",
            vec![
                head(&["Item", "Count", "Risers", "Length (m)"]),
                vec![C::text("Stairs"), C::Num(self.stairs as f64), C::Num(self.risers as f64), C::text("")],
                vec![C::text("Railings"), C::Num(self.railings as f64), C::text(""), num(self.railing_length_m)],
            ],
        )?;
        wb.to_bytes()
    }
}

impl crate::Drawing {
    /// The quantities of the columns, the beams and the slabs of `space`.
    pub fn struct_takeoff(&self, space: &crate::Space) -> StructTakeoff {
        let mut t = StructTakeoff::default();
        let Some(store) = self.space(space) else { return t };
        for e in store.iter() {
            if let Some(o) = struct_of(e) {
                let (list, metres) = match o.kind {
                    StructKind::Column => (&mut t.columns, o.height_mm / 1000.0),
                    StructKind::Beam => (&mut t.beams, o.length_mm() / 1000.0),
                };
                let mass = o.mass_kg();
                match list.iter_mut().find(|q| q.section == o.section) {
                    Some(q) => (q.count, q.length_m, q.mass_kg) = (q.count + 1, q.length_m + metres, q.mass_kg + mass),
                    None => list.push(SectionQuantity { section: o.section.clone(), count: 1, length_m: metres, mass_kg: mass }),
                }
                t.steel_kg += mass;
            } else if let Some(c) = super::ceiling_of(e) {
                let n = c.tiles();
                t.ceilings.push(CeilingQuantity {
                    name: c.name.clone(),
                    tile: format!("{:.0}x{:.0}", c.tile_x_mm, c.tile_y_mm),
                    whole: n.whole,
                    cut: n.cut,
                    area_m2: n.area_m2,
                });
            } else if let Some(st) = super::stair_of(e) {
                t.stairs += 1;
                t.risers += st.risers();
            } else if let Some(r) = super::rail_of(e) {
                t.railings += 1;
                t.railing_length_m += r.length_mm() / 1000.0;
            } else if let Some(p) = super::slab_of(e) {
                let kind = p.kind.name().to_string();
                let (area, volume, holes) = (p.area_m2(), p.volume_m3(), p.holes_area_m2());
                t.slab_area_m2 += area;
                t.slab_volume_m3 += volume;
                match t.slabs.iter_mut().find(|q| q.kind == kind) {
                    Some(q) => {
                        q.count += 1;
                        q.area_m2 += area;
                        q.volume_m3 += volume;
                        q.holes += p.holes.len();
                        q.holes_area_m2 += holes;
                    }
                    None => {
                        t.slabs.push(SlabQuantity { kind, count: 1, area_m2: area, volume_m3: volume, holes: p.holes.len(), holes_area_m2: holes })
                    }
                }
            }
        }
        // The sections in the order of the table, the slabs in the order of their kinds.
        let order = |q: &SectionQuantity| parse_section(&q.section).map(|s| (s.family, s.number));
        t.columns.sort_by_key(order);
        t.beams.sort_by_key(order);
        t.slabs.sort_by(|a, b| a.kind.cmp(&b.kind));
        t
    }
}
