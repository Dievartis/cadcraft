//! Heating and cooling equipment seen from above: EQUIPMENTADD places a chiller, an outdoor unit or an
//! indoor unit from a library of models (by name, or by power: the smallest that gives it), each value
//! of which can be given instead; EQUIPMENTEDIT changes one already placed, or gives it another model.

use cadcraft_doc::{
    Distribution, Drawing, EquipmentKind, EquipmentModel, EquipmentParams, MAX_DISTRIBUTED, distribute_in_polygon, equipment_model_for,
    equipment_model_named, equipment_models_of, equipment_of,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("equipmentadd", "Equipment", |s, p| super::catalogue_helpers::atomic(s, |s| run_add(s, p)))
            .menu(&["Draw", "HVAC", "Equipment"])
            .params(
                "{at, kind? (chiller|vrfOutdoor|splitOutdoor|cassette|ducted|wallUnit|ceilingUnit|floorUnit), model? (a library model's name) | capacity? (kW: the \\
                 smallest model that gives it), name?, reference?, capacityKw?, heatingKw?, length? width? height? (mm), weight? (kg), fans?, clearance? (mm), \\
                 connections?, inlets? / outlets? (a ducted unit: the sizes of its duct connections), rotation? (degrees), labelHeight? (mm)} → {handle, name, model} | {list: true, kind?} → {models} | \
                 {space (a space's handle) + count? | rows? + cols?, + the same as above, for an indoor unit}: the units are spread evenly in the space (a grid of the \
                 outline's extents, the cells outside it left empty) → {handles, model}",
            )
            .interactive(|s| Ok(Box::new(EquipAddM::new(s)))),
        CommandSpec::new("equipmentmodelsave", "Save Equipment Model", run_model_save).params(
            "{handle (an equipment), name} → {models}: keeps the values of the equipment (its kind, power, sizes, weight, fans, reference and connections) as a model of the drawing, \
             which EQUIPMENTADD and EQUIPMENTEDIT take by that name like the library's; one of the same name is replaced; kept in the DXF and the DWG",
        ),
        CommandSpec::new("equipmentmodeldelete", "Delete Equipment Model", run_model_delete)
            .params("{name} → {deleted}: takes a model of the drawing's own out of it (the library's stay)"),
        CommandSpec::new("equipmentsizedisplay", "Show Equipment Sizes", |s, p| super::catalogue_helpers::atomic(s, |s| run_size_display(s, p)))
            .menu(&["Draw", "HVAC", "Show Equipment Sizes"])
            .params("{show? (true: write the size beside every equipment, the default; false: hide them)} → {changed}"),
        CommandSpec::new("equipmentedit", "Edit Equipment", |s, p| super::catalogue_helpers::atomic(s, |s| run_edit(s, p))).params(
            "{handle | handles, model? (take that library model's values), kind?, name?, reference?, capacityKw? or null, heatingKw? or null, electricKw? (the electrical power it draws, or null), circuit? (the circuit that feeds it), address? (its address, written beside it), length? width? height? (mm), \\
             weight? or null, altitude? (m: the underside, or null), refrigerant? (\"R410A\") and charge? (kg of the system, or null: checked by REFRIGERANTCHARGE), showSize? (false: the size is not written beside it), fans?, clearance? (mm), connections? (text written beside it), inlets? / outlets? (a ducted unit: the sizes of the duct connections the air comes in by, along its back long side, and goes out by, along its front long side: a list or text, \"315, 400x250\"; spread along the side), inletCount? / outletCount? (how many connections, 0 to 16: each takes the size of the last one, Ø200 when there is none; a duct that ends on a side gives the unit its connection, as wide as the duct, by itself)} → {changed}",
        ),
    ]
}

/// What EQUIPMENTADD remembers between uses: the kind and the model.
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentLastUsed {
    pub kind: EquipmentKind,
    pub model: String,
}

impl Default for EquipmentLastUsed {
    fn default() -> Self {
        EquipmentLastUsed { kind: EquipmentKind::Chiller, model: String::new() }
    }
}

/// Most equipment one EQUIPMENTEDIT changes.
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

/// A number, or null to clear it; None when the key is absent.
fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(_) => Ok(Some(number(cmd, p, key)?)),
    }
}

fn kind_param(cmd: &str, p: &Value) -> Result<Option<EquipmentKind>> {
    match str_param(p, "kind") {
        None => Ok(None),
        Some(t) => EquipmentKind::parse(t).map(Some).ok_or_else(|| bad(cmd, "unknown `kind`")),
    }
}

fn count(d: &Drawing) -> usize {
    d.model.iter().filter(|e| equipment_of(e).is_some()).count()
}

/// The name a new equipment of `kind` takes: a short code and the next number.
fn next_name(d: &Drawing, kind: EquipmentKind) -> String {
    let code = match kind {
        EquipmentKind::Chiller => "GE",
        EquipmentKind::VrfOutdoor | EquipmentKind::SplitOutdoor => "UE",
        EquipmentKind::ServerCooler => "CRAC",
        EquipmentKind::Dehumidifier | EquipmentKind::DehumidifierWall => "DSH",
        EquipmentKind::WallRack | EquipmentKind::FloorRack => "BAIE",
        _ => "UI",
    };
    format!("{code}{}", count(d) + 1)
}

/// Everything about the equipment a call asks for, over the model it names (or the one its power asks for).
fn params_from(s: &Session, p: &Value, cmd: &str, last: &EquipmentLastUsed) -> Result<EquipmentParams> {
    let d = s.doc()?;
    let models = d.available_equipment_models();
    let kind = kind_param(cmd, p)?;
    let by_name =
        str_param(p, "model").map(|n| equipment_model_named(&models, n).ok_or_else(|| bad(cmd, format!("unknown model \"{n}\"")))).transpose()?;
    let kind_of = by_name.as_ref().map(|m| m.kind).or(kind).unwrap_or(last.kind);
    let model: Option<EquipmentModel> = match (by_name, number(cmd, p, "capacity")?) {
        (Some(m), _) => Some(m),
        (None, Some(kw)) => equipment_model_for(&models, kind_of, kw),
        (None, None) if !last.model.is_empty() && kind.is_none() => equipment_model_named(&models, &last.model),
        (None, None) => equipment_models_of(&models, kind_of).into_iter().next(),
    };
    let units = super::catalogue_helpers::units(s)?;
    let name = str_param(p, "name").map(str::to_string).unwrap_or_else(|| next_name(d, kind_of));
    let mut e = match &model {
        Some(m) => EquipmentParams::from_model(m, &name, units),
        None => return Err(bad(cmd, "no model for that kind")),
    };
    if let Some(r) = str_param(p, "reference") {
        e.reference = r.to_string();
    }
    if let Some(a) = str_param(p, "address") {
        e.address = a.trim().to_string();
    }
    if let Some(v) = p.get("showSize").filter(|v| !v.is_null()) {
        e.show_size = v.as_bool().ok_or_else(|| bad(cmd, "`showSize` must be true or false"))?;
    }
    for (key, slot) in [("length", &mut e.length_mm), ("width", &mut e.width_mm), ("height", &mut e.height_mm)] {
        if let Some(v) = number(cmd, p, key)? {
            *slot = v;
        }
    }
    if let Some(v) = number(cmd, p, "capacityKw")? {
        e.capacity_kw = Some(v);
    }
    if let Some(v) = number(cmd, p, "heatingKw")? {
        e.heating_kw = Some(v);
    }
    if let Some(v) = number(cmd, p, "weight")? {
        e.weight_kg = Some(v);
    }
    if let Some(v) = number(cmd, p, "fans")? {
        e.fans = u32::try_from(v as i64).map_err(|_| bad(cmd, "`fans` must be a whole number"))?;
    }
    if let Some(v) = number(cmd, p, "clearance")? {
        e.clearance_mm = v;
    }
    if let Some(c) = str_param(p, "connections") {
        e.connections = c.to_string();
    }
    if let Some(r) = str_param(p, "refrigerant") {
        e.refrigerant = r.trim().to_string();
    }
    if let Some(c) = clearable(cmd, p, "charge")? {
        e.charge_kg = c;
    }
    if let Some(v) = number(cmd, p, "labelHeight")? {
        e.label_height_mm = v;
    }
    apply_air_ports(cmd, p, &mut e)?;
    if let Some(v) = super::catalogue_helpers::altitude_param(cmd, p)? {
        e.bottom_mm = v;
    }
    e.validate().map_err(doc_err)?;
    Ok(e)
}

/// The sizes of a list of duct connections from a call: a list, or text "315, 400x250" (a diameter or width x height, in mm).
fn sizes_param(cmd: &str, key: &str, v: &Value) -> Result<Vec<cadcraft_doc::RiserSize>> {
    let items: Vec<Value> = match v {
        Value::Array(a) => a.clone(),
        Value::String(t) => t.split([',', ';']).map(str::trim).filter(|x| !x.is_empty()).map(|x| Value::String(x.to_string())).collect(),
        Value::Null => Vec::new(),
        _ => return Err(bad(cmd, format!("`{key}` is a list of sizes"))),
    };
    if items.len() > cadcraft_doc::MAX_AIR_PORTS {
        return Err(bad(cmd, format!("a unit has at most {} duct connections", cadcraft_doc::MAX_AIR_PORTS)));
    }
    items
        .iter()
        .map(|i| {
            super::catalogue_helpers::size_value(cmd, i)?.ok_or_else(|| bad(cmd, format!("`{key}` needs a size for each connection: 315 or 400x250")))
        })
        .collect()
}

/// `inlets` (the connections the air comes in by, along the back long side of a ducted unit) and `outlets` (the ones it goes out by, along the front long
/// side): the sizes, one for each connection; the connections are spread along their side.
fn apply_air_ports(cmd: &str, p: &Value, e: &mut EquipmentParams) -> Result<()> {
    let keys = ["inlets", "outlets", "inletCount", "outletCount"];
    if keys.iter().all(|k| p.get(*k).is_none()) {
        return Ok(());
    }
    if !e.kind.takes_ducts() {
        return Err(bad(cmd, "`inlets`, `outlets`, `inletCount` and `outletCount` are for a ducted unit"));
    }
    let sizes_of = |key: &str, now: Vec<cadcraft_doc::RiserSize>| p.get(key).map(|v| sizes_param(cmd, key, v)).transpose().map(|o| o.unwrap_or(now));
    let (mut ins, mut outs) = (sizes_of("inlets", e.air_sizes(true))?, sizes_of("outlets", e.air_sizes(false))?);
    // How many: each connection takes the size of the last one named (Ø200 when none was), so that "3 outlets" of "160" are three of Ø160.
    for (key, sizes) in [("inletCount", &mut ins), ("outletCount", &mut outs)] {
        let Some(v) = p.get(key).filter(|v| !v.is_null()) else { continue };
        // A whole number, written 3 or 3.0 (a field of the interface sends it either way).
        let n = v
            .as_u64()
            .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0).map(|f| f as u64))
            .filter(|n| *n as usize <= cadcraft_doc::MAX_AIR_PORTS)
            .ok_or_else(|| bad(cmd, format!("`{key}` is a whole number from 0 to {}", cadcraft_doc::MAX_AIR_PORTS)))? as usize;
        let fill = sizes.last().copied().unwrap_or(cadcraft_doc::RiserSize::Round { diameter_mm: 200.0 });
        sizes.resize(n, fill);
    }
    e.set_air_ports(&ins, &outs);
    Ok(())
}

/// Shows or hides the size written beside every equipment of the drawing.
fn run_size_display(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "equipmentsizedisplay";
    let show = match p.get("show").filter(|v| !v.is_null()) {
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "`show` must be true or false"))?,
        None => true,
    };
    let changed = s.doc_mut()?.set_equipment_size_display(show).map_err(doc_err)?;
    Ok(json!({ "changed": changed }))
}

fn models_json(d: &Drawing, kind: Option<EquipmentKind>) -> Vec<Value> {
    let models = d.available_equipment_models();
    let own = |name: &str| d.equipment_models.iter().any(|m| m.name.eq_ignore_ascii_case(name));
    EquipmentKind::ALL
        .iter()
        .filter(|k| kind.is_none_or(|w| w == **k))
        .flat_map(|k| equipment_models_of(&models, *k))
        .map(|m| {
            let mut v = serde_json::to_value(&m).unwrap_or(Value::Null);
            v["own"] = json!(own(&m.name));
            v
        })
        .collect()
}

/// Keeps the values of an equipment as a model of the drawing, under a name: the next equipment can be given them by that name.
fn run_model_save(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "equipmentmodelsave";
    let h = super::curves::handle_param(p, "handle").ok_or_else(|| bad(CMD, "`handle` (an equipment) is required"))?;
    let name = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| bad(CMD, "`name` is required"))?;
    let e = s.doc()?.entity(h).and_then(|e| equipment_of(e)).ok_or_else(|| other("Select an equipment."))?;
    s.doc_mut()?.save_equipment_model(e.to_model(name)).map_err(doc_err)?;
    Ok(json!({ "models": s.doc()?.equipment_models.len() }))
}

/// Takes a model of the drawing's own out of it.
fn run_model_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "equipmentmodeldelete";
    let name = str_param(p, "name").ok_or_else(|| bad(CMD, "`name` is required"))?;
    Ok(json!({ "deleted": s.doc_mut()?.delete_equipment_model(name) }))
}

fn place(s: &mut Session, e: &EquipmentParams, at: Vec2, rotation: f64) -> Result<cadcraft_doc::Handle> {
    let space = s.space();
    let common = s.current_common();
    let h = s.doc_mut()?.add_equipment(&space, common, e, at, rotation).map_err(doc_err)?;
    s.last_used.equipment = EquipmentLastUsed { kind: e.kind, model: e.model.clone() };
    Ok(h)
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "equipmentadd";
    if p.get("list").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(json!({ "models": models_json(s.doc()?, kind_param(CMD, p)?) }));
    }
    let last = s.last_used.equipment.clone();
    let rotation = super::f64_or(p, "rotation", 0.0).to_radians();
    if let Some(space) = super::curves::handle_param(p, "space") {
        let count = |k: &str| number(CMD, p, k).map(|n| n.map_or(0, |v| v.round().max(0.0) as usize));
        let how = match (count("rows")?, count("cols")?, count("count")?) {
            (r, c, _) if r > 0 && c > 0 => Distribution::Grid { rows: r, cols: c },
            (_, _, n) => Distribution::Count(n.max(1)),
        };
        let (handles, model) = spread(s, space, how, p, &last, rotation)?;
        return Ok(json!({ "handles": handles.iter().map(|h| h.hex()).collect::<Vec<_>>(), "model": model }));
    }
    let at = super::point_req(CMD, p, "at")?;
    let e = params_from(s, p, CMD, &last)?;
    let h = place(s, &e, at, rotation)?;
    Ok(json!({ "handle": h.hex(), "name": e.name, "model": e.model }))
}

/// Puts equipment of the kind and model the call (or the last use) names at each point spread over space `space`: one each, under the next names.
fn spread(
    s: &mut Session,
    space: cadcraft_doc::Handle,
    how: Distribution,
    p: &Value,
    last: &EquipmentLastUsed,
    rotation: f64,
) -> Result<(Vec<cadcraft_doc::Handle>, String)> {
    const CMD: &str = "equipmentadd";
    let outline = s.doc()?.space_outline(space).ok_or_else(|| other("Select a space."))?.1;
    let points = distribute_in_polygon(&outline, how).map_err(doc_err)?;
    let first = params_from(s, p, CMD, last)?;
    if matches!(first.kind, EquipmentKind::Chiller | EquipmentKind::VrfOutdoor | EquipmentKind::SplitOutdoor) {
        return Err(bad(CMD, "only indoor units are spread in a space (cassette, ducted, wallUnit, ceilingUnit, floorUnit)"));
    }
    let mut handles = Vec::with_capacity(points.len());
    for (i, at) in points.iter().enumerate() {
        // Each unit under its own name: the one given with a number after it, else the next free one.
        let mut each = p.clone();
        if let (Some(n), true) = (str_param(p, "name"), points.len() > 1) {
            each["name"] = json!(format!("{n}{}", i + 1));
        }
        let e = params_from(s, &each, CMD, last)?;
        handles.push(place(s, &e, *at, rotation)?);
    }
    Ok((handles, first.model))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "equipmentedit";
    let hs = super::catalogue_helpers::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_EDITED {
        return Err(bad(CMD, format!("select 1 to {MAX_EDITED} equipment")));
    }
    let models = s.doc()?.available_equipment_models();
    let model =
        str_param(p, "model").map(|n| equipment_model_named(&models, n).ok_or_else(|| bad(CMD, format!("unknown model \"{n}\"")))).transpose()?;
    let kind = kind_param(CMD, p)?;
    let (cap, heat, weight) = (clearable(CMD, p, "capacityKw")?, clearable(CMD, p, "heatingKw")?, clearable(CMD, p, "weight")?);
    let electric = clearable(CMD, p, "electricKw")?;
    let charge = clearable(CMD, p, "charge")?;
    let sizes = [number(CMD, p, "length")?, number(CMD, p, "width")?, number(CMD, p, "height")?];
    let (fans, clearance) = (number(CMD, p, "fans")?, number(CMD, p, "clearance")?);
    let d = s.doc()?;
    let mut next = Vec::with_capacity(hs.len());
    for h in &hs {
        let mut e = d.entity(*h).and_then(|x| equipment_of(x)).ok_or_else(|| other("Select equipment only."))?;
        if let Some(m) = &model {
            // The model's values, the name and the units kept.
            e = EquipmentParams {
                name: e.name.clone(),
                label_height_mm: e.label_height_mm,
                bottom_mm: e.bottom_mm,
                ..EquipmentParams::from_model(m, &e.name, e.units)
            };
        }
        e.kind = kind.unwrap_or(e.kind);
        if let Some(n) = str_param(p, "name") {
            e.name = n.to_string();
        }
        if let Some(r) = str_param(p, "reference") {
            e.reference = r.to_string();
        }
        if let Some(c) = str_param(p, "connections") {
            e.connections = c.to_string();
        }
        if let Some(r) = str_param(p, "refrigerant") {
            e.refrigerant = r.trim().to_string();
        }
        if let Some(c) = charge {
            e.charge_kg = c;
        }
        if let Some(v) = cap {
            e.capacity_kw = v;
        }
        if let Some(v) = heat {
            e.heating_kw = v;
        }
        if let Some(v) = weight {
            e.weight_kg = v;
        }
        if let Some(v) = electric {
            e.electric_kw = v;
        }
        if let Some(c) = str_param(p, "circuit") {
            e.circuit = c.trim().to_string();
        }
        if let Some(a) = str_param(p, "address") {
            e.address = a.trim().to_string();
        }
        if let Some(v) = p.get("showSize").filter(|v| !v.is_null()) {
            e.show_size = v.as_bool().ok_or_else(|| bad(CMD, "`showSize` must be true or false"))?;
        }
        for (v, slot) in sizes.iter().zip([&mut e.length_mm, &mut e.width_mm, &mut e.height_mm]) {
            if let Some(v) = v {
                *slot = *v;
            }
        }
        if let Some(v) = fans {
            e.fans = u32::try_from(v as i64).map_err(|_| bad(CMD, "`fans` must be a whole number"))?;
        }
        if let Some(v) = clearance {
            e.clearance_mm = v;
        }
        apply_air_ports(CMD, p, &mut e)?;
        if let Some(v) = super::catalogue_helpers::altitude_param(CMD, p)? {
            e.bottom_mm = v;
        }
        if let Some(height) = number(CMD, p, "labelHeight")? {
            e.label_height_mm = height;
        }
        e.validate().map_err(doc_err)?;
        next.push((*h, e));
    }
    let d = s.doc_mut()?;
    for (h, e) in &next {
        d.regenerate_equipment(*h, e).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    Location,
    Type,
    Size,
    Name,
    Space,
    Count(cadcraft_doc::Handle),
    /// The unit is put at this point and waits for its direction (a second click or an angle).
    Aim(Vec2),
}

struct EquipAddM {
    last: EquipmentLastUsed,
    name: Option<String>,
    ask: Ask,
    /// The turn of the last unit put, in degrees: Enter at the direction prompt keeps it.
    rotation_deg: f64,
    /// The point of the unit the cursor holds (Tab goes round them).
    anchor: super::catalogue_helpers::Anchor,
}

impl EquipAddM {
    fn new(s: &Session) -> Self {
        EquipAddM {
            last: s.last_used.equipment.clone(),
            name: None,
            ask: Ask::Location,
            rotation_deg: 0.0,
            anchor: super::catalogue_helpers::Anchor::CENTRE,
        }
    }
    fn params(&self, s: &Session) -> Result<EquipmentParams> {
        let mut p = json!({ "kind": self.last.kind.name() });
        if !self.last.model.is_empty() {
            p["model"] = json!(self.last.model);
        }
        if let Some(n) = &self.name {
            p["name"] = json!(n);
        }
        params_from(s, &p, "equipmentadd", &self.last)
    }
    /// Where the unit's origin (its centre) is when the cursor holds its anchor at `at`.
    fn origin(&self, e: &EquipmentParams, at: Vec2, rotation: f64) -> Vec2 {
        self.anchor.origin_for(at, e.units, e.length_mm, e.width_mm, false, rotation)
    }
    fn settings(&self, s: &mut Session) {
        match self.params(s) {
            Ok(e) => s.echo(format!(
                "Current settings: Type = {}, Model = {} ({} x {} x {} mm), Name = {}",
                e.kind.name(),
                e.model,
                e.length_mm,
                e.width_mm,
                e.height_mm,
                e.name
            )),
            Err(e) => s.echo(e.to_string()),
        }
    }
    fn option(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        match (self.ask, text.as_deref()) {
            (Ask::Type, Some(t)) => {
                self.last = EquipmentLastUsed {
                    kind: EquipmentKind::parse(t).ok_or_else(|| other("Enter Chiller, Vrv, Split, Cassette, Ducted, Wall, Ceiling, Floor, ServerCooler, Dehumidifier, DehumidifierWall, WallRack or FloorRack."))?,
                    model: String::new(),
                };
            }
            // A model's name, or a power in kW: the smallest model of the kind that gives it.
            (Ask::Size, Some(t)) if !t.is_empty() => {
                let models = s.doc()?.available_equipment_models();
                let m = match t.parse::<f64>() {
                    Ok(kw) => equipment_model_for(&models, self.last.kind, kw),
                    Err(_) => equipment_model_named(&models, t),
                };
                let m = m.ok_or_else(|| other("Enter a model's name, or a power in kW (? lists the models)."))?;
                self.last = EquipmentLastUsed { kind: m.kind, model: m.name };
            }
            (Ask::Name, Some(t)) if !t.is_empty() => self.name = Some(t.to_string()),
            (Ask::Space, _) => {
                if let Input::Pick(hs) = &i {
                    let d = s.doc()?;
                    match hs.iter().find(|h| d.space_outline(**h).is_some()) {
                        Some(&h) => self.ask = Ask::Count(h),
                        None => s.echo("Select a space."),
                    }
                }
                return Ok(Step::Continue);
            }
            (Ask::Count(h), t) => return self.distribute(s, h, t.filter(|x| !x.is_empty()).unwrap_or("1")),
            _ => return Ok(Step::Continue),
        }
        self.ask = Ask::Location;
        self.settings(s);
        Ok(Step::Continue)
    }
}

impl EquipAddM {
    /// Spreads the units over space `h`: `t` is a count, or rows × columns ("2x3").
    fn distribute(&mut self, s: &mut Session, h: cadcraft_doc::Handle, t: &str) -> Result<Step> {
        let parse = |x: &str| x.trim().parse::<usize>().ok().filter(|n| (1..=MAX_DISTRIBUTED).contains(n));
        let how = match t.split_once(['x', 'X', '×', '*']) {
            Some((r, c)) => Distribution::Grid {
                rows: parse(r).ok_or_else(|| other("Enter rows x columns, as 2x3."))?,
                cols: parse(c).ok_or_else(|| other("Enter rows x columns, as 2x3."))?,
            },
            None => Distribution::Count(parse(t).ok_or_else(|| other(format!("Enter 1 to {MAX_DISTRIBUTED} units, or rows x columns.")))?),
        };
        let mut p = json!({ "kind": self.last.kind.name() });
        if !self.last.model.is_empty() {
            p["model"] = json!(self.last.model);
        }
        match spread(s, h, how, &p, &self.last, 0.0) {
            Ok((hs, _)) => s.echo(format!("{} unit(s) placed.", hs.len())),
            Err(e) => s.echo(e.to_string()),
        }
        Ok(Step::Done)
    }
}

impl Interactive for EquipAddM {
    fn name(&self) -> &'static str {
        "EQUIPMENTADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn cycle_alignment(&mut self, _s: &mut Session) -> Option<String> {
        matches!(self.ask, Ask::Location | Ask::Aim(_)).then(|| {
            self.anchor = self.anchor.next();
            format!("Insertion point = {}", self.anchor.name())
        })
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Type => Prompt::new("Enter equipment type", super::curves::KW)
                .kw(&[
                    "Chiller",
                    "Vrv",
                    "Split",
                    "Cassette",
                    "Ducted",
                    "Wall",
                    "Ceiling",
                    "Floor",
                    "ServerCooler",
                    "Dehumidifier",
                    "WallRack",
                    "FloorRack",
                ])
                .default(self.last.kind.name()),
            Ask::Size => Prompt::new("Enter a model name or a power (kW)", Accept::TEXT).default(self.last.model.clone()),
            Ask::Name => Prompt::new("Enter equipment name", Accept::TEXT),
            Ask::Location => Prompt::new("Specify insertion point", Accept::POINT).kw(&["Type", "Size", "Name", "Distribute"]),
            Ask::Space => Prompt::new("Select space", Accept::SELECT),
            Ask::Count(_) => Prompt::new("Enter number of units or rows x columns", Accept::TEXT).default("1"),
            Ask::Aim(at) => Prompt::new("Specify rotation angle", Accept::POINT_OR_NUMBER).base(at).default(format!("{}", self.rotation_deg)),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Ask::Aim(at) = self.ask {
            if matches!(i, Input::Cancel) {
                self.ask = Ask::Location;
                return Ok(Step::Continue);
            }
            if let Some(a) = super::catalogue_helpers::aimed_degrees(&i, at)? {
                self.rotation_deg = a;
            }
            let e = self.params(s)?;
            let turn = self.rotation_deg.to_radians();
            place(s, &e, self.origin(&e, at, turn), turn)?;
            self.name = None;
            self.ask = Ask::Location;
            return Ok(Step::Continue);
        }
        if self.ask != Ask::Location {
            return self.option(s, i);
        }
        match i {
            Input::Keyword(k) => {
                self.ask = match k.as_str() {
                    "Type" => Ask::Type,
                    "Size" => Ask::Size,
                    "Name" => Ask::Name,
                    "Distribute" => Ask::Space,
                    _ => Ask::Location,
                };
                Ok(Step::Continue)
            }
            // Equipment keeps being placed, each under the next name, until Enter or Esc.
            Input::Point(at) => {
                self.ask = Ask::Aim(at);
                Ok(Step::Continue)
            }
            Input::Enter | Input::Cancel => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
    fn preview_objects(&self, s: &Session, c: Vec2) -> Vec<cadcraft_doc::Entity> {
        let (at, deg) = match self.ask {
            Ask::Location => (c, self.rotation_deg),
            Ask::Aim(at) if !c.near(at, 1e-9) => (at, (c - at).angle().to_degrees()),
            Ask::Aim(at) => (at, self.rotation_deg),
            _ => return Vec::new(),
        };
        self.params(s).map(|e| e.preview(self.origin(&e, at, deg.to_radians()), deg.to_radians())).unwrap_or_default()
    }
}
