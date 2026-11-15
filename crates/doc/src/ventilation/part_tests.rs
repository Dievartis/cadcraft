use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

#[test]
fn every_kind_has_a_symbol_and_every_model_of_the_library_can_be_drawn() {
    // A symbol per kind, with the tag written under it.
    for kind in ALL_KINDS {
        let p = PartParams::new(kind, "X1", units());
        let pieces = p.generate().unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        assert!(pieces.len() >= 2, "{kind:?} draws a symbol and a label");
        assert!(pieces.iter().all(|x| x.layer == kind.layer()));
        assert_eq!(PartKind::parse(kind.name()), Some(kind));
    }
    let models = standard_part_models();
    assert!(models.len() >= 80, "{} models", models.len());
    let mut names: Vec<&str> = models.iter().map(|m| m.name).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "model names are unique");
    for m in &models {
        m.params("T1", units()).generate().unwrap_or_else(|e| panic!("{}: {e}", m.name));
    }
    // Every category has something in it.
    for c in PartCategory::ALL {
        assert!(models.iter().any(|m| m.category == c), "{c:?}");
    }
}

#[test]
fn the_refrigerant_parts_carry_their_outlets_and_connections_and_name_no_maker() {
    let get = |n: &str| standard_part_model_named(n).unwrap_or_else(|| panic!("{n}"));
    let bs = get("Branch box, 6 outlets (R32 heat recovery)");
    assert_eq!((bs.outlets, bs.kind, bs.brand), (6, PartKind::BranchBox, ""));
    assert!(
        bs.connections.contains("liquid 5/8\"") && bs.connections.contains("HP/LP gas 7/8\"") && bs.connections.contains("suction gas 7/8\""),
        "{}",
        bs.connections
    );
    assert_eq!(get("Branch box, 12 outlets (R32 heat recovery)").outlets, 12);
    assert_eq!(get("Branch box, 16 outlets (R410A heat recovery)").outlets, 16);
    assert_eq!(get("Refrigerant header, index 290 to 640").outlets, 8);
    assert_eq!(get("Refrigerant Y joint, index below 200").kind, PartKind::RefnetJoint);
    assert_eq!(get("Boîte de détente 4 sorties (R32, récupération d'énergie)").outlets, 4, "found by its French name");
    assert_eq!(get("Connection box, 2 outlets (multi)").outlets, 2);
    assert_eq!(get("Vanne papillon").kind, PartKind::ButterflyValve);
    assert!(standard_part_model_named("nothing").is_none());
    // No maker or reference anywhere in what the library shows or a drawing keeps.
    for m in standard_part_models() {
        let text = format!("{} {} {} {}", m.name, m.name_fr, m.brand, m.connections).to_lowercase();
        for word in ["daikin", "vrv", "khr", "bs4", "bs6", "bs8", "bs10", "bs12", "bs16", "bpmks", "refnet"] {
            assert!(!text.contains(word), "{}: {word}", m.name);
        }
    }
}

#[test]
fn the_safety_devices_are_red_and_every_part_has_a_height() {
    for kind in ALL_KINDS {
        assert_eq!(kind.layer() == PART_SAFETY_LAYER, kind.is_safety(), "{kind:?}");
        assert!(PartParams::new(kind, "X", units()).height_mm.is_some_and(|h| h > 0.0), "{kind:?}");
    }
    assert!(PartKind::SafetyValve.is_safety() && PartKind::Disconnector.is_safety() && !PartKind::CheckValve.is_safety());
    assert_eq!(part_layer_color(PART_SAFETY_LAYER), Some(cadcraft_color::Color::True(cadcraft_color::Rgb(0xe0, 0x24, 0x24))));
    assert!(standard_part_models().iter().all(|m| m.height_mm > 0.0));
    assert!(standard_part_model_named("Buffer tank 500 l").is_some_and(|m| m.height_mm == 1600.0));
    // A header or a box is as long as its outlets need.
    assert!(PartKind::BranchBox.length_for_outlets_mm(8) > PartKind::BranchBox.length_for_outlets_mm(4));
    assert_eq!(PartKind::Pump.length_for_outlets_mm(4), None);
}

#[test]
fn a_part_is_added_turned_and_keeps_its_data() {
    let mut d = Drawing::new_metric();
    let m = standard_part_model_named("Circulator (single)").unwrap();
    let mut p = m.params("P1", units());
    p.flow_m3h = Some(12.0);
    let h = d.add_part(&Space::Model, Common::default(), &p, Vec2::new(1000.0, 500.0), 0.5).unwrap();
    assert_eq!(d.entity(h).unwrap().common.layer, PART_EQUIPMENT_LAYER);
    let back = part_of(d.entity(h).unwrap()).unwrap();
    assert_eq!((back.kind, back.model.as_str(), back.dn_mm, back.flow_m3h), (PartKind::Pump, "Circulator (single)", Some(50.0), Some(12.0)));
    let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { panic!("a block reference") };
    assert_eq!((i.insert.x, i.insert.y, i.rotation), (1000.0, 500.0, 0.5));
    assert_eq!(p.label_text(), "P1 DN50");
    // Changed into a valve kind: another layer, redrawn.
    d.regenerate_part(h, &PartParams { kind: PartKind::CheckValve, ..back }).unwrap();
    assert_eq!(d.entity(h).unwrap().common.layer, PART_LAYER);
    // What cannot be drawn is refused.
    for bad in [
        PartParams { length_mm: 0.0, ..p.clone() },
        PartParams { width_mm: f64::NAN, ..p.clone() },
        PartParams { outlets: 1000, ..p.clone() },
        PartParams { name: String::new(), ..p.clone() },
        PartParams { dn_mm: Some(-1.0), ..p.clone() },
        PartParams { model: "x".repeat(500), ..p.clone() },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    assert!(d.add_part(&Space::Model, Common::default(), &p, Vec2::new(f64::NAN, 0.0), 0.0).is_err());
}

#[test]
fn a_header_and_a_box_draw_one_stub_per_outlet() {
    let lines = |p: &PartParams| p.generate().unwrap().iter().filter(|x| matches!(x.kind, EntityKind::Line(_))).count();
    let few = PartParams { outlets: 4, ..PartParams::new(PartKind::RefnetHeader, "H1", units()) };
    let many = PartParams { outlets: 8, ..few.clone() };
    assert_eq!(lines(&many) - lines(&few), 4);
    let b = PartParams { outlets: 4, ..PartParams::new(PartKind::BranchBox, "B1", units()) };
    let b2 = PartParams { outlets: 6, ..b.clone() };
    assert_eq!(lines(&b2) - lines(&b), 4, "a pair of lines for each port");
    assert_eq!(b.label_text(), "B1 4x");
}
