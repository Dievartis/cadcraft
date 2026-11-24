//! The colours of the pipes and ducts of a plan: the plan's own (what the trades draw them in: hot water red, chilled water blue…) or those of NF X 08-100,
//! the French standard that marks the pipes of an installation by the family of the fluid (a ground colour) and, for some, its state (a band).
//!
//! What NF X 08-100 gives (table I and II of the INRS leaflet ED 88): ground colours air light blue, steam light grey, other gases medium orange-yellow,
//! water yellow-green, mineral, vegetable and animal oils light brown, acids and bases pale violet, other liquids black, fire-fighting fluids bright
//! orange-red; state colours hot or superheated orange-grey, cold or cooled medium violet, liquefied gas medium pink, under pressure bright orange-red,
//! rarefied gas light blue, polluted or vitiated medium brown. The standard names the colours and refers to a colour chart for their values: the values
//! here are close tones chosen for a screen and for paper, to be checked against the chart (`NF X 08-100` is a standard to buy).
//!
//! A plan shows the state as a line along the axis of the pipe, as the identification bands of ISO 14726 are shown. A duct has no such line here: in this
//! convention a supply or fresh air duct is light blue (air), and the ducts that carry air that is polluted or vitiated (extract, exhaust) take the colour of
//! that state, so that they are told apart as the plan's colours tell them apart.

use cadcraft_color::{Color, Rgb};
use serde::{Deserialize, Serialize};

use super::pipe::{
    PIPE_BECS_LAYER, PIPE_CHW_RETURN_LAYER, PIPE_CHW_SUPPLY_LAYER, PIPE_CO_RETURN_LAYER, PIPE_CO_SUPPLY_LAYER, PIPE_CONDENSATE_LAYER, PIPE_ECS_LAYER,
    PIPE_EF_LAYER, PIPE_EP_LAYER, PIPE_EU_LAYER, PIPE_EV_LAYER, PIPE_HW_RETURN_LAYER, PIPE_HW_SUPPLY_LAYER, PIPE_REFRIGERANT_GAS_LAYER,
    PIPE_REFRIGERANT_HPLP_LAYER, PIPE_REFRIGERANT_LIQUID_LAYER, PIPE_REFRIGERANT_PAIR_LAYER, PIPE_REFRIGERANT_TRIPLE_LAYER,
};
use super::{
    AIR_EXHAUST_LAYER, AIR_EXTRACT_LAYER, AIR_FRESH_LAYER, AIR_SUPPLY_LAYER, DUCT_EXHAUST_LAYER, DUCT_EXTRACT_LAYER, DUCT_FRESH_LAYER, DUCT_LAYER,
};

/// Which colours the pipes and ducts of a drawing are drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColourConvention {
    /// The plan's own colours.
    #[default]
    Plan,
    /// The ground and state colours of NF X 08-100.
    NfX08100,
}

impl ColourConvention {
    pub fn name(self) -> &'static str {
        match self {
            ColourConvention::Plan => "plan",
            ColourConvention::NfX08100 => "nfx08100",
        }
    }

    pub fn parse(text: &str) -> Option<ColourConvention> {
        let t: String = text.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        match t.as_str() {
            "plan" | "default" | "usage" => Some(ColourConvention::Plan),
            "nfx08100" | "nf" | "nfx" | "x08100" | "norm" | "norme" => Some(ColourConvention::NfX08100),
            _ => None,
        }
    }

    pub(crate) fn is_plan(&self) -> bool {
        *self == ColourConvention::Plan
    }

    /// Whether this is the plan's own, which a drawing does not need to say.
    pub fn is_plan_default(&self) -> bool {
        self.is_plan()
    }
}

/// Water, yellow-green (the ground colour of every water that is not for fire-fighting); the return of a network in the lighter tone.
const WATER: Rgb = Rgb(0x8f, 0xc3, 0x1f);
const WATER_LIGHT: Rgb = Rgb(0xc4, 0xe0, 0x7a);
/// Air, light blue.
const AIR: Rgb = Rgb(0x6e, 0xc6, 0xf5);
/// Other gases, medium orange-yellow: the refrigerants.
const OTHER_GAS: Rgb = Rgb(0xf2, 0xa9, 0x00);
/// States.
const COLD: Rgb = Rgb(0x9b, 0x5f, 0xc0);
const HOT: Rgb = Rgb(0xc9, 0x8b, 0x5a);
const POLLUTED: Rgb = Rgb(0x8a, 0x5a, 0x2b);
const LIQUEFIED: Rgb = Rgb(0xe8, 0x8b, 0xb5);

/// The layer of the band along a pipe that tells the fluid is cold, hot, polluted or a liquefied gas.
pub const PIPE_BAND_COLD_LAYER: &str = "M-PIPE-BAND-COLD";
pub const PIPE_BAND_HOT_LAYER: &str = "M-PIPE-BAND-HOT";
pub const PIPE_BAND_POLLUTED_LAYER: &str = "M-PIPE-BAND-POLLUTED";
pub const PIPE_BAND_LIQUEFIED_LAYER: &str = "M-PIPE-BAND-LIQUEFIED";

/// The colour a pipe, duct or band layer has in the convention of NF X 08-100; `None` for a layer it does not colour.
pub fn nf_layer_color(name: &str) -> Option<Color> {
    let c = |rgb: Rgb| Some(Color::True(rgb));
    match name {
        PIPE_CHW_SUPPLY_LAYER | PIPE_HW_SUPPLY_LAYER | PIPE_CO_SUPPLY_LAYER | PIPE_CONDENSATE_LAYER => c(WATER),
        PIPE_CHW_RETURN_LAYER | PIPE_HW_RETURN_LAYER | PIPE_CO_RETURN_LAYER => c(WATER_LIGHT),
        PIPE_EF_LAYER | PIPE_ECS_LAYER | PIPE_BECS_LAYER | PIPE_EU_LAYER | PIPE_EV_LAYER | PIPE_EP_LAYER => c(WATER),
        PIPE_REFRIGERANT_LIQUID_LAYER
        | PIPE_REFRIGERANT_GAS_LAYER
        | PIPE_REFRIGERANT_PAIR_LAYER
        | PIPE_REFRIGERANT_TRIPLE_LAYER
        | PIPE_REFRIGERANT_HPLP_LAYER => c(OTHER_GAS),
        PIPE_BAND_COLD_LAYER => c(COLD),
        PIPE_BAND_HOT_LAYER => c(HOT),
        PIPE_BAND_POLLUTED_LAYER => c(POLLUTED),
        PIPE_BAND_LIQUEFIED_LAYER => c(LIQUEFIED),
        DUCT_LAYER | DUCT_FRESH_LAYER | AIR_SUPPLY_LAYER | AIR_FRESH_LAYER => c(AIR),
        DUCT_EXTRACT_LAYER | DUCT_EXHAUST_LAYER | AIR_EXTRACT_LAYER | AIR_EXHAUST_LAYER => c(POLLUTED),
        _ => None,
    }
}

impl crate::Drawing {
    /// The colours the drawing's pipes and ducts are drawn in.
    pub fn colour_convention(&self) -> ColourConvention {
        self.colour_convention
    }

    /// The colour layer `name` is created with in this drawing: the convention's, else the plan's.
    pub(crate) fn hvac_colour(&self, name: &str) -> Option<Color> {
        if self.colour_convention == ColourConvention::NfX08100 {
            nf_layer_color(name).or_else(|| super::hvac_layer_color(name))
        } else {
            super::hvac_layer_color(name)
        }
    }

    /// Draws the pipes and ducts in `convention`: the colours of their layers (those the convention colours, which are the standard ones: a layer the user
    /// made is left alone) and the bands along the pipes. Returns how many layers and how many pipes changed.
    pub fn set_colour_convention(&mut self, convention: ColourConvention) -> crate::Result<(usize, usize)> {
        self.colour_convention = convention;
        let mut layers = 0;
        let names: Vec<String> = self.layers.iter().map(|l| l.name.clone()).collect();
        for name in names {
            if nf_layer_color(&name).is_none() {
                continue;
            }
            let Some(colour) = self.hvac_colour(&name) else { continue };
            if let Some(l) = self.layers.iter_mut().find(|l| l.name == name)
                && l.color != colour
            {
                l.color = colour;
                layers += 1;
            }
        }
        // The pipes draw their band: drawn again with the convention (the bands' layers are made on the way).
        let pipes: Vec<crate::Handle> =
            self.model.iter().filter(|e| super::pipe_of(e).is_some()).map(|e| e.handle).take(super::MAX_PIPE_RUNS).collect();
        let mut changed = 0;
        for h in pipes {
            let Some(mut p) = self.entity(h).and_then(|e| super::pipe_of(e)) else { continue };
            if p.colours != convention {
                p.colours = convention;
                self.regenerate_pipe(h, &p)?;
                changed += 1;
            }
        }
        Ok((layers, changed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_convention_is_named_and_parsed() {
        for c in [ColourConvention::Plan, ColourConvention::NfX08100] {
            assert_eq!(ColourConvention::parse(c.name()), Some(c));
        }
        assert_eq!(ColourConvention::parse("NF X 08-100"), Some(ColourConvention::NfX08100));
        assert_eq!(ColourConvention::parse("rouge"), None);
    }

    #[test]
    fn every_water_is_yellow_green_air_light_blue_and_the_states_have_their_bands() {
        let rgb = |n: &str| match nf_layer_color(n) {
            Some(Color::True(c)) => c,
            other => panic!("{n}: {other:?}"),
        };
        for n in [
            PIPE_CHW_SUPPLY_LAYER,
            PIPE_HW_SUPPLY_LAYER,
            PIPE_CO_SUPPLY_LAYER,
            PIPE_CONDENSATE_LAYER,
            PIPE_EF_LAYER,
            PIPE_ECS_LAYER,
            PIPE_EU_LAYER,
            PIPE_EP_LAYER,
        ] {
            assert_eq!(rgb(n), WATER, "{n}");
        }
        assert_eq!((rgb(DUCT_LAYER), rgb(DUCT_FRESH_LAYER)), (AIR, AIR));
        assert_eq!((rgb(DUCT_EXTRACT_LAYER), rgb(DUCT_EXHAUST_LAYER)), (POLLUTED, POLLUTED));
        assert_eq!(rgb(PIPE_REFRIGERANT_GAS_LAYER), OTHER_GAS);
        let bands = [rgb(PIPE_BAND_COLD_LAYER), rgb(PIPE_BAND_HOT_LAYER), rgb(PIPE_BAND_POLLUTED_LAYER), rgb(PIPE_BAND_LIQUEFIED_LAYER)];
        assert_eq!(bands, [COLD, HOT, POLLUTED, LIQUEFIED]);
        assert!(nf_layer_color("A-WALL").is_none());
    }
}
