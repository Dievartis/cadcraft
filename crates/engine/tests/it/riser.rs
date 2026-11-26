//! RISERADD draws the shaft of a riser with its symbol and label, by points, by a closed
//! polyline or by a centre and a size; RISEREDIT changes the way, the air, the flows, the size and the shaft.

use cadcraft_doc::{AirDirection, Drawing, Handle, RiserSize, riser_of};
use cadcraft_engine::Session;
use serde_json::{Value, json};

fn metric() -> Session {
    let mut s = Session::new();
    *s.doc_mut().unwrap() = Drawing::new_metric();
    s
}

fn hd(v: &Value) -> Handle {
    Handle::parse_hex(v.as_str().unwrap()).unwrap()
}

fn riser(s: &Session, h: Handle) -> cadcraft_doc::RiserParams {
    riser_of(s.doc().unwrap().entity(h).unwrap()).unwrap()
}

#[test]
fn riseradd_takes_a_shaft_by_points_by_a_centre_and_size_or_by_a_polyline() {
    let mut s = metric();
    let r = s
        .execute(
            "riseradd",
            &json!({"points": [[0, 0], [600, 0], [600, 400], [0, 400]], "name": "VMC 3", "air": "extract", "size": 200, "flow": 240}),
        )
        .unwrap();
    let p = riser(&s, hd(&r["handle"]));
    assert_eq!((p.name.as_str(), p.air, p.up, p.shaft_mm()), ("VMC 3", AirDirection::Extract, true, (600.0, 400.0)));
    assert_eq!((p.size, p.flow_m3h, p.text().as_str()), (Some(RiserSize::Round { diameter_mm: 200.0 }), Some(240.0), "VMC 3-Ø200-240 m³/h"));
    // A centre and a size make a rectangle around it; the way is given.
    let r = s.execute("riseradd", &json!({"at": [5000, 5000], "width": 800, "depth": 500, "direction": "down", "air": "fresh"})).unwrap();
    let q = riser(&s, hd(&r["handle"]));
    assert_eq!((q.shaft_mm(), q.up, q.air, q.name.as_str()), ((800.0, 500.0), false, AirDirection::Fresh, "Riser 2"));
    // A closed polyline already drawn.
    let pl = s.execute("pline", &json!({"points": [[0, 2000], [300, 2000], [300, 2300], [0, 2300]], "closed": true})).unwrap();
    let r = s.execute("riseradd", &json!({"handle": pl["handle"]})).unwrap();
    assert_eq!(riser(&s, hd(&r["handle"])).shaft_mm(), (300.0, 300.0));
    assert!(s.execute("riseradd", &json!({"points": [[0, 0], [1, 0], [1, 1]], "air": "sideways"})).is_err());
    assert!(s.execute("riseradd", &json!({"name": "x"})).is_err());
}

#[test]
fn riseredit_changes_the_way_the_flows_the_size_and_the_shaft_and_clears_what_is_null() {
    let mut s = metric();
    let r = s.execute("riseradd", &json!({"at": [0, 0], "width": 600, "depth": 400})).unwrap();
    let h = r["handle"].clone();
    s.execute("riseredit", &json!({"handle": h, "direction": "down", "air": "supply", "incomingFlow": 660, "size": "400x250", "width": 800}))
        .unwrap();
    let p = riser(&s, hd(&h));
    assert_eq!((p.up, p.air, p.incoming_flow_m3h, p.shaft_mm()), (false, AirDirection::Supply, Some(660.0), (800.0, 400.0)));
    assert_eq!(p.size, Some(RiserSize::Rectangular { width_mm: 400.0, height_mm: 250.0 }));
    assert_eq!(p.text(), "Riser 1-400×250-660 m³/h");
    s.execute("riseredit", &json!({"handle": h, "incomingFlow": null, "size": "auto"})).unwrap();
    let p = riser(&s, hd(&h));
    assert_eq!((p.incoming_flow_m3h, p.size), (None, None));
    assert!(s.execute("riseredit", &json!({"handle": h, "flow": -5})).is_err());
    assert!(s.execute("riseredit", &json!({"handle": h, "size": "wide"})).is_err());
    // Not a riser.
    let line = s.execute("line", &json!({"points": [[0, 0], [10, 0]]})).unwrap();
    assert!(s.execute("riseredit", &json!({"handle": line["handles"][0], "direction": "up"})).is_err());
}

#[test]
fn riseradd_at_the_command_line_takes_the_options_then_the_shaft_points() {
    let mut s = metric();
    for t in ["riseradd", "n", "A.R", "a", "fresh", "d", "0,0", "600,0", "600,400", "c"] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let d = s.doc().unwrap();
    let p = d.model.iter().find_map(|e| riser_of(e)).unwrap();
    assert_eq!((p.name.as_str(), p.air, p.up, p.shaft_mm()), ("A.R", AirDirection::Fresh, false, (600.0, 400.0)));
    // What was used last is the next default.
    let r = s.execute("riseradd", &json!({"at": [3000, 0], "width": 300, "depth": 300, "name": "B"})).unwrap();
    let q = riser(&s, hd(&r["handle"]));
    assert_eq!((q.air, q.up), (AirDirection::Fresh, false));
}

#[test]
fn ductsize_reads_a_floor_from_its_riser_and_writes_back_the_flow_it_carries_and_its_size() {
    let mut s = metric();
    // The shaft, 600 x 400, extract, 660 m³/h coming from the floors below; two extract terminals of this floor.
    let r =
        s.execute("riseradd", &json!({"at": [300, 200], "width": 600, "depth": 400, "name": "A.R", "air": "extract", "incomingFlow": 660})).unwrap();
    s.execute("ductadd", &json!({"points": [[600, 200], [5000, 200]], "air": "extract"})).unwrap();
    s.execute("airterminaladd", &json!({"at": [5000, 200], "flow": 250, "air": "extract", "type": "squareCeiling"})).unwrap();
    let out = s.execute("ductsize", &json!({})).unwrap();
    let nets = out["networks"].as_array().unwrap();
    assert_eq!((nets.len(), nets[0]["air"].as_str(), nets[0]["totalFlow"].as_f64()), (1, Some("extract"), Some(250.0)), "{out}");
    let p = riser(&s, hd(&r["handle"]));
    assert_eq!(p.flow_m3h, Some(910.0), "this floor's 250 and the 660 from below");
    assert!(p.size.is_some(), "the riser is sized for that flow in its shaft");
    assert!(p.text().contains("910 m³/h"), "{}", p.text());
    // A size typed on the riser is kept by the next sizing.
    s.execute("riseredit", &json!({"handle": r["handle"], "size": 250})).unwrap();
    s.execute("ductsize", &json!({})).unwrap();
    assert_eq!(riser(&s, hd(&r["handle"])).size, Some(RiserSize::Round { diameter_mm: 250.0 }));
}

#[test]
fn a_run_started_on_the_edge_of_another_near_its_corner_leaves_no_short_piece_out_of_the_sizing() {
    // From the riser up, then right to the far end; then, from the corner facing the riser's outlet, to
    // the left: the click lands on the edge of the run, a few centimetres from where the first one turns.
    for start in [[300.0, 3000.0], [300.0, 3090.0], [330.0, 3090.0], [270.0, 3085.0], [380.0, 3090.0]] {
        let mut s = metric();
        s.execute("riseradd", &json!({"at": [300, 200], "width": 600, "depth": 400, "air": "extract", "name": "R"})).unwrap();
        s.execute("ductadd", &json!({"points": [[300, 200], [300, 3000], [6000, 3000]], "diameter": 200})).unwrap();
        s.execute("ductadd", &json!({"points": [start, [-4000, 3090]], "diameter": 200})).unwrap();
        // Another run's end a hand's breadth before the end of the left one: it must not leave a stub either.
        for at in [[6000, 3000], [-4000, 3090]] {
            s.execute("airterminaladd", &json!({"at": at, "flow": 200, "air": "extract", "type": "squareCeiling"})).unwrap();
        }
        let r = s.execute("ductsize", &json!({})).unwrap();
        let net = &r["networks"][0];
        assert_eq!(net["totalFlow"].as_f64(), Some(400.0), "start {start:?}: {r}");
        assert!(net["warnings"].as_array().unwrap().is_empty(), "start {start:?}: {net}");
        assert_eq!((r["reached"].as_u64(), r["ducts"].as_u64()), (r["ducts"].as_u64(), r["ducts"].as_u64()), "{r}");
        let d = s.doc().unwrap();
        for e in d.model.iter() {
            if let Some((p, a, b)) = d.duct_ends(e.handle) {
                assert!(a.dist(b) > 100.0, "start {start:?}: a piece of {:.0} mm", a.dist(b));
                assert_eq!(p.air, cadcraft_doc::AirDirection::Extract, "start {start:?}");
            }
        }
    }
}

#[test]
fn on_the_roof_a_riser_the_runs_of_the_unit_end_on_is_sized_for_the_flow_typed_on_it_in_its_shaft() {
    let mut s = metric();
    s.execute("ahuadd", &json!({"at": [0, 0], "type": "supplyUnit"})).unwrap();
    // The shaft is 1500 x 600; 7000 m³/h from the floors below. A round duct of 600 - 100 mm cannot carry that at 6 m/s.
    let r = s.execute("riseradd", &json!({"at": [4000, 0], "width": 1500, "depth": 600, "air": "supply", "incomingFlow": 7000})).unwrap();
    s.execute("ductadd", &json!({"points": [[1300, 0], [3400, 0]], "air": "supply"})).unwrap();
    let out = s.execute("ductsize", &json!({})).unwrap();
    assert_eq!(out["networks"][0]["totalFlow"].as_f64(), Some(7000.0), "{out}");
    let p = riser(&s, hd(&r["handle"]));
    assert_eq!(p.flow_m3h, Some(7000.0));
    match p.size {
        Some(RiserSize::Rectangular { width_mm, height_mm }) => {
            assert!(height_mm <= 500.0 && width_mm <= 1400.0, "{width_mm} x {height_mm} fits the shaft less the margin");
            assert!(width_mm * height_mm / 1e6 * 6.0 * 3600.0 >= 7000.0 - 1.0, "and carries the flow at 6 m/s");
        }
        other => panic!("a rectangular section is expected, got {other:?}"),
    }
    // A size typed on the riser is kept.
    s.execute("riseredit", &json!({"handle": r["handle"], "size": "900x500"})).unwrap();
    s.execute("ductsize", &json!({})).unwrap();
    assert_eq!(riser(&s, hd(&r["handle"])).size, Some(RiserSize::Rectangular { width_mm: 900.0, height_mm: 500.0 }));
}

#[test]
fn on_the_roof_a_reducer_joins_the_orifice_of_the_unit_to_the_duct_and_the_last_duct_has_the_section_of_the_shaft() {
    let mut s = metric();
    let u = s.execute("ahuadd", &json!({"at": [0, 0], "type": "supplyUnit"})).unwrap();
    s.execute("ahuedit", &json!({"handle": u["handle"], "portSizes": [null, "800x400"]})).unwrap();
    let r = s.execute("riseradd", &json!({"at": [4000, 0], "width": 1500, "depth": 600, "air": "supply", "incomingFlow": 7000})).unwrap();
    let first = s.execute("ductadd", &json!({"points": [[1300, 0], [2300, 0]], "air": "supply"})).unwrap();
    let last = s.execute("ductadd", &json!({"points": [[2300, 0], [3400, 0]], "air": "supply"})).unwrap();
    s.execute("ductsize", &json!({})).unwrap();
    let d = |h: &Value| cadcraft_doc::duct_of(s.doc().unwrap().entity(hd(&h["handles"][0])).unwrap()).unwrap();
    let (a, b) = (d(&first), d(&last));
    assert_eq!(a.reducer.map(|red| red.from_mm), Some(800.0), "from the unit's orifice to the first duct: {:?}", a.reducer);
    let shaft = riser(&s, hd(&r["handle"])).size.unwrap();
    let RiserSize::Rectangular { width_mm, height_mm } = shaft else { panic!("{shaft:?}") };
    assert_eq!((b.width_mm, b.height_mm), (width_mm, height_mm), "the last run is the shaft's section");
}

#[test]
fn a_short_run_that_ends_on_a_riser_is_a_main_not_a_connection_and_is_not_sized_at_the_velocity_of_a_diffuser_drop() {
    let mut s = metric();
    s.execute("ahuadd", &json!({"at": [0, 0], "type": "supplyUnit"})).unwrap();
    s.execute("riseradd", &json!({"at": [4600, 0], "width": 1500, "depth": 600, "air": "supply", "incomingFlow": 7000})).unwrap();
    // Three runs of 1, 1 and 1.5 m: the last two are short and serve one terminal, the riser. A run to
    // a diffuser that short is a connection at 2.5 m/s; one that carries 7000 m³/h to a riser is not.
    let a = s.execute("ductadd", &json!({"points": [[1300, 0], [2300, 0]], "air": "supply"})).unwrap();
    let b = s.execute("ductadd", &json!({"points": [[2300, 0], [3300, 0]], "air": "supply"})).unwrap();
    let c = s.execute("ductadd", &json!({"points": [[3300, 0], [4000, 0]], "air": "supply"})).unwrap();
    s.execute("ductsize", &json!({})).unwrap();
    let d = |h: &Value| cadcraft_doc::duct_of(s.doc().unwrap().entity(hd(&h["handles"][0])).unwrap()).unwrap();
    for (name, p) in [("first", d(&a)), ("second", d(&b)), ("last", d(&c))] {
        assert_ne!(p.kind_found, Some(cadcraft_doc::DuctKind::Connection), "{name} run: {:?}", p.kind_found);
        let v = p.velocity.expect("sized");
        assert!(v > 3.0, "{name} run at {v} m/s: sized like a drop to a diffuser (2.5 m/s)");
    }
}

#[test]
fn a_flow_typed_on_a_riser_is_kept_by_sizing_which_sizes_the_riser_and_its_run_for_it() {
    let mut s = metric();
    let r = s.execute("riseradd", &json!({"at": [300, 200], "width": 600, "depth": 400, "name": "A.R", "air": "extract"})).unwrap();
    let run = s.execute("ductadd", &json!({"points": [[600, 200], [5000, 200]], "air": "extract"})).unwrap();
    s.execute("airterminaladd", &json!({"at": [5000, 200], "flow": 250, "air": "extract", "type": "squareCeiling"})).unwrap();
    s.execute("ductsize", &json!({})).unwrap();
    assert_eq!(riser(&s, hd(&r["handle"])).flow_m3h, Some(250.0), "found by sizing");
    // Typed: 900 m³/h. Sizing keeps it, and the run that leaves the riser carries it.
    s.execute("riseredit", &json!({"handle": r["handle"], "flow": 900})).unwrap();
    let out = s.execute("ductsize", &json!({})).unwrap();
    let p = riser(&s, hd(&r["handle"]));
    assert_eq!((p.flow_m3h, p.flow_set), (Some(900.0), true));
    assert_eq!(out["networks"][0]["totalFlow"].as_f64(), Some(900.0), "{out}");
    let d = cadcraft_doc::duct_of(s.doc().unwrap().entity(hd(&run["handles"][0])).unwrap()).unwrap();
    assert_eq!(d.flow_m3h, Some(900.0));
    // Null gives it back to sizing.
    s.execute("riseredit", &json!({"handle": r["handle"], "flow": null})).unwrap();
    s.execute("ductsize", &json!({})).unwrap();
    let p = riser(&s, hd(&r["handle"]));
    assert_eq!((p.flow_m3h, p.flow_set), (Some(250.0), false));
}

#[test]
fn a_riser_of_cables_counts_its_height_in_its_circuit_and_a_bus_riser_in_the_length_of_its_bus() {
    let mut s = metric();
    // A circuit of two sockets on a 10 m cable, then a riser that carries it between the floors (3 m, and 4 kW from the other floors).
    s.execute("elecdeviceadd", &json!({"points": [[0, 0], [1000, 0]], "kind": "socket", "circuit": "TD1-C1", "power": 1000})).unwrap();
    s.execute("cableadd", &json!({"points": [[0, 0], [10000, 0]], "circuit": "TD1-C1"})).unwrap();
    let before = s.execute("circuitcheck", &json!({})).unwrap();
    let c = |r: &Value| r["circuits"].as_array().unwrap().iter().find(|c| c["name"] == "TD1-C1").cloned().unwrap();
    let (length, power) = (c(&before)["cableLengthM"].as_f64().unwrap(), c(&before)["powerW"].as_f64().unwrap());
    let r = s
        .execute("elecriser", &json!({"at": [2000, 2000], "width": 400, "depth": 400, "circuit": "TD1-C1", "height": 3000, "incomingLoad": 4, "load": 6, "section": 16}))
        .unwrap();
    let p = cadcraft_doc::any_riser_of(s.doc().unwrap().entity(hd(&r["handle"])).unwrap()).unwrap();
    assert_eq!((p.cable.medium, p.name.as_str(), p.cable.lines[0].circuit.as_str()), (cadcraft_doc::RiserMedium::Power, "Power riser 1", "TD1-C1"));
    assert_eq!(p.text(), "Power riser 1 TD1-C1 16 mm² 6 kW");
    // The riser is not an air riser: the air networks and the air takeoff never see it.
    assert!(riser_of(s.doc().unwrap().entity(hd(&r["handle"])).unwrap()).is_none());
    let after = s.execute("circuitcheck", &json!({})).unwrap();
    assert!((c(&after)["cableLengthM"].as_f64().unwrap() - (length + 3.0)).abs() < 0.01, "{after}");
    assert!((c(&after)["powerW"].as_f64().unwrap() - (power + 4000.0)).abs() < 1.0, "{after}");
    // Edited like a riser: the load, the circuit, the way.
    s.execute("riseredit", &json!({"handle": r["handle"], "load": null, "direction": "down", "circuit": "TD1-C2"})).unwrap();
    let p = cadcraft_doc::any_riser_of(s.doc().unwrap().entity(hd(&r["handle"])).unwrap()).unwrap();
    assert_eq!((p.cable.lines[0].load_kw, p.up, p.cable.lines[0].circuit.as_str()), (None, false, "TD1-C2"));
    // The cable takeoff counts the vertical run.
    let t = s.execute("electakeoff", &json!({})).unwrap();
    assert!(t["cables"].as_array().unwrap().iter().any(|q| q["size"].as_str().is_some_and(|z| z.contains("riser"))), "{t}");
    // A bus riser: its height is part of the bus, so the check counts it (a riser alone is a cable drawn for that bus).
    s.execute("controladd", &json!({"at": [0, 0], "kind": "controller", "name": "AU1", "bus": "BUS1", "protocol": "modbus", "ai": 8})).unwrap();
    let b = s.execute("busriser", &json!({"at": [5000, 2000], "width": 300, "depth": 300, "circuit": "BUS1", "height": 5000})).unwrap();
    assert_eq!(cadcraft_doc::any_riser_of(s.doc().unwrap().entity(hd(&b["handle"])).unwrap()).unwrap().name, "Bus riser 1");
    let r = s.execute("controlcheck", &json!({})).unwrap();
    assert!(!r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("no bus cable")), "{r}");
    // 1198 m of cable and the 5 m of the riser are over the 1200 m of a Modbus segment; with the riser at 1 m the bus is within it.
    s.execute("cableadd", &json!({"points": [[0, 900], [1_198_000, 900]], "circuit": "BUS1", "usage": "bus"})).unwrap();
    let too_long = |s: &mut Session| {
        let r = s.execute("controlcheck", &json!({})).unwrap();
        r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("bus BUS1") && w.as_str().unwrap().contains("at most"))
    };
    assert!(too_long(&mut s));
    s.execute("riseredit", &json!({"handle": b["handle"], "height": 1000})).unwrap();
    assert!(!too_long(&mut s));
}

#[test]
fn a_riser_of_cables_holds_several_lines_each_counted_in_its_own_circuit() {
    let mut s = metric();
    s.execute("elecdeviceadd", &json!({"at": [0, 0], "kind": "socket", "circuit": "TD1-C1", "power": 1000})).unwrap();
    s.execute("cableadd", &json!({"points": [[0, 0], [10000, 0]], "circuit": "TD1-C1"})).unwrap();
    s.execute("cableadd", &json!({"points": [[0, 500], [20000, 500]], "circuit": "TD1-C2"})).unwrap();
    let r = s
        .execute(
            "elecriser",
            &json!({"at": [0, 3000], "width": 600, "depth": 600, "height": 3000, "lines": [
                {"circuit": "TD1-C1", "section": 16, "incomingLoad": 2},
                {"circuit": "TD1-C2", "usage": "lighting"},
                {"circuit": "BUS1", "usage": "bus"},
            ]}),
        )
        .unwrap();
    let h = hd(&r["handle"]);
    let lines = |s: &Session| cadcraft_doc::any_riser_of(s.doc().unwrap().entity(h).unwrap()).unwrap().cable.lines;
    assert_eq!(
        lines(&s).iter().map(|l| (l.usage.name(), l.circuit.as_str())).collect::<Vec<_>>(),
        [("power", "TD1-C1"), ("lighting", "TD1-C2"), ("bus", "BUS1")]
    );
    // Each line adds the height of the riser to its own circuit.
    let c = s.execute("circuitcheck", &json!({})).unwrap();
    let length = |name: &str| c["circuits"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap()["cableLengthM"].as_f64().unwrap();
    assert!((length("TD1-C1") - 13.0).abs() < 0.01 && (length("TD1-C2") - 23.0).abs() < 0.01, "{c}");
    // The shaft counts one run of cable for each line in the takeoff.
    let t = s.execute("electakeoff", &json!({})).unwrap();
    let risers: f64 = t["cables"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|q| q["size"].as_str().is_some_and(|z| z.contains("riser")))
        .map(|q| q["lengthM"].as_f64().unwrap())
        .sum();
    assert!((risers - 9.0).abs() < 0.01, "{t}");
    // Add, change and remove a line.
    s.execute("riseredit", &json!({"handle": r["handle"], "addLines": [{"circuit": "TD2-C1"}]})).unwrap();
    assert_eq!(lines(&s).len(), 4);
    s.execute("riseredit", &json!({"handle": r["handle"], "line": {"index": 4, "section": 25, "load": 7, "usage": "power"}})).unwrap();
    assert_eq!((lines(&s)[3].section_mm2, lines(&s)[3].load_kw), (Some(25.0), Some(7.0)));
    s.execute("riseredit", &json!({"handle": r["handle"], "removeLine": 2})).unwrap();
    assert_eq!(lines(&s).iter().map(|l| l.circuit.as_str()).collect::<Vec<_>>(), ["TD1-C1", "BUS1", "TD2-C1"]);
    assert!(s.execute("riseredit", &json!({"handle": r["handle"], "removeLine": 9})).is_err());
    assert!(s.execute("riseredit", &json!({"handle": r["handle"], "line": {"index": 9}})).is_err());
    assert!(s.execute("elecriser", &json!({"at": [0, 0], "width": 600, "depth": 600, "lines": [{"usage": "water"}]})).is_err());
    assert!(
        s.execute(
            "elecriser",
            &json!({"at": [0, 0], "width": 600, "depth": 600, "lines": (0..40).map(|_| json!({"circuit": "X"})).collect::<Vec<_>>()})
        )
        .is_err()
    );
}

#[test]
fn each_line_of_a_riser_of_cables_can_go_its_own_way_or_pass_through() {
    let mut s = metric();
    let r = s
        .execute("elecriser", &json!({"at": [0, 0], "width": 600, "depth": 600, "lines": [{"circuit": "A"}, {"circuit": "B"}, {"circuit": "C"}]}))
        .unwrap();
    let h = hd(&r["handle"]);
    let riser = |s: &Session| cadcraft_doc::any_riser_of(s.doc().unwrap().entity(h).unwrap()).unwrap();
    // The riser goes up; line 2 goes down, line 3 passes through going down.
    s.execute("riseredit", &json!({"handle": r["handle"], "line": {"index": 2, "way": "down"}})).unwrap();
    s.execute("riseredit", &json!({"handle": r["handle"], "line": {"index": 3, "way": "throughDown"}})).unwrap();
    let p = riser(&s);
    let up = p.up;
    let ways: Vec<(bool, bool)> = p.cable.lines.iter().map(|l| (l.flows_up(up), l.passes_through())).collect();
    assert_eq!(ways, [(up, false), (false, false), (false, true)]);
    // Back to the riser's way; a bad way is refused.
    s.execute("riseredit", &json!({"handle": r["handle"], "line": {"index": 2, "way": "auto"}})).unwrap();
    assert_eq!(riser(&s).cable.lines[1].way, None);
    assert!(s.execute("riseredit", &json!({"handle": r["handle"], "line": {"index": 2, "way": "sideways"}})).is_err());
    // The riser's own direction does not touch the lines' ways.
    s.execute("riseredit", &json!({"handle": r["handle"], "direction": "down", "load": 3})).unwrap();
    assert_eq!(riser(&s).cable.lines[0].way, None);
}
