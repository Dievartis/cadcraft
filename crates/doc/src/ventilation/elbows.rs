//! The elbows and reducers of the duct runs, drawn as soon as the runs are laid: where exactly two runs meet end to end and turn, one of them gets the
//! elbow (the arcs across the run, its radius and the length each run gives up to it), without waiting for the sizing, which draws the same elbows later
//! from the network it reads. A joint that is a tee (a third run ends on it or goes through it) or that has a flexible run is not an elbow; a run that moved
//! away loses its elbow. The sizing's own elbows are kept while they still fit the joint.
//!
//! Where two runs of another width meet (end to end, or a run ending on the side of another) the one that joins the other gets a reducer, the cone from the
//! other's width; it is drawn again when either run changes size or shape, and taken away when the widths are equal again. A reducer the sizing drew stays on
//! the run that has it. Where two runs at other altitudes meet end to end, the later one gets the step between them (two 45° elbows) as soon as both have an
//! altitude, and loses it when they are at one again.
//! At a joint that has an elbow the cone is on the other run (the elbow is drawn at its own run's width, and the cone goes from that
//! width to the other run's), just before the bend.

use cadcraft_geom::Vec2;

use super::duct::LEVEL_STEP_MIN_MM;
use super::network::{MAX_DUCTS, STEP_CLEARANCE_MM};
use super::{
    AirDirection, AirPort, DuctElbow, DuctParams, DuctReducer, DuctShape, DuctStep, HvacUnits, RiserSize, ahu_of, air_terminal_of, duct_of,
    equipment_of, riser_of,
};
use crate::{EntityKind, Handle, Result, Space};

/// Distance, mm, within which two ends are one point.
const MEET_MM: f64 = 10.0;
/// The turn, degrees, from which a joint is an elbow (a smaller bend is left as a corner), and the turn at which it is a U-turn.
const MIN_TURN_DEG: f64 = 10.0;
const MAX_TURN_DEG: f64 = 170.0;

/// What the elbows and the reducers need to know of a run, which is a duct or a pipe.
trait RunParams {
    /// The radius of the axis through a bend, as a multiple of the run's width.
    const ELBOW_RADIUS_PER_WIDTH: f64;
    /// The width a plan shows, mm.
    fn width_mm(&self) -> f64;
    fn length_mm(&self) -> f64;
    fn elbow(&self) -> Option<DuctElbow>;
    fn reducer(&self) -> Option<DuctReducer>;
    /// Drawing units per mm, and whether a narrower run laid along one side of a wider one (their axes apart, a side in line) joins it with a cone
    /// that is flat on that side (an eccentric reducer): ducts only.
    fn units_per_mm(&self) -> f64;
    const ECCENTRIC: bool = false;
    /// A flexible run takes no elbow and no cone.
    fn flexible(&self) -> bool {
        false
    }
    /// The air direction a duct was given, when it was set (a collar of a unit sized for it).
    fn air_role(&self) -> Option<AirDirection> {
        None
    }
    /// Whether `other` is of the same network: only runs of one network meet in an elbow or a cone.
    fn same_network(&self, _other: &Self) -> bool {
        true
    }
}

impl RunParams for DuctParams {
    /// Ducts of two air directions are two networks (stacked in a unit, or crossing at another height), unless one has no direction yet.
    fn same_network(&self, other: &Self) -> bool {
        !(self.air_set && other.air_set) || self.air == other.air
    }

    const ELBOW_RADIUS_PER_WIDTH: f64 = 1.0;
    const ECCENTRIC: bool = true;
    fn units_per_mm(&self) -> f64 {
        1.0 / self.units.mm_per_unit
    }
    fn width_mm(&self) -> f64 {
        self.plan_width_mm()
    }
    fn length_mm(&self) -> f64 {
        self.length * self.units.mm_per_unit
    }
    fn elbow(&self) -> Option<DuctElbow> {
        self.elbow
    }
    fn reducer(&self) -> Option<DuctReducer> {
        self.reducer
    }
    fn flexible(&self) -> bool {
        self.flexible
    }
    fn air_role(&self) -> Option<AirDirection> {
        self.air_set.then_some(self.air)
    }
}

impl RunParams for super::PipeParams {
    // A short-radius pipe elbow.
    const ELBOW_RADIUS_PER_WIDTH: f64 = 1.5;
    fn units_per_mm(&self) -> f64 {
        1.0 / self.units.mm_per_unit
    }
    fn width_mm(&self) -> f64 {
        self.plan_width_mm()
    }
    fn length_mm(&self) -> f64 {
        self.length * self.units.mm_per_unit
    }
    fn elbow(&self) -> Option<DuctElbow> {
        self.elbow
    }
    fn reducer(&self) -> Option<DuctReducer> {
        self.reducer
    }
    fn same_network(&self, other: &Self) -> bool {
        self.service == other.service && (!self.service.has_legs() || self.leg == other.leg)
    }
}

/// One run: its handle, its data and its two ends.
struct Run<P = DuctParams> {
    handle: Handle,
    params: P,
    a: Vec2,
    b: Vec2,
}

impl<P> Run<P> {
    fn end(&self, at_end: bool) -> Vec2 {
        if at_end { self.b } else { self.a }
    }

    /// The direction a run leaves the joint at its `at_end` end (away from the joint).
    fn away(&self, at_end: bool) -> Vec2 {
        (self.end(!at_end) - self.end(at_end)).normalized()
    }
}

/// How far `child` turns from `parent` at their joint, degrees, positive to the left of the way the air runs from the parent into the child.
fn turn_deg<P>(parent: &Run<P>, parent_end: bool, child: &Run<P>, child_end: bool) -> f64 {
    let into = -parent.away(parent_end);
    ((child.away(child_end).angle() - into.angle()).to_degrees() + 540.0).rem_euclid(360.0) - 180.0
}

/// The radius of the elbow `child` makes where it turns `turn` degrees from `parent`: its width, or less where the runs are short; `None` when a bend that
/// tight would not fit the width (the joint is left as a corner).
fn elbow_radius<P: RunParams>(child: &Run<P>, parent: &Run<P>, turn: f64) -> Option<f64> {
    let width = child.params.width_mm();
    let wanted = P::ELBOW_RADIUS_PER_WIDTH * width;
    let room = 0.4 * child.params.length_mm().min(parent.params.length_mm());
    let radius = wanted.min(room / (turn.abs().to_radians() / 2.0).tan());
    (radius.is_finite() && radius > 0.55 * width).then_some(radius)
}

fn near_segment(p: Vec2, a: Vec2, b: Vec2, tol: f64) -> bool {
    let ab = b - a;
    let len2 = ab.len2();
    let t = if len2 > 1e-18 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t) <= tol
}

/// How far `o`'s end `o_end` is across `r`'s axis from `r`'s end `at_end`, drawing units, when the two runs go on from each other in line but their axes
/// are apart, the narrower run inside the width of the wider one (laid along one of its sides): an eccentric reducer joins them. `None` otherwise.
fn eccentric_offset<P: RunParams>(r: &Run<P>, at_end: bool, o: &Run<P>, o_end: bool, tol: f64) -> Option<f64> {
    let (p, q, dir) = (r.end(at_end), o.end(o_end), r.away(at_end));
    let d = q - p;
    let across = d.cross(dir);
    let room = (r.params.width_mm() - o.params.width_mm()).abs() / 2.0 * r.params.units_per_mm();
    (o.away(o_end).dot(dir) < -0.999 && d.dot(dir).abs() <= tol && across.abs() > tol && across.abs() <= room + tol).then_some(across)
}

/// The joints where exactly two runs of one network meet end to end: (run, at its end?) of each. A run of another network (an extract duct stacked under a
/// supply one, starting at the same point of a unit; a run of another air going through) is not part of the joint.
fn joints<P: RunParams>(runs: &[Run<P>], tol: f64) -> Vec<[(usize, bool); 2]> {
    let mut out = Vec::new();
    for (i, r) in runs.iter().enumerate() {
        for at_end in [false, true] {
            let p = r.end(at_end);
            let mut partners = Vec::new();
            let mut crossing = false;
            for (k, o) in runs.iter().enumerate() {
                if k == i || !r.params.same_network(&o.params) {
                    continue;
                }
                let mut ends = [false, true].into_iter().filter(|e| o.end(*e).dist(p) <= tol).collect::<Vec<_>>();
                if ends.is_empty() && P::ECCENTRIC {
                    ends.extend([false, true].into_iter().filter(|e| eccentric_offset(r, at_end, o, *e, tol).is_some()));
                }
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

/// The steps each run has once the elbows are decided: where two runs meet end to end at other altitudes, the later one has the step between them, at that
/// end, on the straight part (not over an elbow); a step there that was written for the joint on either run is replaced. Steps elsewhere are left.
fn steps(runs: &[Run], joints: &[[(usize, bool); 2]], next: &[(Option<DuctElbow>, f64, f64)], mm_per_unit: f64) -> Vec<Vec<DuctStep>> {
    let mut out: Vec<Vec<DuctStep>> = runs.iter().map(|r| r.params.steps.clone()).collect();
    let length = |k: usize| runs.get(k).map_or(0.0, |r| r.params.length * mm_per_unit);
    let trims = |k: usize| next.get(k).map_or((0.0, 0.0), |n| (n.1, n.2));
    // The step a run has at its `at_end` end is the one whose elbows are there: one that starts (or ends) at the straight part, or within the clearance of
    // it, or in the bend (a step drawn when the bend was shorter or longer than now must not stay beside the new one).
    let at_end_of = |k: usize, st: &DuctStep, at_end: bool| {
        let (lo, hi) = (trims(k).0, length(k) - trims(k).1);
        if at_end { st.at_mm + st.shift_mm.abs() > hi - STEP_CLEARANCE_MM } else { st.at_mm < lo + STEP_CLEARANCE_MM }
    };
    for j in joints {
        let [(a, ae), (b, be)] = *j;
        let (Some(ra), Some(rb)) = (runs.get(a), runs.get(b)) else { continue };
        if ra.params.flexible || rb.params.flexible {
            continue;
        }
        for (k, e) in [(a, ae), (b, be)] {
            let kept: Vec<DuctStep> = out.get(k).into_iter().flatten().copied().filter(|st| !at_end_of(k, st, e)).collect();
            if let Some(slot) = out.get_mut(k) {
                *slot = kept;
            }
        }
        let (Some(from), Some(to)) = (ra.params.altitude_mm(), rb.params.altitude_mm()) else { continue };
        let shift = to - from;
        let (lo, hi) = (trims(b).0, length(b) - trims(b).1);
        if shift.abs() >= LEVEL_STEP_MIN_MM && hi - lo >= shift.abs() + STEP_CLEARANCE_MM {
            let at_mm = if be { hi - shift.abs() } else { lo };
            if let Some(slot) = out.get_mut(b) {
                slot.push(DuctStep { at_mm, shift_mm: shift });
            }
        }
    }
    out
}

/// A collar of a unit with the size it was given: where it ends, and the width of each collar that ends there (stacked ones share the point), by air direction.
struct Port {
    at: Vec2,
    sizes: Vec<(AirDirection, f64)>,
}

impl Port {
    /// The width of the collar a run `r` starts on here: the one of its own air direction when that was set, else the widest.
    fn width_for(&self, role: Option<AirDirection>) -> Option<f64> {
        let own = role.and_then(|air| self.sizes.iter().find(|(a, _)| *a == air).map(|(_, w)| *w));
        own.or_else(|| self.sizes.iter().map(|(_, w)| *w).reduce(f64::max))
    }
}

/// The reducer each run has once the elbows are decided (`next`: the elbow and the length each end gives up to it). See the module's text for the rules.
fn reducers<P: RunParams>(
    runs: &[Run<P>],
    joints: &[[(usize, bool); 2]],
    next: &[(Option<DuctElbow>, f64, f64)],
    ports: &[Port],
    tol: f64,
) -> Vec<Option<DuctReducer>> {
    let width = |k: usize| runs.get(k).map_or(0.0, |r| r.params.width_mm());
    let elbow_at = |k: usize, at_end: bool| next.get(k).is_some_and(|n| n.0.is_some_and(|e| e.at_end == at_end));
    let mut out: Vec<Option<DuctReducer>> = runs.iter().map(|r| r.params.reducer()).collect();
    // Whether `k` can take a reducer at `at_end` (it has one reducer, at one end).
    fn can_hold(out: &[Option<DuctReducer>], k: usize, at_end: bool) -> bool {
        out.get(k).is_some_and(|r| r.is_none_or(|r| r.at_end == at_end))
    }
    fn has(out: &[Option<DuctReducer>], k: usize, at_end: bool) -> bool {
        out.get(k).copied().flatten().is_some_and(|r| r.at_end == at_end)
    }
    fn set(out: &mut [Option<DuctReducer>], k: usize, at_end: bool, from_mm: f64, differs: bool) {
        if let Some(slot) = out.get_mut(k) {
            *slot = differs.then_some(DuctReducer { from_mm, at_end, across_mm: 0.0 });
        }
    }
    // How far the axis of the run `other` joins is from `holder`'s, mm across its block (positive to the left of its start to its end): an eccentric reducer.
    let across = |(h, he): (usize, bool), (o, oe): (usize, bool)| {
        let (Some(hr), Some(or)) = (runs.get(h), runs.get(o)) else { return 0.0 };
        let d = or.end(oe) - hr.end(he);
        let x = (hr.b - hr.a).normalized();
        if d.dot(x).abs() > tol { 0.0 } else { d.dot(x.perp()) / hr.params.units_per_mm() }
    };
    // Two runs end to end: the one that has a reducer there keeps it, else the later one takes it.
    for j in joints {
        let [(a, ae), (b, be)] = *j;
        let differs = (width(a) - width(b)).abs() > 0.5;
        // The run that has the elbow is drawn at its own width: the other one takes the cone.
        let (holder, other) = if elbow_at(a, ae) {
            ((b, be), (a, ae))
        } else if elbow_at(b, be) || (has(&out, a, ae) && !has(&out, b, be)) {
            ((a, ae), (b, be))
        } else if has(&out, b, be) || can_hold(&out, b, be) {
            ((b, be), (a, ae))
        } else {
            ((a, ae), (b, be))
        };
        if can_hold(&out, holder.0, holder.1) {
            set(&mut out, holder.0, holder.1, width(other.0), differs);
            if let Some(Some(red)) = out.get_mut(holder.0) {
                red.across_mm = across(holder, other);
            }
            if has(&out, other.0, other.1) {
                set(&mut out, other.0, other.1, 0.0, false);
            }
        }
    }
    // A run ending on the side of another: it joins it. An end where several runs meet only keeps (and renews) the reducer it has.
    for (k, r) in runs.iter().enumerate() {
        for at_end in [false, true] {
            let p = r.end(at_end);
            if joints.iter().any(|j| j.contains(&(k, at_end))) {
                continue;
            }
            // A run that starts on the collar of a unit that was given a size: the cone from that size.
            if let Some(w) = ports.iter().find(|q| q.at.dist(p) <= 3.0 * tol).and_then(|q| q.width_for(r.params.air_role())) {
                if can_hold(&out, k, at_end) {
                    set(&mut out, k, at_end, w, (w - width(k)).abs() > 0.5);
                }
                continue;
            }
            // A run laid over this one on the same line is not one this end branches from: its body lies along this run, it is not a trunk it meets.
            let overlaps = |o: &Run<P>| o.a.dist(p) > tol && o.b.dist(p) > tol && (o.b - o.a).normalized().dot(r.away(at_end)).abs() > 0.97;
            let others: Vec<usize> = runs
                .iter()
                .enumerate()
                .filter(|(i, o)| {
                    *i != k
                        && r.params.same_network(&o.params)
                        && !overlaps(o)
                        && (near_segment(p, o.a, o.b, tol) || o.a.dist(p) <= tol || o.b.dist(p) <= tol)
                })
                .map(|(i, _)| i)
                .collect();
            let through: Vec<usize> =
                others.iter().copied().filter(|i| runs.get(*i).is_some_and(|o| o.a.dist(p) > tol && o.b.dist(p) > tol)).collect();
            let held = has(&out, k, at_end);
            // A run that starts where two runs in line meet (a trunk cut in two there) is a branch of that trunk, as one that starts on its side is: the cone is
            // from the trunk's width.
            let in_line = |a: usize, b: usize| -> bool {
                let ends = |i: usize| runs.get(i).and_then(|o| [false, true].into_iter().find(|e| o.end(*e).dist(p) <= tol).map(|e| o.away(e)));
                matches!((ends(a), ends(b)), (Some(x), Some(y)) if x.dot(y) < -0.97)
            };
            if let [a, b] = others[..]
                && through.is_empty()
                && in_line(a, b)
                && can_hold(&out, k, at_end)
            {
                let trunk = width(a).max(width(b));
                set(&mut out, k, at_end, trunk, (trunk - width(k)).abs() > 0.5);
                continue;
            }
            match (others.len(), through.first()) {
                (1, Some(&t)) if can_hold(&out, k, at_end) => set(&mut out, k, at_end, width(t), (width(t) - width(k)).abs() > 0.5),
                (n, _) if n > 1 && held => {
                    let widest = others.iter().map(|i| width(*i)).fold(0.0, f64::max);
                    set(&mut out, k, at_end, widest, (widest - width(k)).abs() > 0.5);
                }
                _ => {}
            }
        }
    }
    // A flexible duct takes no cone: it follows the collar as it is.
    for (r, slot) in runs.iter().zip(out.iter_mut()) {
        if r.params.flexible() {
            *slot = None;
        }
    }
    out
}

/// Which ends of each run are free: no other run ends on it or goes through it, and `blocked` (a unit, a terminal, a riser) is not there.
fn free_ends<P>(runs: &[Run<P>], tol: f64, blocked: &dyn Fn(Vec2) -> bool) -> Vec<[bool; 2]> {
    runs.iter()
        .enumerate()
        .map(|(i, r)| {
            [false, true].map(|at_end| {
                let p = r.end(at_end);
                !blocked(p)
                    && !runs.iter().enumerate().any(|(k, o)| k != i && (o.a.dist(p) <= tol || o.b.dist(p) <= tol || near_segment(p, o.a, o.b, tol)))
            })
        })
        .collect()
}

/// The elbows the runs have, and what each gives up to them (mm at its start and at its end): the ones already there stay while the joint they are at still
/// turns that much and the run is still the size they were drawn for; one at no joint (or that no longer fits) is dropped; a joint with none gives one to the later
/// of its two runs; each of the two gives up the length the bend takes. `joints` are the joints where exactly two runs meet end to end that may have an elbow.
fn plan_elbows<P: RunParams>(runs: &[Run<P>], joints: &[[(usize, bool); 2]]) -> Vec<(Option<DuctElbow>, f64, f64)> {
    let mut next: Vec<(Option<DuctElbow>, f64, f64)> = runs.iter().map(|r| (r.params.elbow(), 0.0, 0.0)).collect();
    // The elbows already there stay while the joint they are at still turns that much.
    let mut holder: Vec<Option<usize>> = vec![None; joints.len()];
    for (n, j) in joints.iter().enumerate() {
        for (side, other) in [(0usize, 1usize), (1, 0)] {
            let (Some(&(c, ce)), Some(&(p, pe))) = (j.get(side), j.get(other)) else { continue };
            let (Some(child), Some(parent)) = (runs.get(c), runs.get(p)) else { continue };
            // The elbow stays while the joint still turns that much and the run is still the size it was drawn for: a run that was made wider (or
            // rectangular) gets its elbow drawn again at the radius of its new width.
            if let Some(e) = child.params.elbow()
                && e.at_end == ce
                && (turn_deg(parent, pe, child, ce).abs() - e.angle_deg.abs()).abs() < 0.5
                && elbow_radius(child, parent, turn_deg(parent, pe, child, ce)).is_some_and(|r| (r - e.radius_mm).abs() < 0.5)
                && holder.get(n).is_some_and(Option::is_none)
            {
                holder[n] = Some(side);
            }
        }
    }
    // An elbow at no joint (or one that no longer fits) is dropped.
    for (k, r) in runs.iter().enumerate() {
        let kept = joints.iter().zip(&holder).any(|(j, h)| h.and_then(|s| j.get(s)).is_some_and(|(c, _)| *c == k));
        if r.params.elbow().is_some() && !kept {
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
            if let Some(radius) = elbow_radius(child, parent, turn) {
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
    next
}

impl crate::Drawing {
    /// Whether a duct run of `space` was added, changed or removed since `before` (an older state of this drawing).
    pub fn ducts_touched_since(&self, before: &crate::Drawing, space: &Space) -> bool {
        let (Some(now), Some(then)) = (self.space(space), before.space(space)) else { return false };
        // (a unit's collar sizes count: a run that starts on one takes the cone from it)
        let relevant = |e: &std::sync::Arc<crate::Entity>| {
            matches!(e.kind, EntityKind::Insert(_))
                && (duct_of(e).is_some()
                    || ahu_of(e).is_some()
                    || super::zone_of(e).is_some()
                    || equipment_of(e).is_some_and(|q| q.kind.takes_ducts()))
        };
        now.iter().any(|e| then.get(e.handle).is_none_or(|old| !std::sync::Arc::ptr_eq(old, e)) && relevant(e))
            || then.iter().any(|e| now.get(e.handle).is_none() && relevant(e))
    }

    /// The ducts of `space` laid by an edge (`kept_edge`) whose width changed since `before` (an older state of this drawing): each axis moves across by
    /// how much the half width changed, so that the edge stays on the line it was laid on. Where runs meet end to end the shared point moves to where
    /// the moved axes cross (a run whose width did not change keeps its axis, the point slides along it). Returns how many runs were moved.
    pub fn keep_duct_edges_since(&mut self, before: &crate::Drawing, space: &Space) -> Result<usize> {
        let Some(now) = self.space(space) else { return Ok(0) };
        let moved: Vec<(Handle, f64)> = now
            .iter()
            .filter(|e| before.space(space).and_then(|t| t.get(e.handle)).is_some_and(|old| !std::sync::Arc::ptr_eq(old, e)))
            .filter_map(|e| {
                let p = duct_of(e)?;
                let side = p.kept_edge?;
                let old = before.duct_ends(e.handle)?.0;
                let d = side * p.units.to_units(p.plan_width_mm() - old.plan_width_mm()) / 2.0;
                (d.abs() > 1e-9).then_some((e.handle, d))
            })
            .collect();
        if moved.is_empty() {
            return Ok(0);
        }
        let units = HvacUnits::of(self);
        let tol = units.to_units(MEET_MM);
        // Every run of the space: its ends, the unit normal to its left, and how far its axis moves along it.
        let runs: Vec<(Handle, Vec2, Vec2, Vec2, f64)> = now
            .iter()
            .filter_map(|e| {
                let (_, a, b) = self.duct_ends(e.handle)?;
                let d = moved.iter().find(|m| m.0 == e.handle).map_or(0.0, |m| m.1);
                Some((e.handle, a, b, (b - a).normalized().perp(), d))
            })
            .collect();
        // Where a point `p` of the moved runs goes: alone, across by its run's move; shared, where the moved axes of two runs through it cross.
        let shift = |p: Vec2, own: (Vec2, f64)| -> Vec2 {
            let others: Vec<(Vec2, f64)> = runs
                .iter()
                .filter(|r| r.1.dist(p) <= tol || r.2.dist(p) <= tol)
                .map(|r| (r.3, r.4))
                .filter(|(n, _)| n.cross(own.0).abs() > 0.2)
                .collect();
            match others.first() {
                Some(&(m, e)) => {
                    let (n, d) = own;
                    let det = n.x * m.y - n.y * m.x;
                    p + Vec2::new(d * m.y - e * n.y, n.x * e - m.x * d) / det
                }
                None => p + own.0 * own.1,
            }
        };
        let mut ends: Vec<(Handle, usize, Vec2)> = Vec::new();
        for r in runs.iter().filter(|r| r.4 != 0.0) {
            for (grip, p) in [(0, r.1), (2, r.2)] {
                let to = shift(p, (r.3, r.4));
                ends.push((r.0, grip, to));
                // The runs that end there follow the shared point (those that did not move slide along their own axis).
                for o in runs.iter().filter(|o| o.0 != r.0 && o.4 == 0.0) {
                    for (g, q) in [(0, o.1), (2, o.2)] {
                        if q.dist(p) <= tol && !ends.iter().any(|x| x.0 == o.0 && x.1 == g) {
                            ends.push((o.0, g, to));
                        }
                    }
                }
            }
        }
        let mut done = Vec::new();
        for (h, grip, to) in ends {
            if self.stretch_duct_grip(h, grip, to)? && !done.contains(&h) {
                done.push(h);
            }
        }
        Ok(done.len())
    }

    /// Whether a pipe of `space` was added, changed or removed since `before` (an older state of this drawing).
    pub fn pipes_touched_since(&self, before: &crate::Drawing, space: &Space) -> bool {
        let (Some(now), Some(then)) = (self.space(space), before.space(space)) else { return false };
        let relevant = |e: &std::sync::Arc<crate::Entity>| matches!(e.kind, EntityKind::Insert(_)) && super::pipe_of(e).is_some();
        now.iter().any(|e| then.get(e.handle).is_none_or(|old| !std::sync::Arc::ptr_eq(old, e)) && relevant(e))
            || then.iter().any(|e| now.get(e.handle).is_none() && relevant(e))
    }

    /// Gives the elbows and the cones to the pipes of `space`, as it does the ducts: where two pipes of one service (and leg) meet end to end and turn, one of them
    /// gets the elbow, a short-radius one, the two giving up what it takes; where they are of another size, a cone from the size of the other (on the pipe
    /// that has no elbow, where there is one). A pipe that moved away loses them. The links of refrigerant (two or three tubes side by side) have them too: their tubes
    /// turn about one centre, each at its own radius, and close up together at a cone. Returns how many pipes were redrawn.
    pub fn draw_pipe_elbows(&mut self, space: &Space) -> Result<usize> {
        let tol = HvacUnits::of(self).to_units(MEET_MM);
        let runs: Vec<Run<super::PipeParams>> = self
            .space(space)
            .into_iter()
            .flat_map(|s| s.iter())
            .filter_map(|e| self.pipe_ends(e.handle).map(|(params, a, b)| Run { handle: e.handle, params, a, b }))
            .take(super::MAX_FITTING_PIPES + 1)
            .collect();
        if runs.len() > super::MAX_FITTING_PIPES {
            return Ok(0);
        }
        let joints: Vec<[(usize, bool); 2]> = joints(&runs, tol)
            .into_iter()
            .filter(|j| match (runs.get(j[0].0), runs.get(j[1].0)) {
                (Some(a), Some(b)) => a.params.same_network(&b.params),
                _ => false,
            })
            .collect();
        let next = plan_elbows(&runs, &joints);
        let reducers = reducers(&runs, &joints, &next, &[], tol);
        let mut redrawn = 0;
        for (k, (r, (elbow, trim_start, trim_end))) in runs.iter().zip(next).enumerate() {
            let same = |a: f64, b: f64| (a - b).abs() < 1e-6;
            let reducer = reducers.get(k).copied().flatten();
            if r.params.elbow == elbow
                && same(r.params.trim_start_mm, trim_start)
                && same(r.params.trim_end_mm, trim_end)
                && r.params.reducer == reducer
            {
                continue;
            }
            let mut p = r.params.clone();
            (p.elbow, p.trim_start_mm, p.trim_end_mm, p.reducer) = (elbow, trim_start, trim_end, reducer);
            self.regenerate_pipe(r.handle, &p)?;
            redrawn += 1;
        }
        Ok(redrawn)
    }

    /// Gives the ducted units of `space` a connection where a duct (rigid or flexible) ends on one of their sides and none is there yet, as wide as the duct, on
    /// the back side for the return (the air comes in) and the front side for the supply, along the unit's length; returns how many units were redrawn.
    pub fn draw_equipment_ports(&mut self, space: &Space) -> Result<usize> {
        let units = HvacUnits::of(self);
        let tol = units.to_units(3.0 * MEET_MM);
        let ends: Vec<(Vec2, DuctParams)> = self
            .space(space)
            .into_iter()
            .flat_map(|s| s.iter())
            .filter_map(|e| self.duct_ends(e.handle))
            .take(MAX_DUCTS)
            .flat_map(|(p, a, b)| [(a, p.clone()), (b, p)])
            .collect();
        let mut updates = Vec::new();
        for e in self.space(space).into_iter().flat_map(|s| s.iter()) {
            let (EntityKind::Insert(i), Some(mut q)) = (&e.kind, equipment_of(e)) else { continue };
            if !q.kind.takes_ducts() {
                continue;
            }
            let (hl, hw) = (units.to_units(q.length_mm) / 2.0, units.to_units(q.width_mm) / 2.0);
            let depth = 0.12 * hl.min(hw) * 2.0;
            let mut changed = false;
            for (end, params) in &ends {
                let Some(at) = super::slab::world_to_block(i, *end) else { continue };
                // A duct that ends on one of the two long sides (the return at the back, the supply at the front).
                if !(at.y.abs() >= hw - tol && at.y.abs() <= hw + depth + tol && at.x.abs() <= hl + tol) || q.air_ports.len() >= super::MAX_AIR_PORTS
                {
                    continue;
                }
                let inlet = at.y > 0.0;
                let taken = q
                    .air_ports
                    .iter()
                    .any(|p| p.inlet == inlet && (units.to_units(p.along_mm) - at.x).abs() <= units.to_units(p.size.width_mm()) / 2.0 + tol);
                if !taken {
                    let size = match params.shape {
                        DuctShape::Round => RiserSize::Round { diameter_mm: params.diameter_mm },
                        DuctShape::Rectangular => RiserSize::Rectangular { width_mm: params.width_mm, height_mm: params.height_mm },
                    };
                    q.air_ports.push(AirPort { inlet, size, along_mm: at.x * units.mm_per_unit });
                    changed = true;
                }
            }
            if changed {
                updates.push((e.handle, q));
            }
        }
        let n = updates.len();
        for (h, q) in updates {
            self.regenerate_equipment(h, &q)?;
        }
        Ok(n)
    }

    /// Gives the elbows to the runs of `space` that turn where they meet, and takes them from those that no longer do; returns how many runs were redrawn.
    pub fn draw_duct_elbows(&mut self, space: &Space) -> Result<usize> {
        let moved = self.draw_duct_elbows_only(space)?;
        Ok(moved + self.space_duct_labels(space)?)
    }

    fn draw_duct_elbows_only(&mut self, space: &Space) -> Result<usize> {
        let units = HvacUnits::of(self);
        // The connections of the ducted units come first: the cones from their sizes follow.
        let grown = self.draw_equipment_ports(space)?;
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
        // The free ends are closed: unless a terminal, a unit or a riser is there.
        let (mut terminals, mut units_at, mut risers): (Vec<Vec2>, Vec<(Vec2, f64)>, Vec<Vec<Vec2>>) = (Vec::new(), Vec::new(), Vec::new());
        for e in self.space(space).into_iter().flat_map(|s| s.iter()) {
            let EntityKind::Insert(i) = &e.kind else { continue };
            let at = Vec2::new(i.insert.x, i.insert.y);
            if air_terminal_of(e).is_some() {
                terminals.push(at);
            } else if let Some(a) = ahu_of(e) {
                units_at.push((at, a.half().len() + units.to_units(200.0)));
            } else if let Some(r) = riser_of(e) {
                risers.push(r.outline.iter().map(|q| super::slab::block_to_world(i, *q)).collect());
            }
        }
        let terminal_reach = units.to_units(150.0);
        // The collars of the units that were given a size, where they end.
        let mut ports: Vec<Port> = Vec::new();
        for e in self.space(space).into_iter().flat_map(|s| s.iter()) {
            let (EntityKind::Insert(i), Some(a)) = (&e.kind, ahu_of(e)) else { continue };
            let roles = a.roles();
            for (k, (tip, _)) in a.port_tips().into_iter().enumerate() {
                let (Some(size), Some(role)) = (a.port_sizes.get(k).copied().flatten(), roles.get(k).copied()) else { continue };
                let at = super::slab::block_to_world(i, tip);
                match ports.iter_mut().find(|q| q.at.dist(at) <= 1e-6) {
                    Some(q) => q.sizes.push((role, size.width_mm())),
                    None => ports.push(Port { at, sizes: vec![(role, size.width_mm())] }),
                }
            }
        }
        // The connections of the ducted units: the return (air in) and the supply (air out), each as wide as it was given.
        for e in self.space(space).into_iter().flat_map(|s| s.iter()) {
            let (EntityKind::Insert(i), Some(q)) = (&e.kind, equipment_of(e)) else { continue };
            for (tip, inlet, size) in q.air_port_tips() {
                let Some(size) = size else { continue };
                let (at, role) = (super::slab::block_to_world(i, tip), if inlet { AirDirection::Extract } else { AirDirection::Supply });
                ports.push(Port { at, sizes: vec![(role, size.width_mm())] });
            }
        }
        let blocked = |p: Vec2| {
            terminals.iter().any(|t| t.dist(p) <= terminal_reach)
                || units_at.iter().any(|(c, reach)| c.dist(p) <= *reach)
                || risers.iter().any(|o| {
                    cadcraft_geom::point_in_polygon(o, p) || o.iter().zip(o.iter().cycle().skip(1)).any(|(a, b)| near_segment(p, *a, *b, tol))
                })
        };
        let caps = free_ends(&runs, tol, &blocked);
        let all_joints = joints(&runs, tol);
        // An end at a joint is not a free end (the narrower run of an eccentric reducer ends inside the wider one, apart from its end).
        let mut caps = caps;
        for (k, at_end) in all_joints.iter().flatten() {
            if let Some(c) = caps.get_mut(*k) {
                c[usize::from(*at_end)] = false;
            }
        }
        let joints: Vec<[(usize, bool); 2]> =
            all_joints.iter().copied().filter(|j| j.iter().all(|(k, _)| runs.get(*k).is_some_and(|r| !r.params.flexible))).collect();
        let next = plan_elbows(&runs, &joints);
        let reducers = reducers(&runs, &all_joints, &next, &ports, tol);
        let steps = steps(&runs, &all_joints, &next, units.mm_per_unit);
        let mut redrawn = 0;
        for (k, (r, (elbow, trim_start, trim_end))) in runs.iter().zip(next).enumerate() {
            let same = |a: f64, b: f64| (a - b).abs() < 1e-6;
            let [cap_start, cap_end] = caps.get(k).copied().unwrap_or([false, false]);
            if r.params.elbow == elbow
                && same(r.params.trim_start_mm, trim_start)
                && same(r.params.trim_end_mm, trim_end)
                && (r.params.cap_start, r.params.cap_end) == (cap_start, cap_end)
                && r.params.reducer == reducers.get(k).copied().flatten()
                && steps.get(k).is_none_or(|st| *st == r.params.steps)
            {
                continue;
            }
            let mut p = r.params.clone();
            (p.elbow, p.trim_start_mm, p.trim_end_mm) = (elbow, trim_start, trim_end);
            (p.cap_start, p.cap_end) = (cap_start, cap_end);
            p.reducer = reducers.get(k).copied().flatten();
            p.reducer_len_mm = p.reducer.and(p.reducer_len_mm);
            p.reducer_shape = p.reducer.and(p.reducer_shape);
            if let Some(st) = steps.get(k) {
                p.steps.clone_from(st);
            }
            self.regenerate_duct(r.handle, &p)?;
            redrawn += 1;
        }
        Ok(redrawn + grown)
    }
}
