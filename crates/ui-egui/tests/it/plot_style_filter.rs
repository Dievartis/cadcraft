//! The dockable Plot Style Manager's "Show only colors used in the current drawing" filter
//! (#3122, on #3142's panel): it hides the rows of colours no object uses, shares the set the
//! Used column and its sort read, and shows every row again when turned off.

use cadcraft_engine::Session;
use cadcraft_engine::doc::{Common, Drawing, EntityKind, Line, Space, color::Color};
use cadcraft_engine::geom::Vec3;
use cadcraft_ui_egui::docking::Panel;
use cadcraft_ui_egui::{CadApp, Services};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A drawing with one red line: colour 1 is used, every other index colour is not.
fn harness() -> Harness<'static, CadApp> {
    let mut session = Session::empty();
    let mut d = Drawing::new_imperial();
    d.add(
        &Space::Model,
        Common { color: Color::Index(1), ..Common::default() },
        EntityKind::Line(Line { a: Vec3::ZERO, b: Vec3::new(1.0, 0.0, 0.0) }),
    )
    .unwrap();
    session.open_drawing(d, "Colours", None);
    let mut app = CadApp::new(session, Services::default());
    app.run("stylesmanager", serde_json::Value::Null).unwrap();
    assert!(app.ui.docking.layout.contains(&Panel::PlotStyles), "STYLESMANAGER opens the dockable panel");
    let mut h = Harness::builder()
        .with_size(vec2(1400.0, 900.0))
        // The table's colour and lineweight controls are label-less by design; this test only
        // needs the row labels and the checkbox.
        .with_accessibility_check(false)
        .build_ui_state(
            |ui, app: &mut CadApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            app,
        );
    h.run_steps(4);
    h
}

#[test]
fn the_filter_hides_unused_colors_and_restores_them() {
    let mut h = harness();
    let filter = "Show only colors used in the current drawing";
    // The table starts complete.
    assert!(h.query_by_label("Color_1").is_some());
    assert!(h.query_by_label("Color_2").is_some(), "the unused colour starts shown");
    h.get_by_label(filter).click();
    h.run_steps(2);
    assert!(h.query_by_label("Color_1").is_some(), "the colour the line uses stays");
    assert!(h.query_by_label("Color_2").is_none(), "the unused colour is hidden");
    assert!(h.query_by_label("Color_255").is_none());
    // Turning the filter off brings the complete table back, settings untouched.
    h.get_by_label(filter).click();
    h.run_steps(2);
    assert!(h.query_by_label("Color_2").is_some(), "the whole table is back");
    assert_eq!(h.state().session.doc().unwrap().plot_style_tables.len(), 0, "the filter changes no table");
}
