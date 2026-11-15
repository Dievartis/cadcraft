//! Pipework drawn in plan: the water networks of an air-conditioning plant (chilled water, hot
//! water, and the change-over network that carries one or the other by the season), condensate
//! drains and refrigerant lines. A pipe is one straight run drawn as two lines to scale, with its
//! size and service written on it, as a block reference with its parameters in extended data like
//! the ducts; the network the runs make is read back from where their ends meet.
//!
//! Colours. Condensate drains are blue with a yellow band and chilled water blue with a white one
//! (ISO 14726, the identification colours of piping: BU–YE–BU and BU–WH–BU); a plan shows the band
//! as a line along the axis between the two blue sides. The other services have no colour taken
//! from a standard here: hot water is red, the change-over network violet, the refrigerant liquid
//! line orange and the gas line grey, the supply of a water network in the strong tone and its
//! return in the light one.

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::pipe_networks::{intern, network_layer};
use super::{HvacPiece, HvacUnits, check_label_height, invalid, label, v3, xdata};
use crate::{EntityKind, Line, RawTag, Result};

/// The registered application name of a pipe's extended data.
pub const PIPE_APP: &str = "CADCRAFT_PIPE";
const PIPE_FORMAT: &str = "PIPE1";

/// The layers of the pipes: chilled water, hot water and change-over, supply and return; the
/// condensate drains; the refrigerant liquid and gas lines.
pub const PIPE_CHW_SUPPLY_LAYER: &str = "M-PIPE-CHWS";
pub const PIPE_CHW_RETURN_LAYER: &str = "M-PIPE-CHWR";
pub const PIPE_HW_SUPPLY_LAYER: &str = "M-PIPE-HWS";
pub const PIPE_HW_RETURN_LAYER: &str = "M-PIPE-HWR";
pub const PIPE_CO_SUPPLY_LAYER: &str = "M-PIPE-COS";
pub const PIPE_CO_RETURN_LAYER: &str = "M-PIPE-COR";
pub const PIPE_CONDENSATE_LAYER: &str = "M-PIPE-COND";
pub const PIPE_REFRIGERANT_LIQUID_LAYER: &str = "M-PIPE-REFL";
pub const PIPE_REFRIGERANT_GAS_LAYER: &str = "M-PIPE-REFG";
/// A refrigerant link drawn as one run for the liquid and the gas lines together.
pub const PIPE_REFRIGERANT_PAIR_LAYER: &str = "M-PIPE-REFP";
/// A heat-recovery refrigerant link: the liquid, the suction gas and the high/low-pressure gas lines together, and the
/// layer of the third tube.
pub const PIPE_REFRIGERANT_TRIPLE_LAYER: &str = "M-PIPE-REFT";
pub const PIPE_REFRIGERANT_HPLP_LAYER: &str = "M-PIPE-REFH";

/// Copper refrigeration tube sizes, which are given in inches: the label and the outer diameter, mm.
pub const REFRIGERANT_TUBES: [(&str, f64); 12] = [
    ("1/4", 6.35),
    ("3/8", 9.52),
    ("1/2", 12.7),
    ("5/8", 15.88),
    ("3/4", 19.05),
    ("7/8", 22.2),
    ("1 1/8", 28.58),
    ("1 3/8", 34.9),
    ("1 5/8", 41.28),
    ("2 1/8", 53.98),
    ("2 5/8", 66.68),
    ("3 1/8", 79.38),
];

/// The size of a refrigerant tube of outer diameter `mm`, as it is spoken: in inches ("3/8\"") when it is a standard
/// size, else the diameter in millimetres.
pub fn refrigerant_size_label(mm: f64) -> String {
    match REFRIGERANT_TUBES.iter().find(|(_, d)| (d - mm).abs() < 0.15) {
        Some((l, _)) => format!("{l}\""),
        None => format!("Ø{mm:.1}"),
    }
}

/// A refrigerant tube size typed as inches ("3/8", "3/8\"", "1 1/8", "1-1/8", "5/8in", "1.125\"") or as millimetres
/// ("9.52", "12 mm"): the outer diameter in millimetres. A bare number is millimetres; a mark (`"`, `in`) or a fraction
/// makes it inches.
pub fn parse_refrigerant_size(text: &str) -> Option<f64> {
    let t = text.trim().to_ascii_lowercase().replace(['”', '″'], "\"");
    let (body, inches_mark) = if let Some(b) = t.strip_suffix('"').or_else(|| t.strip_suffix("in")).or_else(|| t.strip_suffix("pouces")) {
        (b.trim().to_string(), true)
    } else if let Some(b) = t.strip_suffix("mm") {
        (b.trim().to_string(), false)
    } else {
        (t.clone(), false)
    };
    if body.trim_start().starts_with('-') {
        return None;
    }
    let body = body.replace('-', " ");
    let mm = if body.contains('/') {
        let mut whole = 0.0;
        let mut frac = None;
        for part in body.split_whitespace() {
            if let Some((n, d)) = part.split_once('/') {
                let (n, d) = (n.parse::<f64>().ok()?, d.parse::<f64>().ok()?);
                (d != 0.0).then_some(())?;
                frac = Some(n / d);
            } else {
                whole = part.parse::<f64>().ok()?;
            }
        }
        (whole + frac?) * 25.4
    } else {
        let v = body.trim().parse::<f64>().ok()?;
        if inches_mark { v * 25.4 } else { v }
    };
    // A standard size is the tube's real outer diameter (3/8" is 9.52 mm, not 9.525).
    let mm = REFRIGERANT_TUBES.iter().find(|(_, d)| (d - mm).abs() < 0.1).map_or(mm, |(_, d)| *d);
    (mm.is_finite() && mm > 0.0).then_some(mm)
}

/// The gap between the two tubes of a refrigerant pair, mm.
const PAIR_GAP_MM: f64 = 20.0;

/// Smallest and largest nominal diameter (outer diameter for a refrigerant line), mm.
pub const PIPE_DIAMETER_MM: (f64, f64) = (4.0, 600.0);
/// Largest insulation thickness, mm.
pub const MAX_PIPE_INSULATION_MM: f64 = 200.0;
/// Largest load typed on a run, kW.
pub const MAX_PIPE_LOAD_KW: f64 = 1_000_000.0;

/// Steel pipe sizes (ISO 6708 nominal diameter, ISO 4200 outer diameter), mm.
pub const STEEL_PIPE_SIZES: [(f64, f64); 15] = [
    (10.0, 17.2),
    (15.0, 21.3),
    (20.0, 26.9),
    (25.0, 33.7),
    (32.0, 42.4),
    (40.0, 48.3),
    (50.0, 60.3),
    (65.0, 76.1),
    (80.0, 88.9),
    (100.0, 114.3),
    (125.0, 139.7),
    (150.0, 168.3),
    (200.0, 219.1),
    (250.0, 273.0),
    (300.0, 323.9),
];

/// What a pipe carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PipeService {
    /// Chilled water (eau glacée).
    #[default]
    ChilledWater,
    /// Hot water (eau chaude).
    HotWater,
    /// The change-over network (change over): one pair of pipes that carries chilled water in summer and hot water in winter.
    ChangeOver,
    /// A condensate drain (évacuation de condensats), by gravity.
    Condensate,
    /// A refrigerant liquid line.
    RefrigerantLiquid,
    /// A refrigerant gas (suction) line.
    RefrigerantGas,
    /// The liquid and the gas lines of a refrigerant link, drawn together as one run.
    RefrigerantPair,
    /// The three lines of a heat-recovery link (liquid, suction gas, high/low-pressure gas), drawn together as one run.
    RefrigerantTriple,
}

impl PipeService {
    pub const ALL: [PipeService; 8] = [
        PipeService::ChilledWater,
        PipeService::HotWater,
        PipeService::ChangeOver,
        PipeService::Condensate,
        PipeService::RefrigerantLiquid,
        PipeService::RefrigerantGas,
        PipeService::RefrigerantPair,
        PipeService::RefrigerantTriple,
    ];

    /// The name a command takes.
    pub fn name(self) -> &'static str {
        match self {
            PipeService::ChilledWater => "chilledWater",
            PipeService::HotWater => "hotWater",
            PipeService::ChangeOver => "changeOver",
            PipeService::Condensate => "condensate",
            PipeService::RefrigerantLiquid => "refrigerantLiquid",
            PipeService::RefrigerantGas => "refrigerantGas",
            PipeService::RefrigerantPair => "refrigerantPair",
            PipeService::RefrigerantTriple => "refrigerantTriple",
        }
    }

    pub fn parse(text: &str) -> Option<PipeService> {
        let t: String = text.trim().to_ascii_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        PipeService::ALL.into_iter().find(|s| s.name().to_ascii_lowercase() == t).or(match t.as_str() {
            "chw" | "eauglacee" | "eauglacée" | "eauglace" | "eauglacé" => Some(PipeService::ChilledWater),
            "hw" | "eauchaude" => Some(PipeService::HotWater),
            "co" | "changeover" | "reversible" => Some(PipeService::ChangeOver),
            "cond" | "condensats" | "condensat" | "evacuation" | "drain" => Some(PipeService::Condensate),
            "refl" | "liquide" | "liquid" | "frigoliquide" => Some(PipeService::RefrigerantLiquid),
            "refg" | "gaz" | "gas" | "frigogaz" => Some(PipeService::RefrigerantGas),
            "ref" | "refp" | "pair" | "paire" | "liaison" | "liaisonfrigo" | "frigo" | "liquidegaz" => Some(PipeService::RefrigerantPair),
            "triple"
            | "reftriple"
            | "troistubes"
            | "3tubes"
            | "recuperation"
            | "recuperationdenergie"
            | "récupération"
            | "récupérationdénergie"
            | "heatrecovery"
            | "hr" => Some(PipeService::RefrigerantTriple),
            _ => None,
        })
    }

    /// Whether a pipe of the service draws a gas tube beside its liquid tube (the links).
    pub fn has_gas_tube(self) -> bool {
        matches!(self, PipeService::RefrigerantPair | PipeService::RefrigerantTriple)
    }

    /// Whether the service has a supply and a return pipe (the water networks).
    pub fn has_legs(self) -> bool {
        matches!(self, PipeService::ChilledWater | PipeService::HotWater | PipeService::ChangeOver)
    }

    /// Whether the diameter of a pipe of the service is the outer diameter of a copper tube (refrigerant),
    /// not the nominal diameter of a steel pipe.
    pub fn is_refrigerant(self) -> bool {
        matches!(self, PipeService::RefrigerantLiquid | PipeService::RefrigerantGas | PipeService::RefrigerantPair | PipeService::RefrigerantTriple)
    }
}

/// Which pipe of a water network: the supply (départ) or the return (retour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PipeLeg {
    #[default]
    Supply,
    Return,
}

impl PipeLeg {
    pub fn parse(text: &str) -> Option<PipeLeg> {
        match text.trim().to_ascii_lowercase().as_str() {
            "supply" | "s" | "depart" | "départ" | "aller" => Some(PipeLeg::Supply),
            "return" | "r" | "retour" => Some(PipeLeg::Return),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PipeLeg::Supply => "supply",
            PipeLeg::Return => "return",
        }
    }
}

fn default_label_mm() -> f64 {
    100.0
}

/// A pipe run: what its block reference keeps. The run starts at the block's origin and goes along
/// the block's x axis for `length`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipeParams {
    /// Length, in drawing units.
    pub length: f64,
    pub service: PipeService,
    /// The supply or the return, for a water network; the supply for the other services.
    #[serde(default)]
    pub leg: PipeLeg,
    /// The nominal diameter of a steel pipe, mm; the outer diameter of a refrigerant line's copper tube.
    pub dn_mm: f64,
    /// The outer diameter of the gas tube of a refrigerant pair, mm (`dn_mm` is then the liquid tube); 0 for the others.
    #[serde(default)]
    pub gas_dn_mm: f64,
    /// The outer diameter of the third tube of a heat-recovery link (the high/low-pressure gas line), mm; 0 for the others.
    #[serde(default)]
    pub gas2_dn_mm: f64,
    /// Whether the user chose the size: sizing then leaves it.
    #[serde(default)]
    pub imposed: bool,
    /// The insulation round the pipe, mm: it widens what a plan shows.
    #[serde(default)]
    pub insulation_mm: f64,
    /// The load served at this run's free end, kW (a unit's coil, say), or, for a condensate drain, the flow it
    /// takes in there, l/h. Sizing adds these up the network.
    #[serde(default)]
    pub load_kw: Option<f64>,
    /// Whether this run starts at the source of its network (a chiller, a boiler): sizing reads the network from it.
    #[serde(default)]
    pub source: bool,
    /// The flow the run carries once sized, m³/h (litres per hour for a condensate drain: the value is l/h / 1000).
    #[serde(default)]
    pub flow_m3h: Option<f64>,
    /// The velocity in the run, m/s, once sized.
    #[serde(default)]
    pub velocity: Option<f64>,
    /// The pressure lost per metre, Pa/m, once sized.
    #[serde(default)]
    pub loss_pa_m: Option<f64>,
    /// Which way the fluid runs, along the block's x axis (`true`) or against it, once sized.
    #[serde(default)]
    pub flow_forward: Option<bool>,
    /// The altitude of the run's underside, mm above the drawing's zero, when the user gave it.
    #[serde(default)]
    pub level_mm: Option<f64>,
    /// The name of the network the run belongs to (see [`super::pipe_networks`]): its layers, colour and line type, and its text.
    #[serde(default)]
    pub network: String,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl PipeParams {
    /// A run of `service` (the supply), `dn_mm` across, `length` drawing units long.
    pub fn new(service: PipeService, dn_mm: f64, units: HvacUnits) -> PipeParams {
        PipeParams {
            length: 1.0,
            service,
            leg: PipeLeg::Supply,
            dn_mm,
            gas_dn_mm: if service.has_gas_tube() { 15.88 } else { 0.0 },
            gas2_dn_mm: if service == PipeService::RefrigerantTriple { 19.05 } else { 0.0 },
            imposed: false,
            insulation_mm: 0.0,
            load_kw: None,
            source: false,
            flow_m3h: None,
            velocity: None,
            loss_pa_m: None,
            flow_forward: None,
            level_mm: None,
            network: String::new(),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !(self.length.is_finite() && self.length > 0.0 && self.length <= 1e9) {
            return Err(invalid("A pipe has a usable length."));
        }
        if !(self.dn_mm.is_finite() && (PIPE_DIAMETER_MM.0..=PIPE_DIAMETER_MM.1).contains(&self.dn_mm)) {
            return Err(invalid(format!("A pipe is {} to {} mm across.", PIPE_DIAMETER_MM.0, PIPE_DIAMETER_MM.1)));
        }
        let across = |v: f64| v.is_finite() && (PIPE_DIAMETER_MM.0..=PIPE_DIAMETER_MM.1).contains(&v);
        if (self.service.has_gas_tube() && !across(self.gas_dn_mm)) || (self.service == PipeService::RefrigerantTriple && !across(self.gas2_dn_mm)) {
            return Err(invalid(format!("The gas tubes are {} to {} mm across.", PIPE_DIAMETER_MM.0, PIPE_DIAMETER_MM.1)));
        }
        if !(self.insulation_mm.is_finite() && (0.0..=MAX_PIPE_INSULATION_MM).contains(&self.insulation_mm)) {
            return Err(invalid(format!("The insulation is 0 to {MAX_PIPE_INSULATION_MM} mm.")));
        }
        if self.load_kw.is_some_and(|l| !(l.is_finite() && (0.0..=MAX_PIPE_LOAD_KW).contains(&l))) {
            return Err(invalid("The load of a pipe is 0 or more."));
        }
        if self.level_mm.is_some_and(|l| !(l.is_finite() && l.abs() <= 1e6)) {
            return Err(invalid("The altitude of a pipe is not usable."));
        }
        if self.network.chars().count() > super::MAX_NETWORK_NAME_CHARS || self.network.chars().any(char::is_control) {
            return Err(invalid(format!("A network's name has at most {} characters, without control characters.", super::MAX_NETWORK_NAME_CHARS)));
        }
        self.units.validate()?;
        check_label_height(self.label_height_mm)
    }

    /// The outer diameter of the pipe, mm: the steel pipe of the nominal diameter, or the copper tube.
    pub fn outer_diameter_mm(&self) -> f64 {
        if self.service.is_refrigerant() {
            return self.dn_mm;
        }
        STEEL_PIPE_SIZES.iter().min_by(|a, b| (a.0 - self.dn_mm).abs().total_cmp(&(b.0 - self.dn_mm).abs())).map_or(self.dn_mm, |(_, od)| *od)
    }

    /// The width a plan shows: the outer diameter and the insulation either side; for a refrigerant pair, the two
    /// tubes side by side with a small gap.
    pub fn plan_width_mm(&self) -> f64 {
        let tubes = self.tubes();
        if tubes.is_empty() {
            return self.outer_diameter_mm() + 2.0 * self.insulation_mm;
        }
        tubes.iter().map(|(_, w)| w).sum::<f64>() + PAIR_GAP_MM * (tubes.len() as f64 - 1.0)
    }

    /// The tubes of a refrigerant link, from one side to the other: the layer and the width with the insulation, mm.
    /// Empty for a pipe that is a single run.
    pub(super) fn tubes(&self) -> Vec<(&'static str, f64)> {
        let w = |dn: f64| dn + 2.0 * self.insulation_mm;
        let tubes = self.raw_tubes(w);
        tubes.into_iter().map(|(layer, width)| (network_layer(layer, &self.network), width)).collect()
    }

    fn raw_tubes(&self, w: impl Fn(f64) -> f64) -> Vec<(&'static str, f64)> {
        match self.service {
            PipeService::RefrigerantPair => vec![(PIPE_REFRIGERANT_LIQUID_LAYER, w(self.dn_mm)), (PIPE_REFRIGERANT_GAS_LAYER, w(self.gas_dn_mm))],
            PipeService::RefrigerantTriple => vec![
                (PIPE_REFRIGERANT_LIQUID_LAYER, w(self.dn_mm)),
                (PIPE_REFRIGERANT_HPLP_LAYER, w(self.gas2_dn_mm)),
                (PIPE_REFRIGERANT_GAS_LAYER, w(self.gas_dn_mm)),
            ],
            _ => Vec::new(),
        }
    }

    /// The size as written: "DN50", the inches of a refrigerant tube ("3/8\""), or every tube of a link ("3/8\" - 5/8\"": the
    /// liquid, then the gas; for a heat-recovery link, the liquid, the suction gas and the high/low-pressure gas).
    pub fn size_text(&self) -> String {
        let inch = refrigerant_size_label;
        match self.service {
            PipeService::RefrigerantPair => format!("{} - {}", inch(self.dn_mm), inch(self.gas_dn_mm)),
            PipeService::RefrigerantTriple => format!("{} - {} - {}", inch(self.dn_mm), inch(self.gas_dn_mm), inch(self.gas2_dn_mm)),
            PipeService::RefrigerantLiquid | PipeService::RefrigerantGas => inch(self.dn_mm),
            _ => format!("DN{:.0}", self.dn_mm),
        }
    }

    /// The layer of the run: its service's, or the network's copy of it.
    pub fn layer(&self) -> &'static str {
        network_layer(layer_of(self.service, self.leg), &self.network)
    }

    /// Length in metres.
    pub fn length_m(&self) -> f64 {
        self.length * self.units.mm_per_unit / 1000.0
    }

    /// The short code of the service written on the run.
    pub fn tag(&self) -> &'static str {
        match (self.service, self.leg) {
            (PipeService::ChilledWater, PipeLeg::Supply) => "CHW-S",
            (PipeService::ChilledWater, PipeLeg::Return) => "CHW-R",
            (PipeService::HotWater, PipeLeg::Supply) => "HW-S",
            (PipeService::HotWater, PipeLeg::Return) => "HW-R",
            (PipeService::ChangeOver, PipeLeg::Supply) => "CO-S",
            (PipeService::ChangeOver, PipeLeg::Return) => "CO-R",
            (PipeService::Condensate, _) => "COND",
            (PipeService::RefrigerantLiquid, _) => "REF-L",
            (PipeService::RefrigerantGas, _) => "REF-G",
            (PipeService::RefrigerantPair, _) => "REF",
            (PipeService::RefrigerantTriple, _) => "REF-HR",
        }
    }

    /// The text written on the run: its size, its service and what sizing found.
    pub fn label_text(&self) -> String {
        let mut text = format!("{} {}", self.size_text(), self.tag());
        if !self.network.trim().is_empty() {
            text = format!("{} {text}", self.network.trim());
        }
        if let Some(f) = self.flow_m3h {
            if self.service == PipeService::Condensate {
                text.push_str(&format!(" {:.1} l/h", f * 1000.0));
            } else {
                text.push_str(&format!(" {f:.2} m³/h"));
            }
        }
        if let Some(l) = self.level_mm {
            text.push_str(&format!(" Ai:{:.2}", l / 1000.0));
        }
        text
    }

    /// The run drawn to scale: its two sides, the band along its axis for the services that have one, a
    /// label and, once sized, an arrow. `flip_label` turns the label half a turn, so it reads upright on a
    /// run drawn leftwards.
    pub fn generate(&self, flip_label: bool) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let r = self.units.to_units(self.plan_width_mm()) / 2.0;
        let layer = self.layer();
        let line = |layer: &'static str, a: Vec2, b: Vec2| HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(a), b: v3(b) }) };
        let tubes = self.tubes();
        let mut pieces = if tubes.is_empty() {
            vec![line(layer, Vec2::new(0.0, -r), Vec2::new(self.length, -r)), line(layer, Vec2::new(0.0, r), Vec2::new(self.length, r))]
        } else {
            // Each tube as two lines on its own layer, side by side with a small gap.
            let mut v = Vec::new();
            let mut y = -r;
            for (tube_layer, width) in &tubes {
                let w = self.units.to_units(*width);
                v.push(line(tube_layer, Vec2::new(0.0, y), Vec2::new(self.length, y)));
                v.push(line(tube_layer, Vec2::new(0.0, y + w), Vec2::new(self.length, y + w)));
                y += w + self.units.to_units(PAIR_GAP_MM);
            }
            v
        };
        if let Some(colour) = band_colour(self.service) {
            // The line along the axis is on the pipe's layer, drawn in the colour of the identification band.
            let band = intern(format!("{layer}|ByLayer|{colour}")).unwrap_or(layer);
            pieces.push(line(band, Vec2::ZERO, Vec2::new(self.length, 0.0)));
        }
        if let Some(forward) = self.flow_forward {
            let len = (2.4 * r).min(0.15 * self.length);
            let at = 0.25 * self.length;
            let dir = Vec2::new(if forward { 1.0 } else { -1.0 }, 0.0);
            let (tail, tip) = (Vec2::new(at, 0.0) - dir * (len / 2.0), Vec2::new(at, 0.0) + dir * (len / 2.0));
            let head = len * 0.4;
            let side = dir.perp() * (head * 0.6);
            pieces.extend([line(layer, tail, tip), line(layer, tip, tip - dir * head + side), line(layer, tip, tip - dir * head - side)]);
        }
        let h = self.units.to_units(self.label_height_mm);
        let up = if flip_label { -1.0 } else { 1.0 };
        let mut t = label(Vec2::new(self.length / 2.0, (r + 1.1 * h) * up), h, self.label_text());
        if let EntityKind::Text(x) = &mut t {
            x.rotation = if flip_label { std::f64::consts::PI } else { 0.0 };
        }
        pieces.push(HvacPiece { layer, kind: t });
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(PIPE_APP, PIPE_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<PipeParams> {
        let p: PipeParams = xdata::from_xdata(PIPE_APP, PIPE_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The service and leg a short tag stands for ("CHW-S", "HW-R", "CO-S", "COND", "REF-L", "REF-G", "REF", "REF-HR"; hyphen and case do
/// not matter), the tags the plan writes on the pipes.
pub fn pipe_tag_parse(text: &str) -> Option<(PipeService, PipeLeg)> {
    let key = |t: &str| t.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_uppercase();
    let wanted = key(text);
    for service in PipeService::ALL {
        for leg in [PipeLeg::Supply, PipeLeg::Return] {
            if !service.has_legs() && leg == PipeLeg::Return {
                continue;
            }
            if key(PipeParams { leg, ..PipeParams::new(service, 50.0, HvacUnits::default()) }.tag()) == wanted {
                return Some((service, leg));
            }
        }
    }
    None
}

/// The layer of a pipe of `service` and `leg`.
pub fn pipe_layer_of(service: PipeService, leg: PipeLeg) -> &'static str {
    layer_of(service, leg)
}

fn layer_of(service: PipeService, leg: PipeLeg) -> &'static str {
    match (service, leg) {
        (PipeService::ChilledWater, PipeLeg::Supply) => PIPE_CHW_SUPPLY_LAYER,
        (PipeService::ChilledWater, PipeLeg::Return) => PIPE_CHW_RETURN_LAYER,
        (PipeService::HotWater, PipeLeg::Supply) => PIPE_HW_SUPPLY_LAYER,
        (PipeService::HotWater, PipeLeg::Return) => PIPE_HW_RETURN_LAYER,
        (PipeService::ChangeOver, PipeLeg::Supply) => PIPE_CO_SUPPLY_LAYER,
        (PipeService::ChangeOver, PipeLeg::Return) => PIPE_CO_RETURN_LAYER,
        (PipeService::Condensate, _) => PIPE_CONDENSATE_LAYER,
        (PipeService::RefrigerantLiquid, _) => PIPE_REFRIGERANT_LIQUID_LAYER,
        (PipeService::RefrigerantGas, _) => PIPE_REFRIGERANT_GAS_LAYER,
        (PipeService::RefrigerantPair, _) => PIPE_REFRIGERANT_PAIR_LAYER,
        (PipeService::RefrigerantTriple, _) => PIPE_REFRIGERANT_TRIPLE_LAYER,
    }
}

/// The colour layer of the identification band of a service that has one (ISO 14726).
fn band_colour(service: PipeService) -> Option<&'static str> {
    match service {
        PipeService::ChilledWater => Some("M-PIPE-BAND-WHITE"),
        PipeService::Condensate => Some("M-PIPE-BAND-YELLOW"),
        _ => None,
    }
}

/// The colour a pipe layer, or a band, is created with.
pub fn pipe_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        // ISO 14726: blue, with a white band for chilled water and a yellow one for condensate.
        PIPE_CHW_SUPPLY_LAYER | PIPE_CONDENSATE_LAYER => Some(Color::True(Rgb(0x1e, 0x6f, 0xd9))),
        PIPE_CHW_RETURN_LAYER => Some(Color::True(Rgb(0x66, 0xb3, 0xff))),
        "M-PIPE-BAND-WHITE" => Some(Color::Index(7)),
        "M-PIPE-BAND-YELLOW" => Some(Color::True(Rgb(0xf5, 0xc4, 0x00))),
        // Not from a standard: hot water red, change-over violet, refrigerant orange (liquid) and grey (gas).
        PIPE_HW_SUPPLY_LAYER => Some(Color::True(Rgb(0xe0, 0x24, 0x24))),
        PIPE_HW_RETURN_LAYER => Some(Color::True(Rgb(0xff, 0x8a, 0x80))),
        PIPE_CO_SUPPLY_LAYER => Some(Color::True(Rgb(0x8e, 0x44, 0xad))),
        PIPE_CO_RETURN_LAYER => Some(Color::True(Rgb(0xc3, 0x9b, 0xd3))),
        PIPE_REFRIGERANT_LIQUID_LAYER => Some(Color::True(Rgb(0xe6, 0x7e, 0x22))),
        PIPE_REFRIGERANT_GAS_LAYER => Some(Color::True(Rgb(0x8d, 0x99, 0xa3))),
        PIPE_REFRIGERANT_PAIR_LAYER | PIPE_REFRIGERANT_TRIPLE_LAYER => Some(Color::True(Rgb(0xc8, 0x9a, 0x6a))),
        PIPE_REFRIGERANT_HPLP_LAYER => Some(Color::True(Rgb(0xc0, 0x5a, 0x6e))),
        _ => None,
    }
}

/// The pipe an object is, when it is a block reference carrying usable pipe data.
pub fn pipe_of(e: &crate::Entity) -> Option<PipeParams> {
    match &e.kind {
        EntityKind::Insert(_) => PipeParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

impl crate::Drawing {
    /// Adds a pipe from `a` to `b`; its `length` is set from the two points.
    pub fn add_pipe(&mut self, space: &crate::Space, mut common: crate::Common, mut p: PipeParams, a: Vec2, b: Vec2) -> Result<crate::Handle> {
        if !a.is_finite() || !b.is_finite() {
            return Err(invalid("A pipe needs usable end points."));
        }
        p.length = a.dist(b);
        let rotation = (b - a).angle();
        let pieces = p.generate(super::duct::reads_upside_down(rotation))?;
        common.xdata = xdata::replace(PIPE_APP, &common.xdata, p.to_xdata()?);
        common.layer = p.layer().into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(p.layer());
        let insert = crate::Insert { block, insert: v3(a), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives pipe `h` the parameters `p` (its length as it is now), redrawing it in place.
    pub fn regenerate_pipe(&mut self, h: crate::Handle, p: &PipeParams) -> Result<()> {
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let EntityKind::Insert(i) = &e.kind else { return Err(invalid("The object is not a block reference.")) };
        let pieces = p.generate(super::duct::reads_upside_down(i.rotation))?;
        let tags = xdata::replace(PIPE_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        let layer = p.layer();
        self.ensure_layer(layer);
        self.modify_entity(h, |e| e.common.layer = layer.into())?;
        Ok(())
    }

    /// Cuts pipe `h` for a part in its line: a gap of `gap` (drawing units) centred `t` along the pipe from its start. The pipe keeps
    /// what comes before the gap, and a new pipe, a copy of it, takes what comes after: its far end, so the load typed there.
    /// Returns the new pipe, or `None` when the gap does not fit inside the pipe (nothing changes).
    pub fn split_pipe(&mut self, h: crate::Handle, t: f64, gap: f64) -> Result<Option<crate::Handle>> {
        let Some((p, a, b)) = self.pipe_ends(h) else { return Err(invalid("The object is not a pipe.")) };
        let len = a.dist(b);
        let (t0, t1) = (t - gap / 2.0, t + gap / 2.0);
        if !(t.is_finite() && gap.is_finite() && gap >= 0.0 && len > 1e-9 && t0 > 1e-6 && t1 < len - 1e-6) {
            return Ok(None);
        }
        let dir = (b - a) / len;
        let common = self.entity(h).ok_or(crate::DocError::NoEntity(h))?.common.clone();
        let space = crate::Space::Model;
        let second = PipeParams { source: false, ..p.clone() };
        let new = self.add_pipe(&space, common, second, a + dir * t1, b)?;
        self.regenerate_pipe(h, &PipeParams { length: t0, load_kw: None, ..p })?;
        Ok(Some(new))
    }

    /// Pipe `h` with its data and its two ends in drawing coordinates.
    pub fn pipe_ends(&self, h: crate::Handle) -> Option<(PipeParams, Vec2, Vec2)> {
        let e = self.entity(h)?;
        let EntityKind::Insert(i) = &e.kind else { return None };
        let p = pipe_of(e)?;
        let a = Vec2::new(i.insert.x, i.insert.y);
        let b = a + Vec2::from_angle(i.rotation) * (p.length * i.scale.x);
        Some((p, a, b))
    }
}
