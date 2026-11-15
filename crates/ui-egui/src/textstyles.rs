//! Dockable Text Style Manager. All document changes go through engine commands.
use egui::{RichText, TextEdit};
use serde_json::{Value, json};
use crate::CadApp;
use std::path::{Path, PathBuf};

fn initial_font_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    { return PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts"); }
    #[cfg(target_os = "macos")]
    { return PathBuf::from("/Library/Fonts"); }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { return PathBuf::from("/usr/share/fonts"); }
}

fn font_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(), "ttf" | "otf" | "shx")
}

fn browse_entries(dir: &Path, filter: &str) -> Result<Vec<(String, PathBuf, bool)>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let is_dir = path.is_dir();
        if !is_dir && !font_file(&path) { continue; }
        let label = entry.file_name().to_string_lossy().into_owned();
        if !is_dir && !label.to_lowercase().contains(&filter.to_lowercase()) { continue; }
        entries.push((label, path, is_dir));
    }
    entries.sort_by(|a,b| b.2.cmp(&a.2).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    Ok(entries)
}

pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !params.is_null() || !matches!(id, "style" | "st") { return None; }
    Some(crate::docking::command(app, false, &json!({"operation":"open","panel":"textStyles"})))
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
struct State {
    selected: String,
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

/// Import a font into an application-managed folder without installing it in Windows.
/// The font path is stored in the text style and is preserved in DXF/DWG references.
fn import_font(path: &str) -> Result<String, String> {
    let src = std::path::Path::new(path.trim().trim_matches('"'));
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !matches!(ext.as_str(), "ttf" | "otf" | "shx") { return Err("Choose a .ttf, .otf or .shx file".into()); }
    if !src.is_file() { return Err("Font file not found".into()); }
    let name = src.file_name().ok_or("Invalid font filename")?;
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support").into_os_string())
    } else {
        std::env::var_os("XDG_DATA_HOME").or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share").into_os_string()))
    }.ok_or("Application data directory unavailable")?;
    let dest_dir = PathBuf::from(base).join("CADCraft").join("fonts");
    std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let dest = dest_dir.join(name);
    if src != dest { std::fs::copy(src, &dest).map_err(|e| e.to_string())?; }
    Ok(dest.to_string_lossy().to_string())
}

/// Match the native MTEXT font choices; do not enumerate Windows Fonts.
/// Extra fonts appear only after explicit import into CADCraft's font directory.
fn available_fonts() -> Vec<(String, String)> {
    let mut fonts: Vec<(String, String)> = [
        "CADCraft Stroke", "Arial", "Times New Roman", "Courier New",
        "Verdana", "Georgia", "Calibri", "Cheboygan",
    ].iter().map(|name| ((*name).to_string(), (*name).to_string())).collect();
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    if let Some(base) = base {
        let dir = base.join("CADCraft").join("fonts");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !font_file(&path) { continue; }
                let label = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                fonts.push((label, path.to_string_lossy().into_owned()));
            }
        }
    }
    fonts
}

/// Detect styles referenced by the drawing, including blocks and annotation styles.
fn used_style_names(d: &cadcraft_doc::Drawing) -> std::collections::HashSet<String> {
    use cadcraft_doc::EntityKind;
    let mut names = std::collections::HashSet::new();
    let mut add = |s: &str| { if !s.is_empty() { names.insert(s.to_ascii_lowercase()); } };
    for s in &d.dim_styles { add(&s.text_style); }
    for s in &d.mleader_styles { add(&s.text_style); }
    for s in &d.table_styles { for c in &s.cell_styles { add(&c.text_style); } }
    for lt in &d.linetypes { for el in &lt.pattern { if let Some(st) = &el.style { add(st); } } }
    let stores = std::iter::once(&d.model).chain(d.layouts.iter().map(|l| &l.entities)).chain(d.blocks.values().map(|b| &b.entities));
    for store in stores { for e in store.iter() {
        match &e.kind {
            EntityKind::Text(t) => add(&t.style),
            EntityKind::MText(t) => add(&t.style),
            EntityKind::AttDef(a) => add(&a.text.style),
            EntityKind::Insert(i) => { for a in &i.attribs { add(&a.text.style); } },
            EntityKind::MLeader(m) => { if let Some(t) = &m.text { add(&t.style); } },
            EntityKind::Dimension(dm) => {
                for (k,v) in &dm.overrides {
                    if cadcraft_doc::DimStyle::field_name(k) == Some("textStyle") {
                        if let Some(name) = v.as_str() { add(name); }
                    }
                }
            },
            _ => {},
        }
    } }
    names
}

/// Preview using CADCraft's own text shaping and glyph triangulation.
fn draw_preview(ui: &mut egui::Ui, style: &cadcraft_doc::TextStyle) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(100.0), 76.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color), egui::StrokeKind::Inside);
    let font = cadcraft_fonts::TextFont::resolve(&style.font);
    let params = cadcraft_fonts::TextParams {
        width_factor: style.width_factor.clamp(0.1, 10.0),
        oblique: style.oblique,
        backwards: style.backwards,
        upside_down: style.upside_down,
        ..cadcraft_fonts::TextParams::new(cadcraft_geom::Vec2::ZERO, 1.0)
    };
    let (sh, bb) = cadcraft_fonts::place(&font, "AaBb123", &params);
    if bb.is_empty() || bb.width() <= 0.0 || bb.height() <= 0.0 { return; }
    let inner = rect.shrink(10.0);
    let scale = (f64::from(inner.width()) / bb.width()).min(f64::from(inner.height()) / bb.height()).min(36.0);
    if !scale.is_finite() || scale <= 0.0 { return; }
    let center = bb.center();
    let pos = |p: cadcraft_geom::Vec2| egui::pos2(inner.center().x + ((p.x-center.x)*scale) as f32, inner.center().y - ((p.y-center.y)*scale) as f32);
    let color = ui.visuals().text_color();
    for line in &sh.strokes { for pair in line.windows(2) {
        if let [a,b] = pair { painter.line_segment([pos(*a),pos(*b)], egui::Stroke::new(1.3,color)); }
    } }
    if !sh.glyphs.is_empty() {
        let mut mesh = egui::Mesh::default();
        for g in &sh.glyphs { let tris = cadcraft_render::triangulate_evenodd(g);
            for tri in tris.as_chunks::<3>().0 {
                let Ok(base) = u32::try_from(mesh.vertices.len()) else { break };
                for &pt in tri { mesh.vertices.push(egui::epaint::Vertex { pos: pos(pt), uv: egui::epaint::WHITE_UV, color }); }
                mesh.indices.extend_from_slice(&[base,base+1,base+2]);
            }
        }
        if !mesh.indices.is_empty() { painter.add(egui::Shape::mesh(mesh)); }
    }
}

pub fn panel(app: &mut CadApp, ui: &mut egui::Ui) {
    let Ok(doc) = app.session.doc() else { ui.label("Open a drawing to manage text styles."); return; };
    let styles: Vec<cadcraft_doc::TextStyle> = doc.text_styles.iter().cloned().collect();
    let current = doc.header.str("TEXTSTYLE", "Standard");
    let used = used_style_names(doc);
    let id = ui.make_persistent_id("text-style-manager-state");
    let mut state = ui.ctx().data_mut(|d| d.get_persisted::<State>(id)).unwrap_or_default();
    if state.selected.is_empty() || !styles.iter().any(|s| s.name == state.selected) {
        if let Some(st) = styles.iter().find(|s| s.name.eq_ignore_ascii_case(&current)).or_else(|| styles.first()) {
            sync(&mut state, st);
        }
    }
    let mut commands: Vec<(&'static str, Value)> = Vec::new();
    // Consistent 8 px content inset, independent of the dock title bar.
    egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Text styles");
            ui.label(format!("Current: {current}"));
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Styles").strong());
            ui.checkbox(&mut state.only_used, "In use only");
        });
        let chosen = state.selected.clone();
        egui::ComboBox::from_id_salt("text-style-select")
            .selected_text(if chosen.is_empty() { "Select style".to_string() } else { chosen })
            .width(ui.available_width().min(320.0))
            .show_ui(ui, |ui| {
                for style in &styles {
                    if state.only_used && !used.contains(&style.name.to_ascii_lowercase())
                        && !style.name.eq_ignore_ascii_case(&current)
                        && !style.name.eq_ignore_ascii_case(&state.selected) { continue; }
                    let label = if style.name.eq_ignore_ascii_case(&current) { format!("✓ {}", style.name) } else { style.name.clone() };
                    if ui.selectable_label(state.selected == style.name, label).clicked() { sync(&mut state, style); }
                }
            });
        ui.horizontal_wrapped(|ui| {
            let name_response = ui.add(TextEdit::singleline(&mut state.new_name).hint_text("Style name").desired_width(175.0));
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
            if ui.add_enabled(!state.selected.is_empty() && !state.selected.eq_ignore_ascii_case(&current), egui::Button::new("Set current")).clicked() {
                commands.push(("style.current", json!({"name":state.selected})));
            }
            if ui.add_enabled(!state.selected.eq_ignore_ascii_case("Standard") && !state.selected.eq_ignore_ascii_case(&current) && !used.contains(&state.selected.to_ascii_lowercase()), egui::Button::new("Delete")).clicked() {
                commands.push(("style.delete", json!({"name":state.selected})));
                state.selected.clear();
            }
            let rename_to = state.new_name.trim().to_string();
            if ui.add_enabled(!state.selected.eq_ignore_ascii_case("Standard") && !rename_to.is_empty() && !styles.iter().any(|s| s.name.eq_ignore_ascii_case(&rename_to)), egui::Button::new("Rename")).clicked() {
                commands.push(("style.rename", json!({"from":state.selected,"to":rename_to})));
                state.selected = rename_to;
                state.new_name.clear();
            }
        });
        ui.separator();
        ui.label(RichText::new("Properties").strong());
        let mut fonts = available_fonts();
        if !state.font.is_empty() && !fonts.iter().any(|(_, path)| path.eq_ignore_ascii_case(&state.font)) {
            fonts.push((state.font.clone(), state.font.clone()));
        }
        let display_font = fonts.iter().find(|(_,path)| path.eq_ignore_ascii_case(&state.font))
            .map(|(name,_)| name.clone()).unwrap_or_else(|| state.font.clone());
        egui::Grid::new("text-style-properties").num_columns(2).show(ui, |ui| {
            ui.label("Font");
            egui::ComboBox::from_id_salt("text-style-font-select")
                .selected_text(&display_font)
                .width(240.0)
                .show_ui(ui, |ui| {
                    if ui.button("Import font…").clicked() {
                        state.picker_open = true;
                        if state.picker_dir.is_empty() { state.picker_dir = initial_font_dir().to_string_lossy().into_owned(); }
                        state.picker_path_input = state.picker_dir.clone();
                        state.picker_selected.clear();
                        ui.close();
                    }
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        for (name, path) in &fonts {
                            if ui.selectable_label(state.font.eq_ignore_ascii_case(path), name).clicked() {
                                state.font = path.clone();
                            }
                        }
                    });
                });
            ui.end_row();
            ui.label("Height (0 = variable)"); ui.add(TextEdit::singleline(&mut state.height).desired_width(100.0)); ui.end_row();
            ui.label("Width factor"); ui.add(TextEdit::singleline(&mut state.width).desired_width(100.0)); ui.end_row();
            ui.label("Oblique angle (degrees)"); ui.add(TextEdit::singleline(&mut state.oblique).desired_width(100.0)); ui.end_row();
            ui.label("Big Font (SHX)"); ui.add(TextEdit::singleline(&mut state.big_font).desired_width(240.0)); ui.end_row();
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut state.annotative, "Annotative");
            ui.checkbox(&mut state.backwards, "Backwards");
            ui.checkbox(&mut state.upside_down, "Upside down");
            ui.checkbox(&mut state.vertical, "Vertical");
        });
        if ui.button("Apply properties").clicked() && !state.selected.is_empty() {
            let mut props = json!({"name":state.selected,"font":state.font,"current":false});
            if let Ok(v) = state.height.parse::<f64>() { if v >= 0.0 && v.is_finite() { props["height"] = json!(v); } }
            if let Ok(v) = state.width.parse::<f64>() { if v > 0.0 && v.is_finite() { props["widthFactor"] = json!(v); } }
            if let Ok(v) = state.oblique.parse::<f64>() { if v.is_finite() && v.abs() <= 85.0 { props["oblique"] = json!(v); } }
            props["backwards"] = json!(state.backwards);
            props["upsideDown"] = json!(state.upside_down);
            props["vertical"] = json!(state.vertical);
            props["annotative"] = json!(state.annotative);
            props["bigFont"] = json!(state.big_font);
            commands.push(("style", props));
        }
        if !state.message.is_empty() { ui.label(&state.message); }
        ui.separator();
        ui.label("Preview (CADCraft renderer)");
        if let Some(style) = styles.iter().find(|s| s.name.eq_ignore_ascii_case(&state.selected)) {
            let mut preview = style.clone();
            preview.font = state.font.clone();
            if let Ok(w) = state.width.parse::<f64>() { if w > 0.0 { preview.width_factor = w; } }
            if let Ok(angle) = state.oblique.parse::<f64>() { preview.oblique = angle.to_radians(); }
            preview.backwards = state.backwards;
            preview.upside_down = state.upside_down;
            draw_preview(ui, &preview);
        }
    });
    // Cross-platform egui font browser: no WinForms, PowerShell or OS-specific dialog.
    if state.picker_open {
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
                    if ui.button("Up").clicked() {
                        if let Some(parent) = Path::new(&state.picker_dir).parent() { navigate = Some(parent.to_path_buf()); }
                    }
                    let response = ui.add(TextEdit::singleline(&mut state.picker_path_input).desired_width(350.0));
                    if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Go").clicked() {
                        let p = PathBuf::from(state.picker_path_input.trim());
                        if p.is_dir() { navigate = Some(p); }
                        else { state.message = "Folder not found".into(); }
                    }
                });
                ui.horizontal(|ui| { ui.label("Filter"); ui.add(TextEdit::singleline(&mut state.picker_filter).hint_text("Font name")); });
                match browse_entries(Path::new(&state.picker_dir), &state.picker_filter) {
                    Ok(entries) => {
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for (label, path, is_dir) in entries {
                                let text = if is_dir { format!("[Folder] {label}") } else { label };
                                let selected = state.picker_selected == path.to_string_lossy();
                                let response = ui.selectable_label(selected, text);
                                if response.clicked() {
                                    if is_dir { navigate = Some(path.clone()); }
                                    else { state.picker_selected = path.to_string_lossy().into_owned(); }
                                }
                                if response.double_clicked() && !is_dir { selected_font = Some(path); }
                            }
                        });
                    }
                    Err(err) => { ui.label(format!("Unable to list folder: {err}")); }
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(!state.picker_selected.is_empty(), egui::Button::new("Import selected")).clicked() {
                        selected_font = Some(PathBuf::from(&state.picker_selected));
                    }
                    if ui.button("Cancel").clicked() { cancel = true; }
                });
            });
        if let Some(path) = navigate {
            state.picker_dir = path.to_string_lossy().into_owned();
            state.picker_path_input = state.picker_dir.clone();
            state.picker_selected.clear();
        }
        if let Some(path) = selected_font {
            match import_font(&path.to_string_lossy()) {
                Ok(imported) => { state.font = imported; state.message = "Font imported. Click Apply properties to assign it.".into(); }
                Err(err) => { state.message = err; }
            }
            state.picker_open = false;
        }
        if !open || cancel { state.picker_open = false; }
    }
    for (cmd, args) in commands {
        if let Err(err) = app.run(cmd, args) { state.message = err; }
    }
    ui.ctx().data_mut(|d| d.insert_persisted(id, state));
}
