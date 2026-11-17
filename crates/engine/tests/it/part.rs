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
    let r = s.execute("pipepartadd", &json!({"at": [3000, 9000], "model": "Branch box, 8 outlets (R32 heat recovery)"})).unwrap();
    let b = part(&s, hd(&r["handle"]));
    assert_eq!((b.outlets, b.kind, b.brand.as_str()), (8, PartKind::BranchBox, ""));
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
    let bs = s.execute("pipepartmodels", &json!({"category": "branchBoxes", "q": "12 outlets"})).unwrap();
    let names: Vec<&str> = bs["models"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect();
    assert!(
        names.contains(&"Branch box, 12 outlets (R32 heat recovery)") && names.contains(&"Branch box, 12 outlets (R410A heat recovery)"),
        "{names:?}"
    );
    assert!(s.execute("pipepartmodels", &json!({"category": "gizmos"})).is_err());
}

#[test]
fn pipepartadd_at_the_command_line_places_parts_one_after_another() {
    let mut s = metric();
    // A click, then the orientation (Enter: not turned).
    for t in ["pipepartadd", "m", "Pressure gauge", "1000,1000", "", "2000,1000", "", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let parts: Vec<PartParams> = s.doc().unwrap().model.iter().filter_map(|e| part_of(e)).collect();
    assert_eq!(parts.len(), 2);
    assert!(parts.iter().all(|p| p.kind == PartKind::PressureGauge));
    // The next PIPEPARTADD starts with the model last used.
    for t in ["pipepartadd", "3000,1000", "", ""] {
        s.cmdline(t).unwrap();
    }
    assert_eq!(s.doc().unwrap().model.iter().filter_map(|e| part_of(e)).filter(|p| p.kind == PartKind::PressureGauge).count(), 3);
}

#[test]
fn the_orientation_of_a_part_is_asked_after_the_click_as_it_is_for_a_unit() {
    let rot = |s: &Session| -> Vec<f64> {
        let mut v: Vec<f64> = s
            .doc()
            .unwrap()
            .model
            .iter()
            .filter(|e| part_of(e).is_some())
            .filter_map(|e| if let EntityKind::Insert(i) = &e.kind { Some(i.rotation) } else { None })
            .collect();
        v.sort_by(f64::total_cmp);
        v
    };
    let mut s = metric();
    // A header: the click, then a second click gives the direction (a quarter turn up).
    for t in ["pipepartadd", "m", "Refrigerant header, index below 290", "1000,1000", "1000,2000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!((rot(&s)[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-9, "{:?}", rot(&s));
    // A branch box: an angle typed in degrees.
    for t in ["pipepartadd", "m", "Branch box, 4 outlets (R32 heat recovery)", "5000,1000", "180", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(rot(&s).iter().any(|r| (r - std::f64::consts::PI).abs() < 1e-9), "{:?}", rot(&s));
    // A Y joint on a pipe: Enter takes the direction of the pipe (here a quarter turn), clicked off a pipe it is not turned.
    s.execute("pipeadd", &json!({"points": [[0, 5000], [0, 9000]], "service": "refrigerantPair"})).unwrap();
    for t in ["pipepartadd", "m", "Refrigerant Y joint, index below 200", "20,7000", "", "9000,9000", ""] {
        s.cmdline(t).unwrap();
    }
    let r = rot(&s);
    assert_eq!(r.len(), 4);
    assert!(r.iter().filter(|x| (**x - std::f64::consts::FRAC_PI_2).abs() < 1e-9).count() >= 2, "{r:?}");
    assert!(r.iter().any(|x| x.abs() < 1e-9), "{r:?}");
    // A valve clicked on a pipe turns along it and cuts it without asking; a refused angle is asked again.
    s.execute("pipeadd", &json!({"points": [[3000, 0], [3000, 3000]], "service": "chilledWater"})).unwrap();
    let before = s.doc().unwrap().model.iter().filter(|e| cadcraft_doc::pipe_of(e).is_some()).count();
    for t in ["pipepartadd", "m", "Isolation valve (ball)", "3000,1500", ""] {
        s.cmdline(t).unwrap();
    }
    assert_eq!(s.doc().unwrap().model.iter().filter(|e| cadcraft_doc::pipe_of(e).is_some()).count(), before + 1, "the pipe is cut");
    for t in ["pipepartadd", "m", "Pressure gauge", "8000,8000", "abc", "45", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(rot(&s).iter().any(|r| (r - std::f64::consts::FRAC_PI_4).abs() < 1e-9), "{:?}", rot(&s));
}

#[test]
fn the_outlets_of_a_box_change_it_on_the_drawing_and_another_model_can_be_taken() {
    let mut s = metric();
    let r = s.execute("pipepartadd", &json!({"at": [0, 0], "model": "Branch box, 4 outlets (R32 heat recovery)"})).unwrap();
    let h = hd(&r["handle"]);
    let before = part(&s, h);
    assert_eq!(before.outlets, 4);
    // A palette field sends 8.0: whole numbers are accepted, and the box grows with its outlets.
    s.execute("pipepartedit", &json!({"handle": h.hex(), "outlets": 8.0})).unwrap();
    let after = part(&s, h);
    assert_eq!(after.outlets, 8);
    assert!(after.length_mm > before.length_mm, "{} {}", after.length_mm, before.length_mm);
    // A length given with it is kept.
    s.execute("pipepartedit", &json!({"handle": h.hex(), "outlets": 6, "length": 1111})).unwrap();
    assert_eq!(part(&s, h).length_mm, 1111.0);
    assert!(s.execute("pipepartedit", &json!({"handle": h.hex(), "outlets": 2.5})).is_err());
    // The height is typed like the other sizes, and cleared.
    s.execute("pipepartedit", &json!({"handle": h.hex(), "height": 410, "width": 380})).unwrap();
    assert_eq!((part(&s, h).height_mm, part(&s, h).width_mm), (Some(410.0), 380.0));
    s.execute("pipepartedit", &json!({"handle": h.hex(), "height": null})).unwrap();
    assert_eq!(part(&s, h).height_mm, None);
    // Another tank of the library: its sizes and volume, the tag kept.
    let t = s.execute("pipepartadd", &json!({"at": [5000, 0], "model": "Buffer tank 300 l", "name": "BT1"})).unwrap();
    let th = hd(&t["handle"]);
    s.execute("pipepartedit", &json!({"handle": th.hex(), "model": "Buffer tank 1000 l"})).unwrap();
    let p = part(&s, th);
    assert_eq!((p.name.as_str(), p.model.as_str(), p.volume_l, p.height_mm), ("BT1", "Buffer tank 1000 l", Some(1000.0), Some(1900.0)));
    // The safety valve sits on the red layer.
    let v = s.execute("pipepartadd", &json!({"at": [0, 3000], "model": "Safety valve"})).unwrap();
    assert_eq!(s.doc().unwrap().entity(hd(&v["handle"])).unwrap().common.layer, "M-PIPE-SAFETY");
}

#[test]
fn a_valve_put_on_a_pipe_cuts_it_but_a_vessel_or_a_sensor_does_not() {
    let mut s = metric();
    let r = s.execute("pipeadd", &json!({"points": [[0, 0], [6000, 0]], "service": "hotWater", "load": 25, "source": true})).unwrap();
    let first = hd(&r["handles"][0]);
    let pipes = |s: &Session| s.doc().unwrap().model.iter().filter(|e| cadcraft_doc::pipe_of(e).is_some()).count();
    assert_eq!(pipes(&s), 1);
    s.execute("pipepartadd", &json!({"at": [3000, 30], "model": "Balancing valve (TA)"})).unwrap();
    assert_eq!(pipes(&s), 2, "the pipe is cut for the valve");
    let (a, b): (Vec<_>, Vec<_>) = {
        let d = s.doc().unwrap();
        let ends: Vec<_> = d.model.iter().filter_map(|e| d.pipe_ends(e.handle)).collect();
        (ends.iter().map(|x| x.1.x).collect(), ends.iter().map(|x| x.2.x).collect())
    };
    assert!(b.iter().any(|x| (x - 2925.0).abs() < 1e-6) && a.iter().any(|x| (x - 3075.0).abs() < 1e-6), "{a:?} {b:?}");
    assert!(cadcraft_doc::pipe_of(s.doc().unwrap().entity(first).unwrap()).unwrap().source, "the source stays on the first run");
    // The network reads through the valve.
    let sized = s.execute("pipesize", &json!({})).unwrap();
    assert_eq!(sized["networks"].as_array().unwrap().len(), 1);
    assert_eq!(sized["networks"][0]["runs"].as_array().unwrap().len(), 2);
    // A sensor, a vessel and an air vent hang on the pipe: it is not cut. Nor is a valve put where it does not fit.
    for model in ["Temperature sensor", "Expansion vessel 24 l", "Automatic air vent"] {
        s.execute("pipepartadd", &json!({"at": [1000, 0], "model": model})).unwrap();
    }
    assert_eq!(pipes(&s), 2);
    s.execute("pipepartadd", &json!({"at": [20, 0], "model": "Isolation valve (ball)"})).unwrap();
    assert_eq!(pipes(&s), 2, "too near the end of the pipe for its gap");
}

#[test]
fn a_part_kept_as_a_model_is_offered_again_and_can_be_deleted() {
    let mut s = metric();
    let r = s.execute("pipepartadd", &json!({"at": [0, 0], "model": "Balancing valve (TA)", "dn": 65})).unwrap();
    let h = hd(&r["handle"]);
    s.execute("pipepartedit", &json!({"handle": h.hex(), "kvs": 38, "height": 190, "length": 180})).unwrap();
    // Kept as a model of the drawing's own.
    let saved = s.execute("pipepartmodels", &json!({"save": h.hex(), "name": "Vanne TA DN65 maison"})).unwrap();
    assert_eq!(saved["saved"], "Vanne TA DN65 maison");
    let listed = s.execute("pipepartmodels", &json!({"q": "maison"})).unwrap();
    let models = listed["models"].as_array().unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(
        (models[0]["user"].as_bool(), models[0]["category"].as_str(), models[0]["height"].as_f64()),
        (Some(true), Some("valves"), Some(190.0))
    );
    // Offered first, and a part made from it has its values and its tag.
    let again = s.execute("pipepartadd", &json!({"at": [1000, 0], "model": "vanne ta dn65 MAISON"})).unwrap();
    let p = part(&s, hd(&again["handle"]));
    assert_eq!((p.model.as_str(), p.kvs, p.dn_mm, p.length_mm), ("Vanne TA DN65 maison", Some(38.0), Some(65.0), 180.0));
    assert_eq!(again["name"], "V2");
    // Another part can take it as its model (its tag stays).
    let other = s.execute("pipepartadd", &json!({"at": [2000, 0], "model": "Check valve", "name": "CV9"})).unwrap();
    s.execute("pipepartedit", &json!({"handle": other["handle"], "model": "Vanne TA DN65 maison"})).unwrap();
    let q = part(&s, hd(&other["handle"]));
    assert_eq!((q.name.as_str(), q.kvs), ("CV9", Some(38.0)));
    // Kept again under the same name: replaced, not doubled. Another category when asked.
    s.execute("pipepartmodels", &json!({"save": h.hex(), "name": "vanne ta dn65 maison", "category": "fittings"})).unwrap();
    let listed = s.execute("pipepartmodels", &json!({"category": "fittings", "q": "maison"})).unwrap();
    assert_eq!(listed["models"].as_array().unwrap().len(), 1);
    // The model of the drawing's own can be deleted; a standard one cannot; what is wrong is refused.
    assert!(s.execute("pipepartmodels", &json!({"delete": "Check valve"})).is_err());
    assert_eq!(s.execute("pipepartmodels", &json!({"delete": "Vanne TA DN65 maison"})).unwrap()["deleted"], "Vanne TA DN65 maison");
    assert!(s.execute("pipepartadd", &json!({"at": [0, 0], "model": "Vanne TA DN65 maison"})).is_err());
    for bad in [
        json!({"save": h.hex()}),
        json!({"save": h.hex(), "name": ""}),
        json!({"save": "0", "name": "x"}),
        json!({"save": h.hex(), "name": "x", "category": "gizmos"}),
    ] {
        assert!(s.execute("pipepartmodels", &bad).is_err(), "{bad}");
    }
}

#[test]
fn refrigerantcheck_counts_the_units_at_each_outlet_of_a_box() {
    let mut s = metric();
    s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "vrfOutdoor", "capacityKw": 8})).unwrap();
    let b = s.execute("pipepartadd", &json!({"at": [5000, 0], "model": "Branch box, 4 outlets (R32 heat recovery)", "rotation": 0})).unwrap();
    let bx = part(&s, hd(&b["handle"]));
    let port = |outlet: Option<usize>, nth: usize| {
        let q: Vec<_> = bx.ports().into_iter().filter(|x| x.outlet == outlet).collect();
        (5000.0 + q[nth].at.x, q[nth].at.y)
    };
    let inlet = port(None, 1);
    s.execute("pipeadd", &json!({"points": [[500, 0], [inlet.0, inlet.1]], "service": "refrigerantPair"})).unwrap();
    for i in [0, 3] {
        let o = port(Some(i), 0);
        s.execute("pipeadd", &json!({"points": [[o.0, o.1], [o.0, 3000]], "service": "refrigerantPair"})).unwrap();
        s.execute("equipmentadd", &json!({"at": [o.0, 3100], "kind": "cassette", "capacityKw": 2.5, "length": 200, "width": 200, "height": 200}))
            .unwrap();
    }
    let r = s.execute("refrigerantcheck", &json!({})).unwrap();
    let n = &r["networks"][0];
    assert_eq!(n["units"], 2, "{r}");
    let parts = n["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!((parts[0]["kind"].as_str(), parts[0]["outlets"].as_array().unwrap().len()), (Some("branchBox"), 2));
    assert_eq!((parts[0]["outlets"][0]["outlet"].as_u64(), parts[0]["outlets"][1]["outlet"].as_u64()), (Some(1), Some(4)));
    assert!(r["unconnected"].as_array().unwrap().is_empty());
    // Nothing to check without an outdoor unit.
    let empty = metric().execute("refrigerantcheck", &json!({})).unwrap();
    assert!(empty["networks"].as_array().unwrap().is_empty());
}

#[test]
fn a_branch_box_is_chosen_by_its_number_of_outlets_not_by_typing_a_model_name() {
    let mut s = metric();
    s.last_used.part.model = "Branch box, 6 outlets (R32 heat recovery)".into();
    s.cmdline("pipepartadd").unwrap();
    s.cmdline("Model").unwrap();
    let prompt = s.current_prompt().map(|p| p.message).unwrap_or_default();
    assert!(prompt.contains("number of outlets") && prompt.contains("12"), "{prompt}");
    // A number picks the box of that size in the same refrigerant.
    s.cmdline("8").unwrap();
    s.cmdline("0,0").unwrap();
    s.cmdline("").unwrap();
    let placed = s.doc().unwrap().model.iter().filter_map(|e| part_of(e)).map(|p| p.model).collect::<Vec<_>>();
    assert_eq!(placed, vec!["Branch box, 8 outlets (R32 heat recovery)".to_string()], "{:?}", s.log.iter().rev().take(4).collect::<Vec<_>>());
    // A number nobody makes is refused, and the model stays.
    s.cmdline("pipepartadd").unwrap();
    s.cmdline("Model").unwrap();
    s.cmdline("7").unwrap();
    assert!(s.log.iter().any(|l| l.contains("No branch box with that number of outlets")), "{:?}", s.log.iter().rev().take(4).collect::<Vec<_>>());
}
