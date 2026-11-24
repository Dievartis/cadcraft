//! Risers (colonnes): RISERADD draws the shaft outline of a riser where a network goes up or down
//! through the floors, with its symbol, arrow and label; RISEREDIT changes one already drawn. Each
//! floor is its own drawing, so the risers of two floors meet by name.

use cadcraft_doc::{
    AirDirection, CableData, CableLine, Drawing, EntityKind, Handle, HvacUnits, MAX_CABLE_LINES, MAX_RISER_VERTICES, RiserMedium, RiserParams,
    RiserSize, any_riser_of,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("riseradd", "Riser", run_add)
            .menu(&["Draw", "HVAC", "Riser"])
            .params(
                "{points | handle (closed polyline) | at + width + depth (mm), name?, air? (supply|extract|fresh|exhaust), direction? (up|down), \\
                 incomingFlow? (m³/h that comes from, or goes on to, the other floors: written at the lower end of the leader), incomingSize? (its section; default: sized for that flow), flow? (m³/h), size? (diameter, \"400x250\" or \"auto\"), height? (mm, floor to floor), labelHeight? (mm)} → {handle, name}",
            )
            .interactive(|s| Ok(Box::new(RiserAddM::new(s)))),
        CommandSpec::new("elecriser", "Electrical Riser", run_add_power)
            .menu(&["Draw", "Electricity", "Riser"])
            .params(
                "{points | handle (closed polyline) | at + width + depth (mm), name?, medium? (power|lighting|data|safety; default power), lines? (a list: a circuit name, or {usage? (power|lighting|data|safety|bus), circuit?, section? (mm², or the cable as written: 3G1.5, 2x0.75 blindé), colour? (#rrggbb: a colour of its own for the line), \
                 load? (kW carried at this level), incomingLoad? (kW that comes from, or goes on to, the other floors: added to the circuit's load)}; default one line), or circuit, section, load and \
                 incomingLoad for that one line; direction? (up|down: the way the current goes), height? (mm, floor to floor: counted in the length of each circuit it carries), labelHeight? (mm)} → {handle, name}",
            )
            .interactive(|s| Ok(Box::new(RiserAddM::with_medium(s, RiserMedium::Power)))),
        CommandSpec::new("busriser", "Bus Riser", run_add_bus)
            .menu(&["Draw", "Controls", "Bus Riser"])
            .params(
                "{points | handle (closed polyline) | at + width + depth (mm), name?, lines? (a list of {circuit: the bus, usage?}; default one line of the bus) or circuit for that one line, direction? (up|down), \
                 height? (mm, floor to floor), labelHeight? (mm)} → {handle, name}",
            )
            .interactive(|s| Ok(Box::new(RiserAddM::with_medium(s, RiserMedium::Bus)))),
        CommandSpec::new("riseredit", "Edit Riser", run_edit).params(
            "{handle | handles, name?, air?, direction?, width? + depth? (the shaft, mm), incomingFlow? (or null), incomingSize? (or null), flow? (or null), size? (or \"auto\"), height? (mm, or null); for a riser of cables: medium? (air|power|lighting|data|safety|bus), circuit?, load? (kW or null), \
             incomingLoad? (kW or null), section? (mm² or null)} → {changed}",
        ),
    ]
}

/// What RISERADD remembers between uses: the air direction and the way.
#[derive(Clone, Debug, PartialEq)]
pub struct RiserLastUsed {
    pub air: AirDirection,
    pub up: bool,
}

impl Default for RiserLastUsed {
    fn default() -> Self {
        RiserLastUsed { air: AirDirection::Extract, up: true }
    }
}

/// Most risers one RISEREDIT changes.
const MAX_EDITED: usize = 10_000;

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a number"))),
    }
}

/// A key that may be a number, or null to clear it: None when the key is absent.
fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(_) => Ok(Some(number(cmd, p, key)?)),
    }
}

fn air_param(cmd: &str, p: &Value) -> Result<Option<AirDirection>> {
    match str_param(p, "air") {
        None => Ok(None),
        Some(t) => AirDirection::parse(t).map(Some).ok_or_else(|| bad(cmd, "`air` is supply, extract, fresh or exhaust")),
    }
}

fn direction_param(cmd: &str, p: &Value) -> Result<Option<bool>> {
    match str_param(p, "direction").map(|t| t.trim().to_ascii_lowercase()).as_deref() {
        None => Ok(None),
        Some("up" | "u" | "monte") => Ok(Some(true)),
        Some("down" | "d" | "descend") => Ok(Some(false)),
        Some(_) => Err(bad(cmd, "`direction` is up or down")),
    }
}

/// "200" or 200 (a diameter), "400x250" or [400, 250] (a rectangle); "auto", "" or null gives none.
pub(super) fn size_value(cmd: &str, v: &Value) -> Result<Option<RiserSize>> {
    let sides = |t: &str| -> Option<(f64, f64)> {
        let (a, b) = t.split_once(['x', 'X', '×', '*'])?;
        Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
    };
    let err = || bad(cmd, "a size is a diameter, \"400x250\" or \"auto\"");
    match v {
        Value::Null => Ok(None),
        Value::String(t) if t.trim().eq_ignore_ascii_case("auto") || t.trim().is_empty() => Ok(None),
        Value::String(t) => match (sides(t), t.trim().parse::<f64>().ok()) {
            (Some((w, h)), _) => Ok(Some(RiserSize::Rectangular { width_mm: w, height_mm: h })),
            (None, Some(d)) => Ok(Some(RiserSize::Round { diameter_mm: d })),
            _ => Err(err()),
        },
        Value::Number(n) => Ok(Some(RiserSize::Round { diameter_mm: n.as_f64().ok_or_else(err)? })),
        Value::Array(a) => match (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64), a.len()) {
            (Some(w), Some(h), 2) => Ok(Some(RiserSize::Rectangular { width_mm: w, height_mm: h })),
            _ => Err(err()),
        },
        _ => Err(err()),
    }
}

/// The `size` of a riser call: None when absent, else the size or none.
fn size_param(cmd: &str, p: &Value) -> Result<Option<Option<RiserSize>>> {
    p.get("size").map(|v| size_value(cmd, v)).transpose()
}

/// The `incomingSize` of a command: the section of the part that goes to or comes from the other floors (a size, or null for "sized by the flow").
fn incoming_size_param(cmd: &str, p: &Value) -> Result<Option<Option<RiserSize>>> {
    p.get("incomingSize").map(|v| size_value(cmd, v)).transpose()
}

fn risers_count(d: &Drawing, medium: RiserMedium) -> usize {
    // (the air risers, the control buses and the cables of power, lighting, data and safety are numbered apart)
    let class = |m: RiserMedium| match m {
        RiserMedium::Air => 0,
        RiserMedium::Bus => 1,
        _ => 2,
    };
    d.model.iter().filter(|e| any_riser_of(e).is_some_and(|r| class(r.cable.medium) == class(medium))).count()
}

/// The name a new riser of `medium` gets: "Riser 2", "Power riser 1", "Bus riser 1".
fn default_name(d: Option<&Drawing>, medium: RiserMedium) -> String {
    let n = d.map(|d| risers_count(d, medium)).unwrap_or(0) + 1;
    match medium {
        RiserMedium::Air => format!("Riser {n}"),
        RiserMedium::Bus => format!("Bus riser {n}"),
        _ => format!("Power riser {n}"),
    }
}

fn medium_param(cmd: &str, p: &Value) -> Result<Option<RiserMedium>> {
    match str_param(p, "medium") {
        None => Ok(None),
        Some(t) => RiserMedium::parse(t).map(Some).ok_or_else(|| bad(cmd, "`medium` is air, power, lighting, data, safety or bus")),
    }
}

fn add_riser(s: &mut Session, p: RiserParams) -> Result<Handle> {
    p.validate().map_err(doc_err)?;
    let space = s.space();
    let common = s.current_common();
    s.doc_mut()?.add_riser(&space, common, p).map_err(doc_err)
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    run_add_as(s, p, "riseradd", None)
}

fn run_add_power(s: &mut Session, p: &Value) -> Result<Value> {
    run_add_as(s, p, "elecriser", Some(RiserMedium::Power))
}

fn run_add_bus(s: &mut Session, p: &Value) -> Result<Value> {
    run_add_as(s, p, "busriser", Some(RiserMedium::Bus))
}

fn run_add_as(s: &mut Session, p: &Value, cmd: &str, default: Option<RiserMedium>) -> Result<Value> {
    let cmd: &str = cmd;
    let medium = medium_param(cmd, p)?.or(default).unwrap_or_default();
    let units = HvacUnits::of(s.doc()?);
    let outline = match (super::points_param(p, "points"), super::curves::handle_param(p, "handle"), super::point_param(p, "at")) {
        (Some(pts), ..) => pts,
        (None, Some(h), _) => super::hvac::polyline_outline(s.doc()?, h)?,
        (None, None, Some(c)) => {
            let (w, d) = (number(cmd, p, "width")?, number(cmd, p, "depth")?);
            let (Some(w), Some(d)) = (w, d) else { return Err(bad(cmd, "`width` and `depth` (mm) go with `at`")) };
            let mut r = RiserParams {
                name: String::new(),
                air: AirDirection::Extract,
                up: true,
                outline: vec![c; 4],
                incoming_flow_m3h: None,
                flow_m3h: None,
                flow_set: false,
                size: None,
                incoming_size: None,
                size_set: false,
                height_mm: None,
                label_height_mm: 100.0,
                units,
                cable: CableData::of(medium),
            };
            r.set_shaft_mm(w, d).map_err(doc_err)?;
            r.outline
        }
        _ => return Err(bad(cmd, "`points` (three or more [x, y]), `handle` (a closed polyline) or `at` + `width` + `depth` is required")),
    };
    let last = s.last_used.riser.clone();
    let name = str_param(p, "name").map(str::to_string).unwrap_or_else(|| default_name(s.doc().ok(), medium));
    let params = RiserParams {
        name,
        air: air_param(cmd, p)?.unwrap_or(last.air),
        up: direction_param(cmd, p)?.unwrap_or(last.up),
        outline,
        incoming_flow_m3h: number(cmd, p, "incomingFlow")?,
        flow_m3h: number(cmd, p, "flow")?,
        flow_set: number(cmd, p, "flow")?.is_some(),
        size: size_param(cmd, p)?.flatten(),
        incoming_size: incoming_size_param(cmd, p)?.flatten(),
        size_set: size_param(cmd, p)?.flatten().is_some(),
        height_mm: number(cmd, p, "height")?,
        label_height_mm: number(cmd, p, "labelHeight")?.unwrap_or(100.0),
        units,
        cable: cable_data(cmd, p, medium)?,
    };
    let name = params.name.clone();
    let h = add_riser(s, params)?;
    Ok(json!({ "handle": h.hex(), "name": name }))
}

/// One line of a riser of cables from a command: a circuit name, or an object {usage?, circuit?, section?, load?, incomingLoad?}.
fn cable_line_value(cmd: &str, v: &Value, default: cadcraft_doc::CableUsage) -> Result<CableLine> {
    let mut line = CableLine::new(default);
    match v {
        Value::String(c) => line.circuit = c.trim().to_string(),
        Value::Object(_) => apply_cable_line(cmd, &mut line, v)?,
        _ => return Err(bad(cmd, "a line is a circuit name or {usage?, circuit?, section?, load?, incomingLoad?}")),
    }
    Ok(line)
}

/// Changes what `v` names on a line: a number or null for the numbers.
fn apply_cable_line(cmd: &str, line: &mut CableLine, v: &Value) -> Result<()> {
    if let Some(t) = str_param(v, "usage") {
        line.usage = cadcraft_doc::CableUsage::parse(t).ok_or_else(|| bad(cmd, "`usage` is power, lighting, data, safety or bus"))?;
    }
    if let Some(c) = str_param(v, "circuit") {
        line.circuit = c.trim().to_string();
    }
    // The cable: a section, or as it is written ("3G1.5", "5G2.5", "2x0.75 blindé"); null or empty clears it.
    match v.get("section") {
        None => {}
        Some(Value::String(t)) if !t.trim().is_empty() && t.trim().parse::<f64>().is_err() => {
            let spec = cadcraft_doc::parse_cable_spec(t)
                .ok_or_else(|| bad(cmd, "`section` is a number of mm² or a cable such as 3G1.5 or 2x0.75 blindé"))?;
            (line.section_mm2, line.cores, line.earth, line.shielded) = (Some(spec.section_mm2), spec.cores, spec.earth, spec.shielded);
        }
        Some(Value::String(t)) if t.trim().is_empty() => (line.section_mm2, line.cores, line.earth, line.shielded) = (None, None, false, false),
        Some(Value::Null) => (line.section_mm2, line.cores, line.earth, line.shielded) = (None, None, false, false),
        Some(_) => {
            if let Some(n) = clearable(cmd, v, "section")? {
                // A number alone is a section: the cable has no conductors of its own.
                (line.section_mm2, line.cores, line.earth, line.shielded) = (n, None, false, false);
            }
        }
    }
    for (key, slot) in [("load", &mut line.load_kw), ("incomingLoad", &mut line.incoming_load_kw)] {
        if let Some(n) = clearable(cmd, v, key)? {
            *slot = n;
        }
    }
    // A colour of its own for the line, "#rrggbb"; null or empty gives it back the colour of its circuit's wire.
    match v.get("colour").or_else(|| v.get("color")) {
        None => {}
        Some(Value::Null) => line.colour = None,
        Some(Value::String(c)) if c.trim().is_empty() => line.colour = None,
        Some(Value::String(c)) => {
            cadcraft_doc::parse_hex_colour(c.trim()).ok_or_else(|| bad(cmd, "`colour` is written #rrggbb"))?;
            line.colour = Some(c.trim().to_ascii_lowercase());
        }
        Some(_) => return Err(bad(cmd, "`colour` is written #rrggbb")),
    }
    Ok(())
}

/// The lines of a riser of cables from a command: `lines`, else one line built from `circuit`, `section`, `load` and `incomingLoad`.
fn cable_data(cmd: &str, p: &Value, medium: RiserMedium) -> Result<CableData> {
    let mut data = CableData::of(medium);
    let Some(usage) = medium.usage() else { return Ok(data) };
    match p.get("lines") {
        Some(Value::Array(a)) => {
            if a.len() > MAX_CABLE_LINES {
                return Err(bad(cmd, format!("a riser has at most {MAX_CABLE_LINES} lines")));
            }
            data.lines = a.iter().map(|v| cable_line_value(cmd, v, usage)).collect::<Result<_>>()?;
        }
        Some(Value::Null) | None => {
            if let Some(first) = data.lines.first_mut() {
                apply_cable_line(cmd, first, p)?;
            }
        }
        Some(_) => return Err(bad(cmd, "`lines` is a list")),
    }
    Ok(data)
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "riseredit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_EDITED {
        return Err(bad(CMD, format!("select 1 to {MAX_EDITED} risers")));
    }
    let (air, up, size) = (air_param(CMD, p)?, direction_param(CMD, p)?, size_param(CMD, p)?);
    let incoming_size = incoming_size_param(CMD, p)?;
    let (incoming, flow) = (clearable(CMD, p, "incomingFlow")?, clearable(CMD, p, "flow")?);
    let height = clearable(CMD, p, "height")?;
    let medium = medium_param(CMD, p)?;
    let remove = match p.get("removeLine").filter(|v| !v.is_null()) {
        Some(v) => Some(v.as_u64().filter(|n| *n >= 1).ok_or_else(|| bad(CMD, "`removeLine` is the number of a line, from 1"))? as usize),
        None => None,
    };
    let shaft = (number(CMD, p, "width")?, number(CMD, p, "depth")?);
    let d = s.doc()?;
    let mut next = Vec::with_capacity(hs.len());
    for h in &hs {
        let mut r = d.entity(*h).and_then(|e| any_riser_of(e)).ok_or_else(|| other("Select risers only."))?;
        if let Some(n) = str_param(p, "name") {
            r.name = n.to_string();
        }
        r.air = air.unwrap_or(r.air);
        r.up = up.unwrap_or(r.up);
        edit_cable_lines(CMD, p, &mut r, medium, remove)?;
        if let Some(v) = incoming {
            r.incoming_flow_m3h = v;
        }
        // A flow typed on the riser is kept by sizing, which sizes the riser for it; null gives it back.
        if let Some(v) = flow {
            (r.flow_m3h, r.flow_set) = (v, v.is_some());
        }
        if let Some(v) = incoming_size {
            r.incoming_size = v;
        }
        if let Some(v) = height {
            r.height_mm = v;
        }
        if let Some(v) = size {
            (r.size, r.size_set) = (v, v.is_some());
        }
        match shaft {
            (None, None) => {}
            (w, d) => {
                let (cw, cd) = r.shaft_mm();
                r.set_shaft_mm(w.unwrap_or(cw), d.unwrap_or(cd)).map_err(doc_err)?;
            }
        }
        r.validate().map_err(doc_err)?;
        next.push((*h, r));
    }
    let d = s.doc_mut()?;
    for (h, r) in &next {
        d.regenerate_riser(*h, r).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

/// The lines of a riser of cables as an edit asks: `lines` (replace), `addLines`, `removeLine` (from 1), `line` ({index, …}: one changed), and
/// `circuit`, `section`, `load`, `incomingLoad` for the first line (made if there is none).
fn edit_cable_lines(cmd: &str, p: &Value, r: &mut RiserParams, medium: Option<RiserMedium>, remove: Option<usize>) -> Result<()> {
    if let Some(m) = medium {
        if r.cable.medium == RiserMedium::Air || m == RiserMedium::Air {
            r.cable = CableData::of(m);
        }
        r.cable.medium = m;
    }
    let Some(default) = r.cable.medium.usage() else { return Ok(()) };
    match p.get("lines") {
        Some(Value::Array(a)) => r.cable.lines = a.iter().map(|v| cable_line_value(cmd, v, default)).collect::<Result<_>>()?,
        Some(Value::Null) | None => {}
        Some(_) => return Err(bad(cmd, "`lines` is a list")),
    }
    if let Some(Value::Array(a)) = p.get("addLines") {
        for v in a {
            r.cable.lines.push(cable_line_value(cmd, v, default)?);
        }
    }
    if let Some(n) = remove {
        if n > r.cable.lines.len() {
            return Err(other(format!("The riser has {} line(s).", r.cable.lines.len())));
        }
        r.cable.lines.remove(n - 1);
    }
    if let Some(edit) = p.get("line").filter(|v| v.is_object()) {
        let n =
            edit.get("index").and_then(Value::as_u64).filter(|n| *n >= 1).ok_or_else(|| bad(cmd, "`line.index` is the number of a line, from 1"))?
                as usize;
        let count = r.cable.lines.len();
        let l = r.cable.lines.get_mut(n - 1).ok_or_else(|| other(format!("The riser has {count} line(s).")))?;
        apply_cable_line(cmd, l, edit)?;
    }
    // What is typed without a line is the first one's.
    if ["circuit", "section", "load", "incomingLoad"].iter().any(|k| p.get(*k).is_some()) {
        if r.cable.lines.is_empty() {
            r.cable.lines.push(CableLine::new(default));
        }
        if let Some(first) = r.cable.lines.first_mut() {
            apply_cable_line(cmd, first, p)?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    Point,
    Object,
    Name,
    Air,
    Circuit,
}

struct RiserAddM {
    pts: Vec<Vec2>,
    name: String,
    last: RiserLastUsed,
    ask: Ask,
    medium: RiserMedium,
    circuit: String,
}

impl RiserAddM {
    fn new(s: &Session) -> Self {
        Self::with_medium(s, RiserMedium::Air)
    }
    fn with_medium(s: &Session, medium: RiserMedium) -> Self {
        RiserAddM {
            pts: Vec::new(),
            name: default_name(s.doc().ok(), medium),
            last: s.last_used.riser.clone(),
            ask: Ask::Point,
            medium,
            circuit: String::new(),
        }
    }
    fn settings(&self, s: &mut Session) {
        let way = if self.last.up { "Up" } else { "Down" };
        if self.medium == RiserMedium::Air {
            s.echo(format!("Current settings: Name = {}, Air = {}, Direction = {way}", self.name, self.last.air.name()));
        } else {
            s.echo(format!("Current settings: Name = {}, Circuit = {}, Direction = {way}", self.name, self.circuit));
        }
    }
    fn params(&self, s: &Session, outline: Vec<Vec2>) -> Result<RiserParams> {
        Ok(RiserParams {
            name: self.name.clone(),
            air: self.last.air,
            up: self.last.up,
            outline,
            incoming_flow_m3h: None,
            flow_m3h: None,
            flow_set: false,
            size: None,
            incoming_size: None,
            size_set: false,
            height_mm: None,
            label_height_mm: 100.0,
            units: HvacUnits::of(s.doc()?),
            cable: {
                let mut c = CableData::of(self.medium);
                if let Some(first) = c.lines.first_mut() {
                    first.circuit = self.circuit.clone();
                }
                c
            },
        })
    }
    /// Adds the riser; the points stay when it can't be added, with the reason.
    fn finish(&mut self, s: &mut Session, outline: Vec<Vec2>) -> Result<Step> {
        let p = self.params(s, outline)?;
        if let Err(e) = p.validate() {
            s.echo(e.to_string());
            return Ok(Step::Continue);
        }
        add_riser(s, p)?;
        s.last_used.riser = self.last.clone();
        Ok(Step::Done)
    }
    fn option(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        match (self.ask, i, text.as_deref()) {
            (Ask::Object, Input::Pick(hs), _) => {
                let Some(&h) = hs.first() else { return Ok(Step::Continue) };
                return match super::hvac::polyline_outline(s.doc()?, h) {
                    Ok(outline) => self.finish(s, outline),
                    Err(e) => {
                        s.echo(e.to_string());
                        Ok(Step::Continue)
                    }
                };
            }
            (Ask::Name, _, Some(t)) if !t.is_empty() => self.name = t.to_string(),
            (Ask::Circuit, _, Some(t)) => self.circuit = t.to_string(),
            (Ask::Air, _, Some(t)) => {
                self.last.air = AirDirection::parse(t).ok_or_else(|| other("Requires supply, extract, fresh or exhaust."))?;
            }
            (Ask::Object, Input::Enter | Input::Cancel, _) => {}
            _ => return Ok(Step::Continue),
        }
        self.ask = Ask::Point;
        self.settings(s);
        Ok(Step::Continue)
    }
}

impl Interactive for RiserAddM {
    fn name(&self) -> &'static str {
        match self.medium {
            RiserMedium::Air => "RISERADD",
            RiserMedium::Bus => "BUSRISER",
            _ => "ELECRISER",
        }
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Object => return Prompt::new("Select a closed polyline", Accept::SELECT),
            Ask::Name => return Prompt::new("Enter riser name", Accept::TEXT).default(self.name.clone()),
            Ask::Circuit => {
                return Prompt::new(
                    if self.medium == RiserMedium::Bus { "Enter the bus it carries" } else { "Enter the circuit it carries" },
                    Accept::TEXT,
                )
                .default(self.circuit.clone());
            }
            Ask::Air => {
                return Prompt::new("Enter the air direction [supply/extract/fresh/exhaust]", Accept::TEXT).default(self.last.air.name().to_string());
            }
            Ask::Point => {}
        }
        let last = self.pts.last().copied();
        match self.pts.len() {
            0 if self.medium != RiserMedium::Air => {
                Prompt::new("Specify first point of the shaft", Accept::POINT).kw(&["OBject", "Name", "Circuit", "Up", "Down"])
            }
            0 => Prompt::new("Specify first point of the shaft", Accept::POINT).kw(&["OBject", "Name", "Air", "Up", "Down"]),
            1 | 2 => Prompt::new("Specify next point", Accept::POINT).kw(&["Undo"]).base_opt(last),
            _ => Prompt::new("Specify next point", Accept::POINT).kw(&["Close", "Undo"]).base_opt(last),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.ask != Ask::Point {
            return self.option(s, i);
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "OBject" => self.ask = Ask::Object,
                    "Name" => self.ask = Ask::Name,
                    "Air" => self.ask = Ask::Air,
                    "Circuit" => self.ask = Ask::Circuit,
                    "Up" | "Down" => {
                        self.last.up = k == "Up";
                        self.settings(s);
                    }
                    "Undo" => {
                        self.pts.pop();
                    }
                    "Close" if self.pts.len() >= 3 => return self.finish(s, self.pts.clone()),
                    _ => {}
                }
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                if self.pts.len() < MAX_RISER_VERTICES && !self.pts.last().is_some_and(|l| l.near(p, 1e-9)) {
                    self.pts.push(p);
                }
                Ok(Step::Continue)
            }
            Input::Enter if self.pts.len() >= 3 => self.finish(s, self.pts.clone()),
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<EntityKind> {
        let mut pts = self.pts.clone();
        if pts.is_empty() || self.ask != Ask::Point {
            return Vec::new();
        }
        pts.push(c);
        if let Some(first) = self.pts.first() {
            pts.push(*first);
        }
        pts.windows(2).filter_map(|w| Some(EntityKind::Line(cadcraft_doc::Line { a: w.first()?.to3(0.0), b: w.get(1)?.to3(0.0) }))).collect()
    }
}
