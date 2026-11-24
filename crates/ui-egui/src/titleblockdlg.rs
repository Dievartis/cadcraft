//! The Title Block dialog (TITLEBLOCKADD typed with no parameters, the Architecture tab, or a double-click on a title block): the project, the parties with
//! their logos, the picture of the project, the table of revisions and the code of the sheet. Everything goes through TITLEBLOCKADD and TITLEBLOCKEDIT; the
//! block on the sheet is redrawn when Apply is pressed.

use std::collections::BTreeMap;

use cadcraft_doc::{Code, Handle, Party, Pic, Revision, TitleBlockParams, titleblock_of};
use egui::{RichText, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::theme::Tokens;

/// The tabs of the dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Project,
    Parties,
    Image,
    Revisions,
    Codes,
}

/// The dialog while open.
#[derive(Clone, Debug)]
pub struct TitleBlockDlg {
    /// The title block being changed; `None` for a new one.
    pub edit: Option<Handle>,
    pub tb: TitleBlockParams,
    pub tab: Tab,
    pub party: usize,
    /// Pictures chosen from a file and not applied yet, by slot (`image`, or `party:<n>`).
    pub picks: BTreeMap<String, String>,
    /// For a picture picked in a browser, which has no path: its bytes in base64, by slot (the name is in `picks`).
    pub data: BTreeMap<String, String>,
    /// The slot whose picture the browser's file picker is being asked for.
    pub waiting: Option<String>,
    pub message: Option<String>,
    pub error: bool,
    pub raise: bool,
}

impl TitleBlockDlg {
    fn forget_picks(&mut self) {
        self.picks.clear();
        self.data.clear();
    }
}

/// A picture the browser's file picker read (there is no path on the web): given to the slot the dialog asked it for. `false` when no slot was waiting for one.
pub fn picture_arrived(app: &mut CadApp, name: &str, bytes: &[u8]) -> bool {
    let Some(st) = app.title_dlg.as_mut() else { return false };
    let Some(slot) = st.waiting.take() else { return false };
    st.picks.insert(slot.clone(), name.to_string());
    st.data.insert(slot, cadcraft_engine::cmd::file::base64_encode(bytes));
    true
}

const IMAGE_SLOT: &str = "image";

fn party_slot(i: usize) -> String {
    format!("party:{i}")
}

/// Open the dialog on title block `edit`, or on a new one sized for the current layout's paper.
pub fn open(app: &mut CadApp, edit: Option<Handle>) {
    let units = app.session.doc().map(cadcraft_doc::HvacUnits::of).unwrap_or_default();
    let existing = edit.and_then(|h| app.session.doc().ok().and_then(|d| d.entity(h).and_then(|e| titleblock_of(e))));
    let tb = existing.unwrap_or_else(|| {
        let (paper, landscape) = sheet_of_current_space(app);
        TitleBlockParams::new_in(
            if crate::i18n::current_language() == "fr" { "fr" } else { "en" },
            crate::i18n::t("Title block"),
            &paper,
            landscape,
            units,
        )
    });
    let edit = edit.filter(|_| titleblock_ok(app, edit));
    app.title_dlg = Some(TitleBlockDlg {
        edit,
        tb,
        tab: Tab::Project,
        party: 0,
        picks: BTreeMap::new(),
        data: BTreeMap::new(),
        waiting: None,
        message: None,
        error: false,
        raise: true,
    });
    app.ui.dialog = Some("titleblock".into());
}

fn titleblock_ok(app: &CadApp, edit: Option<Handle>) -> bool {
    edit.is_some_and(|h| app.session.doc().is_ok_and(|d| d.entity(h).and_then(|e| titleblock_of(e)).is_some()))
}

/// The paper and orientation of the layout being edited ("A3", landscape when there is none).
fn sheet_of_current_space(app: &CadApp) -> (String, bool) {
    let page = app.session.doc().ok().and_then(|d| match app.session.space() {
        cadcraft_doc::Space::Paper(name) => d.layouts.iter().find(|l| l.name == name).map(|l| l.page.clone()),
        _ => None,
    });
    let Some(page) = page else { return ("A3".into(), true) };
    let long = page.width_mm.max(page.height_mm);
    let paper = cadcraft_doc::PAPERS.iter().min_by(|a, b| (a.2 - long).abs().total_cmp(&(b.2 - long).abs())).map_or("A3", |p| p.0);
    (paper.to_string(), page.landscape)
}

/// Where a new title block goes, millimetres on paper: the right edge of the layout's real sheet, inside its margin; the origin in model space.
fn default_at_mm(app: &CadApp, tb: &TitleBlockParams) -> [f64; 2] {
    let page = app.session.doc().ok().and_then(|d| match app.session.space() {
        cadcraft_doc::Space::Paper(name) => d.layouts.iter().find(|l| l.name == name).map(|l| l.page.clone()),
        _ => None,
    });
    let Some(page) = page else { return [0.0, 0.0] };
    let width = if page.landscape { page.width_mm.max(page.height_mm) } else { page.width_mm.min(page.height_mm) };
    [(width - cadcraft_doc::SHEET_MARGIN_MM - tb.width_mm).max(0.0), cadcraft_doc::SHEET_MARGIN_MM]
}

fn pic_json(st: &TitleBlockDlg, slot: &str, pic: Option<&Pic>) -> Value {
    if let Some(path) = st.picks.get(slot) {
        return match st.data.get(slot) {
            Some(data) => json!({ "path": path, "data": data }),
            None => json!({ "path": path }),
        };
    }
    pic.map_or(Value::Null, |p| json!({ "key": p.key }))
}

/// The parameters of the command: the whole block as the dialog has it.
fn params(st: &TitleBlockDlg) -> Value {
    let t = &st.tb;
    let parties: Vec<Value> = t
        .parties
        .iter()
        .enumerate()
        .map(|(i, p)| {
            json!({ "role": p.role, "name": p.name, "address": p.address, "phone": p.phone, "email": p.email, "afterImage": p.after_image,
                    "logo": pic_json(st, &party_slot(i), p.logo.as_ref()) })
        })
        .collect();
    json!({
        "name": t.name, "lang": t.lang, "paper": t.paper, "landscape": t.landscape, "width": t.width_mm,
        "project": t.project, "description": t.description, "address": t.address, "title": t.title,
        "parties": parties, "image": pic_json(st, IMAGE_SLOT, t.image.as_ref()),
        "revisions": t.revisions.iter().map(|r| json!({ "index": r.index, "date": r.date, "nature": r.nature, "by": r.by, "checked": r.checked })).collect::<Vec<_>>(),
        "revisionRows": t.revision_rows,
        "codes": t.codes.iter().map(|c| json!({ "label": c.label, "value": c.value })).collect::<Vec<_>>(),
    })
}

/// Apply: draw the block, or redraw the one being changed; the dialog then shows what the drawing holds.
fn apply(app: &mut CadApp) {
    let Some(st) = app.title_dlg.as_ref() else { return };
    let mut p = params(st);
    let r = match st.edit {
        Some(h) => {
            p["handle"] = json!(h.hex());
            app.run("titleblockedit", p)
        }
        None => {
            p["atMm"] = json!(default_at_mm(app, &st.tb));
            app.run("titleblockadd", p)
        }
    };
    let handle = match &r {
        Ok(v) => v["handle"].as_str().and_then(Handle::parse_hex).or(app.title_dlg.as_ref().and_then(|s| s.edit)),
        Err(_) => None,
    };
    if let Some(st) = app.title_dlg.as_mut() {
        match r {
            Ok(_) => {
                st.edit = handle;
                st.forget_picks();
                st.message = Some(crate::i18n::t("Title block drawn.").to_string());
                st.error = false;
            }
            Err(e) => (st.message, st.error) = (Some(e), true),
        }
    }
    // The pictures now live in the drawing: read the block back so the dialog shows their keys.
    if let Some(h) = handle {
        let back = app.session.doc().ok().and_then(|d| d.entity(h).and_then(|e| titleblock_of(e)));
        if let (Some(tb), Some(st)) = (back, app.title_dlg.as_mut()) {
            st.tb = tb;
        }
    }
}

fn pick_picture(app: &mut CadApp, slot: String) {
    if let Some(st) = app.title_dlg.as_mut() {
        st.waiting = Some(slot.clone());
    }
    let asked = app.pick_then(crate::Pick::Image, move |app, path| {
        if let Some(st) = app.title_dlg.as_mut() {
            st.picks.insert(slot, path);
        }
        Ok(Value::Null)
    });
    if asked.is_none()
        && let Some(st) = app.title_dlg.as_mut()
    {
        (st.message, st.error) = (Some(crate::i18n::t("Choosing a picture is not available here.").to_string()), true);
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, width: f32) {
    ui.horizontal(|ui| {
        let l = ui.label(crate::i18n::t(label));
        ui.add(egui::TextEdit::singleline(value).desired_width(width)).labelled_by(l.id);
    });
}

fn picture_row(
    ui: &mut egui::Ui,
    picks: &BTreeMap<String, String>,
    slot: &str,
    pic: Option<Pic>,
    want_pick: &mut Option<String>,
    remove: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        let shown = match (picks.get(slot), &pic) {
            (Some(path), _) => std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            (None, Some(p)) => crate::tf!("{w} x {h} pixels", w = p.width, h = p.height),
            (None, None) => crate::i18n::t("No picture").to_string(),
        };
        ui.label(shown);
        if ui.button(crate::i18n::t("Choose a picture...")).clicked() {
            *want_pick = Some(slot.to_string());
        }
        if (pic.is_some() || picks.contains_key(slot)) && ui.button(crate::i18n::t("Remove")).clicked() {
            *remove = Some(slot.to_string());
        }
    });
}

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open_flag: &mut bool) {
    let warn = Tokens::get().warn;
    let (mut apply_clicked, mut want_pick, mut remove) = (false, None::<String>, None::<String>);
    let Some(st) = app.title_dlg.as_mut() else {
        *open_flag = false;
        return;
    };
    let id = egui::Id::unique("titleblock_dialog");
    if std::mem::take(&mut st.raise) {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
    let title = if st.edit.is_some() { crate::tl!("Edit Title Block") } else { crate::tl!("Title Block") };
    egui::Window::new(title).id(id).open(open_flag).collapsible(false).default_size(vec2(560.0, 520.0)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (tab, label) in
                [(Tab::Project, "Project"), (Tab::Parties, "Parties"), (Tab::Image, "Picture"), (Tab::Revisions, "Revisions"), (Tab::Codes, "Codes")]
            {
                ui.selectable_value(&mut st.tab, tab, crate::i18n::t(label));
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, false]).show(ui, |ui| match st.tab {
            Tab::Project => {
                ui.horizontal(|ui| {
                    let l = ui.label(crate::i18n::t("Paper"));
                    craft_ui::ComboBox::from_id_salt("tb_paper").selected_text(st.tb.paper.clone()).show_ui(ui, |ui| {
                        for (name, ..) in cadcraft_doc::PAPERS {
                            if ui.selectable_label(st.tb.paper == name, name).clicked() && st.tb.paper != name {
                                st.tb.paper = name.to_string();
                                st.tb.width_mm = cadcraft_doc::default_width_mm(name);
                            }
                        }
                    });
                    let _ = l;
                    ui.radio_value(&mut st.tb.landscape, true, crate::i18n::t("Landscape"));
                    ui.radio_value(&mut st.tb.landscape, false, crate::i18n::t("Portrait"));
                });
                ui.horizontal(|ui| {
                    let l = ui.label(crate::i18n::t("Width (mm)"));
                    ui.add(egui::DragValue::new(&mut st.tb.width_mm).range(60.0..=400.0).speed(1.0)).labelled_by(l.id);
                });
                field(ui, "Project", &mut st.tb.project, 380.0);
                field(ui, "Description", &mut st.tb.description, 380.0);
                field(ui, "Address", &mut st.tb.address, 380.0);
                field(ui, "Title of the sheet", &mut st.tb.title, 380.0);
            }
            Tab::Parties => {
                if st.party >= st.tb.parties.len() {
                    st.party = st.tb.parties.len().saturating_sub(1);
                }
                ui.horizontal_wrapped(|ui| {
                    for (i, p) in st.tb.parties.iter().enumerate() {
                        let name = if p.role.is_empty() { format!("#{}", i + 1) } else { p.role.clone() };
                        ui.selectable_value(&mut st.party, i, name);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("Add")).clicked() && st.tb.parties.len() < cadcraft_doc::MAX_PARTIES {
                        st.tb.parties.push(Party::default());
                        st.party = st.tb.parties.len() - 1;
                    }
                    let n = st.tb.parties.len();
                    if ui.add_enabled(n > 0, egui::Button::new(crate::i18n::t("Remove"))).clicked() {
                        st.tb.parties.remove(st.party);
                        st.forget_picks();
                    }
                    if ui.add_enabled(st.party > 0, egui::Button::new(crate::i18n::t("Move up"))).clicked() {
                        st.tb.parties.swap(st.party, st.party - 1);
                        st.forget_picks();
                        st.party -= 1;
                    }
                    if ui.add_enabled(st.party + 1 < n, egui::Button::new(crate::i18n::t("Move down"))).clicked() {
                        st.tb.parties.swap(st.party, st.party + 1);
                        st.forget_picks();
                        st.party += 1;
                    }
                });
                ui.separator();
                let i = st.party;
                if let Some(p) = st.tb.parties.get_mut(i) {
                    field(ui, "Role", &mut p.role, 300.0);
                    field(ui, "Name", &mut p.name, 300.0);
                    field(ui, "Address", &mut p.address, 300.0);
                    field(ui, "Phone", &mut p.phone, 300.0);
                    field(ui, "Email", &mut p.email, 300.0);
                    ui.checkbox(&mut p.after_image, crate::i18n::t("Under the picture of the project"));
                    let pic = p.logo.clone();
                    ui.label(RichText::new(crate::i18n::t("Logo")).strong());
                    picture_row(ui, &st.picks, &party_slot(i), pic, &mut want_pick, &mut remove);
                } else {
                    ui.label(crate::i18n::t("No party."));
                }
            }
            Tab::Image => {
                ui.label(crate::i18n::t("The picture of the project: a view or a photo."));
                let pic = st.tb.image.clone();
                picture_row(ui, &st.picks, IMAGE_SLOT, pic, &mut want_pick, &mut remove);
            }
            Tab::Revisions => {
                ui.horizontal(|ui| {
                    let l = ui.label(crate::i18n::t("Lines shown"));
                    let mut rows = st.tb.revision_rows.max(st.tb.revisions.len());
                    if ui.add(egui::DragValue::new(&mut rows).range(st.tb.revisions.len()..=cadcraft_doc::MAX_REVISIONS)).labelled_by(l.id).changed()
                    {
                        st.tb.revision_rows = rows;
                    }
                    if ui.button(crate::i18n::t("Add a revision")).clicked() && st.tb.revisions.len() < cadcraft_doc::MAX_REVISIONS {
                        st.tb.revisions.push(Revision::default());
                    }
                });
                let mut del = None;
                egui::Grid::new("tb_revisions").num_columns(6).show(ui, |ui| {
                    for h in ["Index", "Date", "Nature of the change", "Drawn by", "Checked by", ""] {
                        ui.strong(crate::i18n::t(h));
                    }
                    ui.end_row();
                    for (i, r) in st.tb.revisions.iter_mut().enumerate() {
                        for (v, w, name) in [
                            (&mut r.index, 40.0, "Index"),
                            (&mut r.date, 80.0, "Date"),
                            (&mut r.nature, 200.0, "Nature of the change"),
                            (&mut r.by, 60.0, "Drawn by"),
                            (&mut r.checked, 60.0, "Checked by"),
                        ] {
                            ui.add(egui::TextEdit::singleline(v).desired_width(w)).widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::TextInput, true, format!("{} {}", crate::i18n::t(name), i + 1))
                            });
                        }
                        if ui.button("✕").on_hover_text(crate::i18n::t("Remove")).clicked() {
                            del = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = del {
                    st.tb.revisions.remove(i);
                }
            }
            Tab::Codes => {
                ui.label(crate::i18n::t("The code of the sheet: each cell has a name and a value."));
                let mut del = None;
                egui::Grid::new("tb_codes").num_columns(3).show(ui, |ui| {
                    for (i, c) in st.tb.codes.iter_mut().enumerate() {
                        for (v, name) in [(&mut c.label, "Name"), (&mut c.value, "Value")] {
                            ui.add(egui::TextEdit::singleline(v).desired_width(140.0)).widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::TextInput, true, format!("{} {}", crate::i18n::t(name), i + 1))
                            });
                        }
                        if ui.button("✕").on_hover_text(crate::i18n::t("Remove")).clicked() {
                            del = Some(i);
                        }
                        ui.end_row();
                    }
                });
                if let Some(i) = del {
                    st.tb.codes.remove(i);
                }
                if ui.button(crate::i18n::t("Add")).clicked() && st.tb.codes.len() < cadcraft_doc::MAX_CODES {
                    st.tb.codes.push(Code::default());
                }
            }
        });
        if let Some(m) = &st.message {
            ui.label(if st.error { RichText::new(m).color(warn) } else { RichText::new(m) });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let label = if st.edit.is_some() { crate::tl!("Apply") } else { crate::tl!("Insert the title block") };
            if ui.button(label).clicked() {
                apply_clicked = true;
            }
        });
    });
    // The picture rows only say what was asked: the pick and the removal are applied here.
    if let Some(slot) = remove
        && let Some(st) = app.title_dlg.as_mut()
    {
        st.picks.remove(&slot);
        st.data.remove(&slot);
        if slot == IMAGE_SLOT {
            st.tb.image = None;
        } else if let Some(i) = slot.strip_prefix("party:").and_then(|n| n.parse::<usize>().ok())
            && let Some(p) = st.tb.parties.get_mut(i)
        {
            p.logo = None;
        }
    }
    if let Some(slot) = want_pick {
        pick_picture(app, slot);
    }
    if apply_clicked {
        apply(app);
    }
    if !*open_flag {
        app.title_dlg = None;
    }
}

#[cfg(test)]
mod tests {
    use cadcraft_engine::Session;

    use super::*;
    use crate::Services;

    fn app() -> CadApp {
        let mut app = CadApp::new(Session::new(), Services::default());
        *app.session.doc_mut().unwrap() = cadcraft_doc::Drawing::new_metric();
        app
    }

    fn block(app: &CadApp) -> Option<TitleBlockParams> {
        app.session.doc().unwrap().model.iter().find_map(|e| titleblock_of(e))
    }

    /// One frame of the dialog on every tab: nothing panics and the tabs show their fields.
    fn show(app: &mut CadApp, ctx: &egui::Context) {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut open = true;
            dialog(app, ui.ctx(), &mut open);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn the_dialog_draws_a_title_block_and_changes_it_again() {
        let mut app = app();
        let ctx = egui::Context::default();
        open(&mut app, None);
        for tab in [Tab::Project, Tab::Parties, Tab::Image, Tab::Revisions, Tab::Codes] {
            app.title_dlg.as_mut().unwrap().tab = tab;
            show(&mut app, &ctx);
        }
        {
            let st = app.title_dlg.as_mut().unwrap();
            st.tb.project = "NOM DU PROJET".into();
            st.tb.parties[0].name = "Société A".into();
            st.tb.revisions.push(Revision {
                index: "A".into(),
                date: "30/11/2023".into(),
                nature: "Première diffusion".into(),
                ..Revision::default()
            });
        }
        apply(&mut app);
        let t = block(&app).expect("the block was drawn");
        assert_eq!((t.project.as_str(), t.parties[0].name.as_str(), t.revisions.len()), ("NOM DU PROJET", "Société A", 1));
        assert!(app.title_dlg.as_ref().unwrap().edit.is_some(), "the dialog now edits that block");
        // Another revision from the dialog: the block on the sheet is redrawn, and still one block.
        {
            let st = app.title_dlg.as_mut().unwrap();
            st.tb.revisions.push(Revision { index: "B".into(), ..Revision::default() });
            st.tb.title = "Plan des réseaux".into();
        }
        apply(&mut app);
        let t = block(&app).unwrap();
        assert_eq!((t.revisions.len(), t.title.as_str()), (2, "Plan des réseaux"));
        assert_eq!(app.session.doc().unwrap().model.iter().filter(|e| titleblock_of(e).is_some()).count(), 1);
    }

    #[test]
    fn a_refused_block_says_why_and_draws_nothing() {
        let mut app = app();
        open(&mut app, None);
        app.title_dlg.as_mut().unwrap().tb.width_mm = 10.0;
        apply(&mut app);
        let st = app.title_dlg.as_ref().unwrap();
        assert!(st.error && st.message.is_some());
        assert!(block(&app).is_none());
    }

    #[test]
    fn a_layout_gets_the_column_at_its_right_edge() {
        let mut app = app();
        app.session.execute("layout.new", &json!({"name": "Sheet"})).unwrap();
        app.session.execute("layout.set", &json!({"name": "Sheet"})).unwrap();
        assert!(matches!(app.session.space(), cadcraft_doc::Space::Paper(_)), "the sheet is current");
        open(&mut app, None);
        apply(&mut app);
        let d = app.session.doc().unwrap();
        let (layout, e) =
            d.layouts.iter().find_map(|l| l.entities.iter().find(|e| titleblock_of(e).is_some()).map(|e| (l, e))).expect("on the sheet");
        let (t, at) = (
            titleblock_of(e).unwrap(),
            match &e.kind {
                cadcraft_doc::EntityKind::Insert(i) => i.insert,
                _ => cadcraft_geom::Vec3::ZERO,
            },
        );
        // The column's right edge is one margin from the sheet's right edge.
        let sheet_w = if t.landscape { layout.page.width_mm.max(layout.page.height_mm) } else { layout.page.width_mm.min(layout.page.height_mm) };
        assert!((at.x * t.units.mm_per_unit + t.width_mm - (sheet_w - cadcraft_doc::SHEET_MARGIN_MM)).abs() < 1.0, "{} {sheet_w}", at.x);
        assert_eq!(t.units.mm_per_unit, cadcraft_render::paper::page_unit_mm(d, &layout.page), "drawn in the layout's paper units");
    }
}
