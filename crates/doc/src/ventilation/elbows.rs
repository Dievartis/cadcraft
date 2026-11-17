//! The elbows of the duct runs, drawn as soon as the runs are laid: where exactly two runs meet end to end and turn, one of them gets the elbow (the
//! arcs across the run, its radius and the length each run gives up to it), without waiting for the sizing, which draws the same elbows later from the
//! network it reads. A joint that is a tee (a third run ends on it or goes through it) or that has a flexible run is not an elbow; a run that moved away
//! loses its elbow. The sizing's own elbows are kept while they still fit the joint.

use cadcraft_geom::Vec2;

use super::network::MAX_DUCTS;
use super::{DuctElbow, DuctParams, HvacUnits, duct_of};
use crate::{EntityKind, Handle, Result, Space};

/// Distance, mm, within which two ends are one point.
const MEET_MM: f64 = 10.0;
/// The turn, degrees, from which a joint is an elbow (a smaller bend is left as a corner), and the turn at which it is a U-turn.
const MIN_TURN_DEG: f64 = 10.0;
const MAX_TURN_DEG: f64 = 170.0;

/// One run: its handle, its data and its two ends.
struct Run {
    handle: Handle,
    params: DuctParams,
    a: Vec2,
    b: Vec2,
}

impl Run {
    fn end(&self, at_end: bool) -> Vec2 {
        if at_end { self.b } else { self.a }
    }

    /// The direction a run leaves the joint at its `at_end` end (away from the joint).
    fn away(&self, at_end: bool) -> Vec2 {
        (self.end(!at_end) - self.end(at_end)).normalized()
    }
}

/// How far `child` turns from `parent` at their joint, degrees, positive to the left of the way the air runs from the parent into the child.
fn turn_deg(parent: &Run, parent_end: bool, child: &Run, child_end: bool) -> f64 {
    let into = -parent.away(parent_end);
    ((child.away(child_end).angle() - into.angle()).to_degrees() + 540.0).rem_euclid(360.0) - 180.0
}

fn near_segment(p: Vec2, a: Vec2, b: Vec2, tol: f64) -> bool {
    let ab = b - a;
    let len2 = ab.len2();
    let t = if len2 > 1e-18 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t) <= tol
}

/// The joints where exactly two runs meet end to end: (run, at its end?) of each.
fn joints(runs: &[Run], tol: f64) -> Vec<[(usize, bool); 2]> {
    let mut out = Vec::new();
    for (i, r) in runs.iter().enumerate() {
        for at_end in [false, true] {
            let p = r.end(at_end);
            let mut partners = Vec::new();
            let mut crossing = false;
            for (k, o) in runs.iter().enumerate() {
                if k == i {
                    continue;
                }
                let ends = [false, true].into_iter().filter(|e| o.end(*e).dist(p) <= tol).collect::<Vec<_>>();
                if ends.is_empty() {
                    // A run going through the point makes a tee.
                    crossing |= near_segment(p, o.a, o.b, tol);
                } else {
                    partners.extend(ends.into_iter().map(|e| (k, e)));
                }
            }
            // Each joint once, from its lower run.
            if let [(k, e)] = partners[..]
                && !crossing
                && i < k
                && runs.get(k).is_some_and(|o| o.a.dist(o.b) > tol)
                && r.a.dist(r.b) > tol
            {
                out.push([(i, at_end), (k, e)]);
            }
        }
    }
    out
}

impl crate::Drawing {
    /// Whether a duct run of `space` was added, changed or removed since `before` (an older state of this drawing).
    pub fn ducts_touched_since(&self, before: &crate::Drawing, space: &Space) -> bool {
        let (Some(now), Some(then)) = (self.space(space), before.space(space)) else { return false };
        let relevant = |e: &std::sync::Arc<crate::Entity>| matches!(e.kind, EntityKind::Insert(_)) && duct_of(e).is_some();
        now.iter().any(|e| then.get(e.handle).is_none_or(|old| !std::sync::Arc::ptr_eq(old, e)) && relevant(e))
            || then.iter().any(|e| now.get(e.handle).is_none() && relevant(e))
    }

    /// Gives the elbows to the runs of `space` that turn where they meet, and takes them from those that no longer do; returns how many runs were redrawn.
    pub fn draw_duct_elbows(&mut self, space: &Space) -> Result<usize> {
        let units = HvacUnits::of(self);
        let tol = units.to_units(MEET_MM);
        let runs: Vec<Run> = self
            .space(space)
            .into_iter()
            .flat_map(|s| s.iter())
            .filter_map(|e| self.duct_ends(e.handle).map(|(params, a, b)| Run { handle: e.handle, params, a, b }))
            .take(MAX_DUCTS + 1)
            .collect();
        if runs.len() > MAX_DUCTS {
            return Ok(0);
        }
        let joints: Vec<[(usize, bool); 2]> =
            joints(&runs, tol).into_iter().filter(|j| j.iter().all(|(k, _)| runs.get(*k).is_some_and(|r| !r.params.flexible))).collect();
        let mut next: Vec<(Option<DuctElbow>, f64, f64)> = runs.iter().map(|r| (r.params.elbow, 0.0, 0.0)).collect();
        // The elbows already there stay while the joint they are at still turns that much.
        let mut holder: Vec<Option<usize>> = vec![None; joints.len()];
        for (n, j) in joints.iter().enumerate() {
            for (side, other) in [(0usize, 1usize), (1, 0)] {
                let (Some(&(c, ce)), Some(&(p, pe))) = (j.get(side), j.get(other)) else { continue };
                let (Some(child), Some(parent)) = (runs.get(c), runs.get(p)) else { continue };
                if let Some(e) = child.params.elbow
                    && e.at_end == ce
                    && (turn_deg(parent, pe, child, ce).abs() - e.angle_deg.abs()).abs() < 0.5
                    && holder.get(n).is_some_and(Option::is_none)
                {
                    holder[n] = Some(side);
                }
            }
        }
        // An elbow at no joint (or one that no longer fits) is dropped.
        for (k, r) in runs.iter().enumerate() {
            let kept = joints.iter().zip(&holder).any(|(j, h)| h.and_then(|s| j.get(s)).is_some_and(|(c, _)| *c == k));
            if r.params.elbow.is_some() && !kept {
                next[k].0 = None;
            }
        }
        // The joints with no elbow yet give one to the run of the two that has none.
        for (n, j) in joints.iter().enumerate() {
            if holder.get(n).copied().flatten().is_some() {
                continue;
            }
            for side in [1usize, 0] {
                let Some(&(c, ce)) = j.get(side) else { continue };
                let Some(&(p, pe)) = j.get(1 - side) else { continue };
                let (Some(child), Some(parent), Some(slot)) = (runs.get(c), runs.get(p), next.get(c)) else { continue };
                if slot.0.is_some() {
                    continue;
                }
                let turn = turn_deg(parent, pe, child, ce);
                if !(MIN_TURN_DEG..=MAX_TURN_DEG).contains(&turn.abs()) {
                    break;
                }
                let width = child.params.plan_width_mm();
                let room = 0.4 * (child.params.length * child.params.units.mm_per_unit).min(parent.params.length * parent.params.units.mm_per_unit);
                let radius = width.min(room / (turn.abs().to_radians() / 2.0).tan());
                if radius.is_finite() && radius > 0.55 * width {
                    next[c].0 = Some(DuctElbow { angle_deg: turn, radius_mm: radius, at_end: ce });
                    holder[n] = Some(side);
                }
                break;
            }
        }
        // Each run gives up to the elbow, at the end of the joint, the length the bend takes.
        for (j, h) in joints.iter().zip(&holder) {
            let Some(&(c, ce)) = h.and_then(|s| j.get(s)) else { continue };
            let Some(&(p, pe)) = h.and_then(|s| j.get(1 - s)) else { continue };
            let Some(t) = next.get(c).and_then(|n| n.0).map(|e| e.tangent_mm()) else { continue };
            for (k, at_end) in [(c, ce), (p, pe)] {
                if let Some(slot) = next.get_mut(k) {
                    if at_end { slot.2 = t } else { slot.1 = t }
                }
            }
        }
        let mut redrawn = 0;
        for (r, (elbow, trim_start, trim_end)) in runs.iter().zip(next) {
            let same = |a: f64, b: f64| (a - b).abs() < 1e-6;
            if r.params.elbow == elbow && same(r.params.trim_start_mm, trim_start) && same(r.params.trim_end_mm, trim_end) {
                continue;
            }
            let mut p = r.params.clone();
            (p.elbow, p.trim_start_mm, p.trim_end_mm) = (elbow, trim_start, trim_end);
            self.regenerate_duct(r.handle, &p)?;
            redrawn += 1;
        }
        Ok(redrawn)
    }
}
