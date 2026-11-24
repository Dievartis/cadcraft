//! Design objects: CEILINGADD (the grid of a false ceiling), STAIRADD, RAILADD and DESIGNEDIT, and the terminals and luminaires put at the centre of a tile.

use cadcraft_doc::{CeilingParams, Drawing, EntityKind, Handle, RailParams, StairParams, ceiling_of, rail_of, stair_of};
use cadcraft_engine::Session;
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

fn metric() -> Session {
    let mut s = Session::new();
    *s.doc_mut().unwrap() = Drawing::new_metric();
    s
}

fn hd(v: &Value) -> Handle {
    Handle::parse_hex(v.as_str().unwrap()).unwrap()
}

fn ceiling(s: &Session, h: Handle) -> CeilingParams {
    ceiling_of(s.doc().unwrap().entity(h).unwrap()).expect("a ceiling grid")
}

fn stair(s: &Session, h: Handle) -> StairParams {
    stair_of(s.doc().unwrap().entity(h).unwrap()).expect("a stair")
}

fn rail(s: &Session, h: Handle) -> RailParams {
    rail_of(s.doc().unwrap().entity(h).unwrap()).expect("a railing")
}

fn at(s: &Session, h: Handle) -> Vec2 {
    match &s.doc().unwrap().entity(h).unwrap().kind {
        EntityKind::Insert(i) => i.insert.xy(),
        k => panic!("{k:?}"),
    }
}

#[test]
fn a_ceiling_grid_is_drawn_over_points_a_polyline_or_a_room_and_counts_its_tiles() {
    let mut s = metric();
    let r = s.execute("ceilingadd", &json!({"points": [[0, 0], [3000, 0], [3000, 2400], [0, 2400]], "tile": "600x600", "elevation": 2.7})).unwrap();
    assert_eq!((r["tiles"]["whole"].as_u64(), r["tiles"]["cut"].as_u64(), r["areaM2"].as_f64()), (Some(20), Some(0), Some(7.2)), "{r}");
    let c = ceiling(&s, hd(&r["handle"]));
    assert_eq!((c.name.as_str(), c.tile_x_mm, c.elevation_mm), ("FP1", 600.0, 2700.0));
    // Over a closed polyline, with a long tile typed in centimetres and a name; and over a room of the HVAC tab.
    let pl = s.execute("pline", &json!({"points": [[5000, 0], [8000, 0], [8000, 2400], [5000, 2400]], "closed": true})).unwrap();
    let r2 = s.execute("ceilingadd", &json!({"handle": pl["handle"], "tile": "60x120", "name": "Salle"})).unwrap();
    assert_eq!(ceiling(&s, hd(&r2["handle"])).tile_y_mm, 1200.0);
    let room = s.execute("spaceadd", &json!({"points": [[0, 5000], [4000, 5000], [4000, 8000], [0, 8000]], "name": "Bureau"})).unwrap();
    let r3 = s.execute("ceilingadd", &json!({"space": room["handle"]})).unwrap();
    assert!(r3["areaM2"].as_f64().unwrap() > 11.0, "{r3}");
    // Nothing to draw over, a flat room, a silly tile and a tile too small for the room are refused with nothing added.
    let n = s.doc().unwrap().model.len();
    for bad in [
        json!({}),
        json!({"points": [[0, 0], [1, 0], [2, 0]]}),
        json!({"points": [[0, 0], [1000, 0], [1000, 1000]], "tile": "x"}),
        json!({"points": [[0, 0], [1000, 0], [1000, 1000]], "tile": "5x5000000"}),
        json!({"points": [[0, 0], [3000, 0], [3000, 2400]], "angle": "x"}),
        json!({"points": [[0, 0], [3.0e9, 0], [3.0e9, 3.0e9]]}),
        json!({"space": "0"}),
    ] {
        assert!(s.execute("ceilingadd", &bad).is_err(), "{bad}");
    }
    assert_eq!(s.doc().unwrap().model.len(), n);
}

#[test]
fn terminals_and_luminaires_go_to_the_centre_of_the_ceiling_tile_when_asked() {
    let mut s = metric();
    s.execute("ceilingadd", &json!({"points": [[1000, 500], [4000, 500], [4000, 2900], [1000, 2900]]})).unwrap();
    // Tiles start at the corner (1000, 500): the one under (1100, 600) is centred on (1300, 800).
    let d = s.execute("airterminaladd", &json!({"at": [1100, 600], "ceilingSnap": true, "flow": 100})).unwrap();
    assert!(at(&s, hd(&d["handles"][0])).near(Vec2::new(1300.0, 800.0), 1e-9), "{:?}", at(&s, hd(&d["handles"][0])));
    let free = s.execute("airterminaladd", &json!({"at": [1100, 600], "flow": 100})).unwrap();
    assert!(at(&s, hd(&free["handles"][0])).near(Vec2::new(1100.0, 600.0), 1e-9), "without the option it stays where it is");
    let l = s.execute("elecdeviceadd", &json!({"at": [3950, 2850], "kind": "recessedLight600x600", "ceilingSnap": true})).unwrap();
    assert!(at(&s, hd(&l["handles"][0])).near(Vec2::new(3700.0, 2600.0), 1e-9));
    // Outside every ceiling the point is kept.
    let out = s.execute("elecdeviceadd", &json!({"at": [9000, 9000], "kind": "socket", "ceilingSnap": true})).unwrap();
    assert!(at(&s, hd(&out["handles"][0])).near(Vec2::new(9000.0, 9000.0), 1e-9));
}

#[test]
fn a_stair_has_its_risers_and_says_when_it_is_uncomfortable_and_is_edited() {
    let mut s = metric();
    let r = s.execute("stairadd", &json!({"at": [0, 0], "angle": 90, "rise": 3000, "name": "Escalier A"})).unwrap();
    assert_eq!((r["risers"].as_u64(), r["comfortable"].as_bool()), (Some(17), Some(true)), "{r}");
    let h = hd(&r["handle"]);
    assert_eq!(stair(&s, h).name, "Escalier A");
    // Shallow treads: told, and still drawn.
    let u = s.execute("stairadd", &json!({"at": [5000, 0], "going": 200})).unwrap();
    assert_eq!(u["comfortable"], false);
    // Edited: a higher floor needs more risers; the arrow turns down; the label goes.
    s.execute("designedit", &json!({"handle": h.hex(), "rise": 3400, "up": false, "showLabel": false, "width": 1200})).unwrap();
    let st = stair(&s, h);
    assert_eq!((st.risers(), st.up, st.show_label, st.width_mm), (19, false, false, 1200.0));
    for bad in [
        json!({"at": [0, 0], "rise": 0}),
        json!({"at": [0, 0], "width": "wide"}),
        json!({"at": [0, 0], "going": 5}),
        json!({"angle": 10}),
        json!({"at": [0, 0], "up": "yes"}),
    ] {
        assert!(s.execute("stairadd", &bad).is_err(), "{bad}");
    }
    // What does not apply to a stair is refused, with nothing changed.
    assert!(s.execute("designedit", &json!({"handle": h.hex(), "tile": "600x600"})).is_err());
    assert!(s.execute("designedit", &json!({"handle": h.hex(), "height": 900})).is_err());
    assert_eq!(stair(&s, h).rise_mm, 3400.0);
}

#[test]
fn a_railing_has_posts_and_a_height_and_commands_work_from_the_command_line() {
    let mut s = metric();
    let r = s.execute("railadd", &json!({"points": [[0, 0], [3000, 0], [3000, 1200]], "height": 1100})).unwrap();
    assert_eq!((r["posts"].as_u64(), r["length"].as_f64()), (Some(5), Some(4.2)), "{r}");
    let h = hd(&r["handle"]);
    assert_eq!(rail(&s, h).height_mm, 1100.0);
    s.execute("designedit", &json!({"handle": h.hex(), "postSpacing": 600, "height": 900, "name": "Garde-corps escalier"})).unwrap();
    let p = rail(&s, h);
    assert_eq!((p.post_spacing_mm, p.height_mm, p.name.as_str()), (600.0, 900.0, "Garde-corps escalier"));
    for bad in [
        json!({"points": [[0, 0]]}),
        json!({"points": [[0, 0], [0, 0]]}),
        json!({"points": [[0, 0], [1000, 0]], "height": 10}),
        json!({"points": "x"}),
    ] {
        assert!(s.execute("railadd", &bad).is_err(), "{bad}");
    }
    // At the command line: a ceiling over four corners, a stair from a point along its way up, a railing along three points.
    for t in ["ceilingadd", "t", "60x120", "0,10000", "3000,10000", "3000,12400", "0,12400", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none(), "{:?}", s.log);
    assert!(s.doc().unwrap().model.iter().any(|e| ceiling_of(e).is_some_and(|c| c.tile_y_mm == 1200.0)));
    for t in ["stairadd", "w", "1200", "10000,0", "10000,6000"] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none(), "{:?}", s.log);
    let st = s.doc().unwrap().model.iter().find_map(|e| stair_of(e).filter(|x| x.width_mm == 1200.0)).expect("the stair, 1200 wide");
    assert_eq!(st.risers(), 17);
    for t in ["railadd", "h", "1050", "0,20000", "2000,20000", "2000,22000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.doc().unwrap().model.iter().any(|e| rail_of(e).is_some_and(|x| x.height_mm == 1050.0 && x.path.len() == 3)));
}

#[test]
fn the_takeoff_counts_the_ceiling_tiles_the_stairs_and_the_railings_too() {
    let mut s = metric();
    s.execute("ceilingadd", &json!({"points": [[0, 0], [3100, 0], [3100, 2500], [0, 2500]]})).unwrap();
    s.execute("stairadd", &json!({"at": [5000, 0], "rise": 3000})).unwrap();
    s.execute("stairadd", &json!({"at": [9000, 0], "rise": 1800})).unwrap();
    s.execute("railadd", &json!({"points": [[0, 4000], [3000, 4000]]})).unwrap();
    let t = s.execute("structtakeoff", &json!({})).unwrap();
    assert_eq!((t["ceilings"][0]["whole"].as_u64(), t["ceilings"][0]["cut"].as_u64()), (Some(20), Some(10)), "{t}");
    // 17 and 10 risers: two stairs and 27 risers; one railing of 3 m.
    assert_eq!(
        (t["stairs"].as_u64(), t["risers"].as_u64(), t["railings"].as_u64(), t["railingLengthM"].as_f64()),
        (Some(2), Some(27), Some(1), Some(3.0)),
        "{t}"
    );
}

#[test]
fn a_curtain_wall_is_drawn_counted_and_edited() {
    use cadcraft_doc::curtain_of;
    let mut s = metric();
    let r = s.execute("curtainadd", &json!({"points": [[0, 0], [6000, 0], [6000, 3000]], "height": 3600, "panel": 1500})).unwrap();
    // 9 m at 1500: 6 bays, 7 mullions; 3.6 m at 1200: 3 rows, 18 panels; 32.4 m².
    assert_eq!((r["panels"].as_u64(), r["mullions"].as_u64(), r["transoms"].as_u64()), (Some(18), Some(7), Some(2)), "{r}");
    assert!((r["area"].as_f64().unwrap() - 32.4).abs() < 1e-9);
    let h = Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap();
    s.execute("curtainedit", &json!({"handle": r["handle"], "panel": 3000, "name": "FAC"})).unwrap();
    let c = curtain_of(s.doc().unwrap().entity(h).unwrap()).expect("a curtain wall");
    assert_eq!((c.name.as_str(), c.panel_mm), ("FAC", 3000.0));
    for bad in
        [json!({"points": [[0, 0]]}), json!({"points": [[0, 0], [1000, 0]], "panel": 5}), json!({"points": [[0, 0], [1000, 0]], "height": "tall"})]
    {
        assert!(s.execute("curtainadd", &bad).is_err(), "{bad}");
    }
    assert!(s.execute("curtainedit", &json!({"handle": "FFFF", "panel": 1500})).is_err());
    // The interactive form draws the same.
    for t in ["curtainadd", "0,5000", "4000,5000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.doc().unwrap().model.iter().filter(|e| curtain_of(e).is_some()).count() == 2);
    // The takeoff counts them: the first wall is now 3000 mm panels (3 bays, 4 mullions), 3.6 m high.
    let t = s.execute("structtakeoff", &json!({})).unwrap();
    assert_eq!(t["curtainWalls"].as_u64(), Some(2), "{t}");
    assert!(t["curtainAreaM2"].as_f64().unwrap() > 32.0, "{t}");
}

#[test]
fn a_roof_is_drawn_from_points_or_from_a_room_and_edited() {
    use cadcraft_doc::{RoofKind, roof_of};
    let mut s = metric();
    let r = s.execute("roofadd", &json!({"points": [[0, 0], [10000, 0], [10000, 6000], [0, 6000]], "pitch": 30})).unwrap();
    let t30 = 30f64.to_radians().tan();
    assert!((r["rise"].as_f64().unwrap() - 3.0 * t30).abs() < 1e-9 && (r["ridge"].as_f64().unwrap() - 10.0).abs() < 1e-9, "{r}");
    assert!((r["planArea"].as_f64().unwrap() - 60.0).abs() < 1e-9);
    let h = Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap();
    s.execute("roofedit", &json!({"handle": r["handle"], "kind": "hip", "pitch": 40, "name": "TOIT"})).unwrap();
    let p = roof_of(s.doc().unwrap().entity(h).unwrap()).expect("a roof");
    assert_eq!((p.kind, p.pitch_deg, p.name.as_str()), (RoofKind::Hip, 40.0, "TOIT"));
    for bad in [
        json!({"points": [[0, 0], [1000, 0]]}),
        json!({"points": [[0, 0], [1000, 0], [0, 1000]], "pitch": 95}),
        json!({"points": [[0, 0], [1000, 0], [0, 1000]], "kind": "dome"}),
    ] {
        assert!(s.execute("roofadd", &bad).is_err(), "{bad}");
    }
    assert!(s.execute("roofedit", &json!({"handle": "FFFF", "pitch": 20})).is_err());
    // From a room closed by walls.
    s.execute("walladd", &json!({"points": [[20000, 0], [26000, 0], [26000, 4000], [20000, 4000], [20000, 0]], "thickness": 200})).unwrap();
    let w = s.execute("roofadd", &json!({"inside": [23000, 2000]})).unwrap();
    assert!((20.0..27.0).contains(&w["planArea"].as_f64().unwrap()), "{w}");
    // The interactive form.
    for t in ["roofadd", "0,10000", "8000,10000", "8000,14000", ""] {
        s.cmdline(t).unwrap();
    }
    assert_eq!(s.doc().unwrap().model.iter().filter(|e| roof_of(e).is_some()).count(), 3);
}

#[test]
fn a_roof_stands_out_beyond_its_walls_by_its_overhang_given_edited_or_asked_at_the_command_line() {
    use cadcraft_doc::roof_of;
    let mut s = metric();
    let walls = json!([[0, 0], [10000, 0], [10000, 6000], [0, 6000]]);
    let plain = s.execute("roofadd", &json!({"points": walls, "pitch": 30})).unwrap();
    let wide =
        s.execute("roofadd", &json!({"points": [[0, 10000], [10000, 10000], [10000, 16000], [0, 16000]], "pitch": 30, "overhang": 500})).unwrap();
    assert!((plain["planArea"].as_f64().unwrap() - 60.0).abs() < 1e-9 && (wide["planArea"].as_f64().unwrap() - 77.0).abs() < 1e-9, "{plain} {wide}");
    assert_eq!((plain["rise"].as_f64(), plain["ridge"].as_f64()), (wide["rise"].as_f64(), Some(10.0)), "the top does not move");
    assert!((wide["ridge"].as_f64().unwrap() - 11.0).abs() < 1e-9, "the ridge runs to the edge of the roof");
    // Edited: 1 m instead, then none.
    s.execute("roofedit", &json!({"handle": wide["handle"], "overhang": 1000})).unwrap();
    let h = Handle::parse_hex(wide["handle"].as_str().unwrap()).unwrap();
    assert_eq!(roof_of(s.doc().unwrap().entity(h).unwrap()).unwrap().overhang_mm, 1000.0);
    s.execute("roofedit", &json!({"handle": wide["handle"], "overhang": 0})).unwrap();
    assert_eq!(roof_of(s.doc().unwrap().entity(h).unwrap()).unwrap().overhang_mm, 0.0);
    for bad in [json!({"overhang": -1}), json!({"overhang": 6000})] {
        assert!(s.execute("roofedit", &json!({"handle": wide["handle"], "overhang": bad["overhang"]})).is_err(), "{bad}");
    }
    // At the command line: the option Overhang, remembered for the next roof.
    for t in ["roofadd", "overhang", "400", "0,30000", "6000,30000", "6000,34000", "0,34000", "close"] {
        s.cmdline(t).unwrap();
    }
    let last = s.doc().unwrap().model.iter().filter_map(|e| roof_of(e)).find(|r| r.overhang_mm == 400.0);
    assert!(last.is_some(), "a roof with the overhang typed");
}
