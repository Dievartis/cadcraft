//! Which pipes, parts and riser lines are one network, and what it is called. A network is read from the drawing: the pipes of one service
//! whose ends meet, through the parts in the line of a pipe, through the equipment and units they both end in (the supply and the return
//! of a loop meet at the units), and through the shaft of a riser. A name typed on one of them goes to all the others; networks the
//! user did not name are given a name and a style by one command, which draws each in a colour of its own.

use cadcraft_geom::Vec2;

use super::pipe_riser::in_shaft;
use super::{NetworkStyle, PartParams, PipeLeg, PipeParams, PipeService, invalid, part_of, pipe_riser_of, same_network};
use crate::{Handle, Result};

/// How close two ends are to be one point, mm (as the networks are read for sizing).
const MEET_MM: f64 = 15.0;
/// Most pipes whose networks are read.
const MAX_PIPES: usize = 20_000;

/// A network read from the drawing.
pub(super) struct Component {
    pub service: PipeService,
    pub pipes: Vec<Handle>,
    pub parts: Vec<Handle>,
    /// The lines of risers (the riser and the number of the line) that belong to it.
    pub lines: Vec<(Handle, usize)>,
    /// The names its members already have, each once.
    pub names: Vec<String>,
}

/// A network with a name, for a list.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkSummary {
    pub name: String,
    /// The services (and legs) of its pipes, in the order of their tags.
    pub services: Vec<(PipeService, PipeLeg)>,
    pub pipes: usize,
    pub length_m: f64,
    pub style: Option<NetworkStyle>,
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while let Some(&p) = parent.get(i) {
        if p == i {
            break;
        }
        let gp = parent.get(p).copied().unwrap_or(p);
        if let Some(slot) = parent.get_mut(i) {
            *slot = gp;
        }
        i = gp;
    }
    i
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra != rb
        && let Some(slot) = parent.get_mut(ra)
    {
        *slot = rb;
    }
}

struct RunInfo {
    handle: Handle,
    params: PipeParams,
    a: Vec2,
    b: Vec2,
    tol: f64,
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 <= 1e-18 {
        return p.dist(a);
    }
    p.dist(a + ab * ((p - a).dot(ab) / len2).clamp(0.0, 1.0))
}

fn push_name(names: &mut Vec<String>, name: &str) {
    let n = name.trim();
    if !n.is_empty() && !names.iter().any(|x| same_network(x, n)) {
        names.push(n.to_string());
    }
}

impl crate::Drawing {
    /// Reads the networks of model space: the pipes that are one circuit, with their parts and riser lines, and the names they have.
    pub(super) fn pipe_components(&self) -> Result<Vec<Component>> {
        let runs: Vec<RunInfo> = self
            .model
            .iter()
            .filter_map(|e| self.pipe_ends(e.handle).map(|(params, a, b)| (e.handle, params, a, b)))
            .map(|(handle, params, a, b)| {
                let tol = params.units.to_units(MEET_MM).max(0.75 * params.units.to_units(params.plan_width_mm()));
                RunInfo { handle, params, a, b, tol }
            })
            .collect();
        if runs.len() > MAX_PIPES {
            return Err(invalid(format!("Pipe networks are read {MAX_PIPES} pipes at a time at most.")));
        }
        let n = runs.len();
        let mut parent: Vec<usize> = (0..n).collect();
        // Ends that meet, found through a grid of the ends.
        let cell = runs.iter().map(|r| r.tol).fold(1.0_f64, f64::max) * 2.0;
        let key = |p: Vec2| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        let mut grid: std::collections::HashMap<(i64, i64), Vec<(usize, Vec2)>> = std::collections::HashMap::new();
        for (i, r) in runs.iter().enumerate() {
            for end in [r.a, r.b] {
                grid.entry(key(end)).or_default().push((i, end));
            }
        }
        for (i, r) in runs.iter().enumerate() {
            for end in [r.a, r.b] {
                let (kx, ky) = key(end);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for (j, other) in grid.get(&(kx + dx, ky + dy)).into_iter().flatten() {
                            let Some(o) = runs.get(*j) else { continue };
                            if *j != i && o.params.service == r.params.service && other.near(end, r.tol.max(o.tol)) {
                                union(&mut parent, i, *j);
                            }
                        }
                    }
                }
            }
        }
        // Through a part in the line of a pipe, and through equipment and units: the runs of one service that end in the same outline.
        let mut outlines = self.part_bridges();
        outlines.extend(self.equipment_footprints());
        for o in &outlines {
            let mut first_of: Vec<(PipeService, usize)> = Vec::new();
            for (i, r) in runs.iter().enumerate() {
                if ![r.a, r.b].iter().any(|e| o.holds(*e, r.tol)) {
                    continue;
                }
                match first_of.iter().find(|(s, _)| *s == r.params.service) {
                    Some((_, j)) => union(&mut parent, i, *j),
                    None => first_of.push((r.params.service, i)),
                }
            }
        }
        // The shaft of a riser that has one line of a service joins the runs of that service that end in it.
        let risers = self.pipe_risers();
        for (_, riser, outline) in &risers {
            for line in &riser.lines {
                let same = riser.lines.iter().filter(|l| l.service == line.service).count();
                if same != 1 && !(same == 2 && line.service.has_legs()) {
                    continue;
                }
                let touching: Vec<usize> = runs
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.params.service == line.service && [r.a, r.b].iter().any(|e| in_shaft(outline, *e, r.tol)))
                    .map(|(i, _)| i)
                    .collect();
                for w in touching.windows(2) {
                    if let [a, b] = w {
                        union(&mut parent, *a, *b);
                    }
                }
            }
        }
        // The components.
        let mut index_of_root: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut comps: Vec<Component> = Vec::new();
        for (i, r) in runs.iter().enumerate() {
            let root = find(&mut parent, i);
            let c = *index_of_root.entry(root).or_insert_with(|| {
                comps.push(Component { service: r.params.service, pipes: Vec::new(), parts: Vec::new(), lines: Vec::new(), names: Vec::new() });
                comps.len() - 1
            });
            if let Some(comp) = comps.get_mut(c) {
                comp.pipes.push(r.handle);
                push_name(&mut comp.names, &r.params.network);
            }
        }
        let comp_of_run = |i: usize, parent: &mut Vec<usize>| index_of_root.get(&find(parent, i)).copied();
        // The parts: on the network of the pipe they touch (in the line of it), or of the nearest pipe they sit on.
        for e in self.model.iter() {
            let Some(part) = part_of(e).filter(|p: &PartParams| !p.kind.is_refrigerant() || p.kind.is_inline()) else { continue };
            let crate::EntityKind::Insert(ins) = &e.kind else { continue };
            let at = Vec2::new(ins.insert.x, ins.insert.y);
            let reach = part.units.to_units(part.length_mm.max(part.width_mm)) / 2.0;
            let near = runs
                .iter()
                .enumerate()
                .map(|(i, r)| (i, distance_to_segment(at, r.a, r.b), r.tol))
                .filter(|(_, d, tol)| *d <= reach + *tol)
                .min_by(|x, y| x.1.total_cmp(&y.1));
            if let Some((i, ..)) = near
                && let Some(c) = comp_of_run(i, &mut parent)
                && let Some(comp) = comps.get_mut(c)
            {
                comp.parts.push(e.handle);
                push_name(&mut comp.names, &part.network);
            }
        }
        // The lines of the risers: the network whose pipes end in the shaft (one line of a service, or by the order of the networks there).
        for (h, riser, outline) in &risers {
            for (k, line) in riser.lines.iter().enumerate() {
                let touching: Vec<usize> = runs
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.params.service == line.service && (!line.service.has_legs() || r.params.leg == line.leg))
                    .filter(|(_, r)| [r.a, r.b].iter().any(|e| in_shaft(outline, *e, r.tol)))
                    .map(|(i, _)| i)
                    .collect();
                let mut comps_here: Vec<usize> = touching.iter().filter_map(|i| comp_of_run(*i, &mut parent)).collect();
                comps_here.sort_unstable();
                comps_here.dedup();
                // The same line of two floors may differ: the line's own name counts too.
                let same_lines: Vec<usize> =
                    riser.lines.iter().enumerate().filter(|(_, l)| l.service == line.service && l.leg == line.leg).map(|(i, _)| i).collect();
                let rank = same_lines.iter().position(|i| *i == k).unwrap_or(0);
                let pick = if comps_here.len() == 1 || same_lines.len() == 1 { comps_here.first() } else { comps_here.get(rank) };
                if let Some(c) = pick.copied()
                    && let Some(comp) = comps.get_mut(c)
                {
                    comp.lines.push((*h, k));
                    push_name(&mut comp.names, &line.network);
                }
            }
        }
        Ok(comps)
    }

    /// Sets what is the network of a member: writes `name` on every pipe, part and riser line of `comp` that has none (`overwrite` false) or on
    /// all of them. Returns how many were changed. Best effort: an object that cannot be redrawn (a scaled one) is skipped.
    fn write_network_name(&mut self, comp: &Component, name: &str, overwrite: bool) -> usize {
        let mut changed = 0;
        for h in &comp.pipes {
            if let Some((mut p, ..)) = self.pipe_ends(*h)
                && (overwrite || p.network.trim().is_empty())
                && p.network != name
            {
                p.network = name.to_string();
                if self.regenerate_pipe(*h, &p).is_ok() {
                    changed += 1;
                }
            }
        }
        for h in &comp.parts {
            if let Some(mut p) = self.entity(*h).and_then(|e| part_of(e))
                && (overwrite || p.network.trim().is_empty())
                && p.network != name
            {
                p.network = name.to_string();
                if self.regenerate_part(*h, &p).is_ok() {
                    changed += 1;
                }
            }
        }
        let mut risers: Vec<Handle> = comp.lines.iter().map(|(h, _)| *h).collect();
        risers.sort_by_key(|h| h.0);
        risers.dedup();
        for rh in risers {
            let Some(mut r) = self.entity(rh).and_then(|e| pipe_riser_of(e)) else { continue };
            let mut any = false;
            for (h, k) in &comp.lines {
                if *h == rh
                    && let Some(line) = r.lines.get_mut(*k)
                    && (overwrite || line.network.trim().is_empty())
                    && line.network != name
                {
                    line.network = name.to_string();
                    any = true;
                }
            }
            if any && self.regenerate_pipe_riser(rh, &r).is_ok() {
                changed += 1;
            }
        }
        changed
    }

    /// Gives a network that has only one name that name on all its members: the name typed on one pipe goes to the ones it is connected to.
    /// Networks with two names (two named networks that were joined) are told. Returns how many objects were changed, and the warnings.
    pub fn propagate_pipe_networks(&mut self) -> Result<(usize, Vec<String>)> {
        // Nothing is named: nothing to spread (the common case while drawing).
        let any_named = self.model.iter().any(|e| self.pipe_ends(e.handle).is_some_and(|(p, ..)| !p.network.trim().is_empty()))
            || self.pipe_risers().iter().any(|(_, r, _)| r.lines.iter().any(|l| !l.network.trim().is_empty()));
        if !any_named {
            return Ok((0, Vec::new()));
        }
        let comps = self.pipe_components()?;
        let (mut changed, mut warnings) = (0, Vec::new());
        for c in &comps {
            match c.names.as_slice() {
                [name] => changed += self.write_network_name(c, name, false),
                [] => {}
                many => warnings.push(format!("a network is named {}: the names are kept", many.join(" and "))),
            }
        }
        Ok((changed, warnings))
    }

    /// Names the networks that have no name (by their service: CHW1, HW1, COND1, REF1…), gives each name a style with a hue of its own, and
    /// returns the networks of the drawing.
    pub fn name_pipe_networks(&mut self) -> Result<Vec<NetworkSummary>> {
        self.propagate_pipe_networks()?;
        let comps = self.pipe_components()?;
        let mut used: Vec<String> =
            comps.iter().flat_map(|c| c.names.iter().cloned()).chain(self.network_styles.iter().map(|s| s.name.clone())).collect();
        let mut plans: Vec<(usize, String)> = Vec::new();
        for (i, c) in comps.iter().enumerate() {
            if !c.names.is_empty() {
                continue;
            }
            let prefix = match c.service {
                PipeService::ChilledWater => "CHW",
                PipeService::HotWater => "HW",
                PipeService::ChangeOver => "CO",
                PipeService::Condensate => "COND",
                _ => "REF",
            };
            let n = (1..).find(|n| !used.iter().any(|u| same_network(u, &format!("{prefix}{n}")))).unwrap_or(1);
            let name = format!("{prefix}{n}");
            used.push(name.clone());
            plans.push((i, name));
        }
        // The styles first, so that the layers are made with them.
        for name in used.iter().filter(|u| self.network_style(u).is_none()).cloned().collect::<Vec<_>>() {
            self.set_network_style(&name, None, None)?;
        }
        for (i, name) in &plans {
            if let Some(c) = comps.get(*i) {
                self.write_network_name(c, name, true);
            }
        }
        self.refresh_network_layers();
        self.refresh_network_legends();
        Ok(self.pipe_network_summaries())
    }

    /// Gives every network of the pipes in `handles` (a pipe, a part, a riser) the name `name`, and the name a style when it has none.
    /// Returns how many objects were changed.
    pub fn set_pipe_network_name(&mut self, handles: &[Handle], name: &str) -> Result<usize> {
        let name = name.trim();
        let comps = self.pipe_components()?;
        let hit = |c: &Component| handles.iter().any(|h| c.pipes.contains(h) || c.parts.contains(h) || c.lines.iter().any(|(r, _)| r == h));
        if !comps.iter().any(hit) {
            return Err(invalid("Select a pipe, a part or a riser that is on a network."));
        }
        if self.network_style(name).is_none() {
            self.set_network_style(name, None, None)?;
        }
        let name = self.network_style(name).map_or_else(|| name.to_string(), |s| s.name.clone());
        let mut changed = 0;
        for c in comps.iter().filter(|c| hit(c)) {
            changed += self.write_network_name(c, &name, true);
        }
        self.refresh_network_layers();
        self.refresh_network_legends();
        Ok(changed)
    }

    /// Renames network `from` to `to` on every pipe, part and riser line that has it, and moves its style. Returns how many were changed.
    pub fn rename_pipe_network(&mut self, from: &str, to: &str) -> Result<usize> {
        let (from, to) = (from.trim(), to.trim());
        if same_network(from, to) && self.network_style(from).is_some_and(|s| s.name == to) {
            return Ok(0);
        }
        let old = self.network_style(from).cloned();
        if self.network_style(to).is_none() {
            let template = old.clone().map(|s| (Some(s.color), Some(s.linetype)));
            let (color, linetype) = template.unwrap_or((None, None));
            self.set_network_style(to, color, linetype.as_deref())?;
        }
        let to = self.network_style(to).map_or_else(|| to.to_string(), |s| s.name.clone());
        let comps = self.pipe_components()?;
        let mut changed = 0;
        for c in &comps {
            if c.names.iter().any(|n| same_network(n, from)) {
                changed += self.write_network_name(c, &to, true);
            }
        }
        if old.is_some() && !same_network(from, &to) {
            self.network_styles.retain(|s| !same_network(&s.name, from));
        }
        self.refresh_network_layers();
        self.refresh_network_legends();
        Ok(changed)
    }

    /// The named networks of the pipes: services, how many pipes, their length, their style.
    pub fn pipe_network_summaries(&self) -> Vec<NetworkSummary> {
        let mut out: Vec<NetworkSummary> = Vec::new();
        for e in self.model.iter() {
            let Some((p, ..)) = self.pipe_ends(e.handle) else { continue };
            let name = p.network.trim();
            if name.is_empty() {
                continue;
            }
            let at = match out.iter().position(|s| same_network(&s.name, name)) {
                Some(i) => i,
                None => {
                    out.push(NetworkSummary {
                        name: name.to_string(),
                        services: Vec::new(),
                        pipes: 0,
                        length_m: 0.0,
                        style: self.network_style(name).cloned(),
                    });
                    out.len() - 1
                }
            };
            if let Some(s) = out.get_mut(at) {
                let key = (p.service, if p.service.has_legs() { p.leg } else { PipeLeg::Supply });
                if !s.services.contains(&key) {
                    s.services.push(key);
                }
                s.pipes += 1;
                s.length_m += p.length_m();
            }
        }
        for s in &mut out {
            s.services.sort_by_key(|(sv, lg)| (PipeService::ALL.iter().position(|x| x == sv).unwrap_or(0), *lg as u8));
            s.length_m = (s.length_m * 100.0).round() / 100.0;
        }
        out.sort_by_key(|n| n.name.to_lowercase());
        out
    }
}
