//! Pipe risers (colonnes de tuyauterie): PIPERISERADD draws the shaft where the pipes of a floor go up or down, with the lines it
//! holds (chilled water, hot water, change-over, condensate, refrigerant links); PIPERISEREDIT changes a riser already drawn.

use cadcraft_doc::{
    Drawing, EntityKind, Handle, HvacUnits, MAX_PIPE_RISER_VERTICES, MAX_RISER_LINES, PipeLeg, PipeRiserParams, PipeService, RiserLine,
    pipe_riser_of, pipe_tag_parse,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most risers one PIPERISEREDIT changes.
const MAX_EDITED: usize = 10_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("piperiseradd", "Pipe Riser", run_add)
            .menu(&["Draw", "HVAC", "Pipe Riser"])
            .params(
                "{points (3 or more) | handle (closed polyline), name? (shared by the risers of the other floors), up? (true: the pipes go up from this level; default), height? (mm, floor to floor), \\
                 lines? (a list: a tag \"CHW-S\", \"CHW-R\", \"HW-S\", \"HW-R\", \"CO-S\", \"CO-R\", \"COND\", \"REF-L\", \"REF-G\", \"REF\", \"REF-HR\", or {service, leg?, dn? (mm; inches for a refrigerant tube), gas?, hpGas?, \\
                 insulation? (mm), incoming? (what the line carries to the other floors: kW, l/h for a drain, capacity index for refrigerant)}; default chilled water supply and return), labelHeight? (mm)} → {handle, name, lines}",
            )
            .interactive(|s| Ok(Box::new(PipeRiserAddM::new(s)))),
        CommandSpec::new("piperiseredit", "Edit Pipe Riser", run_edit).params(
            "{handle | handles, name?, up?, height? (mm or null), lines? (replace them all, as for piperiseradd) | addLines? (appended) | removeLine? (the number of a line, from 1), \\
             line? ({index (from 1), dn?, gas?, hpGas?, insulation?, incoming? (or null), imposed?}: one line changed)} → {changed}",
        ),
    ]
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

/// A size in mm from a command: a number, or text (inches for a refrigerant tube).
fn size(cmd: &str, key: &str, v: &Value, refrigerant: bool) -> Result<f64> {
    let mm = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(t) if refrigerant => cadcraft_doc::parse_refrigerant_size(t),
        Value::String(t) => t.trim().parse::<f64>().ok(),
        _ => None,
    };
    mm.filter(|x| x.is_finite() && *x > 0.0)
        .ok_or_else(|| bad(cmd, format!("`{key}` must be a size in millimetres{}", if refrigerant { " or inches (3/8, 1 1/8)" } else { "" })))
}

fn default_dn(service: PipeService) -> f64 {
    super::pipe::default_dn_mm(service)
}

/// One line from a command: a tag, or an object.
fn line_value(cmd: &str, v: &Value) -> Result<RiserLine> {
    if let Some(tag) = v.as_str() {
        let (service, leg) = pipe_tag_parse(tag)
            .ok_or_else(|| bad(cmd, format!("`{tag}` is not a line tag (CHW-S, CHW-R, HW-S, HW-R, CO-S, CO-R, COND, REF-L, REF-G, REF, REF-HR)")))?;
        return Ok(RiserLine::new(service, leg, default_dn(service)));
    }
    let service = str_param(v, "service").and_then(PipeService::parse).ok_or_else(|| bad(cmd, "a line needs a `service`"))?;
    let leg =
        str_param(v, "leg").map(|t| PipeLeg::parse(t).ok_or_else(|| bad(cmd, "`leg` must be supply or return"))).transpose()?.unwrap_or_default();
    let mut l = RiserLine::new(service, leg, default_dn(service));
    apply_line(cmd, &mut l, v)?;
    Ok(l)
}

/// Sets what `v` names on `l`: the sizes, the insulation, what it carries, whether the size is the user's.
fn apply_line(cmd: &str, l: &mut RiserLine, v: &Value) -> Result<()> {
    let refrigerant = l.service.is_refrigerant();
    if let Some(d) = v.get("dn").filter(|x| !x.is_null()) {
        (l.dn_mm, l.imposed) = (size(cmd, "dn", d, refrigerant)?, true);
    }
    if let Some(g) = v.get("gas").filter(|x| !x.is_null()) {
        l.gas_dn_mm = size(cmd, "gas", g, true)?;
    }
    if let Some(g) = v.get("hpGas").filter(|x| !x.is_null()) {
        l.gas2_dn_mm = size(cmd, "hpGas", g, true)?;
    }
    if let Some(i) = v.get("insulation").filter(|x| !x.is_null()) {
        l.insulation_mm =
            i.as_f64().filter(|x| x.is_finite() && *x >= 0.0).ok_or_else(|| bad(cmd, "`insulation` must be a number of millimetres"))?;
    }
    match v.get("incoming") {
        None => {}
        Some(Value::Null) => l.incoming = None,
        Some(x) => {
            l.incoming = Some(x.as_f64().filter(|n| n.is_finite() && *n >= 0.0).ok_or_else(|| bad(cmd, "`incoming` must be a load of 0 or more"))?)
        }
    }
    if let Some(b) = v.get("imposed").and_then(Value::as_bool) {
        l.imposed = b;
    }
    Ok(())
}

fn lines_param(cmd: &str, p: &Value, key: &str) -> Result<Option<Vec<RiserLine>>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) if a.len() <= MAX_RISER_LINES => a.iter().map(|v| line_value(cmd, v)).collect::<Result<Vec<_>>>().map(Some),
        Some(_) => Err(bad(cmd, format!("`{key}` is a list of at most {MAX_RISER_LINES} lines"))),
    }
}

fn height_param(cmd: &str, p: &Value) -> Result<Option<Option<f64>>> {
    match p.get("height") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && *x > 0.0)
            .map(|x| Some(Some(x)))
            .ok_or_else(|| bad(cmd, "`height` must be a number of millimetres or null")),
    }
}

fn risers_count(d: &Drawing) -> usize {
    d.model.iter().filter(|e| pipe_riser_of(e).is_some()).count()
}

fn default_lines() -> Vec<RiserLine> {
    vec![RiserLine::new(PipeService::ChilledWater, PipeLeg::Supply, 50.0), RiserLine::new(PipeService::ChilledWater, PipeLeg::Return, 50.0)]
}

fn add_riser(s: &mut Session, p: PipeRiserParams) -> Result<Handle> {
    p.validate().map_err(doc_err)?;
    let space = s.space();
    let common = s.current_common();
    let h = s.doc_mut()?.add_pipe_riser(&space, common, p).map_err(doc_err)?;
    // The lines of a riser are on the network whose pipes end in the shaft.
    super::network::propagate(s);
    Ok(h)
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "piperiseradd";
    let mut outline = match (super::curves::handle_param(p, "handle"), p.get("points")) {
        (Some(h), _) => super::hvac::polyline_outline(s.doc()?, h)?,
        (None, Some(Value::Array(raw))) => {
            if raw.len() > MAX_PIPE_RISER_VERTICES + 1 {
                return Err(bad(CMD, format!("a shaft has at most {MAX_PIPE_RISER_VERTICES} points")));
            }
            let pts: Option<Vec<Vec2>> = raw.iter().map(super::point_value).collect();
            super::distinct_points(&pts.ok_or_else(|| bad(CMD, "`points` must be [x, y] points"))?)
        }
        _ => return Err(bad(CMD, "`points` (3 or more) or `handle` (a closed polyline) is required")),
    };
    if outline.len() > 3 && outline.first().zip(outline.last()).is_some_and(|(a, b)| a.near(*b, 1e-12)) {
        outline.pop();
    }
    let d = s.doc()?;
    let name = str_param(p, "name").map(str::to_string).unwrap_or_else(|| format!("CT{}", risers_count(d) + 1));
    let lines = lines_param(CMD, p, "lines")?.unwrap_or_else(default_lines);
    let label = p.get("labelHeight").and_then(Value::as_f64).filter(|x| x.is_finite() && *x > 0.0).unwrap_or(100.0);
    let riser = PipeRiserParams {
        name,
        up: p.get("up").and_then(Value::as_bool).unwrap_or(true),
        outline,
        lines,
        height_mm: height_param(CMD, p)?.flatten(),
        label_height_mm: label,
        units: HvacUnits::of(d),
    };
    let h = add_riser(s, riser)?;
    let placed =
        pipe_riser_of(s.doc()?.entity(h).ok_or_else(|| other("Cannot find that riser."))?).ok_or_else(|| other("The riser was not drawn."))?;
    Ok(json!({ "handle": h.hex(), "name": placed.name, "lines": placed.lines.iter().map(|l| l.pipe(placed.units).tag()).collect::<Vec<_>>() }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "piperiseredit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_EDITED {
        return Err(bad(CMD, format!("select 1 to {MAX_EDITED} pipe risers")));
    }
    let (replace, append) = (lines_param(CMD, p, "lines")?, lines_param(CMD, p, "addLines")?);
    let remove = match p.get("removeLine").filter(|v| !v.is_null()) {
        Some(v) => Some(v.as_u64().filter(|n| *n >= 1).ok_or_else(|| bad(CMD, "`removeLine` is the number of a line, from 1"))? as usize),
        None => None,
    };
    let height = height_param(CMD, p)?;
    let d = s.doc()?;
    let mut next = Vec::with_capacity(hs.len());
    for h in &hs {
        let mut r = d.entity(*h).and_then(|e| pipe_riser_of(e)).ok_or_else(|| other("Select pipe risers only."))?;
        if let Some(n) = str_param(p, "name") {
            r.name = n.to_string();
        }
        if let Some(u) = p.get("up").and_then(Value::as_bool) {
            r.up = u;
        }
        if let Some(hh) = height {
            r.height_mm = hh;
        }
        if let Some(l) = &replace {
            r.lines = l.clone();
        }
        if let Some(l) = &append {
            r.lines.extend(l.iter().cloned());
        }
        if let Some(n) = remove {
            if n > r.lines.len() {
                return Err(other(format!("The riser has {} line(s).", r.lines.len())));
            }
            r.lines.remove(n - 1);
        }
        if let Some(edit) = p.get("line").filter(|v| v.is_object()) {
            let n = edit
                .get("index")
                .and_then(Value::as_u64)
                .filter(|n| *n >= 1)
                .ok_or_else(|| bad(CMD, "`line.index` is the number of a line, from 1"))? as usize;
            let l = r.lines.get_mut(n - 1).ok_or_else(|| other(format!("The riser has {} line(s).", n - 1)))?;
            apply_line(CMD, l, edit)?;
        }
        r.validate().map_err(doc_err)?;
        next.push((*h, r));
    }
    let d = s.doc_mut()?;
    for (h, r) in &next {
        d.regenerate_pipe_riser(*h, r).map_err(doc_err)?;
    }
    super::network::propagate(s);
    Ok(json!({ "changed": next.len() }))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    Point,
    Object,
    Name,
    Lines,
    Height,
}

/// PIPERISERADD at the command line: the points of the shaft (or a closed polyline), after Name, Lines, Height and Up.
struct PipeRiserAddM {
    pts: Vec<Vec2>,
    name: String,
    up: bool,
    lines: Vec<RiserLine>,
    height: Option<f64>,
    ask: Ask,
}

impl PipeRiserAddM {
    fn new(s: &Session) -> Self {
        let n = s.doc().map(risers_count).unwrap_or(0) + 1;
        PipeRiserAddM { pts: Vec::new(), name: format!("CT{n}"), up: true, lines: default_lines(), height: None, ask: Ask::Point }
    }
    fn settings(&self, s: &mut Session) {
        let tags: Vec<String> = self.lines.iter().map(|l| l.pipe(HvacUnits::default()).tag().to_string()).collect();
        s.echo(format!(
            "Current settings: Name = {}, Direction = {}, Lines = {}, Height = {}",
            self.name,
            if self.up { "Up" } else { "Down" },
            tags.join(","),
            self.height.map_or("-".to_string(), |h| format!("{h}"))
        ));
    }
    fn finish(&mut self, s: &mut Session, outline: Vec<Vec2>) -> Result<Step> {
        let riser = PipeRiserParams {
            name: self.name.clone(),
            up: self.up,
            outline,
            lines: self.lines.clone(),
            height_mm: self.height,
            label_height_mm: 100.0,
            units: HvacUnits::of(s.doc()?),
        };
        if let Err(e) = riser.validate() {
            s.echo(e.to_string());
            return Ok(Step::Continue);
        }
        add_riser(s, riser)?;
        s.echo(format!("Pipe riser {}: {} line(s)", self.name, self.lines.len()));
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
                match super::hvac::polyline_outline(s.doc()?, h) {
                    Ok(outline) => return self.finish(s, outline),
                    Err(e) => {
                        s.echo(e.to_string());
                        return Ok(Step::Continue);
                    }
                }
            }
            (Ask::Name, _, Some(t)) if !t.is_empty() => self.name = t.to_string(),
            (Ask::Lines, _, Some(t)) => {
                let lines: Option<Vec<RiserLine>> = t
                    .split([',', ' ', ';'])
                    .filter(|x| !x.is_empty())
                    .map(|tag| pipe_tag_parse(tag).map(|(sv, lg)| RiserLine::new(sv, lg, default_dn(sv))))
                    .collect();
                self.lines = lines.filter(|l| !l.is_empty() && l.len() <= MAX_RISER_LINES).ok_or_else(|| {
                    other("Enter line tags separated by commas: CHW-S, CHW-R, HW-S, HW-R, CO-S, CO-R, COND, REF-L, REF-G, REF, REF-HR.")
                })?;
            }
            (Ask::Height, _, Some(t)) => {
                self.height = match t.parse::<f64>().ok().filter(|h| h.is_finite() && *h >= 0.0) {
                    Some(0.0) => None,
                    Some(h) => Some(h),
                    None => return Err(other("Requires a height in millimetres (0 for none).")),
                };
            }
            (Ask::Object, Input::Enter | Input::Cancel, _) => {}
            _ => return Ok(Step::Continue),
        }
        self.ask = Ask::Point;
        self.settings(s);
        Ok(Step::Continue)
    }
}

impl Interactive for PipeRiserAddM {
    fn name(&self) -> &'static str {
        "PIPERISERADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Object => return Prompt::new("Select a closed polyline", Accept::SELECT),
            Ask::Name => return Prompt::new("Enter riser name", Accept::TEXT).default(self.name.clone()),
            Ask::Lines => {
                return Prompt::new("Enter the lines (CHW-S, CHW-R, HW-S, HW-R, CO-S, CO-R, COND, REF-L, REF-G, REF, REF-HR)", Accept::TEXT);
            }
            Ask::Height => {
                return Prompt::new("Enter the height from floor to floor (mm, 0 for none)", Accept::NUMBER)
                    .default(self.height.map_or("0".to_string(), |h| format!("{h}")));
            }
            Ask::Point => {}
        }
        let last = self.pts.last().copied();
        match self.pts.len() {
            0 => Prompt::new("Specify first point of the shaft", Accept::POINT).kw(&["OBject", "Name", "Lines", "Height", "Up"]),
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
                    "Lines" => self.ask = Ask::Lines,
                    "Height" => self.ask = Ask::Height,
                    "Up" => {
                        self.up = !self.up;
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
                if self.pts.len() < MAX_PIPE_RISER_VERTICES && !self.pts.last().is_some_and(|l| l.near(p, 1e-9)) {
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
        if self.pts.is_empty() || self.ask != Ask::Point {
            return Vec::new();
        }
        let mut pts = self.pts.clone();
        pts.push(c);
        if let Some(first) = self.pts.first() {
            pts.push(*first);
        }
        pts.windows(2).filter_map(|w| Some(EntityKind::Line(cadcraft_doc::Line { a: w.first()?.to3(0.0), b: w.get(1)?.to3(0.0) }))).collect()
    }
}
