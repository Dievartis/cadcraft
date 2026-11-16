//! Recovered text styles remain dockable beside DIMSTYLE and use existing engine commands.

use cadcraft_engine::Session;
use cadcraft_ui_egui::{CadApp, Services, docking::Panel, menus, textstyles};
use egui_kittest::Harness;
use egui_kittest::kittest::{Queryable, by};
use serde_json::json;

#[test]
fn text_style_manager_routes_without_replacing_the_other_managers() {
    let mut app = CadApp::new(Session::new(), Services::default());
    for command in ["STYLE", "ST", "DIMSTYLE", "STYLESMANAGER"] {
        assert!(app.cmdline(command).is_none(), "{command}");
        assert!(app.ui.dialog.is_none());
    }
    for panel in [Panel::TextStyles, Panel::DimStyles, Panel::PlotStyles] {
        assert!(app.ui.docking.layout.contains(&panel));
    }
    app.run("ui.dock.reset", json!({})).unwrap();
    menus::activate(&mut app, "style");
    assert!(app.ui.docking.layout.contains(&Panel::TextStyles));
    app.run("ui.dock.reset", json!({})).unwrap();
    app.run("style", json!({"name": "Programmatic", "font": "Arial", "current": false})).unwrap();
    assert!(app.session.doc().unwrap().text_style("Programmatic").is_some());
    assert!(!app.ui.docking.layout.contains(&Panel::TextStyles));
    let saved = serde_json::to_value(Panel::TextStyles).unwrap();
    assert_eq!(saved, json!("textStyles"));
    assert_eq!(serde_json::from_value::<Panel>(saved).unwrap(), Panel::TextStyles);
}

#[test]
fn text_style_manager_survives_drawing_and_workspace_reload() {
    let mut app = CadApp::new(Session::new(), Services::default());
    app.cmdline("STYLE");
    app.cmdline("DIMSTYLE");
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h.state_mut().run("line", json!({"points": [[0,0], [10,10]]})).unwrap();
    h.run_steps(3);
    let saved = serde_json::to_value(&h.state().ui).unwrap();
    h.state_mut().ui = serde_json::from_value(saved).unwrap();
    h.run_steps(3);
    for panel in [Panel::TextStyles, Panel::DimStyles] {
        assert!(h.state().ui.docking.layout.contains(&panel));
    }
}

fn manager() -> Harness<'static, CadApp> {
    let app = CadApp::new(Session::new(), Services::default());
    let builder = Harness::builder().with_size(egui::vec2(900.0, 800.0));
    let builder = if std::env::var_os("CADCRAFT_TEXT_STYLE_SCREENSHOT").is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut CadApp| {
            cadcraft_ui_egui::theme::apply(ui.ctx(), egui::Theme::Dark, false);
            app.logic(&ui.ctx().clone());
            textstyles::panel(app, ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn button(h: &mut Harness<'static, CadApp>, label: &str) {
    h.get(by().label(label).role(egui::accesskit::Role::Button)).click();
    h.run_steps(3);
}

#[test]
fn text_style_manager_creates_applies_and_undoes_style_changes() {
    let mut h = manager();
    h.get(by().label("Style name").role(egui::accesskit::Role::TextInput)).focus();
    h.run_steps(1);
    h.get(by().label("Style name").role(egui::accesskit::Role::TextInput)).type_text("Recovered");
    h.run_steps(2);
    button(&mut h, "Create");
    assert!(h.state().session.doc().unwrap().text_style("Recovered").is_some());
    h.get_by_label("Backwards").click();
    h.run_steps(2);
    button(&mut h, "Apply properties");
    assert!(h.state().session.doc().unwrap().text_style("Recovered").unwrap().backwards);
    h.state_mut().run("undo", json!({})).unwrap();
    assert!(!h.state().session.doc().unwrap().text_style("Recovered").unwrap().backwards);
    if let Some(path) = std::env::var_os("CADCRAFT_TEXT_STYLE_SCREENSHOT") {
        h.run_steps(2);
        h.render().unwrap().save(path).unwrap();
    }
}

#[test]
fn text_style_manager_font_browser_can_be_cancelled_without_document_changes() {
    let mut h = manager();
    let before = h.state().session.doc().unwrap().text_styles.clone();
    h.get(by().label("Font").role(egui::accesskit::Role::ComboBox)).click();
    h.run_steps(3);
    button(&mut h, "Import font…");
    h.get_by_label("Select a TTF, OTF or SHX font file");
    button(&mut h, "Cancel");
    assert_eq!(h.state().session.doc().unwrap().text_styles, before);
}

#[test]
fn text_style_manager_keeps_pending_edits_with_their_drawing() {
    let mut h = manager();
    let first = h.state().session.active;
    h.get(by().label("Style name").role(egui::accesskit::Role::TextInput)).focus();
    h.run_steps(1);
    h.get(by().label("Style name").role(egui::accesskit::Role::TextInput)).type_text("Pending in first");
    h.run_steps(2);
    h.state_mut().session.new_drawing(false);
    h.run_steps(3);
    assert_eq!(h.get(by().label("Style name").role(egui::accesskit::Role::TextInput)).value().as_deref(), Some(""));
    h.state_mut().session.active = first;
    h.run_steps(3);
    assert_eq!(h.get(by().label("Style name").role(egui::accesskit::Role::TextInput)).value().as_deref(), Some("Pending in first"));
}
