use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits::default()
}

#[test]
fn the_standard_models_are_valid_unique_and_cover_every_kind() {
    let models = standard_equipment_models();
    let mut names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "model names are unique");
    for m in &models {
        let p = EquipmentParams::from_model(m, "X", units());
        p.validate().unwrap_or_else(|e| panic!("{}: {e}", m.name));
        assert!(p.generate().is_ok(), "{}", m.name);
    }
    for kind in EquipmentKind::ALL {
        assert!(!equipment_models_of(&models, kind).is_empty(), "{kind:?}");
    }
    // A range in order of power, and chillers from 40 to 570 kW.
    let chillers = equipment_models_of(&models, EquipmentKind::Chiller);
    let kw: Vec<f64> = chillers.iter().filter_map(|m| m.capacity_kw).collect();
    assert!(kw.windows(2).all(|w| w[0] <= w[1]));
    assert!(kw.first().is_some_and(|k| *k < 45.0) && kw.last() == Some(&570.0));
}

#[test]
fn nothing_of_a_maker_is_left_in_the_library_but_the_sizes() {
    for m in standard_equipment_models() {
        // The cabinets, dehumidifiers and racks say their flow or their units in the text beside them.
        let new = matches!(
            m.kind,
            EquipmentKind::ServerCooler
                | EquipmentKind::Dehumidifier
                | EquipmentKind::DehumidifierWall
                | EquipmentKind::WallRack
                | EquipmentKind::FloorRack
        );
        assert!(m.reference.is_empty() && (new || m.connections.is_empty()) && m.weight_kg.is_none(), "{}", m.name);
        for brand in ["Carrier", "Daikin", "Sky Air", "AquaSnap", "Victaulic", "30R", "FTX", "RXY", "VRV IV"] {
            assert!(!m.name.contains(brand), "{}", m.name);
        }
        // The name is the kind and the power: kW, or CV for a VRV outdoor unit.
        let unit = if m.kind == EquipmentKind::VrfOutdoor { "CV" } else { "kW" };
        assert!(new || m.name.ends_with(unit), "{}", m.name);
    }
    let vrv = equipment_model_named(&standard_equipment_models(), "VRV 14 CV").unwrap();
    assert_eq!(EquipmentParams::from_model(&vrv, "UE1", units()).text(), "UE1 14 CV");
    let chiller = equipment_model_named(&standard_equipment_models(), "Chiller 107 kW").unwrap();
    assert_eq!(EquipmentParams::from_model(&chiller, "GE1", units()).text(), "GE1 107 kW");
}

#[test]
fn the_carrier_sizes_follow_their_powers() {
    let models = standard_equipment_models();
    let named = |n: &str| equipment_model_named(&models, n).unwrap_or_else(|| panic!("{n}"));
    // The smallest chiller: one fan, 2109 x 1090 x 1330; at 93.4 kW two fans on the larger casing.
    let m = named("Chiller 41.7 kW");
    assert_eq!((m.length_mm, m.width_mm, m.height_mm, m.fans), (2109.0, 1090.0, 1330.0, 1));
    let m = named("Chiller 93.4 kW");
    assert_eq!((m.length_mm, m.width_mm, m.fans), (2275.0, 2125.0, 2));
    // 30RBM 160-520: three casings, 2253 wide.
    assert_eq!(named("Chiller 168 kW").length_mm, 2410.0);
    assert_eq!(named("Chiller 365 kW").length_mm, 3604.0);
    assert_eq!((named("Chiller 523 kW").length_mm, named("Chiller 523 kW").fans, named("Chiller 523 kW").width_mm), (4797.0, 8, 2253.0));
    // The power picks the size: the smallest that gives it, else the largest.
    assert_eq!(equipment_model_for(&models, EquipmentKind::Chiller, 100.0).map(|m| m.name), Some("Chiller 107 kW".to_string()));
    assert_eq!(equipment_model_for(&models, EquipmentKind::Chiller, 9999.0).and_then(|m| m.capacity_kw), Some(570.0));
}

#[test]
fn every_kind_is_drawn_from_above_with_its_symbol_its_clearance_and_its_text() {
    let models = standard_equipment_models();
    for kind in EquipmentKind::ALL {
        let m = equipment_models_of(&models, kind).into_iter().next().unwrap();
        let p = EquipmentParams::from_model(&m, "G1", units());
        let pieces = p.generate().unwrap();
        assert!(pieces.iter().any(|x| x.layer.split('|').next() == Some(EQUIPMENT_LAYER) && matches!(x.kind, EntityKind::LwPolyline(_))), "{kind:?}");
        let texts: Vec<_> = pieces.iter().filter_map(|x| if let EntityKind::Text(t) = &x.kind { Some(t.value.clone()) } else { None }).collect();
        assert!(texts.iter().any(|t| t.starts_with("G1")), "{kind:?}: {texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("Dim. ")), "{kind:?}: the sizes are written");
        assert!(!texts.iter().any(|t| t.contains("Poids")), "{kind:?}: no weight unless it is given");
        let zone = pieces.iter().filter(|x| x.layer == EQUIPMENT_ZONE_LAYER).count();
        assert_eq!(zone > 0, kind.is_outdoor(), "{kind:?}: only outdoor units show a service clearance by default");
    }
    // Fans: one circle each, in two rows over a wide unit.
    let m = equipment_model_named(&models, "Chiller 523 kW").unwrap();
    let circles = |p: &EquipmentParams| p.generate().unwrap().iter().filter(|x| matches!(x.kind, EntityKind::Circle(_))).count();
    let mut p = EquipmentParams::from_model(&m, "CH", units());
    assert_eq!(circles(&p), 8);
    p.fans = 0;
    assert_eq!(circles(&p), 0);
    p.clearance_mm = 0.0;
    assert!(p.generate().unwrap().iter().all(|x| x.layer != EQUIPMENT_ZONE_LAYER));
}

#[test]
fn an_equipment_keeps_its_data_and_each_value_can_be_changed() {
    let models = standard_equipment_models();
    let m = equipment_model_named(&models, "Chiller 93.4 kW").unwrap();
    let mut d = Drawing::new_metric();
    let p = EquipmentParams::from_model(&m, "CH1", units());
    let h = d.add_equipment(&Space::Model, Common::default(), &p, Vec2::new(1000.0, 2000.0), 0.0).unwrap();
    let got = equipment_of(d.entity(h).unwrap()).unwrap();
    assert_eq!((got.name.as_str(), got.model.as_str(), got.capacity_kw, got.length_mm), ("CH1", "Chiller 93.4 kW", Some(93.4), 2275.0));
    // Everything is the user's: name, power, sizes, weight, fans, clearance, connections, reference.
    let mut next = got.clone();
    next.name = "CH-TOIT".into();
    next.capacity_kw = Some(100.0);
    (next.length_mm, next.width_mm, next.height_mm) = (3000.0, 2200.0, 1500.0);
    next.weight_kg = Some(900.0);
    next.fans = 3;
    next.clearance_mm = 1500.0;
    next.connections = "Victaulic 3\"".into();
    next.reference = "Mon modèle".into();
    d.regenerate_equipment(h, &next).unwrap();
    assert_eq!(equipment_of(d.entity(h).unwrap()).unwrap(), next);
    // Refused values.
    let bad = |f: &dyn Fn(&mut EquipmentParams)| {
        let mut q = got.clone();
        f(&mut q);
        q.validate().is_err()
    };
    assert!(bad(&|q| q.length_mm = 0.0));
    assert!(bad(&|q| q.capacity_kw = Some(-1.0)));
    assert!(bad(&|q| q.fans = 99));
    assert!(bad(&|q| q.clearance_mm = f64::NAN));
    assert!(bad(&|q| q.name = String::new()));
}

fn ducted(d: &mut Drawing) -> (crate::Handle, EquipmentParams) {
    let model = standard_equipment_models().into_iter().find(|m| m.kind == EquipmentKind::Ducted).unwrap();
    let p = EquipmentParams::from_model(&model, "UI1", HvacUnits { mm_per_unit: 1.0 });
    (d.add_equipment(&Space::Model, Common::default(), &p, Vec2::ZERO, 0.0).unwrap(), p)
}

#[test]
fn a_ducted_unit_has_the_connections_it_was_given_on_the_right_sides_at_the_right_size() {
    let (mut p, collars) = (ducted(&mut Drawing::new_metric()).1, |p: &EquipmentParams| {
        p.generate().unwrap().iter().filter(|x| matches!(&x.kind, EntityKind::LwPolyline(l) if l.closed && l.vertices.len() == 4)).count()
    });
    // None given: the casing and one flange on each long side.
    assert_eq!((p.air_ports.len(), collars(&p)), (0, 3));
    // Two returns (Ø250 and 400x250) on the back side, three Ø160 supplies on the front side.
    let round = |d: f64| RiserSize::Round { diameter_mm: d };
    p.set_air_ports(&[round(250.0), RiserSize::Rectangular { width_mm: 400.0, height_mm: 250.0 }], &[round(160.0); 3]);
    assert_eq!((p.air_sizes(true).len(), p.air_sizes(false).len(), collars(&p)), (2, 3, 1 + 5));
    // They are spread along the length of each side, the returns with their tips at the back (+y) and the supplies at the front (-y).
    let tips = p.air_port_tips();
    assert!(tips.iter().filter(|t| t.1).all(|t| t.0.y > 0.0) && tips.iter().filter(|t| !t.1).all(|t| t.0.y < 0.0));
    let xs: Vec<f64> = tips.iter().filter(|t| !t.1).map(|t| t.0.x).collect();
    assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
    assert!(AirPort { inlet: true, size: round(5.0), along_mm: 0.0 }.size.validate().is_err());
    p.air_ports = vec![AirPort { inlet: true, size: round(200.0), along_mm: 0.0 }; MAX_AIR_PORTS + 1];
    assert!(p.validate().is_err(), "too many");
}

/// The widths of the closed 4-point polylines drawn out of the front side (the supply connections) of `p`, mm, left to right.
fn front_collar_widths(p: &EquipmentParams) -> Vec<f64> {
    let hw = p.width_mm / 2.0;
    let mut w: Vec<(f64, f64)> = p
        .generate()
        .unwrap()
        .iter()
        .filter_map(|x| match &x.kind {
            EntityKind::LwPolyline(l) if l.closed && l.vertices.len() == 4 && l.vertices.iter().all(|v| v.p.y < -hw + 1e-6) => {
                let xs: Vec<f64> = l.vertices.iter().map(|v| v.p.x).collect();
                Some((xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max)))
            }
            _ => None,
        })
        .collect();
    w.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(w.windows(2).all(|p| p[0].1 <= p[1].0 + 1e-6), "the connections touch: {w:?}");
    w.iter().map(|(a, b)| b - a).collect()
}

#[test]
fn a_ducted_unit_takes_eight_d200_supplies_and_four_d250_returns_along_its_long_sides_without_the_drawings_touching() {
    let models = standard_equipment_models();
    let m = models.iter().find(|m| m.kind == EquipmentKind::Ducted && m.length_mm == 1550.0).expect("the 16 kW unit of 1550 mm");
    let mut p = EquipmentParams::from_model(m, "UI1", HvacUnits { mm_per_unit: 1.0 });
    let round = |d: f64| RiserSize::Round { diameter_mm: d };
    p.set_air_ports(&[round(250.0); 4], &[round(200.0); 8]);
    assert!(p.validate().is_ok());
    let widths = front_collar_widths(&p);
    assert_eq!(widths.len(), 8);
    // 1550 / 8 = 194 mm a connection: drawn a little narrower than 200, not over its neighbour.
    assert!(widths.iter().all(|w| *w > 180.0 && *w <= 200.0 + 1e-6), "{widths:?}");
    // Four Ø250 returns have room at their full width.
    let back = p.air_ports.iter().filter(|a| a.inlet).count();
    assert_eq!(back, 4);
    // A connection given in an old drawing under its old name keeps its place.
    let old: AirPort = serde_json::from_str(r#"{"inlet":true,"size":{"shape":"round","diameterMm":200.0},"yMm":120.0}"#).unwrap();
    assert_eq!(old.along_mm, 120.0);
}

#[test]
fn server_coolers_dehumidifiers_and_racks_are_drawn_each_in_its_way_and_the_ducted_ones_take_ducts() {
    let models = standard_equipment_models();
    let p = |name: &str| EquipmentParams::from_model(&equipment_model_named(&models, name).unwrap(), "X1", units());
    let count = |p: &EquipmentParams| p.generate().unwrap().len();
    // The sizes of the mockup, and the room each leaves in front.
    let cdp = p("Pool dehumidifier 40 (wall)");
    assert_eq!((cdp.length_mm, cdp.width_mm, cdp.height_mm, cdp.kind), (700.0, 320.0, 570.0, EquipmentKind::DehumidifierWall));
    assert!(
        !cdp.kind.takes_ducts() && p("Pool dehumidifier 75 (ducted)").kind.takes_ducts() && p("Pool dehumidifier unit 6000 m3/h").kind.takes_ducts()
    );
    assert!(!p("Server room cooler 25 kW").kind.takes_ducts());
    // A rack shows its door swinging; the floor one its fan as well; a cooler an arrow for each fan.
    assert_eq!(count(&p("Floor rack 42 U")), count(&p("Wall rack 12 U")) + 1);
    let one = count(&p("Server room cooler 8 kW"));
    assert_eq!(count(&p("Server room cooler 80 kW")), one + 3 * 3);
    // A rack's name carries its dissipated power, and the racks are the ones the Electricity tab offers.
    assert_eq!(p("Wall rack 9 U").text(), "X1 0.5 kW");
    let racks: Vec<_> = EquipmentKind::ALL.iter().filter(|k| k.is_rack()).collect();
    assert_eq!(racks.len(), 2);
    // Ducts can be given to a dehumidifier's sides like a ducted unit's.
    let mut d = p("Pool dehumidifier 75 (ducted)");
    d.set_air_ports(&[RiserSize::Round { diameter_mm: 250.0 }], &[RiserSize::Round { diameter_mm: 250.0 }]);
    assert_eq!(d.air_port_tips().len(), 2);
    for k in ["serverCooler", "dehumidifier", "dehumidifierWall", "wallRack", "floorRack", "baie murale", "CRAC"] {
        assert!(EquipmentKind::parse(k).is_some(), "{k}");
    }
}
