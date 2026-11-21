//! Building sections: SECTIONADD cuts the drawing along a line and draws what it cuts through, seen straight on, beside the plan.

use cadcraft_doc::SECTION_LAYER;
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::design::doc_err;
use super::{CommandSpec, bad, str_param};

use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("sectionadd", "Building Section", run_sectionadd)
            .menu(&["Draw", "Architecture", "Building Section"])
            .params(
                "{a ([x, y]), b ([x, y]): the cut line (or points: [[x, y], [x, y]]), name? (a letter, default A, B…), at? ([x, y]: where the ground level of the picture goes, \
                 default to the right of the drawing)} → {handle, width, walls, openings, slabs, columns, beams, ceilings}: draws the walls with their openings, the slabs, the steel \
                 columns and beams and the false ceilings the line cuts through, each at its elevation, with the levels; a picture made now, it does not follow later changes",
            )
            .interactive(|_| Ok(Box::new(SectionAddM { a: None, b: None }))),
    ]
}

/// The letter the next section takes: A for the first, then B…
fn next_name(s: &Session) -> Result<String> {
    let n = s.doc()?.model.iter().filter(|e| e.common.layer == SECTION_LAYER).count();
    Ok(char::from(b'A' + u8::try_from(n % 26).unwrap_or(0)).to_string())
}

fn point(p: &Value, key: &str) -> Option<Vec2> {
    p.get(key).and_then(super::point_value)
}

fn draw(s: &mut Session, a: Vec2, b: Vec2, name: Option<&str>, at: Option<Vec2>) -> Result<Value> {
    let name = match name {
        Some(n) => n.to_string(),
        None => next_name(s)?,
    };
    let (space, common) = (s.space(), s.current_common());
    let (h, n, width) = s.doc_mut()?.add_section(&space, common, a, b, &name, at).map_err(doc_err)?;
    Ok(json!({
        "handle": h.hex(), "width": width, "walls": n.walls, "openings": n.openings, "slabs": n.slabs, "columns": n.columns, "beams": n.beams, "ceilings": n.ceilings, "roofs": n.roofs,
        "ducts": n.ducts, "terminals": n.terminals, "equipment": n.equipment, "pipes": n.pipes
    }))
}

fn run_sectionadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "sectionadd";
    let pts = p.get("points").and_then(Value::as_array).map(|v| v.iter().map(super::point_value).collect::<Option<Vec<_>>>());
    let (a, b) = match (point(p, "a"), point(p, "b"), pts) {
        (Some(a), Some(b), _) => (a, b),
        (_, _, Some(Some(v))) if v.len() == 2 => (v[0], v[1]),
        _ => return Err(bad(CMD, "`a` and `b` (the cut line's ends, [x, y]) are required")),
    };
    if p.get("at").is_some() && point(p, "at").is_none() {
        return Err(bad(CMD, "`at` must be an [x, y] point"));
    }
    draw(s, a, b, str_param(p, "name"), point(p, "at"))
}

struct SectionAddM {
    a: Option<Vec2>,
    b: Option<Vec2>,
}

impl Interactive for SectionAddM {
    fn name(&self) -> &'static str {
        "SECTIONADD"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match (self.a, self.b) {
            (None, _) => Prompt::new("Specify first point of the cut line", Accept::POINT),
            (Some(a), None) => Prompt::new("Specify second point of the cut line", Accept::POINT).base_opt(Some(a)),
            (Some(_), Some(_)) => Prompt::new("Specify where the section goes (Enter: to the right of the drawing)", Accept::POINT),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        match (self.a, self.b, i) {
            (None, _, Input::Point(p)) => self.a = Some(p),
            (Some(a), None, Input::Point(p)) => {
                if a.near(p, 1e-9) {
                    s.echo("Specify a point away from the first.");
                } else {
                    self.b = Some(p);
                }
            }
            (Some(a), Some(b), Input::Point(p)) => return finish(s, a, b, Some(p)),
            (Some(a), Some(b), Input::Enter) => return finish(s, a, b, None),
            (_, _, Input::Enter | Input::Cancel) => return Ok(Step::Cancel),
            _ => {}
        }
        Ok(Step::Continue)
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        match (self.a, self.b) {
            (Some(a), None) => vec![cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: a.to3(0.0), b: c.to3(0.0) })],
            _ => Vec::new(),
        }
    }
}

fn finish(s: &mut Session, a: Vec2, b: Vec2, at: Option<Vec2>) -> Result<Step> {
    match draw(s, a, b, None, at) {
        Ok(r) => {
            let n = |k: &str| r.get(k).and_then(Value::as_u64).unwrap_or(0);
            s.echo(format!(
                "Section: {} walls, {} openings, {} slabs, {} columns, {} beams, {} ceilings, {} roofs cut",
                n("walls"),
                n("openings"),
                n("slabs"),
                n("columns"),
                n("beams"),
                n("ceilings"),
                n("roofs")
            ));
            // The HVAC objects the line goes through, when there are some.
            if n("ducts") + n("terminals") + n("equipment") + n("pipes") > 0 {
                s.echo(format!(
                    "Section: {} ducts, {} air terminals, {} units and equipment, {} pipes crossed",
                    n("ducts"),
                    n("terminals"),
                    n("equipment"),
                    n("pipes")
                ));
            }
            Ok(Step::Done)
        }
        Err(e) => {
            s.echo(e.to_string());
            Ok(Step::Continue)
        }
    }
}
