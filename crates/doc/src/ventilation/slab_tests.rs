use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn square(side: f64) -> Vec<Vec2> {
    vec![Vec2::new(1000.0, 2000.0), Vec2::new(1000.0 + side, 2000.0), Vec2::new(1000.0 + side, 2000.0 + side), Vec2::new(1000.0, 2000.0 + side)]
}

#[test]
fn a_slab_is_drawn_hatched_with_its_thickness_and_top_elevation_and_keeps_its_data() {
    let mut d = Drawing::new_metric();
    let mut p = SlabParams::new("Dalle RDC", SlabKind::Floor, square(5000.0), HvacUnits { mm_per_unit: 1.0 });
    p.elevation_mm = 3000.0;
    assert_eq!((p.area_m2(), p.volume_m3()), (25.0, 5.0));
    assert_eq!(p.label_text(), "Dalle RDC 200 mm +3.00 m");
    let h = d.add_slab(&Space::Model, Common::default(), p.clone()).unwrap();
    assert_eq!(d.entity(h).unwrap().common.layer, SLAB_FLOOR_LAYER);
    let back = slab_of(d.entity(h).unwrap()).unwrap();
    assert_eq!((back.thickness_mm, back.elevation_mm, back.outline.len()), (200.0, 3000.0, 4));
    // Turned into a roof: the layer follows the kind.
    d.regenerate_slab(h, &SlabParams { kind: SlabKind::Roof, thickness_mm: 250.0, ..back }).unwrap();
    assert_eq!(d.entity(h).unwrap().common.layer, SLAB_ROOF_LAYER);
    assert_eq!(slab_of(d.entity(h).unwrap()).unwrap().thickness_mm, 250.0);
}

#[test]
fn a_slab_that_cannot_be_drawn_is_refused() {
    let u = HvacUnits { mm_per_unit: 1.0 };
    let ok = SlabParams::new("S", SlabKind::Floor, square(1000.0), u);
    for bad in [
        SlabParams { thickness_mm: 0.0, ..ok.clone() },
        SlabParams { thickness_mm: f64::NAN, ..ok.clone() },
        SlabParams { outline: vec![Vec2::ZERO, Vec2::new(1.0, 0.0)], ..ok.clone() },
        SlabParams { outline: vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0)], ..ok.clone() },
        SlabParams { name: String::new(), ..ok.clone() },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
    assert_eq!((SlabKind::parse("plancher"), SlabKind::parse("Toiture"), SlabKind::parse("x")), (Some(SlabKind::Floor), Some(SlabKind::Roof), None));
}

fn hole(x: f64, y: f64, w: f64) -> Vec<Vec2> {
    vec![Vec2::new(x, y), Vec2::new(x + w, y), Vec2::new(x + w, y + w), Vec2::new(x, y + w)]
}

#[test]
fn a_slab_with_openings_has_its_net_area_and_volume_and_the_hatch_goes_round_them() {
    let u = HvacUnits { mm_per_unit: 1.0 };
    let mut p = SlabParams::new("S", SlabKind::Floor, square(5000.0), u);
    p.holes = vec![hole(2000.0, 3000.0, 1000.0), hole(4000.0, 3000.0, 500.0)];
    // 25 m² less 1 m² and 0.25 m²; 200 mm thick.
    assert!((p.area_m2() - 23.75).abs() < 1e-9 && (p.holes_area_m2() - 1.25).abs() < 1e-9 && (p.volume_m3() - 4.75).abs() < 1e-9);
    p.validate().unwrap();
    let pieces = p.generate().unwrap();
    let hatch = pieces.iter().find_map(|x| match &x.kind {
        EntityKind::Hatch(h) => Some(h.clone()),
        _ => None,
    });
    assert_eq!(
        hatch.map(|h| h.loops.iter().map(|l| l.outer).collect::<Vec<_>>()),
        Some(vec![true, false, false]),
        "the openings are islands of the hatch"
    );
    // Each opening is outlined and crossed: the slab's own outline, plus a polyline and two lines for each.
    assert_eq!(pieces.iter().filter(|x| matches!(x.kind, EntityKind::LwPolyline(_))).count(), 3);
    assert_eq!(pieces.iter().filter(|x| matches!(x.kind, EntityKind::Line(_))).count(), 4);
    // They are kept with the object and move with it when the slab is added with them in drawing coordinates.
    let mut d = Drawing::new_metric();
    let mut q = SlabParams::new("S", SlabKind::Floor, square(5000.0), u);
    q.holes = vec![hole(2000.0, 3000.0, 1000.0)];
    let h = d.add_slab(&Space::Model, Common::default(), q).unwrap();
    let back = slab_of(d.entity(h).unwrap()).unwrap();
    assert_eq!(back.holes, vec![hole(1000.0, 1000.0, 1000.0)], "in the block's coordinates, from the slab's first corner");
    assert_eq!(d.slab_holes_world(h), vec![hole(2000.0, 3000.0, 1000.0)]);
    // A third opening is added in the drawing's coordinates, and refused outside the slab.
    d.add_slab_hole(h, &hole(4000.0, 2500.0, 500.0)).unwrap();
    assert_eq!(slab_of(d.entity(h).unwrap()).unwrap().holes.len(), 2);
    assert!(d.add_slab_hole(h, &hole(5800.0, 2500.0, 500.0)).is_err());
    assert!(d.slab_contains(h, &hole(2000.0, 3000.0, 500.0)) && !d.slab_contains(h, &hole(0.0, 0.0, 500.0)));
}

#[test]
fn bad_openings_and_slopes_are_refused() {
    let u = HvacUnits { mm_per_unit: 1.0 };
    let ok = SlabParams::new("S", SlabKind::Floor, square(5000.0), u);
    let inside = hole(2000.0, 3000.0, 1000.0);
    for bad in [
        SlabParams { holes: vec![vec![Vec2::new(2000.0, 3000.0), Vec2::new(3000.0, 3000.0)]], ..ok.clone() },
        SlabParams { holes: vec![hole(9000.0, 3000.0, 1000.0)], ..ok.clone() },
        SlabParams { holes: vec![vec![Vec2::new(2000.0, 3000.0), Vec2::new(3000.0, 3000.0), Vec2::new(4000.0, 3000.0)]], ..ok.clone() },
        SlabParams { holes: vec![vec![Vec2::new(f64::NAN, 3000.0), Vec2::new(3000.0, 3000.0), Vec2::new(4000.0, 4000.0)]], ..ok.clone() },
        SlabParams { holes: vec![inside.clone(); MAX_SLAB_HOLES + 1], ..ok.clone() },
        SlabParams { slope_pct: -1.0, ..ok.clone() },
        SlabParams { slope_pct: f64::NAN, ..ok.clone() },
        SlabParams { slope_pct: 500.0, ..ok.clone() },
        SlabParams { slope_pct: 2.0, slope_direction_deg: f64::INFINITY, ..ok.clone() },
    ] {
        assert!(bad.validate().is_err() && bad.generate().is_err(), "{bad:?}");
    }
    // A slope is told in the label and drawn as an arrow; without one, neither.
    let flat = ok.generate().unwrap();
    let sloped = SlabParams { slope_pct: 2.0, slope_direction_deg: 90.0, ..ok.clone() };
    assert_eq!(sloped.label_text(), "S 200 mm +0.00 m 2.0 %");
    let drawn = sloped.generate().unwrap();
    assert_eq!(
        drawn.iter().filter(|x| matches!(x.kind, EntityKind::Line(_))).count(),
        flat.iter().filter(|x| matches!(x.kind, EntityKind::Line(_))).count() + 3
    );
}

#[test]
fn a_big_slab_has_a_coarser_pattern_so_the_hatch_stays_light() {
    let square = |side_mm: f64| {
        let outline = vec![Vec2::new(0.0, 0.0), Vec2::new(side_mm, 0.0), Vec2::new(side_mm, side_mm), Vec2::new(0.0, side_mm)];
        SlabParams::new("S", SlabKind::Floor, outline, HvacUnits::default())
    };
    // A room: the pattern keeps its scale. A hall of 60 × 60 m: coarser, so no more than 400 repeats (the old default was AR-CONC, which hung the web build).
    assert_eq!(square(5000.0).pattern_scale(), 1.0);
    let big = square(60_000.0).pattern_scale();
    assert!(big > 3.0 && big.is_finite(), "{big}");
    // A new slab is hatched with diagonal lines.
    assert_eq!(square(5000.0).pattern, "ANSI31");
}
