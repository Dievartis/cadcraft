//! The Furniture tab: furniture and appliances put from the catalogue, edited, and listed.

use cadcraft_doc::{Drawing, EntityKind, Handle, furniture_models, furniture_of};
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

fn texts(s: &Session, h: Handle) -> Vec<String> {
    let d = s.doc().unwrap();
    let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { return Vec::new() };
    d.block(&i.block).unwrap().entities.iter().filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None }).collect()
}

#[test]
fn every_model_of_the_catalogue_is_put_at_its_size_by_its_id_and_by_its_name() {
    let mut s = metric();
    for (k, m) in furniture_models().iter().enumerate() {
        let r = s.execute("furnitureadd", &json!({"at": [k as f64 * 3000.0, 0.0], "model": m.id})).unwrap();
        let f = furniture_of(s.doc().unwrap().entity(hd(&r["handles"][0])).unwrap()).unwrap();
        assert_eq!((f.model.as_str(), f.width_mm, f.depth_mm, f.height_mm), (m.id, m.width_mm, m.depth_mm, m.height_mm));
        let by_name = s.execute("furnitureadd", &json!({"at": [k as f64 * 3000.0, 5000.0], "model": m.label})).unwrap();
        assert_eq!(furniture_of(s.doc().unwrap().entity(hd(&by_name["handles"][0])).unwrap()).unwrap().model, m.id);
    }
}

#[test]
fn appliances_have_their_power_a_name_is_written_and_values_are_checked() {
    let mut s = metric();
    let fridge = s.execute("furnitureadd", &json!({"at": [0, 0], "model": "fridge", "name": "FR1", "altitude": 0.1})).unwrap();
    let f = furniture_of(s.doc().unwrap().entity(hd(&fridge["handles"][0])).unwrap()).unwrap();
    assert_eq!((f.power_w, f.bottom_mm, f.name.as_str()), (Some(120.0), Some(100.0), "FR1"));
    assert_eq!(texts(&s, hd(&fridge["handles"][0])), ["FR1"]);
    let sofa = s.execute("furnitureadd", &json!({"at": [3000, 0], "model": "sofa3", "width": 2400, "rotation": 180})).unwrap();
    let f = furniture_of(s.doc().unwrap().entity(hd(&sofa["handles"][0])).unwrap()).unwrap();
    assert_eq!((f.width_mm, f.power_w), (2400.0, None));
    assert!(s.execute("furnitureadd", &json!({"at": [0, 0], "model": "piano a queue"})).is_err());
    assert!(s.execute("furnitureadd", &json!({"at": [0, 0], "model": "fridge", "width": 5})).is_err());
    assert!(s.execute("furnitureadd", &json!({"at": [0, 0], "model": "fridge", "power": -4})).is_err());
    assert!(s.execute("furnitureadd", &json!({})).is_err());
}

#[test]
fn furnitureedit_changes_the_model_the_size_the_power_and_the_altitude() {
    let mut s = metric();
    let r = s.execute("furnitureadd", &json!({"at": [0, 0], "model": "tv32", "name": "TV1", "altitude": 1.2})).unwrap();
    let h = r["handles"][0].clone();
    let f = |s: &Session| furniture_of(s.doc().unwrap().entity(hd(&h)).unwrap()).unwrap();
    s.execute("furnitureedit", &json!({"handle": h, "model": "tv55"})).unwrap();
    assert_eq!((f(&s).model.as_str(), f(&s).width_mm, f(&s).name.as_str(), f(&s).bottom_mm), ("tv55", 1230.0, "TV1", Some(1200.0)));
    s.execute("furnitureedit", &json!({"handle": h, "power": null, "altitude": null, "height": 800})).unwrap();
    assert_eq!((f(&s).power_w, f(&s).bottom_mm, f(&s).height_mm), (None, None, 800.0));
    assert!(s.execute("furnitureedit", &json!({"handle": h, "width": 1})).is_err());
    assert!(s.execute("furnitureedit", &json!({"handle": h, "model": "nope"})).is_err());
}

#[test]
fn furniturelist_gives_the_catalogue_or_one_of_its_groups() {
    let mut s = metric();
    let all = s.execute("furniturelist", &json!({})).unwrap();
    assert_eq!(all["models"].as_array().unwrap().len(), furniture_models().len());
    let kitchen = s.execute("furniturelist", &json!({"group": "kitchen"})).unwrap();
    assert!(!kitchen["models"].as_array().unwrap().is_empty() && kitchen["models"].as_array().unwrap().iter().all(|m| m["group"] == "Kitchen"));
}
