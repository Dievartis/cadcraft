//! The Points List window (CONTROLTAKEOFF typed with no parameters, or the Regulation tab): the load of each controller and module against what it offers, the list of
//! points (what each device asks of its controller), what is wrong, the price of the devices, and a button that writes the controls package as a workbook.

use cadcraft_doc::ControlTakeoff;
use egui::{RichText, vec2};
use serde_json::json;

use crate::CadApp;
use crate::theme::Tokens;

/// The window while open.
#[derive(Clone, Debug, Default)]
pub struct CtrlDlgState {
    pub level: String,
    pub path: String,
    pub takeoff: Option<ControlTakeoff>,
    pub message: Option<String>,
    pub error: bool,
    pub raise: bool,
}

/// Open the window on the controls of the drawing as it is now; open already, count again.
pub fn open(app: &mut CadApp) {
    let mut st = app.ctrl_dlg.take().unwrap_or_else(|| CtrlDlgState { path: "regulation.xlsx".into(), ..Default::default() });
    st.raise = true;
    st.message = None;
    app.ctrl_dlg = Some(st);
    refresh(app);
    app.ui.dialog = Some("controlpoints".into());
}

/// Count again, after the drawing changed.
pub fn refresh(app: &mut CadApp) {
    let level = app.ctrl_dlg.as_ref().map(|s| s.level.clone()).unwrap_or_default();
    let counted = app.session.doc().map(|d| d.control_takeoff(&level)).map_err(|e| e.to_string());
    if let Some(st) = app.ctrl_dlg.as_mut() {
        match counted {
            Ok(t) => (st.takeoff, st.error) = (Some(t), false),
            Err(e) => (st.message, st.error) = (Some(e), true),
        }
    }
}

fn export(app: &mut CadApp, extension: &str) {
    let Some((level, path)) = app.ctrl_dlg.as_ref().map(|s| (s.level.clone(), s.path.clone())) else { return };
    let path = std::path::Path::new(&path).with_extension(extension).display().to_string();
    let mut params = json!({ "level": level, "path": path });
    if !app.takeoff_store.settings.prices.is_empty() {
        params["prices"] = json!(app.takeoff_store.settings.prices);
    }
    let r = app.run("controltakeoff", params);
    if let Some(st) = app.ctrl_dlg.as_mut() {
        st.path = path.clone();
        match r {
            Ok(v) => (st.message, st.error) = (Some(crate::tf!("Written: {path}", path = v["path"].as_str().unwrap_or(&path))), false),
            Err(e) => (st.message, st.error) = (Some(e), true),
        }
    }
}

fn num(v: f64) -> String {
    format!("{v:.1}").trim_end_matches('0').trim_end_matches('.').to_string()
}

fn table(ui: &mut egui::Ui, id: &str, title: &str, heads: &[&str], rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(RichText::new(crate::i18n::t(title)).strong()).id_salt(id).default_open(true).show(ui, |ui| {
        egui::Grid::new(("ctrl_grid", id)).striped(true).num_columns(heads.len()).show(ui, |ui| {
            for h in heads {
                ui.strong(crate::i18n::t(h));
            }
            ui.end_row();
            for r in rows {
                for c in r {
                    ui.label(c);
                }
                ui.end_row();
            }
        });
    });
}

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open_flag: &mut bool) {
    if app.ctrl_dlg.is_none() {
        open(app);
    }
    let warn = Tokens::get().warn;
    let (mut refresh_clicked, mut excel_clicked, mut csv_clicked) = (false, false, false);
    let mut shown = app.ctrl_dlg.as_ref().and_then(|s| s.takeoff.clone());
    if let Some(t) = shown.as_mut() {
        t.costing = Some(t.priced(&app.takeoff_store.settings));
    }
    let settings = &mut app.takeoff_store.settings;
    let Some(st) = app.ctrl_dlg.as_mut() else { return };
    let id = egui::Id::unique("ctrl_dialog");
    if std::mem::take(&mut st.raise) {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
    egui::Window::new(crate::tl!("Points List")).id(id).open(open_flag).collapsible(false).default_size(vec2(840.0, 560.0)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            let label = ui.label(crate::tl!("Level"));
            ui.add(egui::TextEdit::singleline(&mut st.level).desired_width(100.0)).labelled_by(label.id);
            if ui.button(crate::tl!("Count again")).clicked() {
                refresh_clicked = true;
            }
            let label = ui.label(crate::tl!("File"));
            ui.add(egui::TextEdit::singleline(&mut st.path).desired_width(240.0)).labelled_by(label.id);
            if ui.button(crate::tl!("Export Excel")).clicked() {
                excel_clicked = true;
            }
            if ui.button(crate::tl!("Export CSV")).clicked() {
                csv_clicked = true;
            }
        });
        if let Some(m) = &st.message {
            ui.label(if st.error { RichText::new(m).color(warn) } else { RichText::new(m) });
        }
        ui.separator();
        let Some(t) = &shown else { return };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(crate::tf!(
                "Devices: {n}, points: {p}, controllers and modules: {c}",
                n = t.devices.iter().map(|d| d.count).sum::<usize>(),
                p = t.points.len(),
                c = t.controllers.len()
            ));
            for w in &t.warnings {
                ui.label(RichText::new(w).small().color(warn));
            }
            table(
                ui,
                "controllers",
                "Controllers and modules",
                &["Name", "Type", "Extends", "Space", "Bus", "AI used/offered", "AO used/offered", "DI used/offered", "DO used/offered"],
                t.controllers
                    .iter()
                    .map(|c| {
                        vec![
                            c.name.clone(),
                            crate::i18n::t(&c.kind).to_string(),
                            c.extends.clone(),
                            c.space.clone(),
                            c.bus.clone(),
                            format!("{}/{}", c.used.ai, c.capacity.ai),
                            format!("{}/{}", c.used.ao, c.capacity.ao),
                            format!("{}/{}", c.used.di, c.capacity.di),
                            format!("{}/{}", c.used.dout, c.capacity.dout),
                        ]
                    })
                    .collect(),
            );
            table(
                ui,
                "points",
                "Points",
                &["Controller", "Device", "Description", "Point type", "Unit", "Space", "Serves"],
                t.points
                    .iter()
                    .map(|p| {
                        vec![
                            p.controller.clone(),
                            p.device.clone(),
                            crate::i18n::t(&p.description).to_string(),
                            p.point_type.clone(),
                            p.unit.clone(),
                            p.space.clone(),
                            p.served.clone(),
                        ]
                    })
                    .collect(),
            );
            table(
                ui,
                "devices",
                "Devices",
                &["Device", "Count", "Points"],
                t.devices.iter().map(|d| vec![crate::i18n::t(&d.name).to_string(), d.count.to_string(), d.points.to_string()]).collect(),
            );
            if let Some(costing) = &t.costing {
                egui::CollapsingHeader::new(RichText::new(crate::tl!("Prices and cost")).strong()).id_salt("ctrl_prices").default_open(false).show(
                    ui,
                    |ui| {
                        egui::Grid::new("ctrl_prices_grid").striped(true).num_columns(4).show(ui, |ui| {
                            for h in ["Label", "Quantity", "Unit price", "Total"] {
                                ui.strong(crate::i18n::t(h));
                            }
                            ui.end_row();
                            for l in &costing.lines {
                                ui.label(crate::i18n::t(&l.label));
                                ui.label(num(l.quantity));
                                let mut price = l.unit_price.unwrap_or(0.0);
                                if ui.add(egui::DragValue::new(&mut price).speed(0.5).range(0.0..=1.0e6).suffix(" €")).changed() {
                                    if price > 0.0 {
                                        settings.prices.insert(l.key.clone(), price);
                                    } else {
                                        settings.prices.remove(&l.key);
                                    }
                                }
                                ui.label(l.total.map_or("—".to_string(), |x| format!("{} €", num(x))));
                                ui.end_row();
                            }
                        });
                        ui.label(
                            RichText::new(crate::tf!(
                                "Total: {total} € ({n} lines without a price)",
                                total = num(costing.total),
                                n = costing.unpriced
                            ))
                            .strong(),
                        );
                    },
                );
            }
        });
    });
    if refresh_clicked {
        refresh(app);
    }
    if excel_clicked {
        export(app, "xlsx");
    }
    if csv_clicked {
        export(app, "csv");
    }
}
