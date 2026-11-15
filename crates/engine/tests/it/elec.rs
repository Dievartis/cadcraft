use cadcraft_doc::{Drawing, Handle, cable_of, device_of};
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

fn circuit<'a>(r: &'a Value, name: &str) -> &'a Value {
    r["circuits"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no circuit {name} in {r}"))
}

#[test]
fn a_circuit_is_drawn_checked_sized_and_renamed_through_the_commands() {
    let mut s = metric();
    let at: Vec<Value> = (0..6).map(|i| json!([i * 2000, 500])).collect();
    s.execute("elecdeviceadd", &json!({"points": at, "kind": "socket", "circuit": "TD1-C1"})).unwrap();
    s.execute("elecdeviceadd", &json!({"at": [-1500, 0], "kind": "distributionBoard", "name": "TD1", "supply": 40})).unwrap();
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [12000, 0], [12000, 20000]], "circuit": "TD1-C1", "usage": "power"})).unwrap();
    assert_eq!(r["handles"].as_array().unwrap().len(), 2);
    // Six sockets of 500 W, 32 m: the circuit is read, the section is to be sized.
    let check = s.execute("circuitcheck", &json!({})).unwrap();
    let c = circuit(&check, "TD1-C1");
    assert_eq!((c["devices"].as_u64(), c["powerW"].as_f64(), c["panelDrawn"].as_bool()), (Some(6), Some(3000.0), Some(true)));
    assert_eq!(c["cableLengthM"].as_f64(), Some(32.0));
    assert!(c["requiredSectionMm2"].as_f64().is_some(), "{c}");
    // A section that is too small is told; sizing puts the right one and the circuit is then right.
    s.execute("circuitedit", &json!({"name": "td1-c1", "section": 1.5, "protection": 25})).unwrap();
    let bad = s.execute("circuitcheck", &json!({})).unwrap();
    assert_eq!(bad["problems"], 1, "{bad}");
    let sized = s.execute("circuitsize", &json!({"overwrite": true})).unwrap();
    assert_eq!(sized["problems"], 0, "{sized}");
    let first = hd(&r["handles"][0]);
    let cable = cable_of(s.doc().unwrap().entity(first).unwrap()).unwrap();
    assert_eq!(cable.section_mm2, circuit(&sized, "TD1-C1")["requiredSectionMm2"].as_f64());
    // Renaming moves the cables, the devices and the colour.
    s.execute("circuitedit", &json!({"name": "TD1-C1", "rename": "TD1-C7"})).unwrap();
    let after = s.execute("circuitcheck", &json!({})).unwrap();
    assert_eq!(after["circuits"].as_array().unwrap().len(), 1);
    assert_eq!(circuit(&after, "TD1-C7")["devices"], 6);
    // Hostile values are refused, not turned into numbers.
    assert!(s.execute("elecdeviceadd", &json!({"at": [0, 0], "power": "lots"})).is_err());
    assert!(s.execute("cableadd", &json!({"points": [[0, 0], [10, 0]], "section": 0})).is_err());
    assert!(s.execute("circuitcheck", &json!({"diversity": 2})).is_err());
    assert!(s.execute("elecedit", &json!({"handle": first.hex(), "cosPhi": 7})).is_err());
}

#[test]
fn equipment_and_units_are_fed_by_a_circuit_and_the_quantities_are_priced_and_written() {
    let mut s = metric();
    s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "vrfOutdoor", "name": "GE1"})).unwrap();
    let eq = s.doc().unwrap().model.iter().find(|e| cadcraft_doc::equipment_of(e).is_some()).unwrap().handle;
    s.execute("equipmentedit", &json!({"handle": eq.hex(), "electricKw": 14, "circuit": "TD1-V1"})).unwrap();
    s.execute("cableadd", &json!({"points": [[0, 3000], [20000, 3000]], "circuit": "TD1-V1", "phases": "three", "section": 6})).unwrap();
    let r = s.execute("circuitcheck", &json!({})).unwrap();
    let c = circuit(&r, "TD1-V1");
    assert_eq!((c["hvac"].as_u64(), c["powerW"].as_f64()), (Some(1), Some(14_000.0)));
    // The same through the selection.
    s.execute("elecdeviceadd", &json!({"at": [100, 100], "kind": "light"})).unwrap();
    let l = s.doc().unwrap().model.iter().find(|e| device_of(e).is_some()).unwrap().handle;
    s.execute("circuitassign", &json!({"handles": [l.hex()], "circuit": "TD1-E1"})).unwrap();
    assert_eq!(device_of(s.doc().unwrap().entity(l).unwrap()).unwrap().circuit, "TD1-E1");
    let path = std::env::temp_dir().join(format!("cadcraft-elec-{}.xlsx", std::process::id()));
    let t = s
        .execute(
            "electakeoff",
            &json!({"level": "RDC", "prices": {"Cable|3G6": 0.0, "Device|Ceiling light": 45.0}, "path": path.display().to_string()}),
        )
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(bytes.starts_with(b"PK"), "an xlsx");
    assert_eq!(t["costing"]["total"].as_f64(), Some(45.0), "{t}");
    assert_eq!(t["hvac"].as_array().unwrap().len(), 1);
    assert!(s.execute("electakeoff", &json!({"prices": {"Cable|x": -1}})).is_err());
}

#[test]
fn the_lighting_of_a_room_is_checked_against_the_code_by_its_class() {
    let mut s = metric();
    s.execute("spaceadd", &json!({"points": [[0, 0], [6000, 0], [6000, 5000], [0, 5000]], "name": "Bureau", "flow": 100})).unwrap();
    s.execute("spaceadd", &json!({"points": [[10000, 0], [20000, 0], [20000, 1500], [10000, 1500]], "name": "Couloir", "flow": 100})).unwrap();
    let r = s.execute("lightingcheck", &json!({})).unwrap();
    assert_eq!(r["problems"], 2, "no luminaire: {r}");
    // Four panels of 3400 lm in 30 m²: 4 x 3400 x 0.5 x 0.8 / 30 = 181 lux, above 120. One of 700 lm in the corridor (15 m²): 12 lux, below 40.
    for at in [[1000, 1000], [5000, 1000], [1000, 4000], [5000, 4000]] {
        s.execute("elecdeviceadd", &json!({"at": at, "kind": "ceilingLight", "flux": 3400})).unwrap();
    }
    s.execute("elecdeviceadd", &json!({"at": [15000, 700], "kind": "downlight", "flux": 700})).unwrap();
    let r = s.execute("lightingcheck", &json!({})).unwrap();
    let find = |n: &str| r["spaces"].as_array().unwrap().iter().find(|x| x["space"] == n).unwrap().clone();
    assert_eq!((find("Bureau")["lux"].as_f64(), find("Bureau")["ok"].as_bool()), (Some(181.0), Some(true)));
    assert_eq!(
        (find("Couloir")["class"].as_str(), find("Couloir")["minimumLux"].as_f64(), find("Couloir")["ok"].as_bool()),
        (Some("circulation"), Some(40.0), Some(false))
    );
    assert!(s.execute("lightingcheck", &json!({"utilisation": 3})).is_err());
}

#[test]
fn the_cable_and_the_device_are_drawn_from_the_command_line_and_their_grips_work() {
    let mut s = metric();
    for t in ["elecdeviceadd", "ty", "downlight", "c", "TD3-E2", "1000,1000", "2000,1000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none(), "{}", s.prompt_text());
    let lights: Vec<_> = s.doc().unwrap().model.iter().filter_map(|e| device_of(e)).collect();
    assert_eq!((lights.len(), lights[0].circuit.as_str(), lights[0].kind.name()), (2, "TD3-E2", "Downlight"));
    for t in ["cableadd", "c", "TD3-E2", "u", "lighting", "0,0", "4000,0", ""] {
        s.cmdline(t).unwrap();
    }
    let cable = s.doc().unwrap().model.iter().find(|e| cable_of(e).is_some()).unwrap().handle;
    s.set_selection(vec![cable]);
    assert_eq!(s.grips_of(cable).unwrap().len(), 3);
    s.start_grip_edit(&[(cable, 2)]).unwrap();
    s.cmdline("5000,0").unwrap();
    let (_, a, b) = s.doc().unwrap().cable_ends(cable).unwrap();
    assert_eq!((a.x.round(), b.x.round()), (0.0, 5000.0));
}
