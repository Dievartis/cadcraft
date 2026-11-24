//! The plumbing fixtures: FIXTUREADD puts water closets, basins, sinks, showers, baths… on the drawing at their size, FIXTUREEDIT changes them,
//! FIXTURECONNECT draws the branch from each fixture's drain to the nearest drain pipe of its kind (waste or soil water), as wide as the fixture's
//! drain and with its slope.

use cadcraft_doc::{Handle, HvacUnits, PipeParams, PipeService, SanitaryKind, SanitaryParams, sanitary_of};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};
use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most fixtures one command puts, connects or changes.
const MAX_FIXTURES: usize = 5_000;
/// How far a fixture's drain looks for a drain pipe of its kind, mm.
const CONNECT_REACH_MM: f64 = 6_000.0;
/// The slope a branch is given, %: soil water (water closets) 2 %, waste water 1 %.
const SOIL_SLOPE_PCT: f64 = 2.0;
const WASTE_SLOPE_PCT: f64 = 1.0;

/// What FIXTUREADD remembers between uses: the kind and the turn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SanitaryLastUsed {
    pub kind: SanitaryKind,
    pub rotation_deg: f64,
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("fixtureadd", "Sanitary Fixture", run_add)
            .menu(&["Draw", "Plumbing", "Sanitary Fixture"])
            .params(
                "{at ([x, y], the middle of its side against the wall) | points ([[x, y]…]: one fixture at each), kind? (wcWallHung, wcFloor, wcAccessible, urinal, \
                 washbasin, handBasin, doubleBasin, sinkSingle, sinkDouble, shower, bath, bidet, slopSink, floorDrain, washingMachine, dishwasher, waterHeater, tap; \
                 default wcWallHung), rotation? (degrees: 0 has the wall along x and the fixture toward -y), width? / depth? (mm), name?, drain? (wasteWater or soilWater), \
                 drainDn? (mm), cold? / hot? (l/s, base flows), altitude? (metres, its underside), labelHeight? (mm)} → {handles}",
            )
            .interactive(|s| Ok(Box::new(FixtureAddM::new(s)))),
        CommandSpec::new("fixtureedit", "Edit Sanitary Fixture", run_edit).params(
            "{handle | handles, kind? (its size and values become the kind's), width? / depth? (mm), name?, drain? (wasteWater, soilWater, or null for none), drainDn? (mm), \
             cold? / hot? (l/s, or null for none), altitude? (metres, or null for none)} → {changed}",
        ),
        CommandSpec::new("fixtureconnect", "Connect Fixtures", run_connect).menu(&["Draw", "Plumbing", "Connect Fixtures"]).params(
            "{fixtures? (handles; default every fixture with a drain), maxDistance? (mm, default 6000), slope? (%; default 2 for soil water, 1 for waste water)} → \
             {created, skipped: [{fixture, reason}]}",
        ),
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

fn drain_param(cmd: &str, p: &Value) -> Result<Option<Option<PipeService>>> {
    match p.get("drain") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => {
            let s = v.as_str().and_then(PipeService::parse).filter(|s| s.is_gravity_drain() && *s != PipeService::Condensate);
            s.map(|s| Some(Some(s))).ok_or_else(|| bad(cmd, "`drain` must be wasteWater, soilWater, rainWater or null"))
        }
    }
}

/// Applies the values of `p` that FIXTUREADD and FIXTUREEDIT share to `f`.
fn apply(cmd: &str, p: &Value, f: &mut SanitaryParams) -> Result<()> {
    if let Some(w) = number(cmd, p, "width")? {
        f.width_mm = w;
    }
    if let Some(d) = number(cmd, p, "depth")? {
        f.depth_mm = d;
    }
    if let Some(n) = str_param(p, "name") {
        f.name = n.trim().to_string();
    }
    if let Some(d) = drain_param(cmd, p)? {
        f.drain = d;
    }
    if let Some(dn) = number(cmd, p, "drainDn")? {
        f.drain_dn_mm = dn;
    }
    if let Some(c) = clearable(cmd, p, "cold")? {
        f.cold_ls = c;
    }
    if let Some(h) = clearable(cmd, p, "hot")? {
        f.hot_ls = h;
    }
    if let Some(a) = clearable(cmd, p, "altitude")? {
        f.bottom_mm = a.map(|m| m * 1000.0);
    }
    if let Some(l) = number(cmd, p, "labelHeight")? {
        f.label_height_mm = l;
    }
    f.validate().map_err(doc_err)
}

fn kind_param(cmd: &str, p: &Value) -> Result<Option<SanitaryKind>> {
    match p.get("kind") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().and_then(SanitaryKind::parse).map(Some).ok_or_else(|| bad(cmd, "`kind` is not a kind of fixture: see FIXTUREADD")),
    }
}

fn put(s: &mut Session, f: &SanitaryParams, at: Vec2, rotation: f64) -> Result<Handle> {
    let (space, common) = (s.space(), s.current_common());
    s.doc_mut()?.add_sanitary(&space, common, f, at, rotation).map_err(doc_err)
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "fixtureadd";
    let mut pts = super::points_param(p, "points").unwrap_or_default();
    if let Some(at) = super::point_param(p, "at") {
        pts.push(at);
    }
    if pts.is_empty() || pts.len() > MAX_FIXTURES {
        return Err(bad(CMD, format!("`at` or `points` (1 to {MAX_FIXTURES} [x, y]) is required")));
    }
    let kind = kind_param(CMD, p)?.unwrap_or_default();
    let rotation = number(CMD, p, "rotation")?.unwrap_or(0.0).to_radians();
    let mut f = SanitaryParams::new(kind, HvacUnits::of(s.doc()?));
    apply(CMD, p, &mut f)?;
    let mut handles = Vec::new();
    for at in pts {
        handles.push(put(s, &f, at, rotation)?.hex());
    }
    Ok(json!({ "handles": handles }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "fixtureedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_FIXTURES {
        return Err(bad(CMD, format!("select 1 to {MAX_FIXTURES} fixtures")));
    }
    let kind = kind_param(CMD, p)?;
    let mut next = Vec::with_capacity(hs.len());
    for h in &hs {
        let old = s.doc()?.entity(*h).and_then(|e| sanitary_of(e)).ok_or_else(|| other("Select sanitary fixtures only."))?;
        // Another kind takes that kind's size and values; the name and the altitude given stay.
        let mut f = match kind {
            Some(k) if k != old.kind => SanitaryParams { name: old.name.clone(), units: old.units, ..SanitaryParams::new(k, old.units) },
            _ => old,
        };
        apply(CMD, p, &mut f)?;
        next.push((*h, f));
    }
    let d = s.doc_mut()?;
    for (h, f) in &next {
        d.regenerate_sanitary(*h, f).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

/// Every drain pipe of the drawing as (service, one end, the other), read once: the branches a command draws are not collectors for the fixtures after them.
fn drain_pipes(s: &Session) -> Result<Vec<(PipeService, Vec2, Vec2)>> {
    let d = s.doc()?;
    let space = s.space();
    Ok(d.space(&space)
        .into_iter()
        .flat_map(|sp| sp.iter())
        .filter_map(|e| d.pipe_ends(e.handle))
        .filter(|(p, ..)| p.service.is_gravity_drain())
        .map(|(p, a, b)| (p.service, a, b))
        .collect())
}

/// The nearest of `pipes` of `service` to `at`: the foot of `at` on its axis and the distance, drawing units.
fn nearest_drain(pipes: &[(PipeService, Vec2, Vec2)], service: PipeService, at: Vec2) -> Option<(Vec2, f64)> {
    pipes
        .iter()
        .filter(|(sv, ..)| *sv == service)
        .map(|(_, a, b)| {
            let along = (*b - *a).normalized();
            let foot = *a + along * (at - *a).dot(along).clamp(0.0, a.dist(*b));
            (foot, foot.dist(at))
        })
        .min_by(|x, y| x.1.total_cmp(&y.1))
}

fn run_connect(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "fixtureconnect";
    let reach_mm = number(CMD, p, "maxDistance")?.filter(|v| *v > 0.0).unwrap_or(CONNECT_REACH_MM);
    if p.get("maxDistance").is_some_and(|v| !v.is_null()) && number(CMD, p, "maxDistance")?.is_none_or(|v| v <= 0.0) {
        return Err(bad(CMD, "`maxDistance` must be above 0"));
    }
    let slope = number(CMD, p, "slope")?;
    if slope.is_some_and(|i| !(0.0..=cadcraft_doc::MAX_PIPE_SLOPE_PCT).contains(&i)) {
        return Err(bad(CMD, "`slope` is 0 to 100 %"));
    }
    let wanted: Option<Vec<Handle>> = match p.get("fixtures") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) if a.len() <= MAX_FIXTURES => Some(
            a.iter().map(|v| v.as_str().and_then(Handle::parse_hex).ok_or_else(|| bad(CMD, "`fixtures` must be handles"))).collect::<Result<_>>()?,
        ),
        Some(_) => return Err(bad(CMD, format!("`fixtures` must be a list of at most {MAX_FIXTURES} handles"))),
    };
    let fixtures: Vec<(Handle, SanitaryParams, Vec2)> = {
        let d = s.doc()?;
        let space = s.space();
        let all: Vec<Handle> = match &wanted {
            Some(v) => v.clone(),
            None => d
                .space(&space)
                .into_iter()
                .flat_map(|sp| sp.iter())
                .filter(|e| sanitary_of(e).is_some())
                .map(|e| e.handle)
                .take(MAX_FIXTURES)
                .collect(),
        };
        all.into_iter().filter_map(|h| d.sanitary_outlet(h).map(|(f, at)| (h, f, at))).collect()
    };
    let mm = HvacUnits::of(s.doc()?).mm_per_unit;
    let collectors = drain_pipes(s)?;
    let (mut created, mut skipped) = (Vec::new(), Vec::new());
    for (h, f, at) in fixtures {
        let Some(service) = f.drain else { continue };
        let Some((foot, dist)) = nearest_drain(&collectors, service, at).filter(|(_, dist)| dist * mm <= reach_mm) else {
            skipped.push(json!({ "fixture": h.hex(), "reason": "no drain of its kind within reach" }));
            continue;
        };
        if dist * mm < 1.0 {
            skipped.push(json!({ "fixture": h.hex(), "reason": "already on its drain" }));
            continue;
        }
        let mut pp = PipeParams::new(service, f.drain_dn_mm, HvacUnits::of(s.doc()?));
        pp.imposed = true;
        pp.slope_pct = Some(slope.unwrap_or(if service == PipeService::SoilWater { SOIL_SLOPE_PCT } else { WASTE_SLOPE_PCT }));
        PipeParams { length: at.dist(foot), ..pp.clone() }.validate().map_err(doc_err)?;
        let (space, common) = (s.space(), s.current_common());
        let run = s.doc_mut()?.add_pipe(&space, common, pp, at, foot).map_err(doc_err)?;
        created.push(run.hex());
    }
    Ok(json!({ "created": created, "skipped": skipped }))
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    Place,
    Kind,
    Rotation,
    /// The fixture is put at this point and waits for its direction (a second click or an angle).
    Aim(Vec2),
}

struct FixtureAddM {
    last: SanitaryLastUsed,
    ask: Ask,
    /// The point of the fixture the cursor holds (Tab goes round them).
    anchor: super::Anchor,
}

impl FixtureAddM {
    fn new(s: &Session) -> Self {
        FixtureAddM { last: s.last_used.sanitary.clone(), ask: Ask::Place, anchor: super::Anchor::BACK }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!("Current settings: Type = {}, Rotation = {}", self.last.kind.name(), self.last.rotation_deg));
    }
    /// Where the fixture's origin is when the cursor holds its anchor at `at`.
    fn origin(&self, f: &SanitaryParams, at: Vec2, rotation: f64) -> Vec2 {
        self.anchor.origin_for(at, f.units, f.width_mm, f.depth_mm, true, rotation)
    }
    fn params(&self, s: &Session) -> Result<SanitaryParams> {
        Ok(SanitaryParams::new(self.last.kind, HvacUnits::of(s.doc()?)))
    }
}

impl Interactive for FixtureAddM {
    fn name(&self) -> &'static str {
        "FIXTUREADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn cycle_alignment(&mut self, _s: &mut Session) -> Option<String> {
        matches!(self.ask, Ask::Place | Ask::Aim(_)).then(|| {
            self.anchor = self.anchor.next();
            format!("Insertion point = {}", self.anchor.name())
        })
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Kind => Prompt::new("Enter the type of fixture", Accept::TEXT).default(self.last.kind.name()),
            Ask::Rotation => Prompt::new("Specify rotation angle", Accept::NUMBER).default(format!("{}", self.last.rotation_deg)),
            Ask::Place => Prompt::new("Specify the middle of the fixture's back", Accept::POINT).kw(&["Type", "Rotate"]),
            Ask::Aim(at) => Prompt::new("Specify rotation angle", Accept::POINT_OR_NUMBER).base(at).default(format!("{}", self.last.rotation_deg)),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        match (self.ask, i, text) {
            (Ask::Kind, _, Some(t)) => {
                match SanitaryKind::parse(&t) {
                    Some(k) => self.last.kind = k,
                    None => s.echo("Not a type of fixture: see FIXTUREADD."),
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
            (Ask::Place, Input::Keyword(k), _) if k == "Type" => self.ask = Ask::Kind,
            (Ask::Place, Input::Keyword(k), _) if k == "Rotate" => self.ask = Ask::Rotation,
            (Ask::Place, Input::Point(at), _) => self.ask = Ask::Aim(at),
            (Ask::Aim(_), Input::Cancel, _) => self.ask = Ask::Place,
            (Ask::Aim(at), i, _) => {
                if let Some(a) = super::aimed_degrees(&i, at)? {
                    self.last.rotation_deg = a;
                }
                let f = self.params(s)?;
                let turn = self.last.rotation_deg.to_radians();
                match put(s, &f, self.origin(&f, at, turn), turn) {
                    Ok(_) => s.last_used.sanitary = self.last.clone(),
                    Err(e) => s.echo(e.to_string()),
                }
                self.ask = Ask::Place;
            }
            (Ask::Place, Input::Enter | Input::Cancel, _) => return Ok(Step::Done),
            (_, Input::Enter | Input::Cancel, _) => self.ask = Ask::Place,
            _ => {}
        }
        Ok(Step::Continue)
    }
    fn preview_objects(&self, s: &Session, c: Vec2) -> Vec<cadcraft_doc::Entity> {
        let Ok(f) = self.params(s) else { return Vec::new() };
        let (at, deg) = match self.ask {
            Ask::Aim(at) if !c.near(at, 1e-9) => (at, (c - at).angle().to_degrees()),
            Ask::Aim(at) => (at, self.last.rotation_deg),
            _ => (c, self.last.rotation_deg),
        };
        let origin = self.origin(&f, at, deg.to_radians());
        f.generate().map(|pieces| cadcraft_doc::preview_entities(pieces, origin, deg.to_radians())).unwrap_or_default()
    }
}
