//! Names, colours and line types of the pipe networks. A network is a set of pipes that are one circuit (the supply and the return
//! of one chilled-water loop, the condensate drains of one stack…); it has a name, and the drawing keeps a style for each name: a colour
//! from a palette of twelve distinct hues, so that two networks of the same service can be told apart on a plan at a glance, and a
//! line type from five (continuous unless it is chosen), so that they can also be told apart on a print without colour. Each network has a layer of its own for each layer of its service
//! (`M-PIPE-CHWS-CTA1`), created with the style's colour and line type; the return of a water network is the same hue, lighter.
//! Changing a layer's colour by hand sticks until the style is changed again.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use cadcraft_color::{Color, Rgb};
use serde::{Deserialize, Serialize};

use super::{
    PART_EQUIPMENT_LAYER, PART_LAYER, PART_REFRIGERANT_LAYER, PIPE_CHW_RETURN_LAYER, PIPE_CHW_SUPPLY_LAYER, PIPE_CO_RETURN_LAYER,
    PIPE_CO_SUPPLY_LAYER, PIPE_CONDENSATE_LAYER, PIPE_HW_RETURN_LAYER, PIPE_HW_SUPPLY_LAYER, PIPE_REFRIGERANT_GAS_LAYER, PIPE_REFRIGERANT_HPLP_LAYER,
    PIPE_REFRIGERANT_LIQUID_LAYER, PIPE_REFRIGERANT_PAIR_LAYER, PIPE_REFRIGERANT_TRIPLE_LAYER, invalid,
};
use crate::Result;

/// Twelve hues that stay apart on a plan and for most colour-blind readers (the first eight are the Okabe–Ito set, which is public).
pub const NETWORK_PALETTE: [(&str, [u8; 3]); 12] = [
    ("Blue", [0x00, 0x72, 0xb2]),
    ("Orange", [0xe6, 0x9f, 0x00]),
    ("Green", [0x00, 0x9e, 0x73]),
    ("Vermillion", [0xd5, 0x5e, 0x00]),
    ("Pink", [0xcc, 0x79, 0xa7]),
    ("Sky blue", [0x56, 0xb4, 0xe9]),
    ("Purple", [0x6a, 0x3d, 0x9a]),
    ("Brown", [0x8c, 0x56, 0x4b]),
    ("Teal", [0x1b, 0x7f, 0x7a]),
    ("Maroon", [0x8b, 0x1a, 0x2b]),
    ("Olive", [0x7a, 0x8c, 0x2a]),
    ("Gold", [0xb8, 0x86, 0x0b]),
];

/// The line types a network can have, with the English word for each; the first is the drawing's own continuous line. The others
/// are ours, their lengths in millimetres of the paper-size drawing.
pub const NETWORK_LINETYPES: [(&str, &str); 5] =
    [("CONTINUOUS", "Continuous"), ("NET_DASH", "Dashed"), ("NET_DASHDOT", "Dash-dot"), ("NET_DOT", "Dotted"), ("NET_LONG", "Long dash")];

/// Most networks a drawing keeps a style for, and the longest name.
pub const MAX_NETWORKS: usize = 1000;
pub const MAX_NETWORK_NAME_CHARS: usize = 32;
/// Most characters of a name that go into a layer's name.
const LAYER_SUFFIX_CHARS: usize = 24;
/// Most distinct network layer names one program run keeps.
const MAX_INTERNED: usize = 4096;

/// The layers a network gets a copy of: those of the pipes and of the parts.
const NETWORK_BASES: [&str; 20] = [
    super::CABLE_BUS_LAYER,
    super::CABLE_POWER_LAYER,
    super::CABLE_LIGHT_LAYER,
    super::CABLE_DATA_LAYER,
    super::CABLE_SAFETY_LAYER,
    PIPE_CHW_SUPPLY_LAYER,
    PIPE_CHW_RETURN_LAYER,
    PIPE_HW_SUPPLY_LAYER,
    PIPE_HW_RETURN_LAYER,
    PIPE_CO_SUPPLY_LAYER,
    PIPE_CO_RETURN_LAYER,
    PIPE_CONDENSATE_LAYER,
    PIPE_REFRIGERANT_LIQUID_LAYER,
    PIPE_REFRIGERANT_GAS_LAYER,
    PIPE_REFRIGERANT_PAIR_LAYER,
    PIPE_REFRIGERANT_TRIPLE_LAYER,
    PIPE_REFRIGERANT_HPLP_LAYER,
    PART_LAYER,
    PART_EQUIPMENT_LAYER,
    PART_REFRIGERANT_LAYER,
];

/// The style of a network: its name, its colour and its line type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStyle {
    pub name: String,
    pub color: [u8; 3],
    pub linetype: String,
}

impl NetworkStyle {
    /// The style a network takes when it is the `index`th of the drawing: the palette's hue and a continuous line. The colour is what
    /// tells the networks apart at a glance; a line type is chosen network by network, for a plan that is printed without colour.
    pub fn nth(name: &str, index: usize) -> NetworkStyle {
        let (_, rgb) = NETWORK_PALETTE[index % NETWORK_PALETTE.len()];
        NetworkStyle { name: name.to_string(), color: rgb, linetype: NETWORK_LINETYPES[0].0.to_string() }
    }
}

/// Whether two network names are the same: the case does not matter, accents included.
pub fn same_network(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// A network name as it is written in a layer's name: capitals, digits and underscores, accents dropped.
pub fn network_suffix(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        let c = match c {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'à' | 'â' | 'ä' | 'À' | 'Â' => 'A',
            'î' | 'ï' | 'Î' | 'Ï' => 'I',
            'ô' | 'ö' | 'Ô' => 'O',
            'ù' | 'û' | 'ü' | 'Ù' | 'Û' => 'U',
            'ç' | 'Ç' => 'C',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else if matches!(c, ' ' | '_' | '-') && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
        if out.chars().count() >= LAYER_SUFFIX_CHARS {
            break;
        }
    }
    out.trim_end_matches('_').to_string()
}

fn pool() -> &'static Mutex<HashMap<String, &'static str>> {
    static POOL: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    POOL.get_or_init(Default::default)
}

/// A string kept for the life of the program, once for each distinct text (the pieces of a drawing name their layer by a `&'static str`).
/// `None` past [`MAX_INTERNED`] distinct texts.
pub(super) fn intern(text: String) -> Option<&'static str> {
    let mut pool = pool().lock().ok()?;
    if let Some(s) = pool.get(&text) {
        return Some(s);
    }
    if pool.len() >= MAX_INTERNED {
        return None;
    }
    let leaked: &'static str = Box::leak(text.clone().into_boxed_str());
    pool.insert(text, leaked);
    Some(leaked)
}

/// The layer of `network` that goes with `base`: `M-PIPE-CHWS-CTA1`; `base` itself for no network.
pub fn network_layer(base: &'static str, network: &str) -> &'static str {
    let suffix = network_suffix(network);
    if suffix.is_empty() {
        return base;
    }
    intern(format!("{base}-{suffix}")).unwrap_or(base)
}

/// The base layer and the network suffix of `layer`, when it is a network layer.
fn split_network_layer(layer: &str) -> Option<(&'static str, &str)> {
    NETWORK_BASES.iter().find_map(|base| layer.strip_prefix(base).and_then(|r| r.strip_prefix('-')).map(|suffix| (*base, suffix)))
}

/// The layer a network layer is a copy of (`M-PIPE-CHWS` for `M-PIPE-CHWS-CTA1`): until a network has a style, it keeps its service's colour.
pub(super) fn network_base(layer: &str) -> Option<&'static str> {
    split_network_layer(layer).map(|(base, _)| base)
}

/// The colour a layer takes for a style: the return of a water network is the same hue, lighter.
fn layer_color(base: &str, rgb: [u8; 3]) -> Color {
    let lighter = matches!(base, PIPE_CHW_RETURN_LAYER | PIPE_HW_RETURN_LAYER | PIPE_CO_RETURN_LAYER);
    let k = |v: u8| if lighter { (f64::from(v) + (255.0 - f64::from(v)) * 0.5).round() as u8 } else { v };
    Color::True(Rgb(k(rgb[0]), k(rgb[1]), k(rgb[2])))
}

impl crate::Drawing {
    /// The style of network `name`, when the drawing has one.
    pub fn network_style(&self, name: &str) -> Option<&NetworkStyle> {
        self.network_styles.iter().find(|s| same_network(&s.name, name))
    }

    /// The colour and the line type a network layer is created with, when `layer` is one and its network has a style.
    pub(super) fn network_look(&self, layer: &str) -> Option<(Color, String)> {
        let (base, suffix) = split_network_layer(layer)?;
        let style = self.network_styles.iter().find(|s| network_suffix(&s.name) == suffix)?;
        Some((layer_color(base, style.color), style.linetype.clone()))
    }

    /// Makes sure line type `name` is in the drawing: the continuous one always is, ours are made in the drawing's units.
    pub(super) fn ensure_network_linetype(&mut self, name: &str) {
        if self.linetype(name).is_some() {
            return;
        }
        let u = super::HvacUnits::of(self);
        let (desc, dashes): (&str, &[f64]) = match name {
            "NET_DASH" => ("Network, dashed", &[150.0, -80.0]),
            "NET_DASHDOT" => ("Network, dash-dot", &[200.0, -60.0, 20.0, -60.0]),
            "NET_DOT" => ("Network, dotted", &[20.0, -70.0]),
            "NET_LONG" => ("Network, long dash", &[350.0, -100.0]),
            // One of the library's (DASHED, CENTER…): made in the drawing's units, as the line types menu does.
            other => {
                if let Some(lt) = crate::library::standard_linetypes_for(self).into_iter().find(|l| l.name.eq_ignore_ascii_case(other)) {
                    self.linetypes.push(lt);
                }
                return;
            }
        };
        let lengths: Vec<f64> = dashes.iter().map(|d| u.to_units(*d)).collect();
        self.linetypes.push(crate::Linetype::simple(name, desc, &lengths));
    }

    /// Sets the style of network `name`, made with the next hue and line type when it has none: the colour as red, green and blue, and the
    /// line type by name (one of [`NETWORK_LINETYPES`], or a line type of the drawing). The layers of the network follow.
    pub fn set_network_style(&mut self, name: &str, color: Option<[u8; 3]>, linetype: Option<&str>) -> Result<NetworkStyle> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NETWORK_NAME_CHARS || name.chars().any(char::is_control) || network_suffix(name).is_empty() {
            return Err(invalid(format!(
                "A network's name has 1 to {MAX_NETWORK_NAME_CHARS} characters, letters or digits, without control characters."
            )));
        }
        if let Some(lt) = linetype
            && !NETWORK_LINETYPES.iter().any(|(n, _)| n.eq_ignore_ascii_case(lt))
            && self.linetype(lt).is_none()
            && !crate::library::standard_linetypes().iter().any(|l| l.name.eq_ignore_ascii_case(lt))
        {
            return Err(invalid("That line type is not one of the network line types."));
        }
        let at = self.network_styles.iter().position(|s| same_network(&s.name, name));
        if at.is_none() && self.network_styles.len() >= MAX_NETWORKS {
            return Err(invalid(format!("A drawing keeps the style of at most {MAX_NETWORKS} networks.")));
        }
        let lt_name = linetype.map(|lt| {
            NETWORK_LINETYPES
                .iter()
                .map(|(n, _)| *n)
                .chain(crate::library::standard_linetypes().iter().map(|l| l.name.as_str()).collect::<Vec<_>>())
                .find(|n| n.eq_ignore_ascii_case(lt))
                .map_or_else(|| lt.to_string(), str::to_string)
        });
        let mut style = at.and_then(|i| self.network_styles.get(i).cloned()).unwrap_or_else(|| NetworkStyle::nth(name, self.network_styles.len()));
        style.name = at.and_then(|i| self.network_styles.get(i)).map_or_else(|| name.to_string(), |s| s.name.clone());
        if let Some(c) = color {
            style.color = c;
        }
        if let Some(lt) = lt_name {
            style.linetype = lt;
        }
        match at.and_then(|i| self.network_styles.get_mut(i)) {
            Some(slot) => *slot = style.clone(),
            None => self.network_styles.push(style.clone()),
        }
        self.refresh_network_layers();
        self.refresh_network_legends();
        self.refresh_circuit_legends();
        Ok(style)
    }

    /// Gives the layers of every network the colour and the line type of its style (the colour a user typed on a layer is replaced).
    pub fn refresh_network_layers(&mut self) {
        let looks: Vec<(usize, Color, String)> =
            self.layers.iter().enumerate().filter_map(|(i, l)| self.network_look(&l.name).map(|(c, lt)| (i, c, lt))).collect();
        for (i, color, linetype) in looks {
            self.ensure_network_linetype(&linetype);
            if let Some(l) = self.layers.get_mut(i) {
                l.color = color;
                l.linetype = linetype;
            }
        }
    }
}
