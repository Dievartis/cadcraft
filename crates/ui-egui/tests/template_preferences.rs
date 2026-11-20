//! Separate test process: QNEWTEMPLATE is an application-wide profile setting.
use cadcraft_engine::Session;
use cadcraft_ui_egui::{CadApp, Services};
use serde_json::Value;

#[test]
fn default_template_preference_roundtrip() {
    let mut a = CadApp::new(Session::empty(), Services::default());
    a.load_prefs(r#"{"defaultTemplate":"drawing-model.dwt"}"#);
    let saved: Value = serde_json::from_str(&a.prefs_json()).unwrap();
    assert_eq!(saved["defaultTemplate"], "drawing-model.dwt");
    cadcraft_engine::cmd::file::set_qnew_template("");
    let mut b = CadApp::new(Session::empty(), Services::default());
    b.load_prefs(&saved.to_string());
    assert_eq!(cadcraft_engine::cmd::file::qnew_template(), "drawing-model.dwt");
}
