use cadcraft_engine::{
    Session,
    doc::{EntityKind, Handle},
};
use serde_json::{Value, json};
fn handle(v: &Value) -> Handle {
    Handle::parse_hex(v["handle"].as_str().or_else(|| v["handles"][0].as_str()).unwrap()).unwrap()
}
#[test]
fn vertex_window_only_changes_selected_objects_and_undo_restores() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points":[[0,0],[10,0]]})).unwrap());
    let b = handle(&s.execute("line", &json!({"points":[[0,1],[10,1]]})).unwrap());
    let r = s.execute("vertexedit", &json!({"handles":[a.hex()],"first":[9,-2],"second":[11,2],"delta":[5,0]})).unwrap();
    assert_eq!(r["vertices"], 1);
    let EntityKind::Line(line) = &s.doc().unwrap().entity(a).unwrap().kind else { panic!() };
    assert_eq!((line.a.x, line.b.x), (0.0, 15.0));
    let EntityKind::Line(line) = &s.doc().unwrap().entity(b).unwrap().kind else { panic!() };
    assert_eq!(line.b.x, 10.0);
    s.execute("undo", &json!({})).unwrap();
    let EntityKind::Line(line) = &s.doc().unwrap().entity(a).unwrap().kind else { panic!() };
    assert_eq!(line.b.x, 10.0);
}
#[test]
fn xy_scale_converts_circle_without_moving_base() {
    let mut s = Session::new();
    let h = handle(&s.execute("circle", &json!({"center":[10,20],"radius":4})).unwrap());
    s.execute("scalexy", &json!({"handles":[h.hex()],"base":[10,20],"x":2,"y":1})).unwrap();
    let EntityKind::Ellipse(e) = &s.doc().unwrap().entity(h).unwrap().kind else { panic!() };
    assert_eq!((e.center.x, e.center.y, e.major.x, e.ratio), (10.0, 20.0, 8.0, 0.5));
    s.execute("scalexy", &json!({"handles":[h.hex()],"base":[10,20],"x":2,"y":1})).unwrap();
    let EntityKind::Ellipse(e) = &s.doc().unwrap().entity(h).unwrap().kind else { panic!() };
    assert_eq!((e.major.x, e.ratio), (16.0, 0.25));
    assert!(s.execute("scalexy", &json!({"handles":[h.hex()],"base":[0,0],"x":0,"y":1})).is_err());
}

#[test]
fn interactive_vertexedit_keeps_the_original_selection() {
    use cadcraft_engine::{Input, geom::Vec2};
    let mut s = Session::new();
    let h = handle(&s.execute("line", &json!({"points":[[0,0],[10,0]]})).unwrap());
    s.set_selection(vec![h]);
    s.cmdline("vertexedit").unwrap();
    for p in [Vec2::new(9.0, -1.0), Vec2::new(11.0, 1.0), Vec2::ZERO, Vec2::new(2.0, 0.0)] {
        s.input(Input::Point(p)).unwrap();
    }
    let EntityKind::Line(line) = &s.doc().unwrap().entity(h).unwrap().kind else { panic!() };
    assert_eq!((line.a.x, line.b.x), (0.0, 12.0));
}

#[test]
fn new_from_template_loads_relative_title_block_image() {
    use cadcraft_engine::{
        cmd::file::{IoHooks, set_image_decoder, set_io},
        doc::{Common, Drawing, Image, Raster, Space},
        geom::Vec2,
    };
    set_io(IoHooks {
        read: |b, p| cadcraft_io::read(b, p).map_err(|e| e.to_string()),
        write: |d, p| cadcraft_io::write(d, p).map_err(|e| e.to_string()),
        plot: None,
    });
    set_image_decoder(|b| cadcraft_io::raster::decode(b).map_err(|e| e.to_string()));
    let folder = std::env::temp_dir().join(format!("cadcraft-priority-template-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let image_path = folder.join("logo.png");
    let template = folder.join("model.dxf");
    let raster = Raster::new(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
    std::fs::write(&image_path, cadcraft_io::raster::encode_png(&raster).unwrap()).unwrap();
    let mut drawing = Drawing::default();
    drawing
        .add(&Space::Model, Common::default(), EntityKind::Image(Image { path: "logo.png".into(), size: Vec2::new(2.0, 1.0), ..Default::default() }))
        .unwrap();
    std::fs::write(&template, cadcraft_io::write(&drawing, "model.dxf").unwrap()).unwrap();
    let mut s = Session::new();
    s.execute("new", &json!({"template":template.to_string_lossy()})).unwrap();
    assert!(s.state().unwrap().path.is_none());
    assert!(s.doc().unwrap().rasters.get(&image_path.to_string_lossy()).is_some());
    std::fs::remove_file(image_path).unwrap();
    std::fs::remove_file(template).unwrap();
    std::fs::remove_dir(folder).unwrap();
}
