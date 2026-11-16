//! Curtain walls in plan: CURTAINADD draws a glazed façade along a path, CURTAINEDIT changes the ones already drawn.

use cadcraft_doc::{CurtainParams, Handle, HvacUnits, curtain_of};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::design::{DesignLastUsed, doc_err, flag, next_name, number, other, parse_number};
use super::{CommandSpec, bad, str_param};

use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

const MAX_OBJECTS: usize = 10_000;
const MAX_POINTS: usize = cadcraft_doc::MAX_DESIGN_VERTICES;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("curtainadd", "Curtain Wall", run_curtainadd)
            .menu(&["Draw", "Architecture", "Curtain Wall"])
            .params(
                "{points (2 or more), height? (mm, default 3000), panel? (mm: the mullions, default 1500), transom? (mm: the transoms, default 1200), depth? (mm: the frame, default 100), \
                 elevation? (m: the foot), name?, labelHeight? (mm)} → {handle, panels, mullions, transoms, area (m²), length (m)}",
            )
            .interactive(|s| Ok(Box::new(CurtainAddM::new(s)))),
        CommandSpec::new("curtainedit", "Edit Curtain Wall", run_curtainedit).params(
            "{handle | handles: curtain walls, name?, height?, panel?, transom?, depth?, elevation? (m), showLabel?} → {changed}",
        ),
    ]
}

fn add_curtain(s: &mut Session, p: CurtainParams) -> Result<(Handle, cadcraft_doc::CurtainCount, f64)> {
    p.validate().map_err(doc_err)?;
    let (count, length) = (p.count(), p.length_mm() / 1000.0);
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_curtain(&space, common, p).map_err(doc_err)?;
    Ok((h, count, length))
}

fn run_curtainadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "curtainadd";
    let raw = p.get("points").and_then(Value::as_array).ok_or_else(|| bad(CMD, "`points` (2 or more) is required"))?;
    if raw.len() > MAX_POINTS {
        return Err(bad(CMD, format!("a curtain wall has at most {MAX_POINTS} points")));
    }
    let pts: Option<Vec<Vec2>> = raw.iter().map(super::point_value).collect();
    let path = super::distinct_points(&pts.ok_or_else(|| bad(CMD, "`points` must be [x, y] points"))?);
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => next_name(s, "MR", |e| curtain_of(e).is_some())?,
    };
    let mut c = CurtainParams::new(&name, path, HvacUnits::of(s.doc()?));
    c.height_mm = number(CMD, p, "height")?.unwrap_or(s.last_used.design.curtain_height_mm);
    c.panel_mm = number(CMD, p, "panel")?.unwrap_or(c.panel_mm);
    c.transom_mm = number(CMD, p, "transom")?.unwrap_or(c.transom_mm);
    c.depth_mm = number(CMD, p, "depth")?.unwrap_or(c.depth_mm);
    c.elevation_mm = number(CMD, p, "elevation")?.map_or(0.0, |m| m * 1000.0);
    c.label_height_mm = number(CMD, p, "labelHeight")?.unwrap_or(c.label_height_mm);
    let (h, n, length) = add_curtain(s, c)?;
    Ok(json!({ "handle": h.hex(), "panels": n.panels, "mullions": n.mullions, "transoms": n.transoms, "area": n.area_m2, "length": length }))
}

fn run_curtainedit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "curtainedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_OBJECTS {
        return Err(bad(CMD, format!("select 1 to {MAX_OBJECTS} curtain walls")));
    }
    let (height, panel, transom, depth) = (number(CMD, p, "height")?, number(CMD, p, "panel")?, number(CMD, p, "transom")?, number(CMD, p, "depth")?);
    let elevation = number(CMD, p, "elevation")?.map(|m| m * 1000.0);
    let show = flag(CMD, p, "showLabel")?;
    let name = str_param(p, "name").map(|n| n.trim().to_string());
    let mut next: Vec<(Handle, CurtainParams)> = Vec::with_capacity(hs.len());
    for h in &hs {
        let e = s.doc()?.entity(*h).ok_or_else(|| other("Cannot find that object."))?;
        let mut c = curtain_of(e).ok_or_else(|| other("Select curtain walls only."))?;
        c.height_mm = height.unwrap_or(c.height_mm);
        c.panel_mm = panel.unwrap_or(c.panel_mm);
        c.transom_mm = transom.unwrap_or(c.transom_mm);
        c.depth_mm = depth.unwrap_or(c.depth_mm);
        c.elevation_mm = elevation.unwrap_or(c.elevation_mm);
        c.show_label = show.unwrap_or(c.show_label);
        if let Some(n) = &name {
            c.name = n.clone();
        }
        c.validate().map_err(doc_err)?;
        next.push((*h, c));
    }
    let d = s.doc_mut()?;
    for (h, c) in &next {
        d.regenerate_curtain(*h, c).map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

// ---------------------------------------------------------------------------------------------------------------
// Interactive form
// ---------------------------------------------------------------------------------------------------------------

struct CurtainAddM {
    last: DesignLastUsed,
    pts: Vec<Vec2>,
    asking_height: bool,
}

impl CurtainAddM {
    fn new(s: &Session) -> Self {
        CurtainAddM { last: s.last_used.design.clone(), pts: Vec::new(), asking_height: false }
    }
    fn finish(&mut self, s: &mut Session) -> Result<Step> {
        let name = next_name(s, "MR", |e| curtain_of(e).is_some())?;
        let mut c = CurtainParams::new(&name, self.pts.clone(), HvacUnits::of(s.doc()?));
        c.height_mm = self.last.curtain_height_mm;
        match add_curtain(s, c) {
            Ok((_, n, length)) => {
                s.last_used.design = self.last.clone();
                s.echo(format!("Curtain wall {name}: {length:.2} m, {} panels, {:.1} m²", n.panels, n.area_m2));
                Ok(Step::Done)
            }
            Err(e) => {
                s.echo(e.to_string());
                Ok(Step::Continue)
            }
        }
    }
}

impl Interactive for CurtainAddM {
    fn name(&self) -> &'static str {
        "CURTAINADD"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.asking_height {
            return Prompt::new("Enter the height of the curtain wall (mm)", Accept::NUMBER).default(format!("{:.0}", self.last.curtain_height_mm));
        }
        match self.pts.len() {
            0 => Prompt::new("Specify start point of the curtain wall", Accept::POINT).kw(&["Height"]),
            _ => Prompt::new("Specify next point", Accept::POINT).kw(&["Undo"]).base_opt(self.pts.last().copied()),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.asking_height {
            if let Input::Text(t) | Input::Keyword(t) = &i {
                match parse_number(t).filter(|v| (300.0..=100_000.0).contains(v)) {
                    Some(v) => self.last.curtain_height_mm = v,
                    None => s.echo("Requires a height of 300 to 100000 mm."),
                }
            }
            self.asking_height = false;
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "Height" => self.asking_height = true,
                    "Undo" => {
                        self.pts.pop();
                    }
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
            Input::Enter if self.pts.len() >= 2 => self.finish(s),
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        let mut pts = self.pts.clone();
        pts.push(c);
        pts.windows(2).map(|w| cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: w[0].to3(0.0), b: w[1].to3(0.0) })).collect()
    }
}
