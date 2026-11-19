//! The Plumbing tab: the sanitary fixtures to put, the waste, soil, rain and supply networks to draw, and the tool that connects the fixtures to
//! their drains. Every row starts a command (the same the command line, the menus and the control channel reach).

use cadcraft_doc::{PipeLeg, PipeService, SanitaryKind};
use egui::{Rect, Stroke, pos2, vec2};

use super::building::{header, open_id, row_tip, sections_id, separator};
use super::{hvachelp, thumbs};
use crate::CadApp;
use crate::theme::Tokens;

/// The label of a kind of fixture in the tab and in Properties.
pub(crate) fn sanitary_kind_label(k: SanitaryKind) -> &'static str {
    match k {
        SanitaryKind::WcWallHung => "Wall-hung WC",
        SanitaryKind::WcFloor => "Floor-standing WC",
        SanitaryKind::WcAccessible => "Accessible WC",
        SanitaryKind::Urinal => "Urinal",
        SanitaryKind::Washbasin => "Washbasin",
        SanitaryKind::HandBasin => "Hand basin",
        SanitaryKind::DoubleBasin => "Double vanity basin",
        SanitaryKind::SinkSingle => "Sink, one bowl",
        SanitaryKind::SinkDouble => "Sink, two bowls",
        SanitaryKind::Shower => "Shower",
        SanitaryKind::Bath => "Bath",
        SanitaryKind::Bidet => "Bidet",
        SanitaryKind::SlopSink => "Slop sink",
        SanitaryKind::FloorDrain => "Floor drain",
        SanitaryKind::WashingMachine => "Washing machine",
        SanitaryKind::Dishwasher => "Dishwasher",
        SanitaryKind::WaterHeater => "Water heater",
        SanitaryKind::Tap => "Draw-off tap",
    }
}

/// The networks of the tab: the label and the service.
const NETWORKS: &[(&str, PipeService)] = &[
    ("Waste water (EU)", PipeService::WasteWater),
    ("Soil water (EV)", PipeService::SoilWater),
    ("Rain water (EP)", PipeService::RainWater),
    ("Cold water (EF)", PipeService::ColdWater),
    ("Domestic hot water (ECS)", PipeService::DomesticHotWater),
    ("Hot water return loop", PipeService::HotWaterLoop),
];

/// A small picture of fixture `kind` in `r`, seen from above as it is drawn in plan.
fn paint_fixture(p: &egui::Painter, r: Rect, kind: SanitaryKind) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    let c = r.center();
    let h = r.height() / 2.0 - 1.0;
    let rect = |w: f32, d: f32| Rect::from_center_size(c, vec2(w, d));
    let frame = |q: Rect| {
        p.rect_stroke(q, 1.0, s, egui::StrokeKind::Inside);
    };
    let oval = |centre: egui::Pos2, rx: f32, ry: f32| {
        let pts: Vec<egui::Pos2> = (0..24)
            .map(|k| {
                let a = k as f32 / 24.0 * std::f32::consts::TAU;
                centre + vec2(rx * a.cos(), ry * a.sin())
            })
            .collect();
        p.add(egui::Shape::closed_line(pts, s));
    };
    match kind {
        SanitaryKind::WcWallHung | SanitaryKind::WcFloor | SanitaryKind::WcAccessible => {
            let back = Rect::from_min_size(pos2(c.x - h * 0.6, c.y - h), vec2(h * 1.2, h * 0.45));
            frame(back);
            oval(c + vec2(0.0, h * 0.25), h * 0.5, h * 0.7);
            if kind == SanitaryKind::WcAccessible {
                p.line_segment([pos2(c.x + h * 0.95, c.y - h * 0.5), pos2(c.x + h * 0.95, c.y + h)], s);
            }
        }
        SanitaryKind::Urinal => {
            p.line_segment([pos2(c.x - h * 0.6, c.y - h * 0.5), pos2(c.x + h * 0.6, c.y - h * 0.5)], s);
            oval(c, h * 0.6, h * 0.5);
        }
        SanitaryKind::Washbasin | SanitaryKind::HandBasin | SanitaryKind::SlopSink => {
            frame(rect(h * 2.0, h * 1.5));
            oval(c + vec2(0.0, h * 0.1), h * 0.65, h * 0.4);
        }
        SanitaryKind::DoubleBasin => {
            frame(rect(h * 2.0, h * 1.2));
            oval(c + vec2(-h * 0.5, 0.0), h * 0.35, h * 0.3);
            oval(c + vec2(h * 0.5, 0.0), h * 0.35, h * 0.3);
        }
        SanitaryKind::SinkSingle | SanitaryKind::SinkDouble => {
            frame(rect(h * 2.0, h * 1.3));
            let bowls = if kind == SanitaryKind::SinkSingle { 1 } else { 2 };
            for k in 0..bowls {
                let x = c.x - h * 0.85 + k as f32 * h * 0.65;
                frame(Rect::from_min_size(pos2(x, c.y - h * 0.45), vec2(h * 0.55, h * 0.9)));
            }
            for k in 0..3 {
                let y = c.y - h * 0.3 + k as f32 * h * 0.3;
                p.line_segment([pos2(c.x + h * 0.4, y), pos2(c.x + h * 0.9, y)], s);
            }
        }
        SanitaryKind::Shower => {
            let q = rect(h * 1.8, h * 1.8);
            frame(q);
            p.line_segment([q.left_top(), q.right_bottom()], s);
            p.line_segment([q.right_top(), q.left_bottom()], s);
            p.circle_stroke(c, h * 0.2, s);
        }
        SanitaryKind::Bath => {
            frame(rect(h * 2.0, h * 1.0));
            p.rect_stroke(rect(h * 1.7, h * 0.7), h * 0.3, s, egui::StrokeKind::Inside);
        }
        SanitaryKind::Bidet => {
            oval(c, h * 0.55, h * 0.9);
            oval(c + vec2(0.0, h * 0.1), h * 0.3, h * 0.5);
        }
        SanitaryKind::FloorDrain => {
            let q = rect(h * 1.2, h * 1.2);
            frame(q);
            for k in 1..4 {
                let y = q.top() + q.height() * k as f32 / 4.0;
                p.line_segment([pos2(q.left() + 2.0, y), pos2(q.right() - 2.0, y)], s);
            }
        }
        SanitaryKind::WashingMachine => {
            frame(rect(h * 1.8, h * 1.8));
            p.circle_stroke(c, h * 0.6, s);
        }
        SanitaryKind::Dishwasher => {
            let q = rect(h * 1.8, h * 1.8);
            frame(q);
            for k in 1..5 {
                let x = q.left() + q.width() * k as f32 / 5.0;
                p.line_segment([pos2(x, q.top() + 3.0), pos2(x, q.bottom() - 2.0)], s);
            }
        }
        SanitaryKind::WaterHeater => {
            p.circle_stroke(c, h * 0.9, s);
            p.circle_stroke(c, h * 0.7, s);
        }
        SanitaryKind::Tap => {
            p.circle_stroke(c, h * 0.35, s);
            p.line_segment([pos2(c.x, c.y - h), pos2(c.x, c.y - h * 0.35)], s);
        }
    }
}

/// The body of the Plumbing tab.
pub(super) fn body(app: &mut CadApp, ui: &mut egui::Ui) {
    let current = app.session.last_used.sanitary.kind;
    let pipe_now = app.session.last_used.pipe.service;
    let mut chosen_fixture: Option<SanitaryKind> = None;
    let mut chosen_pipe: Option<PipeService> = None;
    let mut start: Option<&'static str> = None;
    let mut connect = false;
    let sections = std::mem::take(&mut app.ui.sections);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.data_mut(|d| {
            d.insert_temp(sections_id(), sections);
            d.insert_temp(open_id(), true);
        });
        header(ui, "Sanitary fixtures");
        for kind in SanitaryKind::ALL {
            if row_tip(ui, crate::i18n::t(sanitary_kind_label(kind)), "", Some("Sanitary Fixture"), kind == current, |p, r| paint_fixture(p, r, kind))
            {
                chosen_fixture = Some(kind);
            }
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Networks");
        for &(label, service) in NETWORKS {
            if row_tip(ui, crate::i18n::t(label), "", Some("Plumbing Networks"), pipe_now == service, |p, r| {
                thumbs::paint_pipe(p, r, service, PipeLeg::Supply)
            }) {
                chosen_pipe = Some(service);
            }
        }
        if row_tip(
            ui,
            crate::i18n::t("Pipe Riser"),
            crate::i18n::t("The shaft where a stack (soil or waste water) or the supply pipes go up or down between the floors."),
            Some("Pipe Riser"),
            false,
            thumbs::paint_pipe_riser,
        ) {
            start = Some("piperiseradd");
        }
        if row_tip(ui, crate::i18n::t("Edit Pipe"), "", None, false, thumbs::paint_edit_pipe) {
            start = Some("pipeedit");
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Tools");
        if row_tip(
            ui,
            crate::i18n::t("Connect Fixtures"),
            crate::i18n::t(
                "Draws from each fixture's drain the branch to the nearest drain of its kind, as wide as the fixture's drain and with its slope.",
            ),
            Some("Connect Fixtures"),
            false,
            thumbs::paint_edit_pipe,
        ) {
            connect = true;
        }
    });
    app.ui.sections = ui
        .data_mut(|d| {
            d.insert_temp(open_id(), true);
            d.remove_temp::<crate::sections::Sections>(sections_id())
        })
        .unwrap_or_default();
    hvachelp::show(ui.ctx());
    if let Some(kind) = chosen_fixture {
        app.session.last_used.sanitary.kind = kind;
        app.cmd.buffer.clear();
        app.start("fixtureadd");
    } else if let Some(service) = chosen_pipe {
        // The service clicked becomes PIPEADD's, at its own size, then it starts with it.
        let last = &mut app.session.last_used.pipe;
        (last.service, last.leg, last.dn_mm) = (service, PipeLeg::Supply, None);
        app.cmd.buffer.clear();
        app.start("pipeadd");
    } else if connect {
        match app.run("fixtureconnect", serde_json::json!({})) {
            Ok(r) => {
                let made = r["created"].as_array().map_or(0, Vec::len);
                app.session.echo(crate::tf!("Connect Fixtures: {n} branches drawn", n = made));
                for s in r["skipped"].as_array().into_iter().flatten() {
                    if let Some(reason) = s["reason"].as_str() {
                        app.session.echo(format!("{} {}", s["fixture"].as_str().unwrap_or(""), crate::i18n::t(reason)));
                    }
                }
            }
            Err(e) => app.session.echo(e),
        }
    } else if let Some(id) = start {
        app.cmd.buffer.clear();
        app.start(id);
    }
}
