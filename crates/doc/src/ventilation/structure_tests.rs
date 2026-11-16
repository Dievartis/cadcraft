use cadcraft_geom::Vec2;
use cadcraft_hvac::{SectionFamily, SteelSection, find_section, parse_section};

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn section(text: &str) -> &'static SteelSection {
    parse_section(text).expect("a section of the table")
}

/// The bounding box of the first closed polyline a symbol draws, as (width, height).
fn outline_size(pieces: &[HvacPiece]) -> (f64, f64) {
    let EntityKind::LwPolyline(l) = &pieces[0].kind else { panic!("the first piece is the outline") };
    let span = |f: fn(&cadcraft_geom::PolyVertex) -> f64| {
        let v: Vec<f64> = l.vertices.iter().map(f).collect();
        v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
    };
    (span(|v| v.p.x), span(|v| v.p.y))
}

fn column(d: &mut Drawing, name: &str, at: (f64, f64)) -> crate::Handle {
    let p = StructParams::column(name, section("HEA 200"), units());
    d.add_column(&Space::Model, Common::default(), p, Vec2::new(at.0, at.1), 0.0).unwrap()
}

fn beam(d: &mut Drawing, name: &str, text: &str, a: (f64, f64), b: (f64, f64)) -> crate::Handle {
    let p = StructParams::beam(name, section(text), 1.0, units());
    d.add_beam(&Space::Model, Common::default(), p, Vec2::new(a.0, a.1), Vec2::new(b.0, b.1)).unwrap()
}

fn params(d: &Drawing, h: crate::Handle) -> StructParams {
    struct_of(d.entity(h).unwrap()).unwrap()
}

#[test]
fn a_column_is_its_section_to_scale_filled_and_labelled() {
    let mut p = StructParams::column("C1", section("HEA 200"), units());
    p.height_mm = 3600.0;
    p.elevation_mm = -150.0;
    let pieces = p.generate(false).unwrap();
    // A HEA 200 is 190 mm high and 200 mm wide.
    assert_eq!(outline_size(&pieces), (200.0, 190.0));
    assert!(pieces.iter().any(|x| matches!(&x.kind, EntityKind::Hatch(h) if h.solid)), "the section is filled");
    assert!(pieces.iter().all(|x| x.layer == COLUMN_LAYER));
    assert_eq!(p.label_text(), "C1 HEA 200 h=3.60 m (-0.15 m)");
    assert!(pieces.iter().any(|x| matches!(&x.kind, EntityKind::Text(t) if t.value.contains("HEA 200"))));
    // The twelve vertices of an I; a channel has eight.
    let EntityKind::LwPolyline(l) = &pieces[0].kind else { panic!() };
    assert_eq!(l.vertices.len(), 12);
    let upn = StructParams::column("C2", section("UPN 200"), units()).generate(false).unwrap();
    let EntityKind::LwPolyline(l) = &upn[0].kind else { panic!() };
    assert_eq!(l.vertices.len(), 8);
    assert_eq!(outline_size(&upn), (75.0, 200.0));
    // In metres the same section is a thousandth as large.
    let metres = StructParams::column("C3", section("HEA 200"), HvacUnits { mm_per_unit: 1000.0 }).generate(false).unwrap();
    let (w, h) = outline_size(&metres);
    assert!((w - 0.2).abs() < 1e-12 && (h - 0.19).abs() < 1e-12, "{w} {h}");
}

#[test]
fn a_beam_is_a_band_of_the_flange_width_with_the_lines_of_its_web() {
    let mut p = StructParams::beam("B1", section("IPN 200"), 4000.0, units());
    p.elevation_mm = 2800.0;
    let pieces = p.generate(false).unwrap();
    // 90 mm wide (the flange), 4 m long; the web's two lines.
    assert_eq!(outline_size(&pieces), (4000.0, 90.0));
    assert_eq!(pieces.iter().filter(|x| matches!(x.kind, EntityKind::Line(_))).count(), 2);
    assert_eq!(p.label_text(), "B1 IPN 200 (+2.80 m)");
    // The top is the underside and the section's height; the mass its length by the mass per metre.
    assert_eq!(p.top_mm(), 3000.0);
    assert!((p.mass_kg() - 4.0 * 26.2).abs() < 1e-9);
    // Cut at both ends it is shorter.
    p.trim_start_mm = 100.0;
    p.trim_end_mm = 50.0;
    assert_eq!(outline_size(&p.generate(false).unwrap()), (3850.0, 90.0));
    // Upside down the label is turned, on the other side.
    let up = |flip: bool| match &p.generate(flip).unwrap().last().unwrap().kind {
        EntityKind::Text(t) => (t.rotation, t.insert.y),
        k => panic!("{k:?}"),
    };
    assert!(up(false).1 > 0.0 && up(true).1 < 0.0 && up(true).0 > 3.0);
}

#[test]
fn a_structural_object_keeps_its_data_and_refuses_what_cannot_be_drawn() {
    let mut p = StructParams::column("C1", section("HEB 300"), units());
    p.elevation_mm = 250.0;
    assert_eq!(StructParams::from_xdata(&p.to_xdata().unwrap()), Some(p.clone()));
    for bad in [
        StructParams { section: "IPN 205".into(), ..p.clone() },
        StructParams { section: "".into(), ..p.clone() },
        StructParams { height_mm: 0.0, ..p.clone() },
        StructParams { height_mm: f64::NAN, ..p.clone() },
        StructParams { height_mm: 1.0e9, ..p.clone() },
        StructParams { elevation_mm: f64::INFINITY, ..p.clone() },
        StructParams { name: "x".repeat(500), ..p.clone() },
        StructParams { label_height_mm: 0.0, ..p.clone() },
    ] {
        assert!(bad.validate().is_err() && bad.generate(false).is_err(), "{bad:?}");
    }
    let b = StructParams::beam("B1", section("IPE 200"), 3000.0, units());
    for bad in [
        StructParams { length: 0.0, ..b.clone() },
        StructParams { length: f64::NAN, ..b.clone() },
        StructParams { trim_start_mm: -1.0, ..b.clone() },
        StructParams { trim_start_mm: 2000.0, trim_end_mm: 1000.0, ..b.clone() },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    // Data that is no longer valid is not read back as a structural object.
    let mut tags = p.to_xdata().unwrap();
    assert!(StructParams::from_xdata(&tags).is_some());
    tags.clear();
    assert!(StructParams::from_xdata(&tags).is_none());
    assert_eq!(StructKind::parse("Poteau"), Some(StructKind::Column));
    assert_eq!(StructKind::parse("poutre"), Some(StructKind::Beam));
    assert_eq!(StructKind::parse("wall"), None);
}

#[test]
fn a_beam_stops_at_the_face_of_the_columns_it_rests_on() {
    let mut d = Drawing::new_metric();
    // Two HEA 200 (200 wide, 190 deep), 5 m apart, and a beam from the centre of one to the centre of the other.
    column(&mut d, "C1", (0.0, 0.0));
    column(&mut d, "C2", (5000.0, 0.0));
    let b = beam(&mut d, "B1", "IPE 240", (0.0, 0.0), (5000.0, 0.0));
    assert_eq!(d.cleanup_structure(&Space::Model).unwrap(), 1);
    let p = params(&d, b);
    // It stops at the columns' faces: half of their width, 100 mm, at each end.
    assert_eq!((p.trim_start_mm, p.trim_end_mm), (100.0, 100.0));
    // The cleanup is stable: nothing more to cut the second time.
    assert_eq!(d.cleanup_structure(&Space::Model).unwrap(), 0);
    // A column turned a quarter turn is 190 wide along the beam: the beam stops 95 mm short.
    let c2 = d.model.iter().filter(|e| struct_of(e).is_some_and(|p| p.kind == StructKind::Column)).map(|e| e.handle).nth(1).unwrap();
    d.modify_entity(c2, |e| {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.rotation = std::f64::consts::FRAC_PI_2;
        }
    })
    .unwrap();
    assert_eq!(d.cleanup_structure(&Space::Model).unwrap(), 1);
    let p = params(&d, b);
    assert!((p.trim_start_mm - 100.0).abs() < 1e-6 && (p.trim_end_mm - 95.0).abs() < 1e-6, "{p:?}");
    // Moved away from the end of the beam, the column cuts it no more.
    d.modify_entity(c2, |e| {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert.x = 9000.0;
        }
    })
    .unwrap();
    assert_eq!(d.cleanup_structure(&Space::Model).unwrap(), 1);
    assert_eq!((params(&d, b).trim_start_mm, params(&d, b).trim_end_mm), (100.0, 0.0));
}

#[test]
fn a_beam_ending_on_another_stops_at_its_band_and_crossing_beams_are_left_alone() {
    let mut d = Drawing::new_metric();
    // A primary IPE 300 (150 wide) along x, and a secondary IPE 200 ending on its axis from the north.
    let primary = beam(&mut d, "B1", "IPE 300", (0.0, 0.0), (8000.0, 0.0));
    let secondary = beam(&mut d, "B2", "IPE 200", (4000.0, 3000.0), (4000.0, 0.0));
    // And a beam that crosses the primary in its middle, ending nowhere on it.
    let crossing = beam(&mut d, "B3", "IPE 200", (2000.0, -2000.0), (2000.0, 2000.0));
    assert_eq!(d.cleanup_structure(&Space::Model).unwrap(), 1);
    // The secondary stops at the primary's edge, 75 mm from its axis; the primary and the crossing one are whole.
    let s = params(&d, secondary);
    assert_eq!((s.trim_start_mm, s.trim_end_mm), (0.0, 75.0));
    assert_eq!((params(&d, primary).trim_start_mm, params(&d, primary).trim_end_mm), (0.0, 0.0));
    assert_eq!((params(&d, crossing).trim_start_mm, params(&d, crossing).trim_end_mm), (0.0, 0.0));
    // A beam running along another one is not cut by it.
    let alongside = beam(&mut d, "B4", "IPE 200", (0.0, 0.0), (3000.0, 0.0));
    d.cleanup_structure(&Space::Model).unwrap();
    assert_eq!((params(&d, alongside).trim_start_mm, params(&d, alongside).trim_end_mm), (0.0, 0.0));
}

#[test]
fn a_beam_and_a_column_survive_every_kind_of_hostile_neighbour() {
    let mut d = Drawing::new_metric();
    // A zero-length beam cannot be drawn; coincident, huge and far objects do not break the cleanup.
    let p = StructParams::beam("B0", section("IPE 100"), 1.0, units());
    assert!(d.add_beam(&Space::Model, Common::default(), p.clone(), Vec2::ZERO, Vec2::ZERO).is_err());
    assert!(d.add_beam(&Space::Model, Common::default(), p, Vec2::ZERO, Vec2::new(f64::NAN, 0.0)).is_err());
    let c = StructParams::column("C0", section("HEB 500"), units());
    assert!(d.add_column(&Space::Model, Common::default(), c.clone(), Vec2::new(f64::INFINITY, 0.0), 0.0).is_err());
    assert!(d.add_column(&Space::Model, Common::default(), StructParams { section: "NOPE".into(), ..c.clone() }, Vec2::ZERO, 0.0).is_err());
    column(&mut d, "C1", (0.0, 0.0));
    column(&mut d, "C2", (0.0, 0.0));
    beam(&mut d, "B1", "HEB 500", (0.0, 0.0), (400.0, 0.0));
    beam(&mut d, "B2", "HEB 500", (0.0, 0.0), (400.0, 0.0));
    beam(&mut d, "B3", "IPE 100", (1.0e9, 1.0e9), (1.0e9 + 5000.0, 1.0e9));
    // Whatever it decides, it never cuts a beam to nothing.
    d.cleanup_structure(&Space::Model).unwrap();
    for e in d.model.iter() {
        if let Some(p) = struct_of(e).filter(|p| p.kind == StructKind::Beam) {
            assert!(p.validate().is_ok() && p.trim_start_mm + p.trim_end_mm < p.length_mm(), "{p:?}");
        }
    }
    assert!(find_section(SectionFamily::Heb, 500).is_some());
}

fn wall(d: &mut Drawing, path: &[(f64, f64)]) -> crate::Handle {
    let style = crate::standard_wall_styles().into_iter().next().expect("a standard style");
    let p = crate::WallParams {
        style,
        path: path.iter().map(|&(x, y)| Vec2::new(x, y)).collect(),
        closed: false,
        justification: crate::WallJustification::Left,
        height: 2500.0,
        units: crate::WallUnits::default(),
        joints: Default::default(),
        bulges: Vec::new(),
        start_free: false,
        end_free: false,
    };
    d.add_wall(&Space::Model, Common::default(), p).unwrap()
}

#[test]
fn a_column_finds_the_corner_and_the_junctions_of_the_walls() {
    let mut d = Drawing::new_metric();
    // An L (one wall with a bend at (4000, 0)), and a wall that ends on it from the north: a T at (2000, 0).
    wall(&mut d, &[(0.0, 0.0), (4000.0, 0.0), (4000.0, 3000.0)]);
    wall(&mut d, &[(2000.0, 0.0), (2000.0, 3000.0)]);
    // The bend, the start of a wall, the end of another.
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(4040.0, 30.0), 100.0), Some(Vec2::new(4000.0, 0.0)));
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(60.0, -40.0), 100.0), Some(Vec2::new(0.0, 0.0)));
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(2000.0, 2950.0), 100.0), Some(Vec2::new(2000.0, 3000.0)));
    // Where the two walls meet, the junction itself, even a little off it.
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(2030.0, 20.0), 100.0), Some(Vec2::new(2000.0, 0.0)));
    // The middle of a wall, far from any corner: nothing to snap to; nor outside the tolerance, nor with nonsense.
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(1000.0, 10.0), 100.0), None);
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(4300.0, 300.0), 100.0), None);
    for tol in [0.0, -5.0, f64::NAN] {
        assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(4000.0, 0.0), tol), None);
    }
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(f64::NAN, 0.0), 100.0), None);
    // A wall that was moved is found where it is now.
    let h = d.model.iter().next().map(|e| e.handle).unwrap();
    d.modify_entity(h, |e| {
        if let EntityKind::Insert(i) = &mut e.kind {
            i.insert.x += 10_000.0;
        }
    })
    .unwrap();
    assert_eq!(d.wall_corner_near(&Space::Model, Vec2::new(14_010.0, 10.0), 100.0), Some(Vec2::new(14_000.0, 0.0)));
}
