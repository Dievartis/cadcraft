use cadcraft_engine::Session;
use serde_json::json;

#[test]
fn mixed_states_invert_individually_and_undo_together() {
    for property in ["on", "frozen", "locked"] {
        let mut s = Session::new();
        for name in ["Clicked", "A", "B"] {
            s.execute("layer.new", &json!({"name":name})).unwrap();
        }
        s.execute("layer.set", &json!({"name":"A",property:true})).unwrap();
        s.execute("layer.set", &json!({"name":"B",property:false})).unwrap();
        let before = s.execute("layer", &json!({})).unwrap();
        s.execute("layer.invertothers", &json!({"name":"Clicked","property":property})).unwrap();
        let d = s.doc().unwrap();
        let a = d.layer("A").unwrap();
        let b = d.layer("B").unwrap();
        let clicked = d.layer("Clicked").unwrap();
        assert!(clicked.on);
        assert!(!clicked.frozen);
        assert!(!clicked.locked);
        match property {
            "on" => {
                assert!(!a.on);
                assert!(b.on);
            }
            "locked" => {
                assert!(!a.locked);
                assert!(b.locked);
            }
            _ => {
                assert!(!a.frozen);
                assert!(b.frozen);
                assert!(!d.layer("0").unwrap().frozen);
            }
        }
        s.execute("undo", &json!({})).unwrap();
        assert_eq!(s.execute("layer", &json!({})).unwrap(), before);
    }
}

#[test]
fn invalid_property_leaves_layers_unchanged() {
    let mut s = Session::new();
    let before = s.execute("layer", &json!({})).unwrap();
    assert!(s.execute("layer.invertothers", &json!({"name":"0","property":"plot"})).is_err());
    assert_eq!(s.execute("layer", &json!({})).unwrap(), before);
}
