//! The title block (TITLEBLOCKADD, TITLEBLOCKEDIT): the pictures kept with the drawing, the table of revisions that grows, and bad input refused.

use cadcraft_engine::Session;
use cadcraft_engine::cmd::file::{IoHooks, base64_encode, set_image_decoder, set_io};
use cadcraft_engine::doc::{Drawing, Handle, Raster, TITLEBLOCK_IMAGE_PREFIX, titleblock_of};
use serde_json::{Value, json};

fn hooks() {
    set_io(IoHooks {
        read: |b, name| cadcraft_io::read(b, name).map_err(|e| e.to_string()),
        write: |d, name| cadcraft_io::write(d, name).map_err(|e| e.to_string()),
        plot: None,
    });
    set_image_decoder(|b| cadcraft_io::raster::decode(b).map_err(|e| e.to_string()));
}

/// A PNG of `w` x `h` pixels, base64.
fn png(w: u32, h: u32) -> String {
    let px: Vec<u8> = (0..w * h).flat_map(|i| [(i % 251) as u8, 40, 200, 255]).collect();
    base64_encode(&cadcraft_io::raster::encode_png(&Raster::new(w, h, px).unwrap()).unwrap())
}

fn add(s: &mut Session, p: Value) -> Handle {
    let r = s.execute("titleblockadd", &p).unwrap();
    Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap()
}

fn block(s: &Session, h: Handle) -> cadcraft_engine::doc::TitleBlockParams {
    titleblock_of(s.doc().unwrap().entity(h).unwrap()).expect("a title block")
}

#[test]
fn a_title_block_keeps_its_pictures_in_the_file() {
    hooks();
    let mut s = Session::new();
    let h = add(
        &mut s,
        json!({"paper": "A3", "project": "NOM DU PROJET", "title": "Plan des réseaux",
               "image": {"data": png(200, 100)},
               "parties": [{"role": "Entreprise", "name": "Société A", "afterImage": true, "logo": {"data": png(40, 40)}}]}),
    );
    let t = block(&s, h);
    assert_eq!(t.project, "NOM DU PROJET");
    assert!(t.image.as_ref().is_some_and(|p| p.key.starts_with(TITLEBLOCK_IMAGE_PREFIX)));
    assert_eq!(s.doc().unwrap().rasters.iter().count(), 2);
    // Saved and reopened, the block and its pixels are there.
    let bytes = cadcraft_io::write_dxf(s.doc().unwrap());
    let back: Drawing = cadcraft_io::read(bytes.as_bytes(), "a.dxf").unwrap();
    let again = back.model.iter().find_map(|e| titleblock_of(e)).expect("the block came back");
    assert_eq!(again.project, "NOM DU PROJET");
    for pic in again.picture_keys() {
        assert!(back.rasters.get(pic).is_some(), "{pic} came back with the file");
    }
}

#[test]
fn revisions_can_be_added_and_a_dropped_picture_is_forgotten() {
    hooks();
    let mut s = Session::new();
    let h = add(&mut s, json!({"image": {"data": png(64, 32)}, "revisionRows": 4}));
    for i in 0..6 {
        s.execute(
            "titleblockedit",
            &json!({"handle": h.hex(), "addRevision": {"index": format!("R{i}"), "date": "01/01/2026", "nature": "Diffusion"}}),
        )
        .unwrap();
    }
    let t = block(&s, h);
    assert_eq!(t.revisions.len(), 6);
    assert!(t.revision_rows >= 6, "the table grew with its lines");
    s.execute("titleblockedit", &json!({"handle": h.hex(), "image": null})).unwrap();
    assert!(block(&s, h).image.is_none());
    assert_eq!(s.doc().unwrap().rasters.iter().count(), 0, "a picture no block shows is not kept");
    // Undo brings the picture back with the block.
    s.undo().unwrap();
    assert!(block(&s, h).image.is_some());
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    hooks();
    let mut s = Session::new();
    for p in [
        json!({"paper": "B5"}),
        json!({"width": 1.0}),
        json!({"width": 1e9}),
        json!({"width": "wide"}),
        json!({"project": "x".repeat(500)}),
        json!({"parties": "none"}),
        json!({"parties": vec![json!({"name": "x"}); 99]}),
        json!({"revisions": vec![json!({"index": "A"}); 99]}),
        json!({"image": {"data": "not base64 !!"}}),
        json!({"image": 5}),
        json!({"image": {"key": "titleblock-image:404"}}),
        json!({"paper": "A4", "revisionRows": 40}),
        json!({"at": "here"}),
    ] {
        assert!(s.execute("titleblockadd", &p).is_err(), "{p} was refused");
    }
    assert!(s.doc().unwrap().model.iter().next().is_none(), "nothing was drawn");
    let h = add(&mut s, json!({}));
    for p in [json!({"handle": h.hex(), "paper": "Z1"}), json!({"handle": "FFFF"}), json!({"handle": h.hex(), "addRevision": 3, "revisionRows": -1})]
    {
        assert!(s.execute("titleblockedit", &p).is_err(), "{p} was refused");
    }
    assert_eq!(block(&s, h).paper, "A3");
}

#[test]
fn every_paper_gets_a_column_that_fits_its_sheet() {
    hooks();
    for paper in ["A4", "A3", "A2", "A1", "A0"] {
        let mut s = Session::new();
        let h = add(&mut s, json!({"paper": paper, "landscape": false}));
        let t = block(&s, h);
        assert!(t.width_mm < t.sheet_mm().unwrap().0 && t.height_mm() > 100.0, "{paper}");
    }
}

#[test]
fn on_a_sheet_the_block_is_drawn_in_the_sheets_paper_units_not_the_drawings() {
    hooks();
    // A drawing in metres whose sheet is in inches: the column is still 130 mm wide on paper.
    let mut s = Session::new();
    s.doc_mut().unwrap().header.set_i64("INSUNITS", 6);
    s.doc_mut().unwrap().header.set_i64("MEASUREMENT", 0);
    s.execute("layout.set", &json!({"name": "Layout1"})).unwrap();
    let r = s.execute("titleblockadd", &json!({"paper": "A3", "atMm": [10.0, 10.0]})).unwrap();
    let h = Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap();
    let t = titleblock_of(s.doc().unwrap().layouts.iter().flat_map(|l| l.entities.iter()).find(|e| e.handle == h).unwrap()).unwrap();
    assert_eq!(t.units.mm_per_unit, 25.4);
    let at = s.doc().unwrap().layouts.iter().flat_map(|l| l.entities.iter()).find(|e| e.handle == h).map(|e| match &e.kind {
        cadcraft_engine::doc::EntityKind::Insert(i) => i.insert.x,
        _ => 0.0,
    });
    assert!((at.unwrap() * 25.4 - 10.0).abs() < 1e-6, "atMm is millimetres on paper");
}
