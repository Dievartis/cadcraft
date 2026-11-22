//! EQUIPMENTADD places chillers, outdoor units and indoor units from a library of models, by name or by
//! power, with any value given instead; EQUIPMENTEDIT changes them.

use cadcraft_doc::{Drawing, EquipmentKind, Handle, equipment_of};
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

fn equip(s: &Session, h: &Value) -> cadcraft_doc::EquipmentParams {
    equipment_of(s.doc().unwrap().entity(hd(h)).unwrap()).unwrap()
}

#[test]
fn equipmentadd_takes_a_model_by_name_or_by_power_and_any_value_given() {
    let mut s = metric();
    // By power: the smallest chiller that gives 100 kW.
    let r = s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "chiller", "capacity": 100})).unwrap();
    let e = equip(&s, &r["handle"]);
    assert_eq!(
        (e.model.as_str(), e.capacity_kw, e.length_mm, e.width_mm, e.fans, e.clearance_mm),
        ("Chiller 107 kW", Some(107.0), 2275.0, 2125.0, 2, 1000.0),
        "{r}"
    );
    assert_eq!(e.name, "GE1");
    // By name, with the name, the sizes and the power given instead.
    let r = s
        .execute("equipmentadd", &json!({"at": [5000, 0], "model": "Chiller 523 kW", "name": "CH-TOIT", "length": 5000, "weight": 3000, "capacityKw": 500, "connections": "Victaulic 5\""}))
        .unwrap();
    let e = equip(&s, &r["handle"]);
    assert_eq!(
        (e.name.as_str(), e.length_mm, e.width_mm, e.weight_kg, e.capacity_kw, e.fans),
        ("CH-TOIT", 5000.0, 2253.0, Some(3000.0), Some(500.0), 8)
    );
    // An indoor unit by kind and power.
    let r = s.execute("equipmentadd", &json!({"at": [0, 5000], "kind": "cassette", "capacity": 3})).unwrap();
    let e = equip(&s, &r["handle"]);
    assert_eq!((e.kind, e.model.as_str(), e.name.as_str()), (EquipmentKind::Cassette, "Cassette 600 3.5 kW", "UI3"));
    // Refused.
    assert!(s.execute("equipmentadd", &json!({"at": [0, 0], "model": "nothing"})).is_err());
    assert!(s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "boiler"})).is_err());
    assert!(s.execute("equipmentadd", &json!({"at": [0, 0], "length": -5})).is_err());
    assert!(s.execute("equipmentadd", &json!({"kind": "chiller"})).is_err(), "a point is required");
    // The library is listed.
    let all = s.execute("equipmentadd", &json!({"list": true})).unwrap();
    let chillers = s.execute("equipmentadd", &json!({"list": true, "kind": "chiller"})).unwrap();
    assert!(all["models"].as_array().unwrap().len() > chillers["models"].as_array().unwrap().len());
    assert!(chillers["models"].as_array().unwrap().iter().all(|m| m["kind"] == "chiller"));
}

#[test]
fn equipmentedit_changes_each_value_or_takes_another_model_and_keeps_the_name() {
    let mut s = metric();
    let r = s.execute("equipmentadd", &json!({"at": [0, 0], "model": "Chiller 41.7 kW", "name": "CH1"})).unwrap();
    let h = r["handle"].clone();
    s.execute(
        "equipmentedit",
        &json!({"handle": h, "capacityKw": 45, "height": 1400, "fans": 2, "clearance": 0, "weight": null, "reference": "mon modèle"}),
    )
    .unwrap();
    let e = equip(&s, &h);
    assert_eq!(
        (e.capacity_kw, e.height_mm, e.fans, e.clearance_mm, e.weight_kg, e.reference.as_str()),
        (Some(45.0), 1400.0, 2, 0.0, None, "mon modèle")
    );
    // Another model: its values, the name kept, then a value over it.
    s.execute("equipmentedit", &json!({"handle": h, "model": "Chiller 124 kW", "width": 2000})).unwrap();
    let e = equip(&s, &h);
    assert_eq!(
        (e.name.as_str(), e.model.as_str(), e.capacity_kw, e.length_mm, e.width_mm, e.weight_kg),
        ("CH1", "Chiller 124 kW", Some(124.0), 2275.0, 2000.0, None)
    );
    assert!(s.execute("equipmentedit", &json!({"handle": h, "length": 0})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": h, "model": "nothing"})).is_err());
    let line = s.execute("line", &json!({"points": [[0, 0], [10, 0]]})).unwrap();
    assert!(s.execute("equipmentedit", &json!({"handle": line["handles"][0], "name": "x"})).is_err(), "not an equipment");
}

#[test]
fn equipmentadd_at_the_command_line_keeps_placing_under_the_next_name() {
    let mut s = metric();
    // Each point is followed by its direction (Enter keeps the turn).
    for t in ["equipmentadd", "t", "vrv", "s", "10", "0,0", "", "3000,0", "", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let d = s.doc().unwrap();
    let all: Vec<_> = d.model.iter().filter_map(|e| equipment_of(e)).collect();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|e| e.kind == EquipmentKind::VrfOutdoor && e.model == "VRV 10 CV"), "{all:?}");
    assert_eq!(all.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["UE1", "UE2"]);
}

#[test]
fn a_ducted_unit_takes_a_number_of_inlets_and_outlets_each_with_the_size_of_the_last_one() {
    let mut s = metric();
    let r = s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "ducted"})).unwrap();
    let round = |d: f64| cadcraft_doc::RiserSize::Round { diameter_mm: d };
    // Three outlets of Ø160: typed as a count after the size.
    s.execute("equipmentedit", &json!({"handle": r["handle"], "outlets": "160", "outletCount": 3})).unwrap();
    let e = equip(&s, &r["handle"]);
    assert_eq!((e.air_sizes(false), e.air_sizes(true)), (vec![round(160.0); 3], Vec::new()));
    // A count alone gives that many of the size the last one had, or Ø200 when there was none: two inlets.
    s.execute("equipmentedit", &json!({"handle": r["handle"], "inletCount": 2})).unwrap();
    assert_eq!(equip(&s, &r["handle"]).air_sizes(true), vec![round(200.0); 2]);
    s.execute("equipmentedit", &json!({"handle": r["handle"], "outletCount": 5})).unwrap();
    assert_eq!(equip(&s, &r["handle"]).air_sizes(false), vec![round(160.0); 5], "more: the size of the last");
    s.execute("equipmentedit", &json!({"handle": r["handle"], "outletCount": 2})).unwrap();
    assert_eq!(equip(&s, &r["handle"]).air_sizes(false).len(), 2, "fewer: the first ones stay");
    // The interface sends a count as a number that may carry a decimal point: 3.0 is 3.
    s.execute("equipmentedit", &json!({"handle": r["handle"], "outletCount": 3.0})).unwrap();
    assert_eq!(equip(&s, &r["handle"]).air_sizes(false).len(), 3);
    s.execute("equipmentedit", &json!({"handle": r["handle"], "outletCount": 2})).unwrap();
    // Spread along the length of the side: the ports are not on one another.
    let ys: Vec<f64> = equip(&s, &r["handle"]).air_ports.iter().filter(|p| !p.inlet).map(|p| p.along_mm).collect();
    assert!(ys.windows(2).all(|w| w[1] > w[0]), "{ys:?}");
    // Refused: too many, not a whole number, not a ducted unit.
    assert!(s.execute("equipmentedit", &json!({"handle": r["handle"], "outletCount": 40})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": r["handle"], "outletCount": -1})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": r["handle"], "inletCount": 1.5})).is_err());
    let chiller = s.execute("equipmentadd", &json!({"at": [5000, 0], "kind": "chiller"})).unwrap();
    assert!(s.execute("equipmentedit", &json!({"handle": chiller["handle"], "outletCount": 2})).is_err());
}

#[test]
fn racks_coolers_and_dehumidifiers_are_put_by_model_named_by_kind_and_the_ducted_dehumidifier_takes_connections() {
    let mut s = metric();
    let put = |s: &mut Session, p: Value| s.execute("equipmentadd", &p).unwrap();
    let r = put(&mut s, json!({"at": [0, 0], "kind": "floorRack"}));
    let e = equip(&s, &r["handle"]);
    assert_eq!((e.kind, e.name.as_str()), (EquipmentKind::FloorRack, "BAIE1"));
    let r = put(&mut s, json!({"at": [3000, 0], "model": "Wall rack 9 U"}));
    assert_eq!(equip(&s, &r["handle"]).height_mm, 500.0);
    let r = put(&mut s, json!({"at": [6000, 0], "model": "Server room cooler 45 kW"}));
    let e = equip(&s, &r["handle"]);
    assert_eq!((e.kind, e.length_mm, e.name.as_str()), (EquipmentKind::ServerCooler, 2000.0, "CRAC3"));
    let wall = put(&mut s, json!({"at": [9000, 0], "model": "Pool dehumidifier 40 (wall)"}));
    assert!(s.execute("equipmentedit", &json!({"handle": wall["handle"], "inlets": [250]})).is_err(), "a wall unit has no duct");
    let duct = put(&mut s, json!({"at": [12000, 0], "model": "Pool dehumidifier 75 (ducted)", "inlets": [250], "outlets": [250]}));
    assert_eq!(equip(&s, &duct["handle"]).air_ports.len(), 2);
    let list = s.execute("equipmentadd", &json!({"list": true, "kind": "wallRack"})).unwrap();
    assert_eq!(list["models"].as_array().unwrap().len(), 5);
}

#[test]
fn an_equipment_is_kept_as_a_model_of_the_drawing_and_the_drawing_gives_it_by_name_and_keeps_it_in_the_file() {
    // Only drawings generated by this test are decoded in process.
    cadcraft_io::dwg_worker::allow_in_process();
    let mut s = metric();
    let a = s
        .execute("equipmentadd", &json!({"at": [0, 0], "model": "Cassette 600 3.5 kW", "weight": 22, "connections": "1/4 - 3/8", "name": "UI9"}))
        .unwrap();
    s.execute("equipmentmodelsave", &json!({"handle": a["handle"], "name": "Cassette maison"})).unwrap();
    let list = s.execute("equipmentadd", &json!({"list": true, "kind": "cassette"})).unwrap();
    let own: Vec<_> = list["models"].as_array().unwrap().iter().filter(|m| m["own"] == true).collect();
    assert_eq!((own.len(), own[0]["name"].as_str(), own[0]["weightKg"].as_f64()), (1, Some("Cassette maison"), Some(22.0)));
    // Given by its name: its values, the library's untouched.
    let b = s.execute("equipmentadd", &json!({"at": [3000, 0], "model": "cassette MAISON"})).unwrap();
    let e = equip(&s, &b["handle"]);
    assert_eq!((e.model.as_str(), e.weight_kg, e.connections.as_str()), ("Cassette maison", Some(22.0), "1/4 - 3/8"));
    s.execute("equipmentedit", &json!({"handle": b["handle"], "model": "Cassette 600 3.5 kW"})).unwrap();
    assert_eq!(equip(&s, &b["handle"]).weight_kg, None);
    // Kept in the file, DXF and DWG.
    for name in ["m.dxf", "m.dwg"] {
        let back = cadcraft_io::read(&cadcraft_io::write(s.doc().unwrap(), name).unwrap(), name).unwrap();
        assert_eq!(back.equipment_models, s.doc().unwrap().equipment_models, "{name}");
    }
    // Replaced under the same name, refused with none, deleted.
    s.execute("equipmentedit", &json!({"handle": a["handle"], "weight": 30})).unwrap();
    s.execute("equipmentmodelsave", &json!({"handle": a["handle"], "name": "cassette maison"})).unwrap();
    assert_eq!(s.doc().unwrap().equipment_models.len(), 1);
    assert_eq!(s.doc().unwrap().equipment_models[0].weight_kg, Some(30.0));
    assert!(s.execute("equipmentmodelsave", &json!({"handle": a["handle"], "name": " "})).is_err());
    assert_eq!(s.execute("equipmentmodeldelete", &json!({"name": "CASSETTE MAISON"})).unwrap()["deleted"], true);
    assert!(s.execute("equipmentadd", &json!({"at": [0, 0], "model": "Cassette maison"})).is_err());
}
