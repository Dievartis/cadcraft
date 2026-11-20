//! Independent state inversion for every layer except the clicked layer.
use super::{Result, Session, Value, bad, json, str_param};

pub(super) fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("layer.invertothers", "`name` is required"))?;
    let property = str_param(p, "property").ok_or_else(|| bad("layer.invertothers", "`property` is required"))?;
    if !matches!(property, "on" | "frozen" | "locked") {
        return Err(bad("layer.invertothers", "Property must be on, frozen or locked"));
    }
    let d = s.doc()?;
    if d.layer(name).is_none() {
        return Err(bad("layer.invertothers", "Layer does not exist"));
    }
    let current = d.header.str("CLAYER", "0");
    let mut skipped = false;
    let mut changed = 0usize;
    for layer in &mut s.doc_mut()?.layers {
        if layer.name.eq_ignore_ascii_case(name) {
            continue;
        }
        match property {
            "on" => layer.on = !layer.on,
            "locked" => layer.locked = !layer.locked,
            // Match layer.set: freezing the current layer is forbidden.
            "frozen" if layer.name.eq_ignore_ascii_case(&current) && !layer.frozen => {
                skipped = true;
                continue;
            }
            "frozen" => layer.frozen = !layer.frozen,
            _ => {}
        }
        changed += 1;
    }
    if skipped {
        s.echo("The current layer was kept thawed");
    }
    Ok(json!({"changed":changed,"currentLayerSkipped":skipped}))
}
