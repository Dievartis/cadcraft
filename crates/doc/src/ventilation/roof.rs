//! A roof in plan: a shed, a gable or a hip roof over an outline, with its ridge, its hips, the arrows of the falls, and the quantities (plan area, true area,
//! ridge height). Like the other plan objects it is a block reference to an anonymous block, its data travelling as extended data of its own application.

use cadcraft_geom::{Vec2, shoelace};
use serde::{Deserialize, Serialize};

use super::design::MAX_DESIGN_VERTICES;
use super::part::{line, poly};
use super::{HvacPiece, HvacUnits, check_label_height, check_name, invalid, label, v3, xdata};
use crate::{EntityKind, RawTag, Result};

pub const ROOF_APP: &str = "CADCRAFT_ROOF";
const ROOF_FORMAT: &str = "ROOF1";
pub const ROOF_LAYER: &str = "A-ROOF";

fn default_label_mm() -> f64 {
    150.0
}

fn yes() -> bool {
    true
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// The most a roof stands out beyond the walls, mm.
pub const MAX_OVERHANG_MM: f64 = 5_000.0;

/// `outline` moved outward by `d` (drawing units; a negative one moves it in): each corner is where the two edges moved by `d` meet (a mitre, no longer than
/// four times `d` at a sharp corner). It is meant for the plain outlines of buildings: a deep notch with a large `d` may fold on itself.
fn offset_outline(outline: &[Vec2], d: f64) -> Vec<Vec2> {
    if d == 0.0 || outline.len() < 3 {
        return outline.to_vec();
    }
    // The outward side of an edge is its right for a counter-clockwise outline and its left for a clockwise one.
    let sign = if shoelace(outline) >= 0.0 { 1.0 } else { -1.0 };
    let n = outline.len();
    let normal = |a: Vec2, b: Vec2| {
        let e = b - a;
        let len = e.len();
        if len <= 1e-12 { Vec2::ZERO } else { Vec2::new(e.y, -e.x) / len * sign }
    };
    (0..n)
        .map(|i| {
            let (prev, at, next) = (outline[(i + n - 1) % n], outline[i], outline[(i + 1) % n]);
            let (n0, n1) = (normal(prev, at), normal(at, next));
            let k = 1.0 + n0.dot(n1);
            let mut v = if k.abs() < 1e-9 { n0 } else { (n0 + n1) / k };
            if v.len() > 4.0 {
                v = v / v.len() * 4.0;
            }
            at + v * d
        })
        .collect()
}

/// The shape of a roof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoofKind {
    /// One slope, rising towards one side.
    Shed,
    /// Two slopes meeting at a ridge along the middle.
    #[default]
    Gable,
    /// Four slopes: the ridge is shortened by a hip at each end.
    Hip,
}

impl RoofKind {
    pub const ALL: [RoofKind; 3] = [RoofKind::Shed, RoofKind::Gable, RoofKind::Hip];

    pub fn name(self) -> &'static str {
        match self {
            RoofKind::Shed => "Shed",
            RoofKind::Gable => "Gable",
            RoofKind::Hip => "Hip",
        }
    }

    pub fn parse(text: &str) -> Option<RoofKind> {
        match text.trim().to_ascii_lowercase().as_str() {
            "shed" | "mono" | "monopitch" | "lean-to" => Some(RoofKind::Shed),
            "gable" | "dual" | "duopitch" | "pitched" => Some(RoofKind::Gable),
            "hip" | "hipped" => Some(RoofKind::Hip),
            _ => None,
        }
    }
}

/// A roof over an outline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoofParams {
    pub name: String,
    pub kind: RoofKind,
    /// The outline at the eaves in the block's coordinates (drawing units), the first point at the origin.
    pub outline: Vec<Vec2>,
    /// The slope, degrees from the horizontal.
    pub pitch_deg: f64,
    /// The direction of the ridge, degrees from the x axis (of the fall's side for a shed: the fall goes across it).
    pub angle_deg: f64,
    /// The height of the eaves above the floor, mm: where the roof's planes start, on the line of the outline (the wall plate).
    #[serde(default)]
    pub elevation_mm: f64,
    /// How far the roof stands out beyond the outline all round, mm (the outline is the walls': the roof's own edge is that outline moved out by this).
    /// The planes go on past the walls at the same slope, so the edge of the roof is lower than the wall plate by this times the slope.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub overhang_mm: f64,
    #[serde(default = "yes")]
    pub show_label: bool,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

/// What a roof is: its areas and heights.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoofCount {
    /// The area in plan, and the true area of the slopes, m².
    pub plan_m2: f64,
    pub area_m2: f64,
    /// The rise from the eaves to the top, m, and the length of the ridge, m (0 for a shed or a pyramid).
    pub rise_m: f64,
    pub ridge_m: f64,
}

/// The frame of a roof: the ridge's direction and across it, with the extent of an outline in them.
struct Frame {
    u: Vec2,
    v: Vec2,
    u_min: f64,
    u_max: f64,
    v_min: f64,
    v_max: f64,
}

impl Frame {
    fn at(&self, u: f64, v: f64) -> Vec2 {
        self.u * u + self.v * v
    }
}

impl RoofParams {
    /// A gable roof of 30° over `outline`, the ridge along the first side.
    pub fn new(name: &str, outline: Vec<Vec2>, units: HvacUnits) -> RoofParams {
        let angle_deg = match (outline.first(), outline.get(1)) {
            (Some(a), Some(b)) => (*b - *a).angle().to_degrees(),
            _ => 0.0,
        };
        RoofParams {
            name: name.trim().to_string(),
            kind: RoofKind::Gable,
            outline,
            pitch_deg: 30.0,
            angle_deg,
            elevation_mm: 3000.0,
            overhang_mm: 0.0,
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("roof", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.outline.len() < 3
            || self.outline.len() > MAX_DESIGN_VERTICES
            || self.outline.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12)
        {
            return Err(invalid(format!("A roof's outline has 3 to {MAX_DESIGN_VERTICES} usable points.")));
        }
        if !(self.pitch_deg.is_finite() && (1.0..=80.0).contains(&self.pitch_deg)) || !self.angle_deg.is_finite() {
            return Err(invalid("A roof's slope is 1 to 80 degrees."));
        }
        if !(self.overhang_mm.is_finite() && (0.0..=MAX_OVERHANG_MM).contains(&self.overhang_mm)) {
            return Err(invalid(format!("A roof's overhang is 0 to {MAX_OVERHANG_MM} mm.")));
        }
        if !(self.elevation_mm.is_finite() && self.elevation_mm.abs() <= 1.0e5) {
            return Err(invalid("A roof's eaves height is not a usable number."));
        }
        if shoelace(&self.outline).abs() * self.units.mm_per_unit * self.units.mm_per_unit <= 1.0 {
            return Err(invalid("A roof's outline encloses an area."));
        }
        Ok(())
    }

    /// The edge of the roof: the outline moved out by the overhang (the outline itself when there is none).
    pub fn eaves_outline(&self) -> Vec<Vec2> {
        offset_outline(&self.outline, self.units.to_units(self.overhang_mm))
    }

    /// The frame of the walls' outline, which gives the rise.
    fn frame(&self) -> Frame {
        self.frame_of(&self.outline)
    }

    fn frame_of(&self, outline: &[Vec2]) -> Frame {
        let a = self.angle_deg.to_radians();
        let (u, v) = (Vec2::new(a.cos(), a.sin()), Vec2::new(-a.sin(), a.cos()));
        let (mut u_min, mut u_max, mut v_min, mut v_max) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in outline {
            let (pu, pv) = (p.dot(u), p.dot(v));
            (u_min, u_max, v_min, v_max) = (u_min.min(pu), u_max.max(pu), v_min.min(pv), v_max.max(pv));
        }
        Frame { u, v, u_min, u_max, v_min, v_max }
    }

    /// Where the line `v = at` crosses the outline: its first and last points along the ridge's direction.
    fn crossing(outline: &[Vec2], f: &Frame, at: f64) -> Option<(f64, f64)> {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for (i, a) in outline.iter().enumerate() {
            let b = outline[(i + 1) % outline.len()];
            let (av, bv) = (a.dot(f.v), b.dot(f.v));
            if (av - at) * (bv - at) <= 0.0 && (bv - av).abs() > 1e-12 {
                let u = a.dot(f.u) + (at - av) / (bv - av) * (b.dot(f.u) - a.dot(f.u));
                (lo, hi) = (lo.min(u), hi.max(u));
            }
        }
        (lo <= hi).then_some((lo, hi))
    }

    pub fn count(&self) -> RoofCount {
        let k = self.units.mm_per_unit;
        // The roof covers the edge of its eaves; the rise is measured from the walls' outline, where the planes start, and the ridge of a gable runs to the
        // edge of the roof at each end.
        let eaves = self.eaves_outline();
        let plan_m2 = shoelace(&eaves).abs() * k * k / 1.0e6;
        let (f, fe) = (self.frame(), self.frame_of(&eaves));
        let tan = self.pitch_deg.to_radians().tan();
        let (uspan, vspan) = ((f.u_max - f.u_min) * k / 1000.0, (f.v_max - f.v_min) * k / 1000.0);
        let (rise_m, ridge_m) = match self.kind {
            RoofKind::Shed => (vspan * tan, 0.0),
            RoofKind::Gable => {
                (vspan / 2.0 * tan, Self::crossing(&eaves, &fe, (fe.v_min + fe.v_max) / 2.0).map_or(0.0, |(lo, hi)| (hi - lo) * k / 1000.0))
            }
            RoofKind::Hip => (uspan.min(vspan) / 2.0 * tan, (uspan - vspan).max(0.0)),
        };
        RoofCount { plan_m2, area_m2: plan_m2 / self.pitch_deg.to_radians().cos(), rise_m, ridge_m }
    }

    pub fn label_text(&self) -> String {
        let n = self.count();
        format!("{} {} {:.0}\u{b0} top {:+.2} m", self.name, self.kind.name(), self.pitch_deg, self.elevation_mm / 1000.0 + n.rise_m)
    }

    /// The outline, the ridge and the hips, the arrows of the falls, and the label.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        // The roof is drawn on the edge of its eaves; the walls' outline is drawn inside it when the roof stands out beyond it.
        let eaves = self.eaves_outline();
        let f = self.frame_of(&eaves);
        let pts: Vec<(f64, f64)> = eaves.iter().map(|p| (p.x, p.y)).collect();
        let mut pieces = vec![HvacPiece { layer: ROOF_LAYER, kind: poly(&pts, true) }];
        if self.overhang_mm > 0.0 {
            let walls: Vec<(f64, f64)> = self.outline.iter().map(|p| (p.x, p.y)).collect();
            pieces.push(HvacPiece { layer: ROOF_LAYER, kind: poly(&walls, true) });
        }
        let (vm, um) = ((f.v_min + f.v_max) / 2.0, (f.u_min + f.u_max) / 2.0);
        let seg = |a: Vec2, b: Vec2| HvacPiece { layer: ROOF_LAYER, kind: line((a.x, a.y), (b.x, b.y)) };
        // An arrow of a fall: from `at` along `dir`, with a head.
        let size = ((f.u_max - f.u_min).min(f.v_max - f.v_min) / 6.0).max(1e-9);
        let arrow = |pieces: &mut Vec<HvacPiece>, at: Vec2, dir: Vec2| {
            let tip = at + dir * size;
            let side = Vec2::new(-dir.y, dir.x);
            pieces.push(seg(at, tip));
            pieces.push(seg(tip, tip - dir * (size * 0.3) + side * (size * 0.18)));
            pieces.push(seg(tip, tip - dir * (size * 0.3) - side * (size * 0.18)));
        };
        match self.kind {
            RoofKind::Shed => arrow(&mut pieces, f.at(um, vm + size / 2.0), -f.v),
            RoofKind::Gable => {
                if let Some((lo, hi)) = Self::crossing(&eaves, &f, vm) {
                    pieces.push(seg(f.at(lo, vm), f.at(hi, vm)));
                }
                arrow(&mut pieces, f.at(um, vm - size * 0.4), -f.v);
                arrow(&mut pieces, f.at(um, vm + size * 0.4), f.v);
            }
            RoofKind::Hip => {
                let h = ((f.u_max - f.u_min).min(f.v_max - f.v_min)) / 2.0;
                let (r0, r1) = (f.u_min + h, (f.u_max - h).max(f.u_min + h));
                pieces.push(seg(f.at(r0, vm), f.at(r1, vm)));
                for (cu, ru) in [(f.u_min, r0), (f.u_max, r1)] {
                    for cv in [f.v_min, f.v_max] {
                        pieces.push(seg(f.at(cu, cv), f.at(ru, vm)));
                    }
                }
                arrow(&mut pieces, f.at(um, vm - size * 0.4), -f.v);
                arrow(&mut pieces, f.at(um, vm + size * 0.4), f.v);
            }
        }
        if self.show_label {
            let h = self.units.to_units(self.label_height_mm);
            pieces.push(HvacPiece { layer: ROOF_LAYER, kind: label(f.at(um, vm) + Vec2::new(0.0, -1.5 * h), h, self.label_text()) });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(ROOF_APP, ROOF_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<RoofParams> {
        let p: RoofParams = xdata::from_xdata(ROOF_APP, ROOF_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The roof an object is, when it is a block reference carrying usable data.
pub fn roof_of(e: &crate::Entity) -> Option<RoofParams> {
    match &e.kind {
        EntityKind::Insert(_) => RoofParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

pub(super) fn roof_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    (name == ROOF_LAYER).then_some(Color::True(Rgb(0xc0, 0x70, 0x58)))
}

impl crate::Drawing {
    /// Adds a roof over `p.outline` (drawing coordinates).
    pub fn add_roof(&mut self, space: &crate::Space, mut common: crate::Common, mut p: RoofParams) -> Result<crate::Handle> {
        let at = p.outline.first().copied().ok_or_else(|| invalid("A roof needs an outline."))?;
        for v in p.outline.iter_mut() {
            *v -= at;
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(ROOF_APP, &common.xdata, p.to_xdata()?);
        common.layer = ROOF_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(ROOF_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives roof `h` the parameters `p` (outline in the block's coordinates), redrawing it in place.
    pub fn regenerate_roof(&mut self, h: crate::Handle, p: &RoofParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(ROOF_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        self.ensure_layer(ROOF_LAYER);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> RoofParams {
        let o = vec![Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)];
        RoofParams::new("T1", o, HvacUnits::default())
    }

    #[test]
    fn a_gable_roof_rises_by_half_its_span_times_the_slope() {
        let r = rect(10000.0, 6000.0);
        let n = r.count();
        // The ridge runs along the first side (10 m); the span across is 6 m: 3 m × tan 30°.
        assert!((n.rise_m - 3.0 * 30f64.to_radians().tan()).abs() < 1e-9, "{n:?}");
        assert!((n.ridge_m - 10.0).abs() < 1e-9 && (n.plan_m2 - 60.0).abs() < 1e-9);
        assert!((n.area_m2 - 60.0 / 30f64.to_radians().cos()).abs() < 1e-9);
    }

    #[test]
    fn a_hip_roof_shortens_its_ridge_by_the_span_and_a_shed_has_none() {
        let mut r = rect(10000.0, 6000.0);
        r.kind = RoofKind::Hip;
        let n = r.count();
        assert!((n.ridge_m - 4.0).abs() < 1e-9, "10 - 6: {n:?}");
        r.kind = RoofKind::Shed;
        let n = r.count();
        assert!(n.ridge_m == 0.0 && (n.rise_m - 6.0 * 30f64.to_radians().tan()).abs() < 1e-9, "{n:?}");
        // A square hip roof is a pyramid: no ridge.
        let mut p = rect(6000.0, 6000.0);
        p.kind = RoofKind::Hip;
        assert_eq!(p.count().ridge_m, 0.0);
    }

    #[test]
    fn each_kind_draws_and_keeps_its_data() {
        for kind in RoofKind::ALL {
            let mut r = rect(8000.0, 5000.0);
            r.kind = kind;
            let pieces = r.generate().unwrap();
            let lines = pieces.iter().filter(|p| matches!(p.kind, EntityKind::Line(_))).count();
            // Shed: 3 (an arrow). Gable: ridge + 2 arrows. Hip: ridge + 4 hips + 2 arrows.
            assert_eq!(
                lines,
                match kind {
                    RoofKind::Shed => 3,
                    RoofKind::Gable => 7,
                    RoofKind::Hip => 11,
                },
                "{kind:?}"
            );
            assert_eq!(RoofParams::from_xdata(&r.to_xdata().unwrap()), Some(r));
        }
    }

    #[test]
    fn hostile_roofs_are_refused() {
        let mut r = rect(8000.0, 5000.0);
        for pitch in [f64::NAN, 0.0, 90.0, -3.0] {
            r.pitch_deg = pitch;
            assert!(r.validate().is_err());
        }
        r.pitch_deg = 30.0;
        r.angle_deg = f64::INFINITY;
        assert!(r.validate().is_err());
        let flat = RoofParams::new("T", vec![Vec2::new(0.0, 0.0), Vec2::new(1000.0, 0.0), Vec2::new(2000.0, 0.0)], HvacUnits::default());
        assert!(flat.validate().is_err());
    }
}

#[cfg(test)]
mod overhang_tests {
    use super::*;

    fn rect(w: f64, h: f64, overhang: f64) -> RoofParams {
        let o = vec![Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)];
        RoofParams { overhang_mm: overhang, ..RoofParams::new("T1", o, HvacUnits::default()) }
    }

    #[test]
    fn the_edge_of_the_roof_is_the_outline_moved_out_all_round_whatever_its_direction() {
        let mut r = rect(10000.0, 6000.0, 500.0);
        let e = r.eaves_outline();
        let (xs, ys): (Vec<f64>, Vec<f64>) = e.iter().map(|p| (p.x, p.y)).unzip();
        let ext = |v: &[f64]| (v.iter().cloned().fold(f64::MAX, f64::min), v.iter().cloned().fold(f64::MIN, f64::max));
        assert_eq!((ext(&xs), ext(&ys)), ((-500.0, 10500.0), (-500.0, 6500.0)));
        // Clockwise gives the same edge.
        r.outline.reverse();
        let e2 = r.eaves_outline();
        let (xs2, ys2): (Vec<f64>, Vec<f64>) = e2.iter().map(|p| (p.x, p.y)).unzip();
        assert_eq!((ext(&xs2), ext(&ys2)), ((-500.0, 10500.0), (-500.0, 6500.0)));
        // An L-shaped outline moves out on every side, its inner corner too.
        let l = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(8000.0, 0.0),
            Vec2::new(8000.0, 4000.0),
            Vec2::new(4000.0, 4000.0),
            Vec2::new(4000.0, 8000.0),
            Vec2::new(0.0, 8000.0),
        ];
        let o = offset_outline(&l, 500.0);
        assert!(o.contains(&Vec2::new(-500.0, -500.0)) && o.contains(&Vec2::new(8500.0, 4500.0)) && o.contains(&Vec2::new(4500.0, 4500.0)), "{o:?}");
        assert!(shoelace(&o).abs() > shoelace(&l).abs());
    }

    #[test]
    fn the_roof_covers_its_edge_the_ridge_runs_to_it_and_the_top_stays_where_the_walls_give_it() {
        let (plain, over) = (rect(10000.0, 6000.0, 0.0), rect(10000.0, 6000.0, 500.0));
        let (a, b) = (plain.count(), over.count());
        // 11 m x 7 m is roofed instead of 10 m x 6 m, along a ridge of 11 m; the rise is from the walls' 6 m span.
        assert!((b.plan_m2 - 77.0).abs() < 1e-9 && (b.ridge_m - 11.0).abs() < 1e-9, "{b:?}");
        assert!((b.rise_m - a.rise_m).abs() < 1e-12);
        assert!((b.area_m2 - 77.0 / 30f64.to_radians().cos()).abs() < 1e-9);
        assert_eq!(plain.label_text().replace("T1", ""), over.label_text().replace("T1", ""), "the top is the same");
        // A hip roof's ridge does not move: its hips start at the corners of the edge.
        let (mut h0, mut h1) = (plain.clone(), over.clone());
        (h0.kind, h1.kind) = (RoofKind::Hip, RoofKind::Hip);
        assert!((h0.count().ridge_m - h1.count().ridge_m).abs() < 1e-9);
        // The walls' outline is drawn inside the edge, and the data keeps the overhang.
        let polys = |r: &RoofParams| r.generate().unwrap().iter().filter(|p| matches!(p.kind, EntityKind::LwPolyline(_))).count();
        assert_eq!((polys(&plain), polys(&over)), (1, 2));
        assert_eq!(RoofParams::from_xdata(&over.to_xdata().unwrap()), Some(over.clone()));
        // Without one, the data is what it was before the overhang existed.
        assert!(!String::from_utf8_lossy(&serde_json::to_vec(&plain).unwrap()).contains("overhang"));
    }

    #[test]
    fn an_overhang_beyond_the_limits_is_refused() {
        for v in [-1.0, f64::NAN, MAX_OVERHANG_MM + 1.0, f64::INFINITY] {
            assert!(rect(8000.0, 5000.0, v).validate().is_err(), "{v}");
        }
        assert!(rect(8000.0, 5000.0, MAX_OVERHANG_MM).validate().is_ok());
    }
}
