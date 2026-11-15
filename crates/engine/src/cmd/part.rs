//! Pipework parts: PIPEPARTADD puts a valve, a pump, a vessel, a refrigerant branch or a branch-selector box on the
//! drawing (turned along the pipe it is put on), PIPEPARTEDIT changes parts already drawn, PIPEPARTMODELS lists the library.

use cadcraft_doc::{
    Drawing, EntityKind, Handle, HvacUnits, PartCategory, PartModel, PartParams, part_of, standard_part_model_named, standard_part_models,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most parts one PIPEPARTEDIT changes.
const MAX_EDITED: usize = 10_000;
/// How far from a pipe's axis a click still means that pipe, mm.
const REACH_MM: f64 = 150.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("pipepartadd", "Pipework Part", run_add)
            .menu(&["Draw", "HVAC", "Pipework Part"])
            .params(
                "{at ([x, y]), model (a name of the library, in English or French; see pipepartmodels) | kind?, name? (the tag; default V1, P2…), rotation? (degrees; default along the pipe it is put on), \
                 dn? (mm), outlets? (a header or a box: its length follows), volume? (l), power? (kW), height? | length? | width? (mm: the part's H, and L along the pipe and P across), connections?, labelHeight? (mm)} → {handle, name, model}",
            )
            .interactive(|s| Ok(Box::new(PartAddM::new(s)))),
        CommandSpec::new("pipepartedit", "Edit Pipework Part", run_edit).params(
            "{handle | handles, name?, model?, brand?, dn? (mm or null), outlets?, volume? (l or null), power? (kW or null), flow? (m³/h or null), head? (kPa or null), \
             weight? (kg or null), loss? (kPa at the design flow, or null) | kvs? (m³/h for 1 bar) | equivalentLength? (m of pipe) | zeta? (loss coefficient): the pressure the part \
             loses in a network, the first given in that order, else the kind's indicative one; connections?, height? (mm or null), length? | width? (mm: L along the pipe, P across); model? a model of the library: the part takes its size, outlets and connections (its tag and its pipe size stay)} → {changed}",
        ),
        CommandSpec::new("pipepartmodels", "Pipework Part Library", run_models).params(
            "{category? (valves|fittings|airAndDirt|instruments|pumps|vesselsAndTanks|heatSources|refrigerantBranches|branchBoxes), q? (words found in the name)} → \
             {categories: [{name, title}], models: [{name, nameFr, category, kind, brand, outlets, connections, dn, volume, power}]}",
        ),
    ]
}

/// What PIPEPARTADD remembers between uses: the model chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct PartLastUsed {
    pub model: String,
}

impl Default for PartLastUsed {
    fn default() -> Self {
        PartLastUsed { model: "Isolation valve (ball)".into() }
    }
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite() && *x > 0.0).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a number above 0"))),
    }
}

/// A value that a number sets and null clears: absent keeps.
fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(_) => Ok(Some(number(cmd, p, key)?)),
    }
}

/// A whole number from a command: `6` or `6.0` (a palette field sends the second).
fn whole(cmd: &str, p: &Value, key: &str) -> Result<Option<u32>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(f)).map(|f| f as u64))
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| bad(cmd, format!("`{key}` must be a whole number"))),
    }
}

/// The tag a new part of `model` takes: its prefix and the number after the parts of that prefix already drawn.
fn next_tag(d: &Drawing, model: &PartModel) -> String {
    let prefix = model.tag_prefix();
    let n = d.model.iter().filter_map(|e| part_of(e)).filter(|p| p.name.starts_with(prefix)).count() + 1;
    format!("{prefix}{n}")
}

/// The pipe nearest `at` within reach: its handle, the point on its axis, its direction (radians) and how far along it the point is.
fn pipe_under(d: &Drawing, at: Vec2) -> Option<(Handle, Vec2, f64, f64)> {
    let mut best: Option<(f64, Handle, Vec2, f64, f64)> = None;
    for e in d.model.iter() {
        let Some((p, a, b)) = d.pipe_ends(e.handle) else { continue };
        let reach = p.units.to_units(REACH_MM.max(p.plan_width_mm() / 2.0));
        let len = a.dist(b);
        if len <= 1e-9 {
            continue;
        }
        let dir = (b - a) / len;
        let t = (at - a).dot(dir).clamp(0.0, len);
        let foot = a + dir * t;
        let off = foot.dist(at);
        if off <= reach && best.as_ref().is_none_or(|x| off < x.0) {
            best = Some((off, e.handle, foot, dir.angle(), t));
        }
    }
    best.map(|(_, h, foot, angle, t)| (h, foot, angle, t))
}

/// Puts a part made from `model` at `at` (on the pipe under it, turned along it, unless `rotation` is given).
fn place(
    s: &mut Session,
    model: &PartModel,
    name: Option<String>,
    at: Vec2,
    rotation: Option<f64>,
    tweak: impl FnOnce(&mut PartParams),
) -> Result<Handle> {
    let d = s.doc()?;
    let name = name.unwrap_or_else(|| next_tag(d, model));
    let mut p = model.params(&name, HvacUnits::of(d));
    tweak(&mut p);
    let (at, angle, on) = match (rotation, pipe_under(d, at)) {
        (Some(r), _) => (at, r, None),
        (None, Some((pipe, foot, a, t))) => (foot, a, Some((pipe, t))),
        (None, None) => (at, 0.0, None),
    };
    p.validate().map_err(doc_err)?;
    let (inline, gap) = (p.kind.is_inline(), p.units.to_units(p.length_mm));
    let space = s.space();
    let common = s.current_common();
    let h = s.doc_mut()?.add_part(&space, common, &p, at, angle).map_err(doc_err)?;
    // A part in the line of a pipe cuts it: the pipe goes up to the part and on from it (when the part fits inside the pipe).
    if let (true, Some((pipe, t))) = (inline, on) {
        s.doc_mut()?.split_pipe(pipe, t, gap).map_err(doc_err)?;
    }
    Ok(h)
}

fn model_param(cmd: &str, p: &Value) -> Result<PartModel> {
    if let Some(n) = str_param(p, "model") {
        return standard_part_model_named(n).ok_or_else(|| bad(cmd, format!("no model `{n}` in the library (see pipepartmodels)")));
    }
    if let Some(k) = str_param(p, "kind") {
        let kind = cadcraft_doc::PartKind::parse(k).ok_or_else(|| bad(cmd, "unknown `kind`"))?;
        return standard_part_models().into_iter().find(|m| m.kind == kind).ok_or_else(|| bad(cmd, "no model of that kind"));
    }
    Err(bad(cmd, "`model` (a name of the library) or `kind` is required"))
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "pipepartadd";
    let at = super::point_param(p, "at").ok_or_else(|| bad(CMD, "`at` ([x, y]) is required"))?;
    let model = model_param(CMD, p)?;
    let rotation = p
        .get("rotation")
        .filter(|v| !v.is_null())
        .map(|v| v.as_f64().filter(|r| r.is_finite()).map(f64::to_radians))
        .map(|r| r.ok_or_else(|| bad(CMD, "`rotation` must be a number of degrees")))
        .transpose()?;
    let (dn, volume, power) = (number(CMD, p, "dn")?, number(CMD, p, "volume")?, number(CMD, p, "power")?);
    let (length, width, label) = (number(CMD, p, "length")?, number(CMD, p, "width")?, number(CMD, p, "labelHeight")?);
    let outlets = whole(CMD, p, "outlets")?;
    let height = number(CMD, p, "height")?;
    let connections = str_param(p, "connections").map(str::to_string);
    let (loss, kvs) = (number(CMD, p, "loss")?, number(CMD, p, "kvs")?);
    let h = place(s, &model, str_param(p, "name").map(str::to_string), at, rotation, |part| {
        part.dn_mm = dn.or(part.dn_mm);
        part.volume_l = volume.or(part.volume_l);
        part.power_kw = power.or(part.power_kw);
        part.length_mm = length.unwrap_or(part.length_mm);
        part.width_mm = width.unwrap_or(part.width_mm);
        part.label_height_mm = label.unwrap_or(part.label_height_mm);
        part.height_mm = height.or(part.height_mm);
        (part.loss_kpa, part.kvs) = (loss.or(part.loss_kpa), kvs.or(part.kvs));
        if let Some(n) = outlets {
            part.outlets = n;
            // A header or a box as long as its outlets need, unless a length was given.
            if length.is_none()
                && let Some(l) = part.kind.length_for_outlets_mm(n)
            {
                part.length_mm = l;
            }
        }
        if let Some(c) = connections {
            part.connections = c;
        }
    })?;
    let placed = part_of(s.doc()?.entity(h).ok_or_else(|| other("Cannot find that part."))?).ok_or_else(|| other("The part was not drawn."))?;
    Ok(json!({ "handle": h.hex(), "name": placed.name, "model": placed.model }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "pipepartedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_EDITED {
        return Err(bad(CMD, format!("select 1 to {MAX_EDITED} parts")));
    }
    let (dn, volume, power) = (clearable(CMD, p, "dn")?, clearable(CMD, p, "volume")?, clearable(CMD, p, "power")?);
    let (flow, head, weight) = (clearable(CMD, p, "flow")?, clearable(CMD, p, "head")?, clearable(CMD, p, "weight")?);
    let (loss, kvs) = (clearable(CMD, p, "loss")?, clearable(CMD, p, "kvs")?);
    let (eq_length, zeta) = (clearable(CMD, p, "equivalentLength")?, clearable(CMD, p, "zeta")?);
    let (length, width) = (number(CMD, p, "length")?, number(CMD, p, "width")?);
    let height = clearable(CMD, p, "height")?;
    let outlets = whole(CMD, p, "outlets")?;
    let d = s.doc()?;
    let mut next = Vec::with_capacity(hs.len());
    for h in &hs {
        let mut pp = d.entity(*h).and_then(|e| part_of(e)).ok_or_else(|| other("Select pipework parts only."))?;
        if let Some(n) = str_param(p, "name") {
            pp.name = n.to_string();
        }
        if let Some(m) = str_param(p, "model") {
            // A model of the library gives its values (the tag and the pipe's size stay); another text is only a name.
            match standard_part_model_named(m) {
                Some(lib) => take_model(&mut pp, &lib),
                None => pp.model = m.to_string(),
            }
        }
        if let Some(b) = str_param(p, "brand") {
            pp.brand = b.to_string();
        }
        if let Some(c) = str_param(p, "connections") {
            pp.connections = c.to_string();
        }
        pp.dn_mm = dn.unwrap_or(pp.dn_mm);
        pp.volume_l = volume.unwrap_or(pp.volume_l);
        pp.power_kw = power.unwrap_or(pp.power_kw);
        pp.flow_m3h = flow.unwrap_or(pp.flow_m3h);
        pp.head_kpa = head.unwrap_or(pp.head_kpa);
        pp.weight_kg = weight.unwrap_or(pp.weight_kg);
        pp.loss_kpa = loss.unwrap_or(pp.loss_kpa);
        pp.kvs = kvs.unwrap_or(pp.kvs);
        pp.equivalent_length_m = eq_length.unwrap_or(pp.equivalent_length_m);
        pp.zeta = zeta.unwrap_or(pp.zeta);
        pp.length_mm = length.unwrap_or(pp.length_mm);
        pp.width_mm = width.unwrap_or(pp.width_mm);
        pp.height_mm = height.unwrap_or(pp.height_mm);
        if let Some(n) = outlets {
            pp.outlets = n;
            // The part is as long as its outlets need, unless a length was given: it changes on the drawing as a unit does.
            if length.is_none()
                && let Some(l) = pp.kind.length_for_outlets_mm(n)
            {
                pp.length_mm = l;
            }
        }
        pp.validate().map_err(doc_err)?;
        next.push((*h, pp));
    }
    let d = s.doc_mut()?;
    for (h, pp) in &next {
        d.regenerate_part(*h, pp).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

/// Gives `pp` the values of library model `m`: its kind, sizes, outlets and connections (its tag and its pipe's size stay).
fn take_model(pp: &mut PartParams, m: &PartModel) {
    let (kind, dn) = (m.kind, pp.dn_mm);
    let fresh = m.params(&pp.name, pp.units);
    *pp = PartParams { kind, dn_mm: dn.or(fresh.dn_mm), label_height_mm: pp.label_height_mm, ..fresh };
}

fn category_name(c: PartCategory) -> &'static str {
    match c {
        PartCategory::Valves => "valves",
        PartCategory::Fittings => "fittings",
        PartCategory::AirAndDirt => "airAndDirt",
        PartCategory::Instruments => "instruments",
        PartCategory::Pumps => "pumps",
        PartCategory::VesselsAndTanks => "vesselsAndTanks",
        PartCategory::HeatSources => "heatSources",
        PartCategory::RefrigerantBranches => "refrigerantBranches",
        PartCategory::BranchBoxes => "branchBoxes",
    }
}

fn run_models(_s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "pipepartmodels";
    let wanted = match str_param(p, "category") {
        Some(c) => {
            Some(PartCategory::ALL.into_iter().find(|x| category_name(*x).eq_ignore_ascii_case(c)).ok_or_else(|| bad(CMD, "unknown `category`"))?)
        }
        None => None,
    };
    let words: Vec<String> = str_param(p, "q").map(|q| q.to_lowercase().split_whitespace().map(str::to_string).collect()).unwrap_or_default();
    let models: Vec<Value> = standard_part_models()
        .into_iter()
        .filter(|m| wanted.is_none_or(|c| m.category == c))
        .filter(|m| {
            let hay = format!("{} {} {} {}", m.name, m.name_fr, m.brand, m.connections).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .map(|m| {
            json!({
                "name": m.name, "nameFr": m.name_fr, "category": category_name(m.category), "kind": m.kind.name(), "brand": m.brand,
                "outlets": m.outlets, "connections": m.connections, "dn": m.dn_mm, "volume": m.volume_l, "power": m.power_kw,
            })
        })
        .collect();
    let categories: Vec<Value> = PartCategory::ALL.into_iter().map(|c| json!({ "name": category_name(c), "title": c.title() })).collect();
    Ok(json!({ "categories": categories, "models": models }))
}

/// PIPEPARTADD at the command line: pick the model (or use the last), then click where each part goes.
struct PartAddM {
    model: String,
    ask_model: bool,
}

impl PartAddM {
    fn new(s: &Session) -> Self {
        PartAddM { model: s.last_used.part.model.clone(), ask_model: false }
    }
}

impl Interactive for PartAddM {
    fn name(&self) -> &'static str {
        "PIPEPARTADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.echo(format!("Current settings: Model = {}", self.model));
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.ask_model {
            return Prompt::new("Enter the name of a model of the library", Accept::TEXT).default(self.model.clone());
        }
        Prompt::new("Specify the place of the part (on a pipe it turns along it)", Accept::POINT).kw(&["Model"])
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.ask_model {
            if let Input::Text(t) | Input::Keyword(t) = &i {
                match standard_part_model_named(t) {
                    Some(m) => {
                        self.model = m.name.to_string();
                        s.echo(format!("Current settings: Model = {}", self.model));
                    }
                    None => s.echo("Not a model of the library: see PIPEPARTMODELS."),
                }
            }
            self.ask_model = false;
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) if k == "Model" => {
                self.ask_model = true;
                Ok(Step::Continue)
            }
            Input::Point(at) => {
                let Some(model) = standard_part_model_named(&self.model) else { return Ok(Step::Done) };
                match place(s, &model, None, at, None, |_| {}) {
                    Ok(_) => s.last_used.part.model = self.model.clone(),
                    Err(e) => s.echo(e.to_string()),
                }
                Ok(Step::Continue)
            }
            Input::Enter | Input::Cancel => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, _c: Vec2) -> Vec<EntityKind> {
        Vec::new()
    }
    fn preview_objects(&self, s: &Session, c: Vec2) -> Vec<cadcraft_doc::Entity> {
        // The part under the cursor, as it will be drawn, turned along the pipe it would sit on.
        let (Some(model), Ok(d)) = (standard_part_model_named(&self.model), s.doc()) else { return Vec::new() };
        let (at, angle) = pipe_under(d, c).map_or((c, 0.0), |(_, foot, a, _)| (foot, a));
        let p = model.params(&next_tag(d, &model), HvacUnits::of(d));
        p.generate().map(|pieces| cadcraft_doc::preview_entities(pieces, at, angle)).unwrap_or_default()
    }
}
