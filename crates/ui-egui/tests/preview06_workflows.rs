use cadcraft_engine::Session;
use cadcraft_ui_egui::{CadApp, Services, docking::Panel};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;

fn harness() -> Harness<'static, CadApp> {
    let mut app = CadApp::new(Session::new(), Services::default());
    app.ui.start_tab = false;
    app.ui.in_window_menu = false;
    let builder = Harness::builder().with_size(egui::vec2(1400.0, 900.0));
    let builder = if std::env::var_os("CADCRAFT_PREVIEW06_SHOT").is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
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
fn direct_close_hides_and_reopens_the_image_panel() {
    let mut h = harness();
    h.state_mut().run("ui.dock", json!({"operation":"open","panel":"images"})).unwrap();
    h.run_steps(4);
    assert!(h.state().ui.docking.layout.contains(&Panel::Images));
    let close = h.get_by_label("Close Image Manager");
    let tab = craft_ui::docking::DockArea::<Panel>::hit_rects(&h.ctx, egui::Id::unique("cadcraft-docking"))
        .into_iter()
        .find(|(hit, _)| *hit == craft_ui::docking::Hit::Tab(Panel::Images))
        .unwrap()
        .1;
    assert!((close.rect().center().y - tab.center().y).abs() < 2.0, "close belongs to the tab strip");
    close.click();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Images));
    h.state_mut().run("ui.dock", json!({"operation":"open","panel":"images"})).unwrap();
    h.run_steps(4);
    assert!(h.state().ui.docking.layout.contains(&Panel::Images));
    if let Some(path) = std::env::var_os("CADCRAFT_PREVIEW06_SHOT") {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("panels.png")).unwrap();
    }
}

#[test]
fn preferences_open_existing_drawing_settings() {
    let mut h = harness();
    h.state_mut().ui.dialog = Some("options:workflow".into());
    h.run_steps(4);
    h.get_by_label("Drawing Units...").click();
    h.run_steps(4);
    assert_eq!(h.state().ui.dialog.as_deref(), Some("units"));
    h.state_mut().ui.dialog = Some("options:workflow".into());
    h.run_steps(4);
    if let Some(path) = std::env::var_os("CADCRAFT_PREVIEW06_SHOT") {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("preferences.png")).unwrap();
    }
}

#[test]
fn dynamic_prompt_near_right_canvas_edge() {
    let mut h = harness();
    h.state_mut().session.cmdline("line").unwrap();
    h.event(egui::Event::PointerMoved(egui::pos2(1050.0, 300.0)));
    h.run_steps(4);
    assert!(h.state().session.prompting());
    if let Some(path) = std::env::var_os("CADCRAFT_PREVIEW06_SHOT") {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("cursor.png")).unwrap();
    }
}
