use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, EntityKind, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn texts(d: &Drawing, h: crate::Handle) -> Vec<String> {
    match &d.entity(h).unwrap().kind {
        EntityKind::Insert(i) => d
            .block(&i.block)
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect(),
        k => panic!("{k:?}"),
    }
}

fn control(d: &mut Drawing, kind: ControlKind, name: &str, controller: &str, bus: &str, address: &str, at: (f64, f64)) {
    let mut c = ControlParams::new(kind, name, units());
    (c.controller, c.bus, c.address) = (controller.into(), bus.into(), address.into());
    d.add_control(&Space::Model, Common::default(), &c, Vec2::new(at.0, at.1), 0.0).unwrap();
}

#[test]
fn the_principle_diagram_has_the_buses_the_controllers_on_them_and_the_points_under_each() {
    let mut d = Drawing::new_metric();
    control(&mut d, ControlKind::Controller, "AUT1", "", "BUS1", "", (0.0, 0.0));
    control(&mut d, ControlKind::Controller, "AUT2", "", "BUS1", "", (1000.0, 0.0));
    control(&mut d, ControlKind::Controller, "AUT3", "", "", "", (2000.0, 0.0));
    control(&mut d, ControlKind::RoomTemperature, "TA1", "AUT1", "", "3", (0.0, 1000.0));
    control(&mut d, ControlKind::DamperActuator, "VM1", "AUT1", "", "", (500.0, 1000.0));
    control(&mut d, ControlKind::RoomTemperature, "TA2", "AUT2", "", "7", (1000.0, 1000.0));
    let p = ControlSchemaParams::new(units());
    let h = d.add_control_schema(&Space::Model, Common::default(), &p, Vec2::new(0.0, -20_000.0)).unwrap();
    let t = texts(&d, h);
    assert!(t.iter().any(|x| x.contains("Controls principle diagram")), "{t:?}");
    assert!(t.contains(&"BUS1".to_string()), "{t:?}");
    assert!(
        t.iter().any(|x| x.starts_with("AUT1 (")) && t.iter().any(|x| x.starts_with("AUT2 (")) && t.iter().any(|x| x.starts_with("AUT3 (")),
        "{t:?}"
    );
    assert!(t.iter().any(|x| x.starts_with("@3  TA1  AI")), "the address, the device and the type of the point: {t:?}");
    assert!(t.iter().any(|x| x.starts_with("@7  TA2")) && t.iter().any(|x| x.starts_with("VM1")), "{t:?}");
    // One bus only: the controllers on the others are left out.
    let only = ControlSchemaParams { bus: "bus1".into(), ..p.clone() };
    let h2 = d.add_control_schema(&Space::Model, Common::default(), &only, Vec2::new(0.0, -40_000.0)).unwrap();
    let t2 = texts(&d, h2);
    assert!(t2.iter().any(|x| x.starts_with("AUT1 (")) && !t2.iter().any(|x| x.starts_with("AUT3 (")), "{t2:?}");
    // The diagram follows the controls: a point more is drawn when the diagram is redrawn, and a drawing with no controls has an empty one.
    control(&mut d, ControlKind::RoomTemperature, "TA9", "AUT2", "", "11", (1000.0, 2000.0));
    d.refresh_control_schemas();
    assert!(texts(&d, h).iter().any(|x| x.starts_with("@11  TA9")));
    assert!(Drawing::new_metric().generate_control_schema(&p).is_ok());
    assert!(
        ControlSchemaParams { label_height_mm: 0.0, ..p.clone() }.validate().is_err()
            && ControlSchemaParams { bus: "x".repeat(100), ..p.clone() }.validate().is_err()
    );
    assert_eq!(control_schema_of(d.entity(h).unwrap()).map(|x| x.title), Some("Controls principle diagram".to_string()));
}
