//! The IFC4 file of a drawing: a well-formed STEP file with the building objects in it.

use cadcraft_doc::Drawing;
use cadcraft_engine::Session;
use serde_json::json;

/// Checks the STEP syntax of what we write: numbered lines, balanced parentheses outside strings, every reference to an entity that exists.
fn check_step(text: &str) -> Vec<(usize, String)> {
    assert!(text.starts_with("ISO-10303-21;\nHEADER;\n") && text.trim_end().ends_with("END-ISO-10303-21;"));
    assert!(text.contains("FILE_SCHEMA(('IFC4'));"));
    let data = text.split("DATA;\n").nth(1).expect("a data section").split("ENDSEC;").next().unwrap();
    let mut entities = Vec::new();
    for (k, line) in data.lines().enumerate() {
        let (num, rest) = line.strip_prefix('#').and_then(|l| l.split_once('=')).unwrap_or_else(|| panic!("line {k}: {line}"));
        let n: usize = num.parse().expect("a number");
        assert_eq!(n, k + 1, "entities are numbered in order");
        assert!(rest.ends_with(");"), "{line}");
        // Parentheses balanced outside the strings, and the references in what is before this entity.
        let (mut depth, mut in_text, mut chars) = (0i32, false, rest.chars().peekable());
        let mut refs = Vec::new();
        while let Some(c) = chars.next() {
            match c {
                '\'' if in_text && chars.peek() == Some(&'\'') => {
                    chars.next();
                }
                '\'' => in_text = !in_text,
                '(' if !in_text => depth += 1,
                ')' if !in_text => depth -= 1,
                '#' if !in_text => {
                    let digits: String = std::iter::from_fn(|| chars.next_if(|d| d.is_ascii_digit())).collect();
                    refs.push(digits.parse::<usize>().expect("a reference"));
                }
                _ => {}
            }
            assert!(depth >= 0, "{line}");
        }
        assert_eq!((depth, in_text), (0, false), "{line}");
        assert!(refs.iter().all(|r| *r >= 1 && *r <= entities.len()), "a reference to a later or missing entity: {line}");
        let name = rest.split('(').next().unwrap().to_string();
        entities.push((n, name));
    }
    entities
}

#[test]
fn a_drawing_is_written_as_a_well_formed_ifc4_file_with_its_building_objects() {
    let mut s = Session::new();
    *s.doc_mut().unwrap() = Drawing::new_metric();
    s.execute("walladd", &json!({"points": [[0, 0], [6000, 0], [6000, 4000], [0, 4000], [0, 0]], "style": "Partition 72/48"})).unwrap();
    s.execute("slabadd", &json!({"points": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]], "thickness": 200, "elevation": 0.0})).unwrap();
    s.execute("columnadd", &json!({"at": [3000, 2000], "section": "HEA 200", "height": 3000})).unwrap();
    s.execute("spaceadd", &json!({"points": [[0, 0], [6000, 0], [6000, 4000], [0, 4000]], "name": "Bureau é 'A'", "flow": 100})).unwrap();
    s.execute("ductadd", &json!({"points": [[500, 1000], [5000, 1000]], "diameter": 200, "altitude": 2.5})).unwrap();
    s.execute("ductadd", &json!({"points": [[500, 2000], [5000, 2000]], "width": 400, "height": 250})).unwrap();
    s.execute("pipeadd", &json!({"points": [[500, 3000], [5000, 3000]], "service": "chilledWater", "dn": 50})).unwrap();
    s.execute("equipmentadd", &json!({"at": [3000, 3500], "model": "Wall rack 9 U"})).unwrap();
    s.execute("equipmentadd", &json!({"at": [4500, 3500], "model": "Wall unit 2.5 kW"})).unwrap();
    s.execute("elecdeviceadd", &json!({"at": [1000, 100], "kind": "socket"})).unwrap();
    s.execute("furnitureadd", &json!({"at": [2000, 3000]})).unwrap();
    s.execute("openingadd", &json!({"type": "door", "at": [3000, 0]})).unwrap();
    s.execute("openingadd", &json!({"type": "window", "at": [6000, 2000]})).unwrap();
    s.execute("beamadd", &json!({"points": [[0, 2000], [6000, 2000]], "section": "IPE 240", "elevation": 2.7})).unwrap();
    s.execute("airterminaladd", &json!({"at": [2500, 1000], "flow": 100})).unwrap();
    s.execute("ahuadd", &json!({"at": [-3000, 0]})).unwrap();
    let bytes = cadcraft_io::write(s.doc().unwrap(), "plan.ifc").unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let entities = check_step(&text);
    let count = |class: &str| entities.iter().filter(|(_, n)| n == class).count();
    // The wall (one object, four straight pieces) is one IFCWALL with four solids; a slab, a column, the rest, one each.
    assert_eq!(count("IFCWALL"), 1);
    assert!(count("IFCEXTRUDEDAREASOLID") >= 4 + 10, "{}", count("IFCEXTRUDEDAREASOLID"));
    for (class, at_least) in [
        ("IFCSLAB", 1),
        ("IFCCOLUMN", 1),
        ("IFCSPACE", 1),
        ("IFCDUCTSEGMENT", 2),
        ("IFCPIPESEGMENT", 1),
        ("IFCUNITARYEQUIPMENT", 1),
        ("IFCBUILDINGELEMENTPROXY", 1),
        ("IFCOUTLET", 1),
        ("IFCFURNITURE", 1),
    ] {
        assert!(count(class) >= at_least, "{class}: {}", count(class));
    }
    for (class, at_least) in [("IFCDOOR", 1), ("IFCWINDOW", 1), ("IFCBEAM", 1), ("IFCAIRTERMINAL", 1)] {
        assert!(count(class) >= at_least, "{class}: {}", count(class));
    }
    // The number of attributes of each entity is the schema's.
    let arity = |line: &str| {
        let body = line.split_once('=').unwrap().1;
        let inner = &body[body.find('(').unwrap() + 1..body.len() - 2];
        let (mut depth, mut in_text, mut n, mut chars) = (0i32, false, 1usize, inner.chars().peekable());
        while let Some(c) = chars.next() {
            match c {
                '\'' if in_text && chars.peek() == Some(&'\'') => {
                    chars.next();
                }
                '\'' => in_text = !in_text,
                '(' if !in_text => depth += 1,
                ')' if !in_text => depth -= 1,
                ',' if !in_text && depth == 0 => n += 1,
                _ => {}
            }
        }
        n
    };
    let schema = [
        ("IFCPROJECT", 9),
        ("IFCSITE", 14),
        ("IFCBUILDING", 12),
        ("IFCBUILDINGSTOREY", 10),
        ("IFCSPACE", 11),
        ("IFCWALL", 9),
        ("IFCSLAB", 9),
        ("IFCCOLUMN", 9),
        ("IFCBEAM", 9),
        ("IFCDOOR", 13),
        ("IFCWINDOW", 13),
        ("IFCDUCTSEGMENT", 9),
        ("IFCPIPESEGMENT", 9),
        ("IFCAIRTERMINAL", 9),
        ("IFCUNITARYEQUIPMENT", 9),
        ("IFCBUILDINGELEMENTPROXY", 9),
        ("IFCFURNITURE", 9),
        ("IFCOUTLET", 9),
        ("IFCEXTRUDEDAREASOLID", 4),
        ("IFCLOCALPLACEMENT", 2),
        ("IFCAXIS2PLACEMENT3D", 3),
        ("IFCRELAGGREGATES", 6),
        ("IFCRELCONTAINEDINSPATIALSTRUCTURE", 6),
        ("IFCRELDEFINESBYPROPERTIES", 6),
        ("IFCPROPERTYSET", 5),
    ];
    for line in text.lines().filter(|l| l.starts_with('#')) {
        let name = line.split_once('=').unwrap().1.split('(').next().unwrap();
        if let Some((_, n)) = schema.iter().find(|(c, _)| *c == name) {
            assert_eq!(arity(line), *n, "{line}");
        }
    }
    assert_eq!((count("IFCPROJECT"), count("IFCSITE"), count("IFCBUILDING"), count("IFCBUILDINGSTOREY")), (1, 1, 1, 1));
    // Every element is in the storey, the space's name is escaped, and the same drawing gives the same file.
    assert!(text.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE"));
    assert!(text.contains("'Bureau \\X2\\00E9\\X0\\ ''A'''"), "the name of the space escaped");
    assert_eq!(text, String::from_utf8(cadcraft_io::write(s.doc().unwrap(), "plan.ifc").unwrap()).unwrap());
    // An empty drawing is still a file; the format is write only.
    let empty = cadcraft_io::write(&Drawing::new_metric(), "vide.ifc").unwrap();
    check_step(&String::from_utf8(empty).unwrap());
    assert!(cadcraft_io::read(text.as_bytes(), "plan.ifc").is_err());
}
