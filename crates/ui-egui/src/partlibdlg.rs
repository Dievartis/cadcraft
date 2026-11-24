//! The Pipework Part Library window (PIPEPARTMODELS typed, or the "Library…" row of the HVAC tab): the valves, pumps,
//! vessels, boilers, instruments and refrigerant branches and boxes, by family, with a search, and the drawing's own models. A
//! click chooses a part and starts PIPEPARTADD with it; dragging one onto the drawing puts it where it is dropped; a part of the
//! drawing can be kept as a model of its own.

use cadcraft_doc::{PartCategory, PartEntry};
use egui::{RichText, Sense, Stroke, pos2, vec2};
use serde_json::json;

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
    /// Only the models the drawing keeps of its own.
    pub mine: bool,
    /// The words searched for in a name and the connections.
    pub query: String,
    /// The name a part of the drawing is kept under.
    pub save_name: String,
    pub message: Option<String>,
    pub raise: bool,
}

/// Open the window; open already, bring it forward.
pub fn open(app: &mut CadApp) {
    let mut st = app.part_lib.take().unwrap_or_default();
    st.raise = true;
    app.part_lib = Some(st);
    app.ui.dialog = Some("partlib".into());
}

/// The models of `category` (all when none; only the drawing's own when `mine`) whose name or connections hold every word of `query`.
pub fn filtered(models: Vec<PartEntry>, category: Option<PartCategory>, mine: bool, query: &str) -> Vec<PartEntry> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    models
        .into_iter()
        .filter(|m| (!mine || m.user) && category.is_none_or(|c| m.category == c))
        .filter(|m| {
            let hay = format!("{} {} {}", m.name, m.name_fr, m.connections()).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .collect()
}

/// The name of a model in the language of the interface.
pub fn display_name(m: &PartEntry) -> &str {
    if crate::i18n::current_language().starts_with("fr") { &m.name_fr } else { &m.name }
}

/// Put a model of the library where it was dropped, at `at` in drawing coordinates, then ask its orientation as a click on the plan
/// does (PIPEPARTADD's second prompt: a click, an angle, or Enter). What went wrong is said on the command line.
pub fn drop_part(app: &mut CadApp, model: &str, at: cadcraft_geom::Vec2) {
    let known = app.session.doc().map(|d| d.part_library()).unwrap_or_default();
    let Some(entry) = known.iter().find(|m| m.name.eq_ignore_ascii_case(model) || m.name_fr.eq_ignore_ascii_case(model)) else {
        app.session.echo(format!("PIPEPARTADD: no part model `{model}` in the library."));
        return;
    };
    app.session.last_used.part.model = entry.name.clone();
    app.cmd.buffer.clear();
    app.start("pipepartadd");
    app.cmdline(&format!("{},{}", at.x, at.y));
}

const CARD: egui::Vec2 = vec2(150.0, 120.0);

/// What a click in the window asks for, done once the window is drawn.
#[derive(Default)]
struct Asked {
    choose: Option<String>,
    delete: Option<String>,
    save: bool,
}

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open_flag: &mut bool) {
    if app.part_lib.is_none() {
        open(app);
    }
    let t = Tokens::get();
    let current = app.session.last_used.part.model.clone();
    let models = app.session.doc().map(|d| d.part_library()).unwrap_or_default();
    let selected_part = app
        .session
        .selection()
        .into_iter()
        .find(|h| app.session.doc().ok().and_then(|d| d.entity(*h)).and_then(|e| cadcraft_doc::part_of(e)).is_some());
    let Some(st) = app.part_lib.as_mut() else { return };
    let id = egui::Id::unique("part_library_window");
    if std::mem::take(&mut st.raise) {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
    let mut asked = Asked::default();
    egui::Window::new(crate::tl!("Pipework Part Library")).id(id).open(open_flag).collapsible(false).default_size(vec2(780.0, 580.0)).show(
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
                    if ui.selectable_label(st.category.is_none() && !st.mine, crate::tl!("All")).clicked() {
                        (st.category, st.mine) = (None, false);
                    }
                    if ui.selectable_label(st.mine, crate::tl!("My models")).clicked() {
                        (st.category, st.mine) = (None, true);
                    }
                    ui.separator();
                    for c in PartCategory::ALL {
                        if ui.selectable_label(st.category == Some(c) && !st.mine, crate::i18n::t(c.title())).clicked() {
                            (st.category, st.mine) = (Some(c), false);
                        }
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    let shown = filtered(models.clone(), st.category, st.mine, &st.query);
                    let height = ui.available_height() - 52.0;
                    egui::ScrollArea::vertical().max_height(height.max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            for m in &shown {
                                card(ui, m, m.name == current, &t, &mut asked);
                            }
                        });
                        if shown.is_empty() {
                            ui.label(if st.mine {
                                crate::tl!("No model of your own yet: select a part of the drawing and keep it as a model.")
                            } else {
                                crate::tl!("No part matches.")
                            });
                        }
                    });
                    ui.separator();
                    // A part of the drawing kept as a model of its own.
                    ui.horizontal(|ui| {
                        let label = ui.label(crate::tl!("Name of the model"));
                        ui.add(egui::TextEdit::singleline(&mut st.save_name).desired_width(220.0)).labelled_by(label.id);
                        let can = selected_part.is_some() && !st.save_name.trim().is_empty();
                        if ui.add_enabled(can, egui::Button::new(crate::tl!("Keep the selected part as a model"))).clicked() {
                            asked.save = true;
                        }
                    });
                    if let Some(m) = &st.message {
                        ui.label(RichText::new(m).small());
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
                    ui.label(RichText::new(models.iter().find(|m| m.name == drag.0).map_or(drag.0.clone(), |m| display_name(m).to_string())).small());
                });
            },
        );
    }
    if asked.save
        && let Some(h) = selected_part
    {
        let name = app.part_lib.as_ref().map(|s| s.save_name.clone()).unwrap_or_default();
        let r = app.run("pipepartmodels", json!({ "save": h.hex(), "name": name }));
        if let Some(st) = app.part_lib.as_mut() {
            st.message = Some(match r {
                Ok(_) => crate::tf!("Model kept: {name}", name = name.trim()),
                Err(e) => e,
            });
        }
    }
    if let Some(name) = asked.delete {
        let r = app.run("pipepartmodels", json!({ "delete": name }));
        if let (Err(e), Some(st)) = (r, app.part_lib.as_mut()) {
            st.message = Some(e);
        }
    }
    if let Some(name) = asked.choose {
        app.session.last_used.part.model = name;
        app.cmd.buffer.clear();
        app.start("pipepartadd");
    }
}

/// One model as a card: its symbol, its name; clicked, it is chosen; dragged, it carries its name; a model of the drawing's own
/// can be deleted from its menu.
fn card(ui: &mut egui::Ui, m: &PartEntry, selected: bool, t: &Tokens, asked: &mut Asked) {
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
            egui::Color32::TRANSPARENT
        },
    );
    p.rect_stroke(rect, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let part = m.params("T", cadcraft_doc::HvacUnits::default());
    let art = egui::Rect::from_center_size(pos2(rect.center().x, rect.top() + 38.0), vec2(30.0, 30.0));
    crate::palettes::paint_part(p, art, &part);
    let name = p.layout(display_name(m).to_string(), egui::FontId::proportional(11.5), t.text, CARD.x - 14.0);
    p.galley(pos2(rect.left() + 7.0, rect.top() + 72.0), name, t.text);
    let extra = match (m.user, m.outlets()) {
        (true, 0) => crate::i18n::t("My models").to_string(),
        (true, n) => format!("{} · {n}x", crate::i18n::t("My models")),
        (false, n) if n > 0 => format!("{n}x"),
        _ => String::new(),
    };
    p.text(pos2(rect.left() + 7.0, rect.bottom() - 6.0), egui::Align2::LEFT_BOTTOM, extra, egui::FontId::proportional(10.0), t.text_dim);
    if resp.drag_started() {
        resp.dnd_set_drag_payload(PartDrag(m.name.clone()));
    }
    let resp = if m.connections().is_empty() {
        resp
    } else {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(360.0);
            ui.label(RichText::new(display_name(m)).strong());
            ui.label(m.connections());
        })
    };
    if m.user {
        resp.context_menu(|ui| {
            if ui.button(crate::tl!("Delete this model")).clicked() {
                asked.delete = Some(m.name.clone());
                ui.close();
            }
        });
    }
    if resp.clicked() {
        asked.choose = Some(m.name.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CadApp, Services};
    use cadcraft_engine::Session;

    fn library(app: &CadApp) -> Vec<PartEntry> {
        app.session.doc().unwrap().part_library()
    }

    fn app() -> CadApp {
        let mut app = CadApp::new(Session::new(), Services::default());
        *app.session.doc_mut().unwrap() = cadcraft_doc::Drawing::new_metric();
        app
    }

    #[test]
    fn the_library_is_searched_by_family_and_by_words() {
        let all = library(&app());
        let n = all.len();
        assert_eq!(filtered(all.clone(), None, false, "").len(), n);
        let valves = filtered(all.clone(), Some(PartCategory::Valves), false, "");
        assert!(!valves.is_empty() && valves.iter().all(|m| m.category == PartCategory::Valves));
        // Words from the name, in either language, or the connections; all of them must be found.
        assert!(filtered(all.clone(), None, false, "balancing").iter().any(|m| m.name == "Balancing valve (TA)"));
        assert!(filtered(all.clone(), None, false, "papillon").iter().any(|m| m.name == "Butterfly valve"));
        let bs = filtered(all.clone(), None, false, "box 12 outlets");
        assert!(bs.len() >= 2 && bs.iter().all(|m| m.kind() == cadcraft_doc::PartKind::BranchBox && m.outlets() == 12));
        assert!(filtered(all.clone(), None, false, "zzzz").is_empty());
        assert!(filtered(all.clone(), Some(PartCategory::Pumps), false, "box").is_empty());
        assert!(filtered(all, None, true, "").is_empty(), "no model of its own yet");
    }

    #[test]
    fn a_part_kept_as_a_model_is_listed_under_my_models_and_can_be_deleted() {
        let mut app = app();
        let r = app.run("pipepartadd", json!({ "at": [0, 0], "model": "Check valve" })).unwrap();
        let h = cadcraft_doc::Handle::parse_hex(r["handle"].as_str().unwrap()).unwrap();
        app.session.set_selection(vec![h]);
        open(&mut app);
        app.part_lib.as_mut().unwrap().save_name = "Clapet maison".into();
        let r = app.run("pipepartmodels", json!({ "save": h.hex(), "name": "Clapet maison" }));
        assert!(r.is_ok());
        let mine = filtered(library(&app), None, true, "");
        assert_eq!(mine.len(), 1);
        assert!(mine[0].user && mine[0].name == "Clapet maison");
        assert!(app.run("pipepartmodels", json!({ "delete": "Clapet maison" })).is_ok());
        assert!(filtered(library(&app), None, true, "").is_empty());
    }

    #[test]
    fn dropping_a_part_on_the_drawing_puts_it_there_and_remembers_the_model() {
        let mut app = app();
        drop_part(&mut app, "Circulator (single)", cadcraft_geom::Vec2::new(1500.0, 800.0));
        // The orientation is asked: nothing is put until it is given (Enter takes the default).
        assert_eq!(app.session.doc().unwrap().model.iter().filter_map(|e| cadcraft_doc::part_of(e)).count(), 0);
        assert!(app.session.prompting());
        app.cmdline("");
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
