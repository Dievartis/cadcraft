//! Refrigerant link sizes of a variable-refrigerant-flow system (R410A), read from the manufacturer's public
//! installation guides and kept here as plain tables so the drawing's checks and the design charts show the same
//! numbers. Nothing is named by brand or model: a system is a heat pump (two pipes: liquid and gas) or a heat recovery
//! system (three pipes: liquid, suction gas and high/low-pressure gas).
//!
//! Units: pipe outer diameters in mm; capacity of the outdoor unit in horsepower classes (HP); capacity index of
//! indoor units (about ten times the cooling power in kW).

use crate::{HvacError, Result, checked};

/// The two kinds of system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VrvSystem {
    /// Two pipes: liquid and gas.
    HeatPump,
    /// Three pipes: liquid, suction gas, high/low-pressure gas.
    HeatRecovery,
}

/// The outer diameters, mm, of the pipes of one link. `high_gas` is 0 for a heat pump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkSize {
    pub liquid: f64,
    pub gas: f64,
    pub high_gas: f64,
}

/// Outdoor unit total capacity (upper bound of the class, HP), then liquid, gas: heat pump, outdoor unit to first branch.
pub const HP_MAIN: [(f64, f64, f64); 7] =
    [(8.0, 9.5, 19.1), (10.0, 9.5, 22.2), (16.0, 12.7, 28.6), (22.0, 15.9, 28.6), (24.0, 15.9, 34.9), (34.0, 19.1, 34.9), (54.0, 19.1, 41.3)];
/// Outdoor unit capacity (HP, upper bound), liquid, suction gas, high/low-pressure gas: heat recovery, outdoor to first branch.
pub const HR_MAIN: [(f64, f64, f64, f64); 9] = [
    (8.0, 9.5, 19.1, 15.9),
    (10.0, 9.5, 22.2, 19.1),
    (12.0, 12.7, 28.6, 19.1),
    (16.0, 12.7, 28.6, 22.2),
    (18.0, 15.9, 28.6, 22.2),
    (22.0, 15.9, 28.6, 28.6),
    (24.0, 15.9, 34.9, 28.6),
    (34.0, 19.1, 34.9, 28.6),
    (54.0, 19.1, 41.3, 34.9),
];
/// Downstream capacity index (exclusive upper bound), liquid, gas: heat pump, between branch kits.
pub const HP_BRANCH: [(f64, f64, f64); 7] = [
    (150.0, 9.5, 15.9),
    (200.0, 9.5, 19.1),
    (290.0, 9.5, 22.2),
    (420.0, 12.7, 28.6),
    (640.0, 15.9, 28.6),
    (920.0, 19.1, 34.9),
    (f64::INFINITY, 19.1, 41.3),
];
/// Same, heat recovery: liquid, suction gas, high/low-pressure gas.
pub const HR_BRANCH: [(f64, f64, f64, f64); 7] = [
    (150.0, 9.5, 15.9, 12.7),
    (200.0, 9.5, 19.1, 15.9),
    (290.0, 9.5, 22.2, 19.1),
    (420.0, 12.7, 28.6, 19.1),
    (640.0, 15.9, 28.6, 28.6),
    (920.0, 19.1, 34.9, 28.6),
    (f64::INFINITY, 19.1, 41.3, 28.6),
];
/// Capacity index of the indoor unit (upper bound), liquid, gas: the link into an indoor unit (heat pump).
pub const UNIT_LINK: [(f64, f64, f64); 4] = [(50.0, 6.4, 12.7), (140.0, 9.5, 15.9), (200.0, 9.5, 19.1), (250.0, 9.5, 22.2)];
/// Total index behind a branch-selector box (upper bound), liquid, gas.
pub const BOX_LINK: [(f64, f64, f64); 3] = [(62.0, 6.4, 12.7), (149.0, 9.5, 15.9), (208.0, 9.5, 19.1)];
/// Pipe diameter (upper bound, mm), then the minimal wall thickness, mm.
pub const MIN_WALL: [(f64, f64); 6] = [(12.7, 0.80), (15.9, 0.99), (22.2, 0.80), (28.6, 0.99), (34.9, 1.21), (41.3, 1.43)];

fn bad(what: &str) -> HvacError {
    HvacError::BadValue(what.into())
}

/// The link between the outdoor unit (of `hp` horsepower in all) and the first branch, A, B or C.
pub fn main_link(system: VrvSystem, hp: f64) -> Result<LinkSize> {
    let hp = checked("outdoor capacity (HP)", hp, 1.0, 54.0)?;
    match system {
        VrvSystem::HeatPump => HP_MAIN.iter().find(|r| hp <= r.0).map(|r| LinkSize { liquid: r.1, gas: r.2, high_gas: 0.0 }),
        VrvSystem::HeatRecovery => HR_MAIN.iter().find(|r| hp <= r.0).map(|r| LinkSize { liquid: r.1, gas: r.2, high_gas: r.3 }),
    }
    .ok_or_else(|| bad("no size for this outdoor capacity"))
}

/// The link between two branch kits (D) that serves units of `index` in all, never larger than `main`.
pub fn branch_link(system: VrvSystem, index: f64, main: LinkSize) -> Result<LinkSize> {
    let index = checked("capacity index", index, 0.0, 1e5)?;
    let l = match system {
        VrvSystem::HeatPump => HP_BRANCH.iter().find(|r| index < r.0).map(|r| LinkSize { liquid: r.1, gas: r.2, high_gas: 0.0 }),
        VrvSystem::HeatRecovery => HR_BRANCH.iter().find(|r| index < r.0).map(|r| LinkSize { liquid: r.1, gas: r.2, high_gas: r.3 }),
    }
    .ok_or_else(|| bad("no size for this capacity index"))?;
    Ok(LinkSize { liquid: l.liquid.min(main.liquid), gas: l.gas.min(main.gas), high_gas: l.high_gas.min(main.high_gas) })
}

/// The link into a single indoor unit (E) of capacity index `index`.
pub fn unit_link(index: f64) -> Result<LinkSize> {
    let index = checked("capacity index", index, 0.0, 250.0)?;
    UNIT_LINK.iter().find(|r| index <= r.0).map(|r| LinkSize { liquid: r.1, gas: r.2, high_gas: 0.0 }).ok_or_else(|| bad("no size for this unit"))
}

/// The link from a branch kit to a branch-selector box (F) serving units of `index` in all.
pub fn box_link(index: f64) -> Result<LinkSize> {
    let index = checked("capacity index", index, 0.0, 208.0)?;
    BOX_LINK.iter().find(|r| index <= r.0).map(|r| LinkSize { liquid: r.1, gas: r.2, high_gas: 0.0 }).ok_or_else(|| bad("no size for this box"))
}

/// The liquid pipe one size up, mm, when the equivalent length of the longest run is 90 m or more: the size-up rule.
pub fn liquid_size_up(liquid: f64) -> f64 {
    match liquid {
        l if l <= 9.5 => 12.7,
        l if l <= 12.7 => 15.9,
        l if l <= 15.9 => 19.1,
        l if l <= 19.1 => 22.2,
        l => l,
    }
}

/// The minimal wall thickness, mm, of a refrigerant pipe of outer diameter `od` (mm).
pub fn min_wall_mm(od: f64) -> Option<f64> {
    MIN_WALL.iter().find(|r| od <= r.0 + 1e-9).map(|r| r.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PipeCriteria, WaterTemperature, drain_capacity_lph, max_flow_m3h, size_condensate_drain, size_water_pipe};

    #[test]
    fn main_link_follows_the_guide() {
        let hp = main_link(VrvSystem::HeatPump, 20.0).unwrap();
        assert_eq!((hp.liquid, hp.gas), (15.9, 28.6));
        let hr = main_link(VrvSystem::HeatRecovery, 20.0).unwrap();
        assert_eq!((hr.liquid, hr.gas, hr.high_gas), (15.9, 28.6, 28.6));
        assert!(main_link(VrvSystem::HeatPump, 60.0).is_err());
        assert!(main_link(VrvSystem::HeatPump, f64::NAN).is_err());
    }

    #[test]
    fn branches_never_exceed_the_main_link() {
        let main = main_link(VrvSystem::HeatPump, 8.0).unwrap();
        assert_eq!(branch_link(VrvSystem::HeatPump, 1000.0, main).unwrap().gas, 19.1);
        assert_eq!(branch_link(VrvSystem::HeatPump, 100.0, main).unwrap().gas, 15.9);
        assert_eq!(unit_link(71.0).unwrap().gas, 15.9);
        assert_eq!(box_link(100.0).unwrap().liquid, 9.5);
        assert_eq!(liquid_size_up(9.5), 12.7);
        assert_eq!(min_wall_mm(28.6), Some(0.99));
    }

    #[test]
    fn charts_agree_with_the_sizing() {
        let c = PipeCriteria::default();
        let q = max_flow_m3h(50.0, WaterTemperature::Cold, &c).unwrap();
        assert_eq!(size_water_pipe(q * 0.99, WaterTemperature::Cold, &c).unwrap().dn_mm, 50.0);
        assert!(size_water_pipe(q * 1.05, WaterTemperature::Cold, &c).unwrap().dn_mm > 50.0);
        let cap = drain_capacity_lph(32.0, 1.0).unwrap();
        assert_eq!(size_condensate_drain(cap * 0.99, 1.0).unwrap().dn_mm, 32.0);
        assert!(max_flow_m3h(51.0, WaterTemperature::Cold, &c).is_err());
    }
}
