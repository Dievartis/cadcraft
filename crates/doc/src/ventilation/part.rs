//! Pipework parts: the valves, fittings, instruments, pumps, vessels and heat sources of a hydraulic network, and the
//! branches, headers and branch-selector boxes of a refrigerant one. A part is a block reference to an anonymous block
//! holding a plan symbol drawn to scale (every symbol is drawn here, in code), its data travelling as extended data of its own
//! registered application, like the other plan objects. The library of ready-made models is in [`super::part_library`].

use cadcraft_geom::{PolyVertex, Vec2};
use serde::{Deserialize, Serialize};

use super::{HvacPiece, HvacUnits, check_label_height, check_name, invalid, label, v3, xdata};
use crate::{Arc, Circle, EntityKind, Line, LwPolyline, RawTag, Result};

/// The registered application name of a part's extended data.
pub const PART_APP: &str = "CADCRAFT_PART";
/// The layers of the parts: valves, fittings and instruments; pumps, vessels and heat sources; refrigerant branches.
pub const PART_LAYER: &str = "M-PIPE-ACC";
pub const PART_EQUIPMENT_LAYER: &str = "M-PIPE-EQP";
pub const PART_REFRIGERANT_LAYER: &str = "M-PIPE-REFACC";
/// The layer of the safety devices (safety valves, disconnectors): red.
pub const PART_SAFETY_LAYER: &str = "M-PIPE-SAFETY";
/// Largest side of a part in plan, mm.
pub const MAX_PART_MM: f64 = 20_000.0;
/// Most outlets of a header or a box.
pub const MAX_PART_OUTLETS: u32 = 64;
const XDATA_FORMAT: &str = "PART1";
const MAX_TEXT_CHARS: usize = 400;

fn default_label_mm() -> f64 {
    100.0
}

/// What a part is: the symbol it is drawn with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PartKind {
    #[default]
    IsolationValve,
    ButterflyValve,
    CheckValve,
    BalancingValve,
    ControlValve,
    ThreeWayValve,
    PicValve,
    SafetyValve,
    PressureReducingValve,
    DrainValve,
    Strainer,
    FlexConnector,
    Reducer,
    AirVent,
    Deaerator,
    DirtSeparator,
    Disconnector,
    FillingGroup,
    Thermometer,
    PressureGauge,
    FlowMeter,
    EnergyMeter,
    Sensor,
    Pump,
    DoublePump,
    ExpansionVessel,
    Tank,
    DecouplingBottle,
    Boiler,
    HeatExchanger,
    Manifold,
    RefnetJoint,
    RefnetHeader,
    BranchBox,
}

/// The family a part is listed under in the library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PartCategory {
    #[default]
    Valves,
    Fittings,
    AirAndDirt,
    Instruments,
    Pumps,
    VesselsAndTanks,
    HeatSources,
    RefrigerantBranches,
    BranchBoxes,
}

impl PartCategory {
    pub const ALL: [PartCategory; 9] = [
        PartCategory::Valves,
        PartCategory::Fittings,
        PartCategory::AirAndDirt,
        PartCategory::Instruments,
        PartCategory::Pumps,
        PartCategory::VesselsAndTanks,
        PartCategory::HeatSources,
        PartCategory::RefrigerantBranches,
        PartCategory::BranchBoxes,
    ];

    /// The English title, also the key of its translation.
    pub fn title(self) -> &'static str {
        match self {
            PartCategory::Valves => "Valves",
            PartCategory::Fittings => "Fittings",
            PartCategory::AirAndDirt => "Air and dirt",
            PartCategory::Instruments => "Instruments",
            PartCategory::Pumps => "Pumps",
            PartCategory::VesselsAndTanks => "Vessels and tanks",
            PartCategory::HeatSources => "Heat sources",
            PartCategory::RefrigerantBranches => "Refrigerant branches (Refnet)",
            PartCategory::BranchBoxes => "Branch selector boxes",
        }
    }
}

impl PartKind {
    /// The name a command takes.
    pub fn name(self) -> &'static str {
        match self {
            PartKind::IsolationValve => "isolationValve",
            PartKind::ButterflyValve => "butterflyValve",
            PartKind::CheckValve => "checkValve",
            PartKind::BalancingValve => "balancingValve",
            PartKind::ControlValve => "controlValve",
            PartKind::ThreeWayValve => "threeWayValve",
            PartKind::PicValve => "picValve",
            PartKind::SafetyValve => "safetyValve",
            PartKind::PressureReducingValve => "pressureReducingValve",
            PartKind::DrainValve => "drainValve",
            PartKind::Strainer => "strainer",
            PartKind::FlexConnector => "flexConnector",
            PartKind::Reducer => "reducer",
            PartKind::AirVent => "airVent",
            PartKind::Deaerator => "deaerator",
            PartKind::DirtSeparator => "dirtSeparator",
            PartKind::Disconnector => "disconnector",
            PartKind::FillingGroup => "fillingGroup",
            PartKind::Thermometer => "thermometer",
            PartKind::PressureGauge => "pressureGauge",
            PartKind::FlowMeter => "flowMeter",
            PartKind::EnergyMeter => "energyMeter",
            PartKind::Sensor => "sensor",
            PartKind::Pump => "pump",
            PartKind::DoublePump => "doublePump",
            PartKind::ExpansionVessel => "expansionVessel",
            PartKind::Tank => "tank",
            PartKind::DecouplingBottle => "decouplingBottle",
            PartKind::Boiler => "boiler",
            PartKind::HeatExchanger => "heatExchanger",
            PartKind::Manifold => "manifold",
            PartKind::RefnetJoint => "refnetJoint",
            PartKind::RefnetHeader => "refnetHeader",
            PartKind::BranchBox => "branchBox",
        }
    }

    pub fn parse(text: &str) -> Option<PartKind> {
        let t: String = text.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        ALL_KINDS.into_iter().find(|k| k.name().to_ascii_lowercase() == t)
    }

    /// Whether the part is a safety device: it is drawn in red.
    pub fn is_safety(self) -> bool {
        matches!(self, PartKind::SafetyValve | PartKind::Disconnector)
    }

    /// The layer of the kind.
    pub fn layer(self) -> &'static str {
        if self.is_safety() {
            return PART_SAFETY_LAYER;
        }
        match self {
            PartKind::Pump
            | PartKind::DoublePump
            | PartKind::ExpansionVessel
            | PartKind::Tank
            | PartKind::DecouplingBottle
            | PartKind::Boiler
            | PartKind::HeatExchanger
            | PartKind::Manifold => PART_EQUIPMENT_LAYER,
            PartKind::RefnetJoint | PartKind::RefnetHeader | PartKind::BranchBox => PART_REFRIGERANT_LAYER,
            _ => PART_LAYER,
        }
    }

    /// The family the kind is listed under.
    pub fn category(self) -> PartCategory {
        match self {
            PartKind::IsolationValve
            | PartKind::ButterflyValve
            | PartKind::CheckValve
            | PartKind::BalancingValve
            | PartKind::ControlValve
            | PartKind::ThreeWayValve
            | PartKind::PicValve
            | PartKind::SafetyValve
            | PartKind::PressureReducingValve
            | PartKind::DrainValve => PartCategory::Valves,
            PartKind::Strainer
            | PartKind::FlexConnector
            | PartKind::Reducer
            | PartKind::Disconnector
            | PartKind::FillingGroup
            | PartKind::Manifold => PartCategory::Fittings,
            PartKind::AirVent | PartKind::Deaerator | PartKind::DirtSeparator => PartCategory::AirAndDirt,
            PartKind::Thermometer | PartKind::PressureGauge | PartKind::FlowMeter | PartKind::EnergyMeter | PartKind::Sensor => {
                PartCategory::Instruments
            }
            PartKind::Pump | PartKind::DoublePump => PartCategory::Pumps,
            PartKind::ExpansionVessel | PartKind::Tank | PartKind::DecouplingBottle => PartCategory::VesselsAndTanks,
            PartKind::Boiler | PartKind::HeatExchanger => PartCategory::HeatSources,
            PartKind::RefnetJoint | PartKind::RefnetHeader => PartCategory::RefrigerantBranches,
            PartKind::BranchBox => PartCategory::BranchBoxes,
        }
    }

    /// Whether the part belongs to a refrigerant network.
    pub fn is_refrigerant(self) -> bool {
        self.layer() == PART_REFRIGERANT_LAYER
    }

    /// Whether the part sits in the line of a pipe (the pipe is cut where it is put and the network reads through it).
    pub fn is_inline(self) -> bool {
        matches!(
            self,
            PartKind::IsolationValve
                | PartKind::ButterflyValve
                | PartKind::CheckValve
                | PartKind::BalancingValve
                | PartKind::ControlValve
                | PartKind::ThreeWayValve
                | PartKind::PicValve
                | PartKind::PressureReducingValve
                | PartKind::Strainer
                | PartKind::FlexConnector
                | PartKind::Reducer
                | PartKind::Deaerator
                | PartKind::DirtSeparator
                | PartKind::Disconnector
                | PartKind::FlowMeter
                | PartKind::EnergyMeter
                | PartKind::Pump
                | PartKind::DoublePump
        )
    }

    /// The height a new part of the kind starts with, mm (indicative: the data sheet's is typed over it).
    pub fn default_height_mm(self) -> f64 {
        match self {
            PartKind::IsolationValve | PartKind::ButterflyValve | PartKind::CheckValve | PartKind::BalancingValve | PartKind::DrainValve => 150.0,
            PartKind::ControlValve | PartKind::ThreeWayValve | PartKind::PicValve | PartKind::PressureReducingValve | PartKind::SafetyValve => 250.0,
            PartKind::Strainer | PartKind::FlexConnector | PartKind::Reducer | PartKind::AirVent => 150.0,
            PartKind::Deaerator | PartKind::DirtSeparator => 500.0,
            PartKind::Disconnector | PartKind::FillingGroup => 250.0,
            PartKind::Thermometer | PartKind::PressureGauge | PartKind::FlowMeter | PartKind::EnergyMeter | PartKind::Sensor => 120.0,
            PartKind::Pump | PartKind::DoublePump => 250.0,
            PartKind::ExpansionVessel => 450.0,
            PartKind::Tank => 1500.0,
            PartKind::DecouplingBottle => 1200.0,
            PartKind::Boiler => 900.0,
            PartKind::HeatExchanger => 800.0,
            PartKind::Manifold | PartKind::RefnetJoint | PartKind::RefnetHeader => 100.0,
            PartKind::BranchBox => 300.0,
        }
    }

    /// For a header or a box: the length of the symbol for `outlets` outlets, mm (a base and a pitch).
    pub fn length_for_outlets_mm(self, outlets: u32) -> Option<f64> {
        let (base, pitch) = match self {
            PartKind::BranchBox => (200.0, 90.0),
            PartKind::RefnetHeader => (100.0, 75.0),
            PartKind::Manifold => (100.0, 125.0),
            _ => return None,
        };
        Some(base + pitch * f64::from(outlets.max(1)))
    }

    /// The length and the width a new part of the kind starts with in plan, mm.
    pub fn default_size_mm(self) -> (f64, f64) {
        match self {
            PartKind::IsolationValve | PartKind::ButterflyValve | PartKind::CheckValve | PartKind::BalancingValve | PartKind::DrainValve => {
                (150.0, 100.0)
            }
            PartKind::ControlValve | PartKind::ThreeWayValve | PartKind::PicValve | PartKind::PressureReducingValve | PartKind::SafetyValve => {
                (200.0, 120.0)
            }
            PartKind::Strainer | PartKind::FlexConnector | PartKind::Reducer => (200.0, 100.0),
            PartKind::AirVent => (80.0, 160.0),
            PartKind::Deaerator | PartKind::DirtSeparator => (250.0, 400.0),
            PartKind::Disconnector | PartKind::FillingGroup => (300.0, 200.0),
            PartKind::Thermometer | PartKind::PressureGauge | PartKind::FlowMeter | PartKind::Sensor => (120.0, 120.0),
            PartKind::EnergyMeter => (200.0, 150.0),
            PartKind::Pump => (250.0, 250.0),
            PartKind::DoublePump => (500.0, 250.0),
            PartKind::ExpansionVessel => (400.0, 400.0),
            PartKind::Tank => (800.0, 800.0),
            PartKind::DecouplingBottle => (400.0, 900.0),
            PartKind::Boiler => (700.0, 500.0),
            PartKind::HeatExchanger => (400.0, 600.0),
            PartKind::Manifold => (800.0, 120.0),
            PartKind::RefnetJoint => (300.0, 150.0),
            PartKind::RefnetHeader => (600.0, 100.0),
            PartKind::BranchBox => (700.0, 300.0),
        }
    }
}

/// Every kind, for lookups by name.
pub const ALL_KINDS: [PartKind; 34] = [
    PartKind::IsolationValve,
    PartKind::ButterflyValve,
    PartKind::CheckValve,
    PartKind::BalancingValve,
    PartKind::ControlValve,
    PartKind::ThreeWayValve,
    PartKind::PicValve,
    PartKind::SafetyValve,
    PartKind::PressureReducingValve,
    PartKind::DrainValve,
    PartKind::Strainer,
    PartKind::FlexConnector,
    PartKind::Reducer,
    PartKind::AirVent,
    PartKind::Deaerator,
    PartKind::DirtSeparator,
    PartKind::Disconnector,
    PartKind::FillingGroup,
    PartKind::Thermometer,
    PartKind::PressureGauge,
    PartKind::FlowMeter,
    PartKind::EnergyMeter,
    PartKind::Sensor,
    PartKind::Pump,
    PartKind::DoublePump,
    PartKind::ExpansionVessel,
    PartKind::Tank,
    PartKind::DecouplingBottle,
    PartKind::Boiler,
    PartKind::HeatExchanger,
    PartKind::Manifold,
    PartKind::RefnetJoint,
    PartKind::RefnetHeader,
    PartKind::BranchBox,
];

/// The pressure loss of a kind of part when none is given: a loss coefficient ζ, or a loss in kPa at the design flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PartLoss {
    Zeta(f64),
    Kpa(f64),
}

impl PartKind {
    /// The loss a part of the kind is given until the data sheet's is typed (indicative values, fully open for a valve); none for
    /// the parts that add no loss to a run (a sensor, a vessel on a branch).
    pub fn default_loss(self) -> Option<PartLoss> {
        Some(match self {
            PartKind::IsolationValve => PartLoss::Zeta(0.3),
            PartKind::ButterflyValve => PartLoss::Zeta(0.8),
            PartKind::CheckValve => PartLoss::Zeta(2.5),
            PartKind::BalancingValve => PartLoss::Zeta(4.0),
            PartKind::ControlValve => PartLoss::Zeta(6.0),
            PartKind::ThreeWayValve => PartLoss::Zeta(5.0),
            PartKind::PicValve => PartLoss::Kpa(25.0),
            PartKind::PressureReducingValve => PartLoss::Kpa(30.0),
            PartKind::Strainer => PartLoss::Zeta(3.0),
            PartKind::FlexConnector => PartLoss::Zeta(0.3),
            PartKind::Reducer => PartLoss::Zeta(0.5),
            PartKind::Deaerator | PartKind::DirtSeparator => PartLoss::Zeta(1.5),
            PartKind::Disconnector => PartLoss::Kpa(60.0),
            PartKind::FlowMeter => PartLoss::Kpa(5.0),
            PartKind::EnergyMeter => PartLoss::Kpa(10.0),
            _ => return None,
        })
    }
}

/// A part: what its block reference keeps to regenerate it. It is centred on the block's origin, its length along the
/// block's x axis (the pipe's direction).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartParams {
    /// The tag written beside the symbol ("V1", "P2").
    pub name: String,
    #[serde(default)]
    pub kind: PartKind,
    /// The library model it was made from, or empty.
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub brand: String,
    /// The size of the part, mm: its width along the block's x axis (the pipe's direction, L), its depth across (P) and its
    /// height (H, written on the data sheets before the other two).
    pub length_mm: f64,
    pub width_mm: f64,
    #[serde(default)]
    pub height_mm: Option<f64>,
    /// The number of outlets of a header or a box (the ports of a branch-selector box).
    #[serde(default)]
    pub outlets: u32,
    /// The connections, as free text ("Liq 5/8\" · HP/LP 7/8\" · Suc 7/8\"").
    #[serde(default)]
    pub connections: String,
    /// The nominal diameter of the pipe the part sits on, mm.
    #[serde(default)]
    pub dn_mm: Option<f64>,
    /// The volume of a vessel or a tank, l.
    #[serde(default)]
    pub volume_l: Option<f64>,
    /// The power of a boiler, an exchanger or a pump, kW.
    #[serde(default)]
    pub power_kw: Option<f64>,
    /// The flow, m³/h, and the head, kPa, of a pump; the kvs of a valve.
    #[serde(default)]
    pub flow_m3h: Option<f64>,
    #[serde(default)]
    pub head_kpa: Option<f64>,
    #[serde(default)]
    pub weight_kg: Option<f64>,
    /// The pressure loss of the part, from the data sheet: a loss in kPa at the design flow, or a kvs (m³/h for 1 bar), or an
    /// equivalent length of pipe (m), or a loss coefficient ζ; the first given counts, in that order, and a kind has an
    /// indicative one of its own ([`PartKind::default_loss`]) when none is.
    #[serde(default)]
    pub loss_kpa: Option<f64>,
    #[serde(default)]
    pub kvs: Option<f64>,
    #[serde(default)]
    pub equivalent_length_m: Option<f64>,
    #[serde(default)]
    pub zeta: Option<f64>,
    /// The name of the network the part is on: it is drawn on the network's layer (but for a safety device, which stays red).
    #[serde(default)]
    pub network: String,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

fn text_ok(t: &str) -> bool {
    t.chars().count() <= MAX_TEXT_CHARS && !t.chars().any(char::is_control)
}

impl PartParams {
    /// A part of `kind` with the kind's size.
    pub fn new(kind: PartKind, name: &str, units: HvacUnits) -> PartParams {
        let (length_mm, width_mm) = kind.default_size_mm();
        PartParams {
            name: name.to_string(),
            kind,
            model: String::new(),
            brand: String::new(),
            length_mm,
            width_mm,
            height_mm: Some(kind.default_height_mm()),
            outlets: match kind {
                PartKind::RefnetHeader => 8,
                PartKind::Manifold => 6,
                PartKind::BranchBox => 4,
                _ => 0,
            },
            connections: String::new(),
            dn_mm: None,
            volume_l: None,
            power_kw: None,
            flow_m3h: None,
            head_kpa: None,
            weight_kg: None,
            loss_kpa: None,
            kvs: None,
            equivalent_length_m: None,
            zeta: None,
            network: String::new(),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("part", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        let side = |v: f64| v.is_finite() && (20.0..=MAX_PART_MM).contains(&v);
        if !(side(self.length_mm) && side(self.width_mm)) {
            return Err(invalid(format!("A part's sizes are 20 to {MAX_PART_MM} mm.")));
        }
        if self.outlets > MAX_PART_OUTLETS {
            return Err(invalid(format!("A part has at most {MAX_PART_OUTLETS} outlets.")));
        }
        if !(text_ok(&self.model) && text_ok(&self.brand) && text_ok(&self.connections) && text_ok(&self.network)) {
            return Err(invalid(format!("A part's texts have at most {MAX_TEXT_CHARS} characters, without control characters.")));
        }
        let positive = |v: Option<f64>| v.is_none_or(|v| v.is_finite() && v > 0.0 && v <= 1.0e7);
        if !(positive(self.height_mm)
            && positive(self.dn_mm)
            && positive(self.volume_l)
            && positive(self.power_kw)
            && positive(self.flow_m3h)
            && positive(self.head_kpa)
            && positive(self.weight_kg)
            && positive(self.loss_kpa)
            && positive(self.kvs)
            && positive(self.equivalent_length_m)
            && positive(self.zeta))
        {
            return Err(invalid("A part's size, volume, power, flow, head, weight and losses are more than 0."));
        }
        Ok(())
    }

    /// The pressure loss of the part, kPa, for water of `flow_m3h` running at `velocity_ms` in a pipe losing `gradient_pa_m`.
    pub fn pressure_loss_kpa(&self, flow_m3h: f64, velocity_ms: f64, gradient_pa_m: f64) -> f64 {
        let zeta = |z: f64| 0.5 * z * velocity_ms * velocity_ms;
        if let Some(k) = self.loss_kpa {
            return k;
        }
        if let Some(kv) = self.kvs {
            // Δp = (Q / kvs)² bar.
            return 100.0 * (flow_m3h / kv).powi(2);
        }
        if let Some(l) = self.equivalent_length_m {
            return gradient_pa_m * l / 1000.0;
        }
        match (self.zeta, self.kind.default_loss()) {
            (Some(z), _) | (None, Some(PartLoss::Zeta(z))) => zeta(z),
            (None, Some(PartLoss::Kpa(k))) => k,
            (None, None) => 0.0,
        }
    }

    /// The text written beside the symbol: the tag, then what is known of its size.
    pub fn label_text(&self) -> String {
        let mut text = self.name.trim().to_string();
        let mut add = |s: String| {
            text.push(' ');
            text.push_str(&s);
        };
        if let Some(d) = self.dn_mm {
            add(format!("DN{d:.0}"));
        }
        if let Some(v) = self.volume_l {
            add(format!("{v:.0} l"));
        }
        if let Some(p) = self.power_kw {
            add(format!("{p:.0} kW"));
        }
        if matches!(self.kind, PartKind::BranchBox | PartKind::RefnetHeader | PartKind::Manifold) {
            add(format!("{}x", self.outlets));
        }
        text
    }

    /// The symbol and the label.
    /// The layer the part is drawn on: its kind's, or its network's copy of it (a safety device keeps its red one).
    pub fn layer(&self) -> &'static str {
        if self.kind.is_safety() { self.kind.layer() } else { super::network_layer(self.kind.layer(), &self.network) }
    }

    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let layer = self.layer();
        let u = self.units;
        let (l, w) = (u.to_units(self.length_mm), u.to_units(self.width_mm));
        let mut pieces: Vec<HvacPiece> = symbol(self.kind, l, w, self.outlets).into_iter().map(|kind| HvacPiece { layer, kind }).collect();
        let h = u.to_units(self.label_height_mm);
        let text = self.label_text();
        if !text.is_empty() {
            pieces.push(HvacPiece { layer, kind: label(Vec2::new(0.0, -(lowest(self.kind, l, w) + 1.1 * h)), h, text) });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(PART_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<PartParams> {
        let p: PartParams = xdata::from_xdata(PART_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// A connection point of a refrigerant part, in the part's own coordinates (drawing units): where the end of a pipe is put.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartPort {
    pub at: Vec2,
    /// The number of the outlet; none for the inlet side (the one towards the outdoor unit).
    pub outlet: Option<usize>,
}

impl PartParams {
    /// The connection points of a Y joint, a header or a branch box (the ends of the lines its symbol draws); none for the others.
    pub fn ports(&self) -> Vec<PartPort> {
        let (l, w) = (self.units.to_units(self.length_mm), self.units.to_units(self.width_mm));
        let n = self.outlets.max(1);
        let along = |i: u32| -l / 2.0 + l * (f64::from(i) + 0.5) / f64::from(n);
        let inlet = |x: f64, y: f64| PartPort { at: Vec2::new(x, y), outlet: None };
        match self.kind {
            PartKind::RefnetJoint => vec![
                inlet(-l / 2.0, 0.0),
                PartPort { at: Vec2::new(l / 2.0, w / 2.0), outlet: Some(0) },
                PartPort { at: Vec2::new(l / 2.0, -w / 2.0), outlet: Some(1) },
            ],
            PartKind::RefnetHeader => {
                let mut v = vec![inlet(-l / 2.0 - l * 0.15, 0.0)];
                v.extend((0..n).map(|i| PartPort { at: Vec2::new(along(i), w / 2.0 + w * 1.5), outlet: Some(i as usize) }));
                v
            }
            PartKind::BranchBox => {
                let mut v: Vec<PartPort> = [-0.3, 0.0, 0.3].into_iter().map(|dy| inlet(-l / 2.0 - l * 0.1, w * dy)).collect();
                v.extend((0..n).map(|i| PartPort { at: Vec2::new(along(i), w / 2.0 + w * 0.8), outlet: Some(i as usize) }));
                v
            }
            _ => Vec::new(),
        }
    }
}

/// The part an object is, when it is a block reference carrying usable part data.
pub fn part_of(e: &crate::Entity) -> Option<PartParams> {
    match &e.kind {
        EntityKind::Insert(_) => PartParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour of a part layer.
pub fn part_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        PART_LAYER => Some(Color::True(Rgb(0xe6, 0xb4, 0x3c))),
        PART_EQUIPMENT_LAYER => Some(Color::True(Rgb(0x3f, 0xb8, 0xa8))),
        PART_REFRIGERANT_LAYER => Some(Color::True(Rgb(0xe6, 0x7e, 0x22))),
        PART_SAFETY_LAYER => Some(Color::True(Rgb(0xe0, 0x24, 0x24))),
        _ => None,
    }
}

impl crate::Drawing {
    /// Adds part `p` centred at `at`, turned `rotation` radians.
    pub fn add_part(&mut self, space: &crate::Space, mut common: crate::Common, p: &PartParams, at: Vec2, rotation: f64) -> Result<crate::Handle> {
        if !at.is_finite() || !rotation.is_finite() {
            return Err(invalid("A part needs a usable position."));
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(PART_APP, &common.xdata, p.to_xdata()?);
        let layer = p.layer();
        common.layer = layer.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(layer);
        let insert = crate::Insert { block, insert: v3(at), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives part `h` the parameters `p`, redrawing it in place.
    pub fn regenerate_part(&mut self, h: crate::Handle, p: &PartParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(PART_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        let layer = p.layer();
        self.ensure_layer(layer);
        self.modify_entity(h, |e| e.common.layer = layer.into())?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Symbols, in the part's own coordinates: centred on the origin, `l` long along x, `w` wide along y.
// ---------------------------------------------------------------------------------------------------------------

fn line(a: (f64, f64), b: (f64, f64)) -> EntityKind {
    EntityKind::Line(Line { a: v3(Vec2::new(a.0, a.1)), b: v3(Vec2::new(b.0, b.1)) })
}

fn poly(points: &[(f64, f64)], closed: bool) -> EntityKind {
    let vertices = points.iter().map(|&(x, y)| PolyVertex { p: Vec2::new(x, y), bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
    EntityKind::LwPolyline(LwPolyline { vertices, closed, const_width: 0.0, elevation: 0.0, plinegen: false })
}

fn circle(c: (f64, f64), r: f64) -> EntityKind {
    EntityKind::Circle(Circle { center: v3(Vec2::new(c.0, c.1)), radius: r })
}

fn half_circle(c: (f64, f64), r: f64) -> EntityKind {
    EntityKind::Arc(Arc { center: v3(Vec2::new(c.0, c.1)), radius: r, start: 0.0, end: std::f64::consts::PI })
}

fn rect(cx: f64, cy: f64, hl: f64, hw: f64) -> EntityKind {
    poly(&[(cx - hl, cy - hw), (cx + hl, cy - hw), (cx + hl, cy + hw), (cx - hl, cy + hw)], true)
}

/// The two triangles that meet at the middle of a valve body.
fn bowtie(l: f64, w: f64) -> EntityKind {
    poly(&[(-l / 2.0, -w / 2.0), (l / 2.0, w / 2.0), (l / 2.0, -w / 2.0), (-l / 2.0, w / 2.0)], true)
}

fn text_in(c: (f64, f64), h: f64, text: &str) -> EntityKind {
    label(Vec2::new(c.0, c.1), h, text.to_string())
}

/// The actuator over a valve: a stem and a half circle.
fn actuator(w: f64) -> Vec<EntityKind> {
    vec![line((0.0, 0.0), (0.0, w * 0.9)), half_circle((0.0, w * 0.9), w * 0.35)]
}

/// How far below the axis the symbol of `kind` reaches (the label goes under it).
fn lowest(kind: PartKind, l: f64, w: f64) -> f64 {
    match kind {
        PartKind::ThreeWayValve => w * 1.2,
        PartKind::DrainValve | PartKind::Strainer | PartKind::Disconnector => w,
        PartKind::DirtSeparator => w * 0.7,
        PartKind::ExpansionVessel => l.min(w) * 0.7,
        _ => w / 2.0,
    }
}

/// The symbol of a part of `kind`.
fn symbol(kind: PartKind, l: f64, w: f64, outlets: u32) -> Vec<EntityKind> {
    match kind {
        PartKind::IsolationValve | PartKind::ButterflyValve | PartKind::CheckValve | PartKind::BalancingValve | PartKind::DrainValve => {
            valve_symbol(kind, l, w)
        }
        PartKind::ControlValve | PartKind::ThreeWayValve | PartKind::PicValve | PartKind::SafetyValve | PartKind::PressureReducingValve => {
            control_symbol(kind, l, w)
        }
        PartKind::Strainer | PartKind::FlexConnector | PartKind::Reducer | PartKind::Disconnector | PartKind::FillingGroup => {
            fitting_symbol(kind, l, w)
        }
        PartKind::AirVent | PartKind::Deaerator | PartKind::DirtSeparator => air_symbol(kind, l, w),
        PartKind::Thermometer | PartKind::PressureGauge | PartKind::FlowMeter | PartKind::EnergyMeter | PartKind::Sensor => {
            instrument_symbol(kind, l, w)
        }
        PartKind::Pump | PartKind::DoublePump => pump_symbol(kind, l, w),
        PartKind::ExpansionVessel | PartKind::Tank | PartKind::DecouplingBottle | PartKind::Boiler | PartKind::HeatExchanger => {
            vessel_symbol(kind, l, w)
        }
        PartKind::RefnetJoint | PartKind::RefnetHeader | PartKind::BranchBox | PartKind::Manifold => refrigerant_symbol(kind, l, w, outlets),
    }
}

fn valve_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    let mut v = vec![bowtie(l, w)];
    match kind {
        // A bar across the body.
        PartKind::ButterflyValve => v.push(line((0.0, -w * 0.75), (0.0, w * 0.75))),
        // A flap: the triangle that seats on a bar.
        PartKind::CheckValve => {
            v = vec![poly(&[(-l / 2.0, -w / 2.0), (l / 2.0, 0.0), (-l / 2.0, w / 2.0)], true), line((l / 2.0, -w / 2.0), (l / 2.0, w / 2.0))]
        }
        // The arrow across the body, ending in a bar: a balancing valve is set.
        PartKind::BalancingValve => v.extend([line((-l * 0.6, -w * 0.9), (l * 0.6, w * 0.9)), line((l * 0.45, w * 1.0), (l * 0.75, w * 0.7))]),
        // A stem down to a cap: the drain.
        _ => v.extend([line((0.0, 0.0), (0.0, -w)), line((-w * 0.2, -w), (w * 0.2, -w))]),
    }
    v
}

fn control_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    match kind {
        PartKind::ThreeWayValve => {
            let mut v = vec![
                poly(&[(-l / 2.0, w / 2.0), (0.0, 0.0), (-l / 2.0, -w / 2.0)], true),
                poly(&[(l / 2.0, w / 2.0), (0.0, 0.0), (l / 2.0, -w / 2.0)], true),
                poly(&[(-l / 4.0, -w * 1.2), (0.0, 0.0), (l / 4.0, -w * 1.2)], true),
            ];
            v.extend(actuator(w));
            v
        }
        PartKind::SafetyValve => {
            // A valve on the pipe and the spring over it.
            let spring: Vec<(f64, f64)> = (0..=6).map(|i| (if i % 2 == 0 { -w * 0.2 } else { w * 0.2 }, w * (0.5 + 0.25 * f64::from(i)))).collect();
            vec![
                poly(&[(-l / 2.0, -w / 2.0), (l / 2.0, w / 2.0), (l / 2.0, -w / 2.0), (-l / 2.0, w / 2.0)], true),
                line((0.0, 0.0), (0.0, w * 0.5)),
                poly(&spring, false),
            ]
        }
        PartKind::PressureReducingValve => {
            // A body, a stem and a diaphragm over it.
            let mut v = vec![bowtie(l, w), line((0.0, 0.0), (0.0, w * 0.7)), line((-w * 0.45, w * 0.7), (w * 0.45, w * 0.7))];
            v.push(half_circle((0.0, w * 0.7), w * 0.45));
            v
        }
        PartKind::PicValve => {
            // A control valve with a small circle on the body: the pressure regulation inside.
            let mut v = vec![bowtie(l, w)];
            v.extend(actuator(w));
            v.push(circle((0.0, 0.0), w * 0.22));
            v
        }
        _ => {
            let mut v = vec![bowtie(l, w)];
            v.extend(actuator(w));
            v
        }
    }
}

fn fitting_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    match kind {
        // A filter: a body across the pipe with the screen as a diagonal.
        PartKind::Strainer => vec![rect(0.0, 0.0, l / 2.0, w / 2.0), line((-l / 2.0, -w / 2.0), (l / 2.0, w / 2.0)), line((0.0, 0.0), (0.0, -w))],
        // A bellows: a zigzag between two flanges.
        PartKind::FlexConnector => {
            let n = 6;
            let zig: Vec<(f64, f64)> =
                (0..=n).map(|i| (-l / 2.0 + l * f64::from(i) / f64::from(n), if i % 2 == 0 { -w / 2.0 } else { w / 2.0 })).collect();
            vec![line((-l / 2.0, -w * 0.7), (-l / 2.0, w * 0.7)), poly(&zig, false), line((l / 2.0, -w * 0.7), (l / 2.0, w * 0.7))]
        }
        PartKind::Reducer => vec![poly(&[(-l / 2.0, -w / 2.0), (l / 2.0, -w / 4.0), (l / 2.0, w / 4.0), (-l / 2.0, w / 2.0)], true)],
        // A backflow disconnector: two triangles in a body.
        PartKind::Disconnector => vec![
            rect(0.0, 0.0, l / 2.0, w / 2.0),
            poly(&[(-l * 0.35, -w * 0.3), (-l * 0.05, 0.0), (-l * 0.35, w * 0.3)], true),
            poly(&[(l * 0.35, -w * 0.3), (l * 0.05, 0.0), (l * 0.35, w * 0.3)], true),
            line((0.0, -w / 2.0), (0.0, -w)),
        ],
        // A filling group: a body with a tap.
        _ => vec![
            rect(0.0, 0.0, l / 2.0, w / 2.0),
            line((-l * 0.25, 0.0), (l * 0.25, 0.0)),
            line((0.0, 0.0), (0.0, w * 0.5)),
            circle((0.0, w * 0.5), w * 0.1),
        ],
    }
}

fn air_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    match kind {
        // An air vent: a stem up to a small float.
        PartKind::AirVent => {
            vec![line((0.0, 0.0), (0.0, w * 0.55)), circle((0.0, w * 0.55 + w * 0.2), w * 0.2), line((-w * 0.2, w * 0.2), (w * 0.2, w * 0.2))]
        }
        // A deaerator: a vessel with a vent on top.
        PartKind::Deaerator => vec![
            rect(0.0, 0.0, l / 2.0, w / 2.0),
            line((-l / 2.0, w * 0.15), (l / 2.0, w * 0.15)),
            line((0.0, w / 2.0), (0.0, w * 0.75)),
            circle((0.0, w * 0.85), w * 0.1),
        ],
        // A dirt separator: a vessel with a drain below.
        _ => vec![
            rect(0.0, 0.0, l / 2.0, w / 2.0),
            line((-l / 2.0, -w * 0.25), (l / 2.0, -w * 0.25)),
            line((0.0, -w / 2.0), (0.0, -w * 0.7)),
            line((-l * 0.15, -w * 0.7), (l * 0.15, -w * 0.7)),
        ],
    }
}

fn instrument_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    let r = l.min(w) / 2.0;
    let h = r * 0.9;
    match kind {
        PartKind::Thermometer => vec![circle((0.0, 0.0), r), text_in((0.0, 0.0), h, "T")],
        PartKind::PressureGauge => vec![circle((0.0, 0.0), r), text_in((0.0, 0.0), h, "P")],
        PartKind::FlowMeter => vec![circle((0.0, 0.0), r), text_in((0.0, 0.0), h, "Q")],
        PartKind::Sensor => vec![circle((0.0, 0.0), r), line((-r, 0.0), (r, 0.0)), line((0.0, r), (0.0, r * 1.8))],
        // An energy meter: a box with the pipe going through it.
        _ => vec![rect(0.0, 0.0, l / 2.0, w / 2.0), text_in((0.0, 0.0), w * 0.45, "kWh")],
    }
}

fn pump_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    let one = |cx: f64, r: f64| vec![circle((cx, 0.0), r), poly(&[(cx - r * 0.45, -r * 0.7), (cx + r * 0.8, 0.0), (cx - r * 0.45, r * 0.7)], true)];
    if kind == PartKind::DoublePump {
        let r = (l / 4.0).min(w / 2.0);
        let mut v = one(-r, r);
        v.extend(one(r, r));
        v
    } else {
        one(0.0, l.min(w) / 2.0)
    }
}

fn vessel_symbol(kind: PartKind, l: f64, w: f64) -> Vec<EntityKind> {
    let r = l.min(w) / 2.0;
    match kind {
        // An expansion vessel: a round body with the diaphragm across and the connection below.
        PartKind::ExpansionVessel => vec![circle((0.0, 0.0), r), line((-r, 0.0), (r, 0.0)), line((0.0, -r), (0.0, -r * 1.4))],
        // A tank seen from above: a circle with its two diagonals.
        PartKind::Tank => vec![circle((0.0, 0.0), r), line((-r * 0.7, -r * 0.7), (r * 0.7, r * 0.7)), line((-r * 0.7, r * 0.7), (r * 0.7, -r * 0.7))],
        // A decoupling bottle: a tall body with two connections each side.
        PartKind::DecouplingBottle => vec![
            rect(0.0, 0.0, l / 2.0, w / 2.0),
            line((-l / 2.0, w * 0.3), (-l / 2.0 - l * 0.4, w * 0.3)),
            line((-l / 2.0, -w * 0.3), (-l / 2.0 - l * 0.4, -w * 0.3)),
            line((l / 2.0, w * 0.3), (l / 2.0 + l * 0.4, w * 0.3)),
            line((l / 2.0, -w * 0.3), (l / 2.0 + l * 0.4, -w * 0.3)),
        ],
        // A boiler: a casing with a flame.
        PartKind::Boiler => vec![
            rect(0.0, 0.0, l / 2.0, w / 2.0),
            poly(&[(-l * 0.12, -w * 0.3), (0.0, w * 0.3), (l * 0.12, -w * 0.3)], true),
            line((-l / 2.0, w * 0.3), (-l / 2.0 - l * 0.15, w * 0.3)),
            line((-l / 2.0, -w * 0.3), (-l / 2.0 - l * 0.15, -w * 0.3)),
        ],
        // A plate heat exchanger: a frame with its plates.
        _ => {
            let mut v = vec![rect(0.0, 0.0, l / 2.0, w / 2.0)];
            v.extend((-2..=2).map(|i| line((l * 0.15 * f64::from(i), -w * 0.4), (l * 0.15 * f64::from(i), w * 0.4))));
            v
        }
    }
}

fn refrigerant_symbol(kind: PartKind, l: f64, w: f64, outlets: u32) -> Vec<EntityKind> {
    match kind {
        // A Y: the stem and its two branches.
        PartKind::RefnetJoint => vec![line((-l / 2.0, 0.0), (0.0, 0.0)), line((0.0, 0.0), (l / 2.0, w / 2.0)), line((0.0, 0.0), (l / 2.0, -w / 2.0))],
        // A header: a tube with its outlets on one side and its inlet at one end.
        PartKind::RefnetHeader | PartKind::Manifold => {
            let n = outlets.max(1);
            let mut v = vec![rect(0.0, 0.0, l / 2.0, w / 2.0), line((-l / 2.0, 0.0), (-l / 2.0 - l * 0.15, 0.0))];
            for i in 0..n {
                let x = -l / 2.0 + l * (f64::from(i) + 0.5) / f64::from(n);
                v.push(line((x, w / 2.0), (x, w / 2.0 + w * 1.5)));
            }
            v
        }
        // A branch box: a casing, one line for each outlet on one side (the links are drawn as one line, so a pipe snaps to the
        // middle of the outlet, as it does on a header) and the three header pipes on the other.
        _ => {
            let n = outlets.max(1);
            let mut v = vec![rect(0.0, 0.0, l / 2.0, w / 2.0)];
            for i in 0..n {
                let x = -l / 2.0 + l * (f64::from(i) + 0.5) / f64::from(n);
                v.push(line((x, w / 2.0), (x, w / 2.0 + w * 0.8)));
            }
            for dy in [-0.3, 0.0, 0.3] {
                v.push(line((-l / 2.0, w * dy), (-l / 2.0 - l * 0.1, w * dy)));
            }
            v
        }
    }
}
