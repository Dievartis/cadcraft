use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn equipment(d: &mut Drawing, kind: EquipmentKind, kw: f64, at: (f64, f64)) -> crate::Handle {
    let m = EquipmentModel {
        name: "x".into(),
        kind,
        reference: String::new(),
        capacity_kw: Some(kw),
        heating_kw: None,
        length_mm: 1000.0,
        width_mm: 600.0,
        height_mm: 500.0,
        weight_kg: None,
        fans: 0,
        connections: String::new(),
    };
    d.add_equipment(&Space::Model, Common::default(), &EquipmentParams::from_model(&m, "E", units()), Vec2::new(at.0, at.1), 0.0).unwrap()
}

fn link(d: &mut Drawing, a: Vec2, b: Vec2) {
    let p = PipeParams::new(PipeService::RefrigerantPair, 9.52, units());
    d.add_pipe(&Space::Model, Common::default(), p, a, b).unwrap();
}

fn part(d: &mut Drawing, model: &str, at: (f64, f64)) -> (crate::Handle, PartParams) {
    let p = d.part_entry_named(model).unwrap().params("P", units());
    (d.add_part(&Space::Model, Common::default(), &p, Vec2::new(at.0, at.1), 0.0).unwrap(), p)
}

/// The world position of a port of a part put at `at`, not turned.
fn port(p: &PartParams, at: (f64, f64), outlet: Option<usize>, nth: usize) -> Vec2 {
    let q: Vec<_> = p.ports().into_iter().filter(|x| x.outlet == outlet).collect();
    Vec2::new(at.0 + q[nth].at.x, at.1 + q[nth].at.y)
}

#[test]
fn a_refrigerant_network_is_followed_from_the_outdoor_unit_to_the_indoor_units() {
    let mut d = Drawing::new_metric();
    // An outdoor unit, a pair to a branch box of four outlets, and a unit of 2.5 kW on each of three outlets.
    equipment(&mut d, EquipmentKind::VrfOutdoor, 10.0, (0.0, 0.0));
    let (_, bx) = part(&mut d, "Branch box, 4 outlets (R32 heat recovery)", (5000.0, 0.0));
    let inlet = port(&bx, (5000.0, 0.0), None, 1);
    link(&mut d, Vec2::new(500.0, 0.0), inlet);
    let outs: Vec<Vec2> = (0..4).map(|i| port(&bx, (5000.0, 0.0), Some(i), 0)).collect();
    for (i, o) in outs.iter().take(3).enumerate() {
        let target = Vec2::new(o.x, 3000.0 + 1000.0 * i as f64);
        link(&mut d, *o, target);
        equipment(&mut d, EquipmentKind::Cassette, 2.5, (target.x, target.y + 300.0));
    }
    let r = d.check_refrigerant().unwrap();
    assert_eq!(r.networks.len(), 1);
    let n = &r.networks[0];
    assert_eq!((n.units, n.index), (3, 75.0), "{:?}", n.warnings);
    assert!(r.unconnected.is_empty());
    let b = n.parts.iter().find(|x| x.kind == PartKind::BranchBox).expect("the box is on the way");
    assert_eq!((b.units, b.index, b.outlets.len()), (3, 75.0, 3));
    assert!(b.outlets.iter().all(|o| o.units == 1 && o.index == 25.0));
    assert!(n.warnings.is_empty(), "{:?}", n.warnings);
}

#[test]
fn what_a_box_outlet_and_a_joint_carry_is_checked() {
    let mut d = Drawing::new_metric();
    equipment(&mut d, EquipmentKind::VrfOutdoor, 10.0, (0.0, 0.0));
    let (_, joint) = part(&mut d, "Refrigerant Y joint, index below 200", (4000.0, 0.0));
    link(&mut d, Vec2::new(500.0, 0.0), port(&joint, (4000.0, 0.0), None, 0));
    // One branch of the joint ends on a unit of 2.5 kW (index 25), the other on a big unit of 30 kW (index 300).
    let up = port(&joint, (4000.0, 0.0), Some(0), 0);
    link(&mut d, up, Vec2::new(up.x, 4000.0));
    equipment(&mut d, EquipmentKind::Cassette, 2.5, (up.x, 4300.0));
    let down = port(&joint, (4000.0, 0.0), Some(1), 0);
    link(&mut d, down, Vec2::new(down.x, -4000.0));
    equipment(&mut d, EquipmentKind::Ducted, 30.0, (down.x, -4300.0));
    let r = d.check_refrigerant().unwrap();
    let n = &r.networks[0];
    let j = n.parts.iter().find(|x| x.kind == PartKind::RefnetJoint).unwrap();
    assert_eq!(j.units, 2, "one unit at the end of each branch");
    assert_eq!(j.index, 25.0 + 300.0);
    // 325 is more than the joint was chosen for: the one that suits is told.
    assert_eq!(j.recommended.as_deref(), Some("Refrigerant Y joint, index 290 to 640"));
    assert!(n.warnings.iter().any(|w| w.contains("Y joint") && w.contains("290 to 640")), "{:?}", n.warnings);
}

#[test]
fn an_indoor_unit_reached_from_no_outdoor_unit_is_told_and_a_coil_counts_as_a_unit() {
    let mut d = Drawing::new_metric();
    equipment(&mut d, EquipmentKind::VrfOutdoor, 10.0, (0.0, 0.0));
    let lone = equipment(&mut d, EquipmentKind::WallUnit, 2.0, (9000.0, 9000.0));
    // An air handling unit with a direct-expansion coil on a pipe from the outdoor unit.
    let mut a = AhuParams {
        name: "CTA1".into(),
        kind: AhuKind::DoubleFlow,
        ports: vec![],
        width_mm: 2000.0,
        depth_mm: 1000.0,
        port_sizes: Vec::new(),
        height_mm: None,
        model: String::new(),
        weight_kg: None,
        flow_m3h: None,
        pressure_pa: None,
        extract_flow_m3h: None,
        extract_pressure_pa: None,
        fresh_flow_m3h: None,
        fresh_pressure_pa: None,
        exhaust_flow_m3h: None,
        exhaust_pressure_pa: None,
        coils: AhuCoils::default(),
        label_height_mm: 100.0,
        units: units(),
    };
    a.coils.direct_expansion_kw = Some(20.0);
    d.add_ahu(&Space::Model, Common::default(), &a, Vec2::new(5000.0, 0.0), 0.0).unwrap();
    link(&mut d, Vec2::new(500.0, 0.0), Vec2::new(4500.0, 0.0));
    let r = d.check_refrigerant().unwrap();
    assert_eq!((r.networks[0].units, r.networks[0].index), (1, 200.0), "the coil is a unit of index 200");
    assert_eq!(r.unconnected, vec![lone]);
    assert!(!r.warnings.is_empty());
    // Nothing to read in a drawing without any.
    let empty = Drawing::new_metric().check_refrigerant().unwrap();
    assert!(empty.networks.is_empty() && empty.unconnected.is_empty() && empty.warnings.is_empty());
}
