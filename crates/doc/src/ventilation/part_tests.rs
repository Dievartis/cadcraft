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
fn the_daikin_parts_carry_their_outlets_and_connections() {
    let get = |n: &str| standard_part_model_named(n).unwrap_or_else(|| panic!("{n}"));
    let bs = get("BS6A14AJV1B9");
    assert_eq!((bs.outlets, bs.brand, bs.kind), (6, "Daikin", PartKind::BranchBox));
    assert!(
        bs.connections.contains("Liq 5/8\"") && bs.connections.contains("HP/LP 7/8\"") && bs.connections.contains("Suc 7/8\""),
        "{}",
        bs.connections
    );
    assert_eq!(get("BS12A14AJV1B9").outlets, 12);
    assert_eq!(get("BS16Q14AV1B").outlets, 16);
    assert_eq!(get("Refnet header KHRQ22M64H").outlets, 8);
    assert_eq!(get("Refnet joint KHRQ23M20T").kind, PartKind::RefnetJoint);
    assert_eq!(get("Vanne papillon").kind, PartKind::ButterflyValve, "found by its French name");
    assert!(standard_part_model_named("nothing").is_none());
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
