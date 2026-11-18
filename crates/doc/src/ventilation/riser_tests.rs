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
    assert_eq!(p.text(), "VMC 3-Ø0200-240 m³/h");
    p.size = Some(RiserSize::Rectangular { width_mm: 400.0, height_mm: 250.0 });
    assert_eq!(p.text(), "VMC 3-0400×0250-240 m³/h");
    // Flow typed from the other floors shows until the riser carries its own.
    p.flow_m3h = None;
    p.incoming_flow_m3h = Some(660.0);
    assert_eq!(p.text(), "VMC 3-0400×0250-660 m³/h");
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
    assert!(t.iter().any(|x| x == "VMC 3-Ø0400-2620 m³/h"), "{t:?}");
    assert!(t.iter().any(|x| x.ends_with("-660 m³/h") && x.starts_with("VMC 3-Ø")), "the lower end is sized for 660 m³/h: {t:?}");
    // A size typed for that part is the one written.
    p.incoming_size = Some(RiserSize::Round { diameter_mm: 250.0 });
    assert!(texts(&p).iter().any(|x| x == "VMC 3-Ø0250-660 m³/h"));
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
