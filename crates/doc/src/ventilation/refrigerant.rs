//! The check of a refrigerant network: from each outdoor unit, the pipes, joints, headers and branch boxes are followed to the
//! indoor units (and the air handling units with a direct-expansion coil), and what each part serves is counted: the units and the
//! capacity index at each outlet of a box, the index below a joint or a header and the model that suits it. The network is read
//! from where the ends of the pipes meet the parts' connection points and the units' outlines; nothing is stored.

use cadcraft_geom::Vec2;

use super::pipe_riser::in_shaft;
use super::{EquipmentKind, PartKind, PartParams, PipeRiserParams, ahu_of, equipment_of, part_of};
use crate::{EntityKind, Handle, Result};

/// How close a pipe's end is to a connection point or to a unit's outline, mm.
const MEET_MM: f64 = 40.0;
/// Most units and parts one check reads.
const MAX_NODES: usize = 20_000;
/// The capacity index of a unit: its cooling power in kW times ten (a 2.5 kW unit is class 25).
const INDEX_PER_KW: f64 = 10.0;
/// What a branch-box outlet takes: indoor units, and capacity index (250 when two outlets are joined).
const MAX_UNITS_PER_OUTLET: usize = 5;
const MAX_INDEX_PER_OUTLET: f64 = 140.0;
const MAX_INDEX_TWO_OUTLETS: f64 = 250.0;

/// One outlet of a part and what it serves.
#[derive(Clone, Debug, PartialEq)]
pub struct OutletReport {
    pub outlet: usize,
    pub units: usize,
    pub index: f64,
}

/// A joint, a header or a box of a network: what is downstream of it.
#[derive(Clone, Debug, PartialEq)]
pub struct PartReport {
    pub handle: Handle,
    pub kind: PartKind,
    pub model: String,
    /// The units and the capacity index below the part.
    pub units: usize,
    pub index: f64,
    /// Each outlet that has something on it.
    pub outlets: Vec<OutletReport>,
    /// The model of the library that suits the index below a joint or a header, when it is not the one used.
    pub recommended: Option<String>,
}

/// The network of one outdoor unit.
#[derive(Clone, Debug, PartialEq)]
pub struct RefrigerantNetwork {
    /// The outdoor unit, or the pipe riser the network starts at when its outdoor unit is on another floor.
    pub outdoor: Handle,
    /// The name of that riser.
    pub source_riser: Option<String>,
    /// The risers the network reaches (what the other floors serve is typed on their lines: it is added to the index).
    pub risers: Vec<String>,
    pub units: usize,
    /// The indoor units (and air handling units with a direct-expansion coil) the network reaches.
    pub unit_handles: Vec<Handle>,
    pub index: f64,
    pub parts: Vec<PartReport>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefrigerantReport {
    pub networks: Vec<RefrigerantNetwork>,
    /// The indoor units no outdoor unit reaches.
    pub unconnected: Vec<Handle>,
    pub warnings: Vec<String>,
}

/// A rectangle turned about its centre, in drawing units.
struct Footprint {
    centre: Vec2,
    rotation: f64,
    half_length: f64,
    half_width: f64,
}

impl Footprint {
    fn holds(&self, p: Vec2, margin: f64) -> bool {
        let (s, c) = self.rotation.sin_cos();
        let (dx, dy) = (p.x - self.centre.x, p.y - self.centre.y);
        let (x, y) = (dx * c + dy * s, -dx * s + dy * c);
        x.abs() <= self.half_length + margin && y.abs() <= self.half_width + margin
    }
}

struct Unit {
    handle: Handle,
    footprint: Footprint,
    index: f64,
}

struct PartNode {
    handle: Handle,
    params: PartParams,
    /// The connection points in the drawing: where, and the outlet's number (none: the inlet side).
    ports: Vec<(Vec2, Option<usize>)>,
}

struct Run {
    a: Vec2,
    b: Vec2,
}

struct Graph {
    runs: Vec<Run>,
    parts: Vec<PartNode>,
    units: Vec<Unit>,
    outdoors: Vec<(Handle, Footprint)>,
    /// The pipe risers that have a refrigerant line, with their shafts.
    risers: Vec<(Handle, PipeRiserParams, Vec<Vec2>)>,
    tol: f64,
    mm: f64,
}

/// What the walk found below a point.
#[derive(Default)]
struct Below {
    units: Vec<usize>,
    /// The capacity index the risers reached carry to the other floors.
    extra: f64,
}

struct Walk {
    seen_runs: Vec<bool>,
    seen_parts: Vec<bool>,
    seen_units: Vec<bool>,
    /// For each part reached: its outlets (the units and the extra index of each), and the units and extra index below it.
    found: Vec<(usize, Vec<(usize, Vec<usize>, f64)>, Vec<usize>, f64)>,
    /// How many refrigerant lines of each riser a network has taken, and the risers reached in this walk.
    riser_used: Vec<usize>,
    reached: Vec<usize>,
    nodes: usize,
}

impl Graph {
    /// Follows what is at `point`: the units whose outline holds it, the pipes that start there and the parts whose connection point is
    /// there (entered by their inlet), as far down as there is anything.
    fn below(&self, point: Vec2, w: &mut Walk, depth: usize) -> Below {
        let mut out = Below::default();
        if depth > 500 || w.nodes > MAX_NODES {
            return out;
        }
        for (k, u) in self.units.iter().enumerate() {
            if u.footprint.holds(point, self.tol) && !w.seen_units.get(k).copied().unwrap_or(true) {
                if let Some(s) = w.seen_units.get_mut(k) {
                    *s = true;
                }
                out.units.push(k);
            }
        }
        // A shaft with a refrigerant line: what the other floors serve is typed on the line (the next one not taken yet).
        for (k, (_, riser, outline)) in self.risers.iter().enumerate() {
            if !in_shaft(outline, point, self.tol) {
                continue;
            }
            let used = w.riser_used.get(k).copied().unwrap_or(usize::MAX);
            if let Some(line) = riser.lines.iter().filter(|l| l.service.is_refrigerant()).nth(used) {
                out.extra += line.incoming.unwrap_or(0.0);
                if let Some(u) = w.riser_used.get_mut(k) {
                    *u += 1;
                }
                w.reached.push(k);
            }
        }
        for (k, r) in self.runs.iter().enumerate() {
            if w.seen_runs.get(k).copied().unwrap_or(true) {
                continue;
            }
            let far = if r.a.near(point, self.tol) {
                r.b
            } else if r.b.near(point, self.tol) {
                r.a
            } else {
                continue;
            };
            if let Some(s) = w.seen_runs.get_mut(k) {
                *s = true;
            }
            w.nodes += 1;
            let below = self.below(far, w, depth + 1);
            out.units.extend(below.units);
            out.extra += below.extra;
        }
        for (k, part) in self.parts.iter().enumerate() {
            if w.seen_parts.get(k).copied().unwrap_or(true) {
                continue;
            }
            // Entered by an inlet: the part passes the refrigerant on to its outlets.
            if !part.ports.iter().any(|(at, outlet)| outlet.is_none() && at.near(point, self.tol)) {
                continue;
            }
            if let Some(s) = w.seen_parts.get_mut(k) {
                *s = true;
            }
            w.nodes += 1;
            let mut per_outlet: Vec<(usize, Vec<usize>, f64)> = Vec::new();
            let (mut all, mut all_extra) = (Vec::new(), 0.0);
            for (at, outlet) in &part.ports {
                if let Some(o) = outlet {
                    let below = self.below(*at, w, depth + 1);
                    all.extend(below.units.iter().copied());
                    all_extra += below.extra;
                    if !below.units.is_empty() || below.extra > 0.0 {
                        per_outlet.push((*o, below.units, below.extra));
                    }
                }
            }
            out.units.extend(all.iter().copied());
            out.extra += all_extra;
            w.found.push((k, per_outlet, all, all_extra));
        }
        out
    }
}

/// The model of the library that suits `index` below a joint or a header.
fn recommended_model(kind: PartKind, index: f64) -> Option<&'static str> {
    match kind {
        PartKind::RefnetJoint if index < 200.0 => Some("Refrigerant Y joint, index below 200"),
        PartKind::RefnetJoint if index < 290.0 => Some("Refrigerant Y joint, index 200 to 290"),
        PartKind::RefnetJoint if index < 640.0 => Some("Refrigerant Y joint, index 290 to 640"),
        PartKind::RefnetJoint => Some("Refrigerant Y joint, index 640 or more"),
        PartKind::RefnetHeader if index < 290.0 => Some("Refrigerant header, index below 290"),
        PartKind::RefnetHeader if index < 640.0 => Some("Refrigerant header, index 290 to 640"),
        PartKind::RefnetHeader => Some("Refrigerant header, index 640 or more"),
        _ => None,
    }
}

impl crate::Drawing {
    /// Reads the refrigerant networks of model space and checks them (see the module). The refrigerant pipes, the Y joints, the
    /// headers and the branch boxes, the outdoor units (variable-refrigerant-flow and split) and the indoor units are looked at; a
    /// drawing without any gives an empty report.
    pub fn check_refrigerant(&self) -> Result<RefrigerantReport> {
        let mm = self
            .model
            .iter()
            .find_map(|e| self.pipe_ends(e.handle).map(|(p, ..)| p.units.mm_per_unit))
            .unwrap_or_else(|| super::HvacUnits::of(self).mm_per_unit);
        let mut g = Graph { runs: Vec::new(), parts: Vec::new(), units: Vec::new(), outdoors: Vec::new(), risers: Vec::new(), tol: MEET_MM / mm, mm };
        for e in self.model.iter() {
            if let Some((p, a, b)) = self.pipe_ends(e.handle).filter(|(p, ..)| p.service.is_refrigerant()) {
                let _ = p;
                g.runs.push(Run { a, b });
            } else if let (EntityKind::Insert(i), Some(p)) =
                (&e.kind, part_of(e).filter(|p| matches!(p.kind, PartKind::RefnetJoint | PartKind::RefnetHeader | PartKind::BranchBox)))
            {
                let (s, c) = i.rotation.sin_cos();
                let at = Vec2::new(i.insert.x, i.insert.y);
                let ports = p.ports().into_iter().map(|q| (at + Vec2::new(q.at.x * c - q.at.y * s, q.at.x * s + q.at.y * c), q.outlet)).collect();
                g.parts.push(PartNode { handle: e.handle, params: p, ports });
            } else if let (EntityKind::Insert(i), Some(q)) = (&e.kind, equipment_of(e)) {
                let footprint = Footprint {
                    centre: Vec2::new(i.insert.x, i.insert.y),
                    rotation: i.rotation,
                    half_length: q.units.to_units(q.length_mm) / 2.0,
                    half_width: q.units.to_units(q.width_mm) / 2.0,
                };
                match q.kind {
                    EquipmentKind::VrfOutdoor | EquipmentKind::SplitOutdoor => g.outdoors.push((e.handle, footprint)),
                    k if !k.is_outdoor() => g.units.push(Unit { handle: e.handle, footprint, index: q.capacity_kw.unwrap_or(0.0) * INDEX_PER_KW }),
                    _ => {}
                }
            } else if let (EntityKind::Insert(i), Some(a)) = (&e.kind, ahu_of(e)) {
                // An air handling unit with a direct-expansion coil is fed like an indoor unit.
                if let Some(kw) = a.coils.direct_expansion_kw {
                    let footprint = Footprint {
                        centre: Vec2::new(i.insert.x, i.insert.y),
                        rotation: i.rotation,
                        half_length: a.units.to_units(a.width_mm) / 2.0,
                        half_width: a.units.to_units(a.depth_mm) / 2.0,
                    };
                    g.units.push(Unit { handle: e.handle, footprint, index: kw * INDEX_PER_KW });
                }
            }
            if g.runs.len() + g.parts.len() + g.units.len() > MAX_NODES {
                return Err(super::invalid(format!("A refrigerant network is read {MAX_NODES} objects at a time at most.")));
            }
        }
        g.risers = self.pipe_risers().into_iter().filter(|(_, r, _)| r.lines.iter().any(|l| l.service.is_refrigerant())).collect();
        let mut walk = Walk {
            seen_runs: vec![false; g.runs.len()],
            seen_parts: vec![false; g.parts.len()],
            seen_units: vec![false; g.units.len()],
            found: Vec::new(),
            riser_used: vec![0; g.risers.len()],
            reached: Vec::new(),
            nodes: 0,
        };
        let mut networks = Vec::new();
        for (outdoor, foot) in &g.outdoors {
            // The pipes that start on the outdoor unit, and the parts put right against it.
            let mut starts: Vec<Vec2> = g.runs.iter().flat_map(|r| [r.a, r.b]).filter(|p| foot.holds(*p, g.tol)).collect();
            starts.extend(
                g.parts.iter().flat_map(|p| p.ports.iter().filter(|(_, o)| o.is_none()).map(|(at, _)| *at)).filter(|p| foot.holds(*p, g.tol)),
            );
            networks.push(g.network(*outdoor, None, &starts, &mut walk));
        }
        // A riser that no outdoor unit reaches is where a network starts: its outdoor unit is on another floor.
        for k in 0..g.risers.len() {
            if walk.riser_used.get(k).copied().unwrap_or(1) > 0 {
                continue;
            }
            let Some((handle, riser, outline)) = g.risers.get(k) else { continue };
            let starts: Vec<Vec2> = g
                .runs
                .iter()
                .enumerate()
                .filter(|(i, _)| !walk.seen_runs.get(*i).copied().unwrap_or(true))
                .flat_map(|(_, r)| [r.a, r.b])
                .filter(|p| in_shaft(outline, *p, g.tol))
                .collect();
            if starts.is_empty() {
                continue;
            }
            // It is where the network starts, not a place it ends.
            if let Some(u) = walk.riser_used.get_mut(k) {
                *u = riser.lines.len();
            }
            networks.push(g.network(*handle, Some(riser.name.clone()), &starts, &mut walk));
        }
        let unconnected: Vec<Handle> = g.units.iter().zip(&walk.seen_units).filter(|(_, s)| !**s).map(|(u, _)| u.handle).collect();
        let mut warnings = Vec::new();
        if !unconnected.is_empty() && !networks.is_empty() {
            warnings.push(format!("{} indoor unit(s) are reached from no outdoor unit", unconnected.len()));
        }
        Ok(RefrigerantReport { networks, unconnected, warnings })
    }
}

impl Graph {
    /// The network that starts at `starts` (the ends of the pipes on the outdoor unit or in the shaft of a riser).
    fn network(&self, outdoor: Handle, source_riser: Option<String>, starts: &[Vec2], w: &mut Walk) -> RefrigerantNetwork {
        w.found.clear();
        w.reached.clear();
        let mut units: Vec<usize> = Vec::new();
        let mut extra = 0.0;
        for start in starts {
            let below = self.below(*start, w, 0);
            units.extend(below.units);
            extra += below.extra;
        }
        let index = units.iter().filter_map(|k| self.units.get(*k)).fold(0.0, |a, u| a + u.index) + extra;
        let mut warnings = Vec::new();
        let mut parts = Vec::new();
        for (k, per_outlet, all, all_extra) in &w.found {
            let Some(node) = self.parts.get(*k) else { continue };
            parts.push(self.report_part(node, per_outlet, all, *all_extra, &mut warnings));
        }
        if starts.is_empty() {
            warnings.push("no refrigerant pipe starts at this outdoor unit".to_string());
        } else if units.is_empty() && extra <= 0.0 {
            warnings.push("no indoor unit is reached from here".to_string());
        }
        let mut risers: Vec<String> = w.reached.iter().filter_map(|k| self.risers.get(*k)).map(|(_, r, _)| r.name.clone()).collect();
        risers.sort();
        risers.dedup();
        let unit_handles = units.iter().filter_map(|k| self.units.get(*k)).map(|u| u.handle).collect();
        RefrigerantNetwork { outdoor, source_riser, risers, units: units.len(), unit_handles, index, parts, warnings }
    }

    fn report_part(
        &self,
        node: &PartNode,
        per_outlet: &[(usize, Vec<usize>, f64)],
        all: &[usize],
        all_extra: f64,
        warnings: &mut Vec<String>,
    ) -> PartReport {
        let index_of = |list: &[usize]| list.iter().filter_map(|k| self.units.get(*k)).fold(0.0, |a, u| a + u.index);
        let index = index_of(all) + all_extra;
        let outlets: Vec<OutletReport> =
            per_outlet.iter().map(|(o, list, extra)| OutletReport { outlet: *o, units: list.len(), index: index_of(list) + extra }).collect();
        let at = node.ports.first().map_or(Vec2::ZERO, |p| p.0);
        let place = format!("({:.0}, {:.0})", at.x * self.mm / 1000.0, at.y * self.mm / 1000.0);
        let kind = node.params.kind;
        let recommended = recommended_model(kind, index).filter(|m| *m != node.params.model).map(str::to_string);
        if let (Some(r), true) = (&recommended, standard_name(&node.params.model)) {
            warnings.push(format!(
                "the {} at {place} m carries an index of {index:.0}: use \"{r}\"",
                if kind == PartKind::RefnetJoint { "Y joint" } else { "header" }
            ));
        }
        if kind == PartKind::BranchBox {
            for o in &outlets {
                if o.units > MAX_UNITS_PER_OUTLET {
                    warnings.push(format!(
                        "outlet {} of the branch box at {place} m serves {} units: {MAX_UNITS_PER_OUTLET} at most",
                        o.outlet + 1,
                        o.units
                    ));
                }
                if o.index > MAX_INDEX_TWO_OUTLETS {
                    warnings.push(format!("outlet {} of the branch box at {place} m carries an index of {:.0}: {MAX_INDEX_TWO_OUTLETS:.0} at most, even joining two outlets", o.outlet + 1, o.index));
                } else if o.index > MAX_INDEX_PER_OUTLET {
                    warnings.push(format!("outlet {} of the branch box at {place} m carries an index of {:.0}: {MAX_INDEX_PER_OUTLET:.0} at most, {MAX_INDEX_TWO_OUTLETS:.0} joining two outlets", o.outlet + 1, o.index));
                }
            }
            if outlets.len() as u32 > node.params.outlets {
                warnings.push(format!("the branch box at {place} m has {} outlets in use, {} drawn", outlets.len(), node.params.outlets));
            }
        }
        PartReport { handle: node.handle, kind, model: node.params.model.clone(), units: all.len(), index, outlets, recommended }
    }
}

/// Whether `model` is the name of a standard joint or header (a model the user renamed is left alone).
fn standard_name(model: &str) -> bool {
    super::standard_part_model_named(model).is_some()
}
