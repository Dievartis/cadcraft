//! The Electricity tab: the devices and the cables to draw, the circuits to name, check and size, the lighting to check against the code, and the
//! quantities. Every row starts a command (the same the command line, the menus and the control channel reach); the checks run at once and say what
//! they found on the command line.

use cadcraft_doc::{CableUsage, DeviceKind};
use egui::{Rect, Stroke, pos2, vec2};

use super::building::{header, open_id, row_tip, sections_id, separator};
use super::hvachelp;
use crate::CadApp;
use crate::theme::Tokens;

/// A small picture of `kind` in `r`, as it is drawn in plan.
fn paint_device(p: &egui::Painter, r: Rect, kind: DeviceKind) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    let c = r.center();
    let h = r.height() / 2.0;
    let line = |a: egui::Pos2, b: egui::Pos2| {
        p.line_segment([a, b], s);
    };
    match kind {
        DeviceKind::Socket | DeviceKind::SocketSpecial => {
            p.circle_stroke(c, h * 0.7, s);
            line(c + vec2(0.0, -h * 0.7), c + vec2(0.0, -h * 1.2));
            line(c + vec2(-h * 0.7, 0.0), c + vec2(h * 0.7, 0.0));
            if kind == DeviceKind::SocketSpecial {
                p.circle_stroke(c, h * 0.95, s);
            }
        }
        DeviceKind::Switch | DeviceKind::SwitchDouble | DeviceKind::SwitchTwoWay | DeviceKind::PushButton | DeviceKind::Dimmer => {
            p.circle_stroke(c + vec2(-3.0, 3.0), h * 0.4, s);
            line(c + vec2(-1.0, 1.0), c + vec2(h, -h));
            match kind {
                DeviceKind::SwitchDouble => line(c + vec2(-1.0, 5.0), c + vec2(h, h)),
                DeviceKind::SwitchTwoWay => line(c + vec2(h * 0.5, -h), c + vec2(h, -h * 0.5)),
                DeviceKind::PushButton => {
                    p.circle_filled(c + vec2(-3.0, 3.0), h * 0.2, t.icon_accent);
                }
                DeviceKind::Dimmer => {
                    p.add(egui::Shape::convex_polygon(
                        vec![c + vec2(-7.0, 7.0), c + vec2(1.0, 7.0), c + vec2(-3.0, 1.0)],
                        egui::Color32::TRANSPARENT,
                        s,
                    ));
                }
                _ => {}
            }
        }
        DeviceKind::Presence | DeviceKind::DataOutlet => {
            let q = Rect::from_center_size(c, vec2(h * 2.0, h * 1.5));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            p.text(
                c,
                egui::Align2::CENTER_CENTER,
                if kind == DeviceKind::Presence { "D" } else { "RJ" },
                egui::FontId::proportional(8.0),
                t.icon_accent,
            );
        }
        DeviceKind::Light => {
            p.circle_stroke(c, h, s);
            let d = h * 0.7;
            line(c + vec2(-d, -d), c + vec2(d, d));
            line(c + vec2(-d, d), c + vec2(d, -d));
        }
        DeviceKind::Recessed => {
            let q = Rect::from_center_size(c, vec2(h * 1.8, h * 1.8));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            line(q.left_top(), q.right_bottom());
            line(q.left_bottom(), q.right_top());
        }
        DeviceKind::DownLight => {
            p.circle_stroke(c, h, s);
            p.circle_stroke(c, h * 0.4, s);
        }
        DeviceKind::WallLight => {
            p.circle_stroke(c, h * 0.7, s);
            line(c + vec2(-h, h * 0.7), c + vec2(h, h * 0.7));
        }
        DeviceKind::StripLight => {
            let q = Rect::from_center_size(c, vec2(r.width(), h * 0.8));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            line(pos2(q.left(), c.y), pos2(q.right(), c.y));
        }
        DeviceKind::Emergency | DeviceKind::ExitSign => {
            let q = Rect::from_center_size(c, vec2(h * 2.4, h * 1.4));
            p.rect_stroke(q, 0.0, Stroke::new(1.2, egui::Color32::from_rgb(0x20, 0xb0, 0x50)), egui::StrokeKind::Inside);
            if kind == DeviceKind::ExitSign {
                line(c + vec2(-h, 0.0), c + vec2(h, 0.0));
                line(c + vec2(h, 0.0), c + vec2(h * 0.5, -h * 0.4));
                line(c + vec2(h, 0.0), c + vec2(h * 0.5, h * 0.4));
            } else {
                p.text(c, egui::Align2::CENTER_CENTER, "B", egui::FontId::proportional(8.0), egui::Color32::from_rgb(0x20, 0xb0, 0x50));
            }
        }
        DeviceKind::Panel => {
            let q = Rect::from_center_size(c, vec2(h * 2.2, h * 1.6));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            line(q.left_top(), q.right_bottom());
        }
    }
}

/// A cable as a line with a bend, in the colour of its use.
fn paint_cable(p: &egui::Painter, r: Rect, usage: CableUsage) {
    let colour = match usage {
        CableUsage::Power => egui::Color32::from_rgb(0xd0, 0x30, 0x30),
        CableUsage::Lighting => egui::Color32::from_rgb(0xe0, 0xb0, 0x20),
        CableUsage::Data => egui::Color32::from_rgb(0x30, 0x90, 0xd0),
        CableUsage::Safety => egui::Color32::from_rgb(0x20, 0xb0, 0x50),
        CableUsage::Bus => egui::Color32::from_rgb(0x90, 0x60, 0xd0),
        CableUsage::Remote => egui::Color32::from_rgb(0xff, 0x6f, 0xb5),
    };
    let s = Stroke::new(1.8, colour);
    let (a, b, c) = (pos2(r.left() + 2.0, r.bottom() - 3.0), pos2(r.center().x, r.bottom() - 3.0), pos2(r.right() - 2.0, r.top() + 3.0));
    p.line_segment([a, b], s);
    p.line_segment([b, c], s);
}

fn paint_check(p: &egui::Painter, r: Rect) {
    let t = Tokens::get();
    let s = Stroke::new(1.6, t.icon_accent);
    let c = r.center();
    p.line_segment([c + vec2(-6.0, 0.0), c + vec2(-2.0, 4.0)], s);
    p.line_segment([c + vec2(-2.0, 4.0), c + vec2(7.0, -5.0)], s);
}

fn paint_size(p: &egui::Painter, r: Rect) {
    let t = Tokens::get();
    let s = Stroke::new(1.4, t.icon_accent);
    let c = r.center();
    p.line_segment([c + vec2(-8.0, 0.0), c + vec2(8.0, 0.0)], s);
    p.line_segment([c + vec2(-8.0, -4.0), c + vec2(-8.0, 4.0)], s);
    p.line_segment([c + vec2(8.0, -4.0), c + vec2(8.0, 4.0)], s);
}

fn paint_lux(p: &egui::Painter, r: Rect) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    let c = r.center();
    p.circle_stroke(c, 3.5, s);
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        p.line_segment([c + vec2(a.cos(), a.sin()) * 5.5, c + vec2(a.cos(), a.sin()) * 8.0], s);
    }
}

/// The body of the Electricity tab.
pub(super) fn body(app: &mut CadApp, ui: &mut egui::Ui) {
    let current = app.session.last_used.device.kind;
    let usage = app.session.last_used.cable.usage;
    let mut chosen_device: Option<DeviceKind> = None;
    let mut chosen_usage: Option<CableUsage> = None;
    let mut start: Option<&'static str> = None;
    let (mut check, mut size, mut lighting, mut emergency) = (false, false, false, false);
    let sections = std::mem::take(&mut app.ui.sections);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.data_mut(|d| {
            d.insert_temp(sections_id(), sections);
            d.insert_temp(open_id(), true);
        });
        let groups: [(&str, &[DeviceKind]); 4] = [
            (
                "Sockets and controls",
                &[
                    DeviceKind::Socket,
                    DeviceKind::SocketSpecial,
                    DeviceKind::Switch,
                    DeviceKind::SwitchDouble,
                    DeviceKind::SwitchTwoWay,
                    DeviceKind::PushButton,
                    DeviceKind::Dimmer,
                    DeviceKind::Presence,
                    DeviceKind::DataOutlet,
                ],
            ),
            ("Luminaires", &[DeviceKind::Light, DeviceKind::Recessed, DeviceKind::DownLight, DeviceKind::WallLight, DeviceKind::StripLight]),
            ("Safety", &[DeviceKind::Emergency, DeviceKind::ExitSign]),
            ("Distribution", &[DeviceKind::Panel]),
        ];
        for (title, kinds) in groups {
            header(ui, title);
            for kind in kinds {
                if row_tip(ui, crate::i18n::t(kind.name()), "", Some("Electrical Device"), *kind == current, |p, r| paint_device(p, r, *kind)) {
                    chosen_device = Some(*kind);
                }
            }
            ui.add_space(6.0);
            separator(ui);
        }
        header(ui, "Cables");
        for (u, label) in [
            (CableUsage::Power, "Power cable"),
            (CableUsage::Lighting, "Lighting cable"),
            (CableUsage::Data, "Data cable"),
            (CableUsage::Safety, "Safety cable"),
            (CableUsage::Remote, "Remote control cable"),
        ] {
            if row_tip(ui, crate::i18n::t(label), "", Some("Cable"), u == usage, |p, r| paint_cable(p, r, u)) {
                chosen_usage = Some(u);
            }
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Circuits");
        if row_tip(ui, crate::i18n::t("Assign to Circuit"), crate::i18n::t("Puts the selected devices, cables and equipment on one circuit."), Some("Assign to Circuit"), false, paint_check) {
            start = Some("circuitassign");
        }
        if row_tip(ui, crate::i18n::t("Check Circuits"), crate::i18n::t("Checks the load, the sections, the voltage drop and the short-circuit current of every circuit."), Some("Check Circuits"), false, paint_check) {
            check = true;
        }
        if row_tip(ui, crate::i18n::t("Size Circuits"), crate::i18n::t("Writes on the cables of each circuit the section it needs."), Some("Size Circuits"), false, paint_size) {
            size = true;
        }
        if row_tip(ui, crate::i18n::t("Check Lighting"), crate::i18n::t("Compares the illuminance of each space with the regulation's minimum."), Some("Check Lighting"), false, paint_lux) {
            lighting = true;
        }
        if row_tip(ui, crate::i18n::t("Check Emergency Lighting"), crate::i18n::t("Says where emergency lighting or an exit sign is missing."), Some("Check Emergency Lighting"), false, paint_lux) {
            emergency = true;
        }
        if row_tip(ui, crate::i18n::t("Circuit Legend"), crate::i18n::t("A row for each circuit: its colour, name, cable, length, power and breaker."), Some("Circuit Legend"), false, super::thumbs::paint_quantities) {
            start = Some("circuitlegend");
        }
        if row_tip(ui, crate::i18n::t("Single-Line Diagram"), crate::i18n::t("The boards with their breakers and, for each circuit, its breaker, cable and load."), Some("Single-Line Diagram"), false, super::thumbs::paint_quantities) {
            start = Some("singleline");
        }
        if row_tip(
            ui,
            crate::i18n::t("Electrical Riser"),
            crate::i18n::t("Draw the shaft where the cables go up or down between the floors. Name the circuit it carries: its vertical run (the height from floor to floor) is counted in the circuit's cable length, and the load of the other floors in its load."),
            Some("Electrical Riser"),
            false,
            super::thumbs::paint_riser,
        ) {
            start = Some("elecriser");
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Quantities");
        if row_tip(ui, crate::i18n::t("Electrical Quantities"), crate::i18n::t("The cables, devices, boards and loads of the level, for Excel."), Some("Electrical Quantities"), false, super::thumbs::paint_quantities) {
            start = Some("electakeoff");
        }
    });
    app.ui.sections = ui
        .data_mut(|d| {
            d.insert_temp(open_id(), true);
            d.remove_temp::<crate::sections::Sections>(sections_id())
        })
        .unwrap_or_default();
    hvachelp::show(ui.ctx());
    if let Some(kind) = chosen_device {
        app.session.last_used.device.kind = kind;
        app.cmd.buffer.clear();
        app.start("elecdeviceadd");
    } else if let Some(u) = chosen_usage {
        app.session.last_used.cable.usage = u;
        app.cmd.buffer.clear();
        app.start("cableadd");
    } else if check {
        run_and_say(app, "circuitcheck", serde_json::json!({}), say_circuits);
    } else if size {
        run_and_say(app, "circuitsize", serde_json::json!({}), say_circuits);
    } else if lighting {
        run_and_say(app, "lightingcheck", serde_json::json!({}), say_lighting);
    } else if emergency {
        run_and_say(app, "emergencycheck", serde_json::json!({}), say_emergency);
    } else if let Some(id) = start {
        app.cmd.buffer.clear();
        app.start(id);
    }
}

fn run_and_say(app: &mut CadApp, command: &str, params: serde_json::Value, say: fn(&mut CadApp, &serde_json::Value)) {
    match app.run(command, params) {
        Ok(r) => say(app, &r),
        Err(e) => app.session.echo(e),
    }
}

/// Says how many circuits there are and what is wrong with each that is not right.
fn say_circuits(app: &mut CadApp, r: &serde_json::Value) {
    let circuits = r["circuits"].as_array().cloned().unwrap_or_default();
    if circuits.is_empty() {
        app.session.echo(crate::tl!("No circuit: draw cables and devices on a named circuit first.").to_string());
        return;
    }
    let problems = r["problems"].as_u64().unwrap_or(0);
    app.session.echo(crate::tf!("Circuits: {n}, to correct: {p}", n = circuits.len(), p = problems));
    for w in r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
        app.session.echo(w.to_string());
    }
}

/// Says where emergency lighting is missing, and that only its presence is checked.
fn say_emergency(app: &mut CadApp, r: &serde_json::Value) {
    let problems = r["problems"].as_u64().unwrap_or(0);
    if r["spaces"].as_array().is_none_or(Vec::is_empty) {
        app.session.echo(crate::tl!("No space: draw spaces to check their emergency lighting.").to_string());
        return;
    }
    app.session.echo(crate::tf!("Emergency lighting: {p} space(s) to correct", p = problems));
    for w in r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
        app.session.echo(w.to_string());
    }
    app.session
        .echo(crate::tl!("Presence only: the levels and the autonomy are those of the arrêté du 14 décembre 2011 and of NF EN 1838.").to_string());
}

/// Says each space with its illuminance against its minimum.
fn say_lighting(app: &mut CadApp, r: &serde_json::Value) {
    let spaces = r["spaces"].as_array().cloned().unwrap_or_default();
    if spaces.is_empty() {
        app.session.echo(crate::tl!("No space: draw spaces to check their lighting.").to_string());
        return;
    }
    for s in &spaces {
        app.session.echo(crate::tf!(
            "{space}: {lux} lux (minimum {min})",
            space = s["space"].as_str().unwrap_or(""),
            lux = s["lux"].as_f64().unwrap_or(0.0),
            min = s["minimumLux"].as_f64().unwrap_or(0.0)
        ));
    }
    for w in r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
        app.session.echo(w.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use cadcraft_engine::Session;
    use egui_kittest::kittest::Queryable;

    fn tab(app: CadApp) -> egui_kittest::Harness<'static, CadApp> {
        let mut h = egui_kittest::Harness::builder().with_size(vec2(400.0, 2400.0)).build_ui_state(move |ui, app: &mut CadApp| body(app, ui), app);
        h.run_steps(3);
        h
    }

    #[test]
    fn the_tab_lists_every_device_and_a_click_starts_the_command_with_it() {
        let mut h = tab(CadApp::new(Session::new(), Services::default()));
        for kind in DeviceKind::ALL {
            assert!(h.query_by_label(kind.name()).is_some(), "{}", kind.name());
        }
        h.get_by_label("Downlight").click();
        h.run_steps(3);
        assert_eq!(h.state().session.last_used.device.kind, DeviceKind::DownLight);
        assert!(h.state().session.running.is_some(), "the command is running");
        assert!(h.state().session.prompt_text().contains("device"), "{}", h.state().session.prompt_text());
    }

    #[test]
    fn a_check_with_nothing_drawn_says_so_and_the_quantities_window_opens_from_the_command() {
        let mut h = tab(CadApp::new(Session::new(), Services::default()));
        h.get_by_label("Check Circuits").click();
        h.run_steps(3);
        let said = h.state().session.log.iter().any(|l| l.contains("No circuit"));
        assert!(said, "{:?}", h.state().session.log);
        let mut app = CadApp::new(Session::new(), Services::default());
        app.run("electakeoff", serde_json::Value::Null).unwrap();
        assert_eq!(app.ui.dialog.as_deref(), Some("elecreport"));
        assert!(app.elec_dlg.as_ref().is_some_and(|s| s.takeoff.is_some()));
        // The JSON form still counts.
        assert!(app.run("electakeoff", serde_json::json!({})).unwrap()["circuits"].is_array());
    }
}
