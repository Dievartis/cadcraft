//! HVACTAKEOFF: the quantities of one level, each drawing of a building being a floor: ducts by
//! direction, section and altitude (length and surface), fittings, accessories, terminals, units,
//! risers, equipment and, by space, what it holds. As JSON, and as a `;`-separated file a spreadsheet opens.

use serde_json::{Value, json};

use cadcraft_doc::{CalcNote, NoteNetwork, NoteSegment, ahu_of, riser_of};
use cadcraft_hvac::{Governing, Section};

use super::{CommandSpec, bad, bool_or, str_param};

use crate::{EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("hvactakeoff", "HVAC Quantities", run)
            .menu(&["Draw", "HVAC", "Quantities"])
            .params(
                "{level? (the name of the floor, written on the result), path? (a file to write: .xlsx for an Excel workbook with the calculation note, \\
                 else .csv with `;` between cells), decimal?: comma|point (the decimal mark of a .csv, default comma), lang?: fr|en (the labels of an .xlsx, default en), \\
                 note? (add the calculation note: default true for an .xlsx), prices? ({\"Duct|Ø200\": euros per metre, \"Pipe insulation\": euros per m², \"Elbow 90°|Ø200\": euros each…}: adds `costing`), insulatedAir? (the air directions whose ducts are insulated: supply, extract, fresh, exhaust), levels? (the takeoffs of the other floors, as this command returned them: the result is then the total of the building, this floor replacing the one of the same name), building? (the name of that total, default Building), + the sizing criteria of ductsize} → {level, ducts, fittings, accessories, terminals, units, \\
                 risers, equipment, spaces, totals, levels, costing?, note?, path?}",
            ),
    ]
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn run(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "hvactakeoff";
    let level = str_param(p, "level").unwrap_or("");
    if level.chars().count() > 64 || level.chars().any(char::is_control) {
        return Err(bad(CMD, "`level` has at most 64 characters, without control characters"));
    }
    let comma = match str_param(p, "decimal").map(|t| t.trim().to_ascii_lowercase()).as_deref() {
        None | Some("comma") => true,
        Some("point") => false,
        Some(_) => return Err(bad(CMD, "`decimal` is comma or point")),
    };
    let mut t = s.doc()?.takeoff(level).map_err(|e| other(e.to_string()))?;
    if let Some(others) = p.get("levels").filter(|v| !v.is_null()) {
        t = building(&t, others, str_param(p, "building").unwrap_or("Building"))?;
    }
    if let Some(settings) = settings(p)? {
        t.costing = Some(t.costing(&settings));
    }
    let mut out = serde_json::to_value(&t).map_err(|e| other(e.to_string()))?;
    let path = str_param(p, "path").map(str::trim).filter(|x| !x.is_empty());
    let xlsx = path.is_some_and(|x| x.to_ascii_lowercase().ends_with(".xlsx"));
    let lang = str_param(p, "lang").unwrap_or("en");
    if lang.len() > 16 || lang.chars().any(char::is_control) {
        return Err(bad(CMD, "`lang` is a language code such as fr or en"));
    }
    let note = if bool_or(p, "note", xlsx) { Some(calc_note(s, p)?) } else { None };
    if let Some(n) = &note {
        out["note"] = serde_json::to_value(n).map_err(|e| other(e.to_string()))?;
    }
    if let Some(path) = path {
        if path.len() > 1024 || path.contains('\0') {
            return Err(bad(CMD, "`path` is not a usable file name"));
        }
        let path = std::path::Path::new(path);
        crate::file_access::check_write(path).map_err(other)?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        }
        let bytes = if xlsx {
            t.to_xlsx(note.as_ref(), lang).map_err(|e| other(e.to_string()))?
        } else {
            // A byte order mark, so a spreadsheet reads the accents as UTF-8.
            let mut bytes = vec![0xEF, 0xBB, 0xBF];
            bytes.extend_from_slice(t.to_csv(comma).as_bytes());
            bytes
        };
        std::fs::write(path, bytes).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        out["path"] = json!(path.display().to_string());
    }
    Ok(out)
}

/// The prices and the insulated directions of `p`, when it has either.
fn settings(p: &Value) -> Result<Option<cadcraft_doc::TakeoffSettings>> {
    const CMD: &str = "hvactakeoff";
    if p.get("prices").is_none_or(Value::is_null) && p.get("insulatedAir").is_none_or(Value::is_null) {
        return Ok(None);
    }
    let mut st = cadcraft_doc::TakeoffSettings::default();
    if let Some(prices) = p.get("prices").filter(|v| !v.is_null()) {
        let map = prices.as_object().ok_or_else(|| bad(CMD, "`prices` is an object of unit prices by line"))?;
        if map.len() > 10_000 {
            return Err(bad(CMD, "at most 10000 prices"));
        }
        for (k, v) in map {
            let price =
                v.as_f64().and_then(cadcraft_doc::usable_price).ok_or_else(|| bad(CMD, format!("the price of `{k}` is a number of euros")))?;
            st.prices.insert(k.clone(), price);
        }
    }
    if let Some(a) = p.get("insulatedAir").filter(|v| !v.is_null()) {
        let list = a.as_array().ok_or_else(|| bad(CMD, "`insulatedAir` is a list of supply, extract, fresh or exhaust"))?;
        for v in list {
            let dir: cadcraft_doc::AirDirection =
                serde_json::from_value(v.clone()).map_err(|_| bad(CMD, "`insulatedAir` takes supply, extract, fresh or exhaust"))?;
            st.insulated_air.push(dir);
        }
    }
    Ok(Some(st))
}

/// The total of the building: the other floors given (as this command returned them) and this one, which replaces the floor of its name.
fn building(this: &cadcraft_doc::Takeoff, others: &Value, name: &str) -> Result<cadcraft_doc::Takeoff> {
    const CMD: &str = "hvactakeoff";
    let list = others.as_array().ok_or_else(|| bad(CMD, "`levels` is a list of takeoffs"))?;
    if list.len() > 500 {
        return Err(bad(CMD, "at most 500 levels"));
    }
    let mut levels: Vec<cadcraft_doc::Takeoff> = Vec::new();
    for v in list {
        let t: cadcraft_doc::Takeoff = serde_json::from_value(v.clone()).map_err(|e| bad(CMD, format!("a level is not a takeoff: {e}")))?;
        if t.level != this.level {
            levels.push(t);
        }
    }
    levels.push(this.clone());
    Ok(cadcraft_doc::Takeoff::building(name, &levels))
}

fn r(v: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (v * k).round() / k
}

/// The calculation of each network as sizing computes it now (the same criteria and the same reducers),
/// without changing the drawing.
fn calc_note(s: &Session, p: &Value) -> Result<CalcNote> {
    let criteria = super::duct::criteria(p)?;
    let d = s.doc()?;
    let mut note = CalcNote::default();
    for net in d.duct_networks().map_err(|e| other(e.to_string()))? {
        if net.network.segments.is_empty() {
            continue;
        }
        let report = super::duct::size_with_reducers(&net.network, &criteria).map_err(|e| other(e.to_string()))?;
        let origin =
            d.entity(net.ahu).and_then(|e| ahu_of(e).map(|a| a.name).or_else(|| riser_of(e).map(|r| r.name))).unwrap_or_else(|| net.ahu.hex());
        let segments = report
            .segments
            .iter()
            .enumerate()
            .map(|(i, g)| NoteSegment {
                duct: net.ducts.get(i).map_or(String::new(), |h| h.hex()),
                section: match g.section {
                    Section::Rectangular => format!("{:.0}×{:.0}", g.width_mm, g.height_mm),
                    Section::Round => format!("Ø{:.0}", g.diameter_mm),
                },
                length_m: r(net.network.segments.get(i).map_or(0.0, |x| x.length_m), 2),
                flow_m3h: r(g.flow_m3h, 0),
                velocity_ms: r(g.velocity, 2),
                gradient_pa_m: r(g.gradient_pa_m, 3),
                friction_pa: r(g.friction_pa, 1),
                fittings_pa: r(g.fittings_pa, 1),
                terminal_pa: r(g.terminal_pa, 1),
                total_pa: r(g.total_pa, 1),
                governing: match g.governing {
                    Governing::Velocity => "velocity",
                    Governing::Friction => "friction",
                    Governing::Imposed => "imposed",
                    Governing::Largest => "largest",
                }
                .into(),
                critical: report.critical_path.contains(&i),
            })
            .collect();
        note.networks.push(NoteNetwork {
            origin,
            air: net.air,
            total_flow_m3h: r(report.total_flow_m3h, 0),
            required_pa: r(report.required_pressure_pa, 1),
            segments,
            warnings: report.warnings.clone(),
        });
    }
    Ok(note)
}
