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

fn cable(s: &Session, h: &Value) -> cadcraft_doc::CableParams {
    cable_of(s.doc().unwrap().entity(hd(h)).unwrap()).unwrap()
}

#[test]
fn a_cable_is_given_by_its_conductors_and_section_as_it_is_written_and_by_a_cable_type() {
    let mut s = metric();
    // The conductors, the earth, the shield and the section, in the text of the section.
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [5000, 0]], "circuit": "TD1-C1", "section": "3G1.5"})).unwrap();
    let c = cable(&s, &r["handles"][0]);
    assert_eq!((c.cores, c.earth, c.shielded, c.section_mm2, c.size_text().as_str()), (Some(3), true, false, Some(1.5), "3G1.5"));
    let r = s
        .execute("cableadd", &json!({"points": [[0, 500], [5000, 500]], "circuit": "BUS1", "usage": "bus", "section": "2x0,75 mm² blindé"}))
        .unwrap();
    let c = cable(&s, &r["handles"][0]);
    assert_eq!((c.cores, c.earth, c.shielded, c.section_mm2), (Some(2), false, true, Some(0.75)));
    assert_eq!((c.size_text().as_str(), c.spec_text().as_str(), c.label_text().as_str()), ("2x0.75 blindé", "2x0.75 blindé", "BUS1 2x0.75 blindé"));
    // A number alone is a section: the conductors are those of the phases (5G on three phases), and a number sent to a cable that has its own leaves them.
    let r = s.execute("cableadd", &json!({"points": [[0, 1000], [5000, 1000]], "circuit": "TD1-C2", "phases": "three", "section": 16})).unwrap();
    assert_eq!(cable(&s, &r["handles"][0]).size_text(), "5G16");
    let r2 = s.execute("elecedit", &json!({"handle": r["handles"][0], "section": 25})).unwrap();
    assert_eq!(r2["changed"], 1);
    assert_eq!(cable(&s, &r["handles"][0]).size_text(), "5G25");
    s.execute("elecedit", &json!({"handle": r["handles"][0], "section": "4x1.5 blindé"})).unwrap();
    assert_eq!(cable(&s, &r["handles"][0]).size_text(), "4x1.5 blindé");
    s.execute("elecedit", &json!({"handle": r["handles"][0], "section": "auto"})).unwrap();
    assert_eq!(cable(&s, &r["handles"][0]).size_text(), "");
    // Any other text is kept as it is written, with no section (the circuit check says it is not checked); too long, or not a text, is refused.
    for text in ["3G", "abc", "0x1.5", "3G700"] {
        s.execute("elecedit", &json!({"handle": r["handles"][0], "section": text})).unwrap();
        let c = cable(&s, &r["handles"][0]);
        assert_eq!((c.written.as_deref(), c.section_mm2), (Some(text), None));
    }
    assert!(s.execute("elecedit", &json!({"handle": r["handles"][0], "section": "x".repeat(81)})).is_err());
    assert!(s.execute("elecedit", &json!({"handle": r["handles"][0], "section": true})).is_err());
    // A cable type of the starting list gives its cable and its use.
    let t = s
        .execute("cableadd", &json!({"points": [[0, 1500], [5000, 1500]], "circuit": "TC1", "cableType": "Remote control 2x0.75 shielded"}))
        .unwrap();
    let c = cable(&s, &t["handles"][0]);
    assert_eq!((c.usage, c.size_text().as_str()), (CableUsage::Remote, "2x0.75 blindé"));
    assert!(s.execute("cableadd", &json!({"points": [[0, 0], [1000, 0]], "cableType": "nope"})).is_err());
}

#[test]
fn cable_types_are_saved_in_the_drawing_listed_and_deleted() {
    let mut s = metric();
    let all = s.execute("cabletypelist", &json!({})).unwrap();
    let n = all["types"].as_array().unwrap().len();
    assert!(n > 20 && all["types"].as_array().unwrap().iter().all(|t| t["own"] == false));
    let bus = s.execute("cabletypelist", &json!({"usage": "bus"})).unwrap();
    assert!(bus["types"].as_array().unwrap().iter().all(|t| t["usage"] == "bus") && !bus["types"].as_array().unwrap().is_empty());
    s.execute("cabletypeadd", &json!({"name": "KNX 2x0.8", "spec": "2x0.8 blindé", "usage": "bus"})).unwrap();
    let after = s.execute("cabletypelist", &json!({})).unwrap();
    assert_eq!(after["types"].as_array().unwrap().len(), n + 1);
    assert!(after["types"].as_array().unwrap().iter().any(|t| t["name"] == "KNX 2x0.8" && t["own"] == true && t["spec"] == "2x0.8 blindé"));
    // It is used by name, and saved again under the same name it is replaced.
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [3000, 0]], "circuit": "BUS2", "cableType": "knx 2x0.8"})).unwrap();
    assert_eq!(cable(&s, &r["handles"][0]).size_text(), "2x0.8 blindé");
    s.execute("cabletypeadd", &json!({"name": "knx 2x0.8", "spec": "2x1"})).unwrap();
    assert_eq!(s.doc().unwrap().cable_types.len(), 1);
    assert!(s.execute("cabletypeadd", &json!({"name": "", "spec": "2x1"})).is_err());
    assert!(s.execute("cabletypeadd", &json!({"name": "x", "spec": "rien"})).is_err());
    assert_eq!(s.execute("cabletypedelete", &json!({"name": "KNX 2X0.8"})).unwrap()["deleted"], true);
    assert_eq!(s.execute("cabletypedelete", &json!({"name": "KNX 2X0.8"})).unwrap()["deleted"], false);
}

#[test]
fn a_riser_line_takes_its_cable_as_written_and_a_colour_of_its_own() {
    let mut s = metric();
    let r = s
        .execute(
            "elecriser",
            &json!({"at": [0, 0], "width": 600, "depth": 400, "lines": [{"circuit": "TD1-C1", "section": "3G1.5"}, {"usage": "bus", "circuit": "BUS1", "section": "2x0.75 blindé", "colour": "#FF8800"}, {"circuit": "TD1-C2", "section": 16}]}),
        )
        .unwrap();
    let h = hd(&r["handle"]);
    let riser = |s: &Session| cadcraft_doc::any_riser_of(s.doc().unwrap().entity(h).unwrap()).unwrap();
    let lines = riser(&s).cable.lines;
    assert_eq!(lines[0].text(), "TD1-C1 3G1.5");
    assert_eq!(lines[1].text(), "BUS1 2x0.75 blindé");
    assert_eq!(lines[2].text(), "TD1-C2 16 mm²", "a section alone keeps the text it had");
    assert_eq!(lines[1].colour.as_deref(), Some("#ff8800"));
    // The pieces of the line carry the colour; the others keep the layer's.
    let d = s.doc().unwrap();
    let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { panic!("a block reference") };
    let own = cadcraft_color::Color::True(cadcraft_color::Rgb(0xff, 0x88, 0x00));
    assert!(d.block(&i.block).unwrap().entities.iter().any(|e| e.common.color == own));
    assert!(d.layer("E-CABLE-BUS-BUS1").is_none() || d.layers.iter().all(|l| !l.name.contains('#')), "no layer is made of the colour");
    // Edited: another cable, the colour taken off, a bad colour refused.
    s.execute("riseredit", &json!({"handle": h.hex(), "line": {"index": 1, "section": "5G2.5", "colour": null}})).unwrap();
    assert_eq!(riser(&s).cable.lines[0].text(), "TD1-C1 5G2.5");
    s.execute("riseredit", &json!({"handle": h.hex(), "line": {"index": 2, "colour": ""}})).unwrap();
    assert!(riser(&s).cable.lines[1].colour.is_none());
    assert!(s.execute("riseredit", &json!({"handle": h.hex(), "line": {"index": 1, "colour": "orange"}})).is_err());
    assert!(s.execute("riseredit", &json!({"handle": h.hex(), "line": {"index": 1, "section": "abc"}})).is_err());
}

#[test]
fn a_cables_label_can_be_left_off_and_put_back_and_the_choice_holds_for_the_next_cables() {
    let mut s = metric();
    // Without its label: no text, and the cable keeps its circuit and section.
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [5000, 0]], "circuit": "TD1-C1", "section": "3G1.5", "label": false})).unwrap();
    let h = hd(&r["handles"][0]);
    assert!(texts(&s, h).is_empty());
    assert_eq!(cable(&s, &r["handles"][0]).size_text(), "3G1.5");
    s.execute("elecedit", &json!({"handle": r["handles"][0], "label": true})).unwrap();
    assert!(!texts(&s, h).is_empty(), "the label is back");
    assert!(s.execute("elecedit", &json!({"handle": r["handles"][0], "label": "no"})).is_err());
    // At the command line: Label No is kept for the cables drawn after it.
    for t in ["cableadd", "l", "n", "0,1000", "5000,1000", ""] {
        s.cmdline(t).unwrap();
    }
    for t in ["cableadd", "0,2000", "5000,2000", ""] {
        s.cmdline(t).unwrap();
    }
    let d = s.doc().unwrap();
    let drawn: Vec<Handle> = d.model.iter().filter(|e| cable_of(e).is_some()).map(|e| e.handle).filter(|x| *x != h).collect();
    assert_eq!(drawn.len(), 2);
    assert!(drawn.iter().all(|x| texts(&s, *x).is_empty()), "both drawn without their label");
}

#[test]
fn a_cable_put_on_a_layer_takes_its_colour_and_keeps_it_when_edited() {
    let mut s = metric();
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [5000, 0]], "circuit": "TD1-C1"})).unwrap();
    let h = hd(&r["handles"][0]);
    // Its pieces are on layer 0: they take the layer the cable is on, and so its colour.
    let d = s.doc().unwrap();
    let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { panic!() };
    let block = d.blocks.values().find(|b| b.name == i.block).unwrap();
    assert!(block.entities.iter().all(|e| e.common.layer == "0"), "the pieces follow the cable's layer");
    // Put on a layer of the user's, it stays there when the cable is changed.
    s.execute("layer.new", &json!({"name": "MY-CABLES", "color": 3})).unwrap();
    s.execute("properties.set", &json!({"handles": [r["handles"][0]], "layer": "MY-CABLES"})).unwrap();
    s.execute("elecedit", &json!({"handle": r["handles"][0], "section": "3G2.5"})).unwrap();
    assert_eq!(s.doc().unwrap().entity(h).unwrap().common.layer, "MY-CABLES");
    // One on its circuit's layer follows its circuit.
    let r2 = s.execute("cableadd", &json!({"points": [[0, 1000], [5000, 1000]], "circuit": "TD1-C1"})).unwrap();
    let h2 = hd(&r2["handles"][0]);
    let before = s.doc().unwrap().entity(h2).unwrap().common.layer.clone();
    s.execute("elecedit", &json!({"handle": r2["handles"][0], "circuit": "TD1-C2"})).unwrap();
    assert_ne!(s.doc().unwrap().entity(h2).unwrap().common.layer, before);
}

#[test]
fn every_cable_type_of_the_starting_library_can_be_given_to_a_cable() {
    let mut s = metric();
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [5000, 0]], "circuit": "X1"})).unwrap();
    for t in cadcraft_doc::standard_cable_types() {
        s.execute("elecedit", &json!({"handle": r["handles"][0], "cableType": t.name}))
            .unwrap_or_else(|e| panic!("the cable type {:?} is refused: {e}", t.name));
    }
}

#[test]
fn a_cable_written_as_text_keeps_it_in_its_label_and_is_not_sized_and_its_label_can_go_under_it() {
    let mut s = metric();
    s.execute("elecdeviceadd", &json!({"at": [0, 0], "kind": "socket", "circuit": "TD1-C1", "power": 2000})).unwrap();
    let r = s.execute("cableadd", &json!({"points": [[0, 0], [5000, 0]], "circuit": "TD1-C1"})).unwrap();
    let h = hd(&r["handles"][0]);
    s.execute("elecedit", &json!({"handle": r["handles"][0], "section": "U/UTP cat 6 LSOH"})).unwrap();
    let c = cable(&s, &r["handles"][0]);
    assert_eq!((c.written.as_deref(), c.section_mm2), (Some("U/UTP cat 6 LSOH"), None));
    assert!(texts(&s, h).iter().any(|t| t.contains("U/UTP cat 6 LSOH")), "{:?}", texts(&s, h));
    // Sizing leaves it as it is written, and the check says it is not checked.
    s.execute("circuitsize", &json!({})).unwrap();
    assert_eq!(cable(&s, &r["handles"][0]).section_mm2, None);
    let check = s.execute("circuitcheck", &json!({})).unwrap();
    assert!(check.to_string().contains("written as text"), "{check}");
    // A cable that reads as one gives the section back.
    s.execute("elecedit", &json!({"handle": r["handles"][0], "section": "3G2.5"})).unwrap();
    let c = cable(&s, &r["handles"][0]);
    assert_eq!((c.written, c.section_mm2), (None, Some(2.5)));
    // Under the cable: the label moves to the other side of it.
    let y = |s: &Session| {
        let d = s.doc().unwrap();
        let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { panic!() };
        d.block(&i.block).unwrap().entities.iter().find_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.insert.y) } else { None }).unwrap()
    };
    assert!(y(&s) > 0.0);
    s.execute("elecedit", &json!({"handle": r["handles"][0], "labelSide": "below"})).unwrap();
    assert!(y(&s) < 0.0);
    assert!(s.execute("elecedit", &json!({"handle": r["handles"][0], "labelSide": "left"})).is_err());
}
