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
    let tall_options = h.get_by_label("Repeat Placement").rect().bottom();
    assert!(h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Scale").rect().is_positive());
    assert!(h.query_by_label("Rotation").is_some());
    assert!(h.get_by_role_and_label(egui::Role::CheckBox, "Explode").rect().is_positive());
    h.state_mut().run("ui.dock", json!({"operation":"float","panel":"blocks","rect":[740,40,680,560]})).unwrap();
    h.run_steps(5);
    let short_options = h.get_by_label("Repeat Placement").rect().bottom();
    assert!(tall_options - short_options > 200.0, "insertion options stay anchored to the bottom when resized");
    h.state_mut().run("ui.dock", json!({"operation":"float","panel":"blocks","rect":[740,40,680,880]})).unwrap();
    h.run_steps(5);
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("blocks.png")).unwrap();
    }
    h.get_by_label("Trade Libraries").click();
    h.run_steps(3);
    assert!(h.query_by_label("Enable trade libraries from Library settings.").is_some());
    h.get_by_label("Library settings").click();
    h.run_steps(3);
    h.get_by_label("Furniture and equipment").click();
    h.run_steps(3);
    assert!(h.state().ui.catalogue_tools);
    h.get_by_label("Library settings").click();
    h.run_steps(4);
    h.get_by_label("Trade Libraries").click();
    h.run_steps(4);
    h.get_by_label("Furniture").click();
    h.run_steps(3);
    assert!(h.query_by_label("Living room").is_some(), "furniture keeps category navigation visible");
    assert!(h.query_by_label("View").is_some());
    assert!(h.query_by_label("Filter").is_some());
    assert!(
        h.get_by_label("Sofa, three seats").rect().top() > h.get_by_label("Sofa, two seats").rect().bottom(),
        "details retain compact model rows"
    );
    verify_direct_help(&mut h);
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("furniture.png")).unwrap();
    }
    verify_furniture_categories(&mut h);
    h.get_by_role_and_label(egui::Role::ComboBox, "View").click();
    h.run_steps(3);
    h.get_by_label("Large Icons").click();
    h.run_steps(4);
    assert!(
        h.get_by_label("Sofa, three seats").rect().left() > h.get_by_label("Sofa, two seats").rect().right(),
        "icon view wraps tiles horizontally"
    );
    if let Some(path) = &shot {
        h.render().unwrap().save(std::path::PathBuf::from(path).with_extension("icons.png")).unwrap();
    }
    h.get_by_label("Sofa, two seats").click();
    h.run_steps(3);
    assert_eq!(h.state().session.running.as_ref().map(|r| r.id.as_str()), Some("furnitureadd"));
    assert!(!h.state().session.preview(cadcraft_engine::geom::Vec2::ZERO).is_empty());
    h.state_mut().session.cancel();
    h.get_by_role_and_label(egui::Role::TextInput, "Filter").focus();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.key_press(egui::Key::Backspace);
    h.run_steps(3);
    verify_equipment_categories(&mut h);
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
    assert!(h.query_by_label("Trade Libraries").is_some());
    assert!(h.query_by_label("Enable trade libraries from Library settings.").is_some());
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
    h.get_by_label("Trade Libraries").click();
    h.run_steps(5);
    assert!(h.query_by_label("Draw Wall").is_some());
    h.get_by_label("Openings").click();
    h.run_steps(3);
    assert!(h.query_by_label("Place Opening").is_some());
    assert!(h.query_by_label("Draw Wall").is_none());
    h.get_by_label("Elevations").click();
    h.run_steps(3);
    assert!(h.query_by_label("Wall Elevation").is_some());
    assert!(h.query_by_label("Place Opening").is_none());
    h.get_by_label("Walls").click();
    h.run_steps(3);
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
    assert!(h.query_by_label("Trade Libraries").is_some());
    assert!(h.query_by_label("Enable trade libraries from Library settings.").is_some());
    assert!(wall_of(h.state().session.doc().unwrap().entity(wall).unwrap()).is_some());
    assert!(opening_of(h.state().session.doc().unwrap().entity(door).unwrap()).is_some());
}

fn verify_direct_help(h: &mut Harness<'static, CadApp>) {
    assert!(h.query_by_label("Select one catalogue object to edit its dimensions.").is_none(), "help does not consume gallery space");
    h.get_by_label("?").hover();
    h.run_steps(45);
    assert!(h.query_by_label("Select one catalogue object to edit its dimensions.").is_some());
    assert!(h.query_by_label("Library help").is_none());
    h.get_by_label("?").click();
    h.run_steps(3);
    assert!(h.query_by_label("Catalogue recipes: freemans32/cadcraft (MIT OR Apache-2.0).").is_some());
    h.get_by_label("?").click();
    h.run_steps(3);
}

fn verify_furniture_categories(h: &mut Harness<'static, CadApp>) {
    assert!(h.get_by_label("Dining").rect().left() < h.get_by_label("Sofa, two seats").rect().left());
    assert!(h.get_by_label("Dining").rect().top() > h.get_by_label("Living room").rect().bottom());
    h.get_by_label("Dining").click();
    h.run_steps(3);
    assert!(h.query_by_label("Table, 4 seats").is_some());
    assert!(h.query_by_label("Sofa, two seats").is_none(), "only the active category is shown");
    h.get_by_label("Living room").click();
    h.run_steps(3);
    assert!(h.query_by_label("Sofa, two seats").is_some());
    h.get_by_role_and_label(egui::Role::TextInput, "Filter").focus();
    h.run_steps(2);
    h.get_by_role_and_label(egui::Role::TextInput, "Filter").type_text("Sofa");
    h.run_steps(3);
    assert_eq!(h.get_by_role_and_label(egui::Role::TextInput, "Filter").value().as_deref(), Some("Sofa"));
    assert!(h.query_by_label("Dining").is_some(), "navigation remains available while filtering");
    h.get_by_label("Dining").click();
    h.run_steps(3);
    assert!(h.query_by_label("No block matches the filter.").is_some());
    assert!(h.query_by_label("Sofa, two seats").is_none());
    h.get_by_label("Living room").click();
    h.run_steps(3);
}

fn verify_equipment_categories(h: &mut Harness<'static, CadApp>) {
    h.get_by_label("Equipment").click();
    h.run_steps(3);
    assert!(h.query_by_label("Chiller 41.7 kW").is_some());
    assert!(h.query_by_label("VRV 4 CV").is_none());
    h.get_by_label("VRF outdoor units").click();
    h.run_steps(3);
    assert!(h.query_by_label("VRV 4 CV").is_some());
    assert!(h.query_by_label("Chiller 41.7 kW").is_none());
    h.get_by_label("Split outdoor units").click();
    h.run_steps(3);
    assert!(h.query_by_label("Split outdoor 2 kW").is_some());
    assert!(h.query_by_label("VRV 4 CV").is_none());
    assert!(
        h.get_by_label("Place Split outdoor 7.1 kW").rect().top() > h.get_by_label("Place Split outdoor 2 kW").rect().bottom(),
        "tiles wrap instead of extending past the explorer"
    );
    assert!(h.get_by_label("Place Split outdoor 7.1 kW").rect().right() < h.ctx.content_rect().right());
    let first = h.get_by_label("Place Split outdoor 2 kW").rect();
    let last = h.get_by_label("Place Split outdoor 7.1 kW").rect();
    assert!(last.top() - first.bottom() < first.height(), "wrapped rows remain compact rather than reserving the viewport height");
    h.get_by_label("Furniture").click();
    h.run_steps(3);
    assert!(h.query_by_label("Sofa, two seats").is_some());
    h.get_by_label("Equipment").click();
    h.run_steps(3);
    assert!(h.query_by_label("Split outdoor 2 kW").is_some(), "each trade library retains its own active category");
}
