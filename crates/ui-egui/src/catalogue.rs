//! Optional catalogue view over freemans32's parametric furniture and equipment recipes.
//! The Blocks dock owns the window; this module adds sources and categories, not another host.
use crate::CadApp;
use cadcraft_doc::{
    EquipmentKind, EquipmentParams, FurnitureParams, Handle, HvacUnits, equipment_of, furniture_groups, furniture_models, furniture_of,
};
use egui::{Sense, vec2};
use serde_json::json;

pub(crate) fn command(id: &str) -> bool {
    id.starts_with("furniture") || id.starts_with("equipment")
}

pub(crate) fn visible(app: &CadApp, id: &str) -> bool {
    (app.ui.catalogue_tools || !command(id)) && (app.ui.architecture_tools || !crate::architecture::command(id))
}

#[derive(Clone, Default)]
struct State {
    equipment: bool,
    architecture: bool,
    view: Option<crate::blocks_palette::View>,
    filter: String,
    furniture_group: Categories,
    equipment_group: Categories,
    architecture_group: Categories,
    architecture_view: Option<crate::blocks_palette::View>,
    architecture_filter: String,
    chosen: String,
    error: Option<String>,
    editor: Option<(Handle, Editor)>,
    saved_model: String,
}
/// `None` selects the first category on first use; an empty set stays empty.
#[derive(Clone, Debug, Default, Hash)]
pub(crate) struct Categories(Option<std::collections::BTreeSet<String>>);
impl Categories {
    pub(crate) fn contains(&self, group: &str) -> bool {
        self.0.as_ref().is_some_and(|active| active.contains(group))
    }
    fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|active| active.is_empty())
    }
}
#[derive(Clone)]
enum Editor {
    Furniture(FurnitureParams),
    Equipment(Box<EquipmentParams>),
}

pub(crate) fn panel(app: &mut CadApp, ui: &mut egui::Ui) {
    let id = egui::Id::unique("parametric-catalogue-state");
    let mut st = ui.data(|d| d.get_temp::<State>(id)).unwrap_or_default();
    if !app.ui.catalogue_tools {
        st.architecture = true;
    }
    if !app.ui.architecture_tools {
        st.architecture = false;
    }
    if !(app.ui.catalogue_tools || app.ui.architecture_tools) {
        egui::Frame::NONE.inner_margin(8.0).show(ui, |ui| {
            ui.label(crate::tl!("Enable trade libraries from Library settings."));
        });
        return;
    }
    let panel_height = ui.available_height();
    let footer_id = id.with("footer-height");
    let footer_height = ui.data(|d| d.get_temp::<f32>(footer_id)).unwrap_or(140.0);
    egui::ScrollArea::vertical().id_salt("catalogue-body").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::NONE.inner_margin(8.0).show(ui, |ui| {
            let content_top = ui.cursor().top();
            ui.horizontal_wrapped(|ui| {
                for (equipment, label) in [(false, "Furniture"), (true, "Equipment")] {
                    if app.ui.catalogue_tools && ui.selectable_label(!st.architecture && st.equipment == equipment, label).clicked() {
                        st.architecture = false;
                        st.equipment = equipment;
                        st.chosen.clear();
                    }
                }
                if app.ui.architecture_tools && ui.selectable_label(st.architecture, crate::tl!("Architecture")).clicked() {
                    st.architecture = true;
                }
                ui.menu_button("?", |ui| help(ui, st.architecture)).response.on_hover_ui(|ui| help(ui, st.architecture));
            });
            if st.architecture {
                let view = st.architecture_view.get_or_insert(crate::blocks_palette::View::Medium);
                crate::blocks_palette::filter_view(ui, &mut st.architecture_filter, view, "architecture-library-view");
                let view = *view;
                let header_height = ui.cursor().top() - content_top;
                let height = (panel_height - header_height - 16.0).max(80.0);
                ui.horizontal_top(|ui| {
                    categories(ui, height, &crate::architecture::GROUPS, &mut st.architecture_group, false);
                    ui.vertical(|ui| crate::architecture::panel(app, ui, &st.architecture_group, view, &st.architecture_filter, height));
                });
                return;
            }
            let view = st.view.get_or_insert(crate::blocks_palette::View::Details);
            crate::blocks_palette::filter_view(ui, &mut st.filter, view, "trade-library-view");
            let view = *view;
            let equipment = app.session.doc().map(|d| d.available_equipment_models()).unwrap_or_default();
            let groups: Vec<&str> = if st.equipment { EquipmentKind::ALL.iter().map(|k| k.name()).collect() } else { furniture_groups().to_vec() };
            // Give the gallery the remaining viewport after the actual header and properties.
            // The outer scroll area keeps properties reachable in very small windows.
            let header_height = ui.cursor().top() - content_top;
            let height = (panel_height - header_height - footer_height - 16.0).max(80.0);
            let mut insert = None;
            let mut group = if st.equipment { st.equipment_group.clone() } else { st.furniture_group.clone() };
            ui.horizontal_top(|ui| {
                categories(ui, height, &groups, &mut group, st.equipment);
                egui::ScrollArea::vertical()
                    .id_salt(("catalogue-gallery", st.equipment, &group))
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let layout = if view.tile().is_some() {
                            egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true)
                        } else {
                            egui::Layout::top_down(egui::Align::Min)
                        };
                        ui.allocate_ui_with_layout(vec2(ui.available_width(), ui.spacing().interact_size.y), layout, |ui| {
                            let filter = st.filter.to_lowercase();
                            let matches = if st.equipment {
                                equipment.iter().any(|m| group.contains(m.kind.name()) && m.name.to_lowercase().contains(&filter))
                            } else {
                                furniture_models()
                                    .iter()
                                    .any(|m| group.contains(m.group) && (m.label.to_lowercase().contains(&filter) || m.id.contains(&filter)))
                            };
                            if group.is_empty() {
                                ui.label(crate::tl!("Select one or more categories."));
                            } else if !matches {
                                ui.label(crate::tl!("No block matches the filter."));
                            }
                            if st.equipment {
                                for m in &equipment {
                                    if !group.contains(m.kind.name()) || !m.name.to_lowercase().contains(&filter) {
                                        continue;
                                    }
                                    let mut params = EquipmentParams::from_model(m, "Preview", HvacUnits::default());
                                    params.show_size = false;
                                    let recipe = serde_json::to_string(&params).unwrap_or_default();
                                    let size = format!("{} × {} × {} mm", m.length_mm, m.width_mm, m.height_mm);
                                    if tile(ui, view, &m.name, &size, st.chosen == m.name, &recipe, || {
                                        params
                                            .generate()
                                            .unwrap_or_default()
                                            .into_iter()
                                            .filter(|p| !matches!(p.kind, cadcraft_doc::EntityKind::Text(_)))
                                            .collect()
                                    }) {
                                        st.chosen = m.name.clone();
                                        insert = Some(m.name.clone());
                                    }
                                }
                            } else {
                                for m in furniture_models() {
                                    if !group.contains(m.group) || !(m.label.to_lowercase().contains(&filter) || m.id.contains(&filter)) {
                                        continue;
                                    }
                                    let size = format!("{} × {} × {} mm", m.width_mm, m.depth_mm, m.height_mm);
                                    let selected = st.chosen == m.id;
                                    if tile(ui, view, m.label, &size, selected, m.id, || {
                                        FurnitureParams::new(m, HvacUnits::default()).generate().unwrap_or_default()
                                    }) {
                                        st.chosen = m.id.to_string();
                                        insert = Some(m.id.to_string());
                                    }
                                }
                            }
                        });
                    });
            });
            if st.equipment {
                st.equipment_group = group;
            } else {
                st.furniture_group = group;
            }
            let footer_top = ui.cursor().top();
            if let Some(model) = insert {
                app.cmd.buffer.clear();
                if st.equipment {
                    if let Some(m) = equipment.iter().find(|m| m.name == model) {
                        app.session.last_used.equipment.kind = m.kind;
                        app.session.last_used.equipment.model = model;
                        app.start("equipmentadd");
                    }
                } else {
                    app.session.last_used.furniture.model = model;
                    app.start("furnitureadd");
                }
            }
            editor(app, ui, &mut st);
            if let Some(error) = &st.error {
                ui.colored_label(crate::theme::Tokens::get().warn, error);
            }
            let measured = ui.cursor().top() - footer_top;
            ui.data_mut(|d| d.insert_temp(footer_id, measured));
            if (measured - footer_height).abs() > 0.5 {
                ui.ctx().request_repaint();
            }
        });
    });
    ui.data_mut(|d| d.insert_temp(id, st));
}

/// Show the instructions themselves, both on hover and when the help menu is opened.
fn help(ui: &mut egui::Ui, architecture: bool) {
    ui.set_max_width(420.0);
    if architecture {
        ui.label(crate::tl!("Place doors and windows on a wall; the wall is cut automatically."));
        ui.label(crate::tl!("Elevations are 2D snapshots of walls, openings, furniture and equipment. Regenerate them after changing the plan."));
        ui.label(crate::tl!("Wall and opening recipes: freemans32/cadcraft (MIT OR Apache-2.0)."));
    } else {
        ui.label(crate::tl!("Click a model to place it. Tab changes the insertion anchor; the next click sets rotation. Esc ends placement."));
        ui.label(crate::tl!("Select one catalogue object to edit its dimensions."));
        ui.label("Catalogue recipes: freemans32/cadcraft (MIT OR Apache-2.0).");
    }
}

/// Category navigation remains visible while filtering or scrolling the models on the right.
fn categories(ui: &mut egui::Ui, height: f32, groups: &[&str], selected: &mut Categories, equipment: bool) {
    let active = selected.0.get_or_insert_with(|| groups.first().map(|group| (*group).to_owned()).into_iter().collect());
    active.retain(|group| groups.contains(&group.as_str()));
    let width = (ui.available_width() * 0.28).clamp(72.0, 150.0);
    egui::ScrollArea::vertical().id_salt(("trade-categories", equipment, groups.first())).max_height(height).show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(width);
            for group in groups {
                let label = if equipment { equipment_group_label(group) } else { crate::i18n::t(group) };
                let enabled = active.contains(*group);
                if ui.selectable_label(enabled, label).clicked() {
                    if enabled {
                        active.remove(*group);
                    } else {
                        active.insert((*group).to_owned());
                    }
                }
            }
        });
    });
    let (divider, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter().line_segment([divider.left_top(), divider.left_bottom()], ui.visuals().widgets.noninteractive.bg_stroke);
}

pub(crate) fn tile(
    ui: &mut egui::Ui,
    view: crate::blocks_palette::View,
    name: &str,
    size: &str,
    selected: bool,
    recipe: &str,
    pieces: impl FnOnce() -> Vec<cadcraft_doc::HvacPiece>,
) -> bool {
    use crate::blocks_palette::View;
    let mut clicked = false;
    let side = view.tile().map(|side| side.min(ui.max_rect().width().max(1.0)));
    // Nested tile UIs cannot advertise their size to the wrapping parent in advance.
    if let Some(side) = side
        && ui.available_size_before_wrap().x < side
    {
        ui.end_row();
    }
    ui.push_id(recipe, |ui| {
        if let Some(side) = side {
            ui.vertical(|ui| {
                ui.set_width(side);
                let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click());
                if ui.is_rect_visible(rect) {
                    crate::blocks_palette::catalogue_preview(ui, rect.shrink(4.0), recipe, pieces());
                }
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), format!("Place {name}")));
                clicked |= response.on_hover_text(format!("{name}\n{size}")).clicked();
                clicked |= ui.selectable_label(selected, name).clicked();
            });
        } else {
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 0.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if view == View::Details {
                    let (rect, response) = ui.allocate_exact_size(vec2(54.0, 32.0), Sense::click());
                    if ui.is_rect_visible(rect) {
                        crate::blocks_palette::catalogue_preview(ui, rect.shrink(3.0), recipe, pieces());
                    }
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), format!("Place {name}")));
                    clicked |= response.clicked();
                }
                clicked |= ui.selectable_label(selected, name).on_hover_text(size).clicked();
            });
        }
    });
    clicked
}

fn dimension(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.horizontal(|ui| {
        let l = ui.label(label);
        ui.add(egui::DragValue::new(value).speed(10.0).suffix(" mm")).labelled_by(l.id);
    });
}
fn editor(app: &mut CadApp, ui: &mut egui::Ui, st: &mut State) {
    let selected = app.session.selection();
    let next = if let [h] = selected.as_slice() {
        app.session
            .doc()
            .ok()
            .and_then(|d| d.entity(*h))
            .and_then(|e| furniture_of(e).map(Editor::Furniture).or_else(|| equipment_of(e).map(|p| Editor::Equipment(Box::new(p)))))
            .map(|p| (*h, p))
    } else {
        None
    };
    // Refresh from the document (including undo/redo) unless the user is currently editing.
    let current = next.as_ref().map(|(h, p)| {
        (
            *h,
            match p {
                Editor::Furniture(p) => serde_json::to_string(p),
                Editor::Equipment(p) => serde_json::to_string(p),
            },
        )
    });
    let seen_id = egui::Id::unique("catalogue-editor-version");
    let previous = ui.data(|d| d.get_temp::<String>(seen_id)).unwrap_or_default();
    let version = format!("{current:?}");
    if version != previous {
        st.editor = next;
        ui.data_mut(|d| d.insert_temp(seen_id, version));
    }
    let Some((h, params)) = st.editor.as_mut() else {
        return;
    };
    ui.label("Selected object");
    let (command, mut values) = match &mut *params {
        Editor::Furniture(p) => {
            let l = ui.label("Name");
            ui.text_edit_singleline(&mut p.name).labelled_by(l.id);
            dimension(ui, "Width", &mut p.width_mm);
            dimension(ui, "Depth", &mut p.depth_mm);
            dimension(ui, "Height", &mut p.height_mm);
            optional_number(ui, "Altitude", &mut p.bottom_mm, " mm");
            optional_number(ui, "Power", &mut p.power_w, " W");
            dimension(ui, "Label height", &mut p.label_height_mm);
            (
                "furnitureedit",
                json!({"name":p.name,"width":p.width_mm,"depth":p.depth_mm,"height":p.height_mm,"altitude":p.bottom_mm.map(|v|v/1000.0),"power":p.power_w,"labelHeight":p.label_height_mm}),
            )
        }
        Editor::Equipment(p) => {
            let l = ui.label("Name");
            ui.text_edit_singleline(&mut p.name).labelled_by(l.id);
            dimension(ui, "Length", &mut p.length_mm);
            dimension(ui, "Width", &mut p.width_mm);
            dimension(ui, "Height", &mut p.height_mm);
            ui.checkbox(&mut p.show_size, "Show dimensions in drawing");
            optional_number(ui, "Altitude", &mut p.bottom_mm, " mm");
            egui::CollapsingHeader::new("Technical data").show(ui, |ui| {
                for (label, value) in
                    [("Reference", &mut p.reference), ("Connections", &mut p.connections), ("Circuit", &mut p.circuit), ("Address", &mut p.address)]
                {
                    let l = ui.label(label);
                    ui.text_edit_singleline(value).labelled_by(l.id);
                }
                optional_number(ui, "Cooling capacity", &mut p.capacity_kw, " kW");
                optional_number(ui, "Heating capacity", &mut p.heating_kw, " kW");
                optional_number(ui, "Electrical power", &mut p.electric_kw, " kW");
                optional_number(ui, "Weight", &mut p.weight_kg, " kg");
                dimension(ui, "Service clearance", &mut p.clearance_mm);
                dimension(ui, "Label height", &mut p.label_height_mm);
            });
            (
                "equipmentedit",
                json!({"name":p.name,"length":p.length_mm,"width":p.width_mm,"height":p.height_mm,"showSize":p.show_size,
                "altitude":p.bottom_mm.map(|v|v/1000.0),"reference":p.reference,"connections":p.connections,"circuit":p.circuit,"address":p.address,
                "capacityKw":p.capacity_kw,"heatingKw":p.heating_kw,"electricKw":p.electric_kw,"weight":p.weight_kg,"clearance":p.clearance_mm,"labelHeight":p.label_height_mm}),
            )
        }
    };
    values["handle"] = json!(h.hex());
    if ui.button("Apply changes").clicked() {
        st.error = app.run(command, values).err();
    }
    if matches!(params, Editor::Equipment(_)) {
        ui.horizontal(|ui| {
            let l = ui.label("Model name");
            ui.text_edit_singleline(&mut st.saved_model).labelled_by(l.id);
            if ui.button("Save model in drawing").clicked() {
                st.error = app.run("equipmentmodelsave", json!({"handle":h.hex(),"name":st.saved_model})).err();
            }
        });
    }
}

fn optional_number(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>, unit: &str) {
    ui.horizontal(|ui| {
        let mut enabled = value.is_some();
        let response = ui.checkbox(&mut enabled, label);
        if enabled {
            let n = value.get_or_insert(1.0);
            ui.add(egui::DragValue::new(n).speed(0.1).suffix(unit)).labelled_by(response.id);
        } else {
            *value = None;
        }
    });
}

fn equipment_group_label(group: &str) -> &str {
    match group {
        "chiller" => "Chillers",
        "vrfOutdoor" => "VRF outdoor units",
        "splitOutdoor" => "Split outdoor units",
        "cassette" => "Ceiling cassettes",
        "ducted" => "Ducted units",
        "wallUnit" => "Wall units",
        "ceilingUnit" => "Ceiling units",
        "floorUnit" => "Floor units",
        "serverCooler" => "Server room coolers",
        "dehumidifier" => "Ducted dehumidifiers",
        "dehumidifierWall" => "Wall dehumidifiers",
        "wallRack" => "Wall racks",
        "floorRack" => "Floor racks",
        other => other,
    }
}
