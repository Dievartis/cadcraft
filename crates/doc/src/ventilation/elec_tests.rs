use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, Space};

fn cable(d: &mut Drawing, circuit: &str, a: (f64, f64), b: (f64, f64)) -> crate::Handle {
    let p = CableParams::new(circuit, CableUsage::Power, HvacUnits { mm_per_unit: 1.0 });
    d.add_cable(&Space::Model, Common::default(), p, Vec2::new(a.0, a.1), Vec2::new(b.0, b.1)).unwrap()
}

fn device(d: &mut Drawing, kind: DeviceKind, circuit: &str, at: (f64, f64)) -> crate::Handle {
    let mut p = DeviceParams::new(kind, "", HvacUnits { mm_per_unit: 1.0 });
    p.circuit = circuit.into();
    d.add_device(&Space::Model, Common::default(), &p, Vec2::new(at.0, at.1), 0.0).unwrap()
}

#[test]
fn a_circuit_is_read_from_its_name_and_sized_by_its_current_and_its_voltage_drop() {
    let mut d = Drawing::new_metric();
    for i in 0..10 {
        device(&mut d, DeviceKind::Socket, "TD1-C1", (i as f64 * 1000.0, 0.0));
    }
    let panel = DeviceParams { name: "TD1".into(), ..DeviceParams::new(DeviceKind::Panel, "TD1", HvacUnits::default()) };
    d.add_device(&Space::Model, Common::default(), &panel, Vec2::new(-1000.0, 0.0), 0.0).unwrap();
    let c1 = cable(&mut d, "TD1-C1", (0.0, 0.0), (15000.0, 0.0));
    cable(&mut d, "TD1-C1", (15000.0, 0.0), (30000.0, 0.0));
    let r = d.circuit_reports(&CircuitOptions::default());
    assert_eq!(r.len(), 1);
    let c = &r[0];
    // 10 sockets of 500 W: 5000 W / (230 V x 0.9) = 24.2 A, a 25 A breaker; 4 mm² in conduit (24 A is not enough at 2.5 mm²), 30 m long.
    assert_eq!((c.devices, c.power_w, c.cable_length_m, c.panel.as_str(), c.panel_drawn), (10, 5000.0, 30.0, "TD1", true));
    assert_eq!((c.rating_a, c.thermal_section_mm2, c.required_section_mm2), (Some(25.0), Some(4.0), Some(4.0)));
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    // A section of 1.5 mm² drawn is too small, in both ways.
    d.edit_circuit("TD1-C1", &CircuitChange { section_mm2: Some(Some(1.5)), ..CircuitChange::default() }).unwrap();
    let c = &d.circuit_reports(&CircuitOptions::default())[0];
    assert!(c.warnings.iter().any(|w| w.contains("below the one the current needs")), "{:?}", c.warnings);
    assert!(c.warnings.iter().any(|w| w.contains("voltage drop")), "{:?}", c.warnings);
    // Sizing writes the section and the rating on the cables, and the circuit is then right.
    let after = d.size_circuits(&CircuitOptions::default(), None, true).unwrap();
    assert_eq!((after[0].section_mm2, after[0].protection_a), (Some(4.0), Some(25.0)));
    assert!(after[0].ok(), "{:?}", after[0].warnings);
    assert_eq!(cable_of(d.entity(c1).unwrap()).unwrap().size_text(), "3G4");
}

#[test]
fn a_long_lighting_circuit_needs_a_section_for_its_voltage_drop_and_the_style_follows_a_rename() {
    let mut d = Drawing::new_metric();
    for i in 0..8 {
        device(&mut d, DeviceKind::Light, "TD2-E1", (i as f64 * 10_000.0, 0.0));
    }
    cable(&mut d, "TD2-E1", (0.0, 0.0), (80_000.0, 0.0));
    let c = &d.circuit_reports(&CircuitOptions::default())[0];
    // 8 x 40 W = 320 W is 1.5 A: the current needs 1.5 mm², but 80 m at 3 % asks more.
    assert_eq!(c.usage, "power", "the cable says what the circuit is for");
    assert!(c.drop_section_mm2.is_some() && c.thermal_section_mm2 == Some(1.5), "{c:?}");
    // Renaming moves the cables and the devices, with the style.
    let colour = d.network_style("TD2-E1").unwrap().color;
    assert_eq!(d.rename_circuit("td2-e1", "TD2-E9").unwrap(), 9);
    assert_eq!(d.circuit_reports(&CircuitOptions::default()).len(), 1);
    assert_eq!(d.network_style("TD2-E9").unwrap().color, colour);
    assert!(d.rename_circuit("TD2-E9", "\u{7}").is_err());
}

#[test]
fn equipment_on_a_circuit_adds_its_power_and_hostile_values_are_refused() {
    let mut d = Drawing::new_metric();
    let m = standard_equipment_models().into_iter().next().unwrap();
    let mut q = EquipmentParams::from_model(&m, "GE1", HvacUnits::default());
    q.electric_kw = Some(11.0);
    q.circuit = "TD1-V1".into();
    d.add_equipment(&Space::Model, Common::default(), &q, Vec2::ZERO, 0.0).unwrap();
    cable(&mut d, "TD1-V1", (0.0, 0.0), (20_000.0, 0.0));
    d.edit_circuit("TD1-V1", &CircuitChange { phases: Some(ElecPhases::Three), cos_phi: Some(0.85), ..CircuitChange::default() }).unwrap();
    let c = &d.circuit_reports(&CircuitOptions::default())[0];
    assert_eq!((c.hvac, c.power_w, c.phases.as_str()), (1, 11_000.0, "three"));
    assert!(c.current_a > 18.0 && c.current_a < 19.5, "{}", c.current_a);
    assert!(DeviceParams { power_w: f64::NAN, ..DeviceParams::new(DeviceKind::Socket, "P1", HvacUnits::default()) }.validate().is_err());
    assert!(CableParams { cos_phi: 0.0, ..CableParams::new("C", CableUsage::Power, HvacUnits::default()) }.validate().is_err());
    assert!(CableParams { section_mm2: Some(f64::INFINITY), ..CableParams::new("C", CableUsage::Power, HvacUnits::default()) }.validate().is_err());
}
