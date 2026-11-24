//! The pipe networks of a drawing, read from where the runs' ends meet (never stored), and their sizing: each
//! network of a service (and, for water, of a leg) starts at a run marked as its source; the loads typed at the free
//! ends are added up towards it; the flow follows from the load, and the size from the flow.

use cadcraft_geom::Vec2;
use cadcraft_hvac::{PipeCriteria, PipeGoverning, WaterTemperature, check_water_pipe, size_condensate_drain, size_water_pipe, water_flow_m3h};

use super::pipe_riser::in_shaft;
use super::{EquipmentKind, PipeLeg, PipeParams, PipeRiserParams, PipeService, ahu_of, equipment_of, invalid, part_of};
use crate::{EntityKind, Handle, Result};

/// How close two ends are to be one point, mm; and, as a share of the wider pipe, how close to its axis.
const MEET_MM: f64 = 15.0;
/// Most runs one network reads.
pub const MAX_PIPES: usize = 20_000;

/// The temperature differences the water networks are sized for, K, and the slope of the drains, %.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipeSizingParams {
    pub delta_t_chilled: f64,
    pub delta_t_hot: f64,
    pub delta_t_change_over: f64,
    /// The slope of a condensate drain, %.
    pub drain_slope_percent: f64,
    pub criteria: PipeCriteria,
}

impl Default for PipeSizingParams {
    /// 7/12 °C for chilled water, 70/50 °C for hot water, and the chilled-water difference for the change-over network
    /// (its cooling duty is the larger flow).
    fn default() -> Self {
        PipeSizingParams {
            delta_t_chilled: 5.0,
            delta_t_hot: 20.0,
            delta_t_change_over: 5.0,
            drain_slope_percent: 1.0,
            criteria: PipeCriteria::default(),
        }
    }
}

/// One run of a sized network.
#[derive(Clone, Debug, PartialEq)]
pub struct PipeResult {
    pub handle: Handle,
    pub dn_mm: f64,
    /// The load the run carries, kW (l/h for a drain), and the flow, m³/h.
    pub load: f64,
    pub flow_m3h: f64,
    pub velocity_ms: f64,
    pub gradient_pa_m: f64,
    pub imposed: bool,
    pub governing: PipeGoverning,
    /// The pressure lost in the run itself, kPa, and in the parts in the line of the pipe that the run is reached through
    /// (the ones before it, towards the source), each with its loss.
    pub run_loss_kpa: f64,
    pub part_loss_kpa: f64,
    pub parts: Vec<(Handle, f64)>,
}

/// A sized network: its service and leg, its source run, its total load and its runs.
#[derive(Clone, Debug, PartialEq)]
pub struct PipeNetworkReport {
    pub service: PipeService,
    pub leg: PipeLeg,
    pub source: Handle,
    pub total_load: f64,
    /// The pressure lost from the source to the far end of the most unfavourable run (pipes and parts), kPa, and that run.
    pub total_loss_kpa: f64,
    pub critical: Handle,
    pub runs: Vec<PipeResult>,
    pub warnings: Vec<String>,
}

/// What a pipe can end in: an equipment or an air handling unit, a rectangle turned about its centre (drawing units),
/// with the power it asks of each water service, kW.
pub(super) struct Equip {
    pub(super) centre: Vec2,
    pub(super) rotation: f64,
    pub(super) half_length: f64,
    pub(super) half_width: f64,
    /// A chiller: the water networks start from it.
    pub(super) chiller: bool,
    pub(super) loads: Vec<(PipeService, f64)>,
    /// The part, for the ones in the line of a pipe.
    pub(super) part: Option<(crate::Handle, super::PartParams)>,
}

impl Equip {
    /// Whether `p` is within the footprint, `margin` drawing units out.
    pub(super) fn holds(&self, p: Vec2, margin: f64) -> bool {
        let (s, c) = self.rotation.sin_cos();
        let (dx, dy) = (p.x - self.centre.x, p.y - self.centre.y);
        let (x, y) = (dx * c + dy * s, -dx * s + dy * c);
        x.abs() <= self.half_length + margin && y.abs() <= self.half_width + margin
    }

    /// The power it asks of a network of `service`, kW.
    fn power(&self, service: PipeService) -> Option<f64> {
        let total: f64 = self.loads.iter().filter(|(s, _)| *s == service).map(|(_, kw)| kw).sum();
        (total > 0.0).then_some(total)
    }
}

struct Run {
    handle: Handle,
    params: PipeParams,
    a: Vec2,
    b: Vec2,
}

struct Node {
    run: usize,
    parent: Option<usize>,
    /// Whether the run is entered by its start (the block's origin).
    at_start: bool,
    /// The parts in the line of the pipe that the run is reached through (indices in the bridges).
    via: Vec<usize>,
}

/// A pipe riser of the drawing with its shaft.
type RiserShaft = (Handle, PipeRiserParams, Vec<Vec2>);

/// What a pipe of the network of `params` finds at `at` in a shaft: the line of a riser that is its network.
fn riser_line_at<'a>(risers: &'a [RiserShaft], params: &PipeParams, at: Vec2) -> Option<&'a super::RiserLine> {
    let margin = params.units.to_units(MEET_MM);
    let (service, leg) = group(params);
    risers.iter().filter(|(_, _, outline)| in_shaft(outline, at, margin)).find_map(|(_, r, _)| r.line_for(service, leg))
}

/// What the line a pipe ends on carries to the other floors, when it was typed.
fn riser_incoming(risers: &[RiserShaft], params: &PipeParams, at: Vec2) -> Option<f64> {
    riser_line_at(risers, params, at).and_then(|l| l.incoming).filter(|v| *v > 0.0)
}

fn group(p: &PipeParams) -> (PipeService, PipeLeg) {
    (p.service, if p.service.has_legs() { p.leg } else { PipeLeg::Supply })
}

impl crate::Drawing {
    /// Reads every pipe network of model space and sizes it (the runs whose size was given are checked, not changed),
    /// writing the flow, velocity and pressure loss on the runs and the size on the others. Refrigerant lines are
    /// not sized: their size is the user's. Returns a report per network, and warnings for runs that belong to none.
    pub fn size_pipes(&mut self, sizing: &PipeSizingParams) -> Result<(Vec<PipeNetworkReport>, Vec<String>)> {
        let runs: Vec<Run> = self
            .model
            .iter()
            .filter_map(|e| self.pipe_ends(e.handle).map(|(params, a, b)| Run { handle: e.handle, params, a, b }))
            .filter(|r| !r.params.service.is_refrigerant())
            .collect();
        if runs.len() > MAX_PIPES {
            return Err(invalid(format!("Pipes are sized {MAX_PIPES} at a time at most.")));
        }
        let equipment = self.equipment_footprints();
        let bridges = self.part_bridges();
        let risers = self.pipe_risers();
        let mut taken = vec![false; runs.len()];
        let mut reports = Vec::new();
        let mut writes: Vec<(Handle, PipeParams)> = Vec::new();
        for (i, root) in runs.iter().enumerate() {
            let from_chiller =
                root.params.service.has_legs() && equipment.iter().any(|q| q.chiller && q.holds(root.a, root.params.units.to_units(MEET_MM)));
            // A run that starts in a shaft whose riser has a line of its network takes the riser as its source.
            let from_riser = !root.params.service.is_refrigerant() && riser_line_at(&risers, &root.params, root.a).is_some();
            if !(root.params.source || from_chiller || from_riser) || taken.get(i).copied().unwrap_or(true) {
                continue;
            }
            let nodes = read_tree(&runs, &mut taken, i, &bridges);
            let (report, changed) = size_tree(&runs, &nodes, &equipment, &bridges, &risers, sizing)?;
            writes.extend(changed);
            reports.push(report);
        }
        let mut warnings = Vec::new();
        for (r, t) in runs.iter().zip(&taken) {
            if !t {
                warnings.push(format!(
                    "the {} run at ({:.0}, {:.0}) belongs to no network: mark the run that starts at the source (\"Starts at the source\")",
                    r.params.tag(),
                    r.a.x,
                    r.a.y
                ));
            }
        }
        for (h, p) in writes {
            self.regenerate_pipe(h, &p)?;
        }
        self.size_riser_lines(&runs, &reports, &risers, sizing)?;
        Ok((reports, warnings))
    }
}

/// The tree of runs from source run `root`, entered at its start: a run is the child of the run whose far end it
/// starts on, of the same service and leg.
fn read_tree(runs: &[Run], taken: &mut [bool], root: usize, bridges: &[Equip]) -> Vec<Node> {
    let mut nodes = vec![Node { run: root, parent: None, at_start: true, via: Vec::new() }];
    if let Some(t) = taken.get_mut(root) {
        *t = true;
    }
    let Some(r0) = runs.get(root) else { return nodes };
    let wanted = group(&r0.params);
    let mut next = 0;
    while let Some(n) = nodes.get(next) {
        let Some(cur) = runs.get(n.run) else { break };
        let to = if n.at_start { cur.b } else { cur.a };
        let mut found = Vec::new();
        for (j, other) in runs.iter().enumerate() {
            if taken.get(j).copied().unwrap_or(true) || group(&other.params) != wanted {
                continue;
            }
            let tol = other
                .params
                .units
                .to_units(MEET_MM)
                .max(0.75 * other.params.units.to_units(other.params.plan_width_mm().max(cur.params.plan_width_mm())));
            // An end meets the point directly, or through a part in the line of the pipe (a valve, a pump) that holds both.
            let meets = |end: Vec2| -> Option<Vec<usize>> {
                if end.near(to, tol) {
                    return Some(Vec::new());
                }
                let through: Vec<usize> =
                    (0..bridges.len()).filter(|&k| bridges.get(k).is_some_and(|b| b.holds(to, tol) && b.holds(end, tol))).collect();
                (!through.is_empty()).then_some(through)
            };
            if let Some(via) = meets(other.a) {
                found.push((j, true, via));
            } else if let Some(via) = meets(other.b) {
                found.push((j, false, via));
            }
        }
        for (j, at_start, via) in found {
            if let Some(t) = taken.get_mut(j) {
                *t = true;
            }
            nodes.push(Node { run: j, parent: Some(next), at_start, via });
        }
        next += 1;
        if nodes.len() > MAX_PIPES {
            break;
        }
    }
    nodes
}

fn size_tree(
    runs: &[Run],
    nodes: &[Node],
    equipment: &[Equip],
    bridges: &[Equip],
    risers: &[RiserShaft],
    sizing: &PipeSizingParams,
) -> Result<(PipeNetworkReport, Vec<(Handle, PipeParams)>)> {
    let mut warnings: Vec<String> = Vec::new();
    let Some(root_run) = nodes.first().and_then(|n| runs.get(n.run)) else { return Err(invalid("A network has a source.")) };
    let (service, leg) = group(&root_run.params);
    // The loads added up from the leaves to the source.
    let mut total: Vec<f64> = nodes
        .iter()
        .map(|n| {
            let Some(r) = runs.get(n.run) else { return 0.0 };
            let far = if n.at_start { r.b } else { r.a };
            let leaf = !nodes.iter().any(|m| m.parent.and_then(|p| nodes.get(p)).is_some_and(|q| q.run == n.run));
            r.params
                .load_kw
                .or_else(|| {
                    let margin = r.params.units.to_units(MEET_MM);
                    leaf.then(|| {
                        equipment
                            .iter()
                            .filter(|q| q.holds(far, margin))
                            .find_map(|q| q.power(r.params.service))
                            .or_else(|| riser_incoming(risers, &r.params, far))
                    })
                    .flatten()
                })
                .unwrap_or(0.0)
        })
        .collect();
    let own = total.clone();
    for i in (0..nodes.len()).rev() {
        let (t, parent) = (total.get(i).copied().unwrap_or(0.0), nodes.get(i).and_then(|n| n.parent));
        if let Some(p) = parent.and_then(|p| total.get_mut(p)) {
            *p += t;
        }
    }
    let has_child: Vec<bool> = (0..nodes.len()).map(|i| nodes.iter().any(|n| n.parent == Some(i))).collect();
    for (i, n) in nodes.iter().enumerate() {
        let leaf_without_load = !has_child.get(i).copied().unwrap_or(true) && own.get(i).copied().unwrap_or(0.0) <= 0.0;
        if let (true, Some(r)) = (leaf_without_load, runs.get(n.run)) {
            warnings.push(format!(
                "the {} run ending at ({:.0}, {:.0}) serves no load: type the load of the unit it feeds",
                r.params.tag(),
                r.b.x,
                r.b.y
            ));
        }
    }
    let water = match service {
        PipeService::ChilledWater => Some((WaterTemperature::Cold, sizing.delta_t_chilled)),
        PipeService::HotWater => Some((WaterTemperature::Hot, sizing.delta_t_hot)),
        PipeService::ChangeOver => Some((WaterTemperature::Mixed, sizing.delta_t_change_over)),
        _ => None,
    };
    let mut results = Vec::new();
    let mut changed = Vec::new();
    // The pressure lost from the source to the far end of each run.
    let mut path: Vec<f64> = Vec::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        let Some(r) = runs.get(n.run) else { continue };
        let load = total.get(i).copied().unwrap_or(0.0);
        let mut p = r.params.clone();
        let (flow, size) = match water {
            Some((temp, dt)) => {
                let flow = water_flow_m3h(load, dt).map_err(|e| invalid(e.to_string()))?;
                let size =
                    if p.imposed { check_water_pipe(flow, p.dn_mm, temp, &sizing.criteria) } else { size_water_pipe(flow, temp, &sizing.criteria) }
                        .map_err(|e| invalid(e.to_string()))?;
                (flow, size)
            }
            None => {
                let size = size_condensate_drain(load, sizing.drain_slope_percent).map_err(|e| invalid(e.to_string()))?;
                (load / 1000.0, size)
            }
        };
        if !p.imposed {
            p.dn_mm = size.dn_mm;
        } else if water.is_some() && size.velocity_ms > cadcraft_hvac::max_velocity_ms(p.dn_mm) {
            warnings.push(format!("the {} run at ({:.0}, {:.0}) is too small for its flow: {:.2} m/s", p.tag(), r.a.x, r.a.y, size.velocity_ms));
        }
        (p.flow_m3h, p.velocity, p.loss_pa_m) = (Some(flow), Some(size.velocity_ms), water.map(|_| size.gradient_pa_m));
        // Which way the fluid runs along the block's x axis: away from the source for a supply, towards it for a return and a drain.
        let away = service.has_legs() && leg == PipeLeg::Supply;
        p.flow_forward = Some(away == n.at_start);
        // What the run and the parts before it lose (a drain runs by gravity: nothing).
        let run_loss = if water.is_some() { size.gradient_pa_m * r.params.length_m() / 1000.0 } else { 0.0 };
        let mut parts: Vec<(Handle, f64)> = Vec::new();
        if water.is_some() {
            for k in &n.via {
                if let Some((h, part)) = bridges.get(*k).and_then(|b| b.part.as_ref()) {
                    parts.push((*h, part.pressure_loss_kpa(flow, size.velocity_ms, size.gradient_pa_m)));
                }
            }
        }
        let part_loss: f64 = parts.iter().map(|x| x.1).sum();
        path.push(n.parent.and_then(|pi| path.get(pi)).copied().unwrap_or(0.0) + run_loss + part_loss);
        results.push(PipeResult {
            handle: r.handle,
            dn_mm: p.dn_mm,
            load,
            flow_m3h: flow,
            velocity_ms: size.velocity_ms,
            gradient_pa_m: size.gradient_pa_m,
            imposed: p.imposed,
            governing: size.governing,
            run_loss_kpa: run_loss,
            part_loss_kpa: part_loss,
            parts,
        });
        changed.push((r.handle, p));
    }
    let total_load = total.first().copied().unwrap_or(0.0);
    let (worst, total_loss_kpa) = path.iter().enumerate().fold((0, 0.0), |m, (i, v)| if *v > m.1 { (i, *v) } else { m });
    let critical = results.get(worst).map_or(root_run.handle, |x| x.handle);
    Ok((PipeNetworkReport { service, leg, source: root_run.handle, total_load, total_loss_kpa, critical, runs: results, warnings }, changed))
}

/// How far the end of a tube may be from the body of a refrigerant branch part and still be on it, mm.
const BRANCH_REACH_MM: f64 = 250.0;

impl crate::Drawing {
    /// The parts in the line of a pipe, as footprints the network is read through.
    pub(super) fn part_bridges(&self) -> Vec<Equip> {
        self.model
            .iter()
            .filter_map(|e| {
                let p = part_of(e).filter(|p| p.kind.is_inline())?;
                let EntityKind::Insert(i) = &e.kind else { return None };
                Some(Equip {
                    centre: Vec2::new(i.insert.x, i.insert.y),
                    rotation: i.rotation,
                    half_length: p.units.to_units(p.length_mm) * i.scale.x.abs() / 2.0,
                    half_width: p.units.to_units(p.width_mm) * i.scale.y.abs() / 2.0,
                    chiller: false,
                    loads: Vec::new(),
                    part: Some((e.handle, p)),
                })
            })
            .collect()
    }

    /// The refrigerant branch parts (Y joints, headers, branch boxes) as footprints: the tubes that end in one are one circuit, though the part is not in the line of a
    /// pipe.
    pub(super) fn branch_outlines(&self) -> Vec<Equip> {
        self.model
            .iter()
            .filter_map(|e| {
                let p = part_of(e).filter(|p| p.kind.is_refrigerant() && !p.kind.is_inline())?;
                let EntityKind::Insert(i) = &e.kind else { return None };
                // The tubes end at the outlets, which stand out of the body of the part: the outline is taken wider by the reach of an outlet.
                let reach = p.units.to_units(BRANCH_REACH_MM);
                Some(Equip {
                    centre: Vec2::new(i.insert.x, i.insert.y),
                    rotation: i.rotation,
                    half_length: p.units.to_units(p.length_mm) * i.scale.x.abs() / 2.0 + reach,
                    half_width: p.units.to_units(p.width_mm) * i.scale.y.abs() / 2.0 + reach,
                    chiller: false,
                    loads: Vec::new(),
                    part: Some((e.handle, p)),
                })
            })
            .collect()
    }

    /// The equipment and air handling units of model space with their footprints and powers.
    pub(super) fn equipment_footprints(&self) -> Vec<Equip> {
        let mut out = Vec::new();
        for e in self.model.iter() {
            let EntityKind::Insert(i) = &e.kind else { continue };
            let centre = Vec2::new(i.insert.x, i.insert.y);
            if let Some(p) = equipment_of(e) {
                let mut loads = Vec::new();
                if let Some(kw) = p.capacity_kw {
                    loads.extend([(PipeService::ChilledWater, kw), (PipeService::ChangeOver, kw)]);
                }
                if let Some(kw) = p.heating_kw {
                    loads.push((PipeService::HotWater, kw));
                }
                out.push(Equip {
                    centre,
                    rotation: i.rotation,
                    half_length: p.units.to_units(p.length_mm) * i.scale.x.abs() / 2.0,
                    half_width: p.units.to_units(p.width_mm) * i.scale.y.abs() / 2.0,
                    chiller: p.kind == EquipmentKind::Chiller,
                    loads,
                    part: None,
                });
            } else if let Some(a) = ahu_of(e) {
                let loads = [PipeService::ChilledWater, PipeService::HotWater, PipeService::ChangeOver]
                    .into_iter()
                    .filter_map(|s| a.coils.power(s).map(|kw| (s, kw)))
                    .collect();
                out.push(Equip {
                    centre,
                    rotation: i.rotation,
                    half_length: a.units.to_units(a.width_mm) * i.scale.x.abs() / 2.0,
                    half_width: a.units.to_units(a.depth_mm) * i.scale.y.abs() / 2.0,
                    chiller: false,
                    loads,
                    part: None,
                });
            }
        }
        out
    }
}

impl crate::Drawing {
    /// Sizes the lines of the pipe risers for what they carry at this level: what the line carries to the other floors, as typed, and
    /// the load of the networks of this floor that start in its shaft. A size typed is checked, not changed.
    fn size_riser_lines(&mut self, runs: &[Run], reports: &[PipeNetworkReport], risers: &[RiserShaft], sizing: &PipeSizingParams) -> Result<()> {
        for (h, riser, outline) in risers {
            let mut next = riser.clone();
            let margin = riser.units.to_units(MEET_MM);
            for line in &mut next.lines {
                let from_floor: f64 = reports
                    .iter()
                    .filter(|r| r.service == line.service && (!line.service.has_legs() || r.leg == line.leg))
                    .filter(|r| runs.iter().find(|x| x.handle == r.source).is_some_and(|x| in_shaft(outline, x.a, margin)))
                    .map(|r| r.total_load)
                    .sum();
                let carried = line.incoming.unwrap_or(0.0) + from_floor;
                match size_line(line, carried, sizing)? {
                    Some((flow, size)) => {
                        if !line.imposed {
                            line.dn_mm = size.dn_mm;
                        }
                        (line.load, line.flow_m3h, line.velocity) = (Some(carried), Some(flow), Some(size.velocity_ms));
                    }
                    None => (line.load, line.flow_m3h, line.velocity) = (None, None, None),
                }
            }
            if next != *riser {
                self.regenerate_pipe_riser(*h, &next)?;
            }
        }
        Ok(())
    }
}

/// The flow and the size of a riser line that carries `load` (kW; l/h for a drain); none for refrigerant and for a line that carries nothing.
fn size_line(line: &super::RiserLine, load: f64, sizing: &PipeSizingParams) -> Result<Option<(f64, cadcraft_hvac::PipeSizing)>> {
    if load <= 0.0 || line.service.is_refrigerant() {
        return Ok(None);
    }
    let water = match line.service {
        PipeService::ChilledWater => Some((WaterTemperature::Cold, sizing.delta_t_chilled)),
        PipeService::HotWater => Some((WaterTemperature::Hot, sizing.delta_t_hot)),
        PipeService::ChangeOver => Some((WaterTemperature::Mixed, sizing.delta_t_change_over)),
        _ => None,
    };
    match water {
        Some((temp, dt)) => {
            let flow = water_flow_m3h(load, dt).map_err(|e| invalid(e.to_string()))?;
            let size =
                if line.imposed { check_water_pipe(flow, line.dn_mm, temp, &sizing.criteria) } else { size_water_pipe(flow, temp, &sizing.criteria) }
                    .map_err(|e| invalid(e.to_string()))?;
            Ok(Some((flow, size)))
        }
        None => Ok(Some((load / 1000.0, size_condensate_drain(load, sizing.drain_slope_percent).map_err(|e| invalid(e.to_string()))?))),
    }
}
