//! Regression tests for explicit and inherited paper-space units across DXF saves.
//! The drawing's MEASUREMENT controls only the historical, inherited case.

use cadcraft_doc::{Drawing, Layout};
use cadcraft_io::{read, write_dxf};

fn round_trip(measurement: i64, paper_mm: bool, override_mm: Option<bool>) {
    check_round_trip(measurement, paper_mm, override_mm, false);
    check_round_trip(measurement, paper_mm, override_mm, true);
}

fn check_round_trip(measurement: i64, paper_mm: bool, override_mm: Option<bool>, saved_view: bool) {
    let mut drawing = if measurement == 1 { Drawing::new_metric() } else { Drawing::new_imperial() };
    drawing.header.set_i64("MEASUREMENT", measurement);
    let mut layout = Layout::new("UNITS_TEST", 9);
    layout.page.paper_mm = override_mm.or(paper_mm.then_some(true));
    layout.view = saved_view.then_some((cadcraft_geom::Vec2::new(210.0, 148.5), 320.0));
    drawing.layouts.push(layout);

    let before = &drawing.layout("UNITS_TEST").unwrap().page;
    let expected_mm = cadcraft_render::paper::page_unit_mm(&drawing, before);

    let dxf = write_dxf(&drawing);
    let mut in_layout = false;
    let mut seen_subclass = false;
    let mut app_groups = 0;
    let lines: Vec<_> = dxf.lines().collect();
    for pair in lines.as_chunks::<2>().0 {
        let code = pair[0].trim();
        let value = pair[1].trim();
        if code == "0" {
            if in_layout {
                assert!(app_groups <= 1, "at most one CADCRAFT group per layout; inherited inches need no marker");
            }
            in_layout = value == "LAYOUT";
            seen_subclass = false;
            app_groups = 0;
        } else if in_layout && code == "100" && value == "AcDbLayout" {
            seen_subclass = true;
        } else if in_layout && code == "1001" && value == "CADCRAFT" {
            assert!(seen_subclass, "extended data follows normal layout fields");
            app_groups += 1;
        }
    }
    let reopened = read(dxf.as_bytes(), "paper_units_test.dxf").unwrap();
    let after = &reopened.layout("UNITS_TEST").unwrap().page;

    assert_eq!(reopened.layout("UNITS_TEST").unwrap().view, drawing.layout("UNITS_TEST").unwrap().view);
    assert_eq!(after.paper_mm, before.paper_mm);
    assert_eq!(cadcraft_render::paper::page_unit_mm(&reopened, after), expected_mm);
}

#[test]
fn explicit_inches_survive_metric_dxf_round_trip() {
    // The bug: false used to inherit MEASUREMENT=1 and become millimetres.
    round_trip(1, false, Some(false));
}

#[test]
fn explicit_millimetres_survive_imperial_dxf_round_trip() {
    round_trip(0, true, Some(true));
}

#[test]
fn inherited_units_survive_metric_dxf_round_trip() {
    round_trip(1, false, None);
}

#[test]
fn inherited_units_survive_imperial_dxf_round_trip() {
    round_trip(0, false, None);
}

#[test]
fn explicit_inches_survive_imperial_dxf_round_trip() {
    round_trip(0, false, Some(false));
}

#[test]
fn explicit_millimetres_survive_metric_dxf_round_trip() {
    round_trip(1, true, Some(true));
}

#[test]
fn inherited_millimetres_flag_survives_dxf_round_trip() {
    // Older PageSetup data can set paper_mm=true without an override.
    round_trip(0, true, None);
}

#[test]
fn preview_unit_markers_migrate_to_official_encoding_without_changing_scale() {
    for measurement in [0, 1] {
        for (legacy, expected) in [(0, None), (1, Some(true)), (2, Some(false)), (3, Some(true))] {
            let mut drawing = if measurement == 1 { Drawing::new_metric() } else { Drawing::new_imperial() };
            let mut layout = Layout::new("PREVIEW", 9);
            layout.page.paper_mm = Some(false);
            layout.view = Some((cadcraft_geom::Vec2::new(35.0, 60.0), 180.0));
            drawing.layouts.push(layout);
            let text = write_dxf(&drawing);
            let mut rows: Vec<String> = text.lines().map(str::to_string).collect();
            let mut marker = false;
            for pair in rows.as_chunks_mut::<2>().0 {
                if pair[0].trim() == "1000" && pair[1].trim() == "PSUNITS" {
                    pair[1] = "PAPERUNITS".into();
                    marker = true;
                } else if marker && pair[0].trim() == "1070" {
                    pair[1] = legacy.to_string();
                    marker = false;
                }
            }
            let legacy_file = rows.join("\n") + "\n";
            let opened = read(legacy_file.as_bytes(), "preview.dxf").unwrap();
            let page = &opened.layout("PREVIEW").unwrap().page;
            assert_eq!(page.paper_mm, expected);
            assert_eq!(opened.layout("PREVIEW").unwrap().view, drawing.layout("PREVIEW").unwrap().view);
            let unit = cadcraft_render::paper::page_unit_mm(&opened, page);
            let saved = write_dxf(&opened);
            assert!(!saved.contains("\nPAPERUNITS\n"), "new files use the official format");
            let reopened = read(saved.as_bytes(), "migrated.dxf").unwrap();
            assert_eq!(cadcraft_render::paper::page_unit_mm(&reopened, &reopened.layout("PREVIEW").unwrap().page), unit);
        }
    }
}
