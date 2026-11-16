use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn texts(pieces: &[HvacPiece]) -> Vec<String> {
    pieces
        .iter()
        .filter_map(|p| match &p.kind {
            EntityKind::Text(t) => Some(t.value.clone()),
            _ => None,
        })
        .collect()
}

/// A board TD1 of 63 A with two circuits of sockets (one 20 m long, one 30 m), and a circuit whose board is not drawn.
fn installation() -> Drawing {
    let mut d = Drawing::new_metric();
    let mut board = DeviceParams::new(DeviceKind::Panel, "TD1", units());
    board.supply_a = Some(63.0);
    d.add_device(&Space::Model, Common::default(), &board, Vec2::new(-1000.0, 0.0), 0.0).unwrap();
    for (circuit, len, y) in [("TD1-C1", 20_000.0, 0.0), ("TD1-C2", 30_000.0, 2000.0), ("TD9-C1", 10_000.0, 4000.0)] {
        let mut s = DeviceParams::new(DeviceKind::Socket, "", units());
        s.circuit = circuit.into();
        s.power_w = 2000.0;
        d.add_device(&Space::Model, Common::default(), &s, Vec2::new(0.0, y + 500.0), 0.0).unwrap();
        let mut c = CableParams::new(circuit, CableUsage::Power, units());
        c.section_mm2 = Some(2.5);
        c.protection_a = Some(16.0);
        d.add_cable(&Space::Model, Common::default(), c, Vec2::new(0.0, y), Vec2::new(len, y)).unwrap();
    }
    d
}

#[test]
fn the_diagram_has_a_busbar_for_each_board_and_a_branch_for_each_circuit() {
    let d = installation();
    let pieces = d.generate_single_line(&SingleLineParams::new(units())).unwrap();
    let t = texts(&pieces);
    // The title with the origin's current, the board with its main breaker, the circuits with their breaker, their cable and their load.
    assert!(t.iter().any(|x| x.contains("Single-line diagram") && x.contains("10.0 kA")), "{t:?}");
    assert!(t.contains(&"TD1".to_string()) && t.contains(&"63 A".to_string()), "{t:?}");
    assert!(t.contains(&"TD1-C1".to_string()) && t.contains(&"TD1-C2".to_string()), "{t:?}");
    assert!(t.iter().any(|x| x == "16 A C"), "the breaker with its curve: {t:?}");
    assert!(t.iter().any(|x| x == "3G2.5  20 m") && t.iter().any(|x| x == "3G2.5  30 m"), "{t:?}");
    assert!(t.iter().any(|x| x.contains("1 dev.") && x.contains("2.0 kW")), "{t:?}");
    // The circuit whose board is not drawn is drawn too, under a board named "-".
    assert!(t.iter().any(|x| x.starts_with("TD9-C1")) && t.contains(&"-".to_string()), "a circuit with no board drawn is marked: {t:?}");
    // One board only, by its name.
    let only = d.generate_single_line(&SingleLineParams { board: "td1".into(), ..SingleLineParams::new(units()) }).unwrap();
    assert!(!texts(&only).iter().any(|x| x.starts_with("TD9-C1")) && texts(&only).contains(&"TD1-C2".to_string()));
    // A drawing with no electricity has an empty diagram, not an error.
    assert!(Drawing::new_metric().generate_single_line(&SingleLineParams::new(units())).is_ok());
    let bad = SingleLineParams { label_height_mm: 0.0, ..SingleLineParams::new(units()) };
    assert!(bad.validate().is_err() && installation().generate_single_line(&bad).is_err());
    assert!(SingleLineParams { board: "x".repeat(100), ..SingleLineParams::new(units()) }.validate().is_err());
}

#[test]
fn a_diagram_is_redrawn_with_the_circuits_it_reads_and_keeps_its_settings() {
    let mut d = installation();
    let p = SingleLineParams { options: CircuitOptions { source_ka: 25.0, ..CircuitOptions::default() }, ..SingleLineParams::new(units()) };
    let h = d.add_single_line(&Space::Model, Common::default(), &p, Vec2::new(0.0, -20_000.0)).unwrap();
    assert_eq!(single_line_of(d.entity(h).unwrap()).map(|x| x.options.source_ka), Some(25.0));
    let before: Vec<String> = match &d.entity(h).unwrap().kind {
        EntityKind::Insert(i) => d
            .block(&i.block)
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect(),
        _ => panic!(),
    };
    assert!(before.contains(&"16 A C".to_string()));
    // The breaker of a circuit changes: the next refresh shows it.
    d.edit_circuit("TD1-C1", &CircuitChange { protection_a: Some(Some(20.0)), ..CircuitChange::default() }).unwrap();
    d.refresh_circuit_legends();
    let after: Vec<String> = match &d.entity(h).unwrap().kind {
        EntityKind::Insert(i) => d
            .block(&i.block)
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect(),
        _ => panic!(),
    };
    assert!(after.contains(&"20 A C".to_string()), "{after:?}");
    assert_eq!(single_line_of(d.entity(h).unwrap()).map(|x| x.options.source_ka), Some(25.0), "the settings are kept");
}
