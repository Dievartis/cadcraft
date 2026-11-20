//! Independent positive XY scale with explicit rejection of unsupported geometry.
use super::{CommandSpec, bad, point_req, targets};
use crate::{Accept, Input, Interactive, Prompt, Result, Session, Step};

fn ellipse_scaled(e: &cadcraft_doc::Ellipse, m: &Mat3) -> Result<cadcraft_doc::Ellipse> {
    let u = m.apply_vec(e.major.xy());
    let v = m.apply_vec(e.major.xy().perp() * e.ratio);
    let xx = u.x * u.x + v.x * v.x;
    let yy = u.y * u.y + v.y * v.y;
    let xy = u.x * u.y + v.x * v.y;
    let major_radius = ((xx + yy + (xx - yy).hypot(2.0 * xy)) * 0.5).sqrt();
    let minor_radius = (u.x * v.y - u.y * v.x).abs() / major_radius;
    if !major_radius.is_finite() || !minor_radius.is_finite() || minor_radius <= 0.0 {
        return Err(bad("scalexy", "Ellipse becomes degenerate or exceeds numeric limits"));
    }
    let axis = Vec2::from_angle(0.5 * (2.0 * xy).atan2(xx - yy));
    let center = m.apply(e.center.xy());
    let param = |t: f64| {
        let p = m.apply(e.center.xy() + e.major.xy() * t.cos() + e.major.xy().perp() * (e.ratio * t.sin())) - center;
        cadcraft_geom::norm_angle((p.dot(axis.perp()) / minor_radius).atan2(p.dot(axis) / major_radius))
    };
    let full = (cadcraft_geom::ccw_sweep(e.start, e.end) - std::f64::consts::TAU).abs() < 1e-9;
    Ok(cadcraft_doc::Ellipse {
        center: center.to3(e.center.z),
        major: (axis * major_radius).to3(0.0),
        ratio: minor_radius / major_radius,
        start: if full { 0.0 } else { param(e.start) },
        end: if full { std::f64::consts::TAU } else { param(e.end) },
    })
}
use cadcraft_doc::{EntityKind, Handle};
use cadcraft_geom::{Mat3, Vec2};
use serde_json::{Value, json};
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("scalexy", "Scale X/Y", run)
            .menu(&["Modify", "Scale X/Y..."])
            .params("{handles?, base:[x,y], x:positive factor, y:positive factor}")
            .interactive(|s| {
                let hs = targets(s, &Value::Null)?;
                if hs.is_empty() {
                    return Err(bad("scalexy", "Select objects first"));
                }
                Ok(Box::new(Scale { hs, base: None, x: None }))
            }),
    ]
}
fn factor(p: &Value, key: &str) -> Result<f64> {
    p.get(key)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 1e-6 && *v <= 1e6)
        .ok_or_else(|| bad("scalexy", format!("{key} must be between 0.000001 and 1000000")))
}
fn apply(s: &mut Session, hs: &[Handle], base: Vec2, x: f64, y: f64) -> Result<Value> {
    let m = Mat3::translate(base).then_before(Mat3::scale(x, y)).then_before(Mat3::translate(-base));
    let mut edits = Vec::new();
    for h in hs {
        let e = s.doc()?.entity(*h).ok_or_else(|| bad("scalexy", "Object no longer exists"))?;
        if s.doc()?.layer(&e.common.layer).is_some_and(|l| l.locked) {
            return Err(bad("scalexy", "Selection contains locked objects"));
        }
        let bounds = cadcraft_doc::entity_bounds(s.doc()?, e, 0);
        if !bounds.is_empty() && bounds.corners().iter().any(|p| !m.apply(*p).is_finite()) {
            return Err(bad("scalexy", "Scaled geometry exceeds numeric limits"));
        }
        let mut kind = e.kind.clone();
        match &mut kind {
            EntityKind::Circle(c) => {
                let e = cadcraft_doc::Ellipse {
                    center: c.center,
                    major: Vec2::new(c.radius, 0.0).to3(0.0),
                    ratio: 1.0,
                    start: 0.0,
                    end: std::f64::consts::TAU,
                };
                kind = EntityKind::Ellipse(ellipse_scaled(&e, &m)?);
            }
            EntityKind::Ellipse(e) => {
                kind = EntityKind::Ellipse(ellipse_scaled(e, &m)?);
            }
            EntityKind::Arc(a) => {
                let e = cadcraft_doc::Ellipse { center: a.center, major: Vec2::new(a.radius, 0.0).to3(0.0), ratio: 1.0, start: a.start, end: a.end };
                kind = EntityKind::Ellipse(ellipse_scaled(&e, &m)?);
            }
            EntityKind::LwPolyline(p) => {
                if p.const_width != 0.0 || p.vertices.iter().any(|v| v.bulge != 0.0 || v.start_width != 0.0 || v.end_width != 0.0) {
                    return Err(bad("scalexy", "Curved or wide polylines are not supported yet"));
                }
                kind.transform(&m);
            }
            EntityKind::Line(_)
            | EntityKind::Polyline3d(_)
            | EntityKind::Point(_)
            | EntityKind::Image(_)
            | EntityKind::Spline(_)
            | EntityKind::Solid(_)
            | EntityKind::Trace(_) => kind.transform(&m),
            _ => return Err(bad("scalexy", "Unsupported object: use lines, straight polylines, circles, splines, images or solid faces")),
        }
        edits.push((*h, kind));
    }
    for (h, kind) in edits {
        s.doc_mut()?.modify_entity(h, |e| e.kind = kind)?;
    }
    Ok(json!({"scaled":hs.len()}))
}
fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let hs = targets(s, p)?;
    let base = point_req("scalexy", p, "base")?;
    apply(s, &hs, base, factor(p, "x")?, factor(p, "y")?)
}
struct Scale {
    hs: Vec<Handle>,
    base: Option<Vec2>,
    x: Option<f64>,
}
impl Interactive for Scale {
    fn name(&self) -> &'static str {
        "scalexy"
    }
    fn prompt(&self, _: &Session) -> Prompt {
        if self.base.is_none() {
            Prompt::new("Specify scale base point", Accept::POINT)
        } else {
            Prompt::new(if self.x.is_none() { "Enter X scale factor" } else { "Enter Y scale factor" }, Accept::NUMBER)
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.base.is_none() {
            if let Input::Point(p) = i {
                self.base = Some(p);
            }
            return Ok(Step::Continue);
        }
        if let Input::Text(t) = i {
            let v = t
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 1e-6 && *v <= 1e6)
                .ok_or_else(|| bad("scalexy", "Enter a positive scale factor"))?;
            if let Some(x) = self.x {
                if let Some(base) = self.base {
                    apply(s, &self.hs, base, x, v)?;
                }
                return Ok(Step::Done);
            }
            self.x = Some(v);
        }
        Ok(Step::Continue)
    }
}
