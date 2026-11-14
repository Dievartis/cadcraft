//! PIPEPARTADD puts a library part on the drawing (turned along the pipe under it); PIPEPARTEDIT changes it;
//! PIPEPARTMODELS lists the library.

use cadcraft_doc::{Drawing, EntityKind, Handle, PartKind, PartParams, part_of};
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

fn part(s: &Session, h: Handle) -> PartParams {
    part_of(s.doc().unwrap().entity(h).unwrap()).expect("a part")
}

fn rotation(s: &Session, h: Handle) -> f64 {
    let EntityKind::Insert(i) = &s.doc().unwrap().entity(h).unwrap().kind else { panic!("a block reference") };
    i.rotation
}

#[test]
fn pipepartadd_puts_a_part_on_a_pipe_turned_along_it_with_a_tag_of_its_own() {
    let mut s = metric();
    s.execute("pipeadd", &json!({"points": [[0, 0], [0, 5000]], "service": "chilledWater"})).unwrap();
    // Clicked 40 mm off the pipe: put on its axis and turned along it (a quarter turn).
    let r = s.execute("pipepartadd", &json!({"at": [40, 2000], "model": "Balancing valve (TA)"})).unwrap();
    let h = hd(&r["handle"]);
    assert_eq!((r["name"].as_str(), r["model"].as_str()), (Some("V1"), Some("Balancing valve (TA)")));
    assert!((rotation(&s, h) - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    let EntityKind::Insert(i) = &s.doc().unwrap().entity(h).unwrap().kind else { panic!() };
    assert!(i.insert.x.abs() < 1e-9 && (i.insert.y - 2000.0).abs() < 1e-9, "{:?}", i.insert);
    // Away from any pipe: not turned; the tags count on; the French name works; values typed over the model's.
    let r = s.execute("pipepartadd", &json!({"at": [5000, 5000], "model": "Vanne papillon", "dn": 100, "name": "VP1"})).unwrap();
    let p = part(&s, hd(&r["handle"]));
    assert_eq!((p.name.as_str(), p.kind, p.dn_mm), ("VP1", PartKind::ButterflyValve, Some(100.0)));
    assert_eq!(rotation(&s, hd(&r["handle"])), 0.0);
    let r = s.execute("pipepartadd", &json!({"at": [6000, 5000], "model": "Isolation valve (ball)", "rotation": 90})).unwrap();
    assert!((rotation(&s, hd(&r["handle"])) - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert_eq!(r["name"], "V3");
    let r = s.execute("pipepartadd", &json!({"at": [0, 9000], "model": "Circulator (single)"})).unwrap();
    assert_eq!(r["name"], "P1", "a prefix of its own");
    // A box with its outlets.
    let r = s.execute("pipepartadd", &json!({"at": [3000, 9000], "model": "BS8A14AJV1B9"})).unwrap();
    let b = part(&s, hd(&r["handle"]));
    assert_eq!((b.outlets, b.kind, b.brand.as_str()), (8, PartKind::BranchBox, "Daikin"));
    for bad in [
        json!({"model": "Isolation valve (ball)"}),
        json!({"at": [0, 0]}),
        json!({"at": [0, 0], "model": "nothing"}),
        json!({"at": [0, 0], "model": "Pressure gauge", "dn": -1}),
        json!({"at": [0, 0], "kind": "gizmo"}),
    ] {
        assert!(s.execute("pipepartadd", &bad).is_err(), "{bad}");
    }
}

#[test]
fn pipepartedit_changes_a_part_and_pipepartmodels_lists_the_library() {
    let mut s = metric();
    let r = s.execute("pipepartadd", &json!({"at": [0, 0], "model": "Expansion vessel 80 l"})).unwrap();
    let h = hd(&r["handle"]);
    s.execute("pipepartedit", &json!({"handle": h.hex(), "volume": 150, "name": "VE9", "power": 3})).unwrap();
    let p = part(&s, h);
    assert_eq!((p.name.as_str(), p.volume_l, p.power_kw), ("VE9", Some(150.0), Some(3.0)));
    s.execute("pipepartedit", &json!({"handle": h.hex(), "power": null})).unwrap();
    assert_eq!(part(&s, h).power_kw, None);
    assert!(s.execute("pipepartedit", &json!({"handle": h.hex(), "volume": -2})).is_err());
    assert!(s.execute("pipepartedit", &json!({"handle": "0"})).is_err());
    let all = s.execute("pipepartmodels", &json!({})).unwrap();
    assert!(all["models"].as_array().unwrap().len() >= 80);
    assert_eq!(all["categories"].as_array().unwrap().len(), 9);
    let bs = s.execute("pipepartmodels", &json!({"category": "branchBoxes", "q": "daikin 12"})).unwrap();
    let names: Vec<&str> = bs["models"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"BS12A14AJV1B9") && names.contains(&"BS12Q14AV1B"), "{names:?}");
    assert!(s.execute("pipepartmodels", &json!({"category": "gizmos"})).is_err());
}

#[test]
fn pipepartadd_at_the_command_line_places_parts_one_after_another() {
    let mut s = metric();
    for t in ["pipepartadd", "m", "Pressure gauge", "1000,1000", "2000,1000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let parts: Vec<PartParams> = s.doc().unwrap().model.iter().filter_map(|e| part_of(e)).collect();
    assert_eq!(parts.len(), 2);
    assert!(parts.iter().all(|p| p.kind == PartKind::PressureGauge));
    // The next PIPEPARTADD starts with the model last used.
    for t in ["pipepartadd", "3000,1000", ""] {
        s.cmdline(t).unwrap();
    }
    assert_eq!(s.doc().unwrap().model.iter().filter_map(|e| part_of(e)).filter(|p| p.kind == PartKind::PressureGauge).count(), 3);
}
