use cadcraft_doc::{Drawing, control_of};
use cadcraft_engine::Session;
use serde_json::{Value, json};

fn metric() -> Session {
    let mut s = Session::new();
    *s.doc_mut().unwrap() = Drawing::new_metric();
    s
}

fn has(r: &Value, text: &str) -> bool {
    r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains(text))
}

#[test]
fn the_points_the_load_of_the_controllers_and_what_is_wrong_are_read_from_the_drawing() {
    let mut s = metric();
    s.execute("equipmentadd", &json!({"at": [20000, 0], "kind": "cassette", "name": "UI1"})).unwrap();
    s.execute(
        "controladd",
        &json!({"at": [0, 0], "kind": "controller", "name": "AU1", "ai": 2, "ao": 1, "di": 1, "do": 1, "circuit": "TD1-R1", "bus": "BUS1"}),
    )
    .unwrap();
    // Three room sensors on a controller that offers two analogue inputs, a thermostat on a controller that is not drawn, a sensor wired to nothing.
    s.execute("controladd", &json!({"points": [[1000, 0], [2000, 0], [3000, 0]], "kind": "roomTemperature", "controller": "AU1"})).unwrap();
    s.execute("controladd", &json!({"at": [4000, 0], "kind": "thermostat", "controller": "AU9", "served": "UI1"})).unwrap();
    s.execute("controladd", &json!({"at": [5000, 0], "kind": "co2"})).unwrap();
    s.execute("controladd", &json!({"at": [6000, 0], "kind": "valveActuator", "controller": "AU1", "served": "CTA7"})).unwrap();
    let r = s.execute("controlcheck", &json!({})).unwrap();
    assert!(has(&r, "AU1: 3 AI points are wired to it and it offers 2"), "{r}");
    assert!(has(&r, "the controller AU9 is not drawn"), "{r}");
    assert!(has(&r, "not wired to a controller"), "{r}");
    assert!(has(&r, "serves CTA7, which is not drawn"), "{r}");
    assert!(has(&r, "bus BUS1: no bus cable"), "{r}");
    assert!(!has(&r, "UI1"), "a device that serves a drawn equipment is right: {r}");
    // 3 temperature (3 AI) + valve (1 AO) are wired to AU1.
    let au = r["controllers"].as_array().unwrap().iter().find(|c| c["name"] == "AU1").unwrap();
    assert_eq!((au["used"]["ai"].as_u64(), au["used"]["ao"].as_u64()), (Some(3), Some(1)));
    assert_eq!(au["reservePercent"][1].as_f64(), Some(0.0));
    // The list of points: a thermostat gives two, a sensor one, a valve one.
    assert_eq!(r["points"].as_array().unwrap().len(), 3 + 2 + 1 + 1, "{r}");
    // Fixing it: a bus cable on a circuit of that name, a bigger controller, wired devices.
    s.execute("cableadd", &json!({"points": [[0, 500], [9000, 500]], "circuit": "BUS1", "usage": "bus"})).unwrap();
    let au1 = s.doc().unwrap().model.iter().find(|e| control_of(e).is_some_and(|c| c.name == "AU1")).unwrap().handle;
    s.execute("controledit", &json!({"handle": au1.hex(), "ai": 8})).unwrap();
    let r = s.execute("controlcheck", &json!({})).unwrap();
    assert!(!has(&r, "AU1:") && !has(&r, "bus BUS1"), "{r}");
    // A data or bus circuit carries no power and is not blamed for it.
    let c = s.execute("circuitcheck", &json!({})).unwrap();
    assert!(!c["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("BUS1")), "{c}");
}

#[test]
fn a_module_extends_a_controller_and_hostile_values_are_refused() {
    let mut s = metric();
    s.execute("controladd", &json!({"at": [0, 0], "kind": "ioModule", "name": "MX1"})).unwrap();
    let r = s.execute("controlcheck", &json!({})).unwrap();
    assert!(has(&r, "MX1: a module extends a controller"), "{r}");
    assert!(s.execute("controladd", &json!({"at": [0, 0], "kind": "toaster"})).is_err());
    assert!(s.execute("controladd", &json!({"at": [0, 0], "kind": "controller", "ai": -2})).is_err());
    assert!(s.execute("controladd", &json!({"at": [0, 0], "kind": "controller", "ai": 5000})).is_err());
    assert!(s.execute("controladd", &json!({"at": [0, 0], "setpoint": "warm"})).is_err());
    assert!(s.execute("controltakeoff", &json!({"prices": {"Control|x": -1}})).is_err());
}

#[test]
fn the_points_list_is_written_with_the_prices_and_the_device_is_placed_from_the_command_line() {
    let mut s = metric();
    s.execute("spaceadd", &json!({"points": [[0, 0], [5000, 0], [5000, 5000], [0, 5000]], "name": "Bureau 1", "flow": 100})).unwrap();
    s.execute("controladd", &json!({"at": [100, 100], "kind": "controller", "name": "AU1", "circuit": "TD1-R1"})).unwrap();
    for t in ["controladd", "ty", "thermostat", "c", "AU1", "2000,2000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none(), "{}", s.prompt_text());
    let path = std::env::temp_dir().join(format!("cadcraft-control-{}.xlsx", std::process::id()));
    let t = s
        .execute("controltakeoff", &json!({"level": "RDC", "prices": {"Control|Room thermostat": 85.0}, "path": path.display().to_string()}))
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(bytes.starts_with(b"PK"), "an xlsx");
    assert_eq!(t["points"].as_array().unwrap().len(), 2);
    assert_eq!(t["points"][0]["space"], "Bureau 1", "the point says the room it is in: {t}");
    assert_eq!(t["costing"]["total"].as_f64(), Some(85.0));
    let csv = std::env::temp_dir().join(format!("cadcraft-control-{}.csv", std::process::id()));
    s.execute("controltakeoff", &json!({"path": csv.display().to_string()})).unwrap();
    let text = std::fs::read_to_string(&csv).unwrap();
    let _ = std::fs::remove_file(&csv);
    assert!(text.contains("Controller;Device;Type") && text.contains("Setpoint"), "{text}");
}
