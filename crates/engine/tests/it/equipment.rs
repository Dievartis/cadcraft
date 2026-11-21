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
fn hvactakeoff_counts_the_level_by_section_fitting_terminal_space_and_writes_a_spreadsheet() {
    let mut s = metric();
    // A space of 6 x 5 m, a unit, a run of 4 m then a run of 2 m at another section, two terminals in the space, an equipment.
    s.execute("spaceadd", &json!({"points": [[0, 0], [6000, 0], [6000, 5000], [0, 5000]], "name": "Salle", "flow": 600})).unwrap();
    s.execute("ahuadd", &json!({"at": [-3000, 1000]})).unwrap();
    s.execute("ductadd", &json!({"points": [[-1500, 1250], [1000, 1250], [4000, 1250]], "diameter": 200, "air": "supply", "altitude": 2.5})).unwrap();
    s.execute("airterminaladd", &json!({"at": [4000, 1250], "flow": 300, "air": "supply"})).unwrap();
    s.execute("airterminaladd", &json!({"at": [1000, 3000], "flow": 100, "air": "extract", "type": "extract"})).unwrap();
    s.execute("equipmentadd", &json!({"at": [9000, 0], "model": "VRV 14 CV", "name": "UE1"})).unwrap();
    s.execute("equipmentadd", &json!({"at": [9000, 4000], "model": "VRV 14 CV", "name": "UE2"})).unwrap();
    let t = s.execute("hvactakeoff", &json!({"level": "R+1"})).unwrap();
    assert_eq!(t["level"], "R+1");
    // The two runs are 2500 and 3000 mm long, Ø200, supply, at 2.5 m: one line.
    let ducts = t["ducts"].as_array().unwrap();
    assert_eq!(ducts.len(), 1, "{t}");
    assert_eq!((ducts[0]["count"].as_u64(), ducts[0]["section"].as_str(), ducts[0]["altitudeM"].as_f64()), (Some(2), Some("Ø200"), Some(2.5)));
    assert!((ducts[0]["lengthM"].as_f64().unwrap() - 5.5).abs() < 1e-9);
    assert!((ducts[0]["areaM2"].as_f64().unwrap() - 5.5 * std::f64::consts::PI * 0.2).abs() < 0.01);
    assert_eq!(t["totals"]["terminals"], 2);
    assert_eq!(t["equipment"][0]["count"], 2);
    assert_eq!(t["equipment"][0]["power"], 28.0, "two 14 CV units");
    // The space holds the two terminals and the 3.5 m of duct that run through it (from x = 0 to 4000, and 1000 on).
    let sp = &t["spaces"][0];
    assert_eq!((sp["supplyTerminals"].as_u64(), sp["extractTerminals"].as_u64(), sp["neededM3h"].as_f64()), (Some(1), Some(1), Some(600.0)), "{sp}");
    assert!((sp["ductM"].as_f64().unwrap() - 4.0).abs() < 0.1, "{sp}");
    // The spreadsheet: `;` between cells, a comma for the decimal mark, an accent that survives.
    let path = std::env::temp_dir().join(format!("cadcraft-takeoff-{}.csv", std::process::id()));
    let r = s.execute("hvactakeoff", &json!({"level": "R+1", "path": path.to_str().unwrap()})).unwrap();
    assert_eq!(r["path"], path.to_str().unwrap());
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(&bytes[..3], [0xEF, 0xBB, 0xBF], "a byte order mark");
    let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(text.starts_with("Level;R+1\n"), "{text}");
    assert!(text.contains("supply;rigid;Ø200;2,5;2;5,5;"), "{text}");
    assert!(s.execute("hvactakeoff", &json!({"decimal": "dot"})).is_err());
    let point = s.execute("hvactakeoff", &json!({"path": path.to_str().unwrap(), "decimal": "point"})).unwrap();
    let text = String::from_utf8(std::fs::read(point["path"].as_str().unwrap()).unwrap()[3..].to_vec()).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(text.contains("supply;rigid;Ø200;2.5;2;5.5;"), "{text}");
}

#[test]
fn a_riser_height_is_counted_in_the_takeoff() {
    let mut s = metric();
    s.execute("riseradd", &json!({"at": [0, 0], "width": 600, "depth": 400, "height": 2800})).unwrap();
    let h = s.execute("riseradd", &json!({"at": [2000, 0], "width": 600, "depth": 400})).unwrap();
    let t = s.execute("hvactakeoff", &json!({"level": "R+1"})).unwrap();
    assert_eq!(t["totals"]["riserLengthM"], 2.8);
    s.execute("riseredit", &json!({"handle": h["handle"], "height": 3000})).unwrap();
    let t = s.execute("hvactakeoff", &json!({"level": "R+1"})).unwrap();
    assert_eq!(t["totals"]["riserLengthM"], 5.8);
    assert!(s.execute("riseredit", &json!({"handle": h["handle"], "height": -1})).is_err());
}

#[test]
fn hvactakeoff_writes_an_excel_workbook_with_the_spaces_and_the_calculation_note() {
    let mut s = metric();
    s.execute("spaceadd", &json!({"points": [[0, 0], [6000, 0], [6000, 5000], [0, 5000]], "name": "Salle", "flow": 600})).unwrap();
    s.execute("ahuadd", &json!({"at": [-3000, 1100]})).unwrap();
    s.execute("ductadd", &json!({"points": [[-2400, 1250], [1000, 1250], [4000, 1250]], "diameter": 200, "air": "supply"})).unwrap();
    s.execute("airterminaladd", &json!({"at": [4000, 1250], "flow": 300, "air": "supply"})).unwrap();
    let path = std::env::temp_dir().join(format!("cadcraft-takeoff-{}.xlsx", std::process::id()));
    let r = s.execute("hvactakeoff", &json!({"level": "R+1", "path": path.to_str().unwrap(), "lang": "fr"})).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(&bytes[..4], b"PK\x03\x04", "a zip");
    let text = String::from_utf8_lossy(&bytes);
    for part in ["xl/worksheets/sheet1.xml", "Locaux", "Détail des locaux", "Note de calcul", "Salle", "Pression à fournir"] {
        assert!(text.contains(part), "{part}");
    }
    // The space holds the terminal and 3 m of the duct; the note has the network with its critical run.
    let spaces = &r["spaces"][0];
    assert_eq!(spaces["terminalModels"][0]["count"], 1, "{spaces}");
    assert!(spaces["ducts"][0]["lengthM"].as_f64().unwrap() > 2.0, "{spaces}");
    let net = &r["note"]["networks"][0];
    assert!(net["requiredPa"].as_f64().unwrap() > 0.0 && net["segments"][0]["critical"] == true, "{net}");
    // The English labels; and no note when asked not to.
    let en = s.execute("hvactakeoff", &json!({"path": path.to_str().unwrap(), "note": false})).unwrap();
    std::fs::remove_file(&path).ok();
    assert!(en.get("note").is_none());
    assert!(s.execute("hvactakeoff", &json!({"lang": "fr\u{1}"})).is_err());
}

#[test]
fn indoor_units_are_spread_in_a_space_by_count_or_by_grid_and_outdoor_units_are_refused() {
    use cadcraft_doc::equipment_of;
    let mut s = Session::new();
    *s.doc_mut().unwrap() = cadcraft_doc::Drawing::new_metric();
    let sp = s.execute("spaceadd", &json!({"points": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]], "name": "Open space", "flow": 900})).unwrap();
    let space = sp["handle"].as_str().unwrap().to_string();
    let r = s.execute("equipmentadd", &json!({"space": space, "kind": "cassette", "count": 6})).unwrap();
    assert_eq!(r["handles"].as_array().unwrap().len(), 6, "{r}");
    let places: Vec<(f64, f64)> = s
        .doc()
        .unwrap()
        .model
        .iter()
        .filter(|e| equipment_of(e).is_some())
        .filter_map(|e| if let cadcraft_doc::EntityKind::Insert(i) = &e.kind { Some((i.insert.x, i.insert.y)) } else { None })
        .collect();
    assert!(places.iter().all(|(x, y)| *x > 0.0 && *x < 8000.0 && *y > 0.0 && *y < 6000.0), "inside the space: {places:?}");
    let names: std::collections::BTreeSet<String> = s.doc().unwrap().model.iter().filter_map(|e| equipment_of(e)).map(|e| e.name).collect();
    assert_eq!(names.len(), 6, "each unit has its own name: {names:?}");
    // A grid of 2 rows and 3 columns, under a name that gets a number each.
    let g = s.execute("equipmentadd", &json!({"space": space, "kind": "wallUnit", "rows": 2, "cols": 3, "name": "UIM"})).unwrap();
    assert_eq!(g["handles"].as_array().unwrap().len(), 6);
    assert!(s.doc().unwrap().model.iter().filter_map(|e| equipment_of(e)).any(|e| e.name == "UIM6"));
    // Outdoor units are not spread, a count that is too big or a handle that is no space is refused.
    assert!(s.execute("equipmentadd", &json!({"space": space, "kind": "vrfOutdoor", "count": 2})).is_err());
    assert!(s.execute("equipmentadd", &json!({"space": space, "kind": "cassette", "count": 100000})).is_err());
    assert!(s.execute("equipmentadd", &json!({"space": "ZZZZ", "kind": "cassette", "count": 2})).is_err());
}

#[test]
fn a_ducted_unit_takes_the_sizes_of_its_connections_and_a_duct_laid_on_it_gives_it_one_by_itself() {
    let mut s = metric();
    let r = s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "ducted"})).unwrap();
    let ports = |s: &Session| equip(s, &r["handle"]).air_ports;
    assert!(ports(&s).is_empty(), "one of each, as drawn, until some are given");
    s.execute("equipmentedit", &json!({"handle": r["handle"], "inlets": "250, 400x250", "outlets": [160, 160, 200]})).unwrap();
    let e = equip(&s, &r["handle"]);
    assert_eq!((e.air_sizes(true).len(), e.air_sizes(false).len()), (2, 3));
    assert_eq!(e.air_sizes(true)[1], cadcraft_doc::RiserSize::Rectangular { width_mm: 400.0, height_mm: 250.0 });
    // Only the outlets changed: the inlets stay.
    s.execute("equipmentedit", &json!({"handle": r["handle"], "outlets": "315"})).unwrap();
    let e = equip(&s, &r["handle"]);
    assert_eq!((e.air_sizes(true).len(), e.air_sizes(false)), (2, vec![cadcraft_doc::RiserSize::Round { diameter_mm: 315.0 }]));
    // Refused: not a ducted unit, nothing sized, too many.
    let chiller = s.execute("equipmentadd", &json!({"at": [5000, 0], "kind": "chiller"})).unwrap();
    assert!(s.execute("equipmentedit", &json!({"handle": chiller["handle"], "inlets": "250"})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": r["handle"], "inlets": "abc"})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": r["handle"], "inlets": (0..40).map(|_| 200).collect::<Vec<_>>()})).is_err());
    // A duct ended on the front side of another unit gives it that connection, with no setting.
    let u2 = s.execute("equipmentadd", &json!({"at": [0, 5000], "kind": "ducted"})).unwrap();
    let front = 5000.0 - equip(&s, &u2["handle"]).width_mm / 2.0;
    s.execute("ductadd", &json!({"points": [[100, front - 2000.0], [100, front - 40.0]], "diameter": 250})).unwrap();
    let e = equip(&s, &u2["handle"]);
    assert_eq!(e.air_sizes(false), vec![cadcraft_doc::RiserSize::Round { diameter_mm: 250.0 }]);
    assert!((e.air_ports[0].along_mm - 100.0).abs() < 1.0, "{:?}", e.air_ports);
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
fn an_equipment_a_fixture_and_a_piece_of_furniture_are_turned_when_put_by_a_second_click_or_an_angle() {
    use cadcraft_doc::{EntityKind, Handle};
    let mut s = metric();
    let rotation = |s: &Session, k: usize| {
        let d = s.doc().unwrap();
        d.model
            .iter()
            .filter_map(|e| if let EntityKind::Insert(i) = &e.kind { Some((e.handle, i.rotation)) } else { None })
            .nth(k)
            .map(|(_, r): (Handle, f64)| r)
    };
    // A second click above the point turns it a quarter turn; a typed angle also.
    for t in ["equipmentadd", "0,0", "0,5000", "9000,0", "30", ""] {
        s.cmdline(t).unwrap();
    }
    assert!((rotation(&s, 0).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!((rotation(&s, 1).unwrap() - 30f64.to_radians()).abs() < 1e-9);
    for t in ["furnitureadd", "0,2000", "-1000,2000", ""] {
        s.cmdline(t).unwrap();
    }
    for t in ["fixtureadd", "0,4000", "0,6000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!((rotation(&s, 2).unwrap() - std::f64::consts::PI).abs() < 1e-9);
    assert!((rotation(&s, 3).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!(s.running.is_none());
    // A wrong angle is refused and nothing is put; Enter then puts the unit with the turn it had.
    let n = s.doc().unwrap().model.iter().count();
    for t in ["equipmentadd", "0,0", "abc"] {
        let _ = s.cmdline(t);
    }
    assert_eq!(s.doc().unwrap().model.iter().count(), n);
    s.cmdline("").unwrap();
    s.cmdline("").unwrap();
    assert_eq!(s.doc().unwrap().model.iter().count(), n + 1);
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
fn the_charge_of_a_refrigerant_system_is_checked_against_the_rooms_it_serves() {
    let mut s = metric();
    // A room of 4 x 5 m, 2.5 m high: 50 m³, so 22 kg of R410A (0.44 kg/m³) at most. An indoor unit in it, an outdoor unit outside, a pipe between them.
    s.execute("spaceadd", &json!({"points": [[0, 0], [4000, 0], [4000, 5000], [0, 5000]], "name": "Salle", "flow": 100})).unwrap();
    let ue = s.execute("equipmentadd", &json!({"at": [10000, 2500], "model": "VRV 10 CV", "refrigerant": "R410A", "charge": 30})).unwrap();
    s.execute("equipmentadd", &json!({"at": [2000, 2500], "kind": "cassette", "capacity": 3.5})).unwrap();
    let e = equip(&s, &ue["handle"]);
    assert_eq!((e.refrigerant.as_str(), e.charge_kg), ("R410A", Some(30.0)));
    let ue_edge = 10000.0 - e.length_mm / 2.0;
    s.execute("pipeadd", &json!({"points": [[2000, 2500], [ue_edge, 2500]], "service": "refrigerantLiquid", "dn": 9.52})).unwrap();
    let r = s.execute("refrigerantcharge", &json!({})).unwrap();
    let sys = &r["systems"][0];
    assert_eq!((sys["chargeKg"].as_f64(), sys["limitKgM3"].as_f64()), (Some(30.0), Some(0.44)), "{r}");
    assert_eq!(sys["rooms"][0]["limitKg"].as_f64(), Some(22.0), "{r}");
    assert_eq!((r["problems"].as_u64(), sys["rooms"][0]["ok"].as_bool()), (Some(1), Some(false)), "{r}");
    assert!(sys["warnings"][0].as_str().unwrap().contains("at least 68.2 m³"), "{r}");
    // A smaller charge is right, a higher room too; another fluid is not checked; no charge typed, no check.
    s.execute("equipmentedit", &json!({"handle": ue["handle"], "charge": 18})).unwrap();
    assert_eq!(s.execute("refrigerantcharge", &json!({})).unwrap()["problems"], 0);
    s.execute("equipmentedit", &json!({"handle": ue["handle"], "charge": 30})).unwrap();
    assert_eq!(s.execute("refrigerantcharge", &json!({"height": 3.5})).unwrap()["problems"], 0, "70 m³ hold 30.8 kg");
    s.execute("equipmentedit", &json!({"handle": ue["handle"], "refrigerant": "R32"})).unwrap();
    let r = s.execute("refrigerantcharge", &json!({})).unwrap();
    assert!(r["systems"][0]["warnings"][0].as_str().unwrap().contains("only R410A"), "{r}");
    s.execute("equipmentedit", &json!({"handle": ue["handle"], "charge": null})).unwrap();
    assert!(s.execute("refrigerantcharge", &json!({})).unwrap()["systems"].as_array().unwrap().is_empty());
    assert!(s.execute("refrigerantcharge", &json!({"height": -1})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": ue["handle"], "charge": -5})).is_err());
}

#[test]
fn an_equipment_is_kept_as_a_model_of_the_drawing_and_the_drawing_gives_it_by_name_and_keeps_it_in_the_file() {
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
