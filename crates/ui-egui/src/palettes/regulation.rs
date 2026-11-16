//! The Regulation tab: the sensors, the room controls, the actuators and the automation of the building controls, the bus cable, the check of what each device
//! asks of its controller and the list of points. Every row starts a command (the same the command line, the menus and the control channel reach).

use cadcraft_doc::{CableUsage, ControlGroup, ControlKind};
use egui::{Rect, Stroke, vec2};

use super::building::{header, open_id, row_button, row_tip, sections_id, separator};
use super::hvachelp;
use crate::CadApp;
use crate::theme::Tokens;

/// A small picture of `kind`: a circle (a sensor) or a box (anything else) with the letters of its tag.
fn paint_control(p: &egui::Painter, r: Rect, kind: ControlKind) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    let c = r.center();
    let h = r.height() / 2.0;
    match kind.group() {
        ControlGroup::Sensor => {
            p.circle_stroke(c, h, s);
        }
        ControlGroup::Actuator => {
            let q = Rect::from_center_size(c, vec2(h * 2.2, h * 1.8));
            p.rect_stroke(q, 3.0, s, egui::StrokeKind::Inside);
        }
        _ => {
            let q = Rect::from_center_size(c, vec2(h * 2.4, h * 1.8));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
        }
    }
    p.text(c, egui::Align2::CENTER_CENTER, kind.tag_prefix(), egui::FontId::proportional(8.5), t.icon_accent);
}

fn paint_bus(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(1.8, egui::Color32::from_rgb(0x90, 0x60, 0xd0));
    let y = r.center().y;
    p.line_segment([egui::pos2(r.left() + 2.0, y), egui::pos2(r.right() - 2.0, y)], s);
    for k in 0..3 {
        let x = r.left() + 6.0 + k as f32 * 9.0;
        p.line_segment([egui::pos2(x, y), egui::pos2(x, y - 5.0)], s);
    }
}

fn paint_points(p: &egui::Painter, r: Rect) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    for k in 0..3 {
        let y = r.top() + 3.0 + k as f32 * 5.0;
        p.circle_filled(egui::pos2(r.left() + 4.0, y), 1.6, t.icon_accent);
        p.line_segment([egui::pos2(r.left() + 9.0, y), egui::pos2(r.right() - 2.0, y)], s);
    }
}

/// The body of the Regulation tab.
pub(super) fn body(app: &mut CadApp, ui: &mut egui::Ui) {
    let current = app.session.last_used.control.kind;
    let mut chosen: Option<ControlKind> = None;
    let mut bus = false;
    let mut check = false;
    let mut addresses: Option<bool> = None;
    let mut start: Option<&'static str> = None;
    let sections = std::mem::take(&mut app.ui.sections);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.data_mut(|d| {
            d.insert_temp(sections_id(), sections);
            d.insert_temp(open_id(), true);
        });
        for (title, group) in [
            ("Sensors", ControlGroup::Sensor),
            ("Room controls", ControlGroup::RoomControl),
            ("Actuators", ControlGroup::Actuator),
            ("Automation", ControlGroup::Automation),
        ] {
            header(ui, title);
            for kind in ControlKind::ALL.into_iter().filter(|k| k.group() == group) {
                if row_button(ui, crate::i18n::t(kind.name()), kind == current, |p, r| paint_control(p, r, kind)) {
                    chosen = Some(kind);
                }
            }
            if group == ControlGroup::Automation && row_button(ui, crate::i18n::t("Bus cable"), false, paint_bus) {
                bus = true;
            }
            ui.add_space(6.0);
            separator(ui);
        }
        header(ui, "Points");
        if row_button(ui, crate::i18n::t("Check Controls"), false, paint_points) {
            check = true;
        }
        if row_tip(
            ui,
            crate::i18n::t("Show Addresses"),
            crate::i18n::t(
                "Writes the address beside every object that has one: control devices, equipment, air handling units, electrical devices.",
            ),
            None,
            false,
            paint_address,
        ) {
            addresses = Some(true);
        }
        if row_button(ui, crate::i18n::t("Hide Addresses"), false, paint_address) {
            addresses = Some(false);
        }
        if row_tip(
            ui,
            crate::i18n::t("Points List"),
            crate::i18n::t(
                "The points each device asks of its controller, the load of each controller and module, and a workbook for the controls package.",
            ),
            Some("Points List"),
            false,
            paint_points,
        ) {
            start = Some("controltakeoff");
        }
    });
    app.ui.sections = ui
        .data_mut(|d| {
            d.insert_temp(open_id(), true);
            d.remove_temp::<crate::sections::Sections>(sections_id())
        })
        .unwrap_or_default();
    hvachelp::show(ui.ctx());
    if let Some(kind) = chosen {
        app.session.last_used.control.kind = kind;
        app.cmd.buffer.clear();
        app.start("controladd");
    } else if bus {
        app.session.last_used.cable.usage = CableUsage::Bus;
        app.cmd.buffer.clear();
        app.start("cableadd");
    } else if let Some(show) = addresses {
        show_addresses(app, show);
    } else if check {
        say_controls(app);
    } else if let Some(id) = start {
        app.cmd.buffer.clear();
        app.start(id);
    }
}

fn paint_address(p: &egui::Painter, r: Rect) {
    let t = Tokens::get();
    p.text(r.center(), egui::Align2::CENTER_CENTER, "@12", egui::FontId::proportional(10.0), t.icon_accent);
}

/// Shows or hides the addresses, and says how many objects changed and the clashes.
fn show_addresses(app: &mut CadApp, show: bool) {
    match app.run("addressdisplay", serde_json::json!({ "show": show })) {
        Ok(r) => {
            app.session.echo(crate::tf!(
                "Addresses: {n} object(s) changed, {t} address(es) in the drawing",
                n = r["changed"].as_u64().unwrap_or(0),
                t = r["addresses"].as_array().map_or(0, Vec::len)
            ));
            for c in r["clashes"].as_array().into_iter().flatten().filter_map(|c| c.as_str()) {
                app.session.echo(c.to_string());
            }
        }
        Err(e) => app.session.echo(e),
    }
}

/// Runs CONTROLCHECK and says what it found.
fn say_controls(app: &mut CadApp) {
    match app.run("controlcheck", serde_json::json!({})) {
        Ok(r) => {
            let devices: usize = r["devices"].as_array().into_iter().flatten().filter_map(|d| d["count"].as_u64()).sum::<u64>() as usize;
            if devices == 0 {
                app.session.echo(crate::tl!("No control device: draw sensors, controllers and actuators first.").to_string());
                return;
            }
            app.session.echo(crate::tf!(
                "Controls: {n} device(s), {p} point(s), to correct: {w}",
                n = devices,
                p = r["points"].as_array().map_or(0, Vec::len),
                w = r["warnings"].as_array().map_or(0, Vec::len)
            ));
            for w in r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
                app.session.echo(w.to_string());
            }
        }
        Err(e) => app.session.echo(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use cadcraft_engine::Session;
    use egui_kittest::kittest::Queryable;

    fn tab(app: CadApp) -> egui_kittest::Harness<'static, CadApp> {
        let mut h = egui_kittest::Harness::builder().with_size(vec2(400.0, 2800.0)).build_ui_state(move |ui, app: &mut CadApp| body(app, ui), app);
        h.run_steps(3);
        h
    }

    #[test]
    fn every_kind_is_listed_and_a_click_starts_the_command_with_it() {
        let mut h = tab(CadApp::new(Session::new(), Services::default()));
        for kind in ControlKind::ALL {
            assert!(h.query_by_label(kind.name()).is_some(), "{}", kind.name());
        }
        h.get_by_label("Room thermostat").click();
        h.run_steps(3);
        assert_eq!(h.state().session.last_used.control.kind, ControlKind::Thermostat);
        assert!(h.state().session.running.is_some());
    }

    #[test]
    fn the_bus_cable_row_starts_the_cable_command_with_the_bus_use_and_the_check_says_when_nothing_is_drawn() {
        let mut h = tab(CadApp::new(Session::new(), Services::default()));
        h.get_by_label("Bus cable").click();
        h.run_steps(3);
        assert_eq!(h.state().session.last_used.cable.usage, CableUsage::Bus);
        let mut h = tab(CadApp::new(Session::new(), Services::default()));
        h.get_by_label("Check Controls").click();
        h.run_steps(3);
        assert!(h.state().session.log.iter().any(|l| l.contains("No control device")), "{:?}", h.state().session.log);
        let mut app = CadApp::new(Session::new(), Services::default());
        app.run("controltakeoff", serde_json::Value::Null).unwrap();
        assert_eq!(app.ui.dialog.as_deref(), Some("controlpoints"));
        assert!(app.ctrl_dlg.as_ref().is_some_and(|s| s.takeoff.is_some()));
    }

    #[test]
    fn the_address_rows_show_and_hide_the_addresses_and_say_how_many_objects_changed() {
        let mut app = CadApp::new(Session::new(), Services::default());
        app.run("controladd", serde_json::json!({"at": [0, 0], "kind": "thermostat", "address": "12"})).unwrap();
        let mut h = tab(app);
        h.get_by_label("Hide Addresses").click();
        h.run_steps(3);
        assert!(h.state().session.log.iter().any(|l| l.contains("1 object(s) changed")), "{:?}", h.state().session.log);
        h.get_by_label("Show Addresses").click();
        h.run_steps(3);
        let shown = h.state().session.doc().unwrap().model.iter().filter_map(|e| cadcraft_doc::control_of(e)).all(|c| c.show_address);
        assert!(shown);
    }
}
