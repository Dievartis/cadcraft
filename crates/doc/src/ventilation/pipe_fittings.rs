//! The fittings of a pipework drawing, read from where the pipes meet and cross: an elbow where two pipes of one service turn, a tee where a
//! third joins, a cross where four do, a reducer where two pipes in line differ in size, and a crossover where two pipes cross in plan away
//! from their ends. Nothing is stored: the fittings follow the geometry, as the networks do.

use cadcraft_geom::Vec2;

use super::{PipeLeg, PipeParams, PipeService};
use crate::Handle;

/// Distance, in millimetres, within which an end is on a point or on a pipe.
const MEET_MM: f64 = 10.0;
/// Most pipes read: a drawing with more is read up to this many.
pub const MAX_FITTING_PIPES: usize = 4000;
/// A bend whose angle is within this many degrees of straight is not a bend.
const STRAIGHT_DEG: f64 = 3.0;

/// What a fitting is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeFittingKind {
    Elbow90,
    Elbow45,
    /// A bend of another angle.
    ElbowOther,
    Tee,
    Cross,
    /// Five pipes or more on a point.
    Manifold,
    Reducer,
    /// Two pipes crossing in plan, away from their ends.
    Crossover,
}

impl PipeFittingKind {
    /// The name the quantities give it.
    pub fn name(self) -> &'static str {
        match self {
            PipeFittingKind::Elbow90 => "Pipe elbow 90°",
            PipeFittingKind::Elbow45 => "Pipe elbow 45°",
            PipeFittingKind::ElbowOther => "Pipe elbow, other angle",
            PipeFittingKind::Tee => "Pipe tee",
            PipeFittingKind::Cross => "Pipe cross",
            PipeFittingKind::Manifold => "Pipe junction, 5 or more",
            PipeFittingKind::Reducer => "Pipe reducer",
            PipeFittingKind::Crossover => "Pipe crossover",
        }
    }
}

/// One fitting: its kind, where it is, the service and leg of the pipes it joins, and its size (the larger size, or both for a reducer or a crossover).
#[derive(Clone, Debug, PartialEq)]
pub struct PipeFitting {
    pub kind: PipeFittingKind,
    pub at: Vec2,
    pub service: PipeService,
    pub leg: PipeLeg,
    pub size: String,
    /// The pipes it joins or crosses.
    pub pipes: Vec<Handle>,
}

struct P {
    h: Handle,
    p: PipeParams,
    a: Vec2,
    b: Vec2,
    /// Millimetres in a drawing unit.
    mm: f64,
}

struct Node {
    at: Vec2,
    /// The pipes on the point, each with the direction it leaves the point.
    members: Vec<(usize, Vec2)>,
}

fn same_run(a: &PipeParams, b: &PipeParams) -> bool {
    a.service == b.service && (!a.service.has_legs() || a.leg == b.leg)
}

/// The distance from `p` to segment `a`–`b` and how far along it is, 0 to 1 (drawing units).
fn on_segment(p: Vec2, a: Vec2, b: Vec2) -> (f64, f64) {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 < 1e-12 {
        return (p.dist(a), 0.0);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (p.dist(a + ab * t), t)
}

fn size_rank(p: &PipeParams) -> f64 {
    // The liquid and the gas tubes of a link are sized together: the largest tube stands for the link.
    p.dn_mm.max(p.gas_dn_mm).max(p.gas2_dn_mm)
}

impl crate::Drawing {
    /// The fittings of the pipes of model space: elbows, tees, crosses, reducers and crossovers.
    pub fn pipe_fittings(&self) -> Vec<PipeFitting> {
        let pipes: Vec<P> = self
            .model
            .iter()
            .filter_map(|e| self.pipe_ends(e.handle).map(|(p, a, b)| P { h: e.handle, mm: p.units.mm_per_unit, p, a, b }))
            .take(MAX_FITTING_PIPES)
            .collect();
        let mut nodes: Vec<Node> = Vec::new();
        // The ends: pipes of one run whose ends are on one point share a node.
        for (i, pi) in pipes.iter().enumerate() {
            for (at, other) in [(pi.a, pi.b), (pi.b, pi.a)] {
                let dir = (other - at).normalized();
                if dir == Vec2::ZERO {
                    continue;
                }
                let tol = MEET_MM / pi.mm.max(1e-9);
                match nodes
                    .iter_mut()
                    .find(|n| n.at.dist(at) <= tol && n.members.first().is_some_and(|(m, _)| pipes.get(*m).is_some_and(|q| same_run(&q.p, &pi.p))))
                {
                    Some(n) => n.members.push((i, dir)),
                    None => nodes.push(Node { at, members: vec![(i, dir)] }),
                }
            }
        }
        // A pipe that ends against the middle of another of its run is a branch: the other pipe's two ways join the node.
        for (i, pi) in pipes.iter().enumerate() {
            for at in [pi.a, pi.b] {
                let tol = MEET_MM / pi.mm.max(1e-9);
                for (j, pj) in pipes.iter().enumerate() {
                    if i == j || !same_run(&pi.p, &pj.p) {
                        continue;
                    }
                    let (d, t) = on_segment(at, pj.a, pj.b);
                    let len = pj.a.dist(pj.b);
                    if d <= tol && t * len > tol && (1.0 - t) * len > tol {
                        let ways = [((pj.a - at).normalized()), ((pj.b - at).normalized())];
                        let node = match nodes.iter_mut().find(|n| n.at.dist(at) <= tol && n.members.iter().any(|(m, _)| *m == i)) {
                            Some(n) => n,
                            None => continue,
                        };
                        if !node.members.iter().any(|(m, _)| *m == j) {
                            node.members.extend(ways.into_iter().map(|w| (j, w)));
                        }
                    }
                }
            }
        }
        let mut out: Vec<PipeFitting> = Vec::new();
        for n in &nodes {
            if let Some(f) = classify(&pipes, n) {
                out.push(f);
            }
        }
        out.extend(crossovers(&pipes));
        out
    }
}

/// The fitting on a node, if its pipes make one.
fn classify(pipes: &[P], n: &Node) -> Option<PipeFitting> {
    let mut ids: Vec<usize> = n.members.iter().map(|(m, _)| *m).collect();
    ids.sort_unstable();
    ids.dedup();
    let first = pipes.get(*ids.first()?)?;
    let biggest = ids.iter().filter_map(|i| pipes.get(*i)).max_by(|a, b| size_rank(&a.p).total_cmp(&size_rank(&b.p)))?;
    let handles: Vec<Handle> = ids.iter().filter_map(|i| pipes.get(*i).map(|q| q.h)).collect();
    let make = |kind, size: String| PipeFitting { kind, at: n.at, service: first.p.service, leg: first.p.leg, size, pipes: handles.clone() };
    match n.members.len() {
        0 | 1 => None,
        2 => {
            let (da, db) = (n.members.first()?.1, n.members.get(1)?.1);
            // The bend: 0° when the two ways are opposite (in line).
            let bend = 180.0 - da.dot(db).clamp(-1.0, 1.0).acos().to_degrees();
            if bend <= STRAIGHT_DEG {
                let (qa, qb) = (pipes.get(n.members.first()?.0)?, pipes.get(n.members.get(1)?.0)?);
                if qa.p.size_text() == qb.p.size_text() {
                    return None;
                }
                let (big, small) = if size_rank(&qa.p) >= size_rank(&qb.p) { (qa, qb) } else { (qb, qa) };
                return Some(make(PipeFittingKind::Reducer, format!("{} → {}", big.p.size_text(), small.p.size_text())));
            }
            let kind = if (bend - 90.0).abs() <= 10.0 {
                PipeFittingKind::Elbow90
            } else if (bend - 45.0).abs() <= 10.0 {
                PipeFittingKind::Elbow45
            } else {
                PipeFittingKind::ElbowOther
            };
            Some(make(kind, biggest.p.size_text()))
        }
        3 => Some(make(PipeFittingKind::Tee, biggest.p.size_text())),
        4 => Some(make(PipeFittingKind::Cross, biggest.p.size_text())),
        _ => Some(make(PipeFittingKind::Manifold, biggest.p.size_text())),
    }
}

/// Where two pipes cross in plan, away from the ends of both.
fn crossovers(pipes: &[P]) -> Vec<PipeFitting> {
    let mut out = Vec::new();
    for (i, a) in pipes.iter().enumerate() {
        for b in pipes.iter().skip(i + 1) {
            let (r, s) = (a.b - a.a, b.b - b.a);
            let denom = r.cross(s);
            if denom.abs() < 1e-12 {
                continue;
            }
            let t = (b.a - a.a).cross(s) / denom;
            let u = (b.a - a.a).cross(r) / denom;
            let (ta, tb) = (MEET_MM / a.mm.max(1e-9) / r.len().max(1e-9), MEET_MM / b.mm.max(1e-9) / s.len().max(1e-9));
            if t > ta && t < 1.0 - ta && u > tb && u < 1.0 - tb {
                let sizes = if size_rank(&a.p) >= size_rank(&b.p) {
                    format!("{} × {}", a.p.size_text(), b.p.size_text())
                } else {
                    format!("{} × {}", b.p.size_text(), a.p.size_text())
                };
                out.push(PipeFitting {
                    kind: PipeFittingKind::Crossover,
                    at: a.a + r * t,
                    service: a.p.service,
                    leg: a.p.leg,
                    size: sizes,
                    pipes: vec![a.h, b.h],
                });
            }
        }
    }
    out
}
