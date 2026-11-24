//! Raster images (#956): IMAGEATTACH from a file and from handed-over bytes, the pixels loaded
//! again when the drawing is reopened, a missing file left as a frame, and bad input refused.

use std::path::{Path, PathBuf};

use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, base64_encode, set_image_decoder, set_io};
use cadcraft_engine::doc::{Drawing, EntityKind, Image, Raster};
use serde_json::{Value, json};

fn hooks() {
    set_io(IoHooks {
        read: |b, name| cadcraft_io::read(b, name).map_err(|e| e.to_string()),
        write: |d, name| cadcraft_io::write(d, name).map_err(|e| e.to_string()),
        plot: None,
    });
    set_image_decoder(|b| cadcraft_io::raster::decode(b).map_err(|e| e.to_string()));
    cadcraft_engine::cmd::underlay::set_pdf(cadcraft_engine::cmd::underlay::PdfHooks {
        page: cadcraft_io::pdf_read::page_info,
        render: cadcraft_io::pdf_read::render_page,
        lines: cadcraft_io::pdf_read::page_lines,
        region: cadcraft_io::pdf_read::render_page_region,
        layers: cadcraft_io::pdf_layers::layer_list,
        with_layers: cadcraft_io::pdf_layers::apply_layers,
    });
}

/// A fresh folder for one test.
fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cadcraft-image-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A PNG file of 4 x 2 pixels.
fn png() -> Vec<u8> {
    let px: Vec<u8> = (0..8u8).flat_map(|i| [i * 30, 255 - i * 30, 7, 255]).collect();
    cadcraft_io::raster::encode_png(&Raster::new(4, 2, px).unwrap()).unwrap()
}

/// An original two-page PDF with red and blue rectangles, generated entirely in memory.
fn pdf() -> Vec<u8> {
    let content = "1 0 0 rg 0 0 72 36 re f\n";
    let blue = "0 0 1 rg 0 0 72 36 re f\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 36] /Resources << >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 36] /Resources << >> /Contents 6 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{blue}endstream", blue.len()),
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    bytes
}

#[test]
fn test_pdfattach_page_occupies_world_rectangle() {
    hooks();
    let mut s = Session::new();
    s.execute("line", &json!({ "points": [[0, 0], [1, 1]] })).unwrap();
    s.execute(
        "pdfattach",
        &json!({ "name": "sheet.pdf", "data": base64_encode(&pdf()), "page": 1, "insertion": [10, 20, 3], "width": 100, "height": 80 }),
    )
    .unwrap();
    let i = images(&s).pop().unwrap();
    assert_eq!((i.insert.x, i.insert.y, i.insert.z), (10.0, 20.0, 3.0));
    assert!((i.u.x * i.size.x - 100.0).abs() < 1e-10);
    assert!((i.v.y * i.size.y - 80.0).abs() < 1e-10);
    assert_eq!((i.u.y, i.u.z, i.v.x, i.v.z), (0.0, 0.0, 0.0, 0.0));
    let d = s.doc().unwrap();
    assert_eq!(d.model.len(), 2, "attaching keeps the existing drawing");
    let raster = d.rasters.get(&i.path).unwrap();
    assert!(raster.width() > 0 && raster.height() > 0);
    let list = cadcraft_engine::render::build(d, &cadcraft_engine::doc::Space::Model, &cadcraft_engine::render::Options::default());
    assert_eq!(list.images.len(), 1, "the page uses the existing image renderer");
    s.execute("undo", &json!({})).unwrap();
    assert!(images(&s).is_empty());
    assert_eq!(s.doc().unwrap().model.len(), 1);
    s.execute("redo", &json!({})).unwrap();
    let restored = images(&s).pop().unwrap();
    assert!(s.doc().unwrap().rasters.get(&restored.path).is_some(), "redo keeps the page pixels");
}

#[test]
fn pdf_bytes_select_pages_and_keep_same_named_attachments_separate() {
    hooks();
    let mut s = Session::new();
    let data = base64_encode(&pdf());
    let first = s.execute("pdfattach", &json!({"data": data, "name": "sheet.pdf"})).unwrap();
    let second = s.execute("pdfattach", &json!({"data": data, "name": "sheet.pdf", "page": 2})).unwrap();
    let third = s.execute("pdfattach", &json!({"data": data, "name": "sheet.pdf"})).unwrap();
    // Without `width`: the page at its size on paper, in the drawing's units (the fake page is 2:1).
    let (_, w_mm, h_mm, _) = cadcraft_io::pdf_read::page_info(&pdf(), 1).unwrap();
    let paper = w_mm / cadcraft_engine::cmd::underlay::mm_per_unit(s.doc().unwrap());
    let size = first["size"].as_array().unwrap();
    assert!((size[0].as_f64().unwrap() - paper).abs() < 1e-9 * paper, "{size:?} against {paper}");
    assert!((size[1].as_f64().unwrap() - paper * h_mm / w_mm).abs() < 1e-9 * paper, "{size:?}");
    assert_ne!(first["path"], second["path"]);
    assert_ne!(first["path"], third["path"]);
    let d = s.doc().unwrap();
    let red = d.rasters.get(first["path"].as_str().unwrap()).unwrap();
    let blue = d.rasters.get(second["path"].as_str().unwrap()).unwrap();
    assert!(red.width() <= 4096 && red.height() <= 4096);
    assert!(red.rgba().as_chunks::<4>().0.contains(&[255, 0, 0, 255]), "page one is red");
    assert!(blue.rgba().as_chunks::<4>().0.contains(&[0, 0, 255, 255]), "page two is blue");
    assert_eq!(d.model.len(), 3);
}

#[test]
fn invalid_pdf_bytes_and_placement_leave_the_drawing_unchanged() {
    hooks();
    let mut s = Session::new();
    s.execute("line", &json!({"points": [[0, 0], [1, 1]]})).unwrap();
    let data = base64_encode(&pdf());
    for fields in [
        json!({"data": null}),
        json!({"data": "!"}),
        json!({"data": base64_encode(b"not a PDF")}),
        json!({"page": 0}),
        json!({"page": 3}),
        json!({"page": 1.5}),
        json!({"page": 4294967296_u64}),
        json!({"insertion": [0, 0]}),
        json!({"insertion": [0, "x", 0]}),
        json!({"width": 0}),
        json!({"width": -1}),
        json!({"height": "80"}),
        json!({"insertion": [1e308, 0, 0], "width": 1e308}),
        json!({"name": "x".repeat(4097)}),
        json!({"path": "sheet.pdf"}),
        json!({"pages": [1, 2]}),
        json!({"scale": 2}),
    ] {
        let mut params = json!({"data": data});
        params.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
        assert!(s.execute("pdfattach", &params).is_err(), "{fields}");
        assert!(images(&s).is_empty());
        assert_eq!(s.doc().unwrap().model.len(), 1);
    }
    let oversized = "A".repeat(cadcraft_engine::cmd::MAX_PDF_BYTES.div_ceil(3) * 4 + 1);
    assert!(s.execute("pdfattach", &json!({"data": oversized})).unwrap_err().to_string().contains("64 MiB"));
    assert_eq!(s.doc().unwrap().model.len(), 1);
}

fn images(s: &Session) -> Vec<Image> {
    let d = s.doc().unwrap();
    d.model.iter().filter_map(|e| if let EntityKind::Image(i) = &e.kind { Some(i.clone()) } else { None }).collect()
}

fn open(s: &mut Session, path: &Path) {
    s.execute("open", &json!({ "path": path.to_string_lossy() })).unwrap();
}

fn listed(s: &mut Session) -> Value {
    s.execute("image", &json!({})).unwrap()["images"].clone()
}

#[test]
fn image_dpi_sizing_converts_physical_units() {
    hooks();
    let mut s = Session::new();
    for unit_mm in [1.0, 25.4, 1000.0] {
        let r = s.execute("imageattach", &json!({"path":"dpi.png", "data":base64_encode(&png()), "dpi":300, "unitMm":unit_mm})).unwrap();
        let width = r["size"][0].as_f64().unwrap();
        assert!((width * unit_mm - 4.0 / 300.0 * 25.4).abs() < 1e-10);
    }
    assert!(s.execute("imageattach", &json!({"path":"dpi.png", "dpi":0})).is_err());
}

#[test]
fn image_snaps_include_corners_midpoints_and_center() {
    use cadcraft_engine::{
        doc::Space,
        geom::Vec2,
        snap::{mode, osnap},
    };
    hooks();
    let mut s = Session::new();
    s.execute("imageattach", &json!({"path":"snap.png", "data":base64_encode(&png()), "width":8})).unwrap();
    for (flag, x, y) in [(mode::END, 8.0, 4.0), (mode::MID, 4.0, 4.0), (mode::CEN, 4.0, 2.0)] {
        let p = Vec2::new(x, y);
        let hit = osnap(s.doc().unwrap(), &Space::Model, p, 0.1, flag, None, false).unwrap();
        assert!(hit.point.dist(p) < 1e-10);
    }
}

#[test]
fn image_edit_rotation_is_absolute_and_invalid_values_are_atomic() {
    hooks();
    let mut s = Session::new();
    let result = s.execute("imageattach", &json!({"path":"edit.png", "data":base64_encode(&png()), "width":8})).unwrap();
    let handle = result["handle"].clone();
    s.execute("imageedit", &json!({"handle":handle, "rotation":90, "brightness":70, "width":16})).unwrap();
    let first = images(&s).pop().unwrap();
    s.execute("imageedit", &json!({"handle":handle, "rotation":90})).unwrap();
    let second = images(&s).pop().unwrap();
    assert!((first.u.x - second.u.x).abs() < 1e-10);
    assert!((second.u.y * second.size.x - 16.0).abs() < 1e-10);
    assert_eq!(second.brightness, 70);
    assert!(s.execute("imageedit", &json!({"handle":handle, "brightness":20, "fade":101})).is_err());
    assert_eq!(images(&s).pop().unwrap(), second);
}

#[test]
fn attach_from_bytes_places_the_image() {
    hooks();
    let mut s = Session::new();
    let r = s.execute("imageattach", &json!({ "path": "maps/site.png", "data": base64_encode(&png()), "at": [10, 20], "width": 100 })).unwrap();
    assert_eq!((r["name"].as_str(), r["pixels"].clone(), r["size"].clone()), (Some("site"), json!([4.0, 2.0]), json!([100.0, 50.0])));
    let i = images(&s).pop().unwrap();
    assert_eq!((i.insert.x, i.insert.y, i.u.x, i.u.y, i.v.x, i.v.y), (10.0, 20.0, 25.0, 0.0, 0.0, 25.0));
    assert_eq!((i.size.x, i.size.y, i.path.as_str(), i.name.as_str()), (4.0, 2.0, "maps/site.png", "site"));
    assert_eq!(s.doc().unwrap().rasters.get("maps/site.png").map(|r| (r.width(), r.height())), Some((4, 2)));
    let l = listed(&mut s);
    assert_eq!((l[0]["status"].as_str(), l[0]["references"].as_u64(), l[0]["pixels"].clone()), (Some("Loaded"), Some(1), json!([4, 2])));

    // A second reference to the same file shares the definition and the pixels; another file
    // with the same file name gets its own name.
    s.execute("imageattach", &json!({ "path": "maps/site.png", "at": [200, 0], "rotation": 90, "scale": 8 })).unwrap();
    let r = s.execute("attach", &json!({ "path": "other/site.png", "data": base64_encode(&png()) })).unwrap();
    assert_eq!(r["name"], "site_2");
    let all = images(&s);
    assert_eq!(all.len(), 3);
    assert_eq!(all[1].name, "site");
    assert!((all[1].u.x).abs() < 1e-12 && (all[1].u.y - 2.0).abs() < 1e-12 && (all[1].v.x + 2.0).abs() < 1e-12, "{:?}", all[1]);
    assert_eq!(listed(&mut s)[0]["references"], 2);

    // Undo takes the reference away; the drawing draws with its pixels.
    s.execute("undo", &json!({})).unwrap();
    assert_eq!(images(&s).len(), 2);
    let list = cadcraft_engine::render::build(s.doc().unwrap(), &cadcraft_engine::doc::Space::Model, &cadcraft_engine::render::Options::default());
    assert_eq!(list.images.len(), 2);
}

#[test]
fn a_file_is_found_next_to_the_drawing_and_loaded_again_on_open() {
    hooks();
    let dir = folder("file");
    std::fs::write(dir.join("map.png"), png()).unwrap();
    let host = dir.join("host.dxf");
    std::fs::write(&host, cadcraft_io::write(&Drawing::new_metric(), "host.dxf").unwrap()).unwrap();
    let mut s = Session::new();
    open(&mut s, &host);
    let r = s.execute("imageattach", &json!({ "path": "map.png" })).unwrap();
    assert_eq!(r["size"], json!([1.0, 0.5]), "one drawing unit wide by default");
    s.execute("qsave", &json!({})).unwrap();

    // Reopened: the pixels come back from the file.
    let mut s = Session::new();
    open(&mut s, &host);
    assert_eq!(images(&s).len(), 1);
    assert!(s.doc().unwrap().rasters.get("map.png").is_some());
    assert!(!s.state().unwrap().is_dirty(), "loading images is not a change");
    let l = listed(&mut s);
    assert_eq!(l[0]["status"], "Loaded");
    assert!(l[0]["foundAt"].as_str().is_some_and(|f| f.ends_with("map.png")));

    // The file gone: the drawing still opens, says so, and keeps the reference.
    std::fs::remove_file(dir.join("map.png")).unwrap();
    let mut s = Session::new();
    open(&mut s, &host);
    assert_eq!(images(&s).len(), 1);
    assert!(s.doc().unwrap().rasters.get("map.png").is_none());
    assert!(s.log.iter().any(|l| l.contains("map.png") && l.contains("file not found")), "{:?}", s.log);
    assert_eq!(listed(&mut s)[0]["status"], "Not Found");

    // A file that is not an image is reported the same way.
    std::fs::write(dir.join("map.png"), b"not an image").unwrap();
    let mut s = Session::new();
    open(&mut s, &host);
    assert!(s.doc().unwrap().rasters.get("map.png").is_none());
    assert!(s.log.iter().any(|l| l.contains("map.png")), "{:?}", s.log);
    assert_eq!(listed(&mut s)[0]["status"], "Not loaded");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_input_is_refused_and_changes_nothing() {
    hooks();
    let mut s = Session::new();
    let data = base64_encode(&png());
    for p in [
        json!({}),
        json!({ "path": "  " }),
        json!({ "path": "nowhere/missing.png" }),
        json!({ "path": "a.png", "data": "***" }),
        json!({ "path": "a.png", "data": 5 }),
        json!({ "path": "a.png", "data": base64_encode(b"plain text") }),
        json!({ "path": "a.png", "data": data, "scale": 0 }),
        json!({ "path": "a.png", "data": data, "width": -3 }),
        json!({ "path": "a.png", "data": data, "width": "wide" }),
        json!({ "path": "a.png", "data": data, "width": 1e-320 }),
        // A value of the wrong type is refused, not taken as omitted.
        json!({ "path": "a.png", "data": data, "scale": "big" }),
        json!({ "path": "a.png", "data": data, "width": 2, "scale": "big" }),
        json!({ "path": "a.png", "data": data, "rotation": "right" }),
        json!({ "path": "a.png", "data": data, "at": "nowhere" }),
        json!({ "path": "a.png", "data": data, "at": [1] }),
        json!({ "path": "a.png", "data": data, "name": 7 }),
        json!({ "path": "x".repeat(5000), "data": data }),
    ] {
        assert!(s.execute("imageattach", &p).is_err(), "{p}");
    }
    assert!(images(&s).is_empty());
    assert!(s.execute("attach", &json!({ "path": "plan.pdf" })).unwrap_err().to_string().contains("not available yet"));
    assert_eq!(listed(&mut s), json!([]));
}

/// `data` longer than the largest image file encodes to is refused before it is decoded.
#[test]
fn oversized_data_is_refused_before_decoding() {
    hooks();
    const LIMIT: u64 = 256 << 20;
    let mut s = Session::new();
    // Not base64 at all: only the check before decoding tells it is too long ("not valid base64"
    // otherwise), and nothing is decoded.
    let too_long = "!".repeat(usize::try_from(LIMIT.div_ceil(3) * 4 + 1).unwrap());
    let e = s.execute("imageattach", &json!({ "path": "a.png", "data": too_long })).unwrap_err().to_string();
    assert!(e.contains("`data` is larger than 256 MB"), "{e}");
    // Line breaks don't count: base64 wrapped as MIME writes it is taken.
    let wrapped: String = base64_encode(&png()).as_bytes().chunks(76).map(|c| format!("{}\r\n", String::from_utf8_lossy(c))).collect();
    s.execute("imageattach", &json!({ "path": "wrapped.png", "data": wrapped })).unwrap();
    s.execute("undo", &json!({})).unwrap();

    // The longest text allowed, holding one byte more than the limit, is refused once decoded.
    let groups = usize::try_from(LIMIT / 3).unwrap();
    let one_over = format!("{}AAA=", "AAAA".repeat(groups));
    assert_eq!(u64::try_from(one_over.len()).unwrap(), LIMIT.div_ceil(3) * 4, "as long as the text for the limit itself");
    let e = s.execute("imageattach", &json!({ "path": "a.png", "data": one_over })).unwrap_err().to_string();
    assert!(e.contains("`data` is larger than 256 MB"), "{e}");
    assert!(images(&s).is_empty());
}

/// A relative path in an unsaved drawing has nothing to be relative to, and the error says so.
#[test]
fn a_relative_path_in_an_unsaved_drawing_says_what_to_do() {
    hooks();
    let mut s = Session::new();
    let e = s.execute("imageattach", &json!({ "path": "missing.png" })).unwrap_err().to_string();
    assert!(e.contains("save the drawing first, or give a full path"), "{e}");

    let dir = folder("unsaved");
    let full = dir.join("missing.png");
    let e = s.execute("imageattach", &json!({ "path": full.to_string_lossy() })).unwrap_err().to_string();
    assert!(e.contains("file not found") && !e.contains("save the drawing"), "{e}");
    // A full Windows path, read as relative elsewhere, is a full path all the same.
    for windows in [r"C:\nowhere\a.png", "D:/nowhere/a.png", r"\\server\maps\a.png"] {
        let e = s.execute("imageattach", &json!({ "path": windows })).unwrap_err().to_string();
        assert!(e.contains("file not found") && !e.contains("save the drawing"), "{windows}: {e}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_image_among_many_objects_keeps_its_pixels() {
    // More objects than one build task takes: the display list is built in parts and joined,
    // and the image must still point at its own pixels.
    hooks();
    let mut s = Session::new();
    for i in 0..700 {
        s.execute("line", &json!({ "points": [[i, 0], [i, 1]] })).unwrap();
    }
    s.execute("imageattach", &json!({ "path": "a.png", "data": base64_encode(&png()), "at": [0, 5], "width": 4 })).unwrap();
    for i in 0..700 {
        s.execute("line", &json!({ "points": [[i, 2], [i, 3]] })).unwrap();
    }
    s.execute("imageattach", &json!({ "path": "b.png", "data": base64_encode(&png()), "at": [10, 5], "width": 8 })).unwrap();
    let list = cadcraft_engine::render::build(s.doc().unwrap(), &cadcraft_engine::doc::Space::Model, &cadcraft_engine::render::Options::default());
    let widths: Vec<f64> = list
        .prims
        .iter()
        .filter_map(|p| match p.kind {
            cadcraft_engine::render::Kind::Image { index } => list.images.get(index as usize).map(|i| i.right.x * 4.0),
            _ => None,
        })
        .collect();
    assert_eq!(widths, [4.0, 8.0]);
}

fn drawn_images(s: &Session) -> usize {
    cadcraft_engine::render::build(s.doc().unwrap(), &cadcraft_engine::doc::Space::Model, &cadcraft_engine::render::Options::default()).images.len()
}

#[test]
fn editing_commands_work_on_images_and_undo_keeps_the_pixels() {
    hooks();
    let mut s = Session::new();
    let r = s.execute("imageattach", &json!({ "path": "a.png", "data": base64_encode(&png()), "at": [10, 0], "width": 4 })).unwrap();
    let h = r["handle"].as_str().unwrap().to_string();

    // Move, then a quarter turn about the origin: the corner and both axes follow.
    s.execute("move", &json!({ "handles": [h], "delta": [0, 5] })).unwrap();
    s.execute("rotate", &json!({ "handles": [h], "base": [0, 0], "angle": 90 })).unwrap();
    let i = images(&s).pop().unwrap();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(close(i.insert.x, -5.0) && close(i.insert.y, 10.0), "{i:?}");
    assert!(close(i.u.x, 0.0) && close(i.u.y, 1.0) && close(i.v.x, -1.0) && close(i.v.y, 0.0), "{i:?}");
    assert_eq!(drawn_images(&s), 1);

    // A copy is a second reference to the same file.
    s.execute("copy", &json!({ "handles": [h], "delta": [20, 0] })).unwrap();
    assert_eq!((images(&s).len(), drawn_images(&s)), (2, 2));
    assert_eq!(listed(&mut s)[0]["references"], 2);
    assert_eq!(listed(&mut s).as_array().map(Vec::len), Some(1));

    // Scaled about its corner: twice the size, still the same pixels.
    s.execute("scale", &json!({ "handles": [h], "base": [-5, 10], "factor": 2 })).unwrap();
    let i = images(&s).into_iter().find(|i| close(i.insert.x, -5.0)).unwrap();
    assert!(close(i.u.y, 2.0) && close(i.size.x, 4.0), "{i:?}");

    // Erased, then brought back by undo, with its picture; redo erases again.
    s.execute("erase", &json!({ "handles": [h] })).unwrap();
    assert_eq!((images(&s).len(), drawn_images(&s)), (1, 1));
    s.execute("undo", &json!({})).unwrap();
    assert_eq!((images(&s).len(), drawn_images(&s)), (2, 2));
    s.execute("redo", &json!({})).unwrap();
    assert_eq!(images(&s).len(), 1);

    // Everything undone: no image, and nothing left to draw.
    for _ in 0..10 {
        let _ = s.execute("undo", &json!({}));
    }
    assert!(images(&s).is_empty());
    assert_eq!(drawn_images(&s), 0);
    assert_eq!(listed(&mut s), json!([]));
}

#[test]
fn the_alias_and_the_command_line_reach_the_command() {
    hooks();
    assert_eq!(cadcraft_engine::cmd::resolve_alias("IAT"), "imageattach");
    assert_eq!(cadcraft_engine::cmd::resolve_alias("im"), "image");
    let spec = |id: &str| cadcraft_engine::cmd::command_specs().iter().find(|c| c.id == id).unwrap();
    assert!(!spec("imageattach").params.is_empty() && !spec("image").params.is_empty());
    // Typed without a file it has nothing to attach: an error, not a crash or a stuck prompt.
    let mut s = Session::new();
    let _ = s.cmdline("imageattach");
    let _ = s.cmdline("");
    s.cancel();
    assert!(s.running.is_none() && images(&s).is_empty());
}

#[test]
fn images_in_blocks_and_layouts_are_loaded_on_open() {
    use cadcraft_engine::doc::geom::{Vec2, Vec3};
    use cadcraft_engine::doc::{Block, Common, Entity, Space};
    hooks();
    let dir = folder("spaces");
    std::fs::create_dir_all(dir.join("maps")).unwrap();
    std::fs::write(dir.join("maps").join("a.png"), png()).unwrap();
    std::fs::write(dir.join("b.png"), png()).unwrap();
    let image = |path: &str, name: &str| {
        EntityKind::Image(Image {
            u: Vec3::new(1.0, 0.0, 0.0),
            v: Vec3::new(0.0, 1.0, 0.0),
            size: Vec2::new(4.0, 2.0),
            path: path.into(),
            name: name.into(),
            ..Image::default()
        })
    };
    let mut d = Drawing::new_metric();
    // The same file in model space, in a block and on a layout; another only on the layout,
    // saved with a folder that no longer exists (found by its name next to the drawing); and
    // one that is nowhere.
    d.add(&Space::Model, Common::default(), image("maps/a.png", "a")).unwrap();
    let mut blk = Block::new("MAP");
    let bh = d.new_handle();
    blk.entities.push(Entity::new(bh, image("maps/a.png", "a")));
    d.blocks.insert("MAP".into(), std::sync::Arc::new(blk));
    let layout = Space::Paper("Layout1".into());
    d.add(&layout, Common::default(), image("maps/a.png", "a")).unwrap();
    d.add(&layout, Common::default(), image("C:\\old\\place\\b.png", "b")).unwrap();
    d.add(&layout, Common::default(), image("gone.png", "gone")).unwrap();
    let host = dir.join("host.dxf");
    std::fs::write(&host, cadcraft_io::write(&d, "host.dxf").unwrap()).unwrap();

    let mut s = Session::new();
    open(&mut s, &host);
    let doc = s.doc().unwrap();
    assert_eq!(doc.rasters.len(), 2, "each file once");
    assert!(doc.rasters.get("maps/a.png").is_some() && doc.rasters.get("C:\\old\\place\\b.png").is_some());
    assert!(!s.state().unwrap().is_dirty());
    assert_eq!(s.log.iter().filter(|l| l.starts_with("Image ")).count(), 1, "{:?}", s.log);
    let l = listed(&mut s);
    let row = |name: &str| l.as_array().unwrap().iter().find(|r| r["name"] == name).cloned().unwrap();
    assert_eq!((row("a")["status"].clone(), row("a")["references"].clone()), (json!("Loaded"), json!(3)));
    assert_eq!(row("b")["status"], "Loaded");
    assert_eq!(row("gone")["status"], "Not Found");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drawing_opened_from_bytes_gets_its_pixels_handed_over() {
    // The web app's way: no file system, the drawing arrives as bytes and so do the images.
    use cadcraft_engine::doc::geom::{Vec2, Vec3};
    use cadcraft_engine::doc::{Common, Space};
    hooks();
    let mut d = Drawing::new_metric();
    let i = Image {
        u: Vec3::new(1.0, 0.0, 0.0),
        v: Vec3::new(0.0, 1.0, 0.0),
        size: Vec2::new(4.0, 2.0),
        path: "no-such-folder/web.png".into(),
        name: "web".into(),
        ..Image::default()
    };
    d.add(&Space::Model, Common::default(), EntityKind::Image(i)).unwrap();
    let dxf = cadcraft_io::write(&d, "plan.dxf").unwrap();

    let mut s = Session::new();
    s.execute("open", &json!({ "data": base64_encode(&dxf), "name": "plan.dxf" })).unwrap();
    assert_eq!(images(&s).len(), 1, "opened with its image reference");
    assert!(s.doc().unwrap().rasters.is_empty());
    assert!(s.log.iter().any(|l| l.contains("web.png")), "{:?}", s.log);
    assert_eq!(listed(&mut s)[0]["status"], "Not Found");
    assert_eq!(drawn_images(&s), 0);

    // The same file handed over as bytes: the reference that was there draws too.
    s.execute("imageattach", &json!({ "path": "no-such-folder/web.png", "data": base64_encode(&png()), "at": [10, 0] })).unwrap();
    assert_eq!((images(&s).len(), drawn_images(&s)), (2, 2));
    let l = listed(&mut s);
    assert_eq!((l[0]["status"].clone(), l[0]["references"].clone(), l[0]["name"].clone()), (json!("Loaded"), json!(2), json!("web")));

    // Saved and read back: the references are kept, the pixels are not in the file.
    let bytes = cadcraft_io::write(s.doc().unwrap(), "plan.dxf").unwrap();
    let back = cadcraft_io::read(&bytes, "plan.dxf").unwrap();
    assert!(back.rasters.is_empty());
    assert_eq!(back.model.iter().filter(|e| matches!(e.kind, EntityKind::Image(_))).count(), 2);
}

#[test]
fn many_files_are_loaded_together_and_reported_in_order() {
    // More files than loaders, with missing and unreadable ones among them: each good file is
    // loaded, each bad one reported once, in the order the drawing uses them.
    use cadcraft_engine::doc::geom::{Vec2, Vec3};
    use cadcraft_engine::doc::{Common, Space};
    hooks();
    let dir = folder("many");
    let mut d = Drawing::new_metric();
    let mut expected_notes = Vec::new();
    for n in 0..60u32 {
        let name = format!("tile{n:02}.png");
        match n % 10 {
            3 => expected_notes.push(format!("Image {name}: file not found")),
            7 => {
                std::fs::write(dir.join(&name), b"not an image").unwrap();
                expected_notes.push(format!("Image {name}: "));
            }
            _ => {
                // A different width per file, so a mix-up would show.
                let px = vec![200u8; (n as usize + 1) * 4];
                std::fs::write(dir.join(&name), cadcraft_io::raster::encode_png(&Raster::new(n + 1, 1, px).unwrap()).unwrap()).unwrap();
            }
        }
        let i = Image {
            insert: Vec3::new(f64::from(n) * 3.0, 0.0, 0.0),
            u: Vec3::new(1.0, 0.0, 0.0),
            v: Vec3::new(0.0, 1.0, 0.0),
            size: Vec2::new(f64::from(n + 1), 1.0),
            path: name.clone(),
            name: format!("tile{n:02}"),
            ..Image::default()
        };
        d.add(&Space::Model, Common::default(), EntityKind::Image(i)).unwrap();
    }
    let host = dir.join("host.dxf");
    std::fs::write(&host, cadcraft_io::write(&d, "host.dxf").unwrap()).unwrap();

    for _ in 0..3 {
        let mut s = Session::new();
        open(&mut s, &host);
        let doc = s.doc().unwrap();
        assert_eq!(doc.rasters.len(), 48);
        for n in 0..60u32 {
            let loaded = doc.rasters.get(&format!("tile{n:02}.png")).map(|r| r.width());
            assert_eq!(loaded, if matches!(n % 10, 3 | 7) { None } else { Some(n + 1) }, "tile {n}");
        }
        let notes: Vec<&String> = s.log.iter().filter(|l| l.starts_with("Image ")).collect();
        assert_eq!(notes.len(), expected_notes.len(), "{notes:?}");
        assert!(notes.iter().zip(&expected_notes).all(|(got, want)| got.starts_with(want.as_str())), "{notes:?}");
        assert!(!s.state().unwrap().is_dirty());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Without `width` a PDF attached from bytes is its page at its size on paper in the drawing's units: in a drawing in millimetres the page's
/// millimetres, in metres a thousandth of that; with a `width` alone the height follows the page's proportions; with both, they are used.
#[test]
fn pdf_bytes_without_a_width_are_placed_at_their_size_on_paper() {
    hooks();
    let (_, w_mm, h_mm, _) = cadcraft_io::pdf_read::page_info(&pdf(), 1).unwrap();
    let data = base64_encode(&pdf());
    for (insunits, mm_per_unit) in [(4, 1.0), (6, 1000.0), (5, 10.0), (1, 25.4)] {
        let mut s = Session::new();
        s.doc_mut().unwrap().header.set_i64("INSUNITS", insunits);
        let r = s.execute("pdfattach", &json!({"data": data, "name": "sheet.pdf"})).unwrap();
        let size = r["size"].as_array().unwrap();
        let (w, h) = (size[0].as_f64().unwrap(), size[1].as_f64().unwrap());
        assert!((w - w_mm / mm_per_unit).abs() < 1e-9 * w, "INSUNITS {insunits}: {w} for {} expected", w_mm / mm_per_unit);
        assert!((h - h_mm / mm_per_unit).abs() < 1e-9 * h, "INSUNITS {insunits}: {h}");
    }
    let mut s = Session::new();
    let only_width = s.execute("pdfattach", &json!({"data": data, "width": 400})).unwrap();
    let size = only_width["size"].as_array().unwrap();
    assert!((size[0].as_f64().unwrap() - 400.0).abs() < 1e-9 && (size[1].as_f64().unwrap() - 400.0 * h_mm / w_mm).abs() < 1e-9, "{size:?}");
    let both = s.execute("pdfattach", &json!({"data": data, "width": 400, "height": 90})).unwrap();
    assert_eq!(both["size"], json!([400.0, 90.0]));
}
