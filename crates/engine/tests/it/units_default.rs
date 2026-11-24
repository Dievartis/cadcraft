//! The unit a user chose to be used for every drawing they open or make.

use cadcraft_doc::Drawing;
use cadcraft_engine::Session;

#[test]
fn the_insertion_unit_chosen_once_is_given_to_every_drawing_opened_or_made() {
    let mut s = Session::new();
    s.default_insunits = Some(5);
    let i = s.new_drawing(true);
    assert_eq!(s.docs[i].doc.header.i64("INSUNITS", 0), 5);
    let mut d = Drawing::new_imperial();
    d.header.set_i64("INSUNITS", 1);
    let j = s.open_drawing(d, "plan", None);
    assert_eq!(s.docs[j].doc.header.i64("INSUNITS", 0), 5, "the file said inches");
    // None chosen: each keeps its own.
    s.default_insunits = None;
    let mut d = Drawing::new_metric();
    d.header.set_i64("INSUNITS", 4);
    let k = s.open_drawing(d, "autre", None);
    assert_eq!(s.docs[k].doc.header.i64("INSUNITS", 0), 4);
}
