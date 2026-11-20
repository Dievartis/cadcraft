//! The Furniture tab: the catalogue of furniture and appliances, grouped (living room, bedroom, kitchen, appliances, offices…), each with a small picture of its
//! plan symbol. A row starts FURNITUREADD with that model.

use cadcraft_doc::{FurnitureModel, Shape, furniture_groups, furniture_models};
use egui::{Rect, Stroke, pos2, vec2};

use super::building::{header, header_with, open_id, row_tip, sections_id, separator};
use super::hvachelp;
use crate::CadApp;
use crate::theme::Tokens;

/// A small picture of `model` in `r`: its plan symbol at its proportions.
pub(super) fn paint_model(p: &egui::Painter, r: Rect, model: &FurnitureModel) {
    let t = Tokens::get();
    let s = Stroke::new(1.0, t.icon_accent);
    let (w, d) = (model.width_mm as f32, model.depth_mm as f32);
    // Fit the piece in the box; a very thin one (a television) keeps at least a few points.
    let scale = (r.width() / w).min(r.height() / d);
    let (pw, ph) = ((w * scale).max(2.0), (d * scale).max(2.0));
    let frame = Rect::from_center_size(r.center(), vec2(pw, ph));
    let at = |x: f64, y: f64| pos2(frame.left() + frame.width() * x as f32, frame.top() + frame.height() * y as f32);
    for shape in model.shapes {
        match *shape {
            Shape::Rect(x0, y0, x1, y1) => {
                p.rect_stroke(Rect::from_two_pos(at(x0, y0), at(x1, y1)), 0.0, s, egui::StrokeKind::Middle);
            }
            Shape::Round(x0, y0, x1, y1, rad) => {
                p.rect_stroke(Rect::from_two_pos(at(x0, y0), at(x1, y1)), (rad as f32 * pw.min(ph)).max(0.0), s, egui::StrokeKind::Middle);
            }
            Shape::Oval(cx, cy, rx, ry) => {
                let c = at(cx, cy);
                let (rx, ry) = (rx as f32 * pw, ry as f32 * ph);
                let pts: Vec<egui::Pos2> = (0..20)
                    .map(|k| {
                        let a = k as f32 / 20.0 * std::f32::consts::TAU;
                        c + vec2(rx * a.cos(), ry * a.sin())
                    })
                    .collect();
                p.add(egui::Shape::closed_line(pts, s));
            }
            Shape::Line(x0, y0, x1, y1) => {
                p.line_segment([at(x0, y0), at(x1, y1)], s);
            }
            Shape::Poly(points) => {
                p.add(egui::Shape::closed_line(points.iter().map(|(x, y)| at(*x, *y)).collect(), s));
            }
        }
    }
}

/// The body of the Furniture tab.
pub(super) fn body(app: &mut CadApp, ui: &mut egui::Ui) {
    let current = app.session.last_used.furniture.model.clone();
    let mut chosen: Option<&'static str> = None;
    let sections = std::mem::take(&mut app.ui.sections);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.data_mut(|d| {
            d.insert_temp(sections_id(), sections);
            d.insert_temp(open_id(), true);
        });
        for (i, group) in furniture_groups().iter().enumerate() {
            // The first group's title carries the help of the whole tab.
            if i == 0 {
                header_with(ui, group, Some("Furniture"));
            } else {
                header(ui, group);
            }
            for model in furniture_models().iter().filter(|m| m.group == *group) {
                if row_tip(ui, crate::i18n::t(model.label), "", None, model.id == current, |p, r| paint_model(p, r, model)) {
                    chosen = Some(model.id);
                }
            }
            ui.add_space(6.0);
            separator(ui);
        }
    });
    app.ui.sections = ui
        .data_mut(|d| {
            d.insert_temp(open_id(), true);
            d.remove_temp::<crate::sections::Sections>(sections_id())
        })
        .unwrap_or_default();
    hvachelp::show(ui.ctx());
    if let Some(id) = chosen {
        app.session.last_used.furniture.model = id.to_string();
        app.cmd.buffer.clear();
        app.start("furnitureadd");
    }
}
