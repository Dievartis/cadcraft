//! A curtain wall in plan: a glazed façade along a path, divided by mullions every `panel_mm` and by transoms every `transom_mm` in height. Like the
//! other plan objects it is a block reference to an anonymous block, its data travelling as extended data of its own registered application.

use cadcraft_geom::{PolyVertex, Vec2};
use serde::{Deserialize, Serialize};

use super::design::MAX_DESIGN_VERTICES;
use super::part::line;
use super::{HvacPiece, HvacUnits, check_label_height, check_name, invalid, label, v3, xdata};
use crate::{EntityKind, LwPolyline, RawTag, Result};

/// The registered application name and format of a curtain wall's data.
pub const CURTAIN_APP: &str = "CADCRAFT_CURTAIN";
const CURTAIN_FORMAT: &str = "CURTAIN1";
/// The layers of the frame (mullions and the outline) and of the glass.
pub const CURTAIN_LAYER: &str = "A-GLAZ-CURT";
pub const CURTAIN_GLASS_LAYER: &str = "A-GLAZ";
/// Most mullions of one curtain wall.
const MAX_MULLIONS: usize = 4000;

fn default_label_mm() -> f64 {
    150.0
}

fn yes() -> bool {
    true
}

/// The glazed façade along a path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurtainParams {
    pub name: String,
    /// The path in the block's coordinates (drawing units), open, the first point at the origin.
    pub path: Vec<Vec2>,
    /// The height of the façade, mm.
    pub height_mm: f64,
    /// The spacing of the mullions along the path and of the transoms in height, mm.
    pub panel_mm: f64,
    pub transom_mm: f64,
    /// The depth of the frame in plan, mm.
    pub depth_mm: f64,
    /// The height of the foot above the floor, mm.
    #[serde(default)]
    pub elevation_mm: f64,
    #[serde(default = "yes")]
    pub show_label: bool,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

/// What a curtain wall is made of.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CurtainCount {
    /// The glazed panels, the mullions and their total length (m), the transoms and their total length (m), and the area of the façade (m²).
    pub panels: usize,
    pub mullions: usize,
    pub mullion_m: f64,
    pub transoms: usize,
    pub transom_m: f64,
    pub area_m2: f64,
}

impl CurtainParams {
    /// A façade of 3 m with a mullion every 1500 mm and a transom every 1200 mm along `path`, a frame 100 mm deep.
    pub fn new(name: &str, path: Vec<Vec2>, units: HvacUnits) -> CurtainParams {
        CurtainParams {
            name: name.trim().to_string(),
            path,
            height_mm: 3000.0,
            panel_mm: 1500.0,
            transom_mm: 1200.0,
            depth_mm: 100.0,
            elevation_mm: 0.0,
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("curtain wall", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.path.len() < 2
            || self.path.len() > MAX_DESIGN_VERTICES
            || self.path.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12)
        {
            return Err(invalid(format!("A curtain wall's path has 2 to {MAX_DESIGN_VERTICES} usable points.")));
        }
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !(ok(self.height_mm, 300.0, 100_000.0)
            && ok(self.panel_mm, 300.0, 10_000.0)
            && ok(self.transom_mm, 300.0, 10_000.0)
            && ok(self.depth_mm, 20.0, 1000.0))
        {
            return Err(invalid(
                "A curtain wall is 300 to 100000 mm high, with mullions 300 to 10000 mm apart, transoms 300 to 10000 mm apart and a frame 20 to 1000 mm deep.",
            ));
        }
        if !(self.elevation_mm.is_finite() && self.elevation_mm.abs() <= 1.0e5) {
            return Err(invalid("A curtain wall's elevation is not a usable number."));
        }
        let len = self.length_mm();
        if !(len.is_finite() && len > 0.0) || (len / self.panel_mm).ceil() > MAX_MULLIONS as f64 {
            return Err(invalid(format!("A curtain wall has a length and at most {MAX_MULLIONS} mullions.")));
        }
        Ok(())
    }

    /// The length of the path, mm.
    pub fn length_mm(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>() * self.units.mm_per_unit
    }

    /// The panels along the path (at least 1).
    fn bays(&self) -> usize {
        ((self.length_mm() / self.panel_mm).ceil().max(1.0) as usize).min(MAX_MULLIONS)
    }

    /// The rows of panels in height (at least 1).
    fn rows(&self) -> usize {
        ((self.height_mm / self.transom_mm).ceil().max(1.0) as usize).min(MAX_MULLIONS)
    }

    pub fn count(&self) -> CurtainCount {
        let (bays, rows) = (self.bays(), self.rows());
        let (len, h) = (self.length_mm() / 1000.0, self.height_mm / 1000.0);
        CurtainCount {
            panels: bays * rows,
            mullions: bays + 1,
            mullion_m: (bays + 1) as f64 * h,
            transoms: rows.saturating_sub(1),
            transom_m: rows.saturating_sub(1) as f64 * len,
            area_m2: len * h,
        }
    }

    /// The point `s` millimetres along the path and the unit direction of the path there.
    fn at_mm(&self, s: f64) -> Option<(Vec2, Vec2)> {
        let mut left = s / self.units.mm_per_unit;
        let mut last = None;
        for w in self.path.windows(2) {
            let len = w[0].dist(w[1]);
            if len <= 1e-12 {
                continue;
            }
            let dir = (w[1] - w[0]) * (1.0 / len);
            if left <= len + 1e-9 {
                return Some((w[0] + dir * left.clamp(0.0, len), dir));
            }
            left -= len;
            last = Some((w[1], dir));
        }
        last
    }

    pub fn label_text(&self) -> String {
        format!("{} h={:.2} m  {:.0}", self.name, self.height_mm / 1000.0, self.panel_mm)
    }

    /// The glass as a thin polyline along the path, the frame as a polyline of the frame's depth, a cross line at each mullion, and the label.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let u = |mm: f64| self.units.to_units(mm);
        let vertices =
            |path: &[Vec2]| -> Vec<PolyVertex> { path.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect() };
        let mut pieces = vec![
            HvacPiece {
                layer: CURTAIN_LAYER,
                kind: EntityKind::LwPolyline(LwPolyline {
                    vertices: vertices(&self.path),
                    closed: false,
                    const_width: u(self.depth_mm),
                    elevation: 0.0,
                    plinegen: false,
                }),
            },
            HvacPiece {
                layer: CURTAIN_GLASS_LAYER,
                kind: EntityKind::LwPolyline(LwPolyline {
                    vertices: vertices(&self.path),
                    closed: false,
                    const_width: 0.0,
                    elevation: 0.0,
                    plinegen: false,
                }),
            },
        ];
        let (bays, total) = (self.bays(), self.length_mm());
        // A mullion is a line across the frame, a little proud of it on both sides.
        let half = u(self.depth_mm) * 0.75;
        for k in 0..=bays {
            if let Some((p, dir)) = self.at_mm(total * k as f64 / bays as f64) {
                let n = Vec2::new(-dir.y, dir.x) * half;
                pieces.push(HvacPiece { layer: CURTAIN_LAYER, kind: line((p.x - n.x, p.y - n.y), (p.x + n.x, p.y + n.y)) });
            }
        }
        if self.show_label
            && let Some((mid, _)) = self.at_mm(total / 2.0)
        {
            let h = u(self.label_height_mm);
            pieces.push(HvacPiece { layer: CURTAIN_LAYER, kind: label(mid + Vec2::new(0.0, 1.5 * h), h, self.label_text()) });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(CURTAIN_APP, CURTAIN_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<CurtainParams> {
        let p: CurtainParams = xdata::from_xdata(CURTAIN_APP, CURTAIN_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The curtain wall an object is, when it is a block reference carrying usable data.
pub fn curtain_of(e: &crate::Entity) -> Option<CurtainParams> {
    match &e.kind {
        EntityKind::Insert(_) => CurtainParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour of a curtain wall's layers.
pub(super) fn curtain_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        CURTAIN_LAYER => Some(Color::True(Rgb(0x50, 0x90, 0xa0))),
        CURTAIN_GLASS_LAYER => Some(Color::True(Rgb(0x80, 0xd0, 0xe8))),
        _ => None,
    }
}

impl crate::Drawing {
    /// Adds a curtain wall along `path` (drawing coordinates).
    pub fn add_curtain(&mut self, space: &crate::Space, mut common: crate::Common, mut p: CurtainParams) -> Result<crate::Handle> {
        let at = p.path.first().copied().ok_or_else(|| invalid("A curtain wall needs a path."))?;
        for v in p.path.iter_mut() {
            *v -= at;
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(CURTAIN_APP, &common.xdata, p.to_xdata()?);
        common.layer = CURTAIN_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(CURTAIN_LAYER);
        self.ensure_layer(CURTAIN_GLASS_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives curtain wall `h` the parameters `p` (path in the block's coordinates), redrawing it in place.
    pub fn regenerate_curtain(&mut self, h: crate::Handle, p: &CurtainParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(CURTAIN_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        self.ensure_layer(CURTAIN_LAYER);
        self.ensure_layer(CURTAIN_GLASS_LAYER);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(path: &[(f64, f64)]) -> CurtainParams {
        CurtainParams::new("MR1", path.iter().map(|&(x, y)| Vec2::new(x, y)).collect(), HvacUnits::default())
    }

    #[test]
    fn a_curtain_wall_counts_its_panels_mullions_and_transoms() {
        let c = wall(&[(0.0, 0.0), (6000.0, 0.0)]);
        let n = c.count();
        // 6 m at 1500: 4 bays and 5 mullions; 3 m at 1200: 3 rows and 2 transoms.
        assert_eq!((n.panels, n.mullions, n.transoms), (12, 5, 2));
        assert!((n.area_m2 - 18.0).abs() < 1e-9 && (n.mullion_m - 15.0).abs() < 1e-9 && (n.transom_m - 12.0).abs() < 1e-9, "{n:?}");
    }

    #[test]
    fn a_curtain_wall_is_drawn_with_a_mullion_line_at_each_division_and_survives_its_data() {
        let c = wall(&[(0.0, 0.0), (3000.0, 0.0), (3000.0, 3000.0)]);
        let lines = c.generate().unwrap().iter().filter(|p| matches!(p.kind, EntityKind::Line(_))).count();
        assert_eq!(lines, 5, "6 m at 1500 mm: 4 bays, 5 mullions");
        let tags = c.to_xdata().unwrap();
        assert_eq!(CurtainParams::from_xdata(&tags), Some(c));
    }

    #[test]
    fn hostile_curtain_walls_are_refused() {
        let mut c = wall(&[(0.0, 0.0), (6000.0, 0.0)]);
        for bad in [f64::NAN, -5.0, 1.0e12] {
            c.panel_mm = bad;
            assert!(c.validate().is_err());
        }
        c.panel_mm = 1500.0;
        assert!(wall(&[(0.0, 0.0), (0.0, 0.0)]).validate().is_err());
        assert!(wall(&[(0.0, 0.0), (1.0e11, 0.0)]).validate().is_err(), "more than 4000 mullions");
        assert!(wall(&[(0.0, 0.0)]).validate().is_err());
    }
}
