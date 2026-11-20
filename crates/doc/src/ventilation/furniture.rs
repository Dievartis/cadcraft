//! Furniture and appliances: sofas, tables, beds, desks, cabinets, refrigerators, ovens, televisions, computers… Each is a block reference drawn in plan at its
//! size, seen from above, with its parameters in extended data: its model, its size and height, the altitude of its underside and, for an appliance, the power it
//! draws. The models are a catalogue written in code (see `furniture_catalog.rs`): each is a few shapes in fractions of its size, so a piece can be made wider
//! or deeper than usual and keeps its look.
//!
//! The block's origin is the middle of the side against the wall: the piece stands toward the block's -y, as the sanitary fixtures do.

use cadcraft_geom::{PolyVertex, Vec2};
use serde::{Deserialize, Serialize};

use super::furniture_catalog::{GROUPS, MODELS};
use super::{HvacPiece, HvacUnits, check_altitude, check_label_height, invalid, label, v3, xdata};
use crate::{Ellipse, EntityKind, Line, LwPolyline, RawTag, Result};

pub const FURNITURE_APP: &str = "CADCRAFT_FURNITURE";
const FURNITURE_FORMAT: &str = "FURNITURE1";
/// The layer of the furniture.
pub const FURNITURE_LAYER: &str = "A-FURN";
/// The largest a piece is, mm, and the most power an appliance draws, W.
const MAX_SIZE_MM: f64 = 20_000.0;
const MAX_POWER_W: f64 = 1_000_000.0;
/// The bulge of a quarter of a circle.
const QUARTER: f64 = 0.414_213_562_373_095;

/// One shape of a piece, in fractions of its width (x, from the left) and of its depth (y, from the back to the front).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle: two opposite corners.
    Rect(f64, f64, f64, f64),
    /// A rectangle with rounded corners: two opposite corners, and the radius as a fraction of the smaller side of the piece.
    Round(f64, f64, f64, f64, f64),
    /// An ellipse: its centre and its half axes.
    Oval(f64, f64, f64, f64),
    /// A line: its two ends.
    Line(f64, f64, f64, f64),
    /// A closed polygon.
    Poly(&'static [(f64, f64)]),
}

/// A model of the catalogue.
#[derive(Clone, Copy, Debug)]
pub struct FurnitureModel {
    /// What a command takes: "sofa3", "fridge", "tv55"…
    pub id: &'static str,
    /// The name in English, which is translated where it is shown.
    pub label: &'static str,
    /// The group it is listed in.
    pub group: &'static str,
    /// The usual size, mm, and the height.
    pub width_mm: f64,
    pub depth_mm: f64,
    pub height_mm: f64,
    /// The power an appliance draws, W; 0 for none.
    pub power_w: f64,
    pub shapes: &'static [Shape],
}

/// Every model of the catalogue, grouped as the tab lists them.
pub fn furniture_models() -> &'static [FurnitureModel] {
    MODELS
}

/// The groups of the catalogue, in the order the tab lists them.
pub fn furniture_groups() -> &'static [&'static str] {
    GROUPS
}

fn alnum(text: &str) -> String {
    text.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The model whose id is `id` (case does not matter).
pub fn furniture_model(id: &str) -> Option<&'static FurnitureModel> {
    let key = alnum(id);
    MODELS.iter().find(|m| alnum(m.id) == key)
}

/// The model `text` names: its id or its English name, apart from case, spaces and punctuation.
pub fn parse_furniture_model(text: &str) -> Option<&'static FurnitureModel> {
    let key = alnum(text);
    if key.is_empty() {
        return None;
    }
    furniture_model(text).or_else(|| MODELS.iter().find(|m| alnum(m.label) == key))
}

fn default_label_mm() -> f64 {
    100.0
}

/// A piece of furniture: what its block reference keeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureParams {
    /// The id of its model in the catalogue.
    pub model: String,
    /// The text written beside it; empty for none.
    #[serde(default)]
    pub name: String,
    /// Its width along the wall and its depth into the room, and its height, mm.
    pub width_mm: f64,
    pub depth_mm: f64,
    pub height_mm: f64,
    /// The altitude of its underside, mm above the drawing's zero, when it is not on the floor.
    #[serde(default)]
    pub bottom_mm: Option<f64>,
    /// The power an appliance draws, W.
    #[serde(default)]
    pub power_w: Option<f64>,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl FurnitureParams {
    /// A piece of `model` at its usual size.
    pub fn new(model: &FurnitureModel, units: HvacUnits) -> FurnitureParams {
        FurnitureParams {
            model: model.id.to_string(),
            name: String::new(),
            width_mm: model.width_mm,
            depth_mm: model.depth_mm,
            height_mm: model.height_mm,
            bottom_mm: None,
            power_w: (model.power_w > 0.0).then_some(model.power_w),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    /// Its model, when the catalogue has it.
    pub fn catalogue(&self) -> Option<&'static FurnitureModel> {
        furniture_model(&self.model)
    }

    pub fn validate(&self) -> Result<()> {
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        check_altitude(self.bottom_mm)?;
        if self.catalogue().is_none() {
            return Err(invalid("This piece of furniture is not in the catalogue."));
        }
        let size = |v: f64| v.is_finite() && (10.0..=MAX_SIZE_MM).contains(&v);
        if !size(self.width_mm) || !size(self.depth_mm) || !size(self.height_mm) {
            return Err(invalid(format!("A piece of furniture is 10 to {MAX_SIZE_MM} mm wide, deep and high.")));
        }
        if self.power_w.is_some_and(|w| !(w.is_finite() && (0.0..=MAX_POWER_W).contains(&w))) {
            return Err(invalid("The power of an appliance is 0 or more watts."));
        }
        if self.name.chars().count() > 40 {
            return Err(invalid("A piece of furniture's name is 40 characters at most."));
        }
        Ok(())
    }

    /// The piece seen from above at its size, with its name below it when it has one.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.generate_turned(false)
    }

    /// [`generate`](Self::generate), the text turned half a turn when `flip_label`: a piece turned so that its text would read upside down.
    pub fn generate_turned(&self, flip_label: bool) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let model = self.catalogue().ok_or_else(|| invalid("This piece of furniture is not in the catalogue."))?;
        let (w, d) = (self.width_mm, self.depth_mm);
        let u = |mm: f64| self.units.to_units(mm);
        // A point of the shape, in the block's coordinates (drawing units): the middle of the back is the origin, the piece goes toward -y.
        let at = |x: f64, y: f64| Vec2::new(u((x - 0.5) * w), u(-y * d));
        let vertex = |p: Vec2, bulge: f64| PolyVertex { p, bulge, start_width: 0.0, end_width: 0.0 };
        let poly = |vertices: Vec<PolyVertex>| {
            EntityKind::LwPolyline(LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false })
        };
        let mut pieces = Vec::with_capacity(model.shapes.len() + 1);
        for shape in model.shapes {
            let kind = match *shape {
                Shape::Rect(x0, y0, x1, y1) => {
                    poly(vec![vertex(at(x0, y0), 0.0), vertex(at(x1, y0), 0.0), vertex(at(x1, y1), 0.0), vertex(at(x0, y1), 0.0)])
                }
                Shape::Round(x0, y0, x1, y1, rad) => {
                    let (xa, xb, ya, yb) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
                    // The radius in mm, at most half of each side of the rectangle.
                    let r = (rad * w.min(d)).min((xb - xa) * w / 2.0).min((yb - ya) * d / 2.0);
                    if r < 1e-6 {
                        poly(vec![vertex(at(xa, ya), 0.0), vertex(at(xb, ya), 0.0), vertex(at(xb, yb), 0.0), vertex(at(xa, yb), 0.0)])
                    } else {
                        let (rx, ry) = (r / w, r / d);
                        // Counter-clockwise from the front left: a quarter arc at each corner.
                        poly(vec![
                            vertex(at(xa + rx, yb), 0.0),
                            vertex(at(xb - rx, yb), QUARTER),
                            vertex(at(xb, yb - ry), 0.0),
                            vertex(at(xb, ya + ry), QUARTER),
                            vertex(at(xb - rx, ya), 0.0),
                            vertex(at(xa + rx, ya), QUARTER),
                            vertex(at(xa, ya + ry), 0.0),
                            vertex(at(xa, yb - ry), QUARTER),
                        ])
                    }
                }
                Shape::Oval(cx, cy, rx, ry) => {
                    let (hx, hy) = (u(rx * w), u(ry * d));
                    let (major, ratio) = if hx >= hy { (Vec2::new(hx, 0.0), hy / hx.max(1e-12)) } else { (Vec2::new(0.0, hy), hx / hy.max(1e-12)) };
                    EntityKind::Ellipse(Ellipse {
                        center: v3(at(cx, cy)),
                        major: v3(major),
                        ratio: ratio.clamp(1e-6, 1.0),
                        start: 0.0,
                        end: std::f64::consts::TAU,
                    })
                }
                Shape::Line(x0, y0, x1, y1) => EntityKind::Line(Line { a: v3(at(x0, y0)), b: v3(at(x1, y1)) }),
                Shape::Poly(points) => poly(points.iter().map(|(x, y)| vertex(at(*x, *y), 0.0)).collect()),
            };
            pieces.push(HvacPiece { layer: FURNITURE_LAYER, kind });
        }
        if !self.name.trim().is_empty() {
            let h = u(self.label_height_mm);
            let mut text = label(Vec2::new(0.0, u(-d) - 1.3 * h), h, self.name.trim().to_string());
            if let (EntityKind::Text(t), true) = (&mut text, flip_label) {
                t.rotation = std::f64::consts::PI;
            }
            pieces.push(HvacPiece { layer: FURNITURE_LAYER, kind: text });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(FURNITURE_APP, FURNITURE_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<FurnitureParams> {
        let p: FurnitureParams = xdata::from_xdata(FURNITURE_APP, FURNITURE_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The piece of furniture an object is, when it is a block reference carrying usable furniture data.
pub fn furniture_of(e: &crate::Entity) -> Option<FurnitureParams> {
    match &e.kind {
        EntityKind::Insert(_) => FurnitureParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour the furniture layer is created with.
pub fn furniture_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    (name == FURNITURE_LAYER).then_some(cadcraft_color::Color::True(cadcraft_color::Rgb(0xc9, 0xb2, 0x8a)))
}

impl crate::Drawing {
    /// Adds a piece of furniture of `p` at `at` (the middle of its side against the wall), turned `rotation` radians.
    pub fn add_furniture(
        &mut self,
        space: &crate::Space,
        mut common: crate::Common,
        p: &FurnitureParams,
        at: Vec2,
        rotation: f64,
    ) -> Result<crate::Handle> {
        if !at.is_finite() || !rotation.is_finite() {
            return Err(invalid("A piece of furniture needs a usable position."));
        }
        let pieces = p.generate_turned(super::duct::reads_upside_down(rotation))?;
        common.xdata = xdata::replace(FURNITURE_APP, &common.xdata, p.to_xdata()?);
        common.layer = FURNITURE_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(FURNITURE_LAYER);
        let insert = crate::Insert { block, insert: v3(at), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives piece `h` the parameters `p`, redrawing it in place.
    pub fn regenerate_furniture(&mut self, h: crate::Handle, p: &FurnitureParams) -> Result<()> {
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let turned = matches!(&e.kind, EntityKind::Insert(i) if super::duct::reads_upside_down(i.rotation));
        let pieces = p.generate_turned(turned)?;
        let tags = xdata::replace(FURNITURE_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_has_unique_ids_sane_sizes_and_known_groups() {
        let mut seen = std::collections::HashSet::new();
        for m in furniture_models() {
            assert!(seen.insert(m.id), "{} twice", m.id);
            assert!(furniture_groups().contains(&m.group), "{}: group {}", m.id, m.group);
            assert!(m.width_mm >= 10.0 && m.depth_mm >= 10.0 && m.height_mm >= 1.0 && m.power_w >= 0.0, "{}", m.id);
            assert!(!m.shapes.is_empty(), "{}", m.id);
            // Every shape stays inside the piece (a little over is allowed), with finite numbers.
            for s in m.shapes {
                let pts: Vec<f64> = match *s {
                    Shape::Rect(a, b, c, d) | Shape::Oval(a, b, c, d) | Shape::Line(a, b, c, d) => vec![a, b, c, d],
                    Shape::Round(a, b, c, d, e) => vec![a, b, c, d, e],
                    Shape::Poly(p) => p.iter().flat_map(|(x, y)| [*x, *y]).collect(),
                };
                assert!(pts.iter().all(|v| v.is_finite() && (-0.01..=1.01).contains(v)), "{}: {s:?}", m.id);
            }
        }
        assert!(furniture_models().len() >= 80);
        for g in furniture_groups() {
            assert!(furniture_models().iter().any(|m| m.group == *g), "group {g} is empty");
        }
    }

    #[test]
    fn every_model_draws_at_its_size_and_at_other_sizes_and_the_text_follows_the_name() {
        for m in furniture_models() {
            let mut p = FurnitureParams::new(m, HvacUnits::default());
            let n = p.generate().unwrap().len();
            assert_eq!(n, m.shapes.len(), "{}", m.id);
            p.width_mm *= 2.0;
            p.depth_mm *= 0.5;
            assert!(p.generate().is_ok(), "{}", m.id);
            p.name = "A1".into();
            assert_eq!(p.generate().unwrap().len(), m.shapes.len() + 1);
            assert_eq!(FurnitureParams::from_xdata(&p.to_xdata().unwrap()), Some(p));
        }
    }

    #[test]
    fn a_model_is_found_by_its_id_or_its_name_and_bad_values_are_refused() {
        assert_eq!(parse_furniture_model("Sofa3").map(|m| m.id), Some("sofa3"));
        assert_eq!(parse_furniture_model("Sofa, three seats").map(|m| m.id), Some("sofa3"));
        assert!(parse_furniture_model("piano a queue").is_none());
        let fridge = furniture_model("fridge").unwrap();
        assert_eq!(FurnitureParams::new(fridge, HvacUnits::default()).power_w, Some(120.0));
        assert_eq!(FurnitureParams::new(furniture_model("sofa2").unwrap(), HvacUnits::default()).power_w, None);
        let ok = FurnitureParams::new(fridge, HvacUnits::default());
        for bad in [
            FurnitureParams { width_mm: 5.0, ..ok.clone() },
            FurnitureParams { depth_mm: f64::NAN, ..ok.clone() },
            FurnitureParams { height_mm: 1e9, ..ok.clone() },
            FurnitureParams { power_w: Some(-1.0), ..ok.clone() },
            FurnitureParams { model: "nope".into(), ..ok.clone() },
            FurnitureParams { name: "x".repeat(41), ..ok.clone() },
            FurnitureParams { bottom_mm: Some(f64::INFINITY), ..ok.clone() },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
