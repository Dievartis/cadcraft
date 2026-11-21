//! TITLEBLOCKADD draws the title block of a sheet (project, the parties with their logos, a picture of the project, the title, the table of
//! revisions and the code of the sheet); TITLEBLOCKEDIT changes one. The dialog behind CARTOUCHE fills these with the whole block each time.

use std::sync::Arc;

use cadcraft_doc::{
    Code, DocError, Handle, HvacUnits, MAX_CODES, MAX_PARTIES, MAX_REVISIONS, Party, Pic, Raster, Revision, TITLEBLOCK_IMAGE_PREFIX,
    TitleBlockParams, titleblock_of,
};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};
use crate::{EngineError, Result, Session};

/// The longest side a picture of a title block is kept at, pixels: it is saved with the drawing.
const MAX_PIC_SIDE: u32 = 1600;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("titleblockadd", "Title Block", run_add).menu(&["Insert", "Title Block"]).alias(&["ttlb"]).params(
            "{at? ([x, y]: the lower left corner of the column; default 0,0), paper?: A4|A3|A2|A1|A0 (default A3), landscape? (default true), width? (mm), lang? (fr|en: the words of the block, default fr), \
             name?, project?, description?, address?, title?, parties? ([{role, name, address?, phone?, email?, afterImage? (under the picture), \
             logo? ({path | data: base64} a new picture, {key} one already kept, null for none)}]), image? (as a logo: the picture of the project), \
             revisions? ([{index, date, nature, by?, checked?}], the oldest first), revisionRows? (lines shown, default 4), codes? ([{label, value}]), \
             codeValues? ([\"…\"]: the values of the default codes, in order)} → {handle, pictures}",
        ),
        CommandSpec::new("titleblockedit", "Edit Title Block", run_edit).params(
            "{handle, the fields of titleblockadd, each given one replacing what the block has (a list is replaced whole), addRevision? ({index, date, \
             nature, by?, checked?}: one more line)} → {changed}",
        ),
    ]
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: DocError) -> EngineError {
    EngineError::Other(e.to_string())
}

fn text(cmd: &str, v: &Value, key: &str) -> Result<Option<String>> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.trim().to_string())),
        Some(_) => Err(bad(cmd, format!("`{key}` must be text"))),
    }
}

/// The pixels shrunk (nearest neighbour is enough for a logo seen on a sheet) so that no side is longer than [`MAX_PIC_SIDE`].
fn shrink(r: &Raster) -> Raster {
    let (w, h) = (r.width(), r.height());
    let longest = w.max(h);
    if longest <= MAX_PIC_SIDE {
        return r.clone();
    }
    let f = f64::from(MAX_PIC_SIDE) / f64::from(longest);
    let (nw, nh) = (((f64::from(w) * f).round() as u32).max(1), ((f64::from(h) * f).round() as u32).max(1));
    let mut out = Vec::with_capacity(nw as usize * nh as usize * 4);
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = (((f64::from(x) + 0.5) / f).floor() as u32, ((f64::from(y) + 0.5) / f).floor() as u32);
            out.extend_from_slice(&r.pixel(sx.min(w - 1), sy.min(h - 1)).unwrap_or([0, 0, 0, 0]));
        }
    }
    Raster::new(nw, nh, out).unwrap_or_else(|| r.clone())
}

/// A picture the caller names: a key already kept in the drawing, or a file or base64 data to decode and keep under a new key.
fn picture(s: &mut Session, cmd: &str, spec: &Value) -> Result<Pic> {
    if let Some(key) = spec.get("key").and_then(Value::as_str) {
        let r = s.doc()?.rasters.get(key).ok_or_else(|| bad(cmd, "that picture is not in the drawing"))?;
        return Ok(Pic { key: key.to_string(), width: r.width(), height: r.height() });
    }
    let name = str_param(spec, "path").unwrap_or("picture").to_string();
    let px = super::image::pixels(s, spec, &name)?;
    let r = Arc::new(shrink(&px));
    let d = s.doc_mut()?;
    let key =
        (1..).map(|n| format!("{TITLEBLOCK_IMAGE_PREFIX}{n}")).find(|k| d.rasters.get(k).is_none()).ok_or_else(|| other("no key for the picture"))?;
    let pic = Pic { key: key.clone(), width: r.width(), height: r.height() };
    d.rasters.insert(&key, r);
    Ok(pic)
}

/// `Some(None)` when the field is null (no picture), `Some(Some(_))` for a picture, `None` when the field is absent.
fn optional_picture(s: &mut Session, cmd: &str, v: &Value, key: &str) -> Result<Option<Option<Pic>>> {
    match v.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(spec) if spec.is_object() => Ok(Some(Some(picture(s, cmd, spec)?))),
        Some(_) => Err(bad(cmd, format!("`{key}` must be an object ({{path}}, {{data}} or {{key}}) or null"))),
    }
}

fn parties(s: &mut Session, cmd: &str, list: &Value) -> Result<Vec<Party>> {
    let items = list.as_array().ok_or_else(|| bad(cmd, "`parties` must be a list"))?;
    if items.len() > MAX_PARTIES {
        return Err(bad(cmd, format!("a title block has at most {MAX_PARTIES} parties")));
    }
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let logo = optional_picture(s, cmd, it, "logo")?.flatten();
        out.push(Party {
            role: text(cmd, it, "role")?.unwrap_or_default(),
            name: text(cmd, it, "name")?.unwrap_or_default(),
            address: text(cmd, it, "address")?.unwrap_or_default(),
            phone: text(cmd, it, "phone")?.unwrap_or_default(),
            email: text(cmd, it, "email")?.unwrap_or_default(),
            logo,
            after_image: it.get("afterImage").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    Ok(out)
}

fn revision(cmd: &str, it: &Value) -> Result<Revision> {
    Ok(Revision {
        index: text(cmd, it, "index")?.unwrap_or_default(),
        date: text(cmd, it, "date")?.unwrap_or_default(),
        nature: text(cmd, it, "nature")?.unwrap_or_default(),
        by: text(cmd, it, "by")?.unwrap_or_default(),
        checked: text(cmd, it, "checked")?.unwrap_or_default(),
    })
}

fn revisions(cmd: &str, list: &Value) -> Result<Vec<Revision>> {
    let items = list.as_array().ok_or_else(|| bad(cmd, "`revisions` must be a list"))?;
    if items.len() > MAX_REVISIONS {
        return Err(bad(cmd, format!("a title block has at most {MAX_REVISIONS} revisions")));
    }
    items.iter().map(|it| revision(cmd, it)).collect()
}

fn codes(cmd: &str, list: &Value) -> Result<Vec<Code>> {
    let items = list.as_array().ok_or_else(|| bad(cmd, "`codes` must be a list"))?;
    if items.len() > MAX_CODES {
        return Err(bad(cmd, format!("a title block has at most {MAX_CODES} codes")));
    }
    items
        .iter()
        .map(|it| Ok(Code { label: text(cmd, it, "label")?.unwrap_or_default(), value: text(cmd, it, "value")?.unwrap_or_default() }))
        .collect()
}

fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a number"))),
    }
}

/// Puts the fields `p` names into `t`.
fn apply(s: &mut Session, cmd: &str, p: &Value, t: &mut TitleBlockParams) -> Result<()> {
    if let Some(v) = text(cmd, p, "name")? {
        t.name = v;
    }
    if let Some(v) = text(cmd, p, "paper")? {
        t.paper = v.to_ascii_uppercase();
    }
    if let Some(v) = p.get("landscape").and_then(Value::as_bool) {
        t.landscape = v;
    }
    if let Some(v) = number(cmd, p, "width")? {
        t.width_mm = v;
    }
    if let Some(v) = text(cmd, p, "lang")? {
        t.lang = if v == "en" { "en".into() } else { "fr".into() };
    }
    for (key, field) in [("project", 0), ("description", 1), ("address", 2), ("title", 3)] {
        if let Some(v) = text(cmd, p, key)? {
            match field {
                0 => t.project = v,
                1 => t.description = v,
                2 => t.address = v,
                _ => t.title = v,
            }
        }
    }
    if let Some(list) = p.get("parties") {
        t.parties = parties(s, cmd, list)?;
    }
    if let Some(pic) = optional_picture(s, cmd, p, "image")? {
        t.image = pic;
    }
    if let Some(list) = p.get("revisions") {
        t.revisions = revisions(cmd, list)?;
    }
    if let Some(one) = p.get("addRevision").filter(|v| v.is_object()) {
        t.revisions.push(revision(cmd, one)?);
        t.revision_rows = t.revision_rows.max(t.revisions.len());
    }
    if let Some(n) = number(cmd, p, "revisionRows")? {
        if !(0.0..=MAX_REVISIONS as f64).contains(&n) {
            return Err(bad(cmd, format!("`revisionRows` is 0 to {MAX_REVISIONS}")));
        }
        t.revision_rows = n as usize;
    }
    if let Some(list) = p.get("codes") {
        t.codes = codes(cmd, list)?;
    }
    if let Some(values) = p.get("codeValues").and_then(Value::as_array) {
        for (c, v) in t.codes.iter_mut().zip(values) {
            c.value = v.as_str().unwrap_or_default().trim().to_string();
        }
    }
    Ok(())
}

/// Drops the pictures no title block of the drawing shows any more.
fn forget_unused_pictures(s: &mut Session) -> Result<()> {
    let d = s.doc()?;
    let mut used = std::collections::BTreeSet::new();
    let spaces = std::iter::once(&d.model).chain(d.layouts.iter().map(|l| &l.entities));
    for e in spaces.flat_map(|st| st.iter()) {
        if let Some(t) = titleblock_of(e) {
            used.extend(t.picture_keys().into_iter().map(str::to_string));
        }
    }
    let stale: Vec<String> =
        d.rasters.iter().map(|(k, _)| k).filter(|k| k.starts_with(TITLEBLOCK_IMAGE_PREFIX) && !used.contains(*k)).map(str::to_string).collect();
    if !stale.is_empty() {
        let d = s.doc_mut()?;
        for k in stale {
            d.rasters.remove(&k);
        }
    }
    Ok(())
}

fn run_add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "titleblockadd";
    let name = text(CMD, p, "name")?.filter(|n| !n.is_empty()).unwrap_or_else(|| "Title block".into());
    let lang = text(CMD, p, "lang")?.filter(|l| l == "en").unwrap_or_else(|| "fr".into());
    let mut t = TitleBlockParams::new_in(&lang, &name, "A3", true, HvacUnits::of(s.doc()?));
    // A paper given without a width takes the width that paper starts with.
    if text(CMD, p, "paper")?.is_some() && number(CMD, p, "width")?.is_none() {
        t.width_mm = cadcraft_doc::default_width_mm(&text(CMD, p, "paper")?.unwrap_or_default().to_ascii_uppercase());
    }
    apply(s, CMD, p, &mut t)?;
    let at = match p.get("at") {
        None | Some(Value::Null) => Vec2::ZERO,
        Some(v) => super::point_value(v).ok_or_else(|| bad(CMD, "`at` must be [x, y]"))?,
    };
    t.validate().map_err(|e| bad(CMD, e.to_string()))?;
    let pictures = t.picture_keys().len();
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_titleblock(&space, common, at, t).map_err(doc_err)?;
    forget_unused_pictures(s)?;
    Ok(json!({ "handle": h.hex(), "pictures": pictures }))
}

fn run_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "titleblockedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.len() != 1 {
        return Err(bad(CMD, "give the `handle` of one title block"));
    }
    let h: Handle = hs[0];
    let mut t =
        titleblock_of(s.doc()?.entity(h).ok_or_else(|| bad(CMD, "no such object"))?).ok_or_else(|| bad(CMD, "that object is not a title block"))?;
    // Changing the paper takes its starting width unless a width is given too.
    let before = t.paper.clone();
    apply(s, CMD, p, &mut t)?;
    if t.paper != before && number(CMD, p, "width")?.is_none() {
        t.width_mm = cadcraft_doc::default_width_mm(&t.paper);
    }
    t.validate().map_err(|e| bad(CMD, e.to_string()))?;
    s.doc_mut()?.regenerate_titleblock(h, &t).map_err(doc_err)?;
    forget_unused_pictures(s)?;
    Ok(json!({ "changed": 1 }))
}
