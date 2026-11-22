//! Optional wall/opening tools over freemans32's generators, hosted in the Blocks dock.
use crate::CadApp;
use cadcraft_doc::{OpeningKind, OpeningParams, OpeningStyle, WallJustification, WallParams, opening_of, wall_of};
use egui::Ui;
use serde_json::{Value, json};

pub(crate) fn command(id: &str) -> bool {
    matches!(
        id,
        "walladd" | "walledit" | "wallstyleedit" | "wallcleanup" | "wallelevation" | "openingadd" | "openingedit" | "openingconvert" | "cornerwindow"
    )
}
pub(crate) fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if id.eq_ignore_ascii_case("wallstyleedit") && params.is_null() {
        crate::wallstyledlg::open(app);
        return Some(Ok(Value::Null));
    }
    None
}
#[derive(Clone)]
enum Draft {
    Wall(Box<WallParams>),
    Opening(OpeningParams),
}
#[derive(Clone, Default)]
struct State {
    version: String,
    draft: Option<(cadcraft_doc::Handle, Draft)>,
    error: Option<String>,
}

pub(crate) fn panel(app: &mut CadApp, ui: &mut Ui) {
    ui.label(crate::tl!("Walls and openings"));
    if !matches!(app.session.space(), cadcraft_doc::Space::Model) {
        ui.label(crate::tl!("Switch to Model or enter a model viewport to draw walls and openings."));
        if ui.button(crate::tl!("Wall Styles")).clicked() {
            crate::wallstyledlg::open(app);
        }
        return;
    }

    let styles = app.session.doc().map(|d| d.available_wall_styles()).unwrap_or_default();
    if app.session.last_used.wall_style.is_empty()
        && let Some(first) = styles.first()
    {
        app.session.last_used.wall_style = first.name.clone();
    }
    let label = ui.label(crate::tl!("Wall style"));
    craft_ui::ComboBox::from_id_salt("architecture-wall-style")
        .selected_text(&app.session.last_used.wall_style)
        .show_ui(ui, |ui| {
            for style in &styles {
                ui.selectable_value(&mut app.session.last_used.wall_style, style.name.clone(), &style.name);
            }
        })
        .response
        .labelled_by(label.id);
    ui.horizontal_wrapped(|ui| {
        if ui.button(crate::tl!("Draw Wall")).clicked() {
            app.start("walladd");
        }
        if ui.button(crate::tl!("Wall Styles")).clicked() {
            crate::wallstyledlg::open(app);
        }
    });
    ui.separator();
    kind_choice(ui, &mut app.session.last_used.opening.kind);
    style_choice(ui, &mut app.session.last_used.opening.style);
    ui.horizontal_wrapped(|ui| {
        if ui.button(crate::tl!("Place Opening")).clicked() {
            app.start("openingadd");
        }
        if ui.button(crate::tl!("Corner Window")).clicked() {
            app.start("cornerwindow");
        }
    });
    ui.separator();
    let id = egui::Id::unique("architecture-editor");
    let mut state = ui.data(|d| d.get_temp::<State>(id)).unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        if ui.button(crate::tl!("Wall Junctions")).clicked() {
            state.error = app.run("wallcleanup", json!({})).err();
        }
        if ui.button(crate::tl!("Wall Elevation")).clicked() {
            app.start("wallelevation");
        }
    });
    editor(app, ui, &mut state);
    if let Some(error) = &state.error {
        ui.colored_label(crate::theme::Tokens::get().warn, error);
    }
    ui.data_mut(|d| d.insert_temp(id, state));
}
fn kind_label(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "Door",
        OpeningKind::DoubleDoor => "Double door",
        OpeningKind::Window => "Window",
        OpeningKind::Bay => "Wall opening",
    }
}
fn kind_choice(ui: &mut Ui, kind: &mut OpeningKind) {
    let label = ui.label(crate::tl!("Opening type"));
    craft_ui::ComboBox::from_id_salt("architecture-opening-kind")
        .selected_text(crate::i18n::t(kind_label(*kind)))
        .show_ui(ui, |ui| {
            for k in OpeningKind::ALL {
                ui.selectable_value(kind, k, crate::i18n::t(kind_label(k)));
            }
        })
        .response
        .labelled_by(label.id);
}
fn style_choice(ui: &mut Ui, style: &mut OpeningStyle) {
    let label = ui.label(crate::tl!("Opening style"));
    craft_ui::ComboBox::from_id_salt("architecture-opening-style")
        .selected_text(crate::i18n::t(opening_style_label(*style)))
        .show_ui(ui, |ui| {
            for s in OpeningStyle::ALL {
                ui.selectable_value(style, s, crate::i18n::t(opening_style_label(s)));
            }
        })
        .response
        .labelled_by(label.id);
}
fn size(ui: &mut Ui, label: &str, n: &mut f64) {
    ui.horizontal(|ui| {
        let l = ui.label(crate::i18n::t(label));
        ui.add(egui::DragValue::new(n).speed(10.0).suffix(" mm")).labelled_by(l.id);
    });
}
fn editor(app: &mut CadApp, ui: &mut Ui, state: &mut State) {
    let selection = app.session.selection();
    let next = if let [h] = selection.as_slice() {
        app.session
            .doc()
            .ok()
            .and_then(|d| d.entity(*h))
            .and_then(|e| wall_of(e).map(|w| Draft::Wall(Box::new(w))).or_else(|| opening_of(e).map(Draft::Opening)))
            .map(|p| (*h, p))
    } else {
        None
    };
    let version = next
        .as_ref()
        .map(|(h, p)| {
            format!(
                "{}:{}",
                h.hex(),
                match p {
                    Draft::Wall(w) => serde_json::to_string(w),
                    Draft::Opening(o) => serde_json::to_string(o),
                }
                .unwrap_or_default()
            )
        })
        .unwrap_or_default();
    if version != state.version {
        state.version = version;
        state.draft = next;
    }
    let Some((h, draft)) = &mut state.draft else {
        return;
    };
    ui.separator();
    let (command, mut params) = match draft {
        Draft::Wall(w) => {
            ui.label(crate::tl!("Selected wall"));
            size(ui, "Wall height", &mut w.height);
            let styles = app.session.doc().map(|d| d.available_wall_styles()).unwrap_or_default();
            let label = ui.label(crate::tl!("Wall style"));
            craft_ui::ComboBox::from_id_salt("selected-wall-style")
                .selected_text(&w.style.name)
                .show_ui(ui, |ui| {
                    for style in styles {
                        let name = style.name.clone();
                        if ui.selectable_label(w.style.name == name, &name).clicked() {
                            w.style = style;
                        }
                    }
                })
                .response
                .labelled_by(label.id);
            let label = ui.label(crate::tl!("Justification"));
            craft_ui::ComboBox::from_id_salt("selected-wall-justification")
                .selected_text(w.justification.name())
                .show_ui(ui, |ui| {
                    for j in [WallJustification::Left, WallJustification::Center, WallJustification::Right] {
                        ui.selectable_value(&mut w.justification, j, crate::i18n::t(j.name()));
                    }
                })
                .response
                .labelled_by(label.id);
            ui.checkbox(&mut w.start_free, crate::tl!("Free start"));
            ui.checkbox(&mut w.end_free, crate::tl!("Free end"));
            (
                "walledit",
                json!({"style":w.style.name,"height":w.height,"justification":w.justification.name(),"freeStart":w.start_free,"freeEnd":w.end_free}),
            )
        }
        Draft::Opening(o) => {
            ui.label(crate::tl!("Selected opening"));
            ui.push_id("selected-opening", |ui| {
                kind_choice(ui, &mut o.kind);
                style_choice(ui, &mut o.style);
            });
            size(ui, "Opening width", &mut o.width_mm);
            size(ui, "Opening height", &mut o.height_mm);
            size(ui, "Sill height", &mut o.sill_mm);
            ui.checkbox(&mut o.flip_side, crate::tl!("Flip side"));
            ui.checkbox(&mut o.flip_hand, crate::tl!("Flip hand"));
            (
                "openingedit",
                json!({"type":o.kind.name(),"style":o.style.name(),"width":o.width_mm,"height":o.height_mm,"sill":o.sill_mm,"flipSide":o.flip_side,"flipHand":o.flip_hand}),
            )
        }
    };
    params["handle"] = json!(h.hex());
    if ui.button(crate::tl!("Apply architectural changes")).clicked() {
        state.error = app.run(command, params).err();
    }
}

fn opening_style_label(style: OpeningStyle) -> &'static str {
    match style {
        OpeningStyle::Standard => "Standard",
        OpeningStyle::Sliding => "Sliding",
        OpeningStyle::Pocket => "Pocket",
        OpeningStyle::Folding => "Folding",
        OpeningStyle::Tilting => "Tilting",
        OpeningStyle::CentreParting => "Centre parting",
        OpeningStyle::Revolving => "Revolving",
        OpeningStyle::Casement => "Casement",
        OpeningStyle::DoubleCasement => "Double casement",
        OpeningStyle::Fixed => "Fixed",
        OpeningStyle::Skylight => "Skylight",
    }
}
