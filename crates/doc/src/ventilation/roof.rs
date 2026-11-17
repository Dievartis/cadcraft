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
    /// The height of the eaves above the floor, mm.
    #[serde(default)]
    pub elevation_mm: f64,
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

/// The frame of a roof: the ridge's direction and across it, with the extent of the outline in them.
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
        if !(self.elevation_mm.is_finite() && self.elevation_mm.abs() <= 1.0e5) {
            return Err(invalid("A roof's eaves height is not a usable number."));
        }
        if shoelace(&self.outline).abs() * self.units.mm_per_unit * self.units.mm_per_unit <= 1.0 {
            return Err(invalid("A roof's outline encloses an area."));
        }
        Ok(())
    }

    fn frame(&self) -> Frame {
        let a = self.angle_deg.to_radians();
        let (u, v) = (Vec2::new(a.cos(), a.sin()), Vec2::new(-a.sin(), a.cos()));
        let (mut u_min, mut u_max, mut v_min, mut v_max) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in &self.outline {
            let (pu, pv) = (p.dot(u), p.dot(v));
            (u_min, u_max, v_min, v_max) = (u_min.min(pu), u_max.max(pu), v_min.min(pv), v_max.max(pv));
        }
        Frame { u, v, u_min, u_max, v_min, v_max }
    }

    /// Where the line `v = at` crosses the outline: its first and last points along the ridge's direction.
    fn crossing(&self, f: &Frame, at: f64) -> Option<(f64, f64)> {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for (i, a) in self.outline.iter().enumerate() {
            let b = self.outline[(i + 1) % self.outline.len()];
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
        let plan_m2 = shoelace(&self.outline).abs() * k * k / 1.0e6;
        let f = self.frame();
        let tan = self.pitch_deg.to_radians().tan();
        let (uspan, vspan) = ((f.u_max - f.u_min) * k / 1000.0, (f.v_max - f.v_min) * k / 1000.0);
        let (rise_m, ridge_m) = match self.kind {
            RoofKind::Shed => (vspan * tan, 0.0),
            RoofKind::Gable => (vspan / 2.0 * tan, self.crossing(&f, (f.v_min + f.v_max) / 2.0).map_or(0.0, |(lo, hi)| (hi - lo) * k / 1000.0)),
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
        let f = self.frame();
        let pts: Vec<(f64, f64)> = self.outline.iter().map(|p| (p.x, p.y)).collect();
        let mut pieces = vec![HvacPiece { layer: ROOF_LAYER, kind: poly(&pts, true) }];
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
                if let Some((lo, hi)) = self.crossing(&f, vm) {
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
