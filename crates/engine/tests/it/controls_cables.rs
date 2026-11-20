//! The remote control cable (pink, apart from the bus and the electrical cables), the colour of the lines of a cable riser (the wire's layer), and the
//! size written beside an equipment.

use cadcraft_doc::{CableUsage, Drawing, EntityKind, Handle, cable_layer_of, cable_of, equipment_of};
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
fn a_remote_control_cable_has_its_own_layer_and_colour_and_is_a_signal_cable() {
    let mut s = metric();
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [5000, 0]], "usage": "telecommande", "circuit": "TC1"})).unwrap();
    let c = cable_of(s.doc().unwrap().entity(hd(&r["handles"][0])).unwrap()).unwrap();
    assert_eq!(c.usage, CableUsage::Remote);
    assert!(c.usage.is_signal());
    assert!(s.doc().unwrap().layer(c.layer()).is_some(), "the layer of the cable exists");
    // Its colour is pink: none of the other cables has it.
    let colour = |u: CableUsage| cadcraft_doc::elec_layer_color(u.layer());
    assert!(colour(CableUsage::Remote).is_some());
    for other in CableUsage::ALL.into_iter().filter(|u| *u != CableUsage::Remote) {
        assert_ne!(colour(other), colour(CableUsage::Remote), "{other:?} has the colour of the remote control cable");
    }
    // A riser of these cables, and the lines of any riser, are on the layer of the cables of their circuit.
    let rr =
        s.execute("elecriser", &json!({"at": [2000, 2000], "width": 400, "depth": 400, "circuit": "TD1-C1", "height": 3000, "load": 6})).unwrap();
    let want = cable_layer_of(CableUsage::Power, "TD1-C1");
    let d = s.doc().unwrap();
    assert!(d.layer(want).is_some(), "the layer of the circuit exists");
    let EntityKind::Insert(i) = &d.entity(hd(&rr["handle"])).unwrap().kind else { panic!("a block reference") };
    assert!(d.block(&i.block).unwrap().entities.iter().any(|e| e.common.layer == want), "a line of the riser is on {want}");
    assert!(s.execute("cableadd", &json!({"points": [[0, 0], [1000, 0]], "usage": "nope"})).is_err());
}

#[test]
fn the_size_of_an_equipment_can_be_hidden_one_by_one_or_for_the_whole_drawing() {
    let mut s = metric();
    let a = s.execute("equipmentadd", &json!({"at": [0, 0], "kind": "ducted", "name": "UI1"})).unwrap();
    let b = s.execute("equipmentadd", &json!({"at": [4000, 0], "kind": "cassette", "name": "UI2"})).unwrap();
    let has_size = |s: &Session, r: &Value| texts(s, hd(&r["handle"])).iter().any(|t| t.starts_with("Dim. "));
    assert!(has_size(&s, &a) && has_size(&s, &b));
    s.execute("equipmentedit", &json!({"handle": a["handle"], "showSize": false})).unwrap();
    assert!(!has_size(&s, &a) && has_size(&s, &b));
    assert!(!equipment_of(s.doc().unwrap().entity(hd(&a["handle"])).unwrap()).unwrap().show_size);
    // The whole drawing: hide what is shown, show what is hidden.
    assert_eq!(s.execute("equipmentsizedisplay", &json!({"show": false})).unwrap()["changed"], 1);
    assert!(!has_size(&s, &a) && !has_size(&s, &b));
    assert_eq!(s.execute("equipmentsizedisplay", &json!({})).unwrap()["changed"], 2);
    assert!(has_size(&s, &a) && has_size(&s, &b));
    assert!(s.execute("equipmentsizedisplay", &json!({"show": "no"})).is_err());
    assert!(s.execute("equipmentedit", &json!({"handle": a["handle"], "showSize": "no"})).is_err());
}
