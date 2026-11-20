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
    // A missing default must still leave a usable first drawing.
    b.ensure_startup_drawing();
    assert_eq!(b.session.docs.len(), 1);
    b.ensure_startup_drawing();
    assert_eq!(b.session.docs.len(), 1);

    use cadcraft_engine::cmd::file::{IoHooks, set_io, set_qnew_template};
    set_io(IoHooks {
        read: |bytes, path| cadcraft_io::read(bytes, path).map_err(|e| e.to_string()),
        write: |drawing, path| cadcraft_io::write(drawing, path).map_err(|e| e.to_string()),
        plot: None,
    });
    let path = std::env::temp_dir().join(format!("cadcraft-startup-template-{}.dxf", std::process::id()));
    let mut drawing = cadcraft_engine::doc::Drawing::default();
    drawing.header.set("INSUNITS", cadcraft_engine::doc::HVal::Int(4));
    std::fs::write(&path, cadcraft_io::write(&drawing, "template.dxf").unwrap()).unwrap();
    set_qnew_template(&path.to_string_lossy());
    let mut c = CadApp::new(Session::empty(), Services::default());
    c.ensure_startup_drawing();
    assert_eq!(c.session.doc().unwrap().header.i64("INSUNITS", 0), 4);
    assert!(c.session.state().unwrap().path.is_none());
    set_qnew_template("");
    std::fs::remove_file(path).unwrap();
}
