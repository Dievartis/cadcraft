//! The controls of a plan read back: the list of points (what each device asks of its controller), the load of each controller and module against what it
//! offers, and what is wrong (a device wired to nothing, a controller over its capacity, an automation device that no circuit feeds, a bus no cable is drawn
//! for). The list of points is the deliverable of a controls package.

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::{
    ControlKind, ControlParams, CostLine, Costing, Points, SheetCell, TakeoffSettings, Workbook, ahu_of, cable_of, control_of, equipment_of, part_of,
    same_network, usable_price,
};
use crate::Result;

/// One point of the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlPoint {
    pub device: String,
    pub kind: String,
    pub description: String,
    /// "AI", "AO", "DI" or "DO".
    #[serde(rename = "type")]
    pub point_type: String,
    pub unit: String,
    pub controller: String,
    pub space: String,
    pub served: String,
    pub setpoint: Option<f64>,
}

/// A controller or a module with what it offers and what is wired to it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerLoad {
    pub name: String,
    pub kind: String,
    /// The controller a module extends.
    pub extends: String,
    pub space: String,
    pub bus: String,
    pub circuit: String,
    pub capacity: Points,
    pub used: Points,
    /// Free points in percent of the capacity, by type (AI, AO, DI, DO); none for a type the controller does not offer.
    pub reserve_percent: [Option<f64>; 4],
}

/// The devices of one kind.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlCount {
    pub name: String,
    pub count: usize,
    pub points: u32,
}

/// The controls of a level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlTakeoff {
    pub level: String,
    pub devices: Vec<ControlCount>,
    pub controllers: Vec<ControllerLoad>,
    pub points: Vec<ControlPoint>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub costing: Option<Costing>,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

impl crate::Drawing {
    /// The list of points, the load of the controllers and what is wrong, for model space; `level` is the name written on the result.
    pub fn control_takeoff(&self, level: &str) -> ControlTakeoff {
        let mut devices: Vec<(ControlParams, Vec2)> = Vec::new();
        for e in self.model.iter() {
            let (Some(p), crate::EntityKind::Insert(i)) = (control_of(e), &e.kind) else { continue };
            devices.push((p, Vec2::new(i.insert.x, i.insert.y)));
        }
        let spaces: Vec<(String, Vec<Vec2>)> = self.model.iter().filter_map(|e| self.space_outline(e.handle).map(|(sp, o)| (sp.name, o))).collect();
        let space_of = |at: Vec2| spaces.iter().find(|(_, o)| cadcraft_geom::point_in_polygon(o, at)).map(|(n, _)| n.clone()).unwrap_or_default();
        let mut t = ControlTakeoff { level: level.to_string(), ..ControlTakeoff::default() };
        let mut warnings: Vec<String> = Vec::new();
        // The devices by kind.
        for (p, _) in &devices {
            let name = p.kind.name().to_string();
            match t.devices.iter_mut().find(|c| c.name == name) {
                Some(c) => (c.count, c.points) = (c.count + 1, c.points + p.kind.points().total()),
                None => t.devices.push(ControlCount { name, count: 1, points: p.kind.points().total() }),
            }
        }
        t.devices.sort_by(|a, b| a.name.cmp(&b.name));
        // The controllers and the modules, by name, with what is wired to each.
        let hosts: Vec<&(ControlParams, Vec2)> =
            devices.iter().filter(|(p, _)| matches!(p.kind, ControlKind::Controller | ControlKind::IoModule)).collect();
        let host_named = |n: &str| hosts.iter().find(|(p, _)| same_network(&p.name, n)).map(|(p, _)| p);
        for (p, at) in &hosts {
            if p.name.trim().is_empty() {
                warnings.push(format!("a {} has no name: the devices are wired to it by its name", p.kind.name()));
            }
            let used = devices
                .iter()
                .filter(|(d, _)| !d.kind.is_automation() && same_network(&d.controller, &p.name) && !p.name.trim().is_empty())
                .fold(Points::default(), |acc, (d, _)| acc.plus(d.kind.points()));
            let reserve = |cap: u32, used: u32| (cap > 0).then(|| round1(100.0 * (f64::from(cap) - f64::from(used)) / f64::from(cap)));
            let (c, u) = (p.capacity, used);
            for ((name, cap), (_, usd)) in c.counts().into_iter().zip(u.counts()) {
                if usd > cap {
                    warnings.push(format!("{}: {usd} {name} points are wired to it and it offers {cap}", p.name));
                }
            }
            if p.kind == ControlKind::IoModule
                && !p.controller.trim().is_empty()
                && host_named(&p.controller).is_none_or(|h| h.kind != ControlKind::Controller)
            {
                warnings.push(format!("{}: the controller {} that it extends is not drawn", p.name, p.controller));
            }
            if p.kind == ControlKind::IoModule && p.controller.trim().is_empty() {
                warnings.push(format!("{}: a module extends a controller: type its name", p.name));
            }
            t.controllers.push(ControllerLoad {
                name: p.name.clone(),
                kind: p.kind.name().to_string(),
                extends: if p.kind == ControlKind::IoModule { p.controller.clone() } else { String::new() },
                space: space_of(*at),
                bus: p.bus.clone(),
                circuit: p.circuit.clone(),
                capacity: c,
                used: u,
                reserve_percent: [reserve(c.ai, u.ai), reserve(c.ao, u.ao), reserve(c.di, u.di), reserve(c.dout, u.dout)],
            });
        }
        t.controllers.sort_by(|a, b| a.name.cmp(&b.name));
        // The points, and the devices wired to nothing.
        let known: Vec<String> = self
            .model
            .iter()
            .flat_map(|e| [equipment_of(e).map(|q| q.name), ahu_of(e).map(|u| u.name), part_of(e).map(|p| p.name)])
            .flatten()
            .collect();
        for (p, at) in devices.iter().filter(|(p, _)| !p.kind.is_automation()) {
            if p.kind.points().total() == 0 {
                continue;
            }
            let wired = !p.controller.trim().is_empty();
            if !wired {
                warnings.push(format!("{}: not wired to a controller", p.name));
            } else if host_named(&p.controller).is_none() {
                warnings.push(format!("{}: the controller {} is not drawn", p.name, p.controller));
            }
            if !p.served.trim().is_empty() && !known.iter().any(|k| same_network(k, &p.served)) {
                warnings.push(format!("{}: serves {}, which is not drawn", p.name, p.served));
            }
            for (ty, what) in p.kind.point_list() {
                t.points.push(ControlPoint {
                    device: p.name.clone(),
                    kind: p.kind.name().to_string(),
                    description: what.to_string(),
                    point_type: ty.to_string(),
                    unit: if what == "Fault" || what == "Run command" || what == "Occupancy" { String::new() } else { p.kind.unit().to_string() },
                    controller: p.controller.clone(),
                    space: space_of(*at),
                    served: p.served.clone(),
                    setpoint: if what == "Setpoint" || what == "Measure" { p.setpoint } else { None },
                });
            }
        }
        t.points.sort_by(|a, b| a.controller.cmp(&b.controller).then(a.device.cmp(&b.device)));
        // What feeds the automation, and the buses.
        let bus_cables: Vec<String> =
            self.model.iter().filter_map(|e| cable_of(e)).filter(|c| c.usage == super::CableUsage::Bus).map(|c| c.circuit).collect();
        let mut buses: Vec<&str> = Vec::new();
        for (p, _) in devices.iter().filter(|(p, _)| p.kind.is_automation()) {
            if matches!(p.kind, ControlKind::Controller | ControlKind::IoModule | ControlKind::Gateway | ControlKind::ControlCabinet)
                && p.circuit.trim().is_empty()
            {
                warnings.push(format!("{}: no circuit feeds it", p.name));
            }
            if !p.bus.trim().is_empty() && !buses.iter().any(|b| same_network(b, &p.bus)) {
                buses.push(&p.bus);
            }
        }
        for b in buses {
            if !bus_cables.iter().any(|c| same_network(c, b)) {
                warnings.push(format!("bus {b}: no bus cable is drawn on a circuit of that name"));
            }
        }
        // The same tag twice.
        let mut tags: Vec<&str> = devices.iter().map(|(p, _)| p.name.trim()).filter(|n| !n.is_empty()).collect();
        tags.sort_unstable();
        for w in tags.windows(2) {
            if let [a, b] = w
                && a.eq_ignore_ascii_case(b)
                && !warnings.iter().any(|x| x.starts_with(&format!("the tag {a} ")))
            {
                warnings.push(format!("the tag {a} is on more than one device"));
            }
        }
        t.warnings = warnings;
        t
    }
}

impl ControlTakeoff {
    /// The lines that have a price: the devices by kind, each.
    pub fn priced(&self, settings: &TakeoffSettings) -> Costing {
        let lines: Vec<CostLine> = self
            .devices
            .iter()
            .map(|d| {
                let key = format!("Control|{}", d.name);
                let unit_price = settings.prices.get(&key).copied().and_then(usable_price);
                CostLine {
                    group: "Control".into(),
                    label: d.name.clone(),
                    unit: "each".into(),
                    quantity: d.count as f64,
                    key,
                    unit_price,
                    total: unit_price.map(|p| (p * d.count as f64 * 100.0).round() / 100.0),
                }
            })
            .collect();
        let total = (lines.iter().filter_map(|l| l.total).sum::<f64>() * 100.0).round() / 100.0;
        let unpriced = lines.iter().filter(|l| l.unit_price.is_none()).count();
        Costing { lines, total, unpriced }
    }

    /// The workbook of the controls package: the summary, the controllers with their reserve, the list of points, the devices and, with prices, the cost.
    pub fn to_xlsx(&self) -> Result<Vec<u8>> {
        use SheetCell as C;
        let head = |cols: &[&str]| cols.iter().map(|c| C::head(*c)).collect::<Vec<_>>();
        let mut wb = Workbook::new();
        wb.sheet(
            "Summary",
            vec![
                vec![C::head("Level"), C::text(self.level.clone())],
                vec![C::head("Devices"), C::Num(self.devices.iter().map(|d| d.count).sum::<usize>() as f64)],
                vec![C::head("Points"), C::Num(self.points.len() as f64)],
                vec![C::head("Controllers and modules"), C::Num(self.controllers.len() as f64)],
                vec![C::head("To correct"), C::Num(self.warnings.len() as f64)],
            ],
        )?;
        let mut rows = vec![head(&[
            "Controller",
            "Type",
            "Extends",
            "Space",
            "Bus",
            "Circuit",
            "AI offered",
            "AI used",
            "AO offered",
            "AO used",
            "DI offered",
            "DI used",
            "DO offered",
            "DO used",
            "AI reserve (%)",
            "AO reserve (%)",
            "DI reserve (%)",
            "DO reserve (%)",
        ])];
        for c in &self.controllers {
            let n = |v: u32| C::Num(f64::from(v));
            rows.push(vec![
                C::text(c.name.clone()),
                C::text(c.kind.clone()),
                C::text(c.extends.clone()),
                C::text(c.space.clone()),
                C::text(c.bus.clone()),
                C::text(c.circuit.clone()),
                n(c.capacity.ai),
                n(c.used.ai),
                n(c.capacity.ao),
                n(c.used.ao),
                n(c.capacity.di),
                n(c.used.di),
                n(c.capacity.dout),
                n(c.used.dout),
                C::opt(c.reserve_percent[0]),
                C::opt(c.reserve_percent[1]),
                C::opt(c.reserve_percent[2]),
                C::opt(c.reserve_percent[3]),
            ]);
        }
        wb.sheet("Controllers", rows)?;
        let mut rows = vec![head(&["Controller", "Device", "Type", "Description", "Point type", "Unit", "Space", "Serves", "Setpoint"])];
        for p in &self.points {
            rows.push(vec![
                C::text(p.controller.clone()),
                C::text(p.device.clone()),
                C::text(p.kind.clone()),
                C::text(p.description.clone()),
                C::text(p.point_type.clone()),
                C::text(p.unit.clone()),
                C::text(p.space.clone()),
                C::text(p.served.clone()),
                C::opt(p.setpoint),
            ]);
        }
        wb.sheet("Points", rows)?;
        let mut rows = vec![head(&["Device", "Count", "Points"])];
        rows.extend(self.devices.iter().map(|d| vec![C::text(d.name.clone()), C::Num(d.count as f64), C::Num(f64::from(d.points))]));
        wb.sheet("Devices", rows)?;
        if let Some(c) = &self.costing {
            let mut rows = vec![head(&["Group", "Label", "Unit", "Quantity", "Unit price", "Total"])];
            rows.extend(c.lines.iter().map(|l| {
                vec![
                    C::text(l.group.clone()),
                    C::text(l.label.clone()),
                    C::text(l.unit.clone()),
                    C::Num(l.quantity),
                    C::opt(l.unit_price),
                    C::opt(l.total),
                ]
            }));
            rows.push(vec![C::head("Total"), C::Empty, C::Empty, C::Empty, C::Empty, C::Num(c.total)]);
            wb.sheet("Cost", rows)?;
        }
        if !self.warnings.is_empty() {
            let mut rows = vec![head(&["Remarks"])];
            rows.extend(self.warnings.iter().map(|w| vec![C::text(w.clone())]));
            wb.sheet("Remarks", rows)?;
        }
        wb.to_bytes()
    }

    /// The list of points as a `;`-separated file.
    pub fn points_csv(&self, decimal_comma: bool) -> String {
        let cell = |t: &str| if t.contains([';', '"', '\n']) { format!("\"{}\"", t.replace('"', "\"\"")) } else { t.to_string() };
        let num = |v: Option<f64>| v.map_or(String::new(), |x| if decimal_comma { format!("{x}").replace('.', ",") } else { format!("{x}") });
        let mut out = String::from("Controller;Device;Type;Description;Point type;Unit;Space;Serves;Setpoint\n");
        for p in &self.points {
            let row = [
                cell(&p.controller),
                cell(&p.device),
                cell(&p.kind),
                cell(&p.description),
                p.point_type.clone(),
                cell(&p.unit),
                cell(&p.space),
                cell(&p.served),
                num(p.setpoint),
            ];
            out.push_str(&row.join(";"));
            out.push('\n');
        }
        out
    }
}
