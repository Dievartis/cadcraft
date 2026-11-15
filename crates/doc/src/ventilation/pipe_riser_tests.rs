use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn shaft(x: f64, y: f64, w: f64, d: f64) -> Vec<Vec2> {
    vec![Vec2::new(x, y), Vec2::new(x + w, y), Vec2::new(x + w, y + d), Vec2::new(x, y + d)]
}

fn riser(lines: Vec<RiserLine>) -> PipeRiserParams {
    PipeRiserParams {
        name: "CT1".into(),
        up: true,
        outline: shaft(0.0, 0.0, 600.0, 400.0),
        lines,
        height_mm: Some(3200.0),
        label_height_mm: 100.0,
        units: units(),
    }
}

fn chw() -> Vec<RiserLine> {
    vec![RiserLine::new(PipeService::ChilledWater, PipeLeg::Supply, 80.0), RiserLine::new(PipeService::ChilledWater, PipeLeg::Return, 80.0)]
}

#[test]
fn a_riser_is_a_shaft_with_a_symbol_and_a_text_for_each_line_in_the_colour_of_its_service() {
    let mut lines = chw();
    lines.push(RiserLine::new(PipeService::HotWater, PipeLeg::Supply, 50.0));
    lines.push(RiserLine::new(PipeService::Condensate, PipeLeg::Supply, 32.0));
    lines.push(RiserLine::new(PipeService::RefrigerantTriple, PipeLeg::Supply, 9.52));
    let p = riser(lines);
    let pieces = p.generate().unwrap();
    // The shaft on its layer.
    assert_eq!(pieces.iter().filter(|x| x.layer == PIPE_RISER_LAYER && matches!(x.kind, EntityKind::LwPolyline(_))).count(), 1);
    // A circle for each water line, and one for each tube of the link, on the layer of the service.
    let circles = |layer: &str| pieces.iter().filter(|x| x.layer == layer && matches!(x.kind, EntityKind::Circle(_))).count();
    assert_eq!(circles(PIPE_CHW_SUPPLY_LAYER), 1 + 1, "the pipe and its dot");
    assert_eq!(circles(PIPE_HW_SUPPLY_LAYER), 2);
    assert!(circles(PIPE_REFRIGERANT_LIQUID_LAYER) >= 1 && circles(PIPE_REFRIGERANT_HPLP_LAYER) >= 1 && circles(PIPE_REFRIGERANT_GAS_LAYER) >= 1);
    // The text: the name and the direction, then a line each.
    let texts: Vec<String> = pieces.iter().filter_map(|x| if let EntityKind::Text(t) = &x.kind { Some(t.value.clone()) } else { None }).collect();
    assert_eq!(texts.len(), 1 + 5);
    assert_eq!(texts[0], "CT1 UP");
    assert!(texts.contains(&"CHW-S DN80".to_string()) && texts.contains(&"HW-S DN50".to_string()) && texts.contains(&"COND DN32".to_string()));
    assert!(texts.iter().any(|t| t.contains("REF-HR") && t.contains("3/8\"")), "{texts:?}");
    // Going down, a cross in place of the dot.
    let down = PipeRiserParams { up: false, ..riser(chw()) }.generate().unwrap();
    assert_eq!(down.iter().filter(|x| x.layer == PIPE_CHW_SUPPLY_LAYER && matches!(x.kind, EntityKind::Line(_))).count(), 2);
}

#[test]
fn the_symbols_of_the_lines_sit_in_rows_inside_the_shaft() {
    let many: Vec<RiserLine> =
        (0..8).map(|i| RiserLine::new(PipeService::ChilledWater, if i % 2 == 0 { PipeLeg::Supply } else { PipeLeg::Return }, 100.0)).collect();
    let p = riser(many);
    let centres = p.symbol_centres();
    assert_eq!(centres.len(), 8);
    let (lo, hi) = (Vec2::new(0.0, 0.0), Vec2::new(600.0, 400.0));
    assert!(centres.iter().all(|c| c.x > lo.x && c.x < hi.x && c.y > lo.y && c.y < hi.y), "{centres:?}");
    // A narrow shaft puts them in a column.
    let narrow = PipeRiserParams { outline: shaft(0.0, 0.0, 150.0, 900.0), ..riser(chw()) };
    let c = narrow.symbol_centres();
    assert!((c[0].x - c[1].x).abs() < 1e-9 && (c[0].y - c[1].y).abs() > 1.0);
    assert!(riser(Vec::new()).symbol_centres().is_empty());
}

#[test]
fn a_riser_is_added_regenerated_and_found_again_with_its_shaft_in_the_drawing() {
    let mut d = Drawing::new_metric();
    let mut p = riser(chw());
    p.outline = shaft(5000.0, 2000.0, 600.0, 400.0);
    let h = d.add_pipe_riser(&Space::Model, Common::default(), p).unwrap();
    assert_eq!(d.entity(h).unwrap().common.layer, PIPE_RISER_LAYER);
    let (back, outline) = d.pipe_riser_outline(h).unwrap();
    assert_eq!((back.name.as_str(), back.lines.len(), back.height_mm), ("CT1", 2, Some(3200.0)));
    assert_eq!((outline[0], outline[2]), (Vec2::new(5000.0, 2000.0), Vec2::new(5600.0, 2400.0)), "the shaft in drawing coordinates");
    let mut next = back.clone();
    next.lines.push(RiserLine::new(PipeService::HotWater, PipeLeg::Return, 40.0));
    d.regenerate_pipe_riser(h, &next).unwrap();
    assert_eq!(pipe_riser_of(d.entity(h).unwrap()).unwrap().lines.len(), 3);
    assert_eq!(d.pipe_risers().len(), 1);
    // A point in the shaft, on its edge, and outside it.
    assert!(
        in_shaft(&outline, Vec2::new(5300.0, 2200.0), 0.0)
            && in_shaft(&outline, Vec2::new(4990.0, 2200.0), 15.0)
            && !in_shaft(&outline, Vec2::new(4900.0, 2200.0), 15.0)
    );
}

#[test]
fn a_riser_that_cannot_be_drawn_is_refused() {
    let ok = riser(chw());
    let mut bad_line = ok.clone();
    bad_line.lines[0].dn_mm = -3.0;
    let mut nan_load = ok.clone();
    nan_load.lines[0].incoming = Some(f64::NAN);
    let many = PipeRiserParams { lines: (0..40).map(|_| RiserLine::new(PipeService::HotWater, PipeLeg::Supply, 20.0)).collect(), ..ok.clone() };
    for bad in [
        PipeRiserParams { name: String::new(), ..ok.clone() },
        PipeRiserParams { outline: vec![Vec2::ZERO, Vec2::new(1.0, 0.0)], ..ok.clone() },
        PipeRiserParams { outline: vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)], ..ok.clone() },
        PipeRiserParams { height_mm: Some(0.0), ..ok.clone() },
        bad_line,
        nan_load,
        many,
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    let l = RiserLine { incoming: Some(120.0), ..RiserLine::new(PipeService::ChilledWater, PipeLeg::Supply, 65.0) };
    assert_eq!(l.text(units()), "CHW-S DN65 120 kW");
    assert!(
        l.is(PipeService::ChilledWater, PipeLeg::Supply)
            && !l.is(PipeService::ChilledWater, PipeLeg::Return)
            && !l.is(PipeService::HotWater, PipeLeg::Supply)
    );
    assert!(RiserLine::new(PipeService::Condensate, PipeLeg::Return, 25.0).is(PipeService::Condensate, PipeLeg::Supply), "a drain has no legs");
}

fn run(d: &mut Drawing, service: PipeService, leg: PipeLeg, a: (f64, f64), b: (f64, f64), load: Option<f64>) -> crate::Handle {
    let p = PipeParams { leg, load_kw: load, ..PipeParams::new(service, 25.0, units()) };
    d.add_pipe(&Space::Model, Common::default(), p, Vec2::new(a.0, a.1), Vec2::new(b.0, b.1)).unwrap()
}

fn line_of(d: &Drawing, h: crate::Handle, i: usize) -> RiserLine {
    pipe_riser_of(d.entity(h).unwrap()).unwrap().lines[i].clone()
}

#[test]
fn a_pipe_that_starts_in_a_shaft_takes_the_riser_as_its_source_and_the_line_carries_the_floor_and_the_floors_above() {
    let mut d = Drawing::new_metric();
    let mut r = riser(chw());
    r.outline = shaft(0.0, -200.0, 600.0, 400.0);
    r.lines[0].incoming = Some(40.0);
    let rh = d.add_pipe_riser(&Space::Model, Common::default(), r).unwrap();
    // The supply starts in the shaft (no "source" flag) and feeds two units of 25 kW; the return is not typed.
    let main = run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (300.0, 0.0), (5000.0, 0.0), None);
    run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (5000.0, 0.0), (5000.0, 3000.0), Some(25.0));
    run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (5000.0, 0.0), (5000.0, -3000.0), Some(25.0));
    let (reports, warnings) = d.size_pipes(&PipeSizingParams::default()).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!((reports.len(), reports[0].source, reports[0].total_load), (1, main, 50.0));
    // The line carries the 50 kW of the floor and the 40 kW of the floors above: DN65 at 90 kW (15.5 m³/h).
    let l = line_of(&d, rh, 0);
    assert_eq!(l.load, Some(90.0));
    assert!((l.flow_m3h.unwrap() - 15.5).abs() < 0.1, "{:?}", l.flow_m3h);
    assert!(l.dn_mm >= 65.0 && !l.imposed && l.velocity.is_some());
    // Typed by the user: kept and checked.
    d.regenerate_pipe_riser(
        rh,
        &PipeRiserParams {
            lines: vec![RiserLine { dn_mm: 100.0, imposed: true, ..line_of(&d, rh, 0) }, line_of(&d, rh, 1)],
            ..pipe_riser_of(d.entity(rh).unwrap()).unwrap()
        },
    )
    .unwrap();
    d.size_pipes(&PipeSizingParams::default()).unwrap();
    assert_eq!(line_of(&d, rh, 0).dn_mm, 100.0);
    // A line that carries nothing says nothing.
    assert_eq!(line_of(&d, rh, 1).load, None);
}

#[test]
fn a_pipe_that_ends_in_a_shaft_carries_what_the_floors_above_need() {
    let mut d = Drawing::new_metric();
    let units = HvacUnits { mm_per_unit: 1.0 };
    // A chiller of 100 kW; the supply goes to a unit of 30 kW and to the shaft, where the line carries 70 kW to the floors above.
    let m = EquipmentModel {
        name: "x".into(),
        kind: EquipmentKind::Chiller,
        reference: String::new(),
        capacity_kw: Some(100.0),
        heating_kw: None,
        length_mm: 1000.0,
        width_mm: 600.0,
        height_mm: 500.0,
        weight_kg: None,
        fans: 0,
        connections: String::new(),
    };
    d.add_equipment(&Space::Model, Common::default(), &EquipmentParams::from_model(&m, "GE", units), Vec2::ZERO, 0.0).unwrap();
    let mut r = riser(chw());
    r.outline = shaft(8000.0, -200.0, 600.0, 400.0);
    r.lines[0].incoming = Some(70.0);
    let rh = d.add_pipe_riser(&Space::Model, Common::default(), r).unwrap();
    run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (200.0, 0.0), (4000.0, 0.0), None);
    run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (4000.0, 0.0), (4000.0, 3000.0), Some(30.0));
    run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (4000.0, 0.0), (8300.0, 0.0), None);
    let (reports, warnings) = d.size_pipes(&PipeSizingParams::default()).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(reports[0].total_load, 100.0, "30 kW typed and 70 kW carried to the floors above");
    assert_eq!(line_of(&d, rh, 0).load, Some(70.0));
    // Without it the run that ends in the shaft serves nothing: it is told.
    let mut bare = Drawing::new_metric();
    bare.add_equipment(&Space::Model, Common::default(), &EquipmentParams::from_model(&m, "GE", units), Vec2::ZERO, 0.0).unwrap();
    let mut r2 = riser(chw());
    r2.outline = shaft(8000.0, -200.0, 600.0, 400.0);
    bare.add_pipe_riser(&Space::Model, Common::default(), r2).unwrap();
    run(&mut bare, PipeService::ChilledWater, PipeLeg::Supply, (200.0, 0.0), (8300.0, 0.0), None);
    let (reports, _) = bare.size_pipes(&PipeSizingParams::default()).unwrap();
    assert!(reports[0].warnings.iter().any(|w| w.contains("serves no load")), "{:?}", reports[0].warnings);
}

#[test]
fn the_takeoff_counts_the_lines_of_a_riser_by_the_height_of_the_floor() {
    let mut d = Drawing::new_metric();
    let mut r = riser(chw());
    r.lines.push(RiserLine::new(PipeService::RefrigerantPair, PipeLeg::Supply, 9.52));
    d.add_pipe_riser(&Space::Model, Common::default(), r).unwrap();
    run(&mut d, PipeService::ChilledWater, PipeLeg::Supply, (0.0, 2000.0), (5000.0, 2000.0), None);
    let t = d.takeoff("").unwrap();
    let find = |s: PipeService, leg: PipeLeg| t.pipes.iter().find(|q| q.service == s && q.leg == leg && q.size != "DN25").cloned();
    let supply = find(PipeService::ChilledWater, PipeLeg::Supply).unwrap();
    assert_eq!((supply.size.as_str(), supply.count), ("DN80", 1), "the riser line is DN80, the run DN25 apart");
    assert!((supply.length_m - 3.2).abs() < 1e-9);
    assert!((find(PipeService::ChilledWater, PipeLeg::Return).unwrap().length_m - 3.2).abs() < 1e-9);
    assert!((find(PipeService::RefrigerantPair, PipeLeg::Supply).unwrap().length_m - 3.2).abs() < 1e-9);
    assert!(t.pipes.iter().any(|q| q.size == "DN25" && (q.length_m - 5.0).abs() < 1e-9), "the pipe of the floor is counted as before");
}

#[test]
fn two_refrigerant_links_in_a_shaft_do_not_overlap() {
    let lines = vec![
        RiserLine::new(PipeService::RefrigerantPair, PipeLeg::Supply, 6.35),
        RiserLine::new(PipeService::RefrigerantPair, PipeLeg::Supply, 9.52),
        RiserLine::new(PipeService::RefrigerantTriple, PipeLeg::Supply, 9.52),
    ];
    let r = riser(lines);
    let centres = r.symbol_centres();
    // Each link is drawn as its tubes side by side: no two links come closer than the half-widths of both.
    for (i, a) in centres.iter().enumerate() {
        for (j, b) in centres.iter().enumerate().skip(i + 1) {
            let half = (r.symbol_width_units(&r.lines[i]) + r.symbol_width_units(&r.lines[j])) / 2.0;
            assert!(a.dist(*b) >= half - 1e-6, "links {i} and {j} overlap: {} < {half}", a.dist(*b));
        }
    }
}
