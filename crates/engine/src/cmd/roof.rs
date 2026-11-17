//! Roofs in plan: ROOFADD draws a shed, gable or hip roof over an outline (points, a closed polyline, or a room closed by walls), ROOFEDIT changes the ones
//! already drawn.

use cadcraft_doc::{Handle, HvacUnits, RoofKind, RoofParams, roof_of};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::design::{DesignLastUsed, doc_err, flag, next_name, number, other, parse_number};
use super::{CommandSpec, bad, str_param};

use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

const MAX_OBJECTS: usize = 10_000;
const MAX_POINTS: usize = cadcraft_doc::MAX_DESIGN_VERTICES;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("roofadd", "Roof", run_roofadd)
            .menu(&["Draw", "Architecture", "Roof"])
            .params(
                "{points (3 or more) | handle (a closed polyline) | inside ([x, y]: a point in a room closed by walls), kind? (shed, gable, hip: default gable), pitch? (degrees, \
                 default 30), angle? (degrees: the direction of the ridge, default the first side), elevation? (m: the eaves, default 3), name?, labelHeight? (mm)} \
                 → {handle, planArea (m²), area (m²: the slopes), rise (m), ridge (m)}",
            )
            .interactive(|s| Ok(Box::new(RoofAddM::new(s)))),
        CommandSpec::new("roofedit", "Edit Roof", run_roofedit).params(
            "{handle | handles: roofs, name?, kind?, pitch?, angle?, elevation? (m), showLabel?} → {changed}",
        ),
    ]
}

fn kind_param(cmd: &str, p: &Value) -> Result<Option<RoofKind>> {
    match p.get("kind") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) => RoofKind::parse(t).map(Some).ok_or_else(|| bad(cmd, "`kind` is shed, gable or hip")),
        Some(_) => Err(bad(cmd, "`kind` is shed, gable or hip")),
    }
}

fn add_roof(s: &mut Session, p: RoofParams) -> Result<(Handle, cadcraft_doc::RoofCount)> {
    p.validate().map_err(doc_err)?;
    let count = p.count();
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_roof(&space, common, p).map_err(doc_err)?;
    Ok((h, count))
}

fn run_roofadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "roofadd";
    let mut outline = if let Some(h) = super::curves::handle_param(p, "handle") {
        super::hvac::polyline_outline(s.doc()?, h)?
    } else if let Some(q) = p.get("inside").and_then(super::point_value) {
        super::slab::room_outline_at(s, q)?
    } else if let Some(Value::Array(raw)) = p.get("points") {
        if raw.len() > MAX_POINTS + 1 {
            return Err(bad(CMD, format!("a roof has at most {MAX_POINTS} points")));
        }
        let pts: Option<Vec<Vec2>> = raw.iter().map(super::point_value).collect();
        super::distinct_points(&pts.ok_or_else(|| bad(CMD, "`points` must be [x, y] points"))?)
    } else {
        return Err(bad(CMD, "`points` (3 or more), `handle` (a closed polyline) or `inside` (a point in a room closed by walls) is required"));
    };
    if outline.len() > 3 && outline.first().zip(outline.last()).is_some_and(|(a, b)| a.near(*b, 1e-12)) {
        outline.pop();
    }
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => next_name(s, "T", |e| roof_of(e).is_some())?,
    };
    let last = s.last_used.design.clone();
    let mut r = RoofParams::new(&name, outline, HvacUnits::of(s.doc()?));
    r.kind = kind_param(CMD, p)?.unwrap_or(last.roof_kind);
    r.pitch_deg = number(CMD, p, "pitch")?.unwrap_or(last.roof_pitch_deg);
    r.angle_deg = number(CMD, p, "angle")?.unwrap_or(r.angle_deg);
    r.elevation_mm = number(CMD, p, "elevation")?.map_or(r.elevation_mm, |m| m * 1000.0);
    r.label_height_mm = number(CMD, p, "labelHeight")?.unwrap_or(r.label_height_mm);
    let (h, n) = add_roof(s, r)?;
    Ok(json!({ "handle": h.hex(), "planArea": n.plan_m2, "area": n.area_m2, "rise": n.rise_m, "ridge": n.ridge_m }))
}

fn run_roofedit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "roofedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_OBJECTS {
        return Err(bad(CMD, format!("select 1 to {MAX_OBJECTS} roofs")));
    }
    let (kind, pitch, angle) = (kind_param(CMD, p)?, number(CMD, p, "pitch")?, number(CMD, p, "angle")?);
    let elevation = number(CMD, p, "elevation")?.map(|m| m * 1000.0);
    let show = flag(CMD, p, "showLabel")?;
    let name = str_param(p, "name").map(|n| n.trim().to_string());
    let mut next: Vec<(Handle, RoofParams)> = Vec::with_capacity(hs.len());
    for h in &hs {
        let e = s.doc()?.entity(*h).ok_or_else(|| other("Cannot find that object."))?;
        let mut r = roof_of(e).ok_or_else(|| other("Select roofs only."))?;
        r.kind = kind.unwrap_or(r.kind);
        r.pitch_deg = pitch.unwrap_or(r.pitch_deg);
        r.angle_deg = angle.unwrap_or(r.angle_deg);
        r.elevation_mm = elevation.unwrap_or(r.elevation_mm);
        r.show_label = show.unwrap_or(r.show_label);
        if let Some(n) = &name {
            r.name = n.clone();
        }
        r.validate().map_err(doc_err)?;
        next.push((*h, r));
    }
    let d = s.doc_mut()?;
    for (h, r) in &next {
        d.regenerate_roof(*h, r).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

// ---------------------------------------------------------------------------------------------------------------
// Interactive form
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    Point,
    Walls,
    Kind,
    Pitch,
}

struct RoofAddM {
    last: DesignLastUsed,
    pts: Vec<Vec2>,
    ask: Ask,
}

impl RoofAddM {
    fn new(s: &Session) -> Self {
        RoofAddM { last: s.last_used.design.clone(), pts: Vec::new(), ask: Ask::Point }
    }
    fn finish(&mut self, s: &mut Session, outline: Vec<Vec2>) -> Result<Step> {
        let name = next_name(s, "T", |e| roof_of(e).is_some())?;
        let mut r = RoofParams::new(&name, outline, HvacUnits::of(s.doc()?));
        (r.kind, r.pitch_deg) = (self.last.roof_kind, self.last.roof_pitch_deg);
        match add_roof(s, r) {
            Ok((_, n)) => {
                s.last_used.design = self.last.clone();
                s.echo(format!("Roof {name}: {:.1} m² in plan, {:.1} m² of slopes, rise {:.2} m", n.plan_m2, n.area_m2, n.rise_m));
                Ok(Step::Done)
            }
            Err(e) => {
                s.echo(e.to_string());
                Ok(Step::Continue)
            }
        }
    }
}

impl Interactive for RoofAddM {
    fn name(&self) -> &'static str {
        "ROOFADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        s.echo(format!("Current settings: Kind = {}, Pitch = {:.0}\u{b0}", self.last.roof_kind.name(), self.last.roof_pitch_deg));
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Walls => Prompt::new("Click inside a room closed by walls", Accept::POINT),
            Ask::Kind => Prompt::new("Enter roof kind", super::curves::KW).kw(&["Shed", "Gable", "Hip"]).default(self.last.roof_kind.name()),
            Ask::Pitch => Prompt::new("Enter the pitch (degrees)", Accept::NUMBER).default(format!("{:.0}", self.last.roof_pitch_deg)),
            Ask::Point => match self.pts.len() {
                0 => Prompt::new("Specify first corner of the roof", Accept::POINT).kw(&["Walls", "Kind", "Pitch"]),
                1 | 2 => Prompt::new("Specify next corner", Accept::POINT).kw(&["Undo"]).base_opt(self.pts.last().copied()),
                _ => Prompt::new("Specify next corner", Accept::POINT).kw(&["Close", "Undo"]).base_opt(self.pts.last().copied()),
            },
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.ask, &i) {
            (Ask::Walls, Input::Point(q)) => {
                return match super::slab::room_outline_at(s, *q) {
                    Ok(o) => self.finish(s, o),
                    Err(e) => {
                        s.echo(e.to_string());
                        Ok(Step::Continue)
                    }
                };
            }
            (Ask::Kind, Input::Text(t) | Input::Keyword(t)) => {
                match RoofKind::parse(t) {
                    Some(k) => self.last.roof_kind = k,
                    None => s.echo("Choose Shed, Gable or Hip."),
                }
                self.ask = Ask::Point;
                return Ok(Step::Continue);
            }
            (Ask::Pitch, Input::Text(t) | Input::Keyword(t)) => {
                match parse_number(t).filter(|v| (1.0..=80.0).contains(v)) {
                    Some(v) => self.last.roof_pitch_deg = v,
                    None => s.echo("Requires a pitch of 1 to 80 degrees."),
                }
                self.ask = Ask::Point;
                return Ok(Step::Continue);
            }
            (Ask::Point, _) => {}
            _ => {
                self.ask = Ask::Point;
                return Ok(Step::Continue);
            }
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "Walls" => self.ask = Ask::Walls,
                    "Kind" => self.ask = Ask::Kind,
                    "Pitch" => self.ask = Ask::Pitch,
                    "Undo" => {
                        self.pts.pop();
                    }
                    "Close" if self.pts.len() >= 3 => return self.finish(s, self.pts.clone()),
                    _ => {}
                }
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                if self.pts.len() < MAX_POINTS && !self.pts.last().is_some_and(|l| l.near(p, 1e-9)) {
                    self.pts.push(p);
                }
                Ok(Step::Continue)
            }
            Input::Enter if self.pts.len() >= 3 => self.finish(s, self.pts.clone()),
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        if self.pts.is_empty() || self.ask != Ask::Point {
            return Vec::new();
        }
        let mut pts = self.pts.clone();
        pts.push(c);
        if let Some(first) = self.pts.first() {
            pts.push(*first);
        }
        pts.windows(2)
            .filter_map(|w| Some(cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: w.first()?.to3(0.0), b: w.get(1)?.to3(0.0) })))
            .collect()
    }
}
