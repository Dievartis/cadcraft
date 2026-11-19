//! The plumbing tab: sanitary fixtures, the waste, soil and supply networks, and the branches FIXTURECONNECT draws.

use cadcraft_doc::{Drawing, EntityKind, Handle, PipeService, SanitaryKind, pipe_of, sanitary_of};
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

fn label_of(s: &Session, h: Handle) -> Vec<String> {
    let d = s.doc().unwrap();
    let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { return Vec::new() };
    d.block(&i.block).unwrap().entities.iter().filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None }).collect()
}

#[test]
fn every_kind_of_fixture_is_put_at_its_size_with_its_drain_written_beside_it() {
    let mut s = metric();
    for (k, kind) in SanitaryKind::ALL.iter().enumerate() {
        let r = s.execute("fixtureadd", &json!({"at": [k as f64 * 2000.0, 0.0], "kind": kind.name()})).unwrap();
        let h = hd(&r["handles"][0]);
        let f = sanitary_of(s.doc().unwrap().entity(h).unwrap()).unwrap();
        assert_eq!(f.kind, *kind);
        assert_eq!((f.width_mm, f.depth_mm), kind.default_size_mm());
        assert!(!label_of(&s, h).is_empty(), "{kind:?}");
    }
    let wc = s.execute("fixtureadd", &json!({"at": [0, 5000], "kind": "wc"})).unwrap();
    assert_eq!(label_of(&s, hd(&wc["handles"][0])), ["WC EV \u{d8}100"]);
    let basin = s.execute("fixtureadd", &json!({"at": [0, 8000], "kind": "lavabo", "name": "LAV1", "altitude": 0.8})).unwrap();
    let f = sanitary_of(s.doc().unwrap().entity(hd(&basin["handles"][0])).unwrap()).unwrap();
    assert_eq!((f.drain, f.drain_dn_mm, f.cold_ls, f.hot_ls, f.bottom_mm), (Some(PipeService::WasteWater), 40.0, Some(0.2), Some(0.2), Some(800.0)));
    assert_eq!(label_of(&s, hd(&basin["handles"][0])), ["LAV1 EU \u{d8}40"]);
    assert!(s.execute("fixtureadd", &json!({"at": [0, 0], "kind": "piano"})).is_err());
    assert!(s.execute("fixtureadd", &json!({"at": [0, 0], "width": 1e9})).is_err());
    assert!(s.execute("fixtureadd", &json!({"at": [0, 0], "drain": "chilledWater"})).is_err());
}

#[test]
fn fixtureedit_changes_the_kind_the_size_the_drain_and_the_altitude() {
    let mut s = metric();
    let r = s.execute("fixtureadd", &json!({"at": [0, 0], "kind": "shower"})).unwrap();
    let h = r["handles"][0].clone();
    let f = |s: &Session| sanitary_of(s.doc().unwrap().entity(hd(&h)).unwrap()).unwrap();
    s.execute("fixtureedit", &json!({"handle": h, "width": 1200, "depth": 800, "altitude": null})).unwrap();
    assert_eq!((f(&s).width_mm, f(&s).depth_mm, f(&s).bottom_mm), (1200.0, 800.0, None));
    s.execute("fixtureedit", &json!({"handle": h, "kind": "bath"})).unwrap();
    assert_eq!((f(&s).kind, f(&s).width_mm), (SanitaryKind::Bath, 1700.0));
    s.execute("fixtureedit", &json!({"handle": h, "drain": "soilWater", "drainDn": 100})).unwrap();
    assert_eq!((f(&s).drain, f(&s).drain_dn_mm), (Some(PipeService::SoilWater), 100.0));
    assert!(s.execute("fixtureedit", &json!({"handle": h, "drainDn": 5})).is_err());
}

#[test]
fn fixtureconnect_draws_each_branch_to_the_nearest_drain_of_its_kind_with_its_size_and_slope() {
    let mut s = metric();
    // A waste-water collector and a soil-water one along x, a basin and a water closet above them, a shower with no collector in reach.
    s.execute("pipeadd", &json!({"points": [[0, 0], [6000, 0]], "service": "eu", "dn": 50, "slope": 1})).unwrap();
    s.execute("pipeadd", &json!({"points": [[0, -1000], [6000, -1000]], "service": "ev", "dn": 100})).unwrap();
    s.execute("fixtureadd", &json!({"at": [1000, 2000], "kind": "washbasin"})).unwrap();
    s.execute("fixtureadd", &json!({"at": [4000, 2000], "kind": "wcWallHung"})).unwrap();
    s.execute("fixtureadd", &json!({"at": [50000, 50000], "kind": "shower"})).unwrap();
    let r = s.execute("fixtureconnect", &json!({})).unwrap();
    assert_eq!(r["created"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(r["skipped"].as_array().unwrap().len(), 1, "the shower has no drain near: {r}");
    let made: Vec<_> = r["created"].as_array().unwrap().iter().map(|h| pipe_of(s.doc().unwrap().entity(hd(h)).unwrap()).unwrap()).collect();
    let basin = made.iter().find(|p| p.service == PipeService::WasteWater).unwrap();
    assert_eq!((basin.dn_mm, basin.slope_pct), (40.0, Some(1.0)));
    let wc = made.iter().find(|p| p.service == PipeService::SoilWater).unwrap();
    assert_eq!((wc.dn_mm, wc.slope_pct), (100.0, Some(2.0)));
    // The basin's branch goes down to the waste-water collector at y = 0, not to the soil-water one.
    let ends = s.doc().unwrap().pipe_ends(hd(&r["created"][0])).map(|(_, a, b)| (a, b)).unwrap();
    assert!(ends.1.y.abs() < 1e-6 || ends.1.y + 1000.0 < 1e-6, "{ends:?}");
    assert!(label_of(&s, hd(&r["created"][0])).iter().chain(label_of(&s, hd(&r["created"][1])).iter()).any(|t| t.contains("i=1 %")));
    // Done again, nothing is left to connect.
    let again = s.execute("fixtureconnect", &json!({})).unwrap();
    assert_eq!(again["created"].as_array().unwrap().len(), 0, "{again}");
    assert!(s.execute("fixtureconnect", &json!({"slope": 500})).is_err());
}

#[test]
fn a_plumbing_pipe_is_written_with_its_outer_diameter_its_service_and_its_slope() {
    let mut s = metric();
    let r = s.execute("pipeadd", &json!({"points": [[0, 0], [3000, 0]], "service": "eu", "dn": 100, "slope": 1.5})).unwrap();
    let h = hd(&r["handles"][0]);
    assert!(label_of(&s, h).iter().any(|t| t.starts_with("\u{d8}100 EU i=1.5 %")), "{:?}", label_of(&s, h));
    s.execute("pipeedit", &json!({"handle": r["handles"][0], "slope": null})).unwrap();
    assert!(label_of(&s, h).iter().all(|t| !t.contains("i=")), "{:?}", label_of(&s, h));
    for (service, tag) in [("ef", "EF"), ("ecs", "ECS"), ("bouclage", "BECS"), ("ep", "EP"), ("ev", "EV")] {
        let r = s.execute("pipeadd", &json!({"points": [[0, 2000], [3000, 2000]], "service": service, "dn": 20})).unwrap();
        assert!(
            label_of(&s, hd(&r["handles"][0])).iter().any(|t| t == &format!("\u{d8}20 {tag}")),
            "{service}: {:?}",
            label_of(&s, hd(&r["handles"][0]))
        );
    }
}

#[test]
fn the_branches_a_command_draws_are_not_collectors_for_the_fixtures_after_them() {
    let mut s = metric();
    s.execute("pipeadd", &json!({"points": [[0, 0], [6000, 0]], "service": "eu", "dn": 50})).unwrap();
    for x in [500.0, 2500.0, 4500.0] {
        s.execute("fixtureadd", &json!({"at": [x, 3000], "kind": "washbasin"})).unwrap();
    }
    let r = s.execute("fixtureconnect", &json!({})).unwrap();
    assert_eq!(r["created"].as_array().unwrap().len(), 3, "{r}");
    // Each branch is square to the collector, straight down from its fixture: none is slanted toward the branch of a fixture beside it.
    for h in r["created"].as_array().unwrap() {
        let (_, a, b) = s.doc().unwrap().pipe_ends(hd(h)).unwrap();
        assert!((a.x - b.x).abs() < 1e-6 && b.y.abs() < 1e-6, "{a:?} {b:?}");
    }
}
