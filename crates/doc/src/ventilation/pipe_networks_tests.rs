use cadcraft_color::{Color, Rgb};
use cadcraft_geom::Vec2;

use super::*;
use crate::{Common, Drawing, Space};

fn units() -> HvacUnits {
    HvacUnits { mm_per_unit: 1.0 }
}

fn pipe(d: &mut Drawing, service: PipeService, leg: PipeLeg, network: &str, y: f64) -> crate::Handle {
    let p = PipeParams { leg, network: network.into(), ..PipeParams::new(service, 50.0, units()) };
    d.add_pipe(&Space::Model, Common::default(), p, Vec2::new(0.0, y), Vec2::new(4000.0, y)).unwrap()
}

#[test]
fn a_layer_name_is_made_of_the_network_name_without_accents_or_spaces() {
    assert_eq!(network_suffix("CTA1"), "CTA1");
    assert_eq!(network_suffix(" Réseau toiture 3 "), "RESEAU_TOITURE_3");
    assert_eq!(network_suffix("a--b  c"), "A_B_C");
    assert_eq!(network_suffix("  "), "");
    assert_eq!(network_suffix("é/*+"), "E");
    assert!(network_suffix(&"X".repeat(100)).chars().count() <= 24);
    assert_eq!(network_layer(PIPE_CHW_SUPPLY_LAYER, "CTA 1"), "M-PIPE-CHWS-CTA_1");
    assert_eq!(network_layer(PIPE_CHW_SUPPLY_LAYER, ""), PIPE_CHW_SUPPLY_LAYER);
    assert_eq!(
        network_layer(PIPE_CHW_SUPPLY_LAYER, "CTA 1").as_ptr(),
        network_layer(PIPE_CHW_SUPPLY_LAYER, "cta 1").as_ptr(),
        "one string for one layer"
    );
}

#[test]
fn a_pipe_of_a_network_is_drawn_on_a_layer_of_its_own_with_the_colour_and_line_type_of_its_style() {
    let mut d = Drawing::new_metric();
    // Two chilled-water networks: the second is the drawing's second style.
    d.set_network_style("A", None, None).unwrap();
    d.set_network_style("B", None, None).unwrap();
    let a = pipe(&mut d, PipeService::ChilledWater, PipeLeg::Supply, "A", 0.0);
    let b = pipe(&mut d, PipeService::ChilledWater, PipeLeg::Supply, "B", 1000.0);
    let br = pipe(&mut d, PipeService::ChilledWater, PipeLeg::Return, "B", 1500.0);
    let plain = pipe(&mut d, PipeService::ChilledWater, PipeLeg::Supply, "", 2000.0);
    assert_eq!(d.entity(a).unwrap().common.layer, "M-PIPE-CHWS-A");
    assert_eq!(d.entity(b).unwrap().common.layer, "M-PIPE-CHWS-B");
    assert_eq!(d.entity(plain).unwrap().common.layer, PIPE_CHW_SUPPLY_LAYER, "no network: the service's layer, as before");
    let la = d.layer("M-PIPE-CHWS-A").unwrap().clone();
    let lb = d.layer("M-PIPE-CHWS-B").unwrap().clone();
    assert_eq!(la.color, Color::True(Rgb(0x00, 0x72, 0xb2)));
    assert_eq!(lb.color, Color::True(Rgb(0xe6, 0x9f, 0x00)));
    assert_eq!((la.linetype.as_str(), lb.linetype.as_str()), ("CONTINUOUS", "CONTINUOUS"), "the colour tells them apart; a line type is chosen");
    d.set_network_style("B", None, Some("NET_DASH")).unwrap();
    assert_eq!(d.layer("M-PIPE-CHWS-B").unwrap().linetype, "NET_DASH");
    assert!(d.linetype("NET_DASH").is_some(), "the line type is made in the drawing");
    // The return of a network is the same hue, lighter.
    let lr = d.layer("M-PIPE-CHWR-B").unwrap();
    assert_eq!(d.entity(br).unwrap().common.layer, "M-PIPE-CHWR-B");
    assert_eq!(lr.color, Color::True(Rgb(0xf3, 0xcf, 0x80)));
    // Everything the block draws is on the network's layers: the two sides, and the text carries the name.
    let EntityKindInsert::Insert(i) = d.entity(b).unwrap().kind.clone() else { panic!("a block reference") };
    let block = d.block(&i.block).unwrap();
    assert!(
        block.entities.iter().all(|e| e.common.layer.starts_with("M-PIPE-CHWS-B")),
        "{:?}",
        block.entities.iter().map(|e| e.common.layer.clone()).collect::<Vec<_>>()
    );
    let text = block.entities.iter().find_map(|e| if let crate::EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None }).unwrap();
    assert!(text.starts_with("B ") && text.contains("DN50 CHW-S"), "{text}");
}

use crate::EntityKind as EntityKindInsert;

#[test]
fn a_style_can_be_changed_and_the_layers_of_the_network_follow() {
    let mut d = Drawing::new_metric();
    let h = pipe(&mut d, PipeService::HotWater, PipeLeg::Supply, "CHAUD", 0.0);
    assert!(d.network_style("chaud").is_none(), "no style until one is set");
    assert_eq!(
        d.layer("M-PIPE-HWS-CHAUD").unwrap().color,
        Color::True(Rgb(0xe0, 0x24, 0x24)),
        "a layer of a network with no style keeps the service's colour"
    );
    let s = d.set_network_style("chaud", Some([10, 20, 30]), Some("net_dot")).unwrap();
    assert_eq!((s.name.as_str(), s.color, s.linetype.as_str()), ("chaud", [10, 20, 30], "NET_DOT"));
    let l = d.layer("M-PIPE-HWS-CHAUD").unwrap();
    assert_eq!((l.color, l.linetype.as_str()), (Color::True(Rgb(10, 20, 30)), "NET_DOT"));
    // A second call changes one thing and keeps the other; the name keeps the case of the first.
    let s = d.set_network_style("CHAUD", None, Some("DASHED")).unwrap();
    assert_eq!((s.name.as_str(), s.color, s.linetype.as_str()), ("chaud", [10, 20, 30], "DASHED"));
    assert_eq!(d.network_styles.len(), 1);
    // Typed on the layer by hand, the colour stays until the style is set again.
    d.layer_mut("M-PIPE-HWS-CHAUD").unwrap().color = Color::Index(5);
    let _ = pipe(&mut d, PipeService::HotWater, PipeLeg::Supply, "CHAUD", 500.0);
    assert_eq!(d.layer("M-PIPE-HWS-CHAUD").unwrap().color, Color::Index(5));
    d.set_network_style("chaud", Some([1, 2, 3]), None).unwrap();
    assert_eq!(d.layer("M-PIPE-HWS-CHAUD").unwrap().color, Color::True(Rgb(1, 2, 3)));
    let _ = h;
    // What cannot be a style is refused.
    for (name, lt) in [("", None), ("/*+", None), ("ok", Some("NOT_A_LINETYPE")), (&"x".repeat(40)[..], None)] {
        assert!(d.set_network_style(name, None, lt).is_err(), "{name:?} {lt:?}");
    }
    // A part is drawn on its network's layer, but a safety device stays red.
    let valve = PartParams { network: "CHAUD".into(), ..PartParams::new(PartKind::IsolationValve, "V1", units()) };
    let safety = PartParams { network: "CHAUD".into(), ..PartParams::new(PartKind::SafetyValve, "S1", units()) };
    assert_eq!((valve.layer(), safety.layer()), ("M-PIPE-ACC-CHAUD", PART_SAFETY_LAYER));
    // And a riser line.
    let mut line = RiserLine::new(PipeService::ChilledWater, PipeLeg::Supply, 65.0);
    line.network = "CHAUD".into();
    assert_eq!(line.pipe(units()).layer(), "M-PIPE-CHWS-CHAUD");
    assert!(line.text(units()).starts_with("CHAUD CHW-S"));
}

#[test]
fn the_default_styles_use_a_new_hue_for_each_network_and_a_continuous_line() {
    let mut seen = std::collections::HashSet::new();
    for i in 0..12 {
        let s = NetworkStyle::nth("X", i);
        assert!(seen.insert(s.color), "hue {i} is a new one");
    }
    assert!((0..12).all(|i| NetworkStyle::nth("X", i).linetype == "CONTINUOUS"), "continuous unless one is chosen");
    // A pipe keeps its network through its extended data.
    let mut d = Drawing::new_metric();
    let h = pipe(&mut d, PipeService::Condensate, PipeLeg::Supply, "COND 2", 0.0);
    assert_eq!(pipe_of(d.entity(h).unwrap()).unwrap().network, "COND 2");
    assert!(PipeParams { network: "x".repeat(40), ..PipeParams::new(PipeService::HotWater, 50.0, units()) }.validate().is_err());
}

fn equipment(d: &mut Drawing, kind: EquipmentKind, at: (f64, f64)) {
    let m = EquipmentModel {
        name: "x".into(),
        kind,
        reference: String::new(),
        capacity_kw: Some(50.0),
        heating_kw: None,
        length_mm: 1000.0,
        width_mm: 600.0,
        height_mm: 500.0,
        weight_kg: None,
        fans: 0,
        connections: String::new(),
    };
    d.add_equipment(&Space::Model, Common::default(), &EquipmentParams::from_model(&m, "E", units()), Vec2::new(at.0, at.1), 0.0).unwrap();
}

/// A loop: a chiller, a unit, a supply and a return between them (both ending inside the two outlines).
fn chilled_loop(d: &mut Drawing, y: f64) -> (crate::Handle, crate::Handle) {
    equipment(d, EquipmentKind::Chiller, (0.0, y));
    equipment(d, EquipmentKind::Cassette, (6000.0, y));
    let s = pipe(d, PipeService::ChilledWater, PipeLeg::Supply, "", y);
    let r = d
        .add_pipe(
            &Space::Model,
            Common::default(),
            PipeParams { leg: PipeLeg::Return, ..PipeParams::new(PipeService::ChilledWater, 50.0, units()) },
            Vec2::new(5800.0, y + 200.0),
            Vec2::new(200.0, y + 200.0),
        )
        .unwrap();
    (s, r)
}

fn network_of(d: &Drawing, h: crate::Handle) -> String {
    pipe_of(d.entity(h).unwrap()).map(|p| p.network).or_else(|| part_of(d.entity(h).unwrap()).map(|p| p.network)).unwrap()
}

#[test]
fn the_networks_are_read_from_the_drawing_and_named_one_by_one_with_a_hue_each() {
    let mut d = Drawing::new_metric();
    let (s1, r1) = chilled_loop(&mut d, 0.0);
    let (s2, r2) = chilled_loop(&mut d, 5000.0);
    // A valve in the line of the first supply, and an air vent hanging on the second.
    let valve = d.part_entry_named("Isolation valve (ball)").unwrap().params("V1", units());
    let vh = d.add_part(&Space::Model, Common::default(), &valve, Vec2::new(2000.0, 0.0), 0.0).unwrap();
    d.split_pipe(s1, 2000.0, 150.0).unwrap();
    let vent = d.part_entry_named("Automatic air vent").unwrap().params("A1", units());
    let ah = d.add_part(&Space::Model, Common::default(), &vent, Vec2::new(3000.0, 5000.0), 0.0).unwrap();
    let comps = d.pipe_components().unwrap();
    assert_eq!(comps.len(), 2, "two loops");
    assert!(comps.iter().all(|c| c.pipes.len() >= 2));
    let nets = d.name_pipe_networks().unwrap();
    assert_eq!(nets.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["CHW1", "CHW2"]);
    // The supply, the return and what is on them are one network, the two loops two.
    assert_eq!((network_of(&d, s1), network_of(&d, r1), network_of(&d, vh)), ("CHW1".into(), "CHW1".into(), "CHW1".into()));
    assert_eq!((network_of(&d, s2), network_of(&d, r2), network_of(&d, ah)), ("CHW2".into(), "CHW2".into(), "CHW2".into()));
    assert!(
        nets[0].services.contains(&(PipeService::ChilledWater, PipeLeg::Supply))
            && nets[0].services.contains(&(PipeService::ChilledWater, PipeLeg::Return))
    );
    // A hue of its own, on the layers the pipes are drawn on.
    assert_ne!(nets[0].style.as_ref().unwrap().color, nets[1].style.as_ref().unwrap().color);
    assert_eq!(d.entity(s1).unwrap().common.layer, "M-PIPE-CHWS-CHW1");
    assert_eq!(d.entity(r2).unwrap().common.layer, "M-PIPE-CHWR-CHW2");
    assert_eq!(d.entity(vh).unwrap().common.layer, "M-PIPE-ACC-CHW1");
    // Naming again changes nothing.
    assert_eq!(d.name_pipe_networks().unwrap(), nets);
    // A pipe drawn on the first network takes its name by itself.
    let extra = d
        .add_pipe(
            &Space::Model,
            Common::default(),
            PipeParams::new(PipeService::ChilledWater, 25.0, units()),
            Vec2::new(4000.0, 0.0),
            Vec2::new(4000.0, -2000.0),
        )
        .unwrap();
    assert_eq!(network_of(&d, extra), "");
    let (changed, warnings) = d.propagate_pipe_networks().unwrap();
    assert!(changed >= 1 && warnings.is_empty(), "{changed} {warnings:?}");
    assert_eq!(network_of(&d, extra), "CHW1");
}

#[test]
fn a_name_typed_on_one_pipe_goes_to_the_whole_network_and_a_network_can_be_renamed() {
    let mut d = Drawing::new_metric();
    let (s1, r1) = chilled_loop(&mut d, 0.0);
    let (s2, _) = chilled_loop(&mut d, 5000.0);
    let n = d.set_pipe_network_name(&[s1], "Réseau toiture").unwrap();
    assert!(n >= 2);
    assert_eq!((network_of(&d, s1), network_of(&d, r1), network_of(&d, s2)), ("Réseau toiture".into(), "Réseau toiture".into(), String::new()));
    assert!(d.network_style("réseau toiture").is_some());
    assert_eq!(d.entity(s1).unwrap().common.layer, "M-PIPE-CHWS-RESEAU_TOITURE");
    // The other one named after it: a name not used.
    d.name_pipe_networks().unwrap();
    assert_eq!(network_of(&d, s2), "CHW1");
    // Renamed: the pipes, the style and the layers follow.
    let colour = d.network_style("CHW1").unwrap().color;
    assert_eq!(d.rename_pipe_network("chw1", "Étage 2").unwrap(), 2);
    assert_eq!(network_of(&d, s2), "Étage 2");
    assert!(d.network_style("CHW1").is_none());
    assert_eq!(d.network_style("étage 2").unwrap().color, colour, "the style goes with the name");
    assert_eq!(d.entity(s2).unwrap().common.layer, "M-PIPE-CHWS-ETAGE_2");
    // Two named networks joined are told, and keep their names.
    d.add_pipe(
        &Space::Model,
        Common::default(),
        PipeParams::new(PipeService::ChilledWater, 25.0, units()),
        Vec2::new(4000.0, 0.0),
        Vec2::new(4000.0, 5000.0),
    )
    .unwrap();
    let (_, warnings) = d.propagate_pipe_networks().unwrap();
    assert!(warnings.iter().any(|w| w.contains("Réseau toiture") && w.contains("Étage 2")), "{warnings:?}");
    assert_eq!(network_of(&d, s1), "Réseau toiture");
    // What cannot be a name.
    assert!(d.set_pipe_network_name(&[s1], "").is_err());
    assert!(d.rename_pipe_network("nothing", "/*").is_err());
}

#[test]
fn the_pipes_of_different_services_are_different_networks_and_a_riser_line_takes_the_name_of_the_network_in_its_shaft() {
    let mut d = Drawing::new_metric();
    let (s1, _) = chilled_loop(&mut d, 0.0);
    // A hot-water run that crosses the chilled one at its end: another network.
    let hw = d
        .add_pipe(
            &Space::Model,
            Common::default(),
            PipeParams::new(PipeService::HotWater, 40.0, units()),
            Vec2::new(0.0, 100.0),
            Vec2::new(4000.0, 100.0),
        )
        .unwrap();
    let outline = vec![Vec2::new(8000.0, -300.0), Vec2::new(8600.0, -300.0), Vec2::new(8600.0, 300.0), Vec2::new(8000.0, 300.0)];
    let line = RiserLine::new(PipeService::ChilledWater, PipeLeg::Supply, 80.0);
    let riser =
        PipeRiserParams { name: "CT1".into(), up: true, outline, lines: vec![line], height_mm: Some(3200.0), label_height_mm: 100.0, units: units() };
    let rh = d.add_pipe_riser(&Space::Model, Common::default(), riser).unwrap();
    // The chilled supply ends in the shaft.
    d.add_pipe(
        &Space::Model,
        Common::default(),
        PipeParams::new(PipeService::ChilledWater, 50.0, units()),
        Vec2::new(6000.0, 0.0),
        Vec2::new(8300.0, 0.0),
    )
    .unwrap();
    let nets = d.name_pipe_networks().unwrap();
    assert_eq!(nets.len(), 2, "{nets:?}");
    assert_ne!(network_of(&d, s1), network_of(&d, hw));
    assert_eq!(
        pipe_riser_of(d.entity(rh).unwrap()).unwrap().lines[0].network,
        network_of(&d, s1),
        "the line is on the network that ends in its shaft"
    );
    assert!(pipe_riser_of(d.entity(rh).unwrap()).unwrap().lines[0].text(units()).starts_with(&network_of(&d, s1)));
}

#[test]
fn the_legend_lists_the_networks_in_their_own_layers_and_follows_the_names() {
    let mut d = Drawing::new_metric();
    chilled_loop(&mut d, 0.0);
    chilled_loop(&mut d, 5000.0);
    let lg = d.add_network_legend(&Space::Model, Common::default(), &NetworkLegendParams::new(units()), Vec2::new(-3000.0, 8000.0)).unwrap();
    let rows = |d: &Drawing| -> Vec<String> {
        let EntityKindInsert::Insert(i) = d.entity(lg).unwrap().kind.clone() else { panic!("a block reference") };
        d.block(&i.block)
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| if let crate::EntityKind::Text(t) = &e.kind { Some(t.value.clone()) } else { None })
            .collect()
    };
    assert_eq!(rows(&d), ["Networks", "-"], "nothing is named yet");
    d.name_pipe_networks().unwrap();
    let r = rows(&d);
    assert_eq!(r.len(), 3);
    assert!(r[1].starts_with("CHW1  CHW-S/CHW-R  ") && r[1].ends_with(" m"), "{}", r[1]);
    assert!(r[2].starts_with("CHW2  "));
    // Each row is on its network's layer: its colour and line type.
    let EntityKindInsert::Insert(i) = d.entity(lg).unwrap().kind.clone() else { panic!() };
    let layers: Vec<String> = d.block(&i.block).unwrap().entities.iter().map(|e| e.common.layer.clone()).collect();
    assert!(layers.contains(&"M-PIPE-CHWS-CHW1".to_string()) && layers.contains(&"M-PIPE-CHWS-CHW2".to_string()), "{layers:?}");
    // Renamed and restyled: the legend is redrawn by itself.
    d.rename_pipe_network("CHW2", "Toiture").unwrap();
    assert!(rows(&d)[2].starts_with("Toiture  "), "{:?}", rows(&d));
    d.set_network_style("Toiture", Some([9, 9, 9]), Some("NET_DOT")).unwrap();
    assert_eq!(d.layer("M-PIPE-CHWS-TOITURE").unwrap().linetype, "NET_DOT");
    assert!(NetworkLegendParams { label_height_mm: 0.0, ..NetworkLegendParams::new(units()) }.validate().is_err());
}

#[test]
fn the_takeoff_counts_the_pipes_by_network() {
    let mut d = Drawing::new_metric();
    chilled_loop(&mut d, 0.0);
    chilled_loop(&mut d, 5000.0);
    d.name_pipe_networks().unwrap();
    let t = d.takeoff("").unwrap();
    let nets: Vec<&str> = t.pipes.iter().map(|q| q.network.as_str()).collect();
    assert!(nets.contains(&"CHW1") && nets.contains(&"CHW2") && !nets.contains(&""), "{nets:?}");
    assert_eq!(t.pipes[0].network, "CHW1", "sorted by network");
    let csv = t.to_csv(true);
    assert!(csv.contains("Network;Service") || csv.contains("Réseau;Service"), "{csv}");
}
