//! The legend of the pipe networks: one row for each network of the drawing, a sample of its pipe in its own colour and line type,
//! its name, the pipes it is made of and their length. It is a block reference with its settings in extended data, and the networks are
//! read again when it is redrawn: naming, renaming or restyling a network redraws the legends of the drawing.

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::{HvacPiece, HvacUnits, check_label_height, invalid, label_left, network_layer, pipe_layer_of, v3, xdata};
use crate::{EntityKind, Line, RawTag, Result};

/// The registered application name of a legend's extended data.
pub const NETWORK_LEGEND_APP: &str = "CADCRAFT_NETLEGEND";
/// The layer of the legend's frame and title; each row is on its network's layer.
pub const NETWORK_LEGEND_LAYER: &str = "M-PIPE-LEGEND";
const XDATA_FORMAT: &str = "NETLEGEND1";
/// Most rows a legend draws.
const MAX_ROWS: usize = 200;

fn default_label_mm() -> f64 {
    150.0
}

fn default_title() -> String {
    "Networks".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkLegendParams {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl NetworkLegendParams {
    pub fn new(units: HvacUnits) -> NetworkLegendParams {
        NetworkLegendParams { title: default_title(), label_height_mm: default_label_mm(), units }
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
        xdata::to_xdata(NETWORK_LEGEND_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<NetworkLegendParams> {
        let p: NetworkLegendParams = xdata::from_xdata(NETWORK_LEGEND_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The legend an object is, when it is a block reference carrying usable legend data.
pub fn network_legend_of(e: &crate::Entity) -> Option<NetworkLegendParams> {
    match &e.kind {
        EntityKind::Insert(_) => NetworkLegendParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// The legend's pieces for the networks the drawing has now: the title, then a row for each network.
    pub fn generate_network_legend(&self, p: &NetworkLegendParams) -> Result<Vec<HvacPiece>> {
        p.validate()?;
        let u = p.units;
        let h = u.to_units(p.label_height_mm);
        let line = |layer: &'static str, a: Vec2, b: Vec2| HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(a), b: v3(b) }) };
        let mut pieces = vec![HvacPiece { layer: NETWORK_LEGEND_LAYER, kind: label_left(Vec2::new(0.0, 0.0), h * 1.2, p.title.clone()) }];
        let (sample, gap) = (u.to_units(1000.0), u.to_units(120.0));
        let mut y = -2.2 * h;
        for n in self.pipe_network_summaries().iter().take(MAX_ROWS) {
            // The row is on the layer its network's pipes are on, so it is the colour and the line type they have.
            let (service, leg) = n.services.first().copied().unwrap_or_default();
            let layer = network_layer(pipe_layer_of(service, leg), &n.name);
            pieces.push(line(layer, Vec2::new(0.0, y + gap / 2.0), Vec2::new(sample, y + gap / 2.0)));
            pieces.push(line(layer, Vec2::new(0.0, y - gap / 2.0), Vec2::new(sample, y - gap / 2.0)));
            let tags: Vec<String> =
                n.services.iter().map(|(s, l)| PipeParamsTag::tag(*s, *l)).collect::<Vec<_>>().into_iter().fold(Vec::new(), |mut v, t| {
                    if !v.contains(&t) {
                        v.push(t);
                    }
                    v
                });
            let text = format!("{}  {}  {:.0} m", n.name, tags.join("/"), n.length_m);
            pieces.push(HvacPiece { layer, kind: label_left(Vec2::new(sample + 1.5 * h, y), h, text) });
            y -= 2.0 * h;
        }
        if self.pipe_network_summaries().is_empty() {
            pieces.push(HvacPiece { layer: NETWORK_LEGEND_LAYER, kind: label_left(Vec2::new(0.0, y), h, "-".to_string()) });
        }
        Ok(pieces)
    }

    /// Adds a legend with its upper left corner at `at`.
    pub fn add_network_legend(
        &mut self,
        space: &crate::Space,
        mut common: crate::Common,
        p: &NetworkLegendParams,
        at: Vec2,
    ) -> Result<crate::Handle> {
        if !at.is_finite() {
            return Err(invalid("A legend needs a usable position."));
        }
        let pieces = self.generate_network_legend(p)?;
        common.xdata = xdata::replace(NETWORK_LEGEND_APP, &common.xdata, p.to_xdata()?);
        common.layer = NETWORK_LEGEND_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(NETWORK_LEGEND_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Redraws legend `h` for the networks the drawing has now.
    pub fn regenerate_network_legend(&mut self, h: crate::Handle, p: &NetworkLegendParams) -> Result<()> {
        let pieces = self.generate_network_legend(p)?;
        let old = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.xdata.clone();
        let tags = xdata::replace(NETWORK_LEGEND_APP, &old, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// Redraws every legend of model space: the networks, their colours or their lengths changed.
    pub fn refresh_network_legends(&mut self) {
        let legends: Vec<(crate::Handle, NetworkLegendParams)> =
            self.model.iter().filter_map(|e| network_legend_of(e).map(|p| (e.handle, p))).collect();
        for (h, p) in legends {
            let _ = self.regenerate_network_legend(h, &p);
        }
    }
}

/// The tag a service and a leg are written with.
struct PipeParamsTag;

impl PipeParamsTag {
    fn tag(service: super::PipeService, leg: super::PipeLeg) -> String {
        super::PipeParams { leg, ..super::PipeParams::new(service, 50.0, HvacUnits::default()) }.tag().to_string()
    }
}
