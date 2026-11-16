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

#[test]
fn addresses_are_written_beside_the_objects_hidden_listed_and_two_on_one_bus_are_told() {
    let mut s = metric();
    s.execute("equipmentadd", &json!({"at": [10000, 0], "kind": "cassette", "name": "UI1", "address": "1-05"})).unwrap();
    s.execute("controladd", &json!({"at": [0, 0], "kind": "controller", "name": "AU1", "bus": "BUS1", "address": "1", "circuit": "TD1-R1"})).unwrap();
    s.execute("controladd", &json!({"at": [1000, 0], "kind": "thermostat", "controller": "AU1", "bus": "BUS1", "address": "12"})).unwrap();
    s.execute("controladd", &json!({"at": [2000, 0], "kind": "roomTemperature", "controller": "AU1", "bus": "BUS1", "address": "12"})).unwrap();
    s.execute("elecdeviceadd", &json!({"at": [3000, 0], "kind": "downlight", "address": "A3"})).unwrap();
    let texts = |s: &Session| -> Vec<String> {
        let d = s.doc().unwrap();
        let mut out = Vec::new();
        for e in d.model.iter() {
            if let cadcraft_doc::EntityKind::Insert(i) = &e.kind
                && let Some(b) = d.block(&i.block)
            {
                out.extend(
                    b.entities.iter().filter_map(|x| if let cadcraft_doc::EntityKind::Text(t) = &x.kind { Some(t.value.clone()) } else { None }),
                );
            }
        }
        out
    };
    // Every kind of addressed object writes it beside itself.
    let t = texts(&s);
    for want in ["@1-05", "@12", "@A3"] {
        assert!(t.iter().any(|x| x.contains(want)), "{want}: {t:?}");
    }
    let r = s.execute("addressdisplay", &json!({"list": true})).unwrap();
    assert_eq!(r["addresses"].as_array().unwrap().len(), 5, "{r}");
    assert_eq!(r["clashes"].as_array().unwrap().len(), 1, "two devices have 12 on BUS1: {r}");
    // Hidden, then shown again.
    let h = s.execute("addressdisplay", &json!({"show": false})).unwrap();
    assert_eq!(h["changed"], 5);
    assert!(!texts(&s).iter().any(|x| x.contains('@')), "{:?}", texts(&s));
    assert_eq!(s.execute("addressdisplay", &json!({"show": true})).unwrap()["changed"], 5);
    assert!(texts(&s).iter().any(|x| x.contains("@1-05")));
    // The clash is in the controls' warnings, and the table is in the workbook (and the address in the list of points).
    let c = s.execute("controlcheck", &json!({})).unwrap();
    assert!(c["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("have the address 12 on BUS1")), "{c}");
    assert!(c["points"].as_array().unwrap().iter().any(|p| p["address"] == "12"), "{c}");
    assert_eq!(c["addresses"].as_array().unwrap().len(), 5);
    assert!(s.execute("addressdisplay", &json!({"show": "yes"})).is_err());
    assert!(s.execute("controladd", &json!({"at": [0, 0], "address": "x\u{7}"})).is_err());
}

#[test]
fn the_controls_principle_diagram_is_drawn_and_follows_the_control_devices() {
    let mut s = metric();
    s.execute("controladd", &json!({"at": [0, 0], "kind": "controller", "name": "AUT1", "bus": "BUS1"})).unwrap();
    s.execute("controladd", &json!({"at": [0, 1000], "kind": "roomTemperature", "name": "TA1", "controller": "AUT1", "address": "3"})).unwrap();
    let r = s.execute("controlschema", &json!({"at": [0, -30000], "title": "GTB"})).unwrap();
    assert_eq!(r["controllers"], 1, "{r}");
    let h = cadcraft_doc::Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap();
    let texts = |s: &Session| -> Vec<String> {
        match &s.doc().unwrap().entity(h).unwrap().kind {
            cadcraft_doc::EntityKind::Insert(i) => s
                .doc()
                .unwrap()
                .block(&i.block)
                .unwrap()
                .entities
                .iter()
                .filter_map(|e| if let cadcraft_doc::EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
                .collect(),
            k => panic!("{k:?}"),
        }
    };
    assert!(texts(&s).iter().any(|x| x.starts_with("GTB")) && texts(&s).iter().any(|x| x.starts_with("@3  TA1")), "{:?}", texts(&s));
    // A device added later is in the diagram without asking.
    s.execute("controladd", &json!({"at": [500, 1000], "kind": "roomTemperature", "name": "TA2", "controller": "AUT1", "address": "9"})).unwrap();
    assert!(texts(&s).iter().any(|x| x.starts_with("@9  TA2")), "{:?}", texts(&s));
    // Bad input is refused; the command line puts a diagram at a click.
    let n = s.doc().unwrap().model.len();
    assert!(s.execute("controlschema", &json!({"title": "x"})).is_err());
    assert!(s.execute("controlschema", &json!({"at": [0, -50000], "labelHeight": 0})).is_err());
    assert_eq!(s.doc().unwrap().model.len(), n);
    s.cmdline("controlschema").unwrap();
    s.cmdline("0,-60000").unwrap();
    assert!(s.running.is_none());
    assert_eq!(s.doc().unwrap().model.len(), n + 1);
}

#[test]
fn a_bus_with_a_protocol_has_its_addresses_its_devices_and_its_cable_checked() {
    let mut s = metric();
    s.execute("controladd", &json!({"at": [0, 0], "kind": "controller", "name": "AUT1", "bus": "BUS1", "protocol": "Modbus"})).unwrap();
    s.execute("controladd", &json!({"at": [0, 1000], "kind": "roomTemperature", "name": "TA1", "controller": "AUT1", "address": "12"})).unwrap();
    s.execute("controladd", &json!({"at": [500, 1000], "kind": "roomTemperature", "name": "TA2", "controller": "AUT1", "address": "300"})).unwrap();
    s.execute("controladd", &json!({"at": [900, 1000], "kind": "roomTemperature", "name": "TA3", "controller": "AUT1", "address": "A5"})).unwrap();
    s.execute("cableadd", &json!({"points": [[0, 3000], [1500000, 3000]], "circuit": "BUS1", "usage": "bus"})).unwrap();
    let r = s.execute("controlcheck", &json!({})).unwrap();
    let w: Vec<&str> = r["warnings"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    // 300 and A5 are not Modbus addresses; 12 is.
    assert!(w.iter().any(|x| x.contains("TA2") && x.contains("Modbus address is 1 to 247")), "{w:?}");
    assert!(w.iter().any(|x| x.contains("TA3") && x.contains("Modbus address is 1 to 247")), "{w:?}");
    assert!(!w.iter().any(|x| x.contains("TA1") && x.contains("address")), "{w:?}");
    // 1500 m of cable on a bus that takes 1200 m.
    assert!(w.iter().any(|x| x.contains("bus BUS1 (Modbus RTU)") && x.contains("at most 1200 m")), "{w:?}");
    // A protocol that does not exist is refused, on the device and at its creation.
    assert!(s.execute("controladd", &json!({"at": [0, 5000], "kind": "controller", "name": "AUT2", "bus": "B2", "protocol": "telepathy"})).is_err());
    // Too many devices for a segment: 33 meters on a bus of 32.
    for i in 0..33 {
        s.execute(
            "controladd",
            &json!({"at": [i * 100, 9000], "kind": "energyMeter", "name": format!("EN{i}"), "controller": "AUT9", "address": (i + 1).to_string()}),
        )
        .unwrap();
    }
    s.execute("controladd", &json!({"at": [0, 8000], "kind": "controller", "name": "AUT9", "bus": "RS", "protocol": "bacnet"})).unwrap();
    let r = s.execute("controlcheck", &json!({})).unwrap();
    let w: Vec<&str> = r["warnings"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert!(w.iter().any(|x| x.contains("bus RS (BACnet MS/TP)") && x.contains("at most 32")), "{w:?}");
    // MAC 33 is a BACnet address (0 to 127): no address warning for the meters.
    assert!(!w.iter().any(|x| x.contains("EN") && x.contains("address")), "{w:?}");
}
