use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, HAlign, Space};

fn riser() -> RiserParams {
    RiserParams {
        name: "VMC 3".into(),
        air: AirDirection::Extract,
        up: true,
        outline: vec![Vec2::new(0.0, 0.0), Vec2::new(600.0, 0.0), Vec2::new(600.0, 400.0), Vec2::new(0.0, 400.0)],
        incoming_flow_m3h: None,
        flow_m3h: None,
        flow_set: false,
        size: None,
        incoming_size: None,
        size_set: false,
        height_mm: None,
        label_height_mm: 100.0,
        units: HvacUnits::default(),
        cable: CableData::default(),
    }
}

#[test]
fn a_riser_is_its_shaft_a_symbol_with_a_leader_and_a_label_that_reads_like_the_plan() {
    let mut p = riser();
    assert_eq!(p.text(), "VMC 3");
    p.size = Some(RiserSize::Round { diameter_mm: 200.0 });
    p.flow_m3h = Some(240.0);
    assert_eq!(p.text(), "VMC 3-Ø200-240 m³/h");
    p.size = Some(RiserSize::Rectangular { width_mm: 400.0, height_mm: 250.0 });
    assert_eq!(p.text(), "VMC 3-400×250-240 m³/h");
    // Flow typed from the other floors shows until the riser carries its own.
    p.flow_m3h = None;
    p.incoming_flow_m3h = Some(660.0);
    assert_eq!(p.text(), "VMC 3-400×250-660 m³/h");
    let pieces = p.generate().unwrap();
    assert!(pieces.iter().any(|x| x.layer == RISER_LAYER && matches!(x.kind, EntityKind::LwPolyline(_))));
    assert!(pieces.iter().any(|x| x.layer == AIR_EXTRACT_LAYER && matches!(x.kind, EntityKind::Text(_))));
    // Going up draws a dot in the symbol, going down a cross; a round section is drawn as a circle, a
    // rectangular one as a rectangle (the shaft's outline is the first polyline).
    let count = |p: &RiserParams, f: fn(&EntityKind) -> bool| p.generate().unwrap().iter().filter(|x| f(&x.kind)).count();
    let (circles, lines, polys) = (
        |k: &EntityKind| matches!(k, EntityKind::Circle(_)),
        |k: &EntityKind| matches!(k, EntityKind::Line(_)),
        |k: &EntityKind| matches!(k, EntityKind::LwPolyline(_)),
    );
    let round = RiserParams { size: Some(RiserSize::Round { diameter_mm: 200.0 }), ..p.clone() };
    let down = RiserParams { up: false, ..round.clone() };
    assert_eq!((count(&round, circles), count(&round, lines), count(&round, polys)), (2, 1, 1));
    assert_eq!((count(&down, circles), count(&down, lines), count(&down, polys)), (1, 3, 1));
    // Rectangular (400 x 250 here): no circle but the dot, and a second polyline, the section.
    assert_eq!((count(&p, circles), count(&p, lines), count(&p, polys)), (1, 1, 2), "{:?}", p.size);
}

#[test]
fn a_riser_keeps_its_data_through_the_block_and_is_edited_in_place() {
    let mut d = Drawing::new_metric();
    let mut p = riser();
    p.outline = vec![Vec2::new(1000.0, 2000.0), Vec2::new(1600.0, 2000.0), Vec2::new(1600.0, 2400.0), Vec2::new(1000.0, 2400.0)];
    let h = d.add_riser(&Space::Model, Common::default(), p).unwrap();
    let e = d.entity(h).unwrap();
    let got = riser_of(e).unwrap();
    assert_eq!((got.name.as_str(), got.shaft_mm()), ("VMC 3", (600.0, 400.0)));
    // The shaft is typed: a rectangle around the same centre.
    let mut next = got.clone();
    next.set_shaft_mm(800.0, 500.0).unwrap();
    next.size = Some(RiserSize::Round { diameter_mm: 250.0 });
    d.regenerate_riser(h, &next).unwrap();
    let again = riser_of(d.entity(h).unwrap()).unwrap();
    assert_eq!((again.shaft_mm(), again.size), ((800.0, 500.0), Some(RiserSize::Round { diameter_mm: 250.0 })));
    assert!(RiserParams { flow_m3h: Some(0.0), ..riser() }.validate().is_err());
    assert!(RiserParams { outline: vec![Vec2::ZERO; 4], ..riser() }.validate().is_err(), "no area");
    assert!(RiserParams { size: Some(RiserSize::Round { diameter_mm: 5.0 }), ..riser() }.validate().is_err());
}

#[test]
fn a_riser_that_goes_on_to_another_floor_has_an_arrow_and_a_text_at_each_end_of_its_leader() {
    // The first floor, extract air: 660 m³/h comes from below, this floor adds its own and 2620 m³/h goes up.
    let mut p = riser();
    p.size = Some(RiserSize::Round { diameter_mm: 400.0 });
    p.flow_m3h = Some(2620.0);
    p.incoming_flow_m3h = Some(660.0);
    p.size_set = true;
    let texts = |p: &RiserParams| -> Vec<String> {
        p.generate().unwrap().iter().filter_map(|x| if let EntityKind::Text(t) = &x.kind { Some(t.value.clone()) } else { None }).collect()
    };
    let t = texts(&p);
    assert_eq!(t.len(), 2, "{t:?}");
    assert!(t.iter().any(|x| x == "VMC 3-Ø400-2620 m³/h"), "{t:?}");
    assert!(t.iter().any(|x| x.ends_with("-660 m³/h") && x.starts_with("VMC 3-Ø")), "the lower end is sized for 660 m³/h: {t:?}");
    // A size typed for that part is the one written.
    p.incoming_size = Some(RiserSize::Round { diameter_mm: 250.0 });
    assert!(texts(&p).iter().any(|x| x == "VMC 3-Ø250-660 m³/h"));
    // A leader and a filled arrow head at each end.
    let count = |p: &RiserParams, f: fn(&EntityKind) -> bool| p.generate().unwrap().iter().filter(|x| f(&x.kind)).count();
    assert_eq!((count(&p, |k| matches!(k, EntityKind::Line(_))), count(&p, |k| matches!(k, EntityKind::Solid(_)))), (2, 2));
    // The ground floor has nothing below: one end only. So has a riser whose flow is all its own, or one not sized yet.
    for (flow, incoming) in [(Some(815.0), None), (Some(660.0), Some(660.0)), (None, Some(660.0))] {
        let q = RiserParams { flow_m3h: flow, incoming_flow_m3h: incoming, ..p.clone() };
        assert_eq!(texts(&q).len(), 1, "{flow:?} {incoming:?}");
    }
}

/// The heads of a riser's leader: how far each one's apex is along the leader's way up (positive: it points up), and where each text is.
fn heads_and_texts(p: &RiserParams) -> (Vec<f64>, Vec<(String, bool)>) {
    let mut heads = Vec::new();
    let mut texts = Vec::new();
    for x in p.generate().unwrap() {
        match x.kind {
            EntityKind::Solid(s) => {
                heads.push((s.corners[0].x + s.corners[0].y) - (s.corners[1].x + s.corners[2].x + s.corners[1].y + s.corners[2].y) / 2.0)
            }
            EntityKind::Text(t) => texts.push((t.value, t.halign == HAlign::Left)),
            _ => {}
        }
    }
    (heads, texts)
}

#[test]
fn the_arrow_heads_of_a_riser_point_the_way_the_air_goes_and_the_whole_flow_is_on_the_side_of_the_unit() {
    let mut p = riser();
    p.size = Some(RiserSize::Round { diameter_mm: 400.0 });
    p.flow_m3h = Some(2620.0);
    p.incoming_flow_m3h = Some(660.0);
    p.size_set = true;
    // Extract air that goes up (to the unit on the roof): both heads point up, the whole flow is above.
    let (heads, texts) = heads_and_texts(&p);
    assert!(heads.len() == 2 && heads.iter().all(|h| *h > 0.0), "{heads:?}");
    assert!(
        texts.iter().any(|(t, above)| t.ends_with("-2620 m³/h") && *above) && texts.iter().any(|(t, above)| t.ends_with("-660 m³/h") && !*above),
        "{texts:?}"
    );
    // Supply air that comes down from the roof: both heads point down, the whole flow is still above (where it comes from).
    let supply = RiserParams { air: AirDirection::Supply, up: false, ..p.clone() };
    let (heads, texts) = heads_and_texts(&supply);
    assert!(heads.len() == 2 && heads.iter().all(|h| *h < 0.0), "{heads:?}");
    assert!(texts.iter().any(|(t, above)| t.ends_with("-2620 m³/h") && *above), "{texts:?}");
    // Supply air that goes up from a unit below: the whole flow is below, the part that goes on is above.
    let rising = RiserParams { air: AirDirection::Supply, up: true, ..p.clone() };
    let (heads, texts) = heads_and_texts(&rising);
    assert!(heads.iter().all(|h| *h > 0.0));
    assert!(
        texts.iter().any(|(t, above)| t.ends_with("-2620 m³/h") && !*above) && texts.iter().any(|(t, above)| t.ends_with("-660 m³/h") && *above),
        "{texts:?}"
    );
    // With nothing to go on to, one arrow only, on the side of the unit, pointing the way.
    let alone = RiserParams { incoming_flow_m3h: None, up: false, air: AirDirection::Extract, ..p.clone() };
    let (heads, texts) = heads_and_texts(&alone);
    assert!(
        heads.len() == 1 && heads[0] < 0.0 && texts.len() == 1 && !texts[0].1,
        "an extract riser that goes down has the unit below: {heads:?} {texts:?}"
    );
}

fn cable_riser(lines: Vec<CableLine>) -> RiserParams {
    RiserParams { cable: CableData { lines, ..CableData::of(RiserMedium::Power) }, name: "CP 1".into(), ..riser() }
}

#[test]
fn a_riser_of_cables_holds_several_lines_with_arrows_that_do_not_overlap() {
    let line = |usage: CableUsage, circuit: &str, section: f64, kw: Option<f64>, from_others: Option<f64>| CableLine {
        usage,
        circuit: circuit.into(),
        section_mm2: Some(section),
        load_kw: kw,
        incoming_load_kw: from_others,
        ..CableLine::new(usage)
    };
    let lines = vec![
        line(CableUsage::Power, "TD1-C1", 35.0, Some(18.0), Some(12.0)),
        line(CableUsage::Power, "TD1-C2", 16.0, Some(9.0), None),
        line(CableUsage::Lighting, "TD1-E1", 6.0, Some(2.5), None),
        line(CableUsage::Bus, "BUS1", 1.5, None, None),
    ];
    let p = cable_riser(lines);
    let pieces = p.generate().unwrap();
    let texts: Vec<(String, f64, bool)> = pieces
        .iter()
        .filter_map(|x| if let EntityKind::Text(t) = &x.kind { Some((t.value.clone(), t.insert.y, t.halign == HAlign::Left)) } else { None })
        .collect();
    // Going up: the whole load is below (the source), the 12 kW that go on above; a bus has no load.
    assert!(texts.iter().any(|(t, _, above)| t == "TD1-C1 35 mm² 18 kW" && !above), "{texts:?}");
    assert!(texts.iter().any(|(t, _, above)| t == "TD1-C1 35 mm² 12 kW" && *above), "{texts:?}");
    assert!(texts.iter().any(|(t, ..)| t == "BUS1 2 mm²"), "{texts:?}");
    assert!(texts.iter().any(|(t, ..)| t == "CP 1 UP"));
    // Each line's circle is on the layer of the cables of its circuit: in the colour of the wire it carries, which follows the circuit's colour.
    let circles = |layer: &str| pieces.iter().filter(|x| x.layer == layer && matches!(x.kind, EntityKind::Circle(_))).count();
    for (usage, circuit) in
        [(CableUsage::Power, "TD1-C1"), (CableUsage::Power, "TD1-C2"), (CableUsage::Lighting, "TD1-E1"), (CableUsage::Bus, "BUS1")]
    {
        assert_eq!(circles(cable_layer_of(usage, circuit)), 2, "{circuit}");
    }
    assert_ne!(cable_layer_of(CableUsage::Power, "TD1-C1"), CableUsage::Power.layer(), "a circuit has a layer of its own");
    // The texts of the lines (rotated 45°, side by side across their leaders) are at least a label and a half apart across the leaders.
    let h = 100.0;
    let mut spots: Vec<(f64, f64)> = pieces
        .iter()
        .filter_map(|x| if let EntityKind::Text(t) = &x.kind { Some((t.insert.x, t.insert.y)) } else { None })
        .filter(|_| true)
        .collect();
    spots.pop(); // the name
    for (i, a) in spots.iter().enumerate() {
        for b in spots.iter().skip(i + 1) {
            let across = ((b.0 - a.0) - (b.1 - a.1)).abs() / std::f64::consts::SQRT_2;
            let along = ((b.0 - a.0) + (b.1 - a.1)).abs() / std::f64::consts::SQRT_2;
            assert!(across >= 1.5 * h - 1e-6 || along >= 3.0 * h, "{a:?} {b:?}: across {across}, along {along}");
        }
    }
    // The heads point up, as the current goes; down for a riser that goes down.
    let heads = |p: &RiserParams| -> Vec<f64> {
        p.generate()
            .unwrap()
            .iter()
            .filter_map(|x| if let EntityKind::Solid(s) = &x.kind { Some(s.corners[0].y - (s.corners[1].y + s.corners[2].y) / 2.0) } else { None })
            .collect()
    };
    assert!(heads(&p).iter().all(|d| *d > 0.0));
    assert!(heads(&RiserParams { up: false, ..p.clone() }).iter().all(|d| *d < 0.0));
    // A riser of cables is not a riser of air.
    assert!(p.is_cable() && !RiserParams { cable: CableData::default(), ..p.clone() }.is_cable());
}

#[test]
fn a_riser_drawn_with_one_circuit_before_it_held_lines_is_read_as_one_line() {
    // What the extended data of such a riser said: one circuit, section and loads, no lines.
    let mut p = cable_riser(Vec::new());
    let mut json = serde_json::to_value(&p).unwrap();
    json["cable"] = serde_json::json!({ "medium": "bus", "circuit": "BUS1", "sectionMm2": 1.5, "loadKw": null, "incomingLoadKw": null });
    p = serde_json::from_value(json).unwrap();
    assert!(p.cable.lines.is_empty() && p.cable.legacy_circuit == "BUS1");
    // Reading it back migrates: the old fields become the line.
    let mut old = p.clone();
    old.cable.migrate_for_test();
    assert_eq!(old.cable.lines.len(), 1);
    assert_eq!((old.cable.lines[0].usage, old.cable.lines[0].circuit.as_str(), old.cable.lines[0].section_mm2), (CableUsage::Bus, "BUS1", Some(1.5)));
}
