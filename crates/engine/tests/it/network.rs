//! PIPENETWORK names the networks and gives each a colour; NETWORKLEGEND draws their legend.

use cadcraft_doc::{Drawing, Handle, NetworkLegendParams, network_legend_of, pipe_of};
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

fn network(s: &Session, h: Handle) -> String {
    pipe_of(s.doc().unwrap().entity(h).unwrap()).unwrap().network
}

/// A chilled-water loop at height `y`: a chiller, a unit, and a supply and a return between them. Returns the two pipes.
fn loop_at(s: &mut Session, y: f64) -> (Handle, Handle) {
    s.execute("equipmentadd", &json!({"at": [0, y], "kind": "chiller", "capacityKw": 50})).unwrap();
    s.execute("equipmentadd", &json!({"at": [6000, y], "kind": "cassette", "capacityKw": 5, "length": 800, "width": 600, "height": 300})).unwrap();
    let a = s.execute("pipeadd", &json!({"points": [[0, y], [5800, y]], "service": "chilledWater", "leg": "supply"})).unwrap();
    let b = s.execute("pipeadd", &json!({"points": [[5800, y + 200.0], [0, y + 200.0]], "service": "chilledWater", "leg": "return"})).unwrap();
    (hd(&a["handles"][0]), hd(&b["handles"][0]))
}

#[test]
fn pipenetwork_names_every_network_and_gives_each_a_colour_of_its_own() {
    let mut s = metric();
    let (s1, r1) = loop_at(&mut s, 0.0);
    let (s2, _) = loop_at(&mut s, 4000.0);
    let r = s.execute("pipenetwork", &json!({})).unwrap();
    let nets = r["networks"].as_array().unwrap();
    assert_eq!(nets.iter().map(|n| n["name"].as_str().unwrap()).collect::<Vec<_>>(), ["CHW1", "CHW2"]);
    assert_ne!(nets[0]["color"], nets[1]["color"]);
    assert!(nets.iter().all(|n| n["linetype"] == "CONTINUOUS"), "the colour tells them apart");
    assert_eq!(nets[0]["services"], json!(["CHW-S", "CHW-R"]));
    assert_eq!((network(&s, s1), network(&s, r1), network(&s, s2)), ("CHW1".into(), "CHW1".into(), "CHW2".into()));
    assert_eq!(s.doc().unwrap().entity(s1).unwrap().common.layer, "M-PIPE-CHWS-CHW1");
    // A pipe drawn on a named network takes its name by itself; a part put on it too.
    let extra = s.execute("pipeadd", &json!({"points": [[5800, 0], [5800, -2000]], "service": "chilledWater"})).unwrap();
    assert_eq!(network(&s, hd(&extra["handles"][0])), "CHW1");
    let v = s.execute("pipepartadd", &json!({"at": [3000, 0], "model": "Isolation valve (ball)"})).unwrap();
    let valve = cadcraft_doc::part_of(s.doc().unwrap().entity(hd(&v["handle"])).unwrap()).unwrap();
    assert_eq!(valve.network, "CHW1");
    assert_eq!(s.doc().unwrap().entity(hd(&v["handle"])).unwrap().common.layer, "M-PIPE-ACC-CHW1");
    // Naming again changes nothing.
    assert_eq!(s.execute("pipenetwork", &json!({})).unwrap()["networks"].as_array().unwrap().len(), 2);
}

#[test]
fn a_name_a_rename_and_a_style_are_given_by_the_command_or_by_pipeedit() {
    let mut s = metric();
    let (s1, r1) = loop_at(&mut s, 0.0);
    let (s2, _) = loop_at(&mut s, 4000.0);
    // The name typed for one pipe is the name of its whole network.
    s.execute("pipenetwork", &json!({"handle": s1.hex(), "name": "Toiture 1"})).unwrap();
    assert_eq!((network(&s, s1), network(&s, r1), network(&s, s2)), ("Toiture 1".into(), "Toiture 1".into(), String::new()));
    s.execute("pipeedit", &json!({"handle": s2.hex(), "network": "Étage 3"})).unwrap();
    assert_eq!(network(&s, s2), "Étage 3");
    // The style: a palette name or number, an rgb text, a line type.
    let st = s.execute("pipenetwork", &json!({"style": {"name": "toiture 1", "color": "Vermillion", "linetype": "NET_DASH"}})).unwrap();
    assert_eq!(
        (st["style"]["name"].as_str(), st["style"]["color"].as_str(), st["style"]["linetype"].as_str()),
        (Some("Toiture 1"), Some("#d55e00"), Some("NET_DASH"))
    );
    assert_eq!(s.execute("pipenetwork", &json!({"style": {"name": "Toiture 1", "color": 3}})).unwrap()["style"]["color"], "#009e73");
    assert_eq!(s.execute("pipenetwork", &json!({"style": {"name": "Toiture 1", "color": "#102030"}})).unwrap()["style"]["color"], "#102030");
    assert_eq!(s.execute("pipenetwork", &json!({"style": {"name": "Toiture 1", "color": [1, 2, 3]}})).unwrap()["style"]["color"], "#010203");
    let l = s.doc().unwrap().layer("M-PIPE-CHWS-TOITURE_1").unwrap().clone();
    assert_eq!(l.linetype, "NET_DASH");
    assert_eq!(l.color, cadcraft_color::Color::True(cadcraft_color::Rgb(1, 2, 3)));
    // Renamed.
    assert_eq!(s.execute("pipenetwork", &json!({"rename": {"from": "Toiture 1", "to": "Toiture"}})).unwrap()["changed"], 2);
    assert_eq!(network(&s, r1), "Toiture");
    // The list.
    let l = s.execute("pipenetwork", &json!({"list": true})).unwrap();
    assert_eq!(l["networks"].as_array().unwrap().len(), 2);
    assert_eq!((l["palette"].as_array().unwrap().len(), l["linetypes"].as_array().unwrap().len()), (12, 5));
    for bad in [
        json!({"handle": s1.hex()}),
        json!({"handle": s1.hex(), "name": ""}),
        json!({"handle": "0", "name": "x"}),
        json!({"style": {"color": 1}}),
        json!({"style": {"name": "x", "color": "nonsense"}}),
        json!({"style": {"name": "x", "color": 13}}),
        json!({"style": {"name": "x", "linetype": "NOT_A_LINETYPE"}}),
        json!({"rename": {"from": "x"}}),
    ] {
        assert!(s.execute("pipenetwork", &bad).is_err(), "{bad}");
    }
}

#[test]
fn networklegend_draws_the_legend_and_it_follows_the_networks() {
    let mut s = metric();
    loop_at(&mut s, 0.0);
    s.execute("pipenetwork", &json!({})).unwrap();
    let r = s.execute("networklegend", &json!({"at": [0, 9000], "title": "Réseaux"})).unwrap();
    let legend = |s: &Session| network_legend_of(s.doc().unwrap().entity(hd(&r["handle"])).unwrap()).unwrap();
    assert_eq!(legend(&s), NetworkLegendParams { title: "Réseaux".into(), ..NetworkLegendParams::new(cadcraft_doc::HvacUnits::default()) });
    // Interactive: a click puts one there.
    for t in ["networklegend", "t", "Légende", "0,12000"] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let all: Vec<_> = s.doc().unwrap().model.iter().filter_map(|e| network_legend_of(e)).collect();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|l| l.title == "Légende"));
    assert!(s.execute("networklegend", &json!({})).is_err());
    assert!(s.execute("networklegend", &json!({"at": [0, 0], "labelHeight": -1})).is_err());
}
