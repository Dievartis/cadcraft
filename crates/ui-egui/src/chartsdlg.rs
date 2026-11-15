//! The design charts window (Help menu, or the "Charts" row at the top of the HVAC and Architecture tabs): the
//! tables and the rules the sizing tools apply, so the logic of a calculation can be read and checked. Every number
//! comes from `cadcraft-hvac` (the tables, or the same functions the tools call), never from a copy.

use cadcraft_hvac::{
    BOX_LINK, DRAIN_PIPES, HP_BRANCH, HP_MAIN, HR_BRANCH, HR_MAIN, MIN_WALL, PipeCriteria, ROUND_DIAMETERS_MM, STEEL_PIPES, UNIT_LINK,
    WaterTemperature, drain_capacity_lph, max_flow_m3h, max_velocity_ms, round_area_m2, steel_inner_diameter_mm,
};
use egui::{RichText, vec2};

use crate::CadApp;

/// The window while open.
#[derive(Clone, Debug, Default)]
pub struct ChartsState {
    /// The chart shown, an index into [`pages`].
    pub page: usize,
    pub raise: bool,
}

/// A table: a title, a header row and rows of text.
pub struct Table {
    pub title: &'static str,
    pub head: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// A chart: what it is for, how the tool calculates, and its tables.
pub struct Page {
    pub name: &'static str,
    pub intro: &'static [&'static str],
    pub tables: Vec<Table>,
}

fn mm(v: f64) -> String {
    format!("{v}")
}

fn bound(v: f64) -> String {
    if v.is_finite() { format!("< {v}") } else { "≥ 920".into() }
}

fn table(title: &'static str, head: &[&str], rows: Vec<Vec<String>>) -> Table {
    Table { title, head: head.iter().map(|h| (*h).to_string()).collect(), rows }
}

fn refrigerant_pages() -> Vec<Page> {
    let hp_main = HP_MAIN.iter().map(|r| vec![format!("≤ {}", r.0), mm(r.1), mm(r.2)]).collect();
    let hp_branch = HP_BRANCH.iter().map(|r| vec![bound(r.0), mm(r.1), mm(r.2)]).collect();
    let unit = UNIT_LINK.iter().map(|r| vec![format!("≤ {}", r.0), mm(r.1), mm(r.2)]).collect();
    let boxes = BOX_LINK.iter().map(|r| vec![format!("≤ {}", r.0), mm(r.1), mm(r.2)]).collect();
    let hr_main = HR_MAIN.iter().map(|r| vec![format!("≤ {}", r.0), mm(r.1), mm(r.2), mm(r.3)]).collect();
    let hr_branch = HR_BRANCH.iter().map(|r| vec![bound(r.0), mm(r.1), mm(r.2), mm(r.3)]).collect();
    let wall = MIN_WALL.iter().map(|r| vec![format!("≤ {}", r.0), format!("{:.2}", r.1)]).collect();
    vec![
        Page {
            name: "Refrigerant, heat pump (2 pipes)",
            intro: &[
                "Two pipes: liquid and gas, copper, outer diameters in mm (inch sizes are the same pipes: 6.4 = 1/4, 9.5 = 3/8, 12.7 = 1/2, 15.9 = 5/8, 19.1 = 3/4).",
                "A, B, C: from the outdoor unit to the first branch, by the total capacity of the outdoor units (HP).",
                "D: between branches, by the capacity index of everything downstream (about ten times the cooling kW); never larger than the pipe A.",
                "E: into one indoor unit, the connection size of that unit. F, G: a branch-selector box and the units behind it.",
            ],
            tables: vec![
                table("A, B, C: outdoor unit to first branch", &["Outdoor HP", "Liquid", "Gas"], hp_main),
                table("D: between branches", &["Index downstream", "Liquid", "Gas"], hp_branch),
                table("E: into an indoor unit", &["Unit index", "Liquid", "Gas"], unit),
                table("F: to a branch-selector box", &["Index behind the box", "Liquid", "Gas"], boxes),
            ],
        },
        Page {
            name: "Refrigerant, heat recovery (3 pipes)",
            intro: &[
                "Three pipes: liquid, suction gas and high/low-pressure gas. The rules are those of the heat pump, with the third pipe.",
                "A, B, C: by the total capacity of the outdoor units (HP). D: by the capacity index downstream, never larger than A.",
            ],
            tables: vec![
                table("A, B, C: outdoor unit to first branch", &["Outdoor HP", "Liquid", "Suction gas", "High/low gas"], hr_main),
                table("D: between branches", &["Index downstream", "Liquid", "Suction gas", "High/low gas"], hr_branch),
            ],
        },
        Page {
            name: "Refrigerant, rules",
            intro: &[
                "Size-up: when the equivalent length of the longest run is 90 m or more, the liquid pipe of the main line goes one size up (9.5 → 12.7, 12.7 → 15.9, 15.9 → 19.1, 19.1 → 22.2).",
                "Wall: the pipe must be at least this thick. Annealed copper up to 15.9, half-hard from 19.1.",
                "A branch kit on a box takes at most 5 units and an index of 140 (250 when it joins two outlets). The Check Refrigerant tool tests this.",
                "The check does not size the links yet: it only reports what exceeds these limits.",
            ],
            tables: vec![table("Minimal wall thickness", &["Pipe ⌀ (mm)", "Wall (mm)"], wall)],
        },
    ]
}

fn water_page() -> Page {
    let c = PipeCriteria::default();
    let rows = STEEL_PIPES
        .iter()
        .filter_map(|(dn, od, _)| {
            let inner = steel_inner_diameter_mm(*dn)?;
            let cold = max_flow_m3h(*dn, WaterTemperature::Cold, &c).ok()?;
            let hot = max_flow_m3h(*dn, WaterTemperature::Hot, &c).ok()?;
            // Power carried at the temperature differences the networks use: 5 K (chilled), 20 K (hot).
            let kw = |q: f64, dt: f64| q * 4.18e6 * dt / 3.6e6;
            Some(vec![
                mm(*dn),
                mm(*od),
                format!("{inner:.1}"),
                format!("{:.1}", max_velocity_ms(*dn)),
                format!("{cold:.2}"),
                format!("{:.0}", kw(cold, 5.0)),
                format!("{hot:.2}"),
                format!("{:.0}", kw(hot, 20.0)),
            ])
        })
        .collect();
    Page {
        name: "Water pipes",
        intro: &[
            "Flow = power ÷ (4.18 × temperature difference): 5 K for chilled water, 20 K for hot water, 5 K for change-over.",
            "The smallest steel pipe is chosen whose velocity stays under the limit of its size (0.7 m/s up to 2 m/s, growing with the diameter) and whose pressure loss stays under 300 Pa/m (Darcy-Weisbach, Colebrook).",
            "The table gives the highest flow each size carries within both limits, and the power that is at 5 K (chilled) and 20 K (hot).",
        ],
        tables: vec![table(
            "Steel pipes, default limits",
            &["DN", "Outer ⌀", "Inner ⌀", "Max m/s", "Chilled m³/h", "kW at 5 K", "Hot m³/h", "kW at 20 K"],
            rows,
        )],
    }
}

fn condensate_page() -> Page {
    let rows = DRAIN_PIPES
        .iter()
        .filter_map(|(od, wall)| {
            let cap = |s: f64| drain_capacity_lph(*od, s).ok().map(|v| format!("{v:.0}"));
            Some(vec![mm(*od), format!("{:.1}", od - 2.0 * wall), cap(1.0)?, cap(2.0)?, cap(3.0)?])
        })
        .collect();
    Page {
        name: "Condensate drains",
        intro: &[
            "A gravity drain runs half full (Manning, n = 0.011) and must carry twice the flow it is given. Smallest size: 20 mm. The usual slope is 1 %.",
            "The table gives the highest condensate flow of each size, in litres per hour, at three slopes.",
        ],
        tables: vec![table("Plastic drains", &["Outer ⌀", "Inner ⌀", "l/h at 1 %", "l/h at 2 %", "l/h at 3 %"], rows)],
    }
}

fn air_page() -> Page {
    let rows = ROUND_DIAMETERS_MM
        .iter()
        .map(|d| {
            let area = round_area_m2(*d);
            // The flow that gives 3, 5 and 8 m/s in a round duct.
            let q = |v: f64| format!("{:.0}", v * area * 3600.0);
            vec![mm(*d), format!("{area:.3}"), q(3.0), q(5.0), q(8.0)]
        })
        .collect();
    Page {
        name: "Ducts and fresh air",
        intro: &[
            "Fresh air (labour code): 25 m³/h per occupant in offices, 30 in restaurants, shops and meeting rooms, 45 in light workshops, 60 in other workshops. Toilets 30, bathrooms 45, bathroom with toilet 60 (15 if not collective).",
            "Ducts are sized by velocity and pressure loss (Darcy-Weisbach, Colebrook). A rectangular duct is replaced by the round duct of the same friction.",
            "The table gives the flow of each round size at 3, 5 and 8 m/s.",
        ],
        tables: vec![table("Round ducts", &["⌀ (mm)", "Area (m²)", "m³/h at 3 m/s", "m³/h at 5 m/s", "m³/h at 8 m/s"], rows)],
    }
}

fn electrical_pages() -> Vec<Page> {
    use cadcraft_hvac::{
        Conductor, Grouping, Insulation, Laying, RoomLighting, Usage, current_capacity, k2, k3, max_voltage_drop_percent, minimum_illuminance,
        resistivity,
    };
    let cell = |v: Option<f64>| v.map_or("—".to_string(), |x| format!("{x}"));
    let capacity = |conductor| -> Vec<Vec<String>> {
        cadcraft_hvac::sections(conductor)
            .into_iter()
            .filter(|s| *s <= 300.0)
            .map(|s| {
                let mut row = vec![mm(s)];
                for laying in [Laying::B, Laying::C, Laying::E, Laying::F] {
                    for ins in [Insulation::Pvc, Insulation::Pr] {
                        row.push(cell(current_capacity(conductor, s, laying, ins, 3)));
                    }
                }
                row
            })
            .collect()
    };
    let heads = ["Section (mm²)", "B PVC", "B PR", "C PVC", "C PR", "E PVC", "E PR", "F PVC", "F PR"];
    let k2_rows = (1..=9)
        .map(|n| {
            vec![
                n.to_string(),
                format!("{:.2}", k2(Grouping::Embedded, n)),
                format!("{:.2}", k2(Grouping::WallsAndFloors, n)),
                format!("{:.2}", k2(Grouping::PerforatedTray, n)),
            ]
        })
        .collect();
    let k3_rows = (2..=12)
        .map(|i| {
            let t = f64::from(i) * 5.0;
            vec![format!("{t}"), cell(k3(Insulation::Pvc, t)), cell(k3(Insulation::Pr, t))]
        })
        .collect();
    let limits = vec![
        vec![
            "Lighting".into(),
            format!("{}", max_voltage_drop_percent(Usage::Lighting, false)),
            format!("{}", max_voltage_drop_percent(Usage::Lighting, true)),
        ],
        vec![
            "Other uses".into(),
            format!("{}", max_voltage_drop_percent(Usage::Other, false)),
            format!("{}", max_voltage_drop_percent(Usage::Other, true)),
        ],
    ];
    let lux = [
        ("Circulation", RoomLighting::Circulation),
        ("Stairs and stores", RoomLighting::StairsWarehouse),
        ("Workrooms, cloakrooms, toilets", RoomLighting::WorkRoom),
    ]
    .into_iter()
    .map(|(name, c)| vec![name.to_string(), format!("{}", minimum_illuminance(c))])
    .collect();
    vec![
        Page {
            name: "Electrical cables",
            intro: &[
                "The current a cable carries is read in the table of its laying letter (B in conduit on a wall, C clipped direct, E multi-core on a tray, F single-core on a tray) and insulation (PVC 70 °C, PR 90 °C), for three loaded conductors, in free conditions at 30 °C.",
                "It is multiplied by K = K1 × K2 × K3: K1 the way of laying, K2 the circuits laid together, K3 the ambient temperature. The smallest section whose corrected current is at least the rating of the breaker is chosen.",
                "Source: Schneider Electric, technical supplement of the electrical distribution catalogue 2016/2017, pages A46 to A49. The standard (NF C 15-100, UTE C 15-105) decides; check the values there.",
            ],
            tables: vec![
                table("Copper, three loaded conductors (A)", &heads, capacity(Conductor::Copper)),
                table("Aluminium, three loaded conductors (A)", &heads, capacity(Conductor::Aluminium)),
                table("K2: circuits laid together", &["Circuits", "Embedded", "On walls and floors", "On a perforated tray"], k2_rows),
                table("K3: ambient temperature", &["°C", "PVC", "PR"], k3_rows),
            ],
        },
        Page {
            name: "Voltage drop and lighting",
            intro: &[
                "ΔU = b × I × L × (R cos φ + X sin φ): b = 2 for a single-phase circuit and √3 for a three-phase one, R = ρ ÷ S (ρ = 22.5 Ω·mm²/km for copper and 36 for aluminium), X = 0.08 Ω/km counted from 50 mm². The drop is compared with 230 V (single-phase) or 400 V (three-phase).",
                "The length of a circuit is the length of all its cables: for a branched circuit this is on the safe side.",
                "Illuminance of a room = total flux of its luminaires × utilisation factor × maintenance factor ÷ area. The minimums are those of the Code du travail, article R4223-4.",
            ],
            tables: vec![
                table("Voltage drop allowed (%)", &["Use", "Public supply", "Own HV/LV substation"], limits),
                table("Minimum illuminance (lux)", &["Room", "Lux"], lux),
                table(
                    "Resistivity (Ω·mm²/km)",
                    &["Metal", "ρ"],
                    vec![
                        vec!["Copper".into(), format!("{}", resistivity(Conductor::Copper))],
                        vec!["Aluminium".into(), format!("{}", resistivity(Conductor::Aluminium))],
                    ],
                ),
            ],
        },
    ]
}

/// Every chart of the window, in order.
pub fn pages() -> Vec<Page> {
    let mut v = refrigerant_pages();
    v.push(water_page());
    v.push(condensate_page());
    v.push(air_page());
    v.extend(electrical_pages());
    v
}

/// Open the window (at the chart `page`, if given); open already, bring it forward.
pub fn open(app: &mut CadApp) {
    let mut st = app.charts_dlg.take().unwrap_or_default();
    st.raise = true;
    app.charts_dlg = Some(st);
    app.ui.dialog = Some("charts".into());
}

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open_flag: &mut bool) {
    let st = app.charts_dlg.get_or_insert_with(ChartsState::default);
    let pages = pages();
    st.page = st.page.min(pages.len().saturating_sub(1));
    let id = egui::Id::unique("charts_dialog");
    if std::mem::take(&mut st.raise) {
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    }
    egui::Window::new(crate::tl!("Design Charts")).id(id).open(open_flag).collapsible(false).default_size(vec2(720.0, 520.0)).show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (i, p) in pages.iter().enumerate() {
                if ui.selectable_label(st.page == i, crate::i18n::t(p.name)).clicked() {
                    st.page = i;
                }
            }
        });
        ui.separator();
        let Some(page) = pages.get(st.page) else { return };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for line in page.intro {
                ui.label(crate::i18n::t(line));
            }
            for t in &page.tables {
                ui.add_space(8.0);
                ui.label(RichText::new(crate::i18n::t(t.title)).strong());
                egui::Grid::new(t.title).striped(true).spacing(vec2(16.0, 3.0)).show(ui, |ui| {
                    for h in &t.head {
                        ui.label(RichText::new(crate::i18n::t(h)).small().strong());
                    }
                    ui.end_row();
                    for r in &t.rows {
                        for c in r {
                            ui.label(c);
                        }
                        ui.end_row();
                    }
                });
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_table_is_rectangular_and_filled() {
        for p in pages() {
            assert!(!p.intro.is_empty() && !p.tables.is_empty(), "{}", p.name);
            for t in &p.tables {
                assert!(!t.rows.is_empty(), "{}: {}", p.name, t.title);
                assert!(t.rows.iter().all(|r| r.len() == t.head.len()), "{}: {}", p.name, t.title);
            }
        }
    }

    #[test]
    fn the_window_opens_from_the_ui_command() {
        let mut app = CadApp::new(cadcraft_engine::Session::new(), crate::Services::default());
        app.start("ui.charts");
        assert_eq!(app.ui.dialog.as_deref(), Some("charts"));
        assert!(app.charts_dlg.is_some());
    }
}
