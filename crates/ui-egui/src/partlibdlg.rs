//! The Pipework Part Library window (PIPEPARTMODELS typed, or the "Library…" row of the HVAC tab): the valves, pumps,
//! vessels, boilers, instruments and refrigerant branches and boxes, by family, with a search. A click chooses a part and starts
//! PIPEPARTADD with it; dragging one onto the drawing puts it where it is dropped.

use cadcraft_doc::{PartCategory, PartModel, standard_part_models};
use egui::{Color32, RichText, Sense, Stroke, pos2, vec2};

use crate::CadApp;
use crate::theme::Tokens;

/// What is carried while a part is dragged from the library: the name of its model.
#[derive(Clone, Debug, PartialEq)]
pub struct PartDrag(pub String);

/// The window while open.
#[derive(Clone, Debug, Default)]
pub struct PartLibState {
    /// The family shown; none: all.
    pub category: Option<PartCategory>,
    /// The words searched for in a name, a brand and the connections.
    pub query: String,
    pub raise: bool,
}

/// Open the window; open already, bring it forward.
pub fn open(app: &mut CadApp) {
    let mut st = app.part_lib.take().unwrap_or_default();
    st.raise = true;
    app.part_lib = Some(st);
    app.ui.dialog = Some("partlib".into());
}

/// The models of `category` (all when none) whose name, brand or connections hold every word of `query`.
pub fn filtered(models: Vec<PartModel>, category: Option<PartCategory>, query: &str) -> Vec<PartModel> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    models
        .into_iter()
        .filter(|m| category.is_none_or(|c| m.category == c))
        .filter(|m| {
            let hay = format!("{} {} {} {}", m.name, m.name_fr, m.brand, m.connections).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .collect()
}

/// The name of a model in the language of the interface.
pub fn display_name(m: &PartModel) -> &'static str {
    if crate::i18n::current_language().starts_with("fr") { m.name_fr } else { m.name }
}

/// Put a model of the library where it was dropped, at `at` in drawing coordinates; what went wrong is said on the command line.
pub fn drop_part(app: &mut CadApp, model: &str, at: cadcraft_geom::Vec2) {
    let r = app.run("pipepartadd", serde_json::json!({ "at": [at.x, at.y], "model": model }));
    match r {
        Ok(_) => app.session.last_used.part.model = model.to_string(),
        Err(e) => app.session.echo(e),
    }
}

const CARD: egui::Vec2 = vec2(150.0, 120.0);

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open_flag: &mut bool) {
    if app.part_lib.is_none() {
        open(app);
    }
    let t = Tokens::get();
    let current = app.session.last_used.part.model.clone();
    let Some(st) = app.part_lib.as_mut() else { return };
    let id = egui::Id::unique("part_library_window");
    if std::mem::take(&mut st.raise) {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
    let mut chosen: Option<String> = None;
    egui::Window::new(crate::tl!("Pipework Part Library")).id(id).open(open_flag).collapsible(false).default_size(vec2(780.0, 560.0)).show(
        ctx,
        |ui| {
            ui.horizontal(|ui| {
                let label = ui.label(crate::tl!("Search"));
                ui.add(egui::TextEdit::singleline(&mut st.query).desired_width(260.0)).labelled_by(label.id);
                if ui.button(crate::tl!("Clear")).clicked() {
                    st.query.clear();
                }
            });
            ui.label(
                RichText::new(crate::tl!("Click a part, then click where it goes (on a pipe it turns along it), or drag it onto the drawing."))
                    .small(),
            );
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(170.0);
                    if ui.selectable_label(st.category.is_none(), crate::tl!("All")).clicked() {
                        st.category = None;
                    }
                    for c in PartCategory::ALL {
                        if ui.selectable_label(st.category == Some(c), crate::i18n::t(c.title())).clicked() {
                            st.category = Some(c);
                        }
                    }
                });
                ui.separator();
                let models = filtered(standard_part_models(), st.category, &st.query);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        for m in &models {
                            card(ui, m, m.name == current, &t, &mut chosen);
                        }
                    });
                    if models.is_empty() {
                        ui.label(crate::tl!("No part matches."));
                    }
                });
            });
        },
    );
    // The part under the pointer while it is dragged, so the drop is seen coming.
    if let Some(drag) = egui::DragAndDrop::payload::<PartDrag>(ctx)
        && let Some(pos) = ctx.pointer_interact_pos()
    {
        egui::Area::new(egui::Id::unique("part_drag_ghost")).order(egui::Order::Tooltip).fixed_pos(pos + vec2(14.0, 14.0)).interactable(false).show(
            ctx,
            |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            standard_part_models().into_iter().find(|m| m.name == drag.0).map_or(drag.0.clone(), |m| display_name(&m).to_string()),
                        )
                        .small(),
                    );
                });
            },
        );
    }
    if let Some(name) = chosen {
        app.session.last_used.part.model = name;
        app.cmd.buffer.clear();
        app.start("pipepartadd");
    }
}

/// One model as a card: its symbol, its name and brand; clicked, it is chosen; dragged, it carries its name.
fn card(ui: &mut egui::Ui, m: &PartModel, selected: bool, t: &Tokens, chosen: &mut Option<String>) {
    let (rect, resp) = ui.allocate_exact_size(CARD, Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(
        rect,
        6.0,
        if selected {
            t.tab_active
        } else if resp.hovered() {
            t.chrome_dark
        } else {
            Color32::TRANSPARENT
        },
    );
    p.rect_stroke(rect, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let part = m.params("T", cadcraft_doc::HvacUnits::default());
    let art = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 38.0), vec2(30.0, 30.0));
    crate::palettes::paint_part(p, art, &part);
    let name = p.layout(display_name(m).to_string(), egui::FontId::proportional(11.5), t.text, CARD.x - 14.0);
    p.galley(pos2(rect.left() + 7.0, rect.top() + 72.0), name, t.text);
    let extra = if m.outlets > 0 { format!("{}x", m.outlets) } else { String::new() };
    p.text(pos2(rect.left() + 7.0, rect.bottom() - 6.0), egui::Align2::LEFT_BOTTOM, extra, egui::FontId::proportional(10.0), t.text_dim);
    if resp.drag_started() {
        resp.dnd_set_drag_payload(PartDrag(m.name.to_string()));
    }
    let resp = if m.connections.is_empty() {
        resp
    } else {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(360.0);
            ui.label(RichText::new(display_name(m)).strong());
            ui.label(&m.connections);
        })
    };
    if resp.clicked() {
        *chosen = Some(m.name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CadApp, Services};
    use cadcraft_engine::Session;

    #[test]
    fn the_library_is_searched_by_family_and_by_words() {
        let all = standard_part_models();
        let n = all.len();
        assert_eq!(filtered(all.clone(), None, "").len(), n);
        let valves = filtered(all.clone(), Some(PartCategory::Valves), "");
        assert!(!valves.is_empty() && valves.iter().all(|m| m.category == PartCategory::Valves));
        // Words from the name, in either language, the brand or the connections; all of them must be found.
        assert!(filtered(all.clone(), None, "balancing").iter().any(|m| m.name == "Balancing valve (TA)"));
        assert!(filtered(all.clone(), None, "papillon").iter().any(|m| m.name == "Butterfly valve"));
        let bs = filtered(all.clone(), None, "box 12 outlets");
        assert!(bs.len() >= 2 && bs.iter().all(|m| m.kind == cadcraft_doc::PartKind::BranchBox && m.outlets == 12));
        assert!(filtered(all.clone(), None, "zzzz").is_empty());
        assert!(filtered(all, Some(PartCategory::Pumps), "box").is_empty());
    }

    #[test]
    fn dropping_a_part_on_the_drawing_puts_it_there_and_remembers_the_model() {
        let mut app = CadApp::new(Session::new(), Services::default());
        *app.session.doc_mut().unwrap() = cadcraft_doc::Drawing::new_metric();
        drop_part(&mut app, "Circulator (single)", cadcraft_geom::Vec2::new(1500.0, 800.0));
        let parts: Vec<_> = app.session.doc().unwrap().model.iter().filter_map(|e| cadcraft_doc::part_of(e)).collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(app.session.last_used.part.model, "Circulator (single)");
        // A model that is not in the library puts nothing and says so.
        drop_part(&mut app, "nothing", cadcraft_geom::Vec2::ZERO);
        assert_eq!(app.session.doc().unwrap().model.iter().filter_map(|e| cadcraft_doc::part_of(e)).count(), 1);
    }

    #[test]
    fn typing_the_library_command_opens_the_window() {
        let mut app = CadApp::new(Session::new(), Services::default());
        app.run("pipepartmodels", serde_json::Value::Null).unwrap();
        assert_eq!(app.ui.dialog.as_deref(), Some("partlib"));
        assert!(app.part_lib.is_some());
        // The JSON form still lists the models.
        let r = app.run("pipepartmodels", serde_json::json!({})).unwrap();
        assert!(r["models"].as_array().is_some_and(|m| m.len() >= 80));
    }
}
