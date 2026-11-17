//! Columns and beams in steel sections: COLUMNADD, BEAMADD, STRUCTEDIT and STRUCTCLEANUP, from JSON and from the command line.

use cadcraft_doc::{Drawing, EntityKind, Handle, StructKind, StructParams, struct_of};
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

fn object(s: &Session, h: Handle) -> StructParams {
    struct_of(s.doc().unwrap().entity(h).expect("the object is in the drawing")).expect("a structural object")
}

fn at(s: &Session, h: Handle) -> Vec2 {
    match &s.doc().unwrap().entity(h).unwrap().kind {
        EntityKind::Insert(i) => i.insert.xy(),
        k => panic!("{k:?}"),
    }
}

fn count(s: &Session) -> usize {
    s.doc().unwrap().model.iter().filter(|e| struct_of(e).is_some()).count()
}

#[test]
fn columns_are_drawn_with_their_section_height_and_foot_and_beams_stop_at_their_faces() {
    let mut s = metric();
    let r = s
        .execute("columnadd", &json!({"points": [[0, 0], [5000, 0]], "section": "HEA 200", "height": 3600, "elevation": -0.15, "rotation": 0}))
        .unwrap();
    let (c1, c2) = (hd(&r["handles"][0]), hd(&r["handles"][1]));
    let p = object(&s, c1);
    assert_eq!((p.kind, p.name.as_str(), p.section.as_str(), p.height_mm, p.elevation_mm), (StructKind::Column, "C1", "HEA 200", 3600.0, -150.0));
    assert_eq!(object(&s, c2).name, "C2");
    // A beam from one column's centre to the other's: it stops at their faces, 100 mm from each centre.
    let r = s.execute("beamadd", &json!({"points": [[0, 0], [5000, 0]], "section": "IPE 240", "elevation": 3.0})).unwrap();
    assert_eq!(r["trimmed"], 1, "{r}");
    let b = object(&s, hd(&r["handles"][0]));
    assert_eq!((b.kind, b.name.as_str(), b.section.as_str(), b.elevation_mm), (StructKind::Beam, "B1", "IPE 240", 3000.0));
    assert_eq!((b.trim_start_mm, b.trim_end_mm), (100.0, 100.0));
    // The top of the beam is its underside and its section's height; its mass its length by 30.7 kg/m.
    assert_eq!(b.top_mm(), 3240.0);
    assert!((b.mass_kg() - 5.0 * 30.7).abs() < 1e-9);
    // Undo takes the beam back, and the columns stay; redo draws it again.
    s.execute("undo", &json!({})).unwrap();
    assert_eq!(count(&s), 2);
    s.execute("redo", &json!({})).unwrap();
    assert_eq!(count(&s), 3);
}

#[test]
fn bad_sections_and_numbers_change_nothing_and_say_so() {
    let mut s = metric();
    for (cmd, params) in [
        ("columnadd", json!({"at": [0, 0], "section": "IPN 205"})),
        ("columnadd", json!({"at": [0, 0], "section": "XYZ 100"})),
        ("columnadd", json!({"at": [0, 0], "height": 0})),
        ("columnadd", json!({"at": [0, 0], "height": "tall"})),
        ("columnadd", json!({"at": [0, 0], "rotation": "x"})),
        ("columnadd", json!({"points": []})),
        ("columnadd", json!({})),
        ("beamadd", json!({"points": [[0, 0]]})),
        ("beamadd", json!({"points": [[0, 0], [0, 0]]})),
        ("beamadd", json!({"points": [[0, 0], [3000, 0]], "section": "HEA 17"})),
        ("beamadd", json!({"points": [[0, 0], [3000, 0]], "elevation": "high"})),
        ("structedit", json!({"handles": []})),
        ("structedit", json!({})),
    ] {
        assert!(s.execute(cmd, &params).is_err(), "{cmd} {params}");
        assert_eq!(count(&s), 0, "{cmd} {params} left something");
    }
}

#[test]
fn a_column_lands_on_the_corner_of_two_walls_and_on_the_end_of_a_beam_when_asked_to() {
    let mut s = metric();
    s.execute("walladd", &json!({"points": [[0, 0], [4000, 0], [4000, 3000]]})).unwrap();
    // 40 mm from the bend of the wall: with `snap` it lands on the corner; without it, it stays where it is.
    let snapped = s.execute("columnadd", &json!({"at": [4040, 30], "snap": 100})).unwrap();
    assert!(at(&s, hd(&snapped["handles"][0])).near(Vec2::new(4000.0, 0.0), 1e-9));
    let free = s.execute("columnadd", &json!({"at": [4040, 30]})).unwrap();
    assert!(at(&s, hd(&free["handles"][0])).near(Vec2::new(4040.0, 30.0), 1e-9));
    // Too far to be taken for the corner or for a column.
    let far = s.execute("columnadd", &json!({"at": [4400, 400], "snap": 100})).unwrap();
    assert!(at(&s, hd(&far["handles"][0])).near(Vec2::new(4400.0, 400.0), 1e-9));
    // A beam ends on the centre of the column nearest to its point.
    let r = s.execute("beamadd", &json!({"points": [[4020, 10], [8000, 0]], "snap": 100})).unwrap();
    let b = hd(&r["handles"][0]);
    assert!(at(&s, b).near(Vec2::new(4000.0, 0.0), 1e-9), "{:?}", at(&s, b));
}

#[test]
fn structures_are_edited_and_the_cut_ends_follow() {
    let mut s = metric();
    let r = s.execute("columnadd", &json!({"points": [[0, 0], [5000, 0]]})).unwrap();
    let (c1, c2) = (hd(&r["handles"][0]), hd(&r["handles"][1]));
    let b = hd(&s.execute("beamadd", &json!({"points": [[0, 0], [5000, 0]]})).unwrap()["handles"][0]);
    // A bigger section for the columns: HEB 300 is 300 wide, the beam is cut 150 mm from each centre.
    s.execute("structedit", &json!({"handles": [c1.hex(), c2.hex()], "section": "HEB 300", "height": 4200, "elevation": 0.5, "name": "P"})).unwrap();
    assert_eq!((object(&s, c1).section.as_str(), object(&s, c1).height_mm, object(&s, c1).elevation_mm), ("HEB 300", 4200.0, 500.0));
    assert_eq!((object(&s, b).trim_start_mm, object(&s, b).trim_end_mm), (150.0, 150.0));
    // A column turned a quarter turn is 300 deep along the beam: the same; HEA 200 turned is 190 along it.
    s.execute("structedit", &json!({"handle": c2.hex(), "section": "HEA 200", "rotation": 90})).unwrap();
    assert_eq!(object(&s, b).trim_end_mm, 95.0);
    // The beam's own section and underside, and its label off.
    s.execute("structedit", &json!({"handle": b.hex(), "section": "IPE 300", "elevation": 3.4, "showLabel": false})).unwrap();
    let p = object(&s, b);
    assert_eq!((p.section.as_str(), p.elevation_mm, p.show_label), ("IPE 300", 3400.0, false));
    // What a beam cannot take, and what is not structural, is refused with nothing changed.
    for params in
        [json!({"handle": b.hex(), "height": 3000}), json!({"handle": b.hex(), "rotation": 10}), json!({"handle": b.hex(), "section": "NOPE 1"})]
    {
        assert!(s.execute("structedit", &params).is_err(), "{params}");
    }
    let line = hd(&s.execute("line", &json!({"points": [[0, 5000], [100, 5000]]})).unwrap()["handles"][0]);
    assert!(s.execute("structedit", &json!({"handle": line.hex(), "section": "IPN 200"})).is_err());
    assert_eq!(object(&s, b).section, "IPE 300");
}

#[test]
fn structcleanup_cuts_the_beams_again_after_a_column_is_moved() {
    let mut s = metric();
    let c = hd(&s.execute("columnadd", &json!({"at": [0, 0]})).unwrap()["handles"][0]);
    let b = hd(&s.execute("beamadd", &json!({"points": [[0, 0], [4000, 0]]})).unwrap()["handles"][0]);
    assert_eq!(object(&s, b).trim_start_mm, 100.0);
    s.execute("move", &json!({"handles": [c.hex()], "from": [0, 0], "to": [0, 5000]})).unwrap();
    // The beam is still cut where the column was, until it is told to look again.
    assert_eq!(object(&s, b).trim_start_mm, 100.0);
    let r = s.execute("structcleanup", &json!({})).unwrap();
    assert_eq!(r["regenerated"], 1, "{r}");
    assert_eq!(object(&s, b).trim_start_mm, 0.0);
    assert_eq!(s.execute("structcleanup", &json!({})).unwrap()["regenerated"], 0);
}

#[test]
fn the_command_line_places_columns_one_after_the_other_and_beams_in_a_chain() {
    let mut s = metric();
    for t in ["columnadd", "s", "heb 300", "0,0", "h", "4200", "5000,0", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let cols: Vec<StructParams> = s.doc().unwrap().model.iter().filter_map(|e| struct_of(e)).collect();
    assert_eq!(
        cols.iter().map(|p| (p.name.as_str(), p.section.as_str(), p.height_mm)).collect::<Vec<_>>(),
        [("C1", "HEB 300", 3000.0), ("C2", "HEB 300", 4200.0)]
    );
    // A wrong section is told and the old one is kept.
    for t in ["columnadd", "s", "nonsense 9", "30000,30000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.log.iter().any(|l| l.contains("Enter a section of the table")), "{:?}", s.log);
    // Beams: two segments from three points, each cut at the columns it ends on.
    for t in ["beamadd", "0,0", "5000,0", "5000,4000", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let beams: Vec<StructParams> = s.doc().unwrap().model.iter().filter_map(|e| struct_of(e)).filter(|p| p.kind == StructKind::Beam).collect();
    assert_eq!(beams.len(), 2);
    assert_eq!(beams[0].section, "HEB 300", "the section chosen for the columns is kept");
    assert_eq!((beams[0].trim_start_mm, beams[0].trim_end_mm), (150.0, 150.0));
    // The aliases and the programmatic form never open a dialog.
    assert!(s.execute("columnadd", &json!({"at": [9000, 0]})).is_ok());
}

#[test]
fn the_takeoff_adds_up_the_steel_by_section_and_the_slabs_by_kind_and_is_written_to_a_file() {
    let mut s = metric();
    s.execute("columnadd", &json!({"points": [[0, 0], [5000, 0]], "section": "HEA 200", "height": 3000})).unwrap();
    s.execute("columnadd", &json!({"at": [0, 5000], "section": "HEB 300", "height": 4000})).unwrap();
    s.execute("beamadd", &json!({"points": [[0, 0], [5000, 0]], "section": "IPE 240"})).unwrap();
    s.execute("beamadd", &json!({"points": [[0, 0], [0, 5000]], "section": "IPE 240"})).unwrap();
    let slab = s.execute("slabadd", &json!({"points": [[0, 0], [8000, 0], [8000, 6000], [0, 6000]]})).unwrap();
    s.execute("slabedit", &json!({"handle": slab["handle"], "hole": [[1000, 1000], [2000, 1000], [2000, 2000], [1000, 2000]]})).unwrap();
    let t = s.execute("structtakeoff", &json!({})).unwrap();
    let col = |i: usize| {
        (
            t["columns"][i]["section"].as_str().unwrap().to_string(),
            t["columns"][i]["count"].as_u64().unwrap(),
            t["columns"][i]["massKg"].as_f64().unwrap(),
        )
    };
    // Two HEA 200 of 3 m (42.3 kg/m), one HEB 300 of 4 m (117 kg/m): in the order of the table.
    assert_eq!((col(0).0.as_str(), col(0).1), ("HEA 200", 2));
    assert!((col(0).2 - 2.0 * 3.0 * 42.3).abs() < 0.01 && (col(1).2 - 4.0 * 117.0).abs() < 0.01, "{t}");
    assert_eq!(col(1).0, "HEB 300");
    // Two IPE 240 of 5 m (30.7 kg/m).
    assert_eq!((t["beams"][0]["count"].as_u64(), t["beams"][0]["lengthM"].as_f64()), (Some(2), Some(10.0)));
    assert!((t["steelKg"].as_f64().unwrap() - (253.8 + 468.0 + 307.0)).abs() < 0.01, "{t}");
    // One slab of 48 m² with a 1 m² opening, 200 mm thick.
    assert_eq!((t["slabs"][0]["kind"].as_str(), t["slabs"][0]["holes"].as_u64()), (Some("floor"), Some(1)));
    assert!((t["slabAreaM2"].as_f64().unwrap() - 47.0).abs() < 1e-9 && (t["slabVolumeM3"].as_f64().unwrap() - 9.4).abs() < 1e-9, "{t}");
    // Written as a workbook and as a text file, decimals with a comma by default.
    let dir = std::env::temp_dir().join(format!("cadcraft-struct-{}", std::process::id()));
    let (xlsx, csv) = (dir.join("structure.xlsx"), dir.join("structure.csv"));
    s.execute("structtakeoff", &json!({"path": xlsx.to_string_lossy()})).unwrap();
    assert!(std::fs::read(&xlsx).unwrap().starts_with(b"PK"), "a workbook is a zip file");
    s.execute("structtakeoff", &json!({"path": csv.to_string_lossy()})).unwrap();
    let text = String::from_utf8(std::fs::read(&csv).unwrap()).unwrap();
    assert!(text.starts_with('\u{feff}') && text.contains("Column;HEA 200;2;6;253,8"), "{text}");
    s.execute("structtakeoff", &json!({"path": csv.to_string_lossy(), "decimal": "point"})).unwrap();
    assert!(String::from_utf8(std::fs::read(&csv).unwrap()).unwrap().contains("Column;HEA 200;2;6;253.8"));
    assert!(s.execute("structtakeoff", &json!({"decimal": "x"})).is_err());
    // An empty drawing has nothing to add up.
    let empty = metric().execute("structtakeoff", &json!({})).unwrap();
    assert_eq!((empty["steelKg"].as_f64(), empty["columns"].as_array().map(Vec::len)), (Some(0.0), Some(0)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn concrete_columns_and_beams_of_any_size_are_drawn_edited_and_counted_in_cubic_metres() {
    let mut s = metric();
    let col = s.execute("columnadd", &json!({"at": [0, 0], "section": "BA 300x500", "height": 3000})).unwrap();
    s.execute("columnadd", &json!({"at": [6000, 0], "section": "ba \u{d8}40", "height": 3000})).unwrap();
    s.execute("beamadd", &json!({"points": [[0, 0], [6000, 0]], "section": "BA 25x60", "elevation": 2.4})).unwrap();
    for bad in [json!({"at": [0, 0], "section": "BA 3x3"}), json!({"at": [0, 0], "section": "HEA 999"}), json!({"at": [0, 0], "section": "300x500"})]
    {
        assert!(s.execute("columnadd", &bad).is_err(), "{bad}");
    }
    let t = s.execute("structtakeoff", &json!({})).unwrap();
    // 0.3 × 0.5 × 3 + π × 0.2² × 3 + 0.25 × 0.6 × 6 (the beam may be cut at the columns, so a little less).
    let expected = 0.45 + std::f64::consts::PI * 0.04 * 3.0 + 0.9;
    let m3 = t["concreteM3"].as_f64().unwrap();
    assert!(m3 > 1.5 && m3 <= expected + 1e-6, "{m3} vs {expected}");
    assert_eq!(t["steelKg"].as_f64(), Some(0.0));
    // A concrete column becomes steel and back.
    s.execute("structedit", &json!({"handle": col["handles"][0], "section": "HEA 200"})).unwrap();
    s.execute("structedit", &json!({"handle": col["handles"][0], "section": "BA 40x40"})).unwrap();
    assert!(s.execute("structedit", &json!({"handle": col["handles"][0], "section": "nonsense"})).is_err());
}
