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
                Ok(Box::new(Edit { hs, points: Vec::new(), selected: std::collections::BTreeSet::new(), confirmed: false, toggle: false }))
            }),
    ]
}
fn checked(s: &Session, p: &Value) -> Result<Vec<Handle>> {
    let hs = targets(s, p)?;
    if hs.is_empty() {
        return Err(bad("vertexedit", "Select lines or polylines before starting this command"));
    }
    let mut vertex_count = 0usize;
    for h in &hs {
        let e = s.doc()?.entity(*h).ok_or_else(|| bad("vertexedit", "Object no longer exists"))?;
        if !matches!(e.kind, EntityKind::Line(_) | EntityKind::LwPolyline(_) | EntityKind::Polyline3d(_)) {
            return Err(bad("vertexedit", "Only lines and polylines are supported"));
        }
        if s.doc()?.layer(&e.common.layer).is_some_and(|l| l.locked) {
            return Err(bad("vertexedit", "Selection contains a locked layer"));
        }
        // Bound the handles drawn and hit-tested on every interactive frame.
        vertex_count = vertex_count.saturating_add(match &e.kind {
            EntityKind::Line(_) => 2,
            EntityKind::LwPolyline(p) => p.vertices.len(),
            EntityKind::Polyline3d(p) => p.points.len(),
            _ => 0,
        });
        if vertex_count > 50_000 {
            return Err(bad("vertexedit", "Select at most 50000 vertices per operation"));
        }
    }
    Ok(hs)
}
fn apply(s: &mut Session, hs: &[Handle], delta: Vec2, inside: impl Fn(Handle, usize, Vec2) -> bool) -> Result<Value> {
    if !delta.is_finite() {
        return Err(bad("vertexedit", "Points must be finite"));
    }
    let mut count = 0usize;
    let mut edits = Vec::new();
    for h in hs {
        let mut e = (**s.doc()?.entity(*h).ok_or_else(|| bad("vertexedit", "Object no longer exists"))?).clone();
        let mut move3 = |index: usize, p: &mut cadcraft_geom::Vec3| {
            if inside(*h, index, p.xy()) {
                p.x += delta.x;
                p.y += delta.y;
                count += 1;
            }
        };
        match &mut e.kind {
            EntityKind::Line(l) => {
                move3(0, &mut l.a);
                move3(1, &mut l.b);
            }
            EntityKind::Polyline3d(p) => p.points.iter_mut().enumerate().for_each(|(i, p)| move3(i, p)),
            EntityKind::LwPolyline(p) => {
                for (index, v) in p.vertices.iter_mut().enumerate() {
                    if inside(*h, index, v.p) {
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
    let area = Bounds2::new(a, b);
    apply(s, &hs, delta, |_, _, p| area.contains(p))
}
struct Edit {
    hs: Vec<Handle>,
    points: Vec<Vec2>,
    selected: std::collections::BTreeSet<(Handle, usize)>,
    confirmed: bool,
    toggle: bool,
}
fn nodes(s: &Session, hs: &[Handle]) -> Vec<(Handle, usize, Vec2)> {
    let mut out = Vec::new();
    for h in hs {
        if let Ok(d) = s.doc()
            && let Some(e) = d.entity(*h)
        {
            let points: Vec<_> = match &e.kind {
                EntityKind::Line(l) => vec![l.a.xy(), l.b.xy()],
                EntityKind::LwPolyline(p) => p.vertices.iter().map(|v| v.p).collect(),
                EntityKind::Polyline3d(p) => p.points.iter().map(|p| p.xy()).collect(),
                _ => Vec::new(),
            };
            out.extend(points.into_iter().enumerate().map(|(index, p)| (*h, index, p)));
        }
    }
    out
}
impl Interactive for Edit {
    fn vertex_markers(&self, s: &Session) -> Vec<(Vec2, bool)> {
        nodes(s, &self.hs).into_iter().map(|(h, i, p)| (p, self.selected.contains(&(h, i)))).collect()
    }
    fn name(&self) -> &'static str {
        "vertexedit"
    }
    fn prompt(&self, _: &Session) -> Prompt {
        let text = if !self.confirmed && self.points.len() >= 2 {
            "Select more vertices with Shift-click or another window; Enter to confirm"
        } else {
            match self.points.len() {
                0 => "Specify first corner of vertex window",
                1 => "Specify opposite corner of vertex window",
                2 => "Specify displacement base point",
                _ => "Specify displacement destination",
            }
        };
        Prompt::new(text, Accept::POINT).kw(&["Toggle"]).base_opt(self.points.last().copied())
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if let Input::Keyword(k) = &i
            && k == "Toggle"
            && !self.confirmed
        {
            self.toggle = true;
            return Ok(Step::Continue);
        }
        if matches!(i, Input::Enter) && !self.confirmed {
            if self.selected.is_empty() {
                s.echo("Select at least one vertex");
            } else {
                self.confirmed = true;
                self.points.truncate(2);
                if self.points.len() < 2 {
                    self.points = vec![Vec2::ZERO, Vec2::ZERO];
                }
            }
            return Ok(Step::Continue);
        }
        if let Input::Point(p) = i {
            if !self.confirmed && self.toggle {
                self.toggle = false;
                let aperture = s.pixel_size() * s.settings.pickbox.max(1.0) * 1.5;
                if let Some((h, index, _)) =
                    nodes(s, &self.hs).into_iter().filter(|(_, _, q)| q.dist(p) <= aperture).min_by(|a, b| a.2.dist(p).total_cmp(&b.2.dist(p)))
                    && !self.selected.remove(&(h, index))
                {
                    self.selected.insert((h, index));
                }
                return Ok(Step::Continue);
            }
            if !self.confirmed {
                if self.points.len() >= 2 {
                    self.points.clear();
                }
                self.points.push(p);
                if let [a, b] = self.points.as_slice() {
                    let area = Bounds2::new(*a, *b);
                    self.selected.extend(nodes(s, &self.hs).into_iter().filter(|(_, _, q)| area.contains(*q)).map(|(h, i, _)| (h, i)));
                }
                return Ok(Step::Continue);
            }
            if self.points.len() < 2 {
                self.points = vec![Vec2::ZERO, Vec2::ZERO];
            }
            self.points.push(p);
            if let [a, b, from, to] = self.points.as_slice() {
                checked(s, &json!({"handles":self.hs.iter().map(|h|h.hex()).collect::<Vec<_>>()}))?;
                let _ = (a, b);
                let result = apply(s, &self.hs, *to - *from, |h, index, _| self.selected.contains(&(h, index)))?;
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
