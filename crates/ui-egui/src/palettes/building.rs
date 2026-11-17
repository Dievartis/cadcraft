//! The Tool Sets tabs for building objects: Architecture (wall styles to draw with, wall
//! editing) and HVAC (spaces, air terminals). Their content comes from the drawing (its wall styles), so they are drawn here
//! rather than from a fixed list of tools.

use cadcraft_color::Color;
use cadcraft_doc::{AccessoryKind, AhuKind, AirDirection, ComponentFunction, OpeningKind, OpeningStyle, PipeLeg, PipeService, TerminalKind};
use egui::{Rect, Sense, Stroke, pos2, vec2};

use super::{hvachelp, thumbs};
use crate::CadApp;
use crate::theme::Tokens;

/// The building tabs, after Drafting and Modeling.
pub(super) const TABS: &[&str] = &["Architecture", "HVAC", "Regulation", "Electricity"];

/// Where the state of the sections travels while a tab is drawn, and whether the section being drawn is open.
pub(super) fn sections_id() -> egui::Id {
    egui::Id::NULL.with("palette_sections")
}

pub(super) fn open_id() -> egui::Id {
    egui::Id::NULL.with("palette_section_open")
}

/// The header of a section of the HVAC tab: a triangle and its title; clicked, it folds or opens the rows under it (the rows of a
/// closed section are not drawn: see [`row_tip`]). The state is kept as the Options say ([`crate::sections`]).
pub(super) fn header(ui: &mut egui::Ui, title: &str) {
    header_with(ui, title, None);
}

/// [`header`] with an "i" at the right that opens the help of the whole group.
fn header_with(ui: &mut egui::Ui, title: &str, help: Option<&'static str>) {
    let t = Tokens::get();
    let key = format!("HVAC/{title}");
    let mut open = ui.data(|d| d.get_temp::<crate::sections::Sections>(sections_id())).is_none_or(|s| s.is_open(&key));
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
    if resp.clicked() {
        open = !open;
        ui.data_mut(|d| d.get_temp_mut_or_default::<crate::sections::Sections>(sections_id()).set(&key, open));
    }
    ui.data_mut(|d| d.insert_temp(open_id(), open));
    let c = pos2(r.left() + 10.0, r.center().y);
    let arrow = if open {
        vec![c + vec2(-4.0, -2.0), c + vec2(4.0, -2.0), c + vec2(0.0, 3.0)]
    } else {
        vec![c + vec2(-2.0, -4.0), c + vec2(-2.0, 4.0), c + vec2(3.0, 0.0)]
    };
    let colour = if resp.hovered() { t.text } else { t.text_dim };
    ui.painter().add(egui::Shape::convex_polygon(arrow, colour, Stroke::NONE));
    ui.painter().text(
        pos2(r.left() + 22.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        crate::i18n::t(title),
        egui::FontId::proportional(12.5),
        t.text,
    );
    crate::icons::describe_control(ui, &resp, title, Some(open), false);
    if let Some(id) = help {
        let i = Rect::from_center_size(pos2(r.right() - 14.0, r.center().y), vec2(18.0, 18.0));
        let ir = ui.interact(i, resp.id.with("help"), Sense::click());
        let colour = if ir.hovered() { t.text } else { t.text_dim };
        ui.painter().circle_stroke(i.center(), 7.5, Stroke::new(1.0, colour));
        ui.painter().text(i.center(), egui::Align2::CENTER_CENTER, "i", egui::FontId::proportional(11.0), colour);
        crate::icons::describe_control(ui, &ir, "Help", None, false);
        if ir.clicked() {
            hvachelp::request(ui.ctx(), id);
        }
    }
}

pub(super) fn separator(ui: &mut egui::Ui) {
    let t = Tokens::get();
    let (sr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(sr.x_range().shrink(6.0), sr.center().y, Stroke::new(1.0, t.border));
}

/// A full-width button with a small picture drawn by `paint` on its left; true when clicked.
pub(super) fn row_button(ui: &mut egui::Ui, label: &str, selected: bool, paint: impl FnOnce(&egui::Painter, Rect)) -> bool {
    row_tip(ui, label, "", None, selected, paint)
}

/// [`row_button`] with a how-to shown when the pointer rests on it (empty: none).
pub(super) fn row_tip(
    ui: &mut egui::Ui,
    label: &str,
    tip: &str,
    help: Option<&'static str>,
    selected: bool,
    paint: impl FnOnce(&egui::Painter, Rect),
) -> bool {
    // The rows of a section that is folded are not drawn.
    if ui.data(|d| d.get_temp::<bool>(open_id())) == Some(false) {
        return false;
    }
    let t = Tokens::get();
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    let r = r.shrink2(vec2(6.0, 2.0));
    let p = ui.painter();
    if selected || resp.hovered() {
        p.rect_filled(r, 4.0, if selected { t.tab_active } else { t.chrome_dark });
    }
    paint(p, Rect::from_min_size(pos2(r.left() + 6.0, r.center().y - 8.0), vec2(30.0, 16.0)));
    p.text(pos2(r.left() + 44.0, r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), t.text);
    crate::icons::describe_control(ui, &resp, label, Some(selected), false);
    let clicked = resp.clicked();
    // The "i" at the right: the tool's help window.
    if let Some(id) = help {
        let i = Rect::from_center_size(pos2(r.right() - 14.0, r.center().y), vec2(18.0, 18.0));
        let ir = ui.interact(i, resp.id.with("help"), Sense::click());
        let p = ui.painter();
        p.circle_stroke(i.center(), 7.5, Stroke::new(1.0, if ir.hovered() { t.text } else { t.text_dim }));
        p.text(i.center(), egui::Align2::CENTER_CENTER, "i", egui::FontId::proportional(11.0), if ir.hovered() { t.text } else { t.text_dim });
        crate::icons::describe_control(ui, &ir, "Help", None, false);
        if ir.clicked() {
            hvachelp::request(ui.ctx(), id);
        }
        return if tip.is_empty() { clicked } else { resp.on_hover_text(tip).clicked() || clicked };
    }
    if tip.is_empty() { clicked } else { resp.on_hover_text(tip).clicked() || clicked }
}

/// A wall style's components side by side, as wide as they are thick.
fn paint_style(p: &egui::Painter, r: Rect, st: &cadcraft_doc::WallStyle) {
    let total = st.thickness().max(1e-9);
    let mut x = r.left();
    for c in &st.components {
        let w = (c.thickness / total) as f32 * r.width();
        // A component with its own colour shows it; one on the layer's colour a shade for what
        // it does, so a wall's make-up reads at a glance.
        let fill = match c.color {
            Color::ByLayer | Color::ByBlock => match c.function {
                ComponentFunction::Structure => egui::Color32::from_rgb(176, 122, 98),
                ComponentFunction::Insulation => egui::Color32::from_rgb(233, 198, 74),
                ComponentFunction::Finish => egui::Color32::from_gray(225),
                ComponentFunction::Membrane => egui::Color32::from_rgb(91, 156, 255),
                ComponentFunction::AirGap => egui::Color32::from_gray(45),
            },
            other => {
                let rgb = other.resolve(Color::Index(8), Color::Index(8));
                egui::Color32::from_rgb(rgb.0, rgb.1, rgb.2)
            }
        };
        p.rect_filled(Rect::from_min_size(pos2(x, r.top()), vec2(w.max(1.0), r.height())), 0.0, fill);
        x += w;
    }
    p.rect_stroke(r, 0.0, Stroke::new(1.0, egui::Color32::from_gray(40)), egui::StrokeKind::Inside);
}

/// The body of building tab `tab`.
pub(super) fn body(app: &mut CadApp, ui: &mut egui::Ui, tab: &str) {
    match tab {
        "Architecture" => architecture(app, ui),
        "HVAC" => hvac(app, ui),
        "Regulation" => super::regulation::body(app, ui),
        "Electricity" => super::electricity::body(app, ui),
        _ => {}
    }
}

/// A small picture of air terminal `kind` in `r`, as it is drawn in plan.
fn paint_terminal(p: &egui::Painter, r: Rect, kind: TerminalKind) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    let c = r.center();
    let h = r.height() / 2.0;
    match kind {
        TerminalKind::SquareCeiling => {
            let q = Rect::from_center_size(c, vec2(2.0 * h, 2.0 * h));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            p.circle_stroke(c, h * 0.35, s);
            for corner in [q.left_top(), q.right_top(), q.left_bottom(), q.right_bottom()] {
                p.line_segment([c + (corner - c) * 0.4, corner], s);
            }
        }
        TerminalKind::Round => {
            p.circle_stroke(c, h, s);
            p.circle_stroke(c, h * 0.45, s);
        }
        TerminalKind::Rectangular => {
            let q = Rect::from_center_size(c, vec2(r.width(), h));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            for k in [-0.25f32, 0.0, 0.25] {
                p.line_segment([pos2(q.left() + 2.0, c.y + k * h), pos2(q.right() - 2.0, c.y + k * h)], s);
            }
        }
        TerminalKind::Linear => {
            let q = Rect::from_center_size(c, vec2(r.width(), h * 0.6));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            p.line_segment([pos2(q.left(), c.y), pos2(q.right(), c.y)], s);
        }
        TerminalKind::Louvre => {
            let q = Rect::from_center_size(c, vec2(2.0 * h, 1.4 * h));
            p.rect_stroke(q, 0.0, s, egui::StrokeKind::Inside);
            for k in [-0.5, 0.0, 0.5] {
                p.line_segment([pos2(q.left() + 3.0, c.y + k * q.height() * 0.8), pos2(q.right() - 3.0, c.y + k * q.height() * 0.8 - 2.0)], s);
            }
        }
        TerminalKind::Extract => {
            p.circle_stroke(c, h, s);
            let d = h * 0.7;
            p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], s);
            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], s);
        }
    }
}

/// A space outline with lines for its label.
fn paint_space(p: &egui::Painter, r: Rect) {
    let t = Tokens::get();
    let s = Stroke::new(1.2, t.icon_accent);
    p.rect_stroke(r, 0.0, s, egui::StrokeKind::Inside);
    p.line_segment([pos2(r.left() + 6.0, r.center().y - 2.0), pos2(r.right() - 6.0, r.center().y - 2.0)], Stroke::new(1.0, t.icon));
    p.line_segment([pos2(r.left() + 9.0, r.center().y + 3.0), pos2(r.right() - 9.0, r.center().y + 3.0)], Stroke::new(1.0, t.icon));
}

/// The label of a direction named in a sizing report.
fn air_label(name: &str) -> &'static str {
    match name {
        "extract" => "Extract",
        "fresh" => "Fresh air",
        "exhaust" => "Exhaust air",
        _ => "Supply",
    }
}

/// What the Design Charts row says, in the HVAC and Architecture tabs.
const CHARTS_TIP: &str = "The tables and rules the sizing tools apply: refrigerant links, water pipes, drains, ducts.";

/// The first row of the HVAC tab: the design charts window (also in the Help menu).
/// The rows of the Equipment group: a row for each kind, then the distribution of indoor units.
fn equipment_rows(
    ui: &mut egui::Ui,
    current: cadcraft_doc::EquipmentKind,
    chosen: &mut Option<cadcraft_doc::EquipmentKind>,
    distribute: &mut Option<bool>,
) {
    for kind in cadcraft_doc::EquipmentKind::ALL {
        if row_button(ui, crate::i18n::t(equipment_kind_label(kind)), kind == current, |p, r| thumbs::paint_equipment(p, r, kind)) {
            *chosen = Some(kind);
        }
    }
    if distribute_row(ui, "Distribute Units in a Space", "Spreads indoor units (cassettes, ducted, wall, ceiling or floor units) evenly in a space.")
    {
        *distribute = Some(true);
    }
}

/// A row of the distribution tools: its tip, and the help window ("How the distribution works") that its "i" opens.
fn distribute_row(ui: &mut egui::Ui, label: &str, tip: &str) -> bool {
    row_tip(ui, crate::i18n::t(label), crate::i18n::t(tip), Some("Distribute in a Space"), false, thumbs::paint_distribute)
}

/// The Distribute option of AIRTERMINALADD (air terminals) or of EQUIPMENTADD (`units`: an indoor unit, the kind in use or else a cassette), then the
/// space and the count.
fn start_distribution(app: &mut CadApp, units: bool) {
    use cadcraft_doc::EquipmentKind::{Chiller, SplitOutdoor, VrfOutdoor};
    let last = &mut app.session.last_used.equipment;
    if units && matches!(last.kind, Chiller | VrfOutdoor | SplitOutdoor) {
        *last = cadcraft_engine::cmd::EquipmentLastUsed { kind: cadcraft_doc::EquipmentKind::Cassette, model: String::new() };
    }
    app.cmd.buffer.clear();
    app.start(if units { "equipmentadd" } else { "airterminaladd" });
    app.cmdline("Distribute");
}

fn charts_row(ui: &mut egui::Ui, start: &mut Option<&'static str>) {
    if row_tip(ui, crate::i18n::t("Design Charts"), crate::i18n::t(CHARTS_TIP), None, false, thumbs::paint_quantities) {
        *start = Some("ui.charts");
    }
}

fn hvac(app: &mut CadApp, ui: &mut egui::Ui) {
    let current = app.session.last_used.air_terminal.kind;
    let app_air = app.session.last_used.air_terminal.air;
    let mut outdoor: Option<AirDirection> = None;
    let mut start: Option<&'static str> = None;
    let mut terminal: Option<TerminalKind> = None;
    let mut distribute: Option<bool> = None;
    let mut size = false;
    let mut connect = false;
    let unit = app.session.last_used.ahu.kind;
    let mut chosen_unit: Option<AhuKind> = None;
    let accessory = app.session.last_used.accessory.kind;
    let mut chosen_accessory: Option<AccessoryKind> = None;
    let equipment = app.session.last_used.equipment.kind;
    let mut chosen_equipment: Option<cadcraft_doc::EquipmentKind> = None;
    let pipe_now = (app.session.last_used.pipe.service, app.session.last_used.pipe.leg);
    let mut chosen_pipe: Option<(PipeService, PipeLeg)> = None;
    let part_now = app.session.last_used.part.model.clone();
    let mut chosen_part: Option<&'static str> = None;
    let mut size_pipes = false;
    let mut name_networks = false;
    let mut check_refrigerant = false;
    let sections = std::mem::take(&mut app.ui.sections);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        ui.data_mut(|d| {
            d.insert_temp(sections_id(), sections);
            d.insert_temp(open_id(), true);
        });
        charts_row(ui, &mut start);
        header(ui, "Spaces");
        if row_button(ui, crate::i18n::t("Space"), false, paint_space) {
            start = Some("spaceadd");
        }
        if row_button(ui, crate::i18n::t("Edit Space"), false, thumbs::paint_edit_space) {
            start = Some("spaceedit");
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Air Terminals");
        for (kind, label) in [
            (TerminalKind::SquareCeiling, "Square ceiling diffuser"),
            (TerminalKind::Round, "Round diffuser"),
            (TerminalKind::Rectangular, "Rectangular grille"),
            (TerminalKind::Linear, "Slot diffuser"),
            (TerminalKind::Extract, "Extract valve"),
        ] {
            if row_button(ui, crate::i18n::t(label), kind == current, |p, r| paint_terminal(p, r, kind)) {
                terminal = Some(kind);
            }
        }
        for (air, label) in [(AirDirection::Fresh, "Fresh air louvre"), (AirDirection::Exhaust, "Exhaust louvre")] {
            if row_button(ui, crate::i18n::t(label), current == TerminalKind::Louvre && app_air == air, |p, r| {
                paint_terminal(p, r, TerminalKind::Louvre)
            }) {
                outdoor = Some(air);
            }
        }
        if distribute_row(ui, "Distribute in a Space", "Spreads air terminals evenly in a space, each with its share of the space's flow.") {
            distribute = Some(false);
        }
        if row_button(ui, crate::i18n::t("Edit Air Terminal"), false, thumbs::paint_edit_terminal) {
            start = Some("airterminaledit");
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Ducts");
        for (kind, label) in [
            (AhuKind::DoubleFlow, "Double-flow unit"),
            (AhuKind::SupplyUnit, "Supply unit"),
            (AhuKind::ExtractUnit, "Extract unit"),
            (AhuKind::VmcBox, "Ventilation box"),
            (AhuKind::StackedDoubleFlow, "Double-flow unit, vertical"),
        ] {
            if row_button(ui, crate::i18n::t(label), kind == unit, |p, r| thumbs::paint_ahu(p, r, kind)) {
                chosen_unit = Some(kind);
            }
        }
        // In the order they are drawn: the riser, the ducts, the height zones, then terminals are connected, ducts sized and crossings settled.
        if row_tip(ui, crate::i18n::t("Riser"), crate::i18n::t("Draw the outline of the shaft where the air goes up or down. Type the flow that comes from the other floors on it; sizing gives it its section."), Some("Riser"), false, thumbs::paint_riser) {
            start = Some("riseradd");
        }
        if row_tip(ui, crate::i18n::t("Duct"), crate::i18n::t("Click the points of a run (one duct per segment), seen from above. Leave the direction on auto: sizing finds supply or extract from the terminals the run reaches."), Some("Duct"), false, thumbs::paint_duct) {
            start = Some("ductadd");
        }
        if row_tip(ui, crate::i18n::t("Height Zone"), crate::i18n::t("Draw an outline where ducts have less height (a beam, a low ceiling), seen from above, then type the height available in mm. Ducts crossing it are cut at its edges and sized flat inside. An altitude is optional."), Some("Height Zone"), false, thumbs::paint_zone) {
            start = Some("zoneadd");
        }
        if row_tip(ui, crate::i18n::t("Connect Terminals"), crate::i18n::t("Draws a flexible duct from each terminal that no duct reaches to the nearest duct (3 m at most)."), Some("Connect Terminals"), false, thumbs::paint_connect) {
            connect = true;
        }
        if row_tip(ui, crate::i18n::t("Size Ducts"), crate::i18n::t("Sizes every network from its terminals (flows, section, velocity) and writes the result on the ducts. Run it again after any change."), Some("Size Ducts"), false, thumbs::paint_size) {
            size = true;
        }
        if row_tip(ui, crate::i18n::t("Duct Crossing"), crate::i18n::t("Click two ducts that cross: one passes under or over the other, drawn broken or with four 45° elbows."), Some("Duct Crossing"), false, thumbs::paint_crossing) {
            start = Some("ductcross");
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Duct Accessories");
        for (kind, label) in [
            (AccessoryKind::Damper, "Damper"),
            (AccessoryKind::FireDamper, "Fire damper"),
            (AccessoryKind::Silencer, "Silencer"),
            (AccessoryKind::FlexConnector, "Flexible connector"),
        ] {
            if row_button(ui, crate::i18n::t(label), kind == accessory, |p, r| thumbs::paint_accessory(p, r, kind)) {
                chosen_accessory = Some(kind);
            }
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Equipment");
        equipment_rows(ui, equipment, &mut chosen_equipment, &mut distribute);
        ui.add_space(6.0);
        separator(ui);
        header_with(ui, "Piping", Some("Piping"));
        for &(label, service, leg) in PIPES {
            let selected = pipe_now.0 == service && (!service.has_legs() || pipe_now.1 == leg);
            if row_button(ui, crate::i18n::t(label), selected, |p, r| thumbs::paint_pipe(p, r, service, leg)) {
                chosen_pipe = Some((service, leg));
            }
        }
        if row_tip(
            ui,
            crate::i18n::t("Pipe Riser"),
            crate::i18n::t("The shaft where the pipes of a floor go up or down, with the pipes it holds (chilled water, hot water, change-over, condensate, refrigerant links). The pipes of the floor that end in the shaft, or start in it, are read through it."),
            Some("Pipe Riser"),
            false,
            thumbs::paint_pipe_riser,
        ) {
            start = Some("piperiseradd");
        }
        if row_button(ui, crate::i18n::t("Edit Pipe Riser"), false, thumbs::paint_edit_pipe_riser) {
            start = Some("piperiseredit");
        }
        if row_button(ui, crate::i18n::t("Edit Pipe"), false, thumbs::paint_edit_pipe) {
            start = Some("pipeedit");
        }
        if row_tip(
            ui,
            crate::i18n::t("Size Pipes"),
            crate::i18n::t("Mark the run that starts at the source of each network, type the load (kW; l/h for a drain) at the free ends, then size: the flow comes from the load, the size from the flow."),
            Some("Size Pipes"),
            false,
            thumbs::paint_size_pipes,
        ) {
            size_pipes = true;
        }
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Pipe Networks");
        if row_tip(
            ui,
            crate::i18n::t("Name Networks"),
            crate::i18n::t("Names every pipe network that has no name (CHW1, HW1, COND1…) and gives each its own colour, so that two networks of the same service can be told apart on the plan. A name typed on a pipe in Properties is the name of its whole network."),
            Some("Name Networks"),
            false,
            thumbs::paint_name_networks,
        ) {
            name_networks = true;
        }
        if row_tip(
            ui,
            crate::i18n::t("Network Legend"),
            crate::i18n::t("Puts a legend on the plan: a row for each network, in its colour and line type, with its name, its pipes and their length. It is redrawn when a network is named, renamed or restyled."),
            Some("Network Legend"),
            false,
            thumbs::paint_network_legend,
        ) {
            start = Some("networklegend");
        }
        ui.add_space(6.0);
        separator(ui);
        parts_section(ui, &part_now, &mut chosen_part, &mut start, &mut check_refrigerant);
        ui.add_space(6.0);
        separator(ui);
        header(ui, "Quantities");
        if row_tip(ui, crate::i18n::t("HVAC Quantities"), crate::i18n::t("Counts the ducts, fittings, terminals, units, risers and equipment of this drawing (one drawing per level), and saves a spreadsheet."), Some("HVAC Quantities"), false, thumbs::paint_quantities) {
            start = Some("hvactakeoff");
        }
    });
    // The state of the sections goes back to the preferences, and the rows of the next tab are drawn.
    app.ui.sections = ui
        .data_mut(|d| {
            d.insert_temp(open_id(), true);
            d.remove_temp::<crate::sections::Sections>(sections_id())
        })
        .unwrap_or_default();
    hvachelp::show(ui.ctx());
    if let Some(kind) = terminal {
        // The type clicked becomes AIRTERMINALADD's, at the smallest size for the flow.
        let last = &mut app.session.last_used.air_terminal;
        last.kind = kind;
        last.model.clear();
        last.air = if kind == TerminalKind::Extract { AirDirection::Extract } else { AirDirection::Supply };
        app.cmd.buffer.clear();
        app.start("airterminaladd");
    } else if let Some(air) = outdoor {
        // A louvre on the outside: fresh air comes in through it, exhaust air goes out.
        let last = &mut app.session.last_used.air_terminal;
        (last.kind, last.air) = (TerminalKind::Louvre, air);
        last.model.clear();
        app.cmd.buffer.clear();
        app.start("airterminaladd");
    } else if let Some(units) = distribute {
        start_distribution(app, units);
    } else if let Some(kind) = chosen_unit {
        // The type clicked becomes AHUADD's, then it starts with it.
        app.session.last_used.ahu.kind = kind;
        app.session.last_used.ahu.size = None;
        app.cmd.buffer.clear();
        app.start("ahuadd");
    } else if let Some((service, leg)) = chosen_pipe {
        // The service clicked becomes PIPEADD's, at its own size, then it starts with it.
        let last = &mut app.session.last_used.pipe;
        (last.service, last.leg, last.dn_mm) = (service, leg, None);
        app.cmd.buffer.clear();
        app.start("pipeadd");
    } else if let Some(name) = chosen_part {
        // The part clicked becomes PIPEPARTADD's model, then it starts with it.
        app.session.last_used.part.model = name.to_string();
        app.cmd.buffer.clear();
        app.start("pipepartadd");
    } else if let Some(kind) = chosen_equipment {
        // The kind clicked becomes EQUIPMENTADD's, at its first model, then it starts with it.
        app.session.last_used.equipment = cadcraft_engine::cmd::EquipmentLastUsed { kind, model: String::new() };
        app.cmd.buffer.clear();
        app.start("equipmentadd");
    } else if let Some(kind) = chosen_accessory {
        // The type clicked becomes DUCTACCESSORYADD's, then it starts with it.
        app.session.last_used.accessory.kind = kind;
        app.cmd.buffer.clear();
        app.start("ductaccessoryadd");
    } else if connect {
        // DUCTCONNECT asks nothing: it runs at once and says what it did.
        match app.run("ductconnect", serde_json::json!({})) {
            Ok(r) => {
                let n = r["created"].as_array().map_or(0, Vec::len);
                app.session.echo(crate::tf!("Terminals connected: {n}", n = n));
                for k in r["skipped"].as_array().into_iter().flatten() {
                    app.session.echo(format!("{}: {}", k["terminal"].as_str().unwrap_or(""), k["reason"].as_str().unwrap_or("")));
                }
            }
            Err(e) => app.session.echo(e.to_string()),
        }
    } else if check_refrigerant {
        // REFRIGERANTCHECK asks nothing: it runs at once and says what it found.
        match app.run("refrigerantcheck", serde_json::json!({})) {
            Ok(r) => echo_refrigerant(app, &r),
            Err(e) => app.session.echo(e.to_string()),
        }
    } else if name_networks {
        // PIPENETWORK asks nothing: it runs at once and says the networks it found.
        match app.run("pipenetwork", serde_json::json!({})) {
            Ok(r) => echo_networks(app, &r),
            Err(e) => app.session.echo(e.to_string()),
        }
    } else if size_pipes {
        // PIPESIZE asks nothing: it runs at once and says what it did.
        match app.run("pipesize", serde_json::json!({})) {
            Ok(r) => {
                for net in r["networks"].as_array().into_iter().flatten() {
                    let load = net["totalLoad"].as_f64().unwrap_or(0.0);
                    let n = net["runs"].as_array().map_or(0, Vec::len);
                    let loss = net["totalLoss"].as_f64().unwrap_or(0.0);
                    app.session.echo(crate::tf!(
                        "Pipes sized: {n} run(s), load {load}, pressure loss {loss} kPa",
                        n = n,
                        load = format!("{load:.1}"),
                        loss = format!("{loss:.1}")
                    ));
                    for w in net["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
                        app.session.echo(w.to_string());
                    }
                }
                for w in r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
                    app.session.echo(w.to_string());
                }
            }
            Err(e) => app.session.echo(e.to_string()),
        }
    } else if size {
        // DUCTSIZE asks nothing: it runs at once and says what it did.
        match app.run("ductsize", serde_json::json!({})) {
            Ok(r) => {
                for net in r["networks"].as_array().into_iter().flatten() {
                    let (flow, pa) = (net["totalFlow"].as_f64().unwrap_or(0.0), net["requiredPressure"].as_f64().unwrap_or(0.0));
                    let air = crate::i18n::t(air_label(net["air"].as_str().unwrap_or("")));
                    app.session.echo(crate::tf!(
                        "Ducts sized, {air}: {flow} m³/h, {pa} Pa",
                        air = air,
                        flow = format!("{flow:.0}"),
                        pa = format!("{pa:.0}")
                    ));
                    for w in net["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
                        app.session.echo(w.to_string());
                    }
                }
                app.session.echo(crate::tf!(
                    "Ducts reached from a unit: {reached} of {ducts}",
                    reached = r["reached"].as_u64().unwrap_or(0),
                    ducts = r["ducts"].as_u64().unwrap_or(0)
                ));
                for b in r["balance"].as_array().into_iter().flatten() {
                    app.session.echo(crate::tf!(
                        "Unbalanced unit: {a} {fa} m³/h against {b} {fb} m³/h",
                        a = crate::i18n::t(air_label(b["left"].as_str().unwrap_or(""))),
                        fa = format!("{:.0}", b["leftFlow"].as_f64().unwrap_or(0.0)),
                        b = crate::i18n::t(air_label(b["right"].as_str().unwrap_or(""))),
                        fb = format!("{:.0}", b["rightFlow"].as_f64().unwrap_or(0.0))
                    ));
                }
                for sp in r["spaces"].as_array().into_iter().flatten() {
                    app.session.echo(crate::tf!(
                        "{name} ({air}): the terminals carry {supplied} m³/h, the space needs {needed} m³/h",
                        name = sp["space"].as_str().unwrap_or(""),
                        air = if sp["air"] == "extract" { crate::i18n::t("Extract") } else { crate::i18n::t("Supply") },
                        supplied = format!("{:.0}", sp["supplied"].as_f64().unwrap_or(0.0)),
                        needed = format!("{:.0}", sp["needed"].as_f64().unwrap_or(0.0))
                    ));
                }
            }
            Err(e) => app.session.echo(e.to_string()),
        }
    } else if let Some(cmd) = start {
        app.cmd.buffer.clear();
        app.start(cmd);
    }
}

/// An opening the Architecture tab offers: its label, kind, drawing style, and width, height and sill, mm.
type Preset = (&'static str, OpeningKind, OpeningStyle, (f64, f64, f64));

const DOORS: &[Preset] = &[
    ("Single door", OpeningKind::Door, OpeningStyle::Standard, (900.0, 2100.0, 0.0)),
    ("Double door", OpeningKind::DoubleDoor, OpeningStyle::Standard, (1400.0, 2100.0, 0.0)),
    ("Exterior single door", OpeningKind::Door, OpeningStyle::Standard, (1000.0, 2100.0, 0.0)),
    ("Exterior double door", OpeningKind::DoubleDoor, OpeningStyle::Standard, (1800.0, 2100.0, 0.0)),
    ("Door-window block", OpeningKind::DoubleDoor, OpeningStyle::Standard, (1800.0, 2200.0, 0.0)),
    ("Sliding door", OpeningKind::Door, OpeningStyle::Sliding, (1800.0, 2100.0, 0.0)),
    ("Exterior sliding double door", OpeningKind::DoubleDoor, OpeningStyle::Sliding, (2400.0, 2100.0, 0.0)),
    ("Pocket sliding door", OpeningKind::Door, OpeningStyle::Pocket, (900.0, 2100.0, 0.0)),
    ("Folding single door", OpeningKind::Door, OpeningStyle::Folding, (800.0, 2100.0, 0.0)),
    ("Folding double door", OpeningKind::DoubleDoor, OpeningStyle::Folding, (1600.0, 2100.0, 0.0)),
    ("Pass-through (rectangle)", OpeningKind::Bay, OpeningStyle::Standard, (1000.0, 2100.0, 0.0)),
    ("Tilting door", OpeningKind::Door, OpeningStyle::Tilting, (2400.0, 2100.0, 0.0)),
    ("Elevator door (centre)", OpeningKind::Door, OpeningStyle::CentreParting, (1100.0, 2100.0, 0.0)),
    ("Elevator door (side)", OpeningKind::Door, OpeningStyle::Sliding, (1100.0, 2100.0, 0.0)),
    ("Revolving door", OpeningKind::Door, OpeningStyle::Revolving, (2000.0, 2200.0, 0.0)),
];

const WINDOWS: &[Preset] = &[
    ("Window", OpeningKind::Window, OpeningStyle::Standard, (1200.0, 1200.0, 900.0)),
    ("Wide window", OpeningKind::Window, OpeningStyle::Standard, (2400.0, 1200.0, 900.0)),
    ("Casement window, 1 leaf", OpeningKind::Window, OpeningStyle::Casement, (700.0, 1200.0, 900.0)),
    ("Casement window, 2 leaves", OpeningKind::Window, OpeningStyle::DoubleCasement, (1200.0, 1200.0, 900.0)),
    ("Sliding window", OpeningKind::Window, OpeningStyle::Sliding, (1500.0, 1200.0, 900.0)),
    ("Fixed window", OpeningKind::Window, OpeningStyle::Fixed, (1000.0, 1200.0, 900.0)),
    ("Roof window", OpeningKind::Window, OpeningStyle::Skylight, (780.0, 980.0, 0.0)),
];

fn architecture(app: &mut CadApp, ui: &mut egui::Ui) {
    let styles = app.session.doc().map(|d| d.available_wall_styles()).unwrap_or_default();
    let current = app.session.last_used.wall_style.clone();
    let mut draw: Option<String> = None;
    let mut start: Option<&'static str> = None;
    let mut cleanup = false;
    let mut cleanup_structure = false;
    let mut takeoff_structure = false;
    let mut design_start: Option<&'static str> = None;
    let last = app.session.last_used.opening.clone();
    let mut chosen: Option<Preset> = None;
    let mut sections = std::mem::take(&mut app.ui.sections);
    let app_slab_kind = app.session.last_used.slab.kind;
    let mut slab_chosen: Option<cadcraft_doc::SlabKind> = None;
    // The section new columns and beams take, and the one chosen in the Structure group this frame.
    let struct_section = app.session.last_used.structure.section.clone();
    let mut struct_chosen: Option<String> = None;
    let mut struct_start: Option<&'static str> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        // A section opens or folds as the Options say and the user left it ([`crate::sections`]); a click on its header changes it.
        let mut section = |ui: &mut egui::Ui, title: &str, _open: bool, body: &mut dyn FnMut(&mut egui::Ui)| {
            let key = format!("Architecture/{title}");
            let open = sections.is_open(&key);
            let shown = egui::CollapsingHeader::new(crate::i18n::t(title)).id_salt(title).open(Some(open)).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                body(ui);
            });
            if shown.header_response.clicked() {
                sections.set(&key, !open);
            }
        };
        section(ui, "Walls", true, &mut |ui| {
            for (i, st) in styles.iter().enumerate() {
                let selected = st.name.eq_ignore_ascii_case(&current) || (current.is_empty() && i == 0);
                if row_button(ui, crate::i18n::t(&st.name), selected, |p, r| paint_style(p, r, st)) {
                    draw = Some(st.name.clone());
                }
            }
        });
        for (title, list) in [("Doors", DOORS), ("Windows", WINDOWS)] {
            section(ui, title, true, &mut |ui| {
                for &(label, kind, style, size) in list {
                    let selected = last.kind == kind && last.style == style && last.size.unwrap_or_else(|| kind.default_size_mm()) == size;
                    if row_button(ui, crate::i18n::t(label), selected, |p, r| thumbs::paint_opening(p, r, kind, style, size)) {
                        chosen = Some((label, kind, style, size));
                    }
                }
            });
        }
        section(ui, "Slabs", true, &mut |ui| {
            let last = app_slab_kind;
            for (label, kind) in SLABS {
                if row_button(ui, crate::i18n::t(label), last == *kind, |p, r| thumbs::paint_slab(p, r, *kind)) {
                    slab_chosen = Some(*kind);
                }
            }
        });
        section(ui, "Design", true, &mut |ui| {
            for (label, cmd, paint) in [
                ("Ceiling Grid", "ceilingadd", thumbs::paint_ceiling_grid as fn(&egui::Painter, Rect)),
                ("Stair", "stairadd", thumbs::paint_stair),
                ("Railing", "railadd", thumbs::paint_railing),
                ("Curtain Wall", "curtainadd", thumbs::paint_curtain_wall),
                ("Roof", "roofadd", thumbs::paint_roof),
            ] {
                if row_tip(ui, crate::i18n::t(label), "", Some(label), false, paint) {
                    design_start = Some(cmd);
                }
            }
        });
        section(ui, "Structure", true, &mut |ui| {
            if let Some(chosen) = section_picker(ui, &struct_section) {
                struct_chosen = Some(chosen);
            }
            if row_tip(ui, crate::i18n::t("Column"), "", Some("Column"), false, thumbs::paint_column) {
                struct_start = Some("columnadd");
            }
            if row_tip(ui, crate::i18n::t("Beam"), "", Some("Beam"), false, thumbs::paint_beam) {
                struct_start = Some("beamadd");
            }
            if row_tip(ui, crate::i18n::t("Edit Structure"), "", Some("Edit Structure"), false, thumbs::paint_edit_structure) {
                struct_start = Some("structedit");
            }
            if row_tip(ui, crate::i18n::t("Structure Joints"), "", Some("Structure Joints"), false, thumbs::paint_junctions) {
                cleanup_structure = true;
            }
            if row_tip(ui, crate::i18n::t("Structure Takeoff"), "", Some("Structure Takeoff"), false, thumbs::paint_struct_takeoff) {
                takeoff_structure = true;
            }
        });
        section(ui, "Tools", true, &mut |ui| {
            if row_tip(ui, crate::i18n::t("Corner Window"), "", Some("Corner Window"), false, thumbs::paint_corner_window) {
                start = Some("cornerwindow");
            }
            if row_button(ui, crate::i18n::t("Edit Slab"), false, thumbs::paint_edit_slab) {
                start = Some("slabedit");
            }
            if row_tip(ui, crate::i18n::t("Slab Opening"), "", Some("Slab Opening"), false, thumbs::paint_slab_opening) {
                start = Some("slabhole");
            }
            if row_tip(ui, crate::i18n::t("Wall Elevation"), "", Some("Wall Elevation"), false, thumbs::paint_elevation) {
                start = Some("wallelevation");
            }
            if row_tip(ui, crate::i18n::t("Convert to Openings"), "", Some("Convert to Openings"), false, thumbs::paint_convert) {
                start = Some("openingconvert");
            }
            if row_button(ui, crate::i18n::t("Edit Wall"), false, thumbs::paint_edit_wall) {
                start = Some("walledit");
            }
            if row_button(ui, crate::i18n::t("Wall Styles"), false, thumbs::paint_wall_styles) {
                start = Some("wallstyleedit");
            }
            if row_tip(ui, crate::i18n::t("Wall Junctions"), "", Some("Wall Junctions"), false, thumbs::paint_junctions) {
                cleanup = true;
            }
        });
    });
    app.ui.sections = sections;
    hvachelp::show(ui.ctx());
    if let Some(sec) = struct_chosen {
        app.session.last_used.structure.section = sec;
    }
    if cleanup_structure {
        // STRUCTCLEANUP asks nothing: it cuts the beams at once and says how many.
        match app.run("structcleanup", serde_json::json!({})) {
            Ok(r) => {
                let n = r["regenerated"].as_u64().unwrap_or(0);
                app.session.echo(crate::tf!("Beams cut: {n} redrawn", n = n));
            }
            Err(e) => app.session.echo(e.to_string()),
        }
    }
    if takeoff_structure {
        // STRUCTTAKEOFF asks nothing: it adds up the steel and the slabs and says what it found.
        match app.run("structtakeoff", serde_json::json!({})) {
            Ok(r) => {
                let count = |k: &str| r[k].as_array().map_or(0, |v| v.iter().filter_map(|q| q["count"].as_u64()).sum::<u64>());
                app.session.echo(crate::tf!(
                    "Structure: {c} columns, {b} beams, {kg} kg of steel; {s} slabs, {a} m², {v} m³",
                    c = count("columns"),
                    b = count("beams"),
                    kg = format!("{:.0}", r["steelKg"].as_f64().unwrap_or(0.0)),
                    s = count("slabs"),
                    a = format!("{:.1}", r["slabAreaM2"].as_f64().unwrap_or(0.0)),
                    v = format!("{:.1}", r["slabVolumeM3"].as_f64().unwrap_or(0.0))
                ));
            }
            Err(e) => app.session.echo(e.to_string()),
        }
    }
    if let Some(cmd) = design_start {
        app.cmd.buffer.clear();
        app.start(cmd);
    } else if let Some(cmd) = struct_start {
        app.cmd.buffer.clear();
        app.start(cmd);
    } else if let Some(name) = draw {
        // The style clicked becomes WALLADD's style, then WALLADD starts with it.
        app.session.last_used.wall_style = name;
        app.cmd.buffer.clear();
        app.start("walladd");
    } else if let Some((_, kind, style, size)) = chosen {
        // The opening clicked becomes OPENINGADD's kind, style and size, then it starts with them.
        let last = &mut app.session.last_used.opening;
        (last.kind, last.style, last.size) = (kind, style, Some(size));
        app.cmd.buffer.clear();
        app.start("openingadd");
    } else if let Some(kind) = slab_chosen {
        // The kind clicked becomes SLABADD's kind (and its own thickness), then it starts.
        app.session.last_used.slab.kind = kind;
        app.session.last_used.slab.thickness_mm = None;
        app.cmd.buffer.clear();
        app.start("slabadd");
    } else if let Some(cmd) = start {
        app.cmd.buffer.clear();
        app.start(cmd);
    } else if cleanup {
        // WALLCLEANUP asks nothing: it runs at once and says what it did.
        match app.run("wallcleanup", serde_json::json!({})) {
            Ok(r) => {
                let n = r["regenerated"].as_u64().unwrap_or(0);
                app.session.echo(crate::tf!("Walls joined: {n} redrawn", n = n));
            }
            Err(e) => app.session.echo(e.to_string()),
        }
    }
}

/// The section chooser of the Structure group: the family and the size, as two lists. Gives the designation chosen this frame ("IPN 200").
fn section_picker(ui: &mut egui::Ui, current: &str) -> Option<String> {
    use cadcraft_hvac::{SectionFamily, parse_section, sections_of};
    let parsed = parse_section(current);
    let family = parsed.map_or(SectionFamily::Hea, |s| s.family);
    let mut chosen: Option<String> = None;
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let name = ui.label(crate::i18n::t("Section"));
        let family_box = craft_ui::ComboBox::from_id_salt("struct_family").width(70.0).selected_text(family.name()).show_ui(ui, |ui| {
            for f in SectionFamily::ALL {
                if ui.selectable_label(f == family, f.name()).clicked() && f != family {
                    // The size nearest to the current one in the new family.
                    let want = parsed.map_or(200, |s| s.number);
                    chosen = sections_of(f).min_by_key(|s| s.number.abs_diff(want)).map(cadcraft_hvac::SteelSection::designation);
                }
            }
        });
        family_box.response.labelled_by(name.id);
        let shown = parsed.map_or(String::from("-"), |s| s.number.to_string());
        let size_box = craft_ui::ComboBox::from_id_salt("struct_size").width(70.0).selected_text(shown).show_ui(ui, |ui| {
            for s in sections_of(family) {
                if ui.selectable_label(parsed.is_some_and(|p| p.number == s.number), format!("{} ({:.1} kg/m)", s.number, s.kg_m)).clicked() {
                    chosen = Some(s.designation());
                }
            }
        });
        size_box.response.labelled_by(name.id);
    });
    chosen
}

/// The slabs of the Slabs group: the label and the kind.
const SLABS: &[(&str, cadcraft_doc::SlabKind)] = &[
    ("Floor slab", cadcraft_doc::SlabKind::Floor),
    ("Roof slab", cadcraft_doc::SlabKind::Roof),
    ("Foundation slab", cadcraft_doc::SlabKind::Foundation),
];

/// The Pipework Parts group: the parts used most, the library, and Edit.
fn parts_section(ui: &mut egui::Ui, part_now: &str, chosen_part: &mut Option<&'static str>, start: &mut Option<&'static str>, check: &mut bool) {
    header(ui, "Pipework Parts");
    let fr = crate::i18n::current_language().starts_with("fr");
    for name in COMMON_PARTS {
        let Some(m) = cadcraft_doc::standard_part_model_named(name) else { continue };
        let params = m.params("T", cadcraft_doc::HvacUnits::default());
        let selected = part_now == m.name;
        if row_button(ui, if fr { m.name_fr } else { m.name }, selected, |p, r| thumbs::paint_part(p, r, &params)) {
            *chosen_part = Some(m.name);
        }
    }
    if row_tip(
        ui,
        crate::i18n::t("Library…"),
        crate::i18n::t(
            "All the valves, pumps, vessels, boilers, instruments and refrigerant branches and boxes: click one, then click where it goes, or drag it onto the drawing.",
        ),
        Some("Library…"),
        false,
        thumbs::paint_library,
    ) {
        *start = Some("pipepartmodels");
    }
    if row_button(ui, crate::i18n::t("Edit Pipework Part"), false, thumbs::paint_edit_part) {
        *start = Some("pipepartedit");
    }
    if row_tip(
        ui,
        crate::i18n::t("Check Refrigerant"),
        crate::i18n::t(
            "Follows the refrigerant pipes from each outdoor unit through the joints, headers and branch boxes to the indoor units, counts the units and the capacity index at each outlet, and tells what is over the limits.",
        ),
        Some("Check Refrigerant"),
        false,
        thumbs::paint_check_refrigerant,
    ) {
        *check = true;
    }
}

/// The parts the HVAC tab offers at once: what is used most; the rest is in the library.
const COMMON_PARTS: &[&str] = &[
    "Isolation valve (ball)",
    "Balancing valve (TA)",
    "2-way control valve",
    "3-way control valve",
    "Check valve",
    "Y strainer",
    "Automatic air vent",
    "Circulator (single)",
    "Expansion vessel 24 l",
    "Buffer tank 500 l",
    "Refrigerant Y joint, index below 200",
    "Refrigerant header, index below 290",
    "Branch box, 6 outlets (R32 heat recovery)",
];

/// The pipes of the Piping group: the label, the service and the leg.
const PIPES: &[(&str, PipeService, PipeLeg)] = &[
    ("Chilled water supply", PipeService::ChilledWater, PipeLeg::Supply),
    ("Chilled water return", PipeService::ChilledWater, PipeLeg::Return),
    ("Hot water supply", PipeService::HotWater, PipeLeg::Supply),
    ("Hot water return", PipeService::HotWater, PipeLeg::Return),
    ("Change-over supply", PipeService::ChangeOver, PipeLeg::Supply),
    ("Change-over return", PipeService::ChangeOver, PipeLeg::Return),
    ("Condensate drain", PipeService::Condensate, PipeLeg::Supply),
    ("Refrigerant liquid line", PipeService::RefrigerantLiquid, PipeLeg::Supply),
    ("Refrigerant gas line", PipeService::RefrigerantGas, PipeLeg::Supply),
    ("Refrigerant link (liquid + gas)", PipeService::RefrigerantPair, PipeLeg::Supply),
    ("Refrigerant link, heat recovery (3 tubes)", PipeService::RefrigerantTriple, PipeLeg::Supply),
];

/// The label of an equipment kind in the Tool Sets and in Properties.
pub(super) fn equipment_kind_label(k: cadcraft_doc::EquipmentKind) -> &'static str {
    use cadcraft_doc::EquipmentKind::{Cassette, Chiller, Ducted, FloorUnit, SplitOutdoor, VrfOutdoor, WallUnit};
    match k {
        Chiller => "Chiller",
        VrfOutdoor => "VRV outdoor unit",
        SplitOutdoor => "Split outdoor unit",
        Cassette => "Cassette",
        Ducted => "Ducted unit",
        WallUnit => "Wall unit",
        cadcraft_doc::EquipmentKind::CeilingUnit => "Ceiling unit",
        FloorUnit => "Floor unit",
    }
}

/// Says what the refrigerant check found, one line a network and one a warning.
fn echo_refrigerant(app: &mut CadApp, r: &serde_json::Value) {
    let networks = r["networks"].as_array().cloned().unwrap_or_default();
    if networks.is_empty() {
        app.session.echo(crate::tl!("No refrigerant network: draw an outdoor unit, its pipes and its indoor units.").to_string());
    }
    for n in &networks {
        let (units, index) = (n["units"].as_u64().unwrap_or(0), n["index"].as_f64().unwrap_or(0.0));
        app.session.echo(crate::tf!(
            "Refrigerant network: {units} indoor unit(s), capacity index {index}",
            units = units,
            index = format!("{index:.0}")
        ));
        for w in n["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
            app.session.echo(w.to_string());
        }
    }
    for w in r["warnings"].as_array().into_iter().flatten().filter_map(|w| w.as_str()) {
        app.session.echo(w.to_string());
    }
}

/// Says the networks of the drawing, one line each: name, colour, tags and length.
fn echo_networks(app: &mut CadApp, r: &serde_json::Value) {
    let networks = r["networks"].as_array().cloned().unwrap_or_default();
    if networks.is_empty() {
        app.session.echo(crate::tl!("No pipe network: draw pipes first.").to_string());
    }
    for n in &networks {
        let tags: Vec<&str> = n["services"].as_array().into_iter().flatten().filter_map(|t| t.as_str()).collect();
        app.session.echo(crate::tf!(
            "Network {name}: {tags}, {pipes} pipe(s), {length} m, colour {color}",
            name = n["name"].as_str().unwrap_or(""),
            tags = tags.join("/"),
            pipes = n["pipes"].as_u64().unwrap_or(0),
            length = format!("{:.0}", n["length"].as_f64().unwrap_or(0.0)),
            color = n["color"].as_str().unwrap_or("")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sections::SectionsMode;
    use crate::{CadApp, Services};
    use cadcraft_engine::Session;
    use egui_kittest::kittest::Queryable;

    fn tab(app: CadApp, name: &'static str) -> egui_kittest::Harness<'static, CadApp> {
        let mut h =
            egui_kittest::Harness::builder().with_size(vec2(400.0, 2400.0)).build_ui_state(move |ui, app: &mut CadApp| body(app, ui, name), app);
        h.run_steps(3);
        h
    }

    fn app(mode: SectionsMode) -> CadApp {
        let mut app = CadApp::new(Session::new(), Services::default());
        app.ui.sections.set_mode(mode);
        app
    }

    #[test]
    fn the_sections_of_the_hvac_tab_are_all_open_or_all_closed_as_the_options_say() {
        let open = tab(app(SectionsMode::Open), "HVAC");
        assert!(open.query_by_label("Edit Space").is_some() && open.query_by_label("Edit Pipe Riser").is_some(), "every row is drawn");
        let closed = tab(app(SectionsMode::Closed), "HVAC");
        assert!(closed.query_by_label("Edit Space").is_none() && closed.query_by_label("Edit Pipe Riser").is_none(), "no row is drawn");
        assert!(closed.query_by_label("Spaces").is_some(), "the headers are");
    }

    #[test]
    fn a_click_on_a_header_folds_its_rows_and_the_last_use_is_remembered_between_runs() {
        let mut hn = tab(app(SectionsMode::Remember), "HVAC");
        assert!(hn.query_by_label("Edit Space").is_some(), "open at first");
        hn.get_by_label("Spaces").click();
        hn.run_steps(3);
        assert!(hn.query_by_label("Edit Space").is_none(), "folded");
        assert!(hn.query_by_label("Edit Air Terminal").is_some(), "the others are not");
        // Kept with the preferences, and back in the next run.
        let prefs = hn.state().prefs_json();
        let mut next = CadApp::new(Session::new(), Services::default());
        next.load_prefs(&prefs);
        assert_eq!(next.ui.sections.mode, SectionsMode::Remember);
        let hn2 = tab(next, "HVAC");
        assert!(hn2.query_by_label("Edit Space").is_none() && hn2.query_by_label("Edit Air Terminal").is_some());
        // Opened again.
        let mut hn = hn2;
        hn.get_by_label("Spaces").click();
        hn.run_steps(3);
        assert!(hn.query_by_label("Edit Space").is_some());
    }

    #[test]
    fn the_architecture_sections_follow_the_same_choice_and_a_click_changes_them() {
        let closed = tab(app(SectionsMode::Closed), "Architecture");
        assert!(closed.query_by_label("Edit Wall").is_none(), "the tools are folded");
        let mut hn = tab(app(SectionsMode::Open), "Architecture");
        assert!(hn.query_by_label("Edit Wall").is_some());
        hn.get_by_label("Tools").click();
        hn.run_steps(3);
        assert!(hn.query_by_label("Edit Wall").is_none(), "folded by a click");
        // Changing the choice applies it at once.
        hn.state_mut().ui.sections.set_mode(SectionsMode::Open);
        hn.run_steps(3);
        assert!(hn.query_by_label("Edit Wall").is_some());
    }

    #[test]
    fn the_structure_group_lists_the_columns_and_beams_and_a_click_starts_the_command_with_the_section() {
        let mut hn = tab(app(SectionsMode::Open), "Architecture");
        for label in ["Structure", "Column", "Beam", "Edit Structure", "Structure Joints"] {
            assert!(hn.query_by_label(label).is_some(), "{label}");
        }
        // The section chooser: its label and the two lists it names.
        assert!(hn.get_all_by_label("Section").count() >= 1);
        hn.get_by_label("Column").click();
        hn.run_steps(3);
        assert!(hn.state().session.running.is_some(), "the command is running");
        // The section new columns take is the one kept by the session.
        assert_eq!(hn.state().session.last_used.structure.section, "HEA 200");
    }

    #[test]
    fn the_design_group_starts_the_ceiling_stair_and_railing_commands() {
        let mut hn = tab(app(SectionsMode::Open), "Architecture");
        for label in ["Design", "Ceiling Grid", "Stair", "Railing"] {
            assert!(hn.query_by_label(label).is_some(), "{label}");
        }
        hn.get_by_label("Stair").click();
        hn.run_steps(3);
        assert!(hn.state().session.running.is_some(), "STAIRADD is running");
    }

    #[test]
    fn the_choice_is_set_and_read_through_the_options_command() {
        let mut app = CadApp::new(Session::new(), Services::default());
        assert_eq!(app.run("options", serde_json::json!({})).unwrap()["paletteSections"], "remember");
        app.run("options", serde_json::json!({"paletteSections": "closed"})).unwrap();
        assert_eq!(app.ui.sections.mode, SectionsMode::Closed);
        assert!(app.run("options", serde_json::json!({"paletteSections": "sometimes"})).is_err());
    }

    #[test]
    fn the_refrigerant_sizes_of_a_new_link_are_set_kept_and_used() {
        let mut app = CadApp::new(Session::new(), Services::default());
        let r = app.run("options", serde_json::json!({"refrigerantLiquid": "1/4", "refrigerantGas": "1/2", "refrigerantHpGas": 22.2})).unwrap();
        assert_eq!(
            (r["refrigerantLiquid"].as_f64(), r["refrigerantGas"].as_f64(), r["refrigerantHpGas"].as_f64()),
            (Some(6.35), Some(12.7), Some(22.2))
        );
        assert!(app.run("options", serde_json::json!({"refrigerantGas": "big"})).is_err());
        // Kept across restarts.
        let prefs = app.prefs_json();
        let mut again = CadApp::new(Session::new(), Services::default());
        again.load_prefs(&prefs);
        assert_eq!(again.session.last_used.pipe.liquid_mm, Some(6.35));
        // A new link starts with them.
        again.run("pipeadd", serde_json::json!({"points": [[0, 0], [3000, 0]], "service": "refrigerantPair"})).unwrap();
        let d = again.run("options", serde_json::json!({"refrigerantLiquid": null})).unwrap();
        assert!(d["refrigerantLiquid"].is_null());
    }

    #[test]
    fn distributing_units_in_a_space_starts_the_equipment_command_at_the_space_with_an_indoor_unit() {
        let mut app = app(SectionsMode::Open);
        // An outdoor unit in use becomes a cassette: only indoor units are spread.
        app.session.last_used.equipment.kind = cadcraft_doc::EquipmentKind::VrfOutdoor;
        let mut h = tab(app, "HVAC");
        h.get_by_label("Distribute Units in a Space").click();
        h.run_steps(3);
        let st = h.state();
        assert_eq!(st.session.last_used.equipment.kind, cadcraft_doc::EquipmentKind::Cassette);
        assert!(st.session.prompt_text().contains("Select space"), "{}", st.session.prompt_text());
    }

    #[test]
    fn the_design_charts_row_is_in_the_hvac_tab_only() {
        let hvac = tab(app(SectionsMode::Open), "HVAC");
        assert!(hvac.query_by_label("Design Charts").is_some());
        for other in ["Architecture", "Electricity"] {
            let t = tab(app(SectionsMode::Open), other);
            assert!(t.query_by_label("Design Charts").is_none(), "{other}");
        }
    }
}
