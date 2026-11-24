//! The single-line diagram of the electrical installation: for each distribution board a busbar fed by its main breaker, and a branch for each of its circuits
//! with its breaker, its cable, its load and the short-circuit currents. It is a block reference with its settings in extended data; the circuits are read
//! again when it is redrawn, and naming, sizing or editing a circuit redraws the diagrams of the drawing, as it does the legends.

use cadcraft_geom::{PolyVertex, Vec2};
use serde::{Deserialize, Serialize};

use super::part::{line, rect};
use super::{
    CableUsage, CircuitOptions, DeviceKind, HvacPiece, HvacUnits, cable_layer_of, check_label_height, device_of, invalid, label, label_left,
    same_network, v3, xdata,
};
use crate::{EntityKind, LwPolyline, RawTag, Result};

/// The registered application name of a diagram's extended data, and its layer.
pub const SINGLE_LINE_APP: &str = "CADCRAFT_SINGLELINE";
pub const SINGLE_LINE_LAYER: &str = "E-SLD";
const XDATA_FORMAT: &str = "SINGLELINE1";
/// Most circuits a diagram draws.
const MAX_BRANCHES: usize = 200;
/// The width of a branch and the heights of the parts of the diagram, mm.
const BRANCH_MM: f64 = 2400.0;

fn default_label_mm() -> f64 {
    120.0
}

fn default_title() -> String {
    "Single-line diagram".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleLineParams {
    #[serde(default = "default_title")]
    pub title: String,
    /// The board drawn; empty for every board.
    #[serde(default)]
    pub board: String,
    #[serde(default)]
    pub options: CircuitOptions,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl SingleLineParams {
    pub fn new(units: HvacUnits) -> SingleLineParams {
        SingleLineParams {
            title: default_title(),
            board: String::new(),
            options: CircuitOptions::default(),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.title.chars().count() > 64
            || self.title.chars().any(char::is_control)
            || self.board.chars().count() > 64
            || self.board.chars().any(char::is_control)
        {
            return Err(invalid("A diagram's title and board have at most 64 characters, without control characters."));
        }
        Ok(())
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(SINGLE_LINE_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<SingleLineParams> {
        let p: SingleLineParams = xdata::from_xdata(SINGLE_LINE_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The diagram an object is, when it is a block reference carrying usable data.
pub fn single_line_of(e: &crate::Entity) -> Option<SingleLineParams> {
    match &e.kind {
        EntityKind::Insert(_) => SingleLineParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// The pieces of the diagram for the circuits the drawing has now.
    pub fn generate_single_line(&self, p: &SingleLineParams) -> Result<Vec<HvacPiece>> {
        p.validate()?;
        let u = |mm: f64| p.units.to_units(mm);
        let h = u(p.label_height_mm);
        let reports = self.circuit_reports(&p.options);
        // The boards drawn, with the rating of their main breaker, and the circuits that start from each; circuits whose board is not drawn are together.
        let mut boards: Vec<(String, Option<f64>)> = self
            .model
            .iter()
            .filter_map(|e| device_of(e).filter(|d| d.kind == DeviceKind::Panel && !d.name.trim().is_empty()))
            .map(|d| (d.name.trim().to_string(), d.supply_a))
            .collect();
        boards.sort_by_key(|b| b.0.to_lowercase());
        boards.dedup_by(|a, b| same_network(&a.0, &b.0));
        let wanted = p.board.trim();
        let mut groups: Vec<(String, Option<f64>, Vec<usize>)> = Vec::new();
        for (name, supply) in &boards {
            if wanted.is_empty() || same_network(name, wanted) {
                let members: Vec<usize> = reports.iter().enumerate().filter(|(_, r)| same_network(&r.panel, name)).map(|(i, _)| i).collect();
                groups.push((name.clone(), *supply, members));
            }
        }
        if wanted.is_empty() {
            let orphans: Vec<usize> =
                reports.iter().enumerate().filter(|(_, r)| !boards.iter().any(|(b, _)| same_network(b, &r.panel))).map(|(i, _)| i).collect();
            if !orphans.is_empty() {
                groups.push(("-".into(), None, orphans));
            }
        }
        let mut pieces = vec![HvacPiece {
            layer: SINGLE_LINE_LAYER,
            kind: label_left(Vec2::new(0.0, 0.0), h * 1.4, format!("{}  (Isc origin {:.1} kA)", p.title, p.options.source_ka)),
        }];
        let (busbar_y, mut x0, mut count) = (-2600.0, 0.0, 0usize);
        for (name, supply, members) in &groups {
            let n = members.len().min(MAX_BRANCHES.saturating_sub(count)).max(1);
            let width = n as f64 * BRANCH_MM;
            let xc = x0 + width / 2.0;
            // The origin, the main breaker and the board's name above the busbar.
            pieces.push(HvacPiece { layer: SINGLE_LINE_LAYER, kind: line((u(xc), u(-900.0)), (u(xc), u(-1500.0))) });
            pieces.push(HvacPiece { layer: SINGLE_LINE_LAYER, kind: rect(u(xc), u(-1850.0), u(250.0), u(350.0)) });
            pieces.push(HvacPiece { layer: SINGLE_LINE_LAYER, kind: line((u(xc), u(-2200.0)), (u(xc), u(busbar_y))) });
            let rating = supply.map_or("-".to_string(), |a| format!("{a:.0} A"));
            pieces.push(HvacPiece { layer: SINGLE_LINE_LAYER, kind: label_left(Vec2::new(u(xc + 400.0), u(-1850.0)), h, rating) });
            pieces.push(HvacPiece { layer: SINGLE_LINE_LAYER, kind: label(Vec2::new(u(xc), u(-600.0)), h * 1.3, name.clone()) });
            // The busbar, thick.
            let bar = vec![
                PolyVertex { p: Vec2::new(u(x0 + BRANCH_MM * 0.1), u(busbar_y)), bulge: 0.0, start_width: 0.0, end_width: 0.0 },
                PolyVertex { p: Vec2::new(u(x0 + width - BRANCH_MM * 0.1), u(busbar_y)), bulge: 0.0, start_width: 0.0, end_width: 0.0 },
            ];
            pieces.push(HvacPiece {
                layer: SINGLE_LINE_LAYER,
                kind: EntityKind::LwPolyline(LwPolyline { vertices: bar, closed: false, const_width: u(40.0), elevation: 0.0, plinegen: false }),
            });
            for (k, &ri) in members.iter().take(n).enumerate() {
                let Some(r) = reports.get(ri) else { continue };
                count += 1;
                let x = x0 + (k as f64 + 0.5) * BRANCH_MM;
                let layer = cable_layer_of(CableUsage::parse(&r.usage).unwrap_or_default(), &r.name);
                let down = |a: f64, b: f64| HvacPiece { layer, kind: line((u(x), u(a)), (u(x), u(b))) };
                // The branch: the line down from the busbar, the breaker, the cable, the load.
                pieces.push(down(busbar_y, -3300.0));
                pieces.push(HvacPiece { layer, kind: rect(u(x), u(-3650.0), u(220.0), u(350.0)) });
                pieces.push(down(-4000.0, -6000.0));
                let breaker = r.protection_a.or(r.rating_a).map_or("-".to_string(), |a| format!("{a:.0} A {}", p.options.curve.to_ascii_uppercase()));
                pieces.push(HvacPiece { layer, kind: label_left(Vec2::new(u(x + 300.0), u(-3650.0)), h, breaker) });
                let cable =
                    r.section_mm2.or(r.required_section_mm2).map_or("-".to_string(), |s| format!("{}G{s}", if r.phases == "three" { 5 } else { 3 }));
                pieces.push(HvacPiece {
                    layer,
                    kind: label_left(Vec2::new(u(x + 150.0), u(-4700.0)), h, format!("{cable}  {:.0} m", r.cable_length_m)),
                });
                if let Some(ik) = r.ik_min_a {
                    pieces.push(HvacPiece { layer, kind: label_left(Vec2::new(u(x + 150.0), u(-5000.0)), h * 0.85, format!("Ik min {ik:.0} A")) });
                }
                // The load: a box with the circuit's name, what it feeds and its power.
                pieces.push(HvacPiece { layer, kind: rect(u(x), u(-6500.0), u(BRANCH_MM * 0.42), u(500.0)) });
                let mark = if r.ok() { "" } else { "  !" };
                pieces.push(HvacPiece { layer, kind: label(Vec2::new(u(x), u(-6300.0)), h, format!("{}{mark}", r.name)) });
                let feeds = [(r.devices, "dev."), (r.hvac, "HVAC")]
                    .iter()
                    .filter(|(c, _)| *c > 0)
                    .map(|(c, w)| format!("{c} {w}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                pieces.push(HvacPiece {
                    layer,
                    kind: label(Vec2::new(u(x), u(-6650.0)), h * 0.9, format!("{feeds}  {:.1} kW  {:.1} A", r.power_w / 1000.0, r.current_a)),
                });
            }
            x0 += width + BRANCH_MM * 0.5;
        }
        if groups.is_empty() {
            pieces.push(HvacPiece { layer: SINGLE_LINE_LAYER, kind: label_left(Vec2::new(0.0, u(-600.0)), h, "-".to_string()) });
        }
        Ok(pieces)
    }

    /// Adds a diagram with its upper left corner at `at`.
    pub fn add_single_line(&mut self, space: &crate::Space, mut common: crate::Common, p: &SingleLineParams, at: Vec2) -> Result<crate::Handle> {
        if !at.is_finite() {
            return Err(invalid("A diagram needs a usable position."));
        }
        let pieces = self.generate_single_line(p)?;
        common.xdata = xdata::replace(SINGLE_LINE_APP, &common.xdata, p.to_xdata()?);
        common.layer = SINGLE_LINE_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(SINGLE_LINE_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Redraws diagram `h` for the circuits the drawing has now.
    pub fn regenerate_single_line(&mut self, h: crate::Handle, p: &SingleLineParams) -> Result<()> {
        let pieces = self.generate_single_line(p)?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(SINGLE_LINE_APP, &old, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// Redraws every single-line diagram of model space.
    pub fn refresh_single_lines(&mut self) {
        let diagrams: Vec<(crate::Handle, SingleLineParams)> = self.model.iter().filter_map(|e| single_line_of(e).map(|p| (e.handle, p))).collect();
        for (h, p) in diagrams {
            let _ = self.regenerate_single_line(h, &p);
        }
    }
}
