//! Reading the circuits of a plan and checking them. A circuit is a name: the cables, devices, equipment and units that carry it are one circuit. For
//! each, the power it carries, the current, the protection, the cable section it needs (by the current it must carry and by the voltage drop along it)
//! and what is wrong with the cable that is drawn. Nothing is stored: the circuits are read from the drawing each time.
//!
//! The length of a circuit is the length of all its cables, which overestimates the voltage drop of a branched circuit: the safe side.

use std::collections::BTreeMap;

use cadcraft_hvac as calc;
use serde::{Deserialize, Serialize};

use super::{CableParams, CableUsage, DeviceKind, ElecConductor, ElecInsulation, ElecLaying, ElecPhases, ahu_of, cable_of, device_of, equipment_of};
use super::{circuit_panel, same_network};
use crate::{Handle, Result};

/// What a check of the circuits assumes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitOptions {
    /// Whether the building has its own HV/LV substation (the voltage drop may then be 6 % and 8 %, not 3 % and 5 %).
    #[serde(default)]
    pub own_substation: bool,
    /// The share of the installed power that is on at once: 1 counts everything.
    #[serde(default = "one")]
    pub diversity: f64,
    /// The short-circuit current at the origin of the installation (the secondary of the transformer, or the meter), kA, three-phase: the greatest the
    /// boards can see. 10 kA by default, to be given by the supplier of the network.
    #[serde(default = "ten")]
    pub source_ka: f64,
    /// The breaking capacity of the breakers, kA (10 kA by default).
    #[serde(default = "ten")]
    pub breaking_ka: f64,
    /// The curve of the breakers' magnetic trip: B, C or D.
    #[serde(default = "curve_c")]
    pub curve: char,
}

fn ten() -> f64 {
    10.0
}

fn curve_c() -> char {
    'C'
}

fn one() -> f64 {
    1.0
}

impl Default for CircuitOptions {
    fn default() -> Self {
        CircuitOptions { own_substation: false, diversity: 1.0, source_ka: ten(), breaking_ka: ten(), curve: curve_c() }
    }
}

/// One circuit as the drawing has it, and as it should be.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitReport {
    pub name: String,
    /// The distribution board it starts from: what comes before the first hyphen of its name.
    pub panel: String,
    /// Whether that board is drawn.
    pub panel_drawn: bool,
    pub usage: String,
    pub phases: String,
    pub devices: usize,
    /// The equipment and units that are fed by the circuit.
    pub hvac: usize,
    pub power_w: f64,
    pub current_a: f64,
    pub cable_length_m: f64,
    /// The section drawn (the smallest of the cables of the circuit), mm².
    pub section_mm2: Option<f64>,
    /// The section the current needs, and the one the voltage drop needs, mm².
    pub thermal_section_mm2: Option<f64>,
    pub drop_section_mm2: Option<f64>,
    /// The section to use: the larger of the two.
    pub required_section_mm2: Option<f64>,
    /// The protection drawn, A, and the rating that is recommended.
    pub protection_a: Option<f64>,
    pub rating_a: Option<f64>,
    /// The voltage drop with the section drawn (or the required one), percent, and the most it may be.
    pub drop_percent: Option<f64>,
    pub max_drop_percent: f64,
    /// The voltage drop already taken along the circuits that feed this one's board (a board fed by another board's circuit), percent: the drop of
    /// a circuit is counted from the origin of the installation.
    #[serde(default)]
    pub upstream_drop_percent: Option<f64>,
    /// The boards this circuit feeds (a distribution board whose "circuit" is this one): the power of their circuits is its load.
    #[serde(default)]
    pub feeds: Vec<String>,
    /// The greatest three-phase (or phase-neutral) short-circuit current at the end of the circuit, kA, and the smallest one (phase to protective
    /// conductor), A; the longest cable, m, the magnetic trip of the protection still sees the smallest one at the end of.
    #[serde(default)]
    pub ik_max_ka: Option<f64>,
    #[serde(default)]
    pub ik_min_a: Option<f64>,
    #[serde(default)]
    pub max_length_m: Option<f64>,
    /// The rating of the main breaker of the board over the rating of this circuit's: 1.6 or more is a rule of thumb for some selectivity.
    #[serde(default)]
    pub selectivity_ratio: Option<f64>,
    pub warnings: Vec<String>,
    pub cables: Vec<String>,
}

impl CircuitReport {
    pub fn ok(&self) -> bool {
        self.warnings.is_empty()
    }
}

#[derive(Clone, Default)]
struct Acc {
    name: String,
    cables: Vec<(Handle, CableParams)>,
    power_w: f64,
    devices: usize,
    hvac: usize,
    lights_only: bool,
    /// The boards this circuit feeds.
    fed: Vec<String>,
}

fn grouping(laying: ElecLaying) -> calc::Grouping {
    match laying {
        ElecLaying::B => calc::Grouping::Embedded,
        ElecLaying::C => calc::Grouping::WallsAndFloors,
        ElecLaying::E | ElecLaying::F => calc::Grouping::PerforatedTray,
    }
}

impl crate::Drawing {
    /// The names of the distribution boards drawn.
    pub fn panel_names(&self) -> Vec<String> {
        self.model
            .iter()
            .filter_map(|e| device_of(e).filter(|d| d.kind == DeviceKind::Panel))
            .map(|d| d.name)
            .filter(|n| !n.trim().is_empty())
            .collect()
    }

    /// The circuits of model space, each with its calculation and what is wrong with it, by name.
    pub fn circuit_reports(&self, opts: &CircuitOptions) -> Vec<CircuitReport> {
        let diversity = if opts.diversity.is_finite() { opts.diversity.clamp(0.0, 1.0) } else { 1.0 };
        let mut circuits: BTreeMap<String, Acc> = BTreeMap::new();
        let mut add = |name: &str, f: &mut dyn FnMut(&mut Acc)| {
            let key = name.trim().to_lowercase();
            if key.is_empty() {
                return;
            }
            let acc = circuits.entry(key).or_insert_with(|| Acc { name: name.trim().to_string(), lights_only: true, ..Acc::default() });
            f(acc);
        };
        for e in self.model.iter() {
            if let Some(c) = cable_of(e) {
                let h = e.handle;
                add(&c.circuit.clone(), &mut |a| a.cables.push((h, c.clone())));
            } else if let Some(d) = device_of(e) {
                if d.kind == DeviceKind::Panel {
                    // A board fed by a circuit (of another board): that circuit carries the load of the board's circuits.
                    if !d.circuit.trim().is_empty() && !d.name.trim().is_empty() {
                        add(&d.circuit.clone(), &mut |a| {
                            a.fed.push(d.name.trim().to_string());
                            a.lights_only = false;
                        });
                    }
                    continue;
                }
                add(&d.circuit.clone(), &mut |a| {
                    a.devices += 1;
                    a.power_w += d.power_w;
                    a.lights_only &= d.kind.is_light() || d.kind.is_emergency();
                });
            } else if let Some(q) = equipment_of(e) {
                add(&q.circuit.clone(), &mut |a| {
                    a.hvac += 1;
                    a.power_w += q.electric_kw.unwrap_or(0.0) * 1000.0;
                    a.lights_only = false;
                });
            } else if let Some(u) = ahu_of(e) {
                add(&u.circuit.clone(), &mut |a| {
                    a.hvac += 1;
                    a.power_w += u.electric_kw.unwrap_or(0.0) * 1000.0;
                    a.lights_only = false;
                });
            }
        }
        let panels = self.panel_names();
        // The rating of the main breaker of each board, by the board's name.
        let supplies: BTreeMap<String, f64> = self
            .model
            .iter()
            .filter_map(|e| device_of(e).filter(|d| d.kind == DeviceKind::Panel))
            .filter_map(|d| Some((d.name.trim().to_lowercase(), d.supply_a?)))
            .collect();
        // The load of a circuit that feeds boards: its own, and the circuits of those boards, down the chain (not more than six levels: a loop is cut).
        let own: BTreeMap<String, f64> = circuits.iter().map(|(k, a)| (k.clone(), a.power_w)).collect();
        fn total(key: &str, depth: usize, circuits: &BTreeMap<String, Acc>, own: &BTreeMap<String, f64>) -> f64 {
            let Some(a) = circuits.get(key) else { return 0.0 };
            let mut sum = own.get(key).copied().unwrap_or(0.0);
            if depth < 6 {
                for panel in &a.fed {
                    for (k, _) in circuits.iter().filter(|(k, c)| k.as_str() != key && same_network(circuit_panel(&c.name), panel)) {
                        sum += total(k, depth + 1, circuits, own);
                    }
                }
            }
            sum
        }
        let totals: BTreeMap<String, f64> = circuits.keys().map(|k| (k.clone(), total(k, 0, &circuits, &own))).collect();
        for (k, a) in circuits.iter_mut() {
            a.power_w = totals.get(k).copied().unwrap_or(a.power_w);
        }
        // Where each board's circuits start: the origin's short-circuit current, or what is left at the end of the circuit that feeds the board, and the
        // voltage drop already taken. Worked out until it stops changing, a level of boards at a time.
        let feeders: Vec<(String, String)> =
            circuits.values().flat_map(|a| a.fed.iter().map(|p| (p.to_lowercase(), a.name.to_lowercase()))).collect();
        let mut origins: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        let mut reports = Vec::new();
        for _ in 0..7 {
            reports = circuits
                .values()
                .map(|a| {
                    let origin = origins.get(&circuit_panel(&a.name).trim().to_lowercase()).copied().unwrap_or((opts.source_ka, 0.0));
                    circuit_report(a.clone(), &panels, &supplies, diversity, opts, origin)
                })
                .collect::<Vec<_>>();
            let mut next = origins.clone();
            for (panel, feeder) in &feeders {
                if let Some(r) = reports.iter().find(|r| r.name.to_lowercase() == *feeder) {
                    let ik = r.ik_max_ka.unwrap_or(opts.source_ka);
                    let drop = r.upstream_drop_percent.unwrap_or(0.0) + r.drop_percent.unwrap_or(0.0);
                    next.insert(panel.clone(), (ik, drop));
                }
            }
            if next == origins {
                break;
            }
            origins = next;
        }
        reports
    }

    /// Gives every cable of circuit `name` the sizes the calculation found for the circuit: the section where there is none (or everywhere when
    /// `overwrite`) and the protection where there is none. Returns the circuits as they are after. A cable is regenerated: its label shows the section.
    pub fn size_circuits(&mut self, opts: &CircuitOptions, only: Option<&str>, overwrite: bool) -> Result<Vec<CircuitReport>> {
        let reports = self.circuit_reports(opts);
        for r in &reports {
            if only.is_some_and(|n| !same_network(n, &r.name)) {
                continue;
            }
            let Some(section) = r.required_section_mm2 else { continue };
            let hs: Vec<Handle> =
                self.model.iter().filter(|e| cable_of(e).is_some_and(|c| same_network(&c.circuit, &r.name))).map(|e| e.handle).collect();
            for h in hs {
                let Some((mut c, ..)) = self.cable_ends(h) else { continue };
                let mut changed = false;
                if overwrite || c.section_mm2.is_none() {
                    changed |= c.section_mm2 != Some(section);
                    c.section_mm2 = Some(section);
                }
                if c.protection_a.is_none() {
                    c.protection_a = r.rating_a;
                    changed |= r.rating_a.is_some();
                }
                if changed {
                    self.regenerate_cable(h, &c)?;
                }
            }
        }
        self.refresh_circuit_legends();
        Ok(self.circuit_reports(opts))
    }
}

/// `origin` is where the circuit's board gets its current: the short-circuit current there (kA) and the voltage drop already taken (percent).
fn circuit_report(
    a: Acc,
    panels: &[String],
    supplies: &BTreeMap<String, f64>,
    diversity: f64,
    opts: &CircuitOptions,
    origin: (f64, f64),
) -> CircuitReport {
    let first = a.cables.first().map(|(_, c)| c.clone());
    let usage = first.as_ref().map(|c| c.usage).unwrap_or(if a.lights_only { CableUsage::Lighting } else { CableUsage::Power });
    let phases = first.as_ref().map_or(ElecPhases::Single, |c| c.phases);
    let conductor = first.as_ref().map_or(ElecConductor::Copper, |c| c.conductor);
    let insulation = first.as_ref().map_or(ElecInsulation::Pvc, |c| c.insulation);
    let laying = first.as_ref().map_or(ElecLaying::B, |c| c.laying);
    let cos_phi = first.as_ref().map_or(0.9, |c| c.cos_phi);
    let panel = circuit_panel(&a.name).to_string();
    let mut r = CircuitReport {
        name: a.name.clone(),
        panel_drawn: panels.iter().any(|p| same_network(p, &panel)),
        panel,
        usage: usage.name().into(),
        phases: phases.name().into(),
        devices: a.devices,
        hvac: a.hvac,
        power_w: (a.power_w * diversity).round(),
        max_drop_percent: calc::max_voltage_drop_percent(usage.drop_usage(), opts.own_substation),
        cables: a.cables.iter().map(|(h, _)| h.hex()).collect(),
        feeds: a.fed.clone(),
        upstream_drop_percent: (origin.1 > 0.0).then_some((origin.1 * 100.0).round() / 100.0),
        ..CircuitReport::default()
    };
    let length_m: f64 = a.cables.iter().map(|(_, c)| c.length_m()).sum();
    r.cable_length_m = (length_m * 100.0).round() / 100.0;
    r.section_mm2 = a.cables.iter().filter_map(|(_, c)| c.section_mm2).min_by(f64::total_cmp);
    r.protection_a = a.cables.iter().filter_map(|(_, c)| c.protection_a).min_by(f64::total_cmp);
    if !r.panel.is_empty() && !r.panel_drawn && a.name.contains('-') {
        r.warnings.push(format!("the distribution board {} is not drawn", r.panel));
    }
    if a.cables.is_empty() {
        r.warnings.push("no cable is drawn for this circuit".into());
    }
    if r.power_w <= 0.0 {
        // A data or bus circuit carries no power: nothing to say.
        if !usage.is_signal() {
            r.warnings.push("the circuit carries no power: type the power of its devices or equipment".into());
        }
        return r;
    }
    let Ok(ib) = calc::design_current(r.power_w, phases.calc(), cos_phi) else { return r };
    r.current_a = (ib * 100.0).round() / 100.0;
    let rating = r.protection_a.or_else(|| calc::protection_rating(ib).ok());
    r.rating_a = rating;
    let Some(rating) = rating else {
        r.warnings.push("the current is above the largest rating of the table".into());
        return r;
    };
    if let Some(p) = r.protection_a
        && p < ib
    {
        r.warnings.push(format!("the protection ({p} A) is below the current of the circuit ({ib:.1} A)"));
    }
    // The factors of the laying.
    let (ambient, grouped) = first.as_ref().map_or((30.0, 1), |c| (c.ambient_c, c.grouped));
    let Some(k3) = calc::k3(insulation.calc(), ambient) else {
        r.warnings.push("the insulation does not stand that ambient temperature".into());
        return r;
    };
    let k = calc::k1(laying.calc(), false, false, false) * calc::k2(grouping(laying), grouped as usize) * k3;
    match calc::size_cable(conductor.calc(), laying.calc(), insulation.calc(), phases.calc(), rating, k) {
        Ok(c) => r.thermal_section_mm2 = Some(c.section),
        Err(e) => {
            r.warnings.push(e.to_string());
            return r;
        }
    }
    // The section the voltage drop needs: the smallest of the table whose drop is within the limit.
    if length_m > 0.0 {
        r.drop_section_mm2 = calc::sections(conductor.calc()).into_iter().find(|s| {
            calc::voltage_drop(conductor.calc(), *s, length_m, ib, phases.calc(), cos_phi)
                .is_ok_and(|du| calc::voltage_drop_percent(du, phases.calc()) + origin.1 <= r.max_drop_percent)
        });
        if r.drop_section_mm2.is_none() {
            r.warnings.push("no section of the table keeps the voltage drop within its limit: shorten the circuit or use cables in parallel".into());
        }
    }
    r.required_section_mm2 = match (r.thermal_section_mm2, r.drop_section_mm2) {
        (Some(t), Some(d)) => Some(t.max(d)),
        (t, d) => t.or(d),
    };
    // What is drawn.
    let judged = r.section_mm2.or(r.required_section_mm2);
    if let (Some(s), true) = (judged, length_m > 0.0)
        && let Ok(du) = calc::voltage_drop(conductor.calc(), s, length_m, ib, phases.calc(), cos_phi)
    {
        let pct = calc::voltage_drop_percent(du, phases.calc());
        r.drop_percent = Some((pct * 100.0).round() / 100.0);
        if r.section_mm2.is_some() && pct + origin.1 > r.max_drop_percent {
            let upstream = if origin.1 > 0.0 { format!(" with the {:.1} % of the circuits upstream", origin.1) } else { String::new() };
            r.warnings.push(format!("the voltage drop ({:.1} %{upstream}) is above {:.0} %", pct + origin.1, r.max_drop_percent));
        }
    }
    short_circuit(&mut r, conductor, phases, judged, length_m, rating, supplies, opts, origin.0);
    // A circuit that feeds a board has the main breaker of that board downstream of it.
    for fed in &a.fed {
        if let Some(supply) = supplies.get(&fed.trim().to_lowercase()).copied().filter(|v| *v > 0.0) {
            let ratio = rating / supply;
            if ratio < calc::SELECTIVITY_RATIO {
                r.warnings.push(format!(
                    "the protection of this circuit ({rating:.0} A) is only {ratio:.1} times the main breaker of the board {fed} it feeds ({supply:.0} A): no selectivity can be expected"
                ));
            }
        }
    }
    if let (Some(drawn), Some(thermal)) = (r.section_mm2, r.thermal_section_mm2)
        && drawn < thermal
    {
        r.warnings.push(format!("the section drawn ({drawn} mm²) is below the one the current needs ({thermal} mm²)"));
    }
    if let Some(drawn) = r.section_mm2
        && let Some(iz) = calc::current_capacity(conductor.calc(), drawn, laying.calc(), insulation.calc(), phases.calc().loaded())
        && rating > iz * k
        && r.thermal_section_mm2.is_some_and(|t| drawn >= t)
    {
        r.warnings.push(format!("the protection ({rating} A) is above what the cable carries ({:.0} A)", iz * k));
    }
    r
}

/// The short-circuit checks of a circuit: what a fault does at its end, whether the breaker breaks it and sees it, and whether the board's main breaker
/// leaves room for the circuit's to trip first.
#[allow(clippy::too_many_arguments)]
fn short_circuit(
    r: &mut CircuitReport,
    conductor: ElecConductor,
    phases: ElecPhases,
    section: Option<f64>,
    length_m: f64,
    rating: f64,
    supplies: &BTreeMap<String, f64>,
    opts: &CircuitOptions,
    source_ka: f64,
) {
    // The breaker is at the board, where the fault current is the origin's: it must be able to break it.
    if source_ka.is_finite() && source_ka > opts.breaking_ka {
        r.warnings.push(format!(
            "the breaking capacity of the breakers ({:.0} kA) is below the short-circuit current at the board ({source_ka:.1} kA)",
            opts.breaking_ka
        ));
    }
    if let (Some(s), true) = (section, length_m > 0.0) {
        if let Ok(ik) = calc::short_circuit_max_a(source_ka, conductor.calc(), s, length_m, phases.calc()) {
            r.ik_max_ka = Some((ik / 10.0).round() / 100.0);
        }
        if let Ok(ik) = calc::short_circuit_min_a(conductor.calc(), s, length_m) {
            r.ik_min_a = Some(ik.round());
            // The magnetic trip must see the smallest short circuit at the end of the cable.
            if let Some(m) = calc::magnetic_trip_multiple(opts.curve) {
                let trip = m * rating;
                if let Ok(max) = calc::max_length_for_magnetic_m(conductor.calc(), s, trip) {
                    r.max_length_m = Some(max.round());
                }
                if ik < trip {
                    r.warnings.push(format!(
                        "the smallest short-circuit current at the end of the cable ({ik:.0} A) is below the magnetic trip of the protection ({trip:.0} A, curve {}): the longest cable is {:.0} m, or use a larger section or a curve with a lower trip",
                        opts.curve.to_ascii_uppercase(),
                        r.max_length_m.unwrap_or(0.0)
                    ));
                }
            }
        }
    }
    if let Some(supply) = supplies.get(&r.panel.trim().to_lowercase()).copied().filter(|a| *a > 0.0 && rating > 0.0) {
        let ratio = supply / rating;
        r.selectivity_ratio = Some((ratio * 100.0).round() / 100.0);
        if ratio < calc::SELECTIVITY_RATIO {
            r.warnings.push(format!(
                "the main breaker of the board ({supply:.0} A) is only {ratio:.1} times the protection of the circuit ({rating:.0} A): no selectivity can be expected (a ratio of {} or more is a rule of thumb; the manufacturer's tables prove it)",
                calc::SELECTIVITY_RATIO
            ));
        }
    }
}

/// What to change on every cable of a circuit.
#[derive(Clone, Debug, Default)]
pub struct CircuitChange {
    pub usage: Option<CableUsage>,
    pub phases: Option<ElecPhases>,
    pub conductor: Option<ElecConductor>,
    pub insulation: Option<ElecInsulation>,
    pub laying: Option<ElecLaying>,
    pub section_mm2: Option<Option<f64>>,
    pub protection_a: Option<Option<f64>>,
    pub cos_phi: Option<f64>,
    pub ambient_c: Option<f64>,
    pub grouped: Option<u32>,
    pub extra_m: Option<f64>,
}

impl crate::Drawing {
    /// Changes every cable of circuit `name` as `change` says; returns how many were changed.
    pub fn edit_circuit(&mut self, name: &str, change: &CircuitChange) -> Result<usize> {
        let hs: Vec<Handle> = self.model.iter().filter(|e| cable_of(e).is_some_and(|c| same_network(&c.circuit, name))).map(|e| e.handle).collect();
        let mut next = Vec::new();
        for h in hs {
            let Some((mut c, ..)) = self.cable_ends(h) else { continue };
            if let Some(v) = change.usage {
                c.usage = v;
            }
            if let Some(v) = change.phases {
                c.phases = v;
            }
            if let Some(v) = change.conductor {
                c.conductor = v;
            }
            if let Some(v) = change.insulation {
                c.insulation = v;
            }
            if let Some(v) = change.laying {
                c.laying = v;
            }
            if let Some(v) = change.section_mm2 {
                c.section_mm2 = v;
            }
            if let Some(v) = change.protection_a {
                c.protection_a = v;
            }
            if let Some(v) = change.cos_phi {
                c.cos_phi = v;
            }
            if let Some(v) = change.ambient_c {
                c.ambient_c = v;
            }
            if let Some(v) = change.grouped {
                c.grouped = v;
            }
            if let Some(v) = change.extra_m {
                c.extra_m = v;
            }
            c.validate()?;
            next.push((h, c));
        }
        for (h, c) in &next {
            self.regenerate_cable(*h, c)?;
        }
        self.refresh_circuit_legends();
        Ok(next.len())
    }

    /// Puts `handles` (cables, devices, equipment, units) on circuit `to` (empty: on none); returns how many were changed.
    pub fn assign_circuit(&mut self, handles: &[Handle], to: &str) -> Result<usize> {
        super::check_circuit_name(to, true)?;
        let to = to.trim();
        let mut n = 0;
        for h in handles {
            let Some(e) = self.entity(*h).cloned() else { continue };
            if let Some(mut c) = cable_of(&e) {
                c.circuit = to.to_string();
                self.regenerate_cable(*h, &c)?;
            } else if let Some(mut d) = device_of(&e) {
                if d.kind == DeviceKind::Panel {
                    continue;
                }
                d.circuit = to.to_string();
                self.regenerate_device(*h, &d)?;
            } else if let Some(mut q) = equipment_of(&e) {
                q.circuit = to.to_string();
                self.regenerate_equipment(*h, &q)?;
            } else if let Some(mut u) = ahu_of(&e) {
                u.circuit = to.to_string();
                self.regenerate_ahu(*h, &u)?;
            } else {
                continue;
            }
            n += 1;
        }
        if !to.is_empty() && self.network_style(to).is_none() {
            self.set_network_style(to, None, None)?;
        }
        self.refresh_circuit_legends();
        Ok(n)
    }

    /// Renames circuit `from` to `to` everywhere (cables, devices, equipment, units); the style follows. Returns how many objects changed.
    pub fn rename_circuit(&mut self, from: &str, to: &str) -> Result<usize> {
        super::check_circuit_name(to, false)?;
        let hs: Vec<Handle> = self
            .model
            .iter()
            .filter(|e| {
                cable_of(e).is_some_and(|c| same_network(&c.circuit, from))
                    || device_of(e).is_some_and(|d| d.kind != DeviceKind::Panel && same_network(&d.circuit, from))
                    || equipment_of(e).is_some_and(|q| same_network(&q.circuit, from))
                    || ahu_of(e).is_some_and(|u| same_network(&u.circuit, from))
            })
            .map(|e| e.handle)
            .collect();
        let style = self.network_style(from).cloned();
        let n = self.assign_circuit(&hs, to)?;
        if let Some(s) = style {
            self.set_network_style(to, Some(s.color), Some(&s.linetype))?;
        }
        Ok(n)
    }
}
