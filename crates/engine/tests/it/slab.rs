//! SLABADD draws a slab over a closed outline; SLABEDIT changes it.

use cadcraft_doc::{Drawing, Handle, SlabKind, SlabParams, slab_of};
use cadcraft_engine::Session;
use serde_json::{Value, json};

fn metric() -> Session {
    let mut s = Session::new();
    *s.doc_mut().unwrap() = Drawing::new_metric();
    s
}

fn slab(s: &Session, v: &Value) -> SlabParams {
    slab_of(s.doc().unwrap().entity(Handle::parse_hex(v.as_str().unwrap()).unwrap()).unwrap()).expect("a slab")
}

#[test]
fn slabadd_draws_a_slab_and_slabedit_changes_it() {
    let mut s = metric();
    let r = s.execute("slabadd", &json!({"points": [[0, 0], [5000, 0], [5000, 4000], [0, 4000]], "kind": "roof", "elevation": 6.0})).unwrap();
    assert_eq!((r["area"].as_f64(), r["volume"].as_f64()), (Some(20.0), Some(5.0)));
    let p = slab(&s, &r["handle"]);
    assert_eq!((p.kind, p.thickness_mm, p.elevation_mm), (SlabKind::Roof, 250.0, 6000.0));
    s.execute("slabedit", &json!({"handle": r["handle"], "kind": "floor", "thickness": 180})).unwrap();
    let p = slab(&s, &r["handle"]);
    assert_eq!((p.kind, p.thickness_mm), (SlabKind::Floor, 180.0));
    assert!(s.execute("slabadd", &json!({"points": [[0, 0], [1, 0], [2, 0]]})).is_err(), "a flat outline");
    assert!(s.execute("slabadd", &json!({"points": [[0, 0], [1, 0]]})).is_err());
    assert!(s.execute("slabedit", &json!({"handle": "0"})).is_err());
}

#[test]
fn slabadd_at_the_command_line_takes_the_options_then_the_points_and_remembers_them() {
    let mut s = metric();
    for t in ["slabadd", "k", "foundation", "t", "500", "0,0", "3000,0", "3000,3000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let slabs: Vec<SlabParams> = s.doc().unwrap().model.iter().filter_map(|e| slab_of(e)).collect();
    assert_eq!(slabs.len(), 1);
    assert_eq!((slabs[0].kind, slabs[0].thickness_mm), (SlabKind::Foundation, 500.0));
    for t in ["slabadd", "0,5000", "1000,5000", "1000,6000", ""] {
        s.cmdline(t).unwrap();
    }
    let last = s.doc().unwrap().model.iter().filter_map(|e| slab_of(e)).last().unwrap();
    assert_eq!((last.kind, last.thickness_mm), (SlabKind::Foundation, 500.0));
}

fn holes(s: &Session, v: &Value) -> usize {
    slab(s, v).holes.len()
}

#[test]
fn a_slab_takes_openings_a_slope_and_the_shafts_of_the_risers_inside_it() {
    let mut s = metric();
    let r = s.execute("slabadd", &json!({"points": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]]})).unwrap();
    let h = &r["handle"];
    // An opening typed in, then cleared; the area follows.
    let a = s.execute("slabedit", &json!({"handle": h, "hole": [[1000, 1000], [2000, 1000], [2000, 2000], [1000, 2000]]})).unwrap();
    assert_eq!(a["holes"], 1);
    assert_eq!(holes(&s, h), 1);
    assert!((slab(&s, h).area_m2() - 47.0).abs() < 1e-9);
    s.execute("slabedit", &json!({"handle": h, "clearHoles": true})).unwrap();
    assert_eq!(holes(&s, h), 0);
    // An opening outside the slab, a flat one and a short one are refused with nothing changed.
    for bad in [
        json!({"handle": h, "hole": [[9000, 1000], [9500, 1000], [9500, 2000]]}),
        json!({"handle": h, "hole": [[1000, 1000], [2000, 1000]]}),
        json!({"handle": h, "hole": "x"}),
        json!({"handle": h, "slope": 500}),
        json!({"handle": h, "slope": "steep"}),
    ] {
        assert!(s.execute("slabedit", &bad).is_err(), "{bad}");
    }
    assert_eq!(holes(&s, h), 0);
    // A slope falls towards a direction and is told on the slab.
    s.execute("slabedit", &json!({"handle": h, "slope": 1.5, "slopeDirection": 90})).unwrap();
    assert_eq!((slab(&s, h).slope_pct, slab(&s, h).slope_direction_deg), (1.5, 90.0));
    // The shafts of the risers inside it become openings, once each; one outside it does not.
    s.execute("riseradd", &json!({"at": [4000, 3000], "width": 800, "depth": 500, "direction": "up", "air": "supply"})).unwrap();
    s.execute("riseradd", &json!({"at": [20000, 3000], "width": 800, "depth": 500, "direction": "up", "air": "supply"})).unwrap();
    let r2 = s.execute("slabhole", &json!({"handle": h, "riserHoles": true})).unwrap();
    assert_eq!(r2["holes"], 1);
    assert_eq!(s.execute("slabhole", &json!({"handle": h, "riserHoles": true})).unwrap()["holes"], 1, "the same shaft is not cut twice");
    let area = slab(&s, h).area_m2();
    assert!((area - (48.0 - 0.4)).abs() < 1e-9, "{area}");
    assert!(s.execute("slabhole", &json!({"handle": h})).is_err());
    assert!(s.execute("slabhole", &json!({"handle": "0", "riserHoles": true})).is_err());
    // At the command line: the slab, then the corners of an opening.
    let hd = Handle::parse_hex(h.as_str().unwrap()).unwrap();
    s.set_selection(vec![hd]);
    for t in ["slabhole", "6000,4000", "7000,4000", "7000,5000", "6000,5000", ""] {
        s.cmdline(t).unwrap();
    }
    assert_eq!(holes(&s, h), 2, "{:?}", s.log);
}
