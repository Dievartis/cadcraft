//! The furniture commands: FURNITUREADD puts furniture and appliances on the drawing from the catalogue, at their size, and FURNITUREEDIT changes them.

use cadcraft_doc::{FurnitureModel, FurnitureParams, Handle, HvacUnits, furniture_models, furniture_of, parse_furniture_model};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most pieces one command puts or changes.
const MAX_PIECES: usize = 5_000;

/// What FURNITUREADD remembers between uses: the model and the turn.
#[derive(Clone, Debug, PartialEq)]
pub struct FurnitureLastUsed {
    /// The id of the model.
    pub model: String,
    pub rotation_deg: f64,
}

impl Default for FurnitureLastUsed {
    fn default() -> Self {
        FurnitureLastUsed { model: furniture_models().first().map_or_else(String::new, |m| m.id.to_string()), rotation_deg: 0.0 }
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("furnitureadd", "Furniture", run_add)
            .menu(&["Draw", "Furniture", "Furniture"])
            .params(
                "{at ([x, y], the middle of its side against the wall) | points ([[x, y]…]: one piece at each), model? (an id of the catalogue: sofa3, bed160, fridge, tv55, desk160, \
                 meet10… or its English name; default sofa2), rotation? (degrees: 0 has the wall along x and the piece toward -y), width? / depth? / height? (mm), name? (written beside it), \
                 altitude? (metres, its underside), power? (W, an appliance), labelHeight? (mm)} → {handles}",
            )
            .interactive(|s| Ok(Box::new(FurnitureAddM::new(s)))),
        CommandSpec::new("furnitureedit", "Edit Furniture", run_edit).params(
            "{handle | handles, model? (its size and values become the model's), width? / depth? / height? (mm), name?, altitude? (metres, or null for none), power? (W, or null for none)} → {changed}",
        ),
        CommandSpec::new("furniturelist", "Furniture Catalogue", run_list)
            .params("{group? (a group of the catalogue)} → {models: [{id, label, group, width, depth, height, power}]}: the catalogue the Furniture tab offers"),
    ]
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

/// The number at `key`: `None` when absent, an error when it is not a finite number.
fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a number"))),
    }
}

/// A value that can be cleared: absent (unchanged), null (none) or a number.
fn clearable(cmd: &str, p: &Value, key: &str) -> Result<Option<Option<f64>>> {
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(_) => Ok(Some(number(cmd, p, key)?)),
    }
}

fn model_param(cmd: &str, p: &Value) -> Result<Option<&'static FurnitureModel>> {
    match p.get("model") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().and_then(parse_furniture_model).map(Some).ok_or_else(|| bad(cmd, "`model` is not in the catalogue: see FURNITURELIST")),
    }
}

/// Applies the values of `p` that FURNITUREADD and FURNITUREEDIT share to `f`.
fn apply(cmd: &str, p: &Value, f: &mut FurnitureParams) -> Result<()> {
    if let Some(w) = number(cmd, p, "width")? {
        f.width_mm = w;
    }
    if let Some(d) = number(cmd, p, "depth")? {
        f.depth_mm = d;
    }
    if let Some(h) = number(cmd, p, "height")? {
        f.height_mm = h;
    }
    if let Some(n) = str_param(p, "name") {
        f.name = n.trim().to_string();
    }
    if let Some(a) = clearable(cmd, p, "altitude")? {
        f.bottom_mm = a.map(|m| m * 1000.0);
    }
    if let Some(w) = clearable(cmd, p, "power")? {
        f.power_w = w;
    }
    if let Some(l) = number(cmd, p, "labelHeight")? {
        f.label_height_mm = l;
    }
    f.validate().map_err(doc_err)
}

fn put(s: &mut Session, f: &FurnitureParams, at: Vec2, rotation: f64) -> Result<Handle> {
    let (space, common) = (s.space(), s.current_common());
    s.doc_mut()?.add_furniture(&space, common, f, at, rotation).map_err(doc_err)
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "furnitureadd";
    let mut pts = super::points_param(p, "points").unwrap_or_default();
    if let Some(at) = super::point_param(p, "at") {
        pts.push(at);
    }
    if pts.is_empty() || pts.len() > MAX_PIECES {
        return Err(bad(CMD, format!("`at` or `points` (1 to {MAX_PIECES} [x, y]) is required")));
    }
    let model = match model_param(CMD, p)? {
        Some(m) => m,
        None => furniture_models().first().ok_or_else(|| other("The furniture catalogue is empty."))?,
    };
    let rotation = number(CMD, p, "rotation")?.unwrap_or(0.0).to_radians();
    let mut f = FurnitureParams::new(model, HvacUnits::of(s.doc()?));
    apply(CMD, p, &mut f)?;
    let mut handles = Vec::new();
    for at in pts {
        handles.push(put(s, &f, at, rotation)?.hex());
    }
    Ok(json!({ "handles": handles }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "furnitureedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_PIECES {
        return Err(bad(CMD, format!("select 1 to {MAX_PIECES} pieces of furniture")));
    }
    let model = model_param(CMD, p)?;
    let mut next = Vec::with_capacity(hs.len());
    for h in &hs {
        let old = s.doc()?.entity(*h).and_then(|e| furniture_of(e)).ok_or_else(|| other("Select furniture only."))?;
        // Another model takes that model's size and values; the name and the altitude given stay.
        let mut f = match model {
            Some(m) if m.id != old.model => {
                FurnitureParams { name: old.name.clone(), bottom_mm: old.bottom_mm, ..FurnitureParams::new(m, old.units) }
            }
            _ => old,
        };
        apply(CMD, p, &mut f)?;
        next.push((*h, f));
    }
    let d = s.doc_mut()?;
    for (h, f) in &next {
        d.regenerate_furniture(*h, f).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

fn run_list(_s: &mut Session, p: &Value) -> Result<Value> {
    let group = str_param(p, "group");
    let models: Vec<Value> = furniture_models()
        .iter()
        .filter(|m| group.is_none_or(|g| m.group.eq_ignore_ascii_case(g.trim())))
        .map(|m| json!({ "id": m.id, "label": m.label, "group": m.group, "width": m.width_mm, "depth": m.depth_mm, "height": m.height_mm, "power": m.power_w }))
        .collect();
    Ok(json!({ "models": models }))
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    Place,
    Model,
    Rotation,
}

struct FurnitureAddM {
    last: FurnitureLastUsed,
    ask: Ask,
}

impl FurnitureAddM {
    fn new(s: &Session) -> Self {
        FurnitureAddM { last: s.last_used.furniture.clone(), ask: Ask::Place }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!("Current settings: Type = {}, Rotation = {}", self.last.model, self.last.rotation_deg));
    }
    fn params(&self, s: &Session) -> Result<FurnitureParams> {
        let model = parse_furniture_model(&self.last.model).ok_or_else(|| other("The furniture catalogue is empty."))?;
        Ok(FurnitureParams::new(model, HvacUnits::of(s.doc()?)))
    }
}

impl Interactive for FurnitureAddM {
    fn name(&self) -> &'static str {
        "FURNITUREADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Model => Prompt::new("Enter the type of furniture", Accept::TEXT).default(self.last.model.clone()),
            Ask::Rotation => Prompt::new("Specify rotation angle", Accept::NUMBER).default(format!("{}", self.last.rotation_deg)),
            Ask::Place => Prompt::new("Specify the middle of the back of the furniture", Accept::POINT).kw(&["Type", "Rotate"]),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        match (self.ask, i, text) {
            (Ask::Model, _, Some(t)) => {
                match parse_furniture_model(&t) {
                    Some(m) => self.last.model = m.id.to_string(),
                    None => s.echo("Not a type of furniture: see FURNITURELIST."),
                }
                self.ask = Ask::Place;
                self.settings(s);
            }
            (Ask::Rotation, _, Some(t)) => {
                match t.parse::<f64>().ok().filter(|v| v.is_finite()) {
                    Some(v) => self.last.rotation_deg = v,
                    None => s.echo("Requires an angle in degrees."),
                }
                self.ask = Ask::Place;
                self.settings(s);
            }
            (Ask::Place, Input::Keyword(k), _) if k == "Type" => self.ask = Ask::Model,
            (Ask::Place, Input::Keyword(k), _) if k == "Rotate" => self.ask = Ask::Rotation,
            (Ask::Place, Input::Point(at), _) => {
                let f = self.params(s)?;
                match put(s, &f, at, self.last.rotation_deg.to_radians()) {
                    Ok(_) => s.last_used.furniture = self.last.clone(),
                    Err(e) => s.echo(e.to_string()),
                }
            }
            (Ask::Place, Input::Enter | Input::Cancel, _) => return Ok(Step::Done),
            (_, Input::Enter | Input::Cancel, _) => self.ask = Ask::Place,
            _ => {}
        }
        Ok(Step::Continue)
    }
    fn preview_objects(&self, s: &Session, c: Vec2) -> Vec<cadcraft_doc::Entity> {
        let Ok(f) = self.params(s) else { return Vec::new() };
        f.generate().map(|pieces| cadcraft_doc::preview_entities(pieces, c, self.last.rotation_deg.to_radians())).unwrap_or_default()
    }
}
