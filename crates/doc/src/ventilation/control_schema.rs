//! The principle diagram of the controls: the buses with the controllers on them, each controller (and the modules that extend it) a box with what it offers and
//! uses, and under it the points wired to it, with their address and type. It is a block reference with its settings in extended data; it is read from the
//! list of points when it is redrawn, and the controls tools redraw it.

use cadcraft_geom::{PolyVertex, Vec2};
use serde::{Deserialize, Serialize};

use super::part::{line, rect};
use super::{HvacPiece, HvacUnits, check_label_height, invalid, label, label_left, same_network, v3, xdata};
use crate::{EntityKind, LwPolyline, RawTag, Result};

/// The registered application name of a diagram's extended data, and its layer.
pub const CONTROL_SCHEMA_APP: &str = "CADCRAFT_CTRLSCHEMA";
pub const CONTROL_SCHEMA_LAYER: &str = "R-SCHEMA";
const XDATA_FORMAT: &str = "CTRLSCHEMA1";
/// Most controllers and points a diagram draws.
const MAX_CONTROLLERS: usize = 100;
const MAX_ROWS: usize = 2000;
const MAX_ROWS_PER_CONTROLLER: usize = 80;
/// The width of the column of a controller and the height of a row of points, mm.
const COLUMN_MM: f64 = 4200.0;
const ROW_MM: f64 = 330.0;

fn default_label_mm() -> f64 {
    120.0
}

fn default_title() -> String {
    "Controls principle diagram".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlSchemaParams {
    #[serde(default = "default_title")]
    pub title: String,
    /// The bus drawn; empty for every bus.
    #[serde(default)]
    pub bus: String,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl ControlSchemaParams {
    pub fn new(units: HvacUnits) -> ControlSchemaParams {
        ControlSchemaParams { title: default_title(), bus: String::new(), label_height_mm: default_label_mm(), units }
    }

    pub fn validate(&self) -> Result<()> {
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.title.chars().count() > 64
            || self.title.chars().any(char::is_control)
            || self.bus.chars().count() > 64
            || self.bus.chars().any(char::is_control)
        {
            return Err(invalid("A diagram's title and bus have at most 64 characters, without control characters."));
        }
        Ok(())
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(CONTROL_SCHEMA_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<ControlSchemaParams> {
        let p: ControlSchemaParams = xdata::from_xdata(CONTROL_SCHEMA_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The diagram an object is, when it is a block reference carrying usable data.
pub fn control_schema_of(e: &crate::Entity) -> Option<ControlSchemaParams> {
    match &e.kind {
        EntityKind::Insert(_) => ControlSchemaParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// The pieces of the diagram for the controls the drawing has now.
    pub fn generate_control_schema(&self, p: &ControlSchemaParams) -> Result<Vec<HvacPiece>> {
        p.validate()?;
        let u = |mm: f64| p.units.to_units(mm);
        let h = u(p.label_height_mm);
        let t = self.control_takeoff("");
        let mut pieces = vec![HvacPiece { layer: CONTROL_SCHEMA_LAYER, kind: label_left(Vec2::ZERO, h * 1.4, p.title.clone()) }];
        // The buses, in the order of their names; the controllers with none are together at the end. A module is drawn with the controller it extends.
        let mut buses: Vec<String> = Vec::new();
        for c in t.controllers.iter().filter(|c| c.extends.trim().is_empty()) {
            let b = c.bus.trim().to_string();
            if !buses.iter().any(|x| same_network(x, &b)) {
                buses.push(b);
            }
        }
        buses.sort_by_key(|b| (b.is_empty(), b.to_lowercase()));
        let wanted = p.bus.trim();
        let (mut x0, mut count, mut total_rows) = (0.0, 0usize, 0usize);
        for bus in buses.iter().filter(|b| wanted.is_empty() || same_network(b, wanted)) {
            let mine: Vec<&super::ControllerLoad> = t
                .controllers
                .iter()
                .filter(|c| c.extends.trim().is_empty() && same_network(c.bus.trim(), bus))
                .take(MAX_CONTROLLERS.saturating_sub(count))
                .collect();
            if mine.is_empty() {
                continue;
            }
            let width = mine.len() as f64 * COLUMN_MM;
            let bar_y = -1500.0;
            let bar = vec![
                PolyVertex { p: Vec2::new(u(x0), u(bar_y)), bulge: 0.0, start_width: 0.0, end_width: 0.0 },
                PolyVertex { p: Vec2::new(u(x0 + width), u(bar_y)), bulge: 0.0, start_width: 0.0, end_width: 0.0 },
            ];
            pieces.push(HvacPiece {
                layer: CONTROL_SCHEMA_LAYER,
                kind: EntityKind::LwPolyline(LwPolyline { vertices: bar, closed: false, const_width: u(40.0), elevation: 0.0, plinegen: false }),
            });
            pieces.push(HvacPiece {
                layer: CONTROL_SCHEMA_LAYER,
                kind: label_left(Vec2::new(u(x0), u(-900.0)), h * 1.2, if bus.is_empty() { "-".to_string() } else { bus.clone() }),
            });
            for (k, c) in mine.iter().enumerate() {
                count += 1;
                let x = x0 + (k as f64 + 0.5) * COLUMN_MM;
                pieces.push(HvacPiece { layer: CONTROL_SCHEMA_LAYER, kind: line((u(x), u(bar_y)), (u(x), u(-2000.0))) });
                // The controller (and its modules) as boxes with their name and what they offer and use.
                let family: Vec<&super::ControllerLoad> =
                    std::iter::once(*c).chain(t.controllers.iter().filter(|m| same_network(m.extends.trim(), &c.name))).collect();
                let mut y = -2000.0;
                for m in &family {
                    pieces.push(HvacPiece { layer: CONTROL_SCHEMA_LAYER, kind: rect(u(x), u(y - 450.0), u(COLUMN_MM * 0.42), u(450.0)) });
                    pieces.push(HvacPiece {
                        layer: CONTROL_SCHEMA_LAYER,
                        kind: label(Vec2::new(u(x), u(y - 330.0)), h, format!("{} ({})", m.name, m.kind)),
                    });
                    let load = format!(
                        "AI {}/{}  AO {}/{}  DI {}/{}  DO {}/{}",
                        m.used.ai, m.capacity.ai, m.used.ao, m.capacity.ao, m.used.di, m.capacity.di, m.used.dout, m.capacity.dout
                    );
                    pieces.push(HvacPiece { layer: CONTROL_SCHEMA_LAYER, kind: label(Vec2::new(u(x), u(y - 640.0)), h * 0.8, load) });
                    y -= 1000.0;
                }
                // The points wired to the controller or to its modules, one row each.
                let mut rows = 0;
                for pt in t.points.iter().filter(|pt| family.iter().any(|m| same_network(&pt.controller, &m.name))) {
                    if rows >= MAX_ROWS_PER_CONTROLLER || total_rows >= MAX_ROWS {
                        break;
                    }
                    total_rows += 1;
                    let row_y = y - 250.0 - rows as f64 * ROW_MM;
                    let addr = if pt.address.is_empty() { String::new() } else { format!("@{}  ", pt.address) };
                    let unit = if pt.unit.is_empty() { String::new() } else { format!(" {}", pt.unit) };
                    pieces.push(HvacPiece {
                        layer: CONTROL_SCHEMA_LAYER,
                        kind: label_left(
                            Vec2::new(u(x - COLUMN_MM * 0.45), u(row_y)),
                            h * 0.85,
                            format!("{addr}{}  {}{unit}", pt.device, pt.point_type),
                        ),
                    });
                    rows += 1;
                }
                if rows > 0 {
                    pieces.push(HvacPiece { layer: CONTROL_SCHEMA_LAYER, kind: line((u(x), u(y)), (u(x), u(y - 150.0))) });
                }
            }
            x0 += width + COLUMN_MM * 0.3;
        }
        if x0 == 0.0 {
            pieces.push(HvacPiece { layer: CONTROL_SCHEMA_LAYER, kind: label_left(Vec2::new(0.0, u(-900.0)), h, "-".to_string()) });
        }
        Ok(pieces)
    }

    /// Adds a diagram with its upper left corner at `at`.
    pub fn add_control_schema(
        &mut self,
        space: &crate::Space,
        mut common: crate::Common,
        p: &ControlSchemaParams,
        at: Vec2,
    ) -> Result<crate::Handle> {
        if !at.is_finite() {
            return Err(invalid("A diagram needs a usable position."));
        }
        let pieces = self.generate_control_schema(p)?;
        common.xdata = xdata::replace(CONTROL_SCHEMA_APP, &common.xdata, p.to_xdata()?);
        common.layer = CONTROL_SCHEMA_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(CONTROL_SCHEMA_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Redraws diagram `h` for the controls the drawing has now.
    pub fn regenerate_control_schema(&mut self, h: crate::Handle, p: &ControlSchemaParams) -> Result<()> {
        let pieces = self.generate_control_schema(p)?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(CONTROL_SCHEMA_APP, &old, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// Whether a control device or a diagram of the controls of `space` was added, changed or removed since `before` (an older state of this drawing).
    pub fn controls_touched_since(&self, before: &crate::Drawing, space: &crate::Space) -> bool {
        let (Some(now), Some(then)) = (self.space(space), before.space(space)) else { return false };
        let relevant = |e: &std::sync::Arc<crate::Entity>| matches!(e.kind, EntityKind::Insert(_)) && super::control_of(e).is_some();
        now.iter().any(|e| then.get(e.handle).is_none_or(|old| !std::sync::Arc::ptr_eq(old, e)) && relevant(e))
            || then.iter().any(|e| now.get(e.handle).is_none() && relevant(e))
    }

    /// Redraws every controls diagram of model space.
    pub fn refresh_control_schemas(&mut self) {
        let diagrams: Vec<(crate::Handle, ControlSchemaParams)> =
            self.model.iter().filter_map(|e| control_schema_of(e).map(|p| (e.handle, p))).collect();
        for (h, p) in diagrams {
            let _ = self.regenerate_control_schema(h, &p);
        }
    }
}
