//! The controls commands: CONTROLADD puts a sensor, a thermostat, an actuator or an automation device on the drawing, CONTROLEDIT changes it, CONTROLCHECK reads the
//! points back and says what is wrong, CONTROLTAKEOFF writes the list of points.

use cadcraft_doc::{ControlKind, ControlParams, Handle, HvacUnits, control_of};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

const MAX_DEVICES: usize = 5_000;
const MAX_EDITED: usize = 10_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("controladd", "Control Device", run_add)
            .menu(&["Draw", "Controls", "Device"])
            .params(
                "{at ([x, y]) | points ([[x, y]…]: one at each), kind? (roomTemperature, ductTemperature, waterTemperature, outdoorTemperature, humidity, co2, voc, differentialPressure, flow, occupancy, thermostat, \
                 vrfRemote, valveActuator, damperActuator, frequencyDrive, energyMeter, controller, ioModule, gateway, controlCabinet, supervisor), name? (the tag; default TA1…), controller? (the name of the controller \
                 or module it is wired to), served? (the equipment, unit or part it serves or measures), circuit? (the circuit that feeds it), bus? (the bus it is on), protocol? (of the bus, on a controller or a gateway: modbus, bacnet, knx or mbus: the addresses, the number of devices and the length of the bus are checked), address? (on its bus or its controller: a Modbus number, a BACnet MAC, a KNX 1.1.5), showAddress? (write it beside the device), setpoint?, ai? ao? di? do? (the points a controller or a \
                 module offers), height? (mm), rotation? (degrees), labelHeight? (mm)} → {handles}",
            )
            .interactive(|s| Ok(Box::new(ControlAddM::new(s)))),
        CommandSpec::new("controledit", "Edit Control Device", run_edit).params(
            "{handle | handles, name?, controller?, served?, circuit?, bus?, address?, showAddress?, setpoint? (or null), ai? ao? di? do?, height?} → {changed}",
        ),
        CommandSpec::new("addressdisplay", "Show Addresses", run_addresses).menu(&["Draw", "Controls", "Show Addresses"]).params(
            "{show? (true: write the address beside every object that has one, the default; false: hide them), list? (true: only list)} → {changed, addresses: [{handle, tag, kind, address, bus, space, shown}], clashes}: \
             the objects that can have an address are the control devices, the equipment, the air handling units and the electrical devices; the table lists every address, and the clashes are the addresses \
             used twice on one bus or on one controller",
        ),
        CommandSpec::new("controlcheck", "Check Controls", run_check).menu(&["Draw", "Controls", "Check Controls"]).params(
            "{} → {devices: [{name, count, points}], controllers: [{name, kind, extends, space, bus, circuit, capacity, used, reservePercent}], points: [{device, kind, description, type, unit, controller, space, served, setpoint}], \
             warnings, problems}: what each device asks of its controller, the load of each controller and module against what it offers, and what is wrong (a device wired to nothing or to a controller that is not drawn, a \
             controller over its capacity, an automation device no circuit feeds, a bus with no cable, a device that serves an object that is not drawn)",
        ),
        CommandSpec::new("controlschema", "Controls Principle Diagram", run_schema)
            .menu(&["Draw", "Controls", "Principle Diagram"])
            .params(
                "{at ([x, y]: the upper left corner), bus? (one bus; default all), title?, labelHeight? (mm)} → {handle, controllers}: the buses with the controllers on them, each controller \
                 and its modules with what they offer and use, and under it the points wired to it with their address and type; it is redrawn when control devices are added, changed or erased",
            )
            .interactive(|_| Ok(Box::new(SchemaM))),
        CommandSpec::new("controltakeoff", "Control Points", run_takeoff).menu(&["Draw", "Controls", "Points List"]).params(
            "{level?, path? (.xlsx: the controls package; else .csv: the list of points), prices? ({\"Control|Room thermostat\": euros each…}: adds `costing`), decimal?: comma|point} → {level, devices, controllers, points, \
             warnings, costing?, path?}",
        ),
    ]
}

/// What CONTROLADD remembers between uses.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlLastUsed {
    pub kind: ControlKind,
    pub controller: String,
}

impl Default for ControlLastUsed {
    fn default() -> Self {
        ControlLastUsed { kind: ControlKind::RoomTemperature, controller: String::new() }
    }
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(|x| Some(Some(x))).ok_or_else(|| bad(cmd, format!("`{key}` must be a number or null"))),
    }
}

/// A count of points: a whole number from 0 to 1000.
fn count(cmd: &str, p: &Value, key: &str) -> Result<Option<u32>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && x.fract() == 0.0 && (0.0..=1000.0).contains(x))
            .map(|x| Some(x as u32))
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number from 0 to 1000"))),
    }
}

fn next_tag(d: &cadcraft_doc::Drawing, kind: ControlKind) -> String {
    let prefix = kind.tag_prefix();
    let n = d.model.iter().filter_map(|e| control_of(e)).filter(|p| p.name.starts_with(prefix)).count() + 1;
    format!("{prefix}{n}")
}

fn put(s: &mut Session, p: &ControlParams, at: Vec2, rotation: f64) -> Result<Handle> {
    p.validate().map_err(doc_err)?;
    let space = s.space();
    let common = s.current_common();
    s.doc_mut()?.add_control(&space, common, p, at, rotation).map_err(doc_err)
}

/// The fields of `p` that a call gives, over `c`.
fn fill(cmd: &str, p: &Value, c: &mut ControlParams) -> Result<()> {
    for (key, slot) in [
        ("name", &mut c.name),
        ("controller", &mut c.controller),
        ("served", &mut c.served),
        ("circuit", &mut c.circuit),
        ("bus", &mut c.bus),
        ("protocol", &mut c.protocol),
        ("address", &mut c.address),
    ] {
        if let Some(t) = str_param(p, key) {
            *slot = t.trim().to_string();
        }
    }
    if let Some(v) = clearable(cmd, p, "setpoint")? {
        c.setpoint = v;
    }
    if let Some(b) = p.get("showAddress").filter(|v| !v.is_null()) {
        c.show_address = b.as_bool().ok_or_else(|| bad(cmd, "`showAddress` must be true or false"))?;
    }
    for (key, slot) in [("ai", &mut c.capacity.ai), ("ao", &mut c.capacity.ao), ("di", &mut c.capacity.di), ("do", &mut c.capacity.dout)] {
        if let Some(n) = count(cmd, p, key)? {
            *slot = n;
        }
    }
    if let Some(Some(h)) = clearable(cmd, p, "height")? {
        c.height_mm = h;
    }
    if let Some(Some(l)) = clearable(cmd, p, "labelHeight")? {
        c.label_height_mm = l;
    }
    Ok(())
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "controladd";
    let mut pts = super::points_param(p, "points").unwrap_or_default();
    if let Some(at) = super::point_param(p, "at") {
        pts.push(at);
    }
    if pts.is_empty() || pts.len() > MAX_DEVICES {
        return Err(bad(CMD, format!("`at` or `points` (1 to {MAX_DEVICES} [x, y]) is required")));
    }
    let kind = match str_param(p, "kind") {
        Some(t) => ControlKind::parse(t).ok_or_else(|| bad(CMD, "`kind` is not a kind of control device: see the parameters"))?,
        None => ControlKind::RoomTemperature,
    };
    let rotation = clearable(CMD, p, "rotation")?.flatten().unwrap_or(0.0).to_radians();
    let mut handles = Vec::new();
    for at in pts {
        let d = s.doc()?;
        let mut c = ControlParams::new(kind, &next_tag(d, kind), HvacUnits::of(d));
        fill(CMD, p, &mut c)?;
        if c.name.is_empty() {
            c.name = next_tag(s.doc()?, kind);
        } else if p.get("points").is_some_and(|v| v.as_array().is_some_and(|a| a.len() > 1)) {
            // One name for several devices: a number after it.
            c.name = format!("{}{}", c.name, handles.len() + 1);
        }
        handles.push(put(s, &c, at, rotation)?.hex());
    }
    Ok(json!({ "handles": handles }))
}

fn schema_at(s: &mut Session, at: Vec2, p: &Value) -> Result<Handle> {
    const CMD: &str = "controlschema";
    let mut params = cadcraft_doc::ControlSchemaParams::new(HvacUnits::of(s.doc()?));
    if let Some(t) = str_param(p, "title") {
        params.title = t.trim().to_string();
    }
    if let Some(b) = str_param(p, "bus") {
        params.bus = b.trim().to_string();
    }
    if let Some(l) = clearable(CMD, p, "labelHeight")?.flatten() {
        params.label_height_mm = l;
    }
    params.validate().map_err(doc_err)?;
    let (space, common) = (s.space(), s.current_common());
    s.doc_mut()?.add_control_schema(&space, common, &params, at).map_err(doc_err)
}

fn run_schema(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "controlschema";
    let at = super::point_req(CMD, p, "at")?;
    let h = schema_at(s, at, p)?;
    let controllers = s.doc()?.control_takeoff("").controllers.len();
    Ok(json!({ "handle": h.hex(), "controllers": controllers }))
}

/// CONTROLSCHEMA at the command line: a click puts the diagram there.
struct SchemaM;

impl Interactive for SchemaM {
    fn name(&self) -> &'static str {
        "CONTROLSCHEMA"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Specify the upper left corner of the diagram", Accept::POINT)
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match i {
            Input::Point(at) => {
                schema_at(s, at, &json!({}))?;
                Ok(Step::Done)
            }
            Input::Enter | Input::Cancel => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "controledit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.len() > MAX_EDITED {
        return Err(bad(CMD, format!("at most {MAX_EDITED} objects")));
    }
    let mut next = Vec::new();
    for h in hs {
        let mut c = s.doc()?.entity(h).and_then(|e| control_of(e)).ok_or_else(|| bad(CMD, "select control devices only"))?;
        fill(CMD, p, &mut c)?;
        c.validate().map_err(doc_err)?;
        next.push((h, c));
    }
    let doc = s.doc_mut()?;
    for (h, c) in &next {
        doc.regenerate_control(*h, c).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

fn takeoff_json(t: &cadcraft_doc::ControlTakeoff) -> Result<Value> {
    let mut v = serde_json::to_value(t).map_err(|e| other(e.to_string()))?;
    v["problems"] = json!(t.warnings.len());
    Ok(v)
}

fn run_check(s: &mut Session, _p: &Value) -> Result<Value> {
    takeoff_json(&s.doc()?.control_takeoff(""))
}

fn run_takeoff(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "controltakeoff";
    let level = str_param(p, "level").unwrap_or("");
    if level.chars().count() > 64 || level.chars().any(char::is_control) {
        return Err(bad(CMD, "`level` has at most 64 characters, without control characters"));
    }
    let comma = match str_param(p, "decimal").map(|t| t.trim().to_ascii_lowercase()).as_deref() {
        None | Some("comma") => true,
        Some("point") => false,
        Some(_) => return Err(bad(CMD, "`decimal` is comma or point")),
    };
    let mut t = s.doc()?.control_takeoff(level);
    if let Some(prices) = p.get("prices").filter(|v| !v.is_null()) {
        let map = prices.as_object().ok_or_else(|| bad(CMD, "`prices` is an object of unit prices by line"))?;
        if map.len() > 10_000 {
            return Err(bad(CMD, "at most 10000 prices"));
        }
        let mut st = cadcraft_doc::TakeoffSettings::default();
        for (k, v) in map {
            let price =
                v.as_f64().and_then(cadcraft_doc::usable_price).ok_or_else(|| bad(CMD, format!("the price of `{k}` is a number of euros")))?;
            st.prices.insert(k.clone(), price);
        }
        t.costing = Some(t.priced(&st));
    }
    let mut out = takeoff_json(&t)?;
    if let Some(path) = str_param(p, "path").map(str::trim).filter(|x| !x.is_empty()) {
        if path.len() > 1024 || path.contains('\0') {
            return Err(bad(CMD, "`path` is not a usable file name"));
        }
        let path = std::path::Path::new(path);
        crate::file_access::check_write(path).map_err(other)?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        }
        let bytes = if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("xlsx")) {
            t.to_xlsx().map_err(doc_err)?
        } else {
            let mut b = vec![0xEF, 0xBB, 0xBF];
            b.extend_from_slice(t.points_csv(comma).as_bytes());
            b
        };
        std::fs::write(path, bytes).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        out["path"] = json!(path.display().to_string());
    }
    Ok(out)
}

struct ControlAddM {
    kind: ControlKind,
    controller: String,
    ask: Option<&'static str>,
}

impl ControlAddM {
    fn new(s: &Session) -> Self {
        ControlAddM { kind: s.last_used.control.kind, controller: s.last_used.control.controller.clone(), ask: None }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!(
            "Current settings: Type = {}, Controller = {}",
            self.kind.name(),
            if self.controller.is_empty() { "-" } else { &self.controller }
        ));
    }
}

impl Interactive for ControlAddM {
    fn name(&self) -> &'static str {
        "CONTROLADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Some("Type") => Prompt::new("Enter the type of device", Accept::TEXT).default(self.kind.name()),
            Some(_) => Prompt::new("Enter the controller it is wired to (empty for none)", Accept::TEXT).default(self.controller.clone()),
            None => Prompt::new("Specify the place of the device", Accept::POINT).kw(&["Type", "Controller"]),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Some(what) = self.ask.take() {
            if let Input::Text(t) | Input::Keyword(t) = &i {
                if what == "Type" {
                    match ControlKind::parse(t) {
                        Some(k) => self.kind = k,
                        None => s.echo("Not a type of control device: see CONTROLADD."),
                    }
                } else {
                    match cadcraft_doc::check_circuit_name(t.trim(), true) {
                        Ok(()) => self.controller = t.trim().to_string(),
                        Err(e) => s.echo(e.to_string()),
                    }
                }
                self.settings(s);
            }
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) if k == "Type" => self.ask = Some("Type"),
            Input::Keyword(k) if k == "Controller" => self.ask = Some("Controller"),
            Input::Point(at) => {
                let d = s.doc()?;
                let mut c = ControlParams::new(self.kind, &next_tag(d, self.kind), HvacUnits::of(d));
                if !self.kind.is_automation() {
                    c.controller = self.controller.clone();
                }
                match put(s, &c, at, 0.0) {
                    Ok(_) => s.last_used.control = ControlLastUsed { kind: self.kind, controller: self.controller.clone() },
                    Err(e) => s.echo(e.to_string()),
                }
            }
            Input::Enter | Input::Cancel => return Ok(Step::Done),
            _ => {}
        }
        Ok(Step::Continue)
    }
    fn preview_objects(&self, s: &Session, c: Vec2) -> Vec<cadcraft_doc::Entity> {
        let Ok(d) = s.doc() else { return Vec::new() };
        let p = ControlParams::new(self.kind, &next_tag(d, self.kind), HvacUnits::of(d));
        p.generate().map(|pieces| cadcraft_doc::preview_entities(pieces, c, 0.0)).unwrap_or_default()
    }
}

fn run_addresses(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "addressdisplay";
    let show = match p.get("show").filter(|v| !v.is_null()) {
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "`show` must be true or false"))?,
        None => true,
    };
    let list_only = p.get("list").and_then(Value::as_bool).unwrap_or(false);
    let changed = if list_only { 0 } else { s.doc_mut()?.set_address_display(show).map_err(doc_err)? };
    let d = s.doc()?;
    Ok(json!({ "changed": changed, "addresses": d.address_rows(), "clashes": d.address_clashes() }))
}
