//! Dockable image reference manager. Image edits remain engine commands.

use egui::RichText;
use serde_json::{Value, json};

use crate::docking::Panel;
use crate::{CadApp, Pick};

pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !params.is_null() {
        return None;
    }
    if matches!(id, "image" | "im" | "imagemanager" | "image.manager") {
        return Some(crate::docking::command(app, false, &json!({"operation":"open", "panel": Panel::Images.id()})));
    }
    None
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Row {
    name: String,
    path: String,
    status: String,
    references: usize,
    pixels: Option<[u32; 2]>,
}

fn rows(app: &mut CadApp) -> Vec<Row> {
    let Ok(result) = app.session.execute("image", &json!({})) else { return Vec::new() };
    let Some(items) = result.get("images").and_then(Value::as_array) else { return Vec::new() };
    items
        .iter()
        .filter_map(|item| {
            Some(Row {
                name: item.get("name")?.as_str()?.to_string(),
                path: item.get("path")?.as_str()?.to_string(),
                status: item.get("status")?.as_str()?.to_string(),
                references: item.get("references").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).unwrap_or(0),
                pixels: item
                    .get("pixels")
                    .and_then(Value::as_array)
                    .and_then(|p| Some([u32::try_from(p.first()?.as_u64()?).ok()?, u32::try_from(p.get(1)?.as_u64()?).ok()?])),
            })
        })
        .collect()
}

pub fn panel(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    let rows = rows(app);
    let id = egui::Id::unique("image-manager-selected");
    let mut selected: Option<String> = ui.ctx().data_mut(|d| d.get_persisted(id));
    if !selected.as_ref().is_some_and(|s| rows.iter().any(|r| &r.name == s)) {
        selected = rows.first().map(|r| r.name.clone());
    }
    let handles: Vec<String> = app
        .session
        .doc()
        .ok()
        .and_then(|d| d.space(&app.session.space()))
        .map(|store| {
            store
                .iter()
                .filter_map(|e| match &e.kind {
                    cadcraft_engine::doc::EntityKind::Image(i) if selected.as_deref() == Some(i.name.as_str()) => Some(e.handle.hex()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    ui.horizontal(|ui| {
        if ui.button("Attach…").clicked() {
            if let Some(result) = app.pick_then(Pick::Image, |app, path| app.run("imageattach", json!({"path": path}))) {
                if let Err(error) = result {
                    app.set_status(error);
                }
            }
        }
        if ui.add_enabled(!handles.is_empty(), egui::Button::new("Reload")).clicked() {
            if let Some(handle) = handles.first() {
                if let Err(error) = app.run("imagereload", json!({"handle": handle})) {
                    app.set_status(error);
                }
            }
        }
        if ui.add_enabled(!handles.is_empty(), egui::Button::new("Detach")).clicked() {
            if let Err(error) = app.run("erase", json!({"handles": handles})) {
                app.set_status(error);
            }
        }
    });
    let search_id = egui::Id::unique("image-manager-search");
    let mut search: String = ui.ctx().data_mut(|d| d.get_persisted(search_id)).unwrap_or_default();
    ui.add(egui::TextEdit::singleline(&mut search).hint_text("Search images…"));
    ui.ctx().data_mut(|d| d.insert_persisted(search_id, search.clone()));
    let search = search.to_lowercase();
    ui.separator();
    egui::Grid::new("image-manager-list").striped(true).num_columns(4).show(ui, |ui| {
        for heading in ["Name", "Status", "References", "Pixels"] {
            ui.label(RichText::new(heading).strong());
        }
        ui.end_row();
        for row in &rows {
            if !search.is_empty() && !row.name.to_lowercase().contains(&search) && !row.path.to_lowercase().contains(&search) {
                continue;
            }
            if ui.selectable_label(selected.as_deref() == Some(row.name.as_str()), &row.name).clicked() {
                selected = Some(row.name.clone());
            }
            let color = if row.status == "Loaded" { t.text } else { t.warn };
            ui.label(RichText::new(&row.status).color(color));
            ui.label(row.references.to_string());
            ui.label(row.pixels.map_or("—".to_string(), |p| format!("{} × {}", p[0], p[1])));
            ui.end_row();
        }
    });
    ui.ctx().data_mut(|d| match &selected {
        Some(value) => d.insert_persisted(id, value.clone()),
        None => d.remove::<String>(id),
    });
    if let Some(row) = selected.and_then(|name| rows.into_iter().find(|r| r.name == name)) {
        ui.separator();
        ui.label(RichText::new("File reference").strong());
        ui.horizontal(|ui| {
            ui.label("Path:");
            ui.label(&row.path);
        });
        ui.horizontal(|ui| {
            ui.label("Status:");
            ui.label(&row.status);
        });
        ui.label(format!("{} reference(s)", row.references));
        let image = app.session.doc().ok().and_then(|d| d.space(&app.session.space())).and_then(|store| {
            store.iter().find_map(|e| match &e.kind {
                cadcraft_engine::doc::EntityKind::Image(i) if i.path == row.path => Some((e.handle, i.clone())),
                _ => None,
            })
        });
        if let Some((handle, image)) = image {
            let mut params = json!({"handle": handle.hex()});
            ui.collapsing("Placement", |ui| {
                let mut x = image.insert.x;
                let mut y = image.insert.y;
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("X:");
                    changed |= ui.add(egui::DragValue::new(&mut x)).changed();
                    ui.label("Y:");
                    changed |= ui.add(egui::DragValue::new(&mut y)).changed();
                });
                if changed {
                    params["insert"] = json!([x, y, image.insert.z]);
                }
                let mut width = image.size.x * image.u.xy().len();
                let mut height = image.size.y * image.v.xy().len();
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    if ui.add(egui::DragValue::new(&mut width).range(0.000001..=1e12)).changed() {
                        params["width"] = json!(width);
                    }
                    ui.label("Height:");
                    if ui.add(egui::DragValue::new(&mut height).range(0.000001..=1e12)).changed() {
                        params["height"] = json!(height);
                    }
                });
                let mut angle = image.u.y.atan2(image.u.x).to_degrees();
                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    if ui.add(egui::DragValue::new(&mut angle).suffix("°")).changed() {
                        params["rotation"] = json!(angle);
                    }
                });
            });
            ui.collapsing("Display", |ui| {
                for (key, label, value) in
                    [("brightness", "Brightness", image.brightness), ("contrast", "Contrast", image.contrast), ("fade", "Fade", image.fade)]
                {
                    let mut value = value;
                    if ui.add(egui::Slider::new(&mut value, 0..=100).text(label)).changed() {
                        params[key] = json!(value);
                    }
                }
                let mut transparency = image.display & 8 != 0;
                if ui.checkbox(&mut transparency, "Transparency").changed() {
                    params["transparency"] = json!(transparency);
                }
                let mut clipping = image.clipping;
                if ui.checkbox(&mut clipping, "Apply clip boundary").changed() {
                    params["clipping"] = json!(clipping);
                }
            });
            if params.as_object().is_some_and(|p| p.len() > 1) {
                if let Err(error) = app.run("imageedit", params) {
                    app.set_status(error);
                }
            }
        }
        ui.separator();
        ui.label(RichText::new("Plot / PDF").strong());
        ui.label("The image will be included when the source file is available.");
        let mut include = false;
        ui.add_enabled(false, egui::Checkbox::new(&mut include, "Include image in output"));
    } else {
        ui.label(RichText::new("No raster images are attached.").color(t.text_dim));
    }
}
