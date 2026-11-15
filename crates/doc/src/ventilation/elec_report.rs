//! The lighting of the rooms against the code, and the quantities of the electrical objects: what a drawing needs of cable and devices, what it
//! costs, and the table of the circuits for the report of the initial verification.

use cadcraft_geom::Vec2;
use cadcraft_hvac::{self as calc, RoomLighting};
use serde::{Deserialize, Serialize};

use super::usable_price;
use super::{
    CircuitOptions, CircuitReport, CostLine, Costing, DeviceKind, SheetCell, TakeoffSettings, Workbook, ahu_of, cable_of, device_of, equipment_of,
};
use crate::Result;

/// What the check of the lighting assumes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingOptions {
    /// The share of the flux of the luminaires that reaches the work plane (0.5 is usual when nothing else is known).
    pub utilisation: f64,
    /// The ageing and dust of the luminaires (0.8 is usual).
    pub maintenance: f64,
}

impl Default for LightingOptions {
    fn default() -> Self {
        LightingOptions { utilisation: 0.5, maintenance: 0.8 }
    }
}

/// One room against the minimum illuminance of the Code du travail (article R4223-4).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingReport {
    pub space: String,
    pub area_m2: f64,
    pub luminaires: usize,
    pub flux_lm: f64,
    pub lux: f64,
    pub minimum_lux: f64,
    /// "circulation", "stairsWarehouse" or "workRoom".
    pub class: String,
    pub ok: bool,
    pub warning: Option<String>,
}

/// The class of a room by its name: circulation, stairs and stores, else a workroom (which holds the cloakrooms and the toilets).
fn class_of(name: &str) -> RoomLighting {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["circulation", "couloir", "corridor", "hall", "dégagement", "degagement", "passage", "sas"]) {
        RoomLighting::Circulation
    } else if has(&["escalier", "stair", "entrepôt", "entrepot", "stock", "réserve", "reserve", "archive", "magasin"]) {
        RoomLighting::StairsWarehouse
    } else {
        RoomLighting::WorkRoom
    }
}

fn class_name(c: RoomLighting) -> &'static str {
    match c {
        RoomLighting::Circulation => "circulation",
        RoomLighting::StairsWarehouse => "stairsWarehouse",
        RoomLighting::WorkRoom => "workRoom",
    }
}

impl crate::Drawing {
    /// The spaces of model space with the illuminance their luminaires give, against the minimum of the code for their class.
    pub fn lighting_reports(&self, opts: &LightingOptions) -> Vec<LightingReport> {
        let lights: Vec<(Vec2, f64)> = self
            .model
            .iter()
            .filter_map(|e| {
                let d = device_of(e).filter(|d| d.kind.is_light())?;
                let crate::EntityKind::Insert(i) = &e.kind else { return None };
                Some((Vec2::new(i.insert.x, i.insert.y), d.flux_lm.unwrap_or(0.0)))
            })
            .collect();
        let mut out = Vec::new();
        for e in self.model.iter() {
            let Some((sp, outline)) = self.space_outline(e.handle) else { continue };
            let inside: Vec<f64> = lights.iter().filter(|(at, _)| cadcraft_geom::point_in_polygon(&outline, *at)).map(|(_, f)| *f).collect();
            let class = class_of(&sp.name);
            let area = sp.area_m2();
            let flux: f64 = inside.iter().sum();
            let lux = calc::illuminance_lux(flux, area.max(0.01), opts.utilisation, opts.maintenance).unwrap_or(0.0);
            let minimum = calc::minimum_illuminance(class);
            let warning = if inside.is_empty() {
                Some("no luminaire is drawn in this room".to_string())
            } else if flux <= 0.0 {
                Some("the luminaires have no flux: type it in lumen".to_string())
            } else if lux < minimum {
                Some(format!("{lux:.0} lux is below the {minimum:.0} lux of the Code du travail"))
            } else {
                None
            };
            out.push(LightingReport {
                space: sp.name.clone(),
                area_m2: (area * 100.0).round() / 100.0,
                luminaires: inside.len(),
                flux_lm: flux.round(),
                lux: lux.round(),
                minimum_lux: minimum,
                class: class_name(class).into(),
                ok: warning.is_none(),
                warning,
            });
        }
        out.sort_by(|a, b| a.space.cmp(&b.space));
        out
    }
}

/// The cables of one section and use.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableQuantity {
    /// "3G2.5", or "to size" while the section is not chosen.
    pub size: String,
    pub usage: String,
    pub count: usize,
    pub length_m: f64,
}

/// The devices of one kind.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceQuantity {
    pub name: String,
    pub count: usize,
    pub power_w: f64,
    /// The luminous flux of the luminaires, lumen.
    pub flux_lm: f64,
}

/// An equipment or a unit that draws electrical power.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HvacLoad {
    pub name: String,
    pub kind: String,
    pub electric_kw: Option<f64>,
    pub circuit: String,
}

/// The quantities of the electrical objects of a level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElecTakeoff {
    pub level: String,
    pub cables: Vec<CableQuantity>,
    pub devices: Vec<DeviceQuantity>,
    pub panels: Vec<String>,
    pub hvac: Vec<HvacLoad>,
    pub circuits: Vec<CircuitReport>,
    pub cable_length_m: f64,
    pub installed_power_w: f64,
    #[serde(default)]
    pub costing: Option<Costing>,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl crate::Drawing {
    /// The cables, devices, boards and loads of the HVAC equipment of model space for the level named `level`, with the circuits as they are checked.
    pub fn elec_takeoff(&self, level: &str, opts: &CircuitOptions) -> Result<ElecTakeoff> {
        let mut t = ElecTakeoff { level: level.to_string(), ..ElecTakeoff::default() };
        for e in self.model.iter() {
            if let Some(c) = cable_of(e) {
                let size = if c.section_mm2.is_some() { c.size_text() } else { "to size".to_string() };
                let usage = c.usage.name().to_string();
                match t.cables.iter_mut().find(|q| q.size == size && q.usage == usage) {
                    Some(q) => {
                        q.count += 1;
                        q.length_m += c.length_m();
                    }
                    None => t.cables.push(CableQuantity { size, usage, count: 1, length_m: c.length_m() }),
                }
            } else if let Some(d) = device_of(e) {
                if d.kind == DeviceKind::Panel {
                    t.panels.push(d.name.clone());
                }
                let name = d.kind.name().to_string();
                match t.devices.iter_mut().find(|q| q.name == name) {
                    Some(q) => {
                        q.count += 1;
                        q.power_w += d.power_w;
                        q.flux_lm += d.flux_lm.unwrap_or(0.0);
                    }
                    None => t.devices.push(DeviceQuantity { name, count: 1, power_w: d.power_w, flux_lm: d.flux_lm.unwrap_or(0.0) }),
                }
            } else if let Some(q) = equipment_of(e) {
                if q.electric_kw.is_some() || !q.circuit.is_empty() {
                    t.hvac.push(HvacLoad {
                        name: q.name.clone(),
                        kind: q.kind.name().to_string(),
                        electric_kw: q.electric_kw,
                        circuit: q.circuit.clone(),
                    });
                }
            } else if let Some(u) = ahu_of(e)
                && (u.electric_kw.is_some() || !u.circuit.is_empty())
            {
                t.hvac.push(HvacLoad { name: u.name.clone(), kind: "AHU".into(), electric_kw: u.electric_kw, circuit: u.circuit.clone() });
            }
        }
        for q in &mut t.cables {
            q.length_m = round2(q.length_m);
        }
        t.cables.sort_by(|a, b| a.usage.cmp(&b.usage).then(a.size.cmp(&b.size)));
        t.devices.sort_by(|a, b| a.name.cmp(&b.name));
        t.panels.sort();
        t.hvac.sort_by(|a, b| a.name.cmp(&b.name));
        t.cable_length_m = round2(t.cables.iter().map(|q| q.length_m).sum());
        t.installed_power_w = t.devices.iter().map(|q| q.power_w).sum::<f64>() + t.hvac.iter().filter_map(|h| h.electric_kw).sum::<f64>() * 1000.0;
        t.circuits = self.circuit_reports(opts);
        Ok(t)
    }
}

impl ElecTakeoff {
    /// The lines that have a price (or could have one) and what they come to: cables by size and use (per metre), devices by kind (each).
    pub fn priced(&self, settings: &TakeoffSettings) -> Costing {
        let mut lines: Vec<CostLine> = Vec::new();
        let mut push = |group: &str, label: String, unit: &str, quantity: f64, key: String| {
            let unit_price = settings.prices.get(&key).copied().and_then(usable_price);
            lines.push(CostLine {
                group: group.into(),
                label,
                unit: unit.into(),
                quantity: round2(quantity),
                key,
                unit_price,
                total: unit_price.map(|p| round2(p * quantity)),
            });
        };
        // Cables by size, whatever the use.
        let mut by_size: Vec<(String, f64)> = Vec::new();
        for q in &self.cables {
            match by_size.iter_mut().find(|(s, _)| *s == q.size) {
                Some((_, l)) => *l += q.length_m,
                None => by_size.push((q.size.clone(), q.length_m)),
            }
        }
        for (size, m) in by_size {
            push("Cable", size.clone(), "m", m, format!("Cable|{size}"));
        }
        for q in &self.devices {
            push("Device", q.name.clone(), "each", q.count as f64, format!("Device|{}", q.name));
        }
        let total = round2(lines.iter().filter_map(|l| l.total).sum());
        let unpriced = lines.iter().filter(|l| l.unit_price.is_none()).count();
        Costing { lines, total, unpriced }
    }

    /// The quantities as an Excel workbook: the summary, the cables, the devices, the circuits (the table of the verification report), the loads of the
    /// HVAC equipment and, when there is a cost, the cost.
    pub fn to_xlsx(&self) -> Result<Vec<u8>> {
        use SheetCell as C;
        let head = |cols: &[&str]| cols.iter().map(|c| C::head(*c)).collect::<Vec<_>>();
        let num = |v: f64| C::Num(round2(v));
        let mut wb = Workbook::new();
        wb.sheet(
            "Summary",
            vec![
                vec![C::head("Level"), C::text(self.level.clone())],
                vec![C::head("Cable length (m)"), num(self.cable_length_m)],
                vec![C::head("Installed power (W)"), num(self.installed_power_w)],
                vec![C::head("Circuits"), C::Num(self.circuits.len() as f64)],
                vec![C::head("Circuits to correct"), C::Num(self.circuits.iter().filter(|c| !c.ok()).count() as f64)],
                vec![C::head("Distribution boards"), C::text(self.panels.join(", "))],
            ],
        )?;
        let mut rows = vec![head(&["Size", "Use", "Count", "Length (m)"])];
        rows.extend(self.cables.iter().map(|q| vec![C::text(q.size.clone()), C::text(q.usage.clone()), C::Num(q.count as f64), num(q.length_m)]));
        wb.sheet("Cables", rows)?;
        let mut rows = vec![head(&["Device", "Count", "Power (W)", "Flux (lm)"])];
        rows.extend(self.devices.iter().map(|q| vec![C::text(q.name.clone()), C::Num(q.count as f64), num(q.power_w), num(q.flux_lm)]));
        wb.sheet("Devices", rows)?;
        let mut rows = vec![head(&[
            "Circuit",
            "Board",
            "Use",
            "Phases",
            "Devices",
            "HVAC loads",
            "Power (W)",
            "Current (A)",
            "Breaker (A)",
            "Length (m)",
            "Section drawn (mm²)",
            "Section required (mm²)",
            "Voltage drop (%)",
            "Limit (%)",
            "Remarks",
        ])];
        for c in &self.circuits {
            rows.push(vec![
                C::text(c.name.clone()),
                C::text(c.panel.clone()),
                C::text(c.usage.clone()),
                C::text(c.phases.clone()),
                C::Num(c.devices as f64),
                C::Num(c.hvac as f64),
                num(c.power_w),
                num(c.current_a),
                C::opt(c.rating_a),
                num(c.cable_length_m),
                C::opt(c.section_mm2),
                C::opt(c.required_section_mm2),
                C::opt(c.drop_percent),
                num(c.max_drop_percent),
                C::text(c.warnings.join("; ")),
            ]);
        }
        wb.sheet("Circuits", rows)?;
        let mut rows = vec![head(&["Equipment", "Type", "Electrical power (kW)", "Circuit"])];
        rows.extend(
            self.hvac.iter().map(|h| vec![C::text(h.name.clone()), C::text(h.kind.clone()), C::opt(h.electric_kw), C::text(h.circuit.clone())]),
        );
        wb.sheet("HVAC loads", rows)?;
        if let Some(c) = &self.costing {
            let mut rows = vec![head(&["Group", "Label", "Unit", "Quantity", "Unit price", "Total"])];
            rows.extend(c.lines.iter().map(|l| {
                vec![
                    C::text(l.group.clone()),
                    C::text(l.label.clone()),
                    C::text(l.unit.clone()),
                    num(l.quantity),
                    C::opt(l.unit_price),
                    C::opt(l.total),
                ]
            }));
            rows.push(vec![C::head("Total"), C::Empty, C::Empty, C::Empty, C::Empty, num(c.total)]);
            wb.sheet("Cost", rows)?;
        }
        wb.to_bytes()
    }

    /// The circuits as a `;`-separated file.
    pub fn circuits_csv(&self, decimal_comma: bool) -> String {
        let num = |v: f64| {
            let t = format!("{v}");
            if decimal_comma { t.replace('.', ",") } else { t }
        };
        let opt = |v: Option<f64>| v.map_or(String::new(), num);
        let cell = |t: &str| if t.contains([';', '"', '\n']) { format!("\"{}\"", t.replace('"', "\"\"")) } else { t.to_string() };
        let mut out = String::from(
            "Circuit;Board;Use;Phases;Power (W);Current (A);Breaker (A);Length (m);Section drawn (mm2);Section required (mm2);Voltage drop (%);Remarks\n",
        );
        for c in &self.circuits {
            let row = [
                cell(&c.name),
                cell(&c.panel),
                c.usage.clone(),
                c.phases.clone(),
                num(c.power_w),
                num(c.current_a),
                opt(c.rating_a),
                num(c.cable_length_m),
                opt(c.section_mm2),
                opt(c.required_section_mm2),
                opt(c.drop_percent),
                cell(&c.warnings.join("; ")),
            ];
            out.push_str(&row.join(";"));
            out.push('\n');
        }
        out
    }
}
