//! Shared placement helpers adapted from freemans32/cadcraft.
use super::bad;
use crate::doc::Handle;
use crate::geom::Vec2;
use crate::{EngineError, Result};
use serde_json::Value;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Anchor {
    pub x: i8,
    pub y: i8,
}

impl Anchor {
    /// The back middle: where a piece with its origin on the back wall is held, and the centre for a piece with its origin in the middle.
    pub const BACK: Anchor = Anchor { x: 0, y: 1 };
    pub const CENTRE: Anchor = Anchor { x: 0, y: 0 };

    /// The anchors in the order Tab goes round: the back row from the left, then the middle, then the front.
    const ORDER: [Anchor; 9] = [
        Anchor { x: -1, y: 1 },
        Anchor { x: 0, y: 1 },
        Anchor { x: 1, y: 1 },
        Anchor { x: -1, y: 0 },
        Anchor { x: 0, y: 0 },
        Anchor { x: 1, y: 0 },
        Anchor { x: -1, y: -1 },
        Anchor { x: 0, y: -1 },
        Anchor { x: 1, y: -1 },
    ];

    pub fn next(self) -> Anchor {
        let i = Self::ORDER.iter().position(|a| *a == self).unwrap_or(0);
        Self::ORDER.get((i + 1) % Self::ORDER.len()).copied().unwrap_or(self)
    }

    /// The words that say where it is, for the command line.
    pub fn name(self) -> &'static str {
        match (self.y, self.x) {
            (1, -1) => "back left",
            (1, 0) => "back middle",
            (1, 1) => "back right",
            (0, -1) => "middle left",
            (0, 0) => "centre",
            (0, 1) => "middle right",
            (-1, -1) => "front left",
            (-1, 0) => "front middle",
            _ => "front right",
        }
    }

    /// Where the piece is put when the cursor is at `at`: its origin moved so that this point of the piece is on the cursor. The piece is `width_mm` along
    /// x and `depth_mm` along y; `from_back` is true when its origin is the middle of its back (it goes toward -y), false when it is its centre.
    pub fn origin_for(self, at: Vec2, units: cadcraft_doc::HvacUnits, width_mm: f64, depth_mm: f64, from_back: bool, rotation: f64) -> Vec2 {
        let (half_w, depth) = (units.to_units(width_mm) / 2.0, units.to_units(depth_mm));
        let cy = if from_back { -depth / 2.0 } else { 0.0 };
        let held = Vec2::new(f64::from(self.x) * half_w, cy + f64::from(self.y) * depth / 2.0);
        at - held.rotate(rotation)
    }
}

/// The turn in degrees that a piece being placed at `at` is given by `i`: the direction of a second click, or a typed angle; `None` for Enter (the piece keeps its turn).
pub(crate) fn aimed_degrees(i: &crate::Input, at: Vec2) -> Result<Option<f64>> {
    use crate::Input;
    match i {
        Input::Point(p) if !p.near(at, 1e-9) => Ok(Some((*p - at).angle().to_degrees())),
        Input::Text(t) | Input::Keyword(t) => {
            let t = t.trim();
            if t.is_empty() {
                return Ok(None);
            }
            t.parse::<f64>().ok().filter(|a| a.is_finite()).map(Some).ok_or_else(|| EngineError::Other("Requires an angle in degrees.".into()))
        }
        _ => Ok(None),
    }
}
pub(crate) fn handles_param(cmd: &str, p: &Value) -> Result<Vec<Handle>> {
    if let Some(h) = super::curves::handle_param(p, "handle") {
        return Ok(vec![h]);
    }
    let Some(Value::Array(a)) = p.get("handles") else { return Err(bad(cmd, "`handle` or `handles` is required")) };
    if a.len() > 5_000 {
        return Err(bad(cmd, "at most 5000 objects"));
    }
    a.iter().map(|v| v.as_str().and_then(Handle::parse_hex).ok_or_else(|| bad(cmd, "`handles` must be handles"))).collect()
}

use crate::doc::RiserSize;
pub(super) fn size_value(cmd: &str, v: &Value) -> Result<Option<RiserSize>> {
    let sides = |t: &str| -> Option<(f64, f64)> {
        let (a, b) = t.split_once(['x', 'X', '×', '*'])?;
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    };
    let err = || bad(cmd, "a size is a diameter, \"400x250\" or \"auto\"");
    match v {
        Value::Null => Ok(None),
        Value::String(t) if t.trim().eq_ignore_ascii_case("auto") || t.trim().is_empty() => Ok(None),
        Value::String(t) => match (sides(t), t.trim().parse::<f64>().ok()) {
            (Some((w, h)), _) => Ok(Some(RiserSize::Rectangular { width_mm: w, height_mm: h })),
            (None, Some(d)) => Ok(Some(RiserSize::Round { diameter_mm: d })),
            _ => Err(err()),
        },
        Value::Number(n) => Ok(Some(RiserSize::Round { diameter_mm: n.as_f64().ok_or_else(err)? })),
        Value::Array(a) => match (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64), a.len()) {
            (Some(w), Some(h), 2) => Ok(Some(RiserSize::Rectangular { width_mm: w, height_mm: h })),
            _ => Err(err()),
        },
        _ => Err(err()),
    }
}

pub(crate) fn altitude_param(cmd: &str, p: &Value) -> Result<Option<Option<f64>>> {
    match p.get("altitude") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => v
            .as_f64()
            .filter(|m| m.is_finite() && m.abs() <= 100_000.0)
            .map(|m| Some(Some(m * 1000.0)))
            .ok_or_else(|| bad(cmd, "`altitude` must be a number of metres or null")),
    }
}

/// Catalogue commands may regenerate several blocks. Rejecting a later object must leave
/// earlier objects, layer creation, handle allocation and remembered model choices unchanged.
pub(super) fn atomic(s: &mut crate::Session, run: impl FnOnce(&mut crate::Session) -> Result<Value>) -> Result<Value> {
    let before = s.state()?.doc.clone();
    let last_used = s.last_used.clone();
    match run(s) {
        Ok(value) => Ok(value),
        Err(error) => {
            s.state_mut()?.doc = before;
            s.last_used = last_used;
            Err(error)
        }
    }
}

/// Interpret catalogue dimensions in the space being edited, including paper-space overrides.
pub(super) fn units(s: &crate::Session) -> Result<crate::doc::HvacUnits> {
    let drawing = s.doc()?;
    if let crate::doc::Space::Paper(name) = s.space()
        && let Some(layout) = drawing.layout(&name)
    {
        return Ok(crate::doc::HvacUnits { mm_per_unit: cadcraft_render::paper::page_unit_mm(drawing, &layout.page) });
    }
    Ok(crate::doc::HvacUnits::of(drawing))
}
