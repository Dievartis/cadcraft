use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Vec2> {
    vec![Vec2::new(x, y), Vec2::new(x + w, y), Vec2::new(x + w, y + h), Vec2::new(x, y + h)]
}

fn lines(pieces: &[HvacPiece]) -> usize {
    pieces.iter().filter(|p| matches!(p.kind, EntityKind::Line(_))).count()
}

#[test]
fn a_ceiling_grid_has_its_lines_inside_the_room_and_counts_its_tiles() {
    // A room of 3000 x 2400 mm: 5 x 4 tiles of 600 x 600, whole; 4 lines one way and 3 the other inside it.
    let p = CeilingParams::new("FP1", rect(1000.0, 500.0, 3000.0, 2400.0), units());
    let pieces = p.generate().unwrap();
    assert_eq!(lines(&pieces), 4 + 3);
    assert_eq!(p.tiles(), TileCount { whole: 20, cut: 0, area_m2: 7.2 });
    assert!(pieces.iter().any(|x| matches!(&x.kind, EntityKind::Text(t) if t.value == "FP1 600x600 +2.60 m")));
    // A room that is not a multiple of the tile: the tiles at the edge are cut.
    let q = CeilingParams::new("FP2", rect(0.0, 0.0, 3100.0, 2500.0), units());
    let t = q.tiles();
    assert_eq!((t.whole, t.cut), (5 * 4, 6 * 5 - 5 * 4), "{t:?}");
    // A long tile and a turned grid.
    let long = CeilingParams { tile_x_mm: 1200.0, tile_y_mm: 600.0, ..p.clone() };
    assert_eq!((long.tiles().whole, long.tiles().cut), (8, 4), "2.5 tiles of 1200 across: two whole, a half one cut");
    let turned = CeilingParams { angle_deg: 45.0, ..p.clone() };
    assert!(lines(&turned.generate().unwrap()) > 0);
    // The grid of an L-shaped room does not cross the notch.
    let l = CeilingParams::new(
        "FP3",
        vec![
            Vec2::ZERO,
            Vec2::new(3000.0, 0.0),
            Vec2::new(3000.0, 1200.0),
            Vec2::new(1200.0, 1200.0),
            Vec2::new(1200.0, 2400.0),
            Vec2::new(0.0, 2400.0),
        ],
        units(),
    );
    assert_eq!(l.tiles().whole, 10 + 4, "{:?}", l.tiles());
}

#[test]
fn a_ceiling_finds_the_centre_of_the_tile_under_a_point_and_refuses_what_it_cannot_draw() {
    let mut d = Drawing::new_metric();
    let p = CeilingParams::new("FP1", rect(1000.0, 500.0, 3000.0, 2400.0), units());
    let h = d.add_ceiling(&Space::Model, Common::default(), p).unwrap();
    assert_eq!(ceiling_of(d.entity(h).unwrap()).map(|c| c.tile_x_mm), Some(600.0));
    // Tiles start at the room's lower left corner (1000, 500): the one under (1100, 600) is centred on (1300, 800).
    assert_eq!(d.ceiling_tile_centre(&Space::Model, Vec2::new(1100.0, 600.0)), Some(Vec2::new(1300.0, 800.0)));
    assert_eq!(d.ceiling_tile_centre(&Space::Model, Vec2::new(3900.0, 2800.0)), Some(Vec2::new(3700.0, 2600.0)));
    assert_eq!(d.ceiling_tile_centre(&Space::Model, Vec2::new(500.0, 600.0)), None, "outside the room");
    assert_eq!(d.ceiling_tile_centre(&Space::Model, Vec2::new(f64::NAN, 0.0)), None);
    let ok = CeilingParams::new("S", rect(0.0, 0.0, 3000.0, 2400.0), units());
    for bad in [
        CeilingParams { tile_x_mm: 10.0, ..ok.clone() },
        CeilingParams { tile_y_mm: f64::NAN, ..ok.clone() },
        CeilingParams { angle_deg: f64::INFINITY, ..ok.clone() },
        CeilingParams { outline: vec![Vec2::ZERO, Vec2::new(1.0, 0.0)], ..ok.clone() },
        CeilingParams { outline: vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)], ..ok.clone() },
        CeilingParams { outline: rect(0.0, 0.0, 1.0e9, 1.0e9), ..ok.clone() },
        CeilingParams { name: String::new(), ..ok.clone() },
    ] {
        assert!(bad.validate().is_err() && bad.generate().is_err(), "{bad:?}");
    }
    assert_eq!(CeilingParams::from_xdata(&ok.to_xdata().unwrap()), Some(ok));
}

#[test]
fn a_stair_has_the_risers_a_storey_needs_and_says_whether_it_is_comfortable() {
    // 3 m at 180 mm at most: 17 risers of 176.5 mm, 16 treads of 280 mm in plan; 2 x 176.5 + 280 = 633 mm, within Blondel's 600 to 650.
    let p = StairParams::new("E1", units());
    assert_eq!((p.risers(), p.run_mm()), (17, 16.0 * 280.0));
    assert!((p.riser_mm() - 3000.0 / 17.0).abs() < 1e-9 && p.comfortable());
    assert_eq!(p.label_text(), "E1 17 x 176 / 280 mm");
    let pieces = p.generate().unwrap();
    // 15 lines between the treads, the walking line and the two strokes of the arrow.
    assert_eq!(lines(&pieces), 15 + 1 + 2);
    let EntityKind::LwPolyline(o) = &pieces[0].kind else { panic!("the outline") };
    let (w, h) = (o.vertices.iter().map(|v| v.p.x).fold(0.0, f64::max), o.vertices.iter().map(|v| v.p.y).fold(f64::MIN, f64::max) * 2.0);
    assert_eq!((w, h), (4480.0, 1000.0));
    assert!(pieces.iter().any(|x| matches!(&x.kind, EntityKind::Text(t) if t.value == "UP")));
    assert!(StairParams { up: false, ..p.clone() }.generate().unwrap().iter().any(|x| matches!(&x.kind, EntityKind::Text(t) if t.value == "DN")));
    // Shallow treads of 200 mm are not comfortable; a single riser has no tread at all.
    assert!(!StairParams { going_mm: 200.0, ..p.clone() }.comfortable());
    let one = StairParams { rise_mm: 150.0, ..p.clone() };
    assert_eq!((one.risers(), one.run_mm()), (1, 0.0));
    for bad in [
        StairParams { width_mm: 10.0, ..p.clone() },
        StairParams { going_mm: f64::NAN, ..p.clone() },
        StairParams { rise_mm: 1.0e9, ..p.clone() },
        StairParams { riser_max_mm: 0.0, ..p.clone() },
        StairParams { elevation_mm: f64::INFINITY, ..p.clone() },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    // The highest a stair can be is 200 risers.
    assert!(StairParams { rise_mm: 20_000.0, riser_max_mm: 100.0, ..p.clone() }.validate().is_ok());
    assert_eq!(StairParams::from_xdata(&p.to_xdata().unwrap()), Some(p));
}

#[test]
fn a_railing_has_a_post_at_each_end_and_evenly_between() {
    let path = vec![Vec2::ZERO, Vec2::new(3000.0, 0.0), Vec2::new(3000.0, 1200.0)];
    let p = RailParams::new("G1", path, units());
    // 4200 mm with posts at most 1200 mm apart: 4 gaps of 1050 mm, 5 posts, the first and the last at the ends.
    let posts = p.posts();
    assert_eq!(posts.len(), 5);
    assert_eq!((posts[0], posts[4]), (Vec2::ZERO, Vec2::new(3000.0, 1200.0)));
    assert!((posts[1].x - 1050.0).abs() < 1e-9 && (posts[3] - Vec2::new(3000.0, 150.0)).len() < 1e-9, "{posts:?}");
    let pieces = p.generate().unwrap();
    assert_eq!(pieces.iter().filter(|x| matches!(x.kind, EntityKind::LwPolyline(_))).count(), 1 + 5, "the rail and a square for each post");
    assert_eq!(p.label_text(), "G1 h=1.00 m");
    assert_eq!(p.length_mm(), 4200.0);
    let ok = p.clone();
    for bad in [
        RailParams { path: vec![Vec2::ZERO], ..ok.clone() },
        RailParams { path: vec![Vec2::ZERO, Vec2::ZERO], ..ok.clone() },
        RailParams { path: vec![Vec2::ZERO, Vec2::new(f64::NAN, 0.0)], ..ok.clone() },
        RailParams { height_mm: 10.0, ..ok.clone() },
        RailParams { post_spacing_mm: 1.0, ..ok.clone() },
        RailParams { rail_mm: f64::NAN, ..ok.clone() },
        RailParams { path: vec![Vec2::ZERO, Vec2::new(1.0e8, 0.0)], post_spacing_mm: 200.0, ..ok.clone() },
    ] {
        assert!(bad.validate().is_err() && bad.generate().is_err(), "{bad:?}");
    }
    let mut d = Drawing::new_metric();
    let h = d
        .add_rail(&Space::Model, Common::default(), RailParams::new("G2", vec![Vec2::new(500.0, 500.0), Vec2::new(2500.0, 500.0)], units()))
        .unwrap();
    assert_eq!(rail_of(d.entity(h).unwrap()).map(|r| r.path), Some(vec![Vec2::ZERO, Vec2::new(2000.0, 0.0)]), "from the first point");
    assert_eq!(d.entity(h).unwrap().common.layer, RAIL_LAYER);
}
