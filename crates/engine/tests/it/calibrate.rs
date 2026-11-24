//! CALIBRATE: two points and a true length scale a drawing (#1181), the discoverable front end
//! for SCALE ▸ Reference with an apply target (everything, the selection, an underlay).

use cadcraft_doc::{EntityKind, Handle};
use cadcraft_engine::Session;
use cadcraft_geom::Vec2;
use serde_json::json;

/// The first handle of a LINE the command just drew.
fn handle(v: &serde_json::Value) -> Handle {
    Handle::parse_hex(v["handles"][0].as_str().unwrap()).unwrap()
}

/// A line's two ends.
fn ends(s: &Session, h: Handle) -> (Vec2, Vec2) {
    let d = s.doc().unwrap();
    let e = d.entity(h).expect("the object is in the drawing");
    let EntityKind::Line(l) = &e.kind else { panic!("expected a line") };
    (l.a.xy(), l.b.xy())
}

/// Compare two points at the tolerance the CALIBRATE tests accept, so valid scaling arithmetic is
/// compared without exact-float brittleness.
fn near(a: Vec2, b: Vec2) -> bool {
    a.near(b, 1e-9)
}

/// The JSON form with no `apply` scales every object about `p1`, so the measured segment becomes
/// exactly `length` and objects elsewhere move by the same factor.
#[test]
fn json_scales_everything_by_the_true_length() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let b = handle(&s.execute("line", &json!({"points": [[10, 0], [10, 5]]})).unwrap());

    // No `apply`: everything is scaled about the first calibration point.
    let r = s.execute("calibrate", &json!({"p1": [0, 0], "p2": [5, 0], "length": 1250})).unwrap();
    assert_eq!(r["apply"], "all", "{r}");
    assert_eq!(r["scaled"], 2, "{r}");
    assert!((r["factor"].as_f64().unwrap() - 250.0).abs() < 1e-9, "{r}");
    assert!((r["measured"].as_f64().unwrap() - 5.0).abs() < 1e-9, "{r}");
    assert!((r["length"].as_f64().unwrap() - 1250.0).abs() < 1e-9, "{r}");

    // The measured segment is now 1250 long; the other object scaled with it.
    let (a1, a2) = ends(&s, a);
    assert!(near(a1, Vec2::ZERO) && near(a2, Vec2::new(1250.0, 0.0)), "{a1:?} {a2:?}");
    let (b1, b2) = ends(&s, b);
    assert!(near(b1, Vec2::new(2500.0, 0.0)) && near(b2, Vec2::new(2500.0, 1250.0)), "{b1:?} {b2:?}");
}

/// `apply: "selection"` scales only the named handles, and an explicit empty selection is an error
/// that leaves the drawing untouched instead of silently falling back to the whole drawing.
#[test]
fn json_selection_leaves_other_objects_untouched() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let b = handle(&s.execute("line", &json!({"points": [[10, 0], [10, 5]]})).unwrap());

    let r = s.execute("calibrate", &json!({"handles": [a.hex()], "p1": [0, 0], "p2": [5, 0], "length": 1250, "apply": "selection"})).unwrap();
    assert_eq!(r["apply"], "selection", "{r}");
    assert_eq!(r["scaled"], 1, "{r}");
    let (a1, a2) = ends(&s, a);
    assert!(near(a1, Vec2::ZERO) && near(a2, Vec2::new(1250.0, 0.0)), "{a1:?} {a2:?}");
    let (b1, b2) = ends(&s, b);
    assert!(near(b1, Vec2::new(10.0, 0.0)) && near(b2, Vec2::new(10.0, 5.0)), "untouched: {b1:?} {b2:?}");

    // An empty selection is a clear error, not a panic, and changes nothing.
    let model = s.doc().unwrap().model.len();
    let e =
        s.execute("calibrate", &json!({"handles": [], "p1": [0, 0], "p2": [5, 0], "length": 1250, "apply": "selection"})).unwrap_err().to_string();
    assert!(e.to_lowercase().contains("selected"), "{e}");
    assert_eq!(s.doc().unwrap().model.len(), model);
}

/// Naming handles without an `apply` target means the selection: the rest of the drawing is never
/// scaled by accident.
#[test]
fn json_handles_default_to_the_selection() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let b = handle(&s.execute("line", &json!({"points": [[10, 0], [10, 5]]})).unwrap());

    // Objects named but no `apply`: only they scale, never the whole drawing.
    let r = s.execute("calibrate", &json!({"handles": [a.hex()], "p1": [0, 0], "p2": [5, 0], "length": 1250})).unwrap();
    assert_eq!(r["apply"], "selection", "{r}");
    assert_eq!(r["scaled"], 1, "{r}");
    let (_, a2) = ends(&s, a);
    assert!(near(a2, Vec2::new(1250.0, 0.0)), "{a2:?}");
    let (b1, b2) = ends(&s, b);
    assert!(near(b1, Vec2::new(10.0, 0.0)) && near(b2, Vec2::new(10.0, 5.0)), "untouched: {b1:?} {b2:?}");
}

/// Hostile inputs — coincident points, a distance that overflows from finite coordinates, zero,
/// negative or textual lengths, a malformed point and an unknown target — each return a message and
/// change nothing, guarding against division by zero and panics.
#[test]
fn json_refuses_degenerate_distances_and_bad_lengths() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let before = ends(&s, a);

    for p in [
        // The two points coincide.
        json!({"p1": [2, 2], "p2": [2, 2], "length": 1250}),
        // ... and the distance may overflow with finite inputs.
        json!({"p1": [1e308, 0], "p2": [-1e308, 0], "length": 1250}),
        json!({"p1": [0, 0], "p2": [5, 0], "length": 0}),
        json!({"p1": [0, 0], "p2": [5, 0], "length": -5}),
        json!({"p1": [0, 0], "p2": [5, 0], "length": "1250"}),
        json!({"p1": "x", "p2": [5, 0], "length": 1250}),
        json!({"p1": [0, 0], "p2": [5, 0], "length": 1250, "apply": "somewhere"}),
    ] {
        let e = s.execute("calibrate", &p).unwrap_err().to_string();
        assert!(!e.is_empty(), "no message for {p}");
    }
    assert_eq!(ends(&s, a), before, "a refused calibration changes nothing");
}

/// The interactive sequence asks for the two points, rejects the same point twice with a clear
/// message and asks again, then takes the true length and Enter at the target prompt to scale
/// everything — the default a user gets without choosing.
#[test]
fn command_line_picks_two_points_the_length_and_everything() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let b = handle(&s.execute("line", &json!({"points": [[10, 0], [10, 5]]})).unwrap());

    s.cmdline("calibrate").unwrap();
    assert_eq!(s.current_prompt().unwrap().message, "Specify first point of the distance to calibrate");
    s.cmdline("0,0").unwrap();
    // The same point twice is reported clearly and the second point is asked for again.
    s.cmdline("0,0").unwrap();
    assert!(s.running.is_some());
    assert!(s.log.iter().any(|l| l.contains("different")), "{:?}", s.log);
    assert_eq!(s.current_prompt().unwrap().message, "Specify second point of the distance to calibrate");
    s.cmdline("5,0").unwrap();
    assert_eq!(s.current_prompt().unwrap().message, "Enter the true length of that distance");
    s.cmdline("1250").unwrap();
    let p = s.current_prompt().unwrap();
    assert!(p.message.starts_with("Apply the scale to"), "{}", p.display());
    assert_eq!(p.default.as_deref(), Some("Everything"));
    assert_eq!(p.keywords, ["Everything", "Selection", "Underlay"]);
    // Enter takes the default: everything.
    s.cmdline("").unwrap();
    assert!(s.running.is_none());
    let (a1, a2) = ends(&s, a);
    assert!(near(a1, Vec2::ZERO) && near(a2, Vec2::new(1250.0, 0.0)), "{a1:?} {a2:?}");
    let (b1, b2) = ends(&s, b);
    assert!(near(b1, Vec2::new(2500.0, 0.0)) && near(b2, Vec2::new(2500.0, 1250.0)), "{b1:?} {b2:?}");
}

/// Choosing Selection at the last prompt applies the calibration to the objects already selected
/// and leaves every other object exactly where it was.
#[test]
fn command_line_selection_scales_only_what_was_selected() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let b = handle(&s.execute("line", &json!({"points": [[10, 0], [10, 5]]})).unwrap());
    s.set_selection(vec![b]);

    for t in ["calibrate", "0,0", "5,0", "1250", "s"] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let (a1, a2) = ends(&s, a);
    assert!(near(a1, Vec2::ZERO) && near(a2, Vec2::new(5.0, 0.0)), "untouched: {a1:?} {a2:?}");
    let (b1, b2) = ends(&s, b);
    assert!(near(b1, Vec2::new(2500.0, 0.0)) && near(b2, Vec2::new(2500.0, 1250.0)), "{b1:?} {b2:?}");
}

/// Choosing Selection with nothing preselected asks for objects, and an empty pick reports
/// "Nothing selected" and ends without scaling anything.
#[test]
fn command_line_selection_with_nothing_selected_scales_nothing() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let before = ends(&s, a);

    for t in ["calibrate", "0,0", "5,0", "1250", "s"] {
        s.cmdline(t).unwrap();
    }
    assert_eq!(s.current_prompt().unwrap().message, "Select objects");
    s.cmdline("").unwrap();
    assert!(s.running.is_none());
    assert!(s.log.iter().any(|l| l.contains("Nothing selected")), "{:?}", s.log);
    assert_eq!(ends(&s, a), before);
}

/// A fake PDF reader: every file has one A4 landscape page, drawn white.
fn pdf_hooks() {
    use cadcraft_engine::cmd::underlay::{PdfHooks, set_pdf};
    set_pdf(PdfHooks {
        page: |bytes, _| if bytes.starts_with(b"%PDF") { Ok((1, 297.0, 210.0, None)) } else { Err("not a readable PDF file".into()) },
        render: |_, _, side| Ok((side, side, vec![255; (side * side * 4) as usize])),
        lines: |_, _| Ok(Vec::new()),
        region: |_, _, _, w, h| Ok((w, h, vec![255; (w * h * 4) as usize])),
        layers: |_| Ok(Vec::new()),
        with_layers: |bytes, _| Ok(bytes.to_vec()),
    });
}

/// A PDF file attached as an underlay at `at`, and the same bytes as a raster image at `at` too: the two backdrops a plan can have.
fn attach_backdrops(s: &mut Session, tag: &str, at: [f64; 2]) -> (Handle, Handle) {
    pdf_hooks();
    // One folder for each test: tests run side by side, and a file (and its cache) is not shared between them.
    let dir = std::env::temp_dir().join(format!("cadcraft-calibrate-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("plan.pdf");
    std::fs::write(&file, b"%PDF-1.7 fake").unwrap();
    let r = s.execute("pdfattach", &json!({"path": file.to_string_lossy(), "at": at})).unwrap();
    let underlay = Handle::parse_hex(r["handle"].as_str().or_else(|| r["handles"][0].as_str()).unwrap()).unwrap();
    let data = cadcraft_engine::cmd::file::base64_encode(b"%PDF-1.7 fake");
    let r = s.execute("pdfattach", &json!({"data": data, "name": format!("scan-{tag}.pdf"), "insertion": [at[0], at[1], 0], "width": 100})).unwrap();
    (underlay, Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap())
}

/// An underlay's insertion point and displayed size, and an image's.
fn backdrop(s: &Session, h: Handle) -> (Vec2, f64) {
    match &s.doc().unwrap().entity(h).unwrap().kind {
        EntityKind::PdfUnderlay(u) => (u.insert.xy(), u.size.x * u.scale),
        EntityKind::Image(i) => (i.insert.xy(), i.u.x.hypot(i.u.y) * i.size.x),
        k => panic!("not a backdrop: {k:?}"),
    }
}

/// The Underlay target scales the PDF underlays and the raster images, about the first point, and nothing else.
#[test]
fn json_underlay_scales_the_backdrops_about_the_first_point_and_nothing_else() {
    let mut s = Session::new();
    let line = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let (pdf, image) = attach_backdrops(&mut s, "scales", [100.0, 50.0]);
    let (pdf_before, image_before) = (backdrop(&s, pdf), backdrop(&s, image));
    let r = s.execute("calibrate", &json!({"p1": [0, 0], "p2": [10, 0], "length": 20, "apply": "underlay"})).unwrap();
    assert_eq!((r["apply"].as_str(), r["scaled"].as_u64()), (Some("underlay"), Some(2)), "{r}");
    for (h, before) in [(pdf, pdf_before), (image, image_before)] {
        let (at, width) = backdrop(&s, h);
        assert!(near(at, before.0 * 2.0), "the corner moves away from p1: {at:?}");
        assert!((width - before.1 * 2.0).abs() < 1e-6 * before.1, "{width} {}", before.1);
    }
    let (a, b) = ends(&s, line);
    assert!(near(a, Vec2::ZERO) && near(b, Vec2::new(5.0, 0.0)), "the drawing did not move: {a:?} {b:?}");
}

/// Without a backdrop, and with only locked ones, the Underlay target is a clear error and changes nothing.
#[test]
fn json_underlay_without_one_or_with_locked_ones_is_a_clear_error() {
    let mut s = Session::new();
    let line = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let apply = json!({"p1": [0, 0], "p2": [10, 0], "length": 20, "apply": "underlay"});
    let e = s.execute("calibrate", &apply).unwrap_err().to_string();
    assert!(e.contains("There is no PDF underlay or raster image to scale"), "{e}");
    assert_eq!(ends(&s, line), (Vec2::ZERO, Vec2::new(5.0, 0.0)));
    let (pdf, image) = attach_backdrops(&mut s, "locked", [10.0, 10.0]);
    let before = (backdrop(&s, pdf), backdrop(&s, image));
    for h in [pdf, image] {
        let layer = s.doc().unwrap().entity(h).unwrap().common.layer.clone();
        s.execute("layer.set", &json!({"name": layer, "locked": true})).unwrap();
    }
    let e = s.execute("calibrate", &apply).unwrap_err().to_string();
    assert!(e.contains("locked layers"), "{e}");
    assert_eq!((backdrop(&s, pdf), backdrop(&s, image)), before, "locked backdrops stay as they are");
}

/// At the command line, `u` scales the backdrops and ends the command; with none it reports, stays at the prompt and Everything still ends it.
#[test]
fn command_line_underlay_scales_the_backdrops_and_without_one_is_a_clear_refusal() {
    let mut s = Session::new();
    let a = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let before = ends(&s, a);
    for t in ["calibrate", "0,0", "5,0", "1250", "u"] {
        s.cmdline(t).unwrap();
    }
    // The command stays at the apply prompt, so another target can be chosen.
    assert!(s.running.is_some());
    assert!(s.log.iter().any(|l| l.contains("There is no PDF underlay or raster image to scale")), "{:?}", s.log);
    assert_eq!(s.current_prompt().unwrap().message, "Apply the scale to");
    assert_eq!(ends(&s, a), before);
    // Everything still ends it.
    s.cmdline("e").unwrap();
    assert!(s.running.is_none());
    let (_, a2) = ends(&s, a);
    assert!(near(a2, Vec2::new(1250.0, 0.0)), "{a2:?}");
    // With a backdrop, Underlay scales it and ends the command.
    let mut s = Session::new();
    let line = handle(&s.execute("line", &json!({"points": [[0, 0], [5, 0]]})).unwrap());
    let (pdf, _) = attach_backdrops(&mut s, "cmdline", [0.0, 0.0]);
    let width = backdrop(&s, pdf).1;
    for t in ["calibrate", "0,0", "5,0", "10", "u"] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none(), "{}", s.prompt_text());
    assert!((backdrop(&s, pdf).1 - width * 2.0).abs() < 1e-6 * width);
    assert_eq!(ends(&s, line), (Vec2::ZERO, Vec2::new(5.0, 0.0)));
}
