//! Dockable Text Style Manager. All document changes go through engine commands.
use crate::CadApp;
use egui::{RichText, TextEdit};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

mod fonts;
pub(crate) use fonts::initialize_fonts;
use fonts::{available_fonts, import_font};

fn initial_font_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into())).join("Fonts")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Fonts")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        PathBuf::from("/usr/share/fonts")
    }
}

fn font_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(), "ttf" | "otf" | "shx")
}

fn browse_entries(dir: &Path, filter: &str) -> Result<Vec<(String, PathBuf, bool)>, String> {
    let mut entries = Vec::new();
    // Bound listing work even when a folder contains many unrelated files.
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())?.take(10_000) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let is_dir = path.is_dir();
        if !is_dir && !font_file(&path) {
            continue;
        }
        let label = entry.file_name().to_string_lossy().into_owned();
        if !is_dir && !label.to_lowercase().contains(&filter.to_lowercase()) {
            continue;
        }
        entries.push((label, path, is_dir));
    }
    entries.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    Ok(entries)
}

pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !params.is_null() || !matches!(id, "style" | "st") {
        return None;
    }
    Some(crate::docking::command(app, false, &json!({"operation":"open","panel":"textStyles"})))
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct State {
    font_filter: String,
    selected: String,
    original: Option<cadcraft_doc::TextStyle>,
    new_name: String,
    font: String,
    height: String,
    width: String,
    oblique: String,
    backwards: bool,
    upside_down: bool,
    vertical: bool,
    annotative: bool,
    big_font: String,
    only_used: bool,
    message: String,
    picker_open: bool,
    picker_dir: String,
    picker_filter: String,
    picker_selected: String,
    picker_path_input: String,
}

fn sync(state: &mut State, style: &cadcraft_doc::TextStyle) {
    state.selected = style.name.clone();
    state.original = Some(style.clone());
    state.font = style.font.clone();
    state.height = style.height.to_string();
    state.width = style.width_factor.to_string();
    state.oblique = style.oblique.to_degrees().to_string();
    state.backwards = style.backwards;
    state.upside_down = style.upside_down;
    state.vertical = style.vertical;
    state.annotative = style.annotative;
    state.big_font = style.big_font.clone();
}

/// Preview using CADCraft's own text shaping and glyph triangulation.
fn draw_preview(ui: &mut egui::Ui, style: &cadcraft_doc::TextStyle) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(100.0), 76.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, crate::theme::Tokens::get().border), egui::StrokeKind::Inside);
    let font = cadcraft_fonts::TextFont::resolve_style(&style.font, &style.big_font, style.vertical);
    let params = cadcraft_fonts::TextParams {
        width_factor: style.width_factor.clamp(0.1, 10.0),
        oblique: style.oblique,
        backwards: style.backwards,
        upside_down: style.upside_down,
        ..cadcraft_fonts::TextParams::new(cadcraft_geom::Vec2::ZERO, 1.0)
    };
    let (sh, bb) = cadcraft_fonts::place(&font, "AaBb123", &params);
    if bb.is_empty() || bb.width() <= 0.0 || bb.height() <= 0.0 {
        return;
    }
    let inner = rect.shrink(10.0);
    let scale = (f64::from(inner.width()) / bb.width()).min(f64::from(inner.height()) / bb.height()).min(36.0);
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let center = bb.center();
    let pos = |p: cadcraft_geom::Vec2| {
        egui::pos2(inner.center().x + ((p.x - center.x) * scale) as f32, inner.center().y - ((p.y - center.y) * scale) as f32)
    };
    let color = crate::theme::Tokens::get().text;
    for line in &sh.strokes {
        for pair in line.windows(2) {
            if let [a, b] = pair {
                painter.line_segment([pos(*a), pos(*b)], egui::Stroke::new(1.3, color));
            }
        }
    }
    if !sh.glyphs.is_empty() {
        let mut mesh = egui::Mesh::default();
        for g in &sh.glyphs {
            let tris = cadcraft_render::triangulate_evenodd(g);
            for tri in tris.as_chunks::<3>().0 {
                let Ok(base) = u32::try_from(mesh.vertices.len()) else { break };
                for &pt in tri {
                    mesh.vertices.push(egui::epaint::Vertex { pos: pos(pt), uv: egui::epaint::WHITE_UV, color });
                }
                mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
        }
        if !mesh.indices.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

pub fn panel(app: &mut CadApp, ui: &mut egui::Ui) {
    let Ok(doc) = app.session.doc() else {
        ui.label("Open a drawing to manage text styles.");
        return;
    };
    let styles: Vec<cadcraft_doc::TextStyle> = doc.text_styles.to_vec();
    let current = doc.header.str("TEXTSTYLE", "Standard");
    let document_key = app.session.docs.get(app.session.active).map(|d| (d.uid, d.revision));
    let used_id = egui::Id::unique("text-style-manager-used-styles");
    type Used = (Option<(u64, u64)>, std::sync::Arc<std::collections::HashSet<String>>);
    let cached = ui.ctx().data(|data| data.get_temp::<Used>(used_id));
    let used = if let Some((_, used)) = cached.filter(|(key, _)| *key == document_key) {
        used
    } else {
        let used: std::sync::Arc<std::collections::HashSet<String>> =
            std::sync::Arc::new(cadcraft_engine::cmd::text_styles_in_use(doc).into_iter().map(|name| name.to_ascii_lowercase()).collect());
        ui.ctx().data_mut(|data| data.insert_temp(used_id, (document_key, used.clone())));
        used
    };
    let document = app.session.docs.get(app.session.active).map(|d| d.uid);
    let id = egui::Id::unique(("text-style-manager-state", document));
    let mut state = ui.ctx().data_mut(|d| d.get_persisted::<State>(id)).unwrap_or_default();
    let selected = styles
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&state.selected))
        .or_else(|| styles.iter().find(|s| s.name.eq_ignore_ascii_case(&current)))
        .or_else(|| styles.first());
    // External edits and Undo refresh the panel, while unsaved field edits stay intact.
    if let Some(style) = selected
        && state.original.as_ref() != Some(style)
    {
        sync(&mut state, style);
    }
    let mut commands: Vec<(&'static str, Value)> = Vec::new();
    egui::ScrollArea::vertical().id_salt("text-styles-content").show(ui, |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            style_list(ui, &mut state, &styles, &current, &used);
            style_actions(ui, &mut state, &styles, &current, &used, &mut commands);
            properties(ui, &mut state, &mut commands, !app.services.no_file_system);
            preview(ui, &state, &styles);
        });
    });
    font_browser(ui, &mut state);
    for (cmd, args) in commands {
        match app.run(cmd, args) {
            Err(err) => state.message = err,
            Ok(_) => {
                state.message.clear();
                if let Some(style) = app.session.doc().ok().and_then(|d| d.text_style(&state.selected)) {
                    sync(&mut state, style);
                }
            }
        }
    }
    ui.ctx().data_mut(|d| d.insert_persisted(id, state));
}

type Commands = Vec<(&'static str, Value)>;

fn style_list(ui: &mut egui::Ui, state: &mut State, styles: &[cadcraft_doc::TextStyle], current: &str, used: &std::collections::HashSet<String>) {
    ui.horizontal(|ui| {
        ui.heading("Text styles");
        ui.label(format!("Current: {current}"));
    });
    ui.separator();
    let styles_label = ui
        .horizontal(|ui| {
            let label = ui.label(RichText::new("Styles").strong());
            ui.checkbox(&mut state.only_used, "In use only");
            label.id
        })
        .inner;
    let chosen = state.selected.clone();
    craft_ui::ComboBox::from_id_salt("text-style-select")
        .selected_text(if chosen.is_empty() { "Select style".to_string() } else { chosen })
        .width(ui.available_width().min(320.0))
        .show_ui(ui, |ui| {
            for style in styles {
                if state.only_used
                    && !used.contains(&style.name.to_ascii_lowercase())
                    && !style.name.eq_ignore_ascii_case(current)
                    && !style.name.eq_ignore_ascii_case(&state.selected)
                {
                    continue;
                }
                let label = if style.name.eq_ignore_ascii_case(current) { format!("✓ {}", style.name) } else { style.name.clone() };
                if ui.selectable_label(state.selected == style.name, label).clicked() {
                    sync(state, style);
                }
            }
        })
        .response
        .labelled_by(styles_label);
}

fn style_actions(
    ui: &mut egui::Ui,
    state: &mut State,
    styles: &[cadcraft_doc::TextStyle],
    current: &str,
    used: &std::collections::HashSet<String>,
    commands: &mut Commands,
) {
    ui.horizontal_wrapped(|ui| {
        let label = ui.label("Style name");
        let name_response = ui.add(TextEdit::singleline(&mut state.new_name).hint_text("Style name").desired_width(175.0)).labelled_by(label.id);
        let submit_name = name_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Create").clicked() || submit_name {
            let name = state.new_name.trim().to_string();
            if name.is_empty() {
                state.message = "Enter a style name.".into();
            } else if styles.iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
                state.message = format!("Style '{name}' already exists.");
            } else {
                commands.push(("style", json!({"name":name,"current":false})));
                state.selected = name;
                state.new_name.clear();
                state.message.clear();
            }
        }
        if ui.add_enabled(!state.selected.is_empty() && !state.selected.eq_ignore_ascii_case(current), egui::Button::new("Set current")).clicked() {
            commands.push(("style.current", json!({"name":state.selected})));
        }
        if ui
            .add_enabled(
                !state.selected.eq_ignore_ascii_case("Standard")
                    && !state.selected.eq_ignore_ascii_case(current)
                    && !used.contains(&state.selected.to_ascii_lowercase()),
                egui::Button::new("Delete"),
            )
            .clicked()
        {
            commands.push(("style.delete", json!({"name":state.selected})));
            state.selected.clear();
        }
        let rename_to = state.new_name.trim().to_string();
        if ui
            .add_enabled(
                !state.selected.eq_ignore_ascii_case("Standard")
                    && !rename_to.is_empty()
                    && !styles.iter().any(|s| s.name.eq_ignore_ascii_case(&rename_to)),
                egui::Button::new("Rename"),
            )
            .clicked()
        {
            commands.push(("style.rename", json!({"from":state.selected,"to":rename_to})));
            state.selected = rename_to;
            state.new_name.clear();
        }
    });
}

fn font_variant(ui: &mut egui::Ui, state: &mut State) {
    let label = ui.label("Font style");
    let description = cadcraft_fonts::ttf::describe_font(&state.font);
    ui.add_enabled_ui(description.is_some(), |ui| {
        let (family, selected, variants) = description.unwrap_or_default();
        craft_ui::ComboBox::from_id_salt("text-style-font-variant")
            .selected_text(if selected.is_empty() { "—" } else { &selected })
            .width(240.0)
            .show_ui(ui, |ui| {
                for variant in variants {
                    if ui.selectable_label(variant == selected, &variant).clicked() {
                        state.font = cadcraft_fonts::ttf::compose_font(&family, &variant);
                    }
                }
            })
            .response
            .labelled_by(label.id);
    });
    ui.end_row();
}

fn properties(ui: &mut egui::Ui, state: &mut State, commands: &mut Commands, allow_import: bool) {
    ui.separator();
    ui.label(RichText::new("Properties").strong());
    let description = cadcraft_fonts::ttf::describe_font(&state.font);
    let display_font = description.as_ref().map(|d| d.0.clone()).unwrap_or_else(|| state.font.clone());
    let fonts = available_fonts(ui.ctx());
    egui::Grid::new("text-style-properties").num_columns(2).show(ui, |ui| {
        let font_label = ui.label("Font");
        craft_ui::ComboBox::from_id_salt("text-style-font-select")
            .selected_text(&display_font)
            .width(240.0)
            .show_ui(ui, |ui| {
                if ui.add_enabled(allow_import && !cfg!(target_arch = "wasm32"), egui::Button::new("Import font…")).clicked() {
                    state.picker_open = true;
                    if state.picker_dir.is_empty() {
                        state.picker_dir = initial_font_dir().to_string_lossy().into_owned();
                    }
                    state.picker_path_input = state.picker_dir.clone();
                    state.picker_selected.clear();
                    ui.close();
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    let filter_label = ui.label("Filter fonts");
                    ui.add(TextEdit::singleline(&mut state.font_filter)).labelled_by(filter_label.id);
                    for name in fonts.iter().filter(|name| name.to_lowercase().contains(&state.font_filter.to_lowercase())) {
                        if ui.selectable_label(display_font.eq_ignore_ascii_case(name), name).clicked() {
                            let variant = description.as_ref().map_or("", |d| d.1.as_str());
                            let chosen = cadcraft_fonts::ttf::describe_font(name);
                            let variant = chosen.as_ref().filter(|d| d.2.iter().any(|s| s == variant)).map_or("", |_| variant);
                            state.font = cadcraft_fonts::ttf::compose_font(name, variant);
                        }
                    }
                });
            })
            .response
            .labelled_by(font_label.id);
        ui.end_row();
        font_variant(ui, state);
        property_input(ui, "Height (0 = variable)", &mut state.height, 100.0);
        property_input(ui, "Width factor", &mut state.width, 100.0);
        property_input(ui, "Oblique angle (degrees)", &mut state.oblique, 100.0);
        property_input(ui, "Big Font (SHX)", &mut state.big_font, 180.0);
    });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut state.annotative, "Annotative");
        ui.checkbox(&mut state.backwards, "Backwards");
        ui.checkbox(&mut state.upside_down, "Upside down");
        ui.checkbox(&mut state.vertical, "Vertical");
    });
    if ui.button("Apply properties").clicked() && !state.selected.is_empty() {
        let mut props = json!({"name":state.selected,"font":state.font,"current":false});
        if let Ok(v) = state.height.parse::<f64>()
            && v >= 0.0
            && v.is_finite()
        {
            props["height"] = json!(v);
        }
        if let Ok(v) = state.width.parse::<f64>()
            && v > 0.0
            && v.is_finite()
        {
            props["widthFactor"] = json!(v);
        }
        if let Ok(v) = state.oblique.parse::<f64>()
            && v.is_finite()
            && v.abs() <= 85.0
        {
            props["oblique"] = json!(v);
        }
        props["backwards"] = json!(state.backwards);
        props["upsideDown"] = json!(state.upside_down);
        props["vertical"] = json!(state.vertical);
        props["annotative"] = json!(state.annotative);
        props["bigFont"] = json!(state.big_font);
        commands.push(("style", props));
    }
}

fn preview(ui: &mut egui::Ui, state: &State, styles: &[cadcraft_doc::TextStyle]) {
    if !state.message.is_empty() {
        ui.label(&state.message);
    }
    ui.separator();
    ui.label("Preview (CADCraft renderer)");
    if let Some(style) = styles.iter().find(|s| s.name.eq_ignore_ascii_case(&state.selected)) {
        let mut preview = style.clone();
        preview.font = state.font.clone();
        preview.big_font = state.big_font.clone();
        preview.vertical = state.vertical;
        if let Ok(w) = state.width.parse::<f64>()
            && w > 0.0
            && w.is_finite()
        {
            preview.width_factor = w;
        }
        if let Ok(angle) = state.oblique.parse::<f64>()
            && angle.is_finite()
            && angle.abs() <= 85.0
        {
            preview.oblique = angle.to_radians();
        }
        preview.backwards = state.backwards;
        preview.upside_down = state.upside_down;
        draw_preview(ui, &preview);
    }
}

fn font_browser(ui: &mut egui::Ui, state: &mut State) {
    if !state.picker_open {
        return;
    }
    let mut open = true;
    let mut selected_font: Option<PathBuf> = None;
    let mut navigate: Option<PathBuf> = None;
    let mut cancel = false;
    egui::Window::new("Import font")
        .id(ui.make_persistent_id("text-style-font-browser"))
        .open(&mut open)
        .default_width(540.0)
        .default_height(420.0)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            ui.label("Select a TTF, OTF or SHX font file");
            ui.horizontal(|ui| {
                if ui.button("Up").clicked()
                    && let Some(parent) = Path::new(&state.picker_dir).parent()
                {
                    navigate = Some(parent.to_path_buf());
                }
                let label = ui.label("Folder");
                let response = ui.add(TextEdit::singleline(&mut state.picker_path_input).desired_width(350.0)).labelled_by(label.id);
                if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Go").clicked() {
                    let p = PathBuf::from(state.picker_path_input.trim());
                    if p.is_dir() {
                        navigate = Some(p);
                    } else {
                        state.message = "Folder not found".into();
                    }
                }
            });
            ui.horizontal(|ui| {
                let label = ui.label("Filter");
                ui.add(TextEdit::singleline(&mut state.picker_filter).hint_text("Font name")).labelled_by(label.id);
            });
            match browse_entries(Path::new(&state.picker_dir), &state.picker_filter) {
                Ok(entries) => {
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for (label, path, is_dir) in entries {
                            let text = if is_dir { format!("[Folder] {label}") } else { label };
                            let selected = state.picker_selected == path.to_string_lossy();
                            let response = ui.selectable_label(selected, text);
                            if response.clicked() {
                                if is_dir {
                                    navigate = Some(path.clone());
                                } else {
                                    state.picker_selected = path.to_string_lossy().into_owned();
                                }
                            }
                            if response.double_clicked() && !is_dir {
                                selected_font = Some(path);
                            }
                        }
                    });
                }
                Err(err) => {
                    ui.label(format!("Unable to list folder: {err}"));
                }
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(!state.picker_selected.is_empty(), egui::Button::new("Import selected")).clicked() {
                    selected_font = Some(PathBuf::from(&state.picker_selected));
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if let Some(path) = navigate {
        state.picker_dir = path.to_string_lossy().into_owned();
        state.picker_path_input = state.picker_dir.clone();
        state.picker_selected.clear();
    }
    if let Some(path) = selected_font {
        match import_font(&path.to_string_lossy()) {
            Ok(imported) => {
                state.font = imported;
                state.message = "Font imported. Click Apply properties to assign it.".into();
            }
            Err(err) => {
                state.message = err;
            }
        }
        state.picker_open = false;
    }
    if !open || cancel {
        state.picker_open = false;
    }
}

fn property_input(ui: &mut egui::Ui, label: &str, value: &mut String, width: f32) {
    let label = ui.label(label);
    ui.add(TextEdit::singleline(value).desired_width(width)).labelled_by(label.id);
    ui.end_row();
}
