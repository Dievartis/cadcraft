//! The electrical commands: ELECDEVICEADD and CABLEADD put devices and cables on the drawing, ELECEDIT changes them, CIRCUITEDIT, CIRCUITASSIGN, CIRCUITCHECK and
//! CIRCUITSIZE work on a circuit as a whole, LIGHTINGCHECK compares the lighting of the rooms with the code, ELECTAKEOFF counts and prices.

use cadcraft_doc::{
    CableParams, CableUsage, CircuitChange, CircuitOptions, DeviceKind, DeviceParams, ElecConductor, ElecInsulation, ElecLaying, ElecPhases, Handle,
    HvacUnits, cable_of, device_of,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most points of one cable, most devices one command puts, most objects one edit changes.
const MAX_CABLE_POINTS: usize = 512;
const MAX_DEVICES: usize = 5_000;
const MAX_EDITED: usize = 10_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("elecdeviceadd", "Electrical Device", run_device_add)
            .menu(&["Draw", "Electricity", "Device"])
            .params(
                "{at ([x, y]) | points ([[x, y]…]: one device at each), kind? (socket, dedicatedSocket, switch, doubleSwitch, twoWaySwitch, pushButton, dimmer, presenceDetector, dataOutlet, \
                 ceilingLight, downlight, wallLight, stripLight, emergencyLight, exitSign, distributionBoard), name? (the tag; default P1, I2, L3…), circuit?, power? (W), flux? (lm, a luminaire), \
                 height? (mm above the floor), supply? (A, a board's main breaker), rotation? (degrees), labelHeight? (mm)} → {handles}",
            )
            .interactive(|s| Ok(Box::new(DeviceAddM::new(s)))),
        CommandSpec::new("cableadd", "Cable", run_cable_add)
            .menu(&["Draw", "Electricity", "Cable"])
            .params(
                "{points (2 or more; one cable run per segment), circuit? (\"TD1-C3\": the board, then the circuit: its cables share a colour), usage? (power|lighting|data|safety), phases? (single|three), \
                 conductor? (copper|aluminium), insulation? (pvc|pr), laying? (B|C|E|F: the selection letter), section? (mm²: none while it is to be sized), protection? (A), cosPhi?, ambient? (°C), \
                 grouped? (circuits laid together), extra? (m of rises and slack per run), labelHeight? (mm)} → {handles}",
            )
            .interactive(|s| Ok(Box::new(CableAddM::new(s)))),
        CommandSpec::new("elecedit", "Edit Electrical Object", run_edit).params(
            "{handle | handles, for a device: name?, circuit?, power? (W), flux? (lm or null), height? (mm), supply? (A or null); for a cable: circuit?, usage?, phases?, conductor?, insulation?, laying?, \
             section? (mm² or null), protection? (A or null), cosPhi?, ambient?, grouped?, extra?} → {changed}",
        ),
        CommandSpec::new("circuitedit", "Edit Circuit", run_circuit_edit).params(
            "{name, rename? (the new name, on every cable, device, equipment and unit of the circuit), + usage?, phases?, conductor?, insulation?, laying?, section?, protection?, cosPhi?, ambient?, grouped?, extra? \
             (on every cable of the circuit)} → {changed}",
        ),
        CommandSpec::new("circuitassign", "Assign to Circuit", run_assign)
            .menu(&["Draw", "Electricity", "Assign to Circuit"])
            .params("{handles? (default: the selection), circuit (a name; empty puts them on none)} → {changed}: a device, a cable, an equipment or an air handling unit")
            .interactive(|s| Ok(Box::new(AssignM { circuit: s.last_used.device.circuit.clone() }))),
        CommandSpec::new("circuitcheck", "Check Circuits", run_check).menu(&["Draw", "Electricity", "Check Circuits"]).params(
            "{ownSubstation? (the building owns its HV/LV substation: 6 % and 8 % of voltage drop, not 3 % and 5 %), diversity? (0 to 1, the share of the power that is on at once, default 1)} → \
             {circuits: [{name, panel, panelDrawn, usage, phases, devices, hvac, powerW, currentA, cableLengthM, sectionMm2, thermalSectionMm2, dropSectionMm2, requiredSectionMm2, protectionA, ratingA, dropPercent, \
             maxDropPercent, warnings, cables}], warnings, problems}: the current of each circuit, the rating of its breaker, the section it needs for the current and for the voltage drop, and what is wrong with the cable drawn",
        ),
        CommandSpec::new("circuitsize", "Size Circuits", run_size).menu(&["Draw", "Electricity", "Size Circuits"]).params(
            "{circuit? (one circuit; default all), overwrite? (also replace the sections already given), ownSubstation?, diversity?} → {circuits}: writes on the cables of each circuit the section it needs and, where there is none, \
             the rating of its breaker",
        ),
        CommandSpec::new("lightingcheck", "Check Lighting", run_lighting).menu(&["Draw", "Electricity", "Check Lighting"]).params(
            "{utilisation? (0 to 1: the share of the flux that reaches the work plane, default 0.5), maintenance? (0 to 1, default 0.8)} → {spaces: [{space, areaM2, luminaires, fluxLm, lux, minimumLux, class, ok, warning}], \
             warnings, problems}: the illuminance each space gets from the luminaires drawn in it (flux × utilisation × maintenance ÷ area), against the minimum of the Code du travail, article R4223-4 (40 lux for a circulation, \
             60 for stairs and stores, 120 for a workroom, a cloakroom or a toilet; the class follows the space's name)",
        ),
        CommandSpec::new("electakeoff", "Electrical Quantities", run_takeoff).menu(&["Draw", "Electricity", "Quantities"]).params(
            "{level? (the name of the floor), path? (a file to write: .xlsx for a workbook, else .csv of the circuits), prices? ({\"Cable|3G2.5\": euros per metre, \"Device|Socket\": euros each…}: adds `costing`), \
             decimal?: comma|point, ownSubstation?, diversity?} → {level, cables, devices, panels, hvac, circuits, cableLengthM, installedPowerW, costing?, path?}",
        ),
    ]
}

/// What CABLEADD remembers between uses.
#[derive(Clone, Debug, PartialEq)]
pub struct CableLastUsed {
    pub circuit: String,
    pub usage: CableUsage,
    pub phases: ElecPhases,
    pub section_mm2: Option<f64>,
    pub protection_a: Option<f64>,
}

impl Default for CableLastUsed {
    fn default() -> Self {
        CableLastUsed { circuit: String::new(), usage: CableUsage::Power, phases: ElecPhases::Single, section_mm2: None, protection_a: None }
    }
}

/// What ELECDEVICEADD remembers between uses.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceLastUsed {
    pub kind: DeviceKind,
    pub circuit: String,
}

impl Default for DeviceLastUsed {
    fn default() -> Self {
        DeviceLastUsed { kind: DeviceKind::Socket, circuit: String::new() }
    }
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

/// A number from a command: absent keeps, null clears (`Some(None)`).
fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(|x| Some(Some(x))).ok_or_else(|| bad(cmd, format!("`{key}` must be a number or null"))),
    }
}

fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    Ok(clearable(cmd, p, key)?.flatten())
}

fn word<T>(cmd: &str, p: &Value, key: &str, parse: impl Fn(&str) -> Option<T>) -> Result<Option<T>> {
    match str_param(p, key) {
        Some(t) => parse(t).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` is not one of the values of the command: see its parameters"))),
        None => Ok(None),
    }
}

/// The changes the keys of `p` ask of every cable (and of the circuit that has them).
fn change_of(cmd: &str, p: &Value) -> Result<CircuitChange> {
    Ok(CircuitChange {
        usage: word(cmd, p, "usage", CableUsage::parse)?,
        phases: word(cmd, p, "phases", ElecPhases::parse)?,
        conductor: word(cmd, p, "conductor", ElecConductor::parse)?,
        insulation: word(cmd, p, "insulation", ElecInsulation::parse)?,
        laying: word(cmd, p, "laying", ElecLaying::parse)?,
        section_mm2: clearable(cmd, p, "section")?,
        protection_a: clearable(cmd, p, "protection")?,
        cos_phi: number(cmd, p, "cosPhi")?,
        ambient_c: number(cmd, p, "ambient")?,
        grouped: number(cmd, p, "grouped")?.map(|g| g.round().clamp(0.0, 1000.0) as u32),
        extra_m: number(cmd, p, "extra")?,
    })
}

fn apply_change(c: &mut CableParams, ch: &CircuitChange) {
    if let Some(v) = ch.usage {
        c.usage = v;
    }
    if let Some(v) = ch.phases {
        c.phases = v;
    }
    if let Some(v) = ch.conductor {
        c.conductor = v;
    }
    if let Some(v) = ch.insulation {
        c.insulation = v;
    }
    if let Some(v) = ch.laying {
        c.laying = v;
    }
    if let Some(v) = ch.section_mm2 {
        c.section_mm2 = v;
    }
    if let Some(v) = ch.protection_a {
        c.protection_a = v;
    }
    if let Some(v) = ch.cos_phi {
        c.cos_phi = v;
    }
    if let Some(v) = ch.ambient_c {
        c.ambient_c = v;
    }
    if let Some(v) = ch.grouped {
        c.grouped = v;
    }
    if let Some(v) = ch.extra_m {
        c.extra_m = v;
    }
}

fn options_of(cmd: &str, p: &Value) -> Result<CircuitOptions> {
    let mut o = CircuitOptions::default();
    if let Some(v) = p.get("ownSubstation").filter(|v| !v.is_null()) {
        o.own_substation = v.as_bool().ok_or_else(|| bad(cmd, "`ownSubstation` must be true or false"))?;
    }
    if let Some(d) = number(cmd, p, "diversity")? {
        if !(0.0..=1.0).contains(&d) {
            return Err(bad(cmd, "`diversity` is 0 to 1"));
        }
        o.diversity = d;
    }
    Ok(o)
}

/// The tag a new device of `kind` takes: its prefix and the number after the devices of that prefix already drawn.
fn next_tag(d: &cadcraft_doc::Drawing, kind: DeviceKind) -> String {
    let prefix = kind.tag_prefix();
    let n = d.model.iter().filter_map(|e| device_of(e)).filter(|p| p.name.starts_with(prefix)).count() + 1;
    format!("{prefix}{n}")
}

fn put_device(s: &mut Session, p: &DeviceParams, at: Vec2, rotation: f64) -> Result<Handle> {
    p.validate().map_err(doc_err)?;
    let space = s.space();
    let common = s.current_common();
    let h = s.doc_mut()?.add_device(&space, common, p, at, rotation).map_err(doc_err)?;
    if !p.circuit.trim().is_empty() && s.doc()?.network_style(&p.circuit).is_none() {
        s.doc_mut()?.set_network_style(&p.circuit, None, None).map_err(doc_err)?;
    }
    Ok(h)
}

fn run_device_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "elecdeviceadd";
    let mut pts = super::points_param(p, "points").unwrap_or_default();
    if let Some(at) = super::points_param(p, "at").and_then(|v| v.first().copied()).or_else(|| super::point_param(p, "at")) {
        pts.push(at);
    }
    if pts.is_empty() || pts.len() > MAX_DEVICES {
        return Err(bad(CMD, format!("`at` or `points` (1 to {MAX_DEVICES} [x, y]) is required")));
    }
    let kind = word(CMD, p, "kind", DeviceKind::parse)?.unwrap_or(DeviceKind::Socket);
    let rotation = number(CMD, p, "rotation")?.unwrap_or(0.0).to_radians();
    let mut handles = Vec::new();
    for at in pts {
        let d = s.doc()?;
        let name = str_param(p, "name").map_or_else(|| next_tag(d, kind), str::to_string);
        let mut dp = DeviceParams::new(kind, &name, HvacUnits::of(d));
        dp.circuit = str_param(p, "circuit").unwrap_or("").trim().to_string();
        if let Some(w) = number(CMD, p, "power")? {
            dp.power_w = w;
        }
        if let Some(f) = clearable(CMD, p, "flux")? {
            dp.flux_lm = f;
        }
        if let Some(h) = number(CMD, p, "height")? {
            dp.height_mm = h;
        }
        if let Some(a) = clearable(CMD, p, "supply")? {
            dp.supply_a = a;
        }
        if let Some(l) = number(CMD, p, "labelHeight")? {
            dp.label_height_mm = l;
        }
        handles.push(put_device(s, &dp, at, rotation)?.hex());
    }
    Ok(json!({ "handles": handles }))
}

fn cable_params(s: &Session, last: &CableLastUsed) -> Result<CableParams> {
    let mut c = CableParams::new(&last.circuit, last.usage, HvacUnits::of(s.doc()?));
    c.phases = last.phases;
    c.section_mm2 = last.section_mm2;
    c.protection_a = last.protection_a;
    Ok(c)
}

fn add_run(s: &mut Session, params: &CableParams, a: Vec2, b: Vec2) -> Result<Handle> {
    if a.near(b, 1e-9) || !a.is_finite() || !b.is_finite() {
        return Err(other("A cable needs two different points."));
    }
    CableParams { length: a.dist(b), ..params.clone() }.validate().map_err(doc_err)?;
    let space = s.space();
    let common = s.current_common();
    s.doc_mut()?.add_cable(&space, common, params.clone(), a, b).map_err(doc_err)
}

fn run_cable_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cableadd";
    let pts = super::points_param(p, "points").filter(|v| v.len() >= 2).ok_or_else(|| bad(CMD, "`points` (two or more [x, y]) is required"))?;
    if pts.len() > MAX_CABLE_POINTS {
        return Err(bad(CMD, format!("at most {MAX_CABLE_POINTS} points")));
    }
    let mut params = cable_params(s, &CableLastUsed::default())?;
    params.circuit = str_param(p, "circuit").unwrap_or("").trim().to_string();
    apply_change(&mut params, &change_of(CMD, p)?);
    if let Some(l) = number(CMD, p, "labelHeight")? {
        params.label_height_mm = l;
    }
    params.length = 1.0;
    params.validate().map_err(doc_err)?;
    let mut handles = Vec::new();
    for w in pts.windows(2) {
        if let [a, b] = w {
            handles.push(add_run(s, &params, *a, *b)?.hex());
        }
    }
    Ok(json!({ "handles": handles }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "elecedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.len() > MAX_EDITED {
        return Err(bad(CMD, format!("at most {MAX_EDITED} objects")));
    }
    let change = change_of(CMD, p)?;
    let circuit = str_param(p, "circuit").map(|c| c.trim().to_string());
    let mut done: Vec<(Handle, Option<CableParams>, Option<DeviceParams>)> = Vec::new();
    for h in hs {
        let e = s.doc()?.entity(h).cloned().ok_or_else(|| bad(CMD, "no such object"))?;
        if let Some(mut c) = cable_of(&e) {
            apply_change(&mut c, &change);
            if let Some(n) = &circuit {
                c.circuit = n.clone();
            }
            c.validate().map_err(doc_err)?;
            done.push((h, Some(c), None));
        } else if let Some(mut d) = device_of(&e) {
            if let Some(n) = str_param(p, "name") {
                d.name = n.trim().to_string();
            }
            if let Some(n) = &circuit {
                d.circuit = n.clone();
            }
            if let Some(w) = number(CMD, p, "power")? {
                d.power_w = w;
            }
            if let Some(f) = clearable(CMD, p, "flux")? {
                d.flux_lm = f;
            }
            if let Some(v) = number(CMD, p, "height")? {
                d.height_mm = v;
            }
            if let Some(a) = clearable(CMD, p, "supply")? {
                d.supply_a = a;
            }
            d.validate().map_err(doc_err)?;
            done.push((h, None, Some(d)));
        } else {
            return Err(bad(CMD, "select cables and electrical devices only"));
        }
    }
    let doc = s.doc_mut()?;
    for (h, c, d) in &done {
        if let Some(c) = c {
            doc.regenerate_cable(*h, c).map_err(doc_err)?;
        }
        if let Some(d) = d {
            doc.regenerate_device(*h, d).map_err(doc_err)?;
            if !d.circuit.is_empty() && doc.network_style(&d.circuit).is_none() {
                doc.set_network_style(&d.circuit, None, None).map_err(doc_err)?;
            }
        }
    }
    Ok(json!({ "changed": done.len() }))
}

fn run_circuit_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "circuitedit";
    let name = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(CMD, "`name` is required"))?.to_string();
    let change = change_of(CMD, p)?;
    let doc = s.doc_mut()?;
    let mut changed = doc.edit_circuit(&name, &change).map_err(doc_err)?;
    if let Some(to) = str_param(p, "rename") {
        changed += doc.rename_circuit(&name, to).map_err(doc_err)?;
    }
    Ok(json!({ "changed": changed }))
}

fn run_assign(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "circuitassign";
    let hs = if p.get("handles").is_some() || p.get("handle").is_some() { super::arch::handles_param(CMD, p)? } else { s.selection() };
    if hs.is_empty() || hs.len() > MAX_EDITED {
        return Err(bad(CMD, "select 1 to 10000 objects"));
    }
    let circuit = str_param(p, "circuit").unwrap_or("").to_string();
    let n = s.doc_mut()?.assign_circuit(&hs, &circuit).map_err(doc_err)?;
    Ok(json!({ "changed": n }))
}

fn reports_json(reports: &[cadcraft_doc::CircuitReport]) -> Value {
    let warnings: Vec<String> = reports.iter().flat_map(|r| r.warnings.iter().map(|w| format!("{}: {w}", r.name))).collect();
    json!({ "circuits": reports, "warnings": warnings, "problems": reports.iter().filter(|r| !r.ok()).count() })
}

fn run_check(s: &mut Session, p: &Value) -> Result<Value> {
    let opts = options_of("circuitcheck", p)?;
    Ok(reports_json(&s.doc()?.circuit_reports(&opts)))
}

fn run_size(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "circuitsize";
    let opts = options_of(CMD, p)?;
    let overwrite = p.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
    let only = str_param(p, "circuit");
    let reports = s.doc_mut()?.size_circuits(&opts, only, overwrite).map_err(doc_err)?;
    Ok(reports_json(&reports))
}

// ---------------------------------------------------------------------------------------------------------------
// Interactive forms
// ---------------------------------------------------------------------------------------------------------------

struct DeviceAddM {
    kind: DeviceKind,
    circuit: String,
    ask: Option<&'static str>,
}

impl DeviceAddM {
    fn new(s: &Session) -> Self {
        DeviceAddM { kind: s.last_used.device.kind, circuit: s.last_used.device.circuit.clone(), ask: None }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!("Current settings: Type = {}, Circuit = {}", self.kind.name(), if self.circuit.is_empty() { "-" } else { &self.circuit }));
    }
}

impl Interactive for DeviceAddM {
    fn name(&self) -> &'static str {
        "ELECDEVICEADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Some("Type") => Prompt::new("Enter the type of device", Accept::TEXT).default(self.kind.name()),
            Some(_) => Prompt::new("Enter the circuit (empty for none)", Accept::TEXT).default(self.circuit.clone()),
            None => Prompt::new("Specify the place of the device", Accept::POINT).kw(&["Type", "Circuit"]),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Some(what) = self.ask.take() {
            if let Input::Text(t) | Input::Keyword(t) = &i {
                if what == "Type" {
                    match DeviceKind::parse(t) {
                        Some(k) => self.kind = k,
                        None => s.echo("Not a type of device: see ELECDEVICEADD."),
                    }
                } else {
                    let name = t.trim().to_string();
                    match cadcraft_doc::check_circuit_name(&name, true) {
                        Ok(()) => self.circuit = name,
                        Err(e) => s.echo(e.to_string()),
                    }
                }
                self.settings(s);
            }
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) if k == "Type" => self.ask = Some("Type"),
            Input::Keyword(k) if k == "Circuit" => self.ask = Some("Circuit"),
            Input::Point(at) => {
                let d = s.doc()?;
                let mut dp = DeviceParams::new(self.kind, &next_tag(d, self.kind), HvacUnits::of(d));
                dp.circuit = self.circuit.clone();
                match put_device(s, &dp, at, 0.0) {
                    Ok(_) => s.last_used.device = DeviceLastUsed { kind: self.kind, circuit: self.circuit.clone() },
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
        let p = DeviceParams::new(self.kind, &next_tag(d, self.kind), HvacUnits::of(d));
        p.generate().map(|pieces| cadcraft_doc::preview_entities(pieces, c, 0.0)).unwrap_or_default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CableAsk {
    Point,
    Circuit,
    Usage,
    Section,
    Protection,
}

struct CableAddM {
    last: CableLastUsed,
    pts: Vec<Vec2>,
    ask: CableAsk,
}

impl CableAddM {
    fn new(s: &Session) -> Self {
        CableAddM { last: s.last_used.cable.clone(), pts: Vec::new(), ask: CableAsk::Point }
    }
    fn settings(&self, s: &mut Session) {
        let l = &self.last;
        s.echo(format!(
            "Current settings: Circuit = {}, Usage = {}, Section = {}, Protection = {}",
            if l.circuit.is_empty() { "-" } else { &l.circuit },
            l.usage.name(),
            l.section_mm2.map_or("Auto".to_string(), |v| format!("{v} mm²")),
            l.protection_a.map_or("Auto".to_string(), |v| format!("{v} A"))
        ));
    }
}

impl Interactive for CableAddM {
    fn name(&self) -> &'static str {
        "CABLEADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            CableAsk::Circuit => Prompt::new("Enter the circuit (e.g. TD1-C3)", Accept::TEXT).default(self.last.circuit.clone()),
            CableAsk::Usage => {
                Prompt::new("Enter the use", super::curves::KW).kw(&["Power", "Lighting", "Data", "Safety"]).default(self.last.usage.name())
            }
            CableAsk::Section => Prompt::new("Enter the section (mm²) or [Auto]", Accept::TEXT)
                .default(self.last.section_mm2.map_or("Auto".to_string(), |v| v.to_string())),
            CableAsk::Protection => Prompt::new("Enter the rating of the breaker (A) or [Auto]", Accept::TEXT)
                .default(self.last.protection_a.map_or("Auto".to_string(), |v| v.to_string())),
            CableAsk::Point => match self.pts.last().copied() {
                None => Prompt::new("Specify start point of cable", Accept::POINT).kw(&["Circuit", "Usage", "Section", "Protection"]),
                last => Prompt::new("Specify next point", Accept::POINT).kw(&["Undo"]).base_opt(last),
            },
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        match (self.ask, text) {
            (CableAsk::Point, _) => {}
            (CableAsk::Circuit, Some(t)) => match cadcraft_doc::check_circuit_name(&t, true) {
                Ok(()) => self.last.circuit = t,
                Err(e) => s.echo(e.to_string()),
            },
            (CableAsk::Usage, Some(t)) => match CableUsage::parse(&t) {
                Some(u) => self.last.usage = u,
                None => s.echo("Enter Power, Lighting, Data or Safety."),
            },
            (CableAsk::Section, Some(t)) => {
                let v =
                    if t.eq_ignore_ascii_case("auto") { Some(None) } else { t.replace(',', ".").parse::<f64>().ok().filter(|x| *x > 0.0).map(Some) };
                match v {
                    Some(v) => self.last.section_mm2 = v,
                    None => s.echo("Requires a section in mm² or Auto."),
                }
            }
            (CableAsk::Protection, Some(t)) => {
                let v =
                    if t.eq_ignore_ascii_case("auto") { Some(None) } else { t.replace(',', ".").parse::<f64>().ok().filter(|x| *x > 0.0).map(Some) };
                match v {
                    Some(v) => self.last.protection_a = v,
                    None => s.echo("Requires a rating in A or Auto."),
                }
            }
            _ => {}
        }
        if self.ask != CableAsk::Point {
            self.ask = CableAsk::Point;
            self.settings(s);
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) => match k.as_str() {
                "Circuit" => self.ask = CableAsk::Circuit,
                "Usage" => self.ask = CableAsk::Usage,
                "Section" => self.ask = CableAsk::Section,
                "Protection" => self.ask = CableAsk::Protection,
                "Undo" => {
                    self.pts.pop();
                }
                _ => {}
            },
            Input::Point(p) => {
                if let Some(prev) = self.pts.last().copied() {
                    let params = cable_params(s, &self.last)?;
                    if let Err(e) = add_run(s, &params, prev, p) {
                        s.echo(e.to_string());
                        return Ok(Step::Continue);
                    }
                    s.last_used.cable = self.last.clone();
                }
                self.pts.push(p);
            }
            Input::Enter | Input::Cancel => return Ok(Step::Done),
            _ => {}
        }
        Ok(Step::Continue)
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        match self.pts.last() {
            Some(p) => vec![cadcraft_doc::EntityKind::Line(cadcraft_doc::Line {
                a: cadcraft_geom::Vec3::new(p.x, p.y, 0.0),
                b: cadcraft_geom::Vec3::new(c.x, c.y, 0.0),
            })],
            None => Vec::new(),
        }
    }
}

fn run_lighting(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "lightingcheck";
    let mut o = cadcraft_doc::LightingOptions::default();
    for (key, slot) in [("utilisation", &mut o.utilisation), ("maintenance", &mut o.maintenance)] {
        if let Some(v) = number(CMD, p, key)? {
            if !(0.05..=1.0).contains(&v) {
                return Err(bad(CMD, format!("`{key}` is 0.05 to 1")));
            }
            *slot = v;
        }
    }
    let reports = s.doc()?.lighting_reports(&o);
    let warnings: Vec<String> = reports.iter().filter_map(|r| r.warning.as_ref().map(|w| format!("{}: {w}", r.space))).collect();
    Ok(json!({ "spaces": reports, "warnings": warnings, "problems": reports.iter().filter(|r| !r.ok).count() }))
}

fn run_takeoff(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "electakeoff";
    let level = str_param(p, "level").unwrap_or("");
    if level.chars().count() > 64 || level.chars().any(char::is_control) {
        return Err(bad(CMD, "`level` has at most 64 characters, without control characters"));
    }
    let comma = match str_param(p, "decimal").map(|t| t.trim().to_ascii_lowercase()).as_deref() {
        None | Some("comma") => true,
        Some("point") => false,
        Some(_) => return Err(bad(CMD, "`decimal` is comma or point")),
    };
    let mut t = s.doc()?.elec_takeoff(level, &options_of(CMD, p)?).map_err(doc_err)?;
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
    let mut out = serde_json::to_value(&t).map_err(|e| other(e.to_string()))?;
    if let Some(path) = str_param(p, "path").map(str::trim).filter(|x| !x.is_empty()) {
        if path.len() > 1024 || path.contains('\0') {
            return Err(bad(CMD, "`path` is not a usable file name"));
        }
        let path = std::path::Path::new(path);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        }
        let bytes = if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("xlsx")) {
            t.to_xlsx().map_err(doc_err)?
        } else {
            let mut b = vec![0xEF, 0xBB, 0xBF];
            b.extend_from_slice(t.circuits_csv(comma).as_bytes());
            b
        };
        std::fs::write(path, bytes).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        out["path"] = json!(path.display().to_string());
    }
    Ok(out)
}

/// CIRCUITASSIGN at the command line: the circuit's name, then the selection (the pickfirst one) goes on it.
struct AssignM {
    circuit: String,
}

impl Interactive for AssignM {
    fn name(&self) -> &'static str {
        "CIRCUITASSIGN"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        if s.selection().is_empty() {
            s.echo("Select the devices, cables and equipment first, then CIRCUITASSIGN.");
            return Ok(Step::Done);
        }
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        Prompt::new("Enter the circuit (e.g. TD1-C3; empty for none)", Accept::TEXT).default(self.circuit.clone())
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let name = match &i {
            Input::Text(t) | Input::Keyword(t) => t.trim().to_string(),
            Input::Enter => self.circuit.clone(),
            _ => return Ok(Step::Done),
        };
        let hs = s.selection();
        match s.doc_mut()?.assign_circuit(&hs, &name) {
            Ok(n) => {
                s.echo(format!("{n} object(s) put on circuit {name}."));
                s.last_used.device.circuit = name;
            }
            Err(e) => s.echo(e.to_string()),
        }
        Ok(Step::Done)
    }
}
