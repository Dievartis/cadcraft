//! Parametric catalogue recipes adapted from freemans32/cadcraft, perso e4964f0ba.
//! Stored block geometry and extended data remain readable with the catalogue UI disabled.
mod distribute;
mod equipment;
pub use distribute::*;
pub use equipment::*;
mod furniture;
mod furniture_catalog;
pub(crate) mod xdata;
use crate::{DocError, Result};
use cadcraft_geom::{Vec2, Vec3};
pub use furniture::*;
use serde::{Deserialize, Serialize};
const LABEL_HEIGHT_MM: (f64, f64) = (1.0, 10_000.0);
/// The pieces of an object as preview entities, turned `rotation` radians and moved to `at`: what a
/// command draws under the cursor before the object exists. Each carries its layer's colour, so it
/// looks as it will even when the layer is not in the drawing yet.
pub fn preview_entities(pieces: Vec<HvacPiece>, at: Vec2, rotation: f64) -> Vec<crate::Entity> {
    let m = cadcraft_geom::Mat3::rotate(rotation).then(cadcraft_geom::Mat3::translate(at));
    pieces
        .into_iter()
        .map(|p| {
            let color = hvac_layer_color(p.layer).unwrap_or(cadcraft_color::Color::ByLayer);
            let mut e = crate::Entity {
                handle: crate::Handle(0),
                common: crate::Common { layer: p.layer.into(), color, ..Default::default() },
                kind: p.kind,
            };
            e.kind.transform(&m);
            e
        })
        .collect()
}

/// How a ventilation object's millimetres become drawing units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvacUnits {
    /// Millimetres per drawing unit (1 for millimetres, 1000 for metres).
    pub mm_per_unit: f64,
}

impl Default for HvacUnits {
    fn default() -> Self {
        HvacUnits { mm_per_unit: 1.0 }
    }
}

impl HvacUnits {
    /// The units of drawing `d`: its `INSUNITS`, else millimetres in a metric drawing and inches
    /// in an imperial one.
    pub fn of(d: &crate::Drawing) -> Self {
        let mm_per_unit = match d.header.i64("INSUNITS", 0) {
            1 => 25.4,
            2 => 304.8,
            4 => 1.0,
            5 => 10.0,
            6 => 1000.0,
            14 => 100.0,
            _ if d.header.i64("MEASUREMENT", 0) == 1 => 1.0,
            _ => 25.4,
        };
        HvacUnits { mm_per_unit }
    }
    fn validate(self) -> crate::Result<()> {
        if self.mm_per_unit.is_finite() && self.mm_per_unit > 0.0 && self.mm_per_unit <= 1e6 {
            Ok(())
        } else {
            Err(invalid("The drawing units are not usable."))
        }
    }
    /// Millimetres as drawing units.
    pub fn to_units(self, mm: f64) -> f64 {
        mm / self.mm_per_unit
    }
    /// Square drawing units as square metres.
    pub fn area_m2(self, area_units: f64) -> f64 {
        let m = self.mm_per_unit / 1000.0;
        area_units * m * m
    }
}

fn invalid(m: impl Into<String>) -> DocError {
    DocError::Invalid(m.into())
}

fn v3(p: Vec2) -> Vec3 {
    Vec3::new(p.x, p.y, 0.0)
}

fn check_label_height(mm: f64) -> crate::Result<()> {
    if mm.is_finite() && (LABEL_HEIGHT_MM.0..=LABEL_HEIGHT_MM.1).contains(&mm) {
        Ok(())
    } else {
        Err(invalid(format!("The label height is {} to {} mm.", LABEL_HEIGHT_MM.0, LABEL_HEIGHT_MM.1)))
    }
}

/// A centred single-line text at `at`.
fn label(at: Vec2, height: f64, value: String) -> crate::EntityKind {
    text_at(at, height, value, crate::HAlign::Center)
}

/// Whether the altitude of an underside, mm, can be kept.
pub(crate) fn check_altitude(bottom_mm: Option<f64>) -> crate::Result<()> {
    match bottom_mm {
        Some(b) if !(b.is_finite() && (-100_000.0..=1_000_000.0).contains(&b)) => {
            Err(invalid("The altitude of the underside is a number of millimetres."))
        }
        _ => Ok(()),
    }
}

fn text_at(at: Vec2, height: f64, value: String, halign: crate::HAlign) -> crate::EntityKind {
    crate::EntityKind::Text(crate::Text {
        insert: v3(at),
        align_pt: Some(v3(at)),
        height,
        value,
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        style: "Standard".into(),
        halign,
        valign: crate::VAlign::Middle,
    })
}

/// One object of a ventilation object's generated geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct HvacPiece {
    pub layer: &'static str,
    pub kind: crate::EntityKind,
}

impl crate::Drawing {
    /// Gives ventilation object `h` (a block reference) the generated `pieces` and the extended
    /// data `xdata`: a new block, the old one removed when nothing else refers to it. A scaled
    /// or arrayed reference is refused: its drawn size would no longer match its data.
    pub(crate) fn regenerate_hvac(&mut self, h: crate::Handle, pieces: Vec<HvacPiece>, xdata: Vec<crate::RawTag>) -> crate::Result<()> {
        let e = self.entity(h).ok_or(DocError::NoEntity(h))?;
        let crate::EntityKind::Insert(i) = &e.kind else { return Err(invalid("The object is not a block reference.")) };
        let unit = |s: f64| (s - 1.0).abs() <= 1e-9;
        if !(unit(i.scale.x) && unit(i.scale.y) && i.cols <= 1 && i.rows <= 1) {
            return Err(invalid("The object was scaled or arrayed: it can't be regenerated."));
        }
        let old = i.block.clone();
        let block = self.hvac_block(pieces);
        self.modify_entity(h, |e| {
            if let crate::EntityKind::Insert(i) = &mut e.kind {
                i.block = block;
            }
            e.common.xdata = xdata;
        })?;
        self.drop_unreferenced_hvac_block(&old);
        Ok(())
    }

    /// Removes anonymous block `name` when no object refers to it.
    fn drop_unreferenced_hvac_block(&mut self, name: &str) {
        let refers = |e: &std::sync::Arc<crate::Entity>| matches!(&e.kind, crate::EntityKind::Insert(i) if i.block.eq_ignore_ascii_case(name));
        let used = self.model.iter().any(refers)
            || self.layouts.iter().any(|l| l.entities.iter().any(refers))
            || self.blocks.values().any(|b| b.entities.iter().any(refers) || b.array.as_ref().is_some_and(|a| a.source.eq_ignore_ascii_case(name)));
        if !used && self.block(name).is_some_and(|b| b.anonymous) {
            self.blocks.retain(|k, _| !k.eq_ignore_ascii_case(name));
        }
    }

    /// A new anonymous block containing generated catalogue geometry.
    pub(crate) fn hvac_block(&mut self, pieces: Vec<HvacPiece>) -> String {
        let name = self.anonymous_block_name("*U");
        let mut block = crate::Block::new(&name);
        block.anonymous = true;
        for piece in pieces {
            if self.ensure_layer(piece.layer)
                && let Some(color) = hvac_layer_color(piece.layer)
                && let Some(layer) = self.layers.iter_mut().find(|l| l.name == piece.layer)
            {
                layer.color = color;
            }
            block.entities.push(crate::Entity {
                handle: self.new_handle(),
                common: crate::Common { layer: piece.layer.into(), ..Default::default() },
                kind: piece.kind,
            });
        }
        self.blocks.insert(name.clone(), std::sync::Arc::new(block));
        name
    }
}
fn hvac_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    if name == EQUIPMENT_ZONE_LAYER { Some(cadcraft_color::Color::True(cadcraft_color::Rgb(0x9a, 0x9a, 0x9a))) } else { furniture_layer_color(name) }
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
        let ok = |v: f64| v.is_finite() && (20.0..=5_000.0).contains(&v);
        let fits = match self {
            RiserSize::Round { diameter_mm } => ok(diameter_mm),
            RiserSize::Rectangular { width_mm, height_mm } => ok(width_mm) && ok(height_mm),
        };
        if fits { Ok(()) } else { Err(invalid(format!("A riser's size is {} to {} mm.", 20.0, 5_000.0))) }
    }
}

pub fn check_circuit_name(name: &str, empty_ok: bool) -> Result<()> {
    if name.trim().is_empty() {
        return if empty_ok { Ok(()) } else { Err(invalid("A circuit needs a name.")) };
    }
    if name.chars().count() > 32 || name.chars().any(char::is_control) {
        return Err(invalid("A circuit's name has at most 32 characters, without control characters."));
    }
    Ok(())
}

fn check_name(what: &str, name: &str) -> crate::Result<()> {
    if name.trim().is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(invalid(format!("A {what} name has 1 to 64 characters, without control characters.")));
    }
    Ok(())
}

/// A single-line text starting at `at`, its middle on the line.
fn label_left(at: Vec2, height: f64, value: String) -> crate::EntityKind {
    text_at(at, height, value, crate::HAlign::Left)
}

/// The altitude of the underside of an element as it is written beside it ("Ai:2.50", metres), when it is given.
pub(crate) fn altitude_tag(bottom_mm: Option<f64>) -> Option<String> {
    bottom_mm.map(|b| format!("Ai:{:.2}", b / 1000.0))
}

fn yes() -> bool {
    true
}
pub fn address_suffix(address: &str, show: bool) -> String {
    let a = address.trim();
    if show && !a.is_empty() { format!(" @{a}") } else { String::new() }
}

fn reads_upside_down(rotation: f64) -> bool {
    let a = rotation.rem_euclid(std::f64::consts::TAU);
    a > std::f64::consts::FRAC_PI_2 + 1e-9 && a <= 3.0 * std::f64::consts::FRAC_PI_2 + 1e-9
}

/// Read only the outline of an existing freemans32 room for distributing catalogue items.
/// Room creation and ventilation calculations belong to the building module.
#[derive(Deserialize)]
struct RoomOutline {
    outline: Vec<Vec2>,
}
impl crate::Drawing {
    pub fn catalogue_room_outline(&self, h: crate::Handle) -> Option<Vec<Vec2>> {
        let e = self.entity(h)?;
        let crate::EntityKind::Insert(i) = &e.kind else { return None };
        let p: RoomOutline = xdata::from_xdata("CADCRAFT_SPACE", "SPACE1", &e.common.xdata)?;
        if !(3..=256).contains(&p.outline.len()) || !i.insert.is_finite() || !i.scale.is_finite() || !i.rotation.is_finite() {
            return None;
        }
        let world: Vec<Vec2> =
            p.outline.into_iter().map(|v| Vec2::new(v.x * i.scale.x, v.y * i.scale.y).rotate(i.rotation) + i.insert.xy()).collect();
        world.iter().all(|p| p.is_finite()).then_some(world)
    }
}

#[cfg(test)]
mod equipment_tests;
