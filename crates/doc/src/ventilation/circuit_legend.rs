//! The legend of the electrical circuits: one row for each circuit of the drawing, a sample of its cable in its own colour and line type, its name,
//! its use, its cable, its length, its power and its breaker. It is a block reference with its settings in extended data; the circuits are read again
//! when it is redrawn, and naming, renaming, sizing or restyling a circuit redraws the legends of the drawing.

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::{CableUsage, CircuitOptions, HvacPiece, HvacUnits, cable_layer_of, check_label_height, invalid, label_left, v3, xdata};
use crate::{EntityKind, Line, RawTag, Result};

/// The registered application name of a legend's extended data.
pub const CIRCUIT_LEGEND_APP: &str = "CADCRAFT_CIRCUITLEGEND";
/// The layer of the legend's title; each row is on its circuit's layer.
pub const CIRCUIT_LEGEND_LAYER: &str = "E-LEGEND";
const XDATA_FORMAT: &str = "CIRCUITLEGEND1";
/// Most rows a legend draws.
const MAX_ROWS: usize = 200;

fn default_label_mm() -> f64 {
    150.0
}

fn default_title() -> String {
    "Circuits".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitLegendParams {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl CircuitLegendParams {
    pub fn new(units: HvacUnits) -> CircuitLegendParams {
        CircuitLegendParams { title: default_title(), label_height_mm: default_label_mm(), units }
    }

    pub fn validate(&self) -> Result<()> {
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.title.chars().count() > 64 || self.title.chars().any(char::is_control) {
            return Err(invalid("A legend's title has at most 64 characters, without control characters."));
        }
        Ok(())
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(CIRCUIT_LEGEND_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<CircuitLegendParams> {
        let p: CircuitLegendParams = xdata::from_xdata(CIRCUIT_LEGEND_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The legend an object is, when it is a block reference carrying usable legend data.
pub fn circuit_legend_of(e: &crate::Entity) -> Option<CircuitLegendParams> {
    match &e.kind {
        EntityKind::Insert(_) => CircuitLegendParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// The legend's pieces for the circuits the drawing has now: the title, then a row for each circuit.
    pub fn generate_circuit_legend(&self, p: &CircuitLegendParams) -> Result<Vec<HvacPiece>> {
        p.validate()?;
        let u = p.units;
        let h = u.to_units(p.label_height_mm);
        let mut pieces = vec![HvacPiece { layer: CIRCUIT_LEGEND_LAYER, kind: label_left(Vec2::new(0.0, 0.0), h * 1.2, p.title.clone()) }];
        let sample = u.to_units(1000.0);
        let mut y = -2.2 * h;
        let reports = self.circuit_reports(&CircuitOptions::default());
        for r in reports.iter().filter(|r| r.cable_length_m > 0.0).take(MAX_ROWS) {
            // The row is on the layer the circuit's cables are on, so it has their colour and their line type.
            let usage = CableUsage::parse(&r.usage).unwrap_or_default();
            let layer = cable_layer_of(usage, &r.name);
            pieces.push(HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(Vec2::new(0.0, y)), b: v3(Vec2::new(sample, y)) }) });
            let cable = r.section_mm2.map_or("-".to_string(), |s| format!("{}G{s}", if r.phases == "three" { 5 } else { 3 }));
            let breaker = r.protection_a.or(r.rating_a).map_or("-".to_string(), |a| format!("{a:.0} A"));
            let text = format!("{}  {}  {cable}  {:.0} m  {:.1} kW  {breaker}", r.name, r.usage, r.cable_length_m, r.power_w / 1000.0);
            pieces.push(HvacPiece { layer, kind: label_left(Vec2::new(sample + 1.5 * h, y), h, text) });
            y -= 2.0 * h;
        }
        if y > -2.2 * h - 1e-9 {
            pieces.push(HvacPiece { layer: CIRCUIT_LEGEND_LAYER, kind: label_left(Vec2::new(0.0, y), h, "-".to_string()) });
        }
        Ok(pieces)
    }

    /// Adds a legend with its upper left corner at `at`.
    pub fn add_circuit_legend(
        &mut self,
        space: &crate::Space,
        mut common: crate::Common,
        p: &CircuitLegendParams,
        at: Vec2,
    ) -> Result<crate::Handle> {
        if !at.is_finite() {
            return Err(invalid("A legend needs a usable position."));
        }
        let pieces = self.generate_circuit_legend(p)?;
        common.xdata = xdata::replace(CIRCUIT_LEGEND_APP, &common.xdata, p.to_xdata()?);
        common.layer = CIRCUIT_LEGEND_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(CIRCUIT_LEGEND_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Redraws legend `h` for the circuits the drawing has now.
    pub fn regenerate_circuit_legend(&mut self, h: crate::Handle, p: &CircuitLegendParams) -> Result<()> {
        let pieces = self.generate_circuit_legend(p)?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(CIRCUIT_LEGEND_APP, &old, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// Redraws every circuit legend of model space: the circuits, their colours, cables or lengths changed.
    pub fn refresh_circuit_legends(&mut self) {
        let legends: Vec<(crate::Handle, CircuitLegendParams)> =
            self.model.iter().filter_map(|e| circuit_legend_of(e).map(|p| (e.handle, p))).collect();
        for (h, p) in legends {
            let _ = self.regenerate_circuit_legend(h, &p);
        }
    }
}
