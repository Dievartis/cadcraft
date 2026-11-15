//! The building controls: the sensors (temperature, humidity, CO₂, pressure, flow, occupancy), the room controls (thermostats, remotes), the actuators (valve and damper
//! actuators, drives, meters) and the automation (controllers, input/output modules, gateways, control cabinets, supervision). Like the other plan objects each is a block
//! reference to an anonymous block holding a symbol drawn to scale (every symbol is drawn here, in code), its data travelling as extended data of its own registered
//! application. A device is wired to a controller by its name; what it asks of the controller (its points) is read from its kind, and the list of points is the
//! deliverable of the controls package ([`super::control_report`]).

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::part::{circle, line, poly, rect, text_in};
use super::{HvacPiece, HvacUnits, check_circuit_name, check_label_height, invalid, label, v3, xdata};
use crate::{EntityKind, RawTag, Result};

/// The registered application name of a control device's extended data.
pub const CONTROL_APP: &str = "CADCRAFT_CONTROL";
const XDATA_FORMAT: &str = "CONTROL1";
/// The layer of the control devices, and of the automation.
pub const CONTROL_LAYER: &str = "R-CONTROL";
pub const CONTROL_AUTOMATION_LAYER: &str = "R-CONTROL-AUTO";

fn default_label_mm() -> f64 {
    80.0
}

/// What a device asks of a controller, or a controller offers: analogue inputs, analogue outputs, digital inputs and digital outputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Points {
    #[serde(default)]
    pub ai: u32,
    #[serde(default)]
    pub ao: u32,
    #[serde(default)]
    pub di: u32,
    #[serde(default, rename = "do")]
    pub dout: u32,
}

impl Points {
    pub fn total(self) -> u32 {
        self.ai + self.ao + self.di + self.dout
    }

    pub fn plus(self, o: Points) -> Points {
        Points { ai: self.ai + o.ai, ao: self.ao + o.ao, di: self.di + o.di, dout: self.dout + o.dout }
    }

    /// The four counts as (name, count) in the order of the lists.
    pub fn counts(self) -> [(&'static str, u32); 4] {
        [("AI", self.ai), ("AO", self.ao), ("DI", self.di), ("DO", self.dout)]
    }
}

/// The families of the tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlGroup {
    Sensor,
    RoomControl,
    Actuator,
    Automation,
}

/// What a control device is: the symbol it is drawn with and the points it asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlKind {
    #[default]
    RoomTemperature,
    DuctTemperature,
    WaterTemperature,
    OutdoorTemperature,
    Humidity,
    Co2,
    Voc,
    DifferentialPressure,
    Flow,
    Occupancy,
    Thermostat,
    VrfRemote,
    ValveActuator,
    DamperActuator,
    FrequencyDrive,
    EnergyMeter,
    Controller,
    IoModule,
    Gateway,
    ControlCabinet,
    Supervisor,
}

impl ControlKind {
    pub const ALL: [ControlKind; 21] = [
        ControlKind::RoomTemperature,
        ControlKind::DuctTemperature,
        ControlKind::WaterTemperature,
        ControlKind::OutdoorTemperature,
        ControlKind::Humidity,
        ControlKind::Co2,
        ControlKind::Voc,
        ControlKind::DifferentialPressure,
        ControlKind::Flow,
        ControlKind::Occupancy,
        ControlKind::Thermostat,
        ControlKind::VrfRemote,
        ControlKind::ValveActuator,
        ControlKind::DamperActuator,
        ControlKind::FrequencyDrive,
        ControlKind::EnergyMeter,
        ControlKind::Controller,
        ControlKind::IoModule,
        ControlKind::Gateway,
        ControlKind::ControlCabinet,
        ControlKind::Supervisor,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ControlKind::RoomTemperature => "Room temperature sensor",
            ControlKind::DuctTemperature => "Duct temperature sensor",
            ControlKind::WaterTemperature => "Water temperature sensor",
            ControlKind::OutdoorTemperature => "Outdoor temperature sensor",
            ControlKind::Humidity => "Humidity sensor",
            ControlKind::Co2 => "CO₂ sensor",
            ControlKind::Voc => "Air quality (VOC) sensor",
            ControlKind::DifferentialPressure => "Differential pressure sensor",
            ControlKind::Flow => "Flow sensor",
            ControlKind::Occupancy => "Occupancy sensor",
            ControlKind::Thermostat => "Room thermostat",
            ControlKind::VrfRemote => "VRV remote controller",
            ControlKind::ValveActuator => "Valve actuator",
            ControlKind::DamperActuator => "Damper actuator",
            ControlKind::FrequencyDrive => "Frequency drive",
            ControlKind::EnergyMeter => "Energy meter",
            ControlKind::Controller => "Controller",
            ControlKind::IoModule => "Input/output module",
            ControlKind::Gateway => "Gateway",
            ControlKind::ControlCabinet => "Control cabinet",
            ControlKind::Supervisor => "Supervision station",
        }
    }

    /// The name a command takes: the camel-case one of the data.
    pub fn parse(text: &str) -> Option<ControlKind> {
        let key = |t: &str| t.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
        let want = key(text);
        ControlKind::ALL.into_iter().find(|k| key(k.name()) == want || key(&format!("{k:?}")) == want)
    }

    pub fn group(self) -> ControlGroup {
        match self {
            ControlKind::Thermostat | ControlKind::VrfRemote => ControlGroup::RoomControl,
            ControlKind::ValveActuator | ControlKind::DamperActuator | ControlKind::FrequencyDrive | ControlKind::EnergyMeter => {
                ControlGroup::Actuator
            }
            ControlKind::Controller | ControlKind::IoModule | ControlKind::Gateway | ControlKind::ControlCabinet | ControlKind::Supervisor => {
                ControlGroup::Automation
            }
            _ => ControlGroup::Sensor,
        }
    }

    pub fn is_automation(self) -> bool {
        self.group() == ControlGroup::Automation
    }

    /// The prefix of the tag a new device gets.
    pub fn tag_prefix(self) -> &'static str {
        match self {
            ControlKind::RoomTemperature => "TA",
            ControlKind::DuctTemperature => "TG",
            ControlKind::WaterTemperature => "TE",
            ControlKind::OutdoorTemperature => "TX",
            ControlKind::Humidity => "HR",
            ControlKind::Co2 => "CO",
            ControlKind::Voc => "QV",
            ControlKind::DifferentialPressure => "PD",
            ControlKind::Flow => "FQ",
            ControlKind::Occupancy => "PR",
            ControlKind::Thermostat => "TH",
            ControlKind::VrfRemote => "TC",
            ControlKind::ValveActuator => "VM",
            ControlKind::DamperActuator => "CM",
            ControlKind::FrequencyDrive => "VF",
            ControlKind::EnergyMeter => "CE",
            ControlKind::Controller => "AU",
            ControlKind::IoModule => "MX",
            ControlKind::Gateway => "PS",
            ControlKind::ControlCabinet => "CR",
            ControlKind::Supervisor => "SV",
        }
    }

    /// The points the device asks of its controller, one by one: the type ("AI", "AO", "DI", "DO") and what it is.
    pub fn point_list(self) -> Vec<(&'static str, &'static str)> {
        match self {
            ControlKind::RoomTemperature
            | ControlKind::DuctTemperature
            | ControlKind::WaterTemperature
            | ControlKind::OutdoorTemperature
            | ControlKind::Humidity
            | ControlKind::Co2
            | ControlKind::Voc
            | ControlKind::DifferentialPressure
            | ControlKind::Flow => vec![("AI", "Measure")],
            ControlKind::Occupancy => vec![("DI", "Occupancy")],
            ControlKind::EnergyMeter => vec![("DI", "Energy pulses")],
            ControlKind::Thermostat => vec![("AI", "Room temperature"), ("AI", "Setpoint")],
            ControlKind::ValveActuator | ControlKind::DamperActuator => vec![("AO", "Position command")],
            ControlKind::FrequencyDrive => vec![("AO", "Speed command"), ("DI", "Fault"), ("DO", "Run command")],
            _ => Vec::new(),
        }
    }

    /// The points the device asks of its controller, counted.
    pub fn points(self) -> Points {
        let mut p = Points::default();
        for (t, _) in self.point_list() {
            match t {
                "AI" => p.ai += 1,
                "AO" => p.ao += 1,
                "DI" => p.di += 1,
                _ => p.dout += 1,
            }
        }
        p
    }

    /// What a new controller or module offers: a typical figure to type over.
    pub fn default_capacity(self) -> Points {
        match self {
            ControlKind::Controller => Points { ai: 8, ao: 4, di: 8, dout: 8 },
            ControlKind::IoModule => Points { ai: 4, ao: 2, di: 8, dout: 4 },
            _ => Points::default(),
        }
    }

    /// The unit of the measure of a sensor, written in the list of points.
    pub fn unit(self) -> &'static str {
        match self {
            ControlKind::RoomTemperature
            | ControlKind::DuctTemperature
            | ControlKind::WaterTemperature
            | ControlKind::OutdoorTemperature
            | ControlKind::Thermostat => "°C",
            ControlKind::Humidity => "%RH",
            ControlKind::Co2 | ControlKind::Voc => "ppm",
            ControlKind::DifferentialPressure => "Pa",
            ControlKind::Flow => "m³/h",
            ControlKind::EnergyMeter => "pulse",
            ControlKind::ValveActuator | ControlKind::DamperActuator | ControlKind::FrequencyDrive => "%",
            _ => "",
        }
    }

    /// The height a new device is fitted at, mm above the floor (room devices at the height of an occupant, the others where they are used).
    pub fn default_height_mm(self) -> f64 {
        match self {
            ControlKind::RoomTemperature
            | ControlKind::Humidity
            | ControlKind::Co2
            | ControlKind::Voc
            | ControlKind::Thermostat
            | ControlKind::VrfRemote => 1500.0,
            ControlKind::Occupancy => 2500.0,
            ControlKind::ControlCabinet | ControlKind::Supervisor | ControlKind::Controller | ControlKind::IoModule | ControlKind::Gateway => 1500.0,
            _ => 0.0,
        }
    }

    pub fn layer(self) -> &'static str {
        if self.is_automation() { CONTROL_AUTOMATION_LAYER } else { CONTROL_LAYER }
    }
}

/// A control device: what its block reference keeps. Its place is the block's origin, turned by the reference's rotation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlParams {
    pub kind: ControlKind,
    /// The tag written beside the symbol ("TA3"); for a controller, a module or a gateway, its name, which the devices it serves are wired to by.
    #[serde(default)]
    pub name: String,
    /// The controller (or module) the device is wired to: its name.
    #[serde(default)]
    pub controller: String,
    /// What the device serves or measures: the name of an equipment, a unit or a part ("CTA1", "UE2").
    #[serde(default)]
    pub served: String,
    /// The circuit that feeds the device (a controller, a gateway, a cabinet or an actuator), in the electrical sense.
    #[serde(default)]
    pub circuit: String,
    /// The bus the device is on (a name: "BUS1"), for gateways and controllers.
    #[serde(default)]
    pub bus: String,
    /// The setpoint or the working value, in the unit of the kind, when there is one.
    #[serde(default)]
    pub setpoint: Option<f64>,
    /// What a controller or a module offers.
    #[serde(default)]
    pub capacity: Points,
    #[serde(default)]
    pub height_mm: f64,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl ControlParams {
    pub fn new(kind: ControlKind, name: &str, units: HvacUnits) -> ControlParams {
        ControlParams {
            kind,
            name: name.trim().to_string(),
            controller: String::new(),
            served: String::new(),
            circuit: String::new(),
            bus: String::new(),
            setpoint: None,
            capacity: kind.default_capacity(),
            height_mm: kind.default_height_mm(),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        for t in [&self.name, &self.controller, &self.served, &self.circuit, &self.bus] {
            check_circuit_name(t, true)?;
        }
        if self.setpoint.is_some_and(|v| !(v.is_finite() && v.abs() <= 1.0e7)) {
            return Err(invalid("The setpoint of a device is a number."));
        }
        if !(self.height_mm.is_finite() && (0.0..=50_000.0).contains(&self.height_mm)) {
            return Err(invalid("The height of a device is 0 to 50000 mm."));
        }
        if [self.capacity.ai, self.capacity.ao, self.capacity.di, self.capacity.dout].iter().any(|n| *n > 1000) {
            return Err(invalid("A controller offers at most 1000 points of each type."));
        }
        self.units.validate()?;
        check_label_height(self.label_height_mm)
    }

    pub fn layer(&self) -> &'static str {
        self.kind.layer()
    }

    /// The text beside the symbol: its tag, and the controller it is wired to.
    pub fn label_text(&self) -> String {
        let mut t = self.name.trim().to_string();
        if !self.controller.trim().is_empty() {
            t.push_str(&format!(" > {}", self.controller.trim()));
        }
        t
    }

    /// The symbol drawn to scale, centred on the origin, and its label under it.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let layer = self.layer();
        let mut shapes: Vec<EntityKind> = Vec::new();
        let mut half = 150.0;
        let letters = |s: &str| text_in((0.0, 0.0), 95.0, s);
        match self.kind {
            ControlKind::RoomTemperature | ControlKind::DuctTemperature | ControlKind::WaterTemperature | ControlKind::OutdoorTemperature => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("T"));
                if self.kind == ControlKind::DuctTemperature {
                    shapes.push(line((-150.0, 0.0), (-260.0, 0.0)));
                } else if self.kind == ControlKind::WaterTemperature {
                    shapes.push(line((0.0, -150.0), (0.0, -260.0)));
                }
            }
            ControlKind::Humidity => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("H"));
            }
            ControlKind::Co2 => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("CO₂"));
            }
            ControlKind::Voc => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("Q"));
            }
            ControlKind::DifferentialPressure => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("dP"));
            }
            ControlKind::Flow => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("F"));
            }
            ControlKind::Occupancy => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("P"));
                shapes.push(line((-150.0, -150.0), (150.0, 150.0)));
            }
            ControlKind::Thermostat | ControlKind::VrfRemote => {
                shapes.push(rect(0.0, 0.0, 150.0, 150.0));
                shapes.push(letters(if self.kind == ControlKind::Thermostat { "TH" } else { "TC" }));
            }
            ControlKind::ValveActuator => {
                shapes.push(poly(&[(-150.0, -100.0), (150.0, 100.0), (150.0, -100.0), (-150.0, 100.0)], true));
                shapes.push(circle((0.0, 220.0), 90.0));
                shapes.push(text_in((0.0, 220.0), 80.0, "M"));
                shapes.push(line((0.0, 0.0), (0.0, 130.0)));
                half = 100.0;
            }
            ControlKind::DamperActuator => {
                shapes.push(rect(0.0, 0.0, 200.0, 40.0));
                shapes.push(circle((0.0, 200.0), 90.0));
                shapes.push(text_in((0.0, 200.0), 80.0, "M"));
                shapes.push(line((0.0, 40.0), (0.0, 110.0)));
                half = 40.0;
            }
            ControlKind::FrequencyDrive => {
                shapes.push(rect(0.0, 0.0, 200.0, 150.0));
                shapes.push(letters("VFD"));
            }
            ControlKind::EnergyMeter => {
                shapes.push(circle((0.0, 0.0), 150.0));
                shapes.push(letters("kWh"));
            }
            ControlKind::Controller => {
                shapes.push(rect(0.0, 0.0, 250.0, 180.0));
                shapes.push(line((-250.0, 70.0), (250.0, 70.0)));
                shapes.push(text_in((0.0, -20.0), 100.0, "DDC"));
                half = 180.0;
            }
            ControlKind::IoModule => {
                shapes.push(rect(0.0, 0.0, 200.0, 140.0));
                shapes.push(letters("I/O"));
                half = 140.0;
            }
            ControlKind::Gateway => {
                shapes.push(rect(0.0, 0.0, 200.0, 140.0));
                shapes.push(line((-200.0, 0.0), (200.0, 0.0)));
                shapes.push(text_in((0.0, 70.0), 70.0, "GW"));
                half = 140.0;
            }
            ControlKind::ControlCabinet => {
                shapes.push(rect(0.0, 0.0, 300.0, 200.0));
                shapes.push(line((-300.0, 200.0), (300.0, -200.0)));
                shapes.push(line((-300.0, -200.0), (300.0, 200.0)));
                half = 200.0;
            }
            ControlKind::Supervisor => {
                shapes.push(rect(0.0, 20.0, 250.0, 160.0));
                shapes.push(line((-120.0, -140.0), (120.0, -140.0)));
                shapes.push(letters("SV"));
                half = 180.0;
            }
        }
        let scale = cadcraft_geom::Mat3::scale_about(Vec2::ZERO, 1.0 / self.units.mm_per_unit);
        let mut pieces: Vec<HvacPiece> = shapes
            .into_iter()
            .map(|mut kind| {
                kind.transform(&scale);
                HvacPiece { layer, kind }
            })
            .collect();
        let text = self.label_text();
        if !text.is_empty() {
            let h = self.units.to_units(self.label_height_mm);
            pieces.push(HvacPiece { layer, kind: label(Vec2::new(0.0, -(self.units.to_units(half) + 1.1 * h)), h, text) });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(CONTROL_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<ControlParams> {
        let p: ControlParams = xdata::from_xdata(CONTROL_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The control device an object is, when it is a block reference carrying usable control data.
pub fn control_of(e: &crate::Entity) -> Option<ControlParams> {
    match &e.kind {
        EntityKind::Insert(_) => ControlParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour a control layer is created with.
pub fn control_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        CONTROL_LAYER => Some(Color::True(Rgb(0x30, 0xb0, 0xb0))),
        CONTROL_AUTOMATION_LAYER => Some(Color::True(Rgb(0x90, 0x60, 0xd0))),
        _ => None,
    }
}

impl crate::Drawing {
    /// Adds a control device of `p` at `at`, turned `rotation` radians.
    pub fn add_control(
        &mut self,
        space: &crate::Space,
        mut common: crate::Common,
        p: &ControlParams,
        at: Vec2,
        rotation: f64,
    ) -> Result<crate::Handle> {
        if !at.is_finite() || !rotation.is_finite() {
            return Err(invalid("A control device needs a usable position."));
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(CONTROL_APP, &common.xdata, p.to_xdata()?);
        let layer = p.layer();
        common.layer = layer.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(layer);
        let insert = crate::Insert { block, insert: v3(at), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives control device `h` the parameters `p`, redrawing it in place.
    pub fn regenerate_control(&mut self, h: crate::Handle, p: &ControlParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(CONTROL_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        let layer = p.layer();
        self.ensure_layer(layer);
        self.modify_entity(h, |e| e.common.layer = layer.into())?;
        Ok(())
    }
}
