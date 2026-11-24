//! Electrical calculations for a low-voltage installation of a building: the current a circuit carries, the cable that carries it, the
//! voltage drop along it and the illuminance of a room. The formulas and the tables are those of the French rules for low-voltage
//! installations as the public technical guides of manufacturers reproduce them (the method of the current-carrying capacity by selection
//! letter and correction factors, and the voltage-drop formula with the resistivity and reactance they give), written here as plain tables
//! and functions. They help a design; the standard and the engineer's own checks decide.
//!
//! Sources: Schneider Electric, "Compléments techniques du catalogue distribution électrique 2016/2017", pages A46 to A49 (current-carrying
//! capacities, correction factors K1, K2, K3, voltage drops); the minimum illuminances are those of the Code du travail, article R4223-4.
//!
//! Units: currents in A, powers in W, lengths in m, sections in mm², voltages in V, illuminances in lux, fluxes in lumen.

use crate::{HvacError, Result, checked};

/// The metal of the conductors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conductor {
    Copper,
    Aluminium,
}

/// The insulation: PVC (the 70 °C thermoplastics) or PR (cross-linked polyethylene and the like, 90 °C).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insulation {
    Pvc,
    Pr,
}

/// The selection letter of the way a cable is laid: B (in conduit or trunking on a wall), C (clipped direct on a wall or a ceiling), E (multi-core
/// cable on a perforated tray, in the air), F (single-core cables on a tray, in the air).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Laying {
    B,
    C,
    E,
    F,
}

/// Single-phase (230 V) or three-phase (400 V).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phases {
    Single,
    Three,
}

impl Phases {
    /// The voltage a drop is compared with: phase to neutral for a single-phase circuit, between phases for a three-phase one.
    pub fn voltage(self) -> f64 {
        match self {
            Phases::Single => 230.0,
            Phases::Three => 400.0,
        }
    }
    /// The number of conductors that carry the current: 2 or 3.
    pub fn loaded(self) -> u8 {
        match self {
            Phases::Single => 2,
            Phases::Three => 3,
        }
    }
}

/// What a circuit feeds, for the voltage drop it may have: lighting 3 %, anything else 5 % (a customer supplied by the public low-voltage
/// network), and 6 % and 8 % for a customer who owns the HV/LV substation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    Lighting,
    Other,
}

/// The voltage drop a circuit may have, in percent, from the origin of the installation.
pub fn max_voltage_drop_percent(usage: Usage, own_substation: bool) -> f64 {
    match (usage, own_substation) {
        (Usage::Lighting, false) => 3.0,
        (Usage::Other, false) => 5.0,
        (Usage::Lighting, true) => 6.0,
        (Usage::Other, true) => 8.0,
    }
}

/// The standard ratings of circuit breakers, A.
pub const PROTECTION_RATINGS: [f64; 20] =
    [2.0, 4.0, 6.0, 10.0, 13.0, 16.0, 20.0, 25.0, 32.0, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 320.0, 400.0];

/// The smallest standard rating that is at least `ib` amperes.
pub fn protection_rating(ib: f64) -> Result<f64> {
    let ib = checked("current (A)", ib, 0.0, 10_000.0)?;
    PROTECTION_RATINGS
        .iter()
        .copied()
        .find(|r| *r >= ib)
        .ok_or_else(|| HvacError::BadValue(format!("{ib} A is above the largest rating of the table")))
}

/// Current-carrying capacity of a cable in free conditions (30 °C, one circuit): per section, nine columns from the left of the guide's table;
/// [`column`] says which one a laying, an insulation and a number of loaded conductors read. Empty where the table has no value.
const IZ_COPPER: &[(f64, [Option<f64>; 9])] = &[
    (1.5, [Some(15.5), Some(17.5), Some(18.5), Some(19.5), Some(22.0), Some(23.0), Some(24.0), Some(26.0), None]),
    (2.5, [Some(21.0), Some(24.0), Some(25.0), Some(27.0), Some(30.0), Some(31.0), Some(33.0), Some(36.0), None]),
    (4.0, [Some(28.0), Some(32.0), Some(34.0), Some(36.0), Some(40.0), Some(42.0), Some(45.0), Some(49.0), None]),
    (6.0, [Some(36.0), Some(41.0), Some(43.0), Some(48.0), Some(51.0), Some(54.0), Some(58.0), Some(63.0), None]),
    (10.0, [Some(50.0), Some(57.0), Some(60.0), Some(63.0), Some(70.0), Some(75.0), Some(80.0), Some(86.0), None]),
    (16.0, [Some(68.0), Some(76.0), Some(80.0), Some(85.0), Some(94.0), Some(100.0), Some(107.0), Some(115.0), None]),
    (25.0, [Some(89.0), Some(96.0), Some(101.0), Some(112.0), Some(119.0), Some(127.0), Some(138.0), Some(149.0), Some(161.0)]),
    (35.0, [Some(110.0), Some(119.0), Some(126.0), Some(138.0), Some(147.0), Some(158.0), Some(169.0), Some(185.0), Some(200.0)]),
    (50.0, [Some(134.0), Some(144.0), Some(153.0), Some(168.0), Some(179.0), Some(192.0), Some(207.0), Some(225.0), Some(242.0)]),
    (70.0, [Some(171.0), Some(184.0), Some(196.0), Some(213.0), Some(229.0), Some(246.0), Some(268.0), Some(289.0), Some(310.0)]),
    (95.0, [Some(207.0), Some(223.0), Some(238.0), Some(258.0), Some(278.0), Some(298.0), Some(328.0), Some(352.0), Some(377.0)]),
    (120.0, [Some(239.0), Some(259.0), Some(276.0), Some(299.0), Some(322.0), Some(346.0), Some(382.0), Some(410.0), Some(437.0)]),
    (150.0, [None, Some(299.0), Some(319.0), Some(344.0), Some(371.0), Some(395.0), Some(441.0), Some(473.0), Some(504.0)]),
    (185.0, [None, Some(341.0), Some(364.0), Some(392.0), Some(424.0), Some(450.0), Some(506.0), Some(542.0), Some(575.0)]),
    (240.0, [None, Some(403.0), Some(430.0), Some(461.0), Some(500.0), Some(538.0), Some(599.0), Some(641.0), Some(679.0)]),
    (300.0, [None, Some(464.0), Some(497.0), Some(530.0), Some(576.0), Some(621.0), Some(693.0), Some(741.0), Some(783.0)]),
];
const IZ_ALUMINIUM: &[(f64, [Option<f64>; 9])] = &[
    (2.5, [Some(16.5), Some(18.5), Some(19.5), Some(21.0), Some(23.0), Some(25.0), Some(26.0), Some(28.0), None]),
    (4.0, [Some(22.0), Some(25.0), Some(26.0), Some(28.0), Some(31.0), Some(33.0), Some(35.0), Some(38.0), None]),
    (6.0, [Some(28.0), Some(32.0), Some(33.0), Some(36.0), Some(39.0), Some(43.0), Some(45.0), Some(49.0), None]),
    (10.0, [Some(39.0), Some(44.0), Some(46.0), Some(49.0), Some(54.0), Some(58.0), Some(62.0), Some(67.0), None]),
    (16.0, [Some(53.0), Some(59.0), Some(61.0), Some(66.0), Some(73.0), Some(77.0), Some(84.0), Some(91.0), None]),
    (25.0, [Some(70.0), Some(73.0), Some(78.0), Some(83.0), Some(90.0), Some(97.0), Some(101.0), Some(108.0), Some(121.0)]),
    (35.0, [Some(86.0), Some(90.0), Some(96.0), Some(103.0), Some(112.0), Some(120.0), Some(126.0), Some(135.0), Some(150.0)]),
    (50.0, [Some(104.0), Some(110.0), Some(117.0), Some(125.0), Some(136.0), Some(146.0), Some(154.0), Some(164.0), Some(184.0)]),
    (70.0, [Some(133.0), Some(140.0), Some(150.0), Some(160.0), Some(174.0), Some(187.0), Some(198.0), Some(211.0), Some(237.0)]),
    (95.0, [Some(161.0), Some(170.0), Some(183.0), Some(195.0), Some(211.0), Some(227.0), Some(241.0), Some(257.0), Some(289.0)]),
    (120.0, [Some(186.0), Some(197.0), Some(212.0), Some(226.0), Some(245.0), Some(263.0), Some(280.0), Some(300.0), Some(337.0)]),
    (150.0, [None, Some(227.0), Some(245.0), Some(261.0), Some(283.0), Some(304.0), Some(324.0), Some(346.0), Some(389.0)]),
    (185.0, [None, Some(259.0), Some(280.0), Some(298.0), Some(323.0), Some(347.0), Some(371.0), Some(397.0), Some(447.0)]),
    (240.0, [None, Some(305.0), Some(330.0), Some(352.0), Some(382.0), Some(409.0), Some(439.0), Some(470.0), Some(530.0)]),
    (300.0, [None, Some(351.0), Some(381.0), Some(406.0), Some(440.0), Some(471.0), Some(508.0), Some(543.0), Some(613.0)]),
];

/// The column of the table for a laying, an insulation and 2 or 3 loaded conductors. B reads columns 0, 1, 3, 5; C 1, 3, 4, 6; E 2, 4, 5, 7;
/// F 3, 5, 6, 8 (PVC with 3, PVC with 2, PR with 3, PR with 2 loaded conductors).
fn column(laying: Laying, insulation: Insulation, loaded: u8) -> usize {
    let k = match (insulation, loaded >= 3) {
        (Insulation::Pvc, true) => 0,
        (Insulation::Pvc, false) => 1,
        (Insulation::Pr, true) => 2,
        (Insulation::Pr, false) => 3,
    };
    let columns: [usize; 4] = match laying {
        Laying::B => [0, 1, 3, 5],
        Laying::C => [1, 3, 4, 6],
        Laying::E => [2, 4, 5, 7],
        Laying::F => [3, 5, 6, 8],
    };
    columns.get(k).copied().unwrap_or(0)
}

/// The sections of the table for a metal, mm².
pub fn sections(conductor: Conductor) -> Vec<f64> {
    table(conductor).iter().map(|(s, _)| *s).collect()
}

fn table(conductor: Conductor) -> &'static [(f64, [Option<f64>; 9])] {
    match conductor {
        Conductor::Copper => IZ_COPPER,
        Conductor::Aluminium => IZ_ALUMINIUM,
    }
}

/// The current a cable of `section` carries in free conditions, A, when the table has the value.
pub fn current_capacity(conductor: Conductor, section: f64, laying: Laying, insulation: Insulation, loaded: u8) -> Option<f64> {
    let col = column(laying, insulation, loaded);
    table(conductor).iter().find(|(s, _)| (*s - section).abs() < 1e-9).and_then(|(_, v)| v.get(col).copied().flatten())
}

/// The correction factor K1 of the way of laying: 0.70 for cables in thermally insulating walls, 0.77 for conduits in them, 0.90 for multi-core cables in
/// B, 0.95 in construction voids and under a ceiling in C, 1 otherwise.
pub fn k1(laying: Laying, in_insulating_wall: bool, conduit_in_insulating_wall: bool, under_ceiling: bool) -> f64 {
    match laying {
        Laying::B if in_insulating_wall => 0.70,
        Laying::B if conduit_in_insulating_wall => 0.77,
        Laying::C if under_ceiling => 0.95,
        _ => 1.0,
    }
}

/// How the circuits are grouped, for the factor K2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    /// Touching, embedded or in the walls (B, C, F).
    Embedded,
    /// Touching, one layer on walls or floors (C).
    WallsAndFloors,
    /// Touching, one layer under the ceiling (C).
    Ceiling,
    /// One layer on perforated trays (E, F).
    PerforatedTray,
    /// One layer on ladders and brackets (E, F).
    Ladder,
}

/// The correction factor K2 of `n` circuits touching one another: the guide's table for 1 to 9, 12, 16 and 20 circuits (the nearest lower one is read
/// in between), and above 9 circuits on a wall, floor, ceiling, tray or ladder no further reduction.
pub fn k2(grouping: Grouping, n: usize) -> f64 {
    let (rows, last): (&[f64], f64) = match grouping {
        Grouping::Embedded => (&[1.0, 0.80, 0.70, 0.65, 0.60, 0.55, 0.55, 0.50, 0.50], 0.45),
        Grouping::WallsAndFloors => (&[1.0, 0.85, 0.79, 0.75, 0.73, 0.72, 0.72, 0.71, 0.70], 0.70),
        Grouping::Ceiling => (&[1.0, 0.85, 0.76, 0.72, 0.69, 0.67, 0.66, 0.65, 0.64], 0.64),
        Grouping::PerforatedTray => (&[1.0, 0.88, 0.82, 0.77, 0.75, 0.73, 0.73, 0.72, 0.72], 0.72),
        Grouping::Ladder => (&[1.0, 0.88, 0.82, 0.80, 0.80, 0.79, 0.79, 0.78, 0.78], 0.78),
    };
    match n {
        0 | 1 => 1.0,
        2..=9 => rows.get(n - 1).copied().unwrap_or(last),
        10..=11 => rows.get(8).copied().unwrap_or(last),
        12..=15 if grouping == Grouping::Embedded => 0.45,
        16..=19 if grouping == Grouping::Embedded => 0.40,
        _ if grouping == Grouping::Embedded => 0.40,
        _ => last,
    }
}

/// The correction factor K3 of the ambient temperature, from the guide's table (10 to 60 °C by 5): the nearest hotter temperature of the table
/// is read when the ambient one is not in it; `None` when the insulation cannot stand it.
pub fn k3(insulation: Insulation, ambient_c: f64) -> Option<f64> {
    // PVC, then PR, for 10, 15, … 60 °C.
    const PVC: [f64; 11] = [1.22, 1.17, 1.12, 1.06, 1.0, 0.94, 0.87, 0.79, 0.71, 0.61, 0.50];
    const PR: [f64; 11] = [1.15, 1.12, 1.08, 1.04, 1.0, 0.96, 0.91, 0.87, 0.82, 0.76, 0.71];
    if !ambient_c.is_finite() || ambient_c > 60.0 {
        return None;
    }
    let i = (((ambient_c.max(10.0) - 10.0) / 5.0).ceil() as usize).min(10);
    match insulation {
        Insulation::Pvc => PVC.get(i).copied(),
        Insulation::Pr => PR.get(i).copied(),
    }
}

/// The correction factor of a loaded neutral (three-phase circuits with harmonics): 0.84, and the section rule that goes with it is the user's.
pub const K_LOADED_NEUTRAL: f64 = 0.84;

/// The current a circuit carries, A: `power_w` at `cos_phi`, on the voltage of `phases`.
pub fn design_current(power_w: f64, phases: Phases, cos_phi: f64) -> Result<f64> {
    let p = checked("power (W)", power_w, 0.0, 1.0e9)?;
    let c = checked("power factor", cos_phi, 0.1, 1.0)?;
    Ok(match phases {
        Phases::Single => p / (phases.voltage() * c),
        Phases::Three => p / (3.0_f64.sqrt() * phases.voltage() * c),
    })
}

/// What a cable chosen for a circuit is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CableChoice {
    /// The section, mm².
    pub section: f64,
    /// The current it carries once the correction factor is applied, A.
    pub capacity: f64,
}

/// The smallest section whose capacity, with the correction factor `k` (the product of K1, K2, K3 and the others), is at least `in_a`, the rating of
/// the protection (which is at least the design current). An error when no section of the table carries it.
pub fn size_cable(conductor: Conductor, laying: Laying, insulation: Insulation, phases: Phases, in_a: f64, k: f64) -> Result<CableChoice> {
    let in_a = checked("rating (A)", in_a, 0.0, 10_000.0)?;
    let k = checked("correction factor", k, 0.1, 1.5)?;
    for (s, _) in table(conductor) {
        if let Some(iz) = current_capacity(conductor, *s, laying, insulation, phases.loaded()) {
            let capacity = iz * k;
            if capacity >= in_a {
                return Ok(CableChoice { section: *s, capacity });
            }
        }
    }
    Err(HvacError::BadValue(format!("no section of the table carries {in_a} A: use several cables in parallel")))
}

/// The resistivity of the conductors in Ω·mm²/km (copper 22.5, aluminium 36: the values at the working temperature of the guide).
pub fn resistivity(conductor: Conductor) -> f64 {
    match conductor {
        Conductor::Copper => 22.5,
        Conductor::Aluminium => 36.0,
    }
}

/// The reactance of a conductor in Ω/km when nothing better is known, which only counts from 50 mm² up.
pub const REACTANCE_OHM_KM: f64 = 0.08;

/// The voltage drop along a cable, in volts: ΔU = b·I·L·(R cos φ + X sin φ), with b = 2 for a single-phase circuit and √3 for a three-phase one,
/// R = ρ/S, X the reactance (counted from 50 mm²).
pub fn voltage_drop(conductor: Conductor, section: f64, length_m: f64, current_a: f64, phases: Phases, cos_phi: f64) -> Result<f64> {
    let s = checked("section (mm²)", section, 0.5, 1000.0)?;
    let l = checked("length (m)", length_m, 0.0, 100_000.0)? / 1000.0;
    let i = checked("current (A)", current_a, 0.0, 10_000.0)?;
    let c = checked("power factor", cos_phi, 0.1, 1.0)?;
    let b = match phases {
        Phases::Single => 2.0,
        Phases::Three => 3.0_f64.sqrt(),
    };
    let r = resistivity(conductor) / s;
    let x = if s >= 50.0 { REACTANCE_OHM_KM } else { 0.0 };
    let sin = (1.0 - c * c).sqrt();
    Ok(b * i * l * (r * c + x * sin))
}

/// The resistivity of the conductors in Ω·mm²/km at 20 °C, for the greatest short-circuit current (copper 18.5, aluminium 29.4: the values the
/// guides round the standard ones to).
pub fn resistivity_cold(conductor: Conductor) -> f64 {
    match conductor {
        Conductor::Copper => 18.5,
        Conductor::Aluminium => 29.4,
    }
}

/// The resistivity for the smallest short-circuit current, with the conductors hot after the fault starts: 1.25 times the working one
/// (copper 28.1, aluminium 45 Ω·mm²/km).
pub fn resistivity_fault(conductor: Conductor) -> f64 {
    1.25 * resistivity(conductor)
}

/// The factor of the voltage for the greatest and for the smallest short-circuit current (IEC 60909: low voltage, 6 % tolerance).
pub const C_MAX: f64 = 1.05;
pub const C_MIN: f64 = 0.95;

/// The power factor of the short circuit at the origin of a low-voltage installation, taken for 10 to 20 kA (IEC 60909 guide: 0.3).
pub const SOURCE_COS_PHI: f64 = 0.3;

/// The greatest short-circuit current at the end of a cable, A: the fault current at its origin `source_ka` (three-phase, kA) and the cable, a
/// single-phase circuit counting its neutral in the loop. `Ik = c U0 / |Zs + Zc|` with the source impedance `Zs = c U0 / Ik_source` (per phase)
/// split by `SOURCE_COS_PHI` and the cable `Zc = L (R + jX)` with R at 20 °C.
pub fn short_circuit_max_a(source_ka: f64, conductor: Conductor, section: f64, length_m: f64, phases: Phases) -> Result<f64> {
    let source = checked("short-circuit current at the origin (kA)", source_ka, 0.1, 1000.0)?;
    let s = checked("section (mm²)", section, 0.5, 1000.0)?;
    let l = checked("length (m)", length_m, 0.0, 100_000.0)? / 1000.0;
    let u0 = 230.0;
    let zs = C_MAX * u0 / (source * 1000.0);
    let (rs, xs) = (zs * SOURCE_COS_PHI, zs * (1.0 - SOURCE_COS_PHI * SOURCE_COS_PHI).sqrt());
    let loop_k = match phases {
        Phases::Single => 2.0,
        Phases::Three => 1.0,
    };
    let (rc, xc) = (loop_k * resistivity_cold(conductor) / s * l, loop_k * REACTANCE_OHM_KM * l);
    let z = ((rs + rc).powi(2) + (xs + xc).powi(2)).sqrt();
    Ok(C_MAX * u0 / z)
}

/// The smallest short-circuit current at the end of a cable, A, between a phase and the protective conductor of the same section (m = 1):
/// `Ik = 0.8 U0 / (R phase + R pe)`, the conductors hot. The source impedance is left out, on the safe side.
pub fn short_circuit_min_a(conductor: Conductor, section: f64, length_m: f64) -> Result<f64> {
    let s = checked("section (mm²)", section, 0.5, 1000.0)?;
    let l = checked("length (m)", length_m, 0.001, 100_000.0)? / 1000.0;
    Ok(0.8 * 230.0 / (2.0 * resistivity_fault(conductor) / s * l))
}

/// The longest cable, m, the magnetic trip of a breaker still sees the smallest short circuit at the end of: `0.8 U0 S / (2 ρ Im)`.
pub fn max_length_for_magnetic_m(conductor: Conductor, section: f64, trip_a: f64) -> Result<f64> {
    let s = checked("section (mm²)", section, 0.5, 1000.0)?;
    let im = checked("trip current (A)", trip_a, 1.0, 1.0e6)?;
    Ok(0.8 * 230.0 * s / (2.0 * resistivity_fault(conductor) / 1000.0 * im))
}

/// The curve of the magnetic trip of a breaker: the multiple of its rating at which it trips for sure (IEC 60898: B 5, C 10, D 20).
pub fn magnetic_trip_multiple(curve: char) -> Option<f64> {
    match curve.to_ascii_uppercase() {
        'B' => Some(5.0),
        'C' => Some(10.0),
        'D' => Some(20.0),
        _ => None,
    }
}

/// The ratio from which a breaker upstream is taken to protect with some selectivity a breaker downstream of it: a rule of thumb (1.6 for breakers of the
/// same family), the manufacturer's selectivity tables being the only proof.
pub const SELECTIVITY_RATIO: f64 = 1.6;

/// The voltage drop in percent of the nominal voltage of `phases`.
pub fn voltage_drop_percent(drop_v: f64, phases: Phases) -> f64 {
    100.0 * drop_v / phases.voltage()
}

/// The illuminance a room gets from its luminaires, lux: `total_lumen` of all the luminaires times the utilisation factor (the part of the flux that
/// reaches the work plane) and the maintenance factor (the ageing and dust), over the area. The factors are the designer's: 0.5 and 0.8 are usual
/// when nothing else is known.
pub fn illuminance_lux(total_lumen: f64, area_m2: f64, utilisation: f64, maintenance: f64) -> Result<f64> {
    let flux = checked("flux (lm)", total_lumen, 0.0, 1.0e9)?;
    let area = checked("area (m²)", area_m2, 0.01, 1.0e7)?;
    let uf = checked("utilisation factor", utilisation, 0.05, 1.0)?;
    let mf = checked("maintenance factor", maintenance, 0.05, 1.0)?;
    Ok(flux * uf * mf / area)
}

/// What a room is for, as the Code du travail's table of minimum illuminance (article R4223-4) groups them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomLighting {
    /// Indoor circulation routes: 40 lux.
    Circulation,
    /// Stairs and warehouses: 60 lux.
    StairsWarehouse,
    /// Workrooms, cloakrooms and toilets: 120 lux.
    WorkRoom,
    /// Windowless rooms where permanent work is done: 200 lux.
    BlindWorkRoom,
    /// Outdoor circulation routes and areas: 10 lux.
    OutdoorCirculation,
    /// Outdoor places where permanent work is done: 40 lux.
    OutdoorWork,
}

/// The minimum illuminance of a room, lux, measured at the work plane or on the floor.
pub fn minimum_illuminance(room: RoomLighting) -> f64 {
    match room {
        RoomLighting::Circulation => 40.0,
        RoomLighting::StairsWarehouse => 60.0,
        RoomLighting::WorkRoom => 120.0,
        RoomLighting::BlindWorkRoom => 200.0,
        RoomLighting::OutdoorCirculation => 10.0,
        RoomLighting::OutdoorWork => 40.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minimum_illuminances_are_those_of_the_table_of_article_r4223_4() {
        let lux = minimum_illuminance;
        assert_eq!(
            [
                lux(RoomLighting::Circulation),
                lux(RoomLighting::StairsWarehouse),
                lux(RoomLighting::WorkRoom),
                lux(RoomLighting::BlindWorkRoom),
                lux(RoomLighting::OutdoorCirculation),
                lux(RoomLighting::OutdoorWork)
            ],
            [40.0, 60.0, 120.0, 200.0, 10.0, 40.0]
        );
    }

    #[test]
    fn the_tables_read_the_guide() {
        // 16 mm² copper: letter B PVC3 68, C PVC3 76, E PVC3 80, F PVC3 85, B PR3 85, C PR3 94, E PR3 100, F PR3 107.
        let iz = |l, i| current_capacity(Conductor::Copper, 16.0, l, i, 3).unwrap();
        assert_eq!(
            (iz(Laying::B, Insulation::Pvc), iz(Laying::C, Insulation::Pvc), iz(Laying::E, Insulation::Pvc), iz(Laying::F, Insulation::Pvc)),
            (68.0, 76.0, 80.0, 85.0)
        );
        assert_eq!(
            (iz(Laying::B, Insulation::Pr), iz(Laying::C, Insulation::Pr), iz(Laying::E, Insulation::Pr), iz(Laying::F, Insulation::Pr)),
            (85.0, 94.0, 100.0, 107.0)
        );
        // Two loaded conductors carry more; a section the table lacks has no value; 25 mm² F with PR and 2 loaded exists.
        assert_eq!(current_capacity(Conductor::Copper, 1.5, Laying::B, Insulation::Pvc, 2), Some(17.5));
        assert_eq!(current_capacity(Conductor::Copper, 25.0, Laying::F, Insulation::Pr, 2), Some(161.0));
        assert_eq!(current_capacity(Conductor::Copper, 16.0, Laying::F, Insulation::Pr, 2), None);
        assert_eq!(current_capacity(Conductor::Copper, 3.0, Laying::B, Insulation::Pvc, 3), None);
        assert_eq!(sections(Conductor::Aluminium).first(), Some(&2.5));
    }

    #[test]
    fn a_cable_is_sized_by_the_rating_and_the_factors() {
        // The guide's example: 58 A, tray (E), PR, three-phase, K = 1 x 0.75 x 0.91 x 0.84 (loaded neutral): 58 / 0.573 = 101 A of table: 25 mm²
        // (16 mm² carries 100 A).
        let k = k1(Laying::E, false, false, false) * k2(Grouping::PerforatedTray, 5) * k3(Insulation::Pr, 40.0).unwrap() * K_LOADED_NEUTRAL;
        assert!((k - 0.75 * 0.91 * 0.84).abs() < 1e-9, "{k}");
        let c = size_cable(Conductor::Copper, Laying::E, Insulation::Pr, Phases::Three, 58.0, k).unwrap();
        assert_eq!(c.section, 25.0);
        // A 16 A breaker on a lighting circuit in conduit with PVC: 1.5 mm² carries 15.5 A, so 2.5 mm² is needed; with 2 loaded conductors 17.5 A is enough
        // for 16 A.
        assert_eq!(size_cable(Conductor::Copper, Laying::B, Insulation::Pvc, Phases::Single, 16.0, 1.0).unwrap().section, 1.5);
        assert_eq!(size_cable(Conductor::Copper, Laying::B, Insulation::Pvc, Phases::Three, 16.0, 1.0).unwrap().section, 2.5);
        assert!(size_cable(Conductor::Copper, Laying::B, Insulation::Pvc, Phases::Three, 5000.0, 1.0).is_err());
        assert!(size_cable(Conductor::Copper, Laying::B, Insulation::Pvc, Phases::Three, f64::NAN, 1.0).is_err());
    }

    #[test]
    fn the_voltage_drop_follows_the_guide() {
        // Guide table: 100 m of 10 mm² copper, 400 V three-phase, 50 A, cos φ = 1: 2.5 %? (R = 2.25 Ω/km: sqrt3 x 50 x 0.1 x 2.25 = 19.5 V = 4.9 %).
        let du = voltage_drop(Conductor::Copper, 10.0, 100.0, 50.0, Phases::Three, 1.0).unwrap();
        assert!((voltage_drop_percent(du, Phases::Three) - 4.87).abs() < 0.05, "{du}");
        // Single phase doubles the length of conductor: 2 x 16 A x 20 m x 22.5 / 2.5 = 5.76 V = 2.5 %.
        let du = voltage_drop(Conductor::Copper, 2.5, 20.0, 16.0, Phases::Single, 1.0).unwrap();
        assert!((du - 5.76).abs() < 1e-9);
        assert_eq!(max_voltage_drop_percent(Usage::Lighting, false), 3.0);
        assert!(voltage_drop(Conductor::Copper, 0.0, 10.0, 1.0, Phases::Single, 1.0).is_err());
    }

    #[test]
    fn current_rating_and_lighting() {
        assert!((design_current(3680.0, Phases::Single, 1.0).unwrap() - 16.0).abs() < 1e-9);
        assert_eq!(protection_rating(17.0).unwrap(), 20.0);
        assert!(protection_rating(1.0e6).is_err());
        // 10 luminaires of 3400 lm in 60 m2 at UF 0.5, MF 0.8: 226 lux.
        assert!((illuminance_lux(34000.0, 60.0, 0.5, 0.8).unwrap() - 226.67).abs() < 0.01);
        assert_eq!(minimum_illuminance(RoomLighting::WorkRoom), 120.0);
        assert_eq!(k3(Insulation::Pvc, 38.0), Some(0.87), "the nearest hotter column");
        assert_eq!(k3(Insulation::Pvc, 70.0), None);
    }
}
