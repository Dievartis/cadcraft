//! Windowed node editing restricted to an explicit object selection.
use super::{CommandSpec, bad, point_req, targets};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};
use cadcraft_doc::{EntityKind, Handle};
use cadcraft_geom::{Bounds2, Vec2};
use serde_json::{Value, json};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("vertexedit", "Edit Selected Vertices", run)
            .menu(&["Modify", "Edit Selected Vertices..."])
            .params("{handles?, first:[x,y], second:[x,y], delta:[x,y]} (lines and polylines)")
            .interactive(|s| {
                let hs = checked(s, &Value::Null)?;
                Ok(Box::new(Edit { hs, points: Vec::new() }))
            }),
    ]
}
fn checked(s: &Session, p: &Value) -> Result<Vec<Handle>> {
    let hs = targets(s, p)?;
    if hs.is_empty() {
        return Err(bad("vertexedit", "Select lines or polylines before starting this command"));
    }
    for h in &hs {
        let e = s.doc()?.entity(*h).ok_or_else(|| bad("vertexedit", "Object no longer exists"))?;
        if !matches!(e.kind, EntityKind::Line(_) | EntityKind::LwPolyline(_) | EntityKind::Polyline3d(_)) {
            return Err(bad("vertexedit", "Only lines and polylines are supported"));
        }
        if s.doc()?.layer(&e.common.layer).is_some_and(|l| l.locked) {
            return Err(bad("vertexedit", "Selection contains a locked layer"));
        }
    }
    Ok(hs)
}
fn apply(s: &mut Session, hs: &[Handle], area: Bounds2, delta: Vec2) -> Result<Value> {
    if !delta.is_finite() || !area.min.is_finite() || !area.max.is_finite() {
        return Err(bad("vertexedit", "Points must be finite"));
    }
    let mut count = 0usize;
    let mut edits = Vec::new();
    for h in hs {
        let mut e = (**s.doc()?.entity(*h).ok_or_else(|| bad("vertexedit", "Object no longer exists"))?).clone();
        let mut move3 = |p: &mut cadcraft_geom::Vec3| {
            if area.contains(p.xy()) {
                p.x += delta.x;
                p.y += delta.y;
                count += 1;
            }
        };
        match &mut e.kind {
            EntityKind::Line(l) => {
                move3(&mut l.a);
                move3(&mut l.b);
            }
            EntityKind::Polyline3d(p) => p.points.iter_mut().for_each(move3),
            EntityKind::LwPolyline(p) => {
                for v in &mut p.vertices {
                    if area.contains(v.p) {
                        v.p += delta;
                        count += 1;
                    }
                }
            }
            _ => {}
        }
        if e.kind.grips().iter().any(|p| !p.is_finite()) {
            return Err(bad("vertexedit", "Displacement exceeds numeric limits"));
        }
        edits.push((*h, e.kind));
    }
    for (h, kind) in edits {
        s.doc_mut()?.modify_entity(h, |entity| entity.kind = kind)?;
    }
    Ok(json!({"vertices":count}))
}
fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = checked(s, p)?;
    let a = point_req("vertexedit", p, "first")?;
    let b = point_req("vertexedit", p, "second")?;
    let delta = point_req("vertexedit", p, "delta")?;
    apply(s, &hs, Bounds2::new(a, b), delta)
}
struct Edit {
    hs: Vec<Handle>,
    points: Vec<Vec2>,
}
impl Interactive for Edit {
    fn name(&self) -> &'static str {
        "vertexedit"
    }
    fn prompt(&self, _: &Session) -> Prompt {
        let text = match self.points.len() {
            0 => "Specify first corner of vertex window",
            1 => "Specify opposite corner of vertex window",
            2 => "Specify displacement base point",
            _ => "Specify displacement destination",
        };
        Prompt::new(text, Accept::POINT).base_opt(self.points.last().copied())
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Input::Point(p) = i {
            self.points.push(p);
            if let [a, b, from, to] = self.points.as_slice() {
                checked(s, &json!({"handles":self.hs.iter().map(|h|h.hex()).collect::<Vec<_>>()}))?;
                let result = apply(s, &self.hs, Bounds2::new(*a, *b), *to - *from)?;
                s.echo(format!("Vertices moved: {}", result["vertices"]));
                return Ok(Step::Done);
            }
        }
        Ok(Step::Continue)
    }
    fn preview(&self, _: &Session, cursor: Vec2) -> Vec<EntityKind> {
        if self.points.len() == 1 {
            let a = self.points[0];
            let b = cursor;
            let corners = [a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y), a];
            return corners.windows(2).map(|w| super::helpers::line(w[0], w[1])).collect();
        }
        Vec::new()
    }
}
