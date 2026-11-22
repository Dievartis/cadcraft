use cadcraft_engine::{
    Session,
    doc::{Drawing, furniture_of},
};
use cadcraft_ui_egui::{
    CadApp, Services,
    menus::{self, Entry},
};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::{Value, json};

fn contains(entries: &[Entry], wanted: &str) -> bool {
    entries.iter().any(|e| match e {
        Entry::Item { id, .. } => id == wanted,
        Entry::Sub { children, .. } => contains(children, wanted),
    })
}
#[test]
fn optional_catalogues_hide_tools_keep_objects_and_survive_preferences_and_profiles() {
    let mut app = CadApp::new(Session::new(), Services::default());
    let r = app.session.execute("furnitureadd", &json!({"at":[0,0],"model":"sofa3"})).unwrap();
    let h = cadcraft_engine::doc::Handle::parse_hex(r["handles"][0].as_str().unwrap()).unwrap();
    assert!(!app.ui.catalogue_tools);
    assert!(!menus::tree(&app).iter().any(|(_, e)| contains(e, "furnitureadd")));
    app.run("options", json!({"catalogueTools":true})).unwrap();
    assert!(menus::tree(&app).iter().any(|(_, e)| contains(e, "furnitureadd")));
    assert!(app.run("options", json!({"catalogueTools":"no"})).is_err());
    assert!(app.ui.catalogue_tools);
    let prefs = app.prefs_json();
    let profile = app.profile_json();
    app.run("options", json!({"catalogueTools":false})).unwrap();
    assert_eq!(furniture_of(app.session.doc().unwrap().entity(h).unwrap()).unwrap().model, "sofa3");
    app.session.execute("furnitureedit", &json!({"handle":h.hex(),"width":2400})).unwrap();
    assert_eq!(furniture_of(app.session.doc().unwrap().entity(h).unwrap()).unwrap().width_mm, 2400.0);
    let mut restored = CadApp::new(Session::new(), Services::default());
    restored.load_prefs(&prefs);
    assert!(restored.ui.catalogue_tools);
    restored.ui.catalogue_tools = false;
    restored.load_profile(&profile);
    assert!(restored.ui.catalogue_tools);
    let saved: Value = serde_json::from_str(&restored.profile_json()).unwrap();
    assert_eq!(saved["catalogueTools"], true);
}

#[test]
fn blocks_palette_offers_catalogue_sources_and_starts_parametric_placement() {
    let mut app = CadApp::new(Session::new(), Services::default());
    *app.session.doc_mut().unwrap() = Drawing::new_metric();
    app.ui.start_tab = false;
    app.ui.in_window_menu = false;
    let shot = std::env::var_os("CADCRAFT_CATALOGUE_SHOT");
    let builder = Harness::builder().with_size(egui::vec2(1500.0, 1000.0));
    let builder = if shot.is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    h.state_mut().run("blockspalette", Value::Null).unwrap();
    h.state_mut().run("ui.dock", json!({"operation":"float","panel":"blocks","rect":[740,40,680,880]})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Catalogues").is_none());
    h.state_mut().run("options", json!({"catalogueTools":true})).unwrap();
    h.run_steps(4);
    h.get_by_label("Catalogues").click();
    h.run_steps(4);
    h.get_by_label("Furniture").click();
    h.run_steps(3);
    assert!(h.query_by_label("All categories").is_some());
    assert!(h.get_by_label("Bedroom").rect().top() > h.get_by_label("Living room").rect().bottom(), "categories form a vertical column");
    assert!(
        h.get_by_label("Sofa, three seats").rect().top() > h.get_by_label("Sofa, two seats").rect().bottom(),
        "catalogue items form separate rows"
    );
    assert!(h.query_by_label("Select one catalogue object to edit its dimensions.").is_some(), "the editor stays visible below the gallery");
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("furniture.png")).unwrap();
    }
    let tall_footer = h.get_by_label("Select one catalogue object to edit its dimensions.").rect().top();
    h.state_mut().run("ui.dock", json!({"operation":"float","panel":"blocks","rect":[740,40,680,560]})).unwrap();
    h.run_steps(5);
    let short_footer = h.get_by_label("Select one catalogue object to edit its dimensions.").rect().top();
    assert!(tall_footer - short_footer > 200.0, "the gallery grows with the window instead of keeping a fixed height");
    h.state_mut().run("ui.dock", json!({"operation":"float","panel":"blocks","rect":[740,40,680,880]})).unwrap();
    h.run_steps(5);
    h.get_by_label("Sofa, two seats").click();
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("furnitureadd"));
    assert!(!h.state().session.preview(cadcraft_engine::geom::Vec2::ZERO).is_empty());
    h.state_mut().session.cancel();
    h.get_by_label("Equipment").click();
    h.run_steps(3);
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("equipment.png")).unwrap();
    }
    let equipment = h.state_mut().session.execute("equipmentadd", &json!({"at":[0,0]})).unwrap();
    let handle = cadcraft_engine::doc::Handle::parse_hex(equipment["handle"].as_str().unwrap()).unwrap();
    h.state_mut().session.state_mut().unwrap().selection = vec![handle];
    h.run_steps(5);
    assert!(
        h.get_by_label("Apply changes").rect().bottom() < h.ctx.content_rect().bottom(),
        "selected-object properties remain reachable below the resized gallery"
    );
    h.state_mut().run("options", json!({"catalogueTools":false})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Catalogues").is_none());
    assert!(h.query_by_label("Current Drawing").is_some());
}

#[test]
fn optional_architecture_has_wall_and_opening_editors_and_a_style_dialog() {
    use cadcraft_engine::doc::{Handle, opening_of, wall_of};
    let mut app = CadApp::new(Session::new(), Services::default());
    *app.session.doc_mut().unwrap() = Drawing::new_metric();
    app.ui.start_tab = false;
    app.ui.in_window_menu = false;
    assert!(!menus::tree(&app).iter().any(|(_, e)| contains(e, "walladd")));
    app.run("options", json!({"architectureTools":true})).unwrap();
    assert!(menus::tree(&app).iter().any(|(_, e)| contains(e, "walladd")));
    let mut restored = CadApp::new(Session::new(), Services::default());
    restored.load_prefs(&app.prefs_json());
    assert!(restored.ui.architecture_tools);
    restored.ui.architecture_tools = false;
    restored.load_profile(&app.profile_json());
    assert!(restored.ui.architecture_tools);
    let wall = app.run("walladd", json!({"points":[[0,0],[6000,0]]})).unwrap();
    let wall = Handle::parse_hex(wall["handle"].as_str().unwrap()).unwrap();
    let door = app.run("openingadd", json!({"wall":wall.hex(),"distance":1200,"type":"door"})).unwrap();
    let door = Handle::parse_hex(door["handle"].as_str().unwrap()).unwrap();
    let builder = Harness::builder().with_size(egui::vec2(1500.0, 1000.0));
    let shot = std::env::var_os("CADCRAFT_ARCHITECTURE_SHOT");
    let builder = if shot.is_some() { builder.wgpu() } else { builder };
    let mut h = builder.build_ui_state(
        |ui, app: &mut CadApp| {
            app.logic(ui.ctx());
            app.ui(ui);
        },
        app,
    );
    h.state_mut().run("blockspalette", Value::Null).unwrap();
    h.state_mut().run("ui.dock", json!({"operation":"float","panel":"blocks","rect":[740,40,680,880]})).unwrap();
    h.run_steps(5);
    h.state_mut().run("zoom", json!({"mode":"extents"})).unwrap();
    h.get_by_label("Catalogues").click();
    h.run_steps(5);
    assert!(h.query_by_label("Draw Wall").is_some());
    h.state_mut().session.state_mut().unwrap().selection = vec![wall];
    h.run_steps(5);
    assert!(h.query_by_label("Selected wall").is_some());
    h.get_by_label("Apply architectural changes").click();
    h.run_steps(4);
    assert!(wall_of(h.state().session.doc().unwrap().entity(wall).unwrap()).is_some());
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("wall.png")).unwrap();
    }
    h.state_mut().session.state_mut().unwrap().selection = vec![door];
    h.run_steps(5);
    assert!(h.query_by_label("Selected opening").is_some());
    h.get_by_label("Apply architectural changes").click();
    h.run_steps(4);
    assert!(opening_of(h.state().session.doc().unwrap().entity(door).unwrap()).is_some());
    h.state_mut().run("wallstyleedit", Value::Null).unwrap();
    h.run_steps(4);
    assert_eq!(h.state().ui.dialog.as_deref(), Some("wallstyles"));
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("styles.png")).unwrap();
    }
    h.state_mut().ui.dialog = None;
    h.state_mut().run("options", json!({"architectureTools":false})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Catalogues").is_none());
    assert!(wall_of(h.state().session.doc().unwrap().entity(wall).unwrap()).is_some());
    assert!(opening_of(h.state().session.doc().unwrap().entity(door).unwrap()).is_some());
}
