//! Structure in plan: COLUMNADD draws steel columns (IPN, IPE, HEA, HEB, UPN sections), BEAMADD draws beams between points, STRUCTEDIT changes
//! what is drawn, STRUCTCLEANUP cuts the beams at the columns and beams they meet. A point lands on the centre of a column, the end of a beam or the
//! corner of two walls when it is close to one.

use cadcraft_doc::{Handle, HvacUnits, StructKind, StructParams, struct_of};
use cadcraft_geom::Vec2;
use cadcraft_hvac::{SteelSection, parse_section};
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most objects one command draws or changes.
const MAX_OBJECTS: usize = 10_000;
/// How close, in pixels, a point must be to land on a column, a beam's end or a corner of the walls.
const SNAP_PIXELS: f64 = 12.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("columnadd", "Column", run_columnadd)
            .menu(&["Draw", "Architecture", "Column"])
            .params(
                "{at ([x, y]) | points ([[x, y]…]: one column at each), section? (\"HEA 200\": IPN, IPE, HEA, HEB or UPN with its size), name? (the tag; default C1, C2…), \
                 height? (mm), elevation? (m, the foot), rotation? (degrees), snap? (mm: land on the centre of a column, the end of a beam or the corner of two walls within this distance), \
                 labelHeight? (mm)} → {handles}",
            )
            .interactive(|s| Ok(Box::new(ColumnAddM::new(s)))),
        CommandSpec::new("beamadd", "Beam", run_beamadd)
            .menu(&["Draw", "Architecture", "Beam"])
            .params(
                "{points (2 or more: one beam per segment), section? (\"IPE 240\"), name? (the tag; default B1, B2…), elevation? (m, the underside), snap? (mm, as COLUMNADD), \
                 labelHeight? (mm)} → {handles, trimmed (beams cut at a column or a beam)}",
            )
            .interactive(|s| Ok(Box::new(BeamAddM::new(s)))),
        CommandSpec::new("structedit", "Edit Structure", run_structedit).params(
            "{handle | handles, section?, name?, height? (mm, a column), elevation? (m: a column's foot, a beam's underside), rotation? (degrees, a column), showLabel?, labelHeight? (mm)} → {changed}",
        ),
        CommandSpec::new("structtakeoff", "Structure Takeoff", run_structtakeoff)
            .menu(&["Draw", "Architecture", "Structure Takeoff"])
            .params(
                "{path? (a .xlsx workbook, or a text file with `;` between the columns), decimal?: comma|point (the text file, default comma)} → {columns, beams, slabs, steelKg, \
                 slabAreaM2, slabVolumeM3}: the steel columns and beams by section (count, length, mass), the slabs by kind (count, net area and volume), the false ceilings (tiles whole and cut), the stairs and the railings",
            ),
        CommandSpec::new("structcleanup", "Structure Joints", run_structcleanup)
            .menu(&["Draw", "Architecture", "Structure Joints"])
            .params("{} → {regenerated}"),
    ]
}

/// What COLUMNADD and BEAMADD remember between uses.
#[derive(Clone, Debug, PartialEq)]
pub struct StructLastUsed {
    /// The section ("HEA 200").
    pub section: String,
    /// A new column's height and foot, mm.
    pub column_height_mm: f64,
    pub column_elevation_mm: f64,
    /// A new beam's underside, mm.
    pub beam_elevation_mm: f64,
}

impl Default for StructLastUsed {
    fn default() -> Self {
        StructLastUsed { section: "HEA 200".into(), column_height_mm: 3000.0, column_elevation_mm: 0.0, beam_elevation_mm: 3000.0 }
    }
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

fn number(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be a number"))),
    }
}

fn section_param(cmd: &str, p: &Value) -> Result<Option<&'static SteelSection>> {
    match str_param(p, "section") {
        Some(t) => parse_section(t).map(Some).ok_or_else(|| {
            bad(cmd, format!("`section` \"{}\" is not in the table (IPN, IPE, HEA, HEB or UPN with a size)", t.chars().take(40).collect::<String>()))
        }),
        None => Ok(None),
    }
}

/// The tolerance, drawing units, to land on the structure or the walls: from the `snap` parameter (mm) in a call, from the pixels on screen.
fn snap_tolerance(s: &Session, mm: Option<f64>) -> Result<f64> {
    let units = HvacUnits::of(s.doc()?);
    Ok(match mm {
        Some(mm) => units.to_units(mm.max(0.0)),
        None => {
            let px = s.pixel_size();
            if px.is_finite() && px > 0.0 { px * SNAP_PIXELS } else { 0.0 }
        }
    })
}

fn snapped(s: &Session, p: Vec2, tol: f64) -> Result<Vec2> {
    Ok(s.doc()?.structure_point_near(&s.space(), p, tol).unwrap_or(p))
}

/// The tag a new object of `kind` gets: the next free number after the ones drawn.
fn next_tag(s: &Session, kind: StructKind) -> Result<String> {
    let d = s.doc()?;
    let n = d.model.iter().filter_map(|e| struct_of(e)).filter(|o| o.kind == kind).count() + 1;
    Ok(format!("{}{n}", kind.tag_prefix()))
}

fn section_of(text: &str) -> Result<&'static SteelSection> {
    parse_section(text).ok_or_else(|| other(format!("There is no section \"{}\" in the table.", text.chars().take(40).collect::<String>())))
}

fn add_column(s: &mut Session, last: &StructLastUsed, at: Vec2, rotation: f64, name: Option<&str>) -> Result<(Handle, String)> {
    let tag = match name {
        Some(n) => n.to_string(),
        None => next_tag(s, StructKind::Column)?,
    };
    let mut p = StructParams::column(&tag, section_of(&last.section)?, HvacUnits::of(s.doc()?));
    p.height_mm = last.column_height_mm;
    p.elevation_mm = last.column_elevation_mm;
    p.validate().map_err(doc_err)?;
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_column(&space, common, p, at, rotation).map_err(doc_err)?;
    Ok((h, tag))
}

fn add_beam(s: &mut Session, last: &StructLastUsed, a: Vec2, b: Vec2, name: Option<&str>) -> Result<(Handle, String)> {
    if a.near(b, 1e-9) {
        return Err(other("A beam needs two different points."));
    }
    let tag = match name {
        Some(n) => n.to_string(),
        None => next_tag(s, StructKind::Beam)?,
    };
    let mut p = StructParams::beam(&tag, section_of(&last.section)?, a.dist(b), HvacUnits::of(s.doc()?));
    p.elevation_mm = last.beam_elevation_mm;
    p.validate().map_err(doc_err)?;
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_beam(&space, common, p, a, b).map_err(doc_err)?;
    Ok((h, tag))
}

/// Cuts the beams at the columns and the beams they meet, and says how many were cut.
fn cleanup(s: &mut Session) -> Result<usize> {
    let space = s.space();
    s.doc_mut()?.cleanup_structure(&space).map_err(doc_err)
}

fn points(cmd: &str, p: &Value) -> Result<Vec<Vec2>> {
    let mut pts = super::points_param(p, "points").unwrap_or_default();
    if let Some(at) = super::points_param(p, "at").and_then(|v| v.first().copied()).or_else(|| super::point_param(p, "at")) {
        pts.push(at);
    }
    if pts.len() > MAX_OBJECTS + 1 {
        return Err(bad(cmd, format!("at most {MAX_OBJECTS} objects")));
    }
    Ok(pts)
}

fn run_columnadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "columnadd";
    let pts = points(CMD, p)?;
    if pts.is_empty() {
        return Err(bad(CMD, "`at` or `points` ([x, y]) is required"));
    }
    let mut last = StructLastUsed::default();
    if let Some(sec) = section_param(CMD, p)? {
        last.section = sec.designation();
    }
    last.column_height_mm = number(CMD, p, "height")?.unwrap_or(last.column_height_mm);
    last.column_elevation_mm = number(CMD, p, "elevation")?.map_or(last.column_elevation_mm, |m| m * 1000.0);
    let rotation = number(CMD, p, "rotation")?.unwrap_or(0.0).to_radians();
    let tol = snap_tolerance(s, number(CMD, p, "snap")?)?;
    let label = number(CMD, p, "labelHeight")?;
    let mut handles = Vec::new();
    for at in pts {
        let at = snapped(s, at, tol)?;
        let (h, _) = add_column(s, &last, at, rotation, str_param(p, "name").filter(|_| handles.is_empty()))?;
        if let Some(l) = label {
            set_label_height(s, h, l)?;
        }
        handles.push(h.hex());
    }
    cleanup(s)?;
    Ok(json!({ "handles": handles }))
}

fn set_label_height(s: &mut Session, h: Handle, mm: f64) -> Result<()> {
    let mut o = s.doc()?.entity(h).and_then(|e| struct_of(e)).ok_or_else(|| other("Select structural objects only."))?;
    o.label_height_mm = mm;
    o.validate().map_err(doc_err)?;
    s.doc_mut()?.regenerate_struct(h, &o).map_err(doc_err)
}

fn run_beamadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "beamadd";
    let pts = points(CMD, p)?;
    if pts.len() < 2 {
        return Err(bad(CMD, "`points` (two or more [x, y]) is required"));
    }
    let mut last = StructLastUsed::default();
    if let Some(sec) = section_param(CMD, p)? {
        last.section = sec.designation();
    } else {
        last.section = "IPE 240".into();
    }
    last.beam_elevation_mm = number(CMD, p, "elevation")?.map_or(last.beam_elevation_mm, |m| m * 1000.0);
    let tol = snap_tolerance(s, number(CMD, p, "snap")?)?;
    let label = number(CMD, p, "labelHeight")?;
    let pts: Vec<Vec2> = pts.into_iter().map(|q| snapped(s, q, tol)).collect::<Result<_>>()?;
    let mut handles = Vec::new();
    for w in pts.windows(2) {
        if let [a, b] = w {
            let (h, _) = add_beam(s, &last, *a, *b, str_param(p, "name").filter(|_| handles.is_empty()))?;
            if let Some(l) = label {
                set_label_height(s, h, l)?;
            }
            handles.push(h.hex());
        }
    }
    let trimmed = cleanup(s)?;
    Ok(json!({ "handles": handles, "trimmed": trimmed }))
}

fn run_structedit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "structedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_OBJECTS {
        return Err(bad(CMD, format!("select 1 to {MAX_OBJECTS} structural objects")));
    }
    let section = section_param(CMD, p)?;
    let height = number(CMD, p, "height")?;
    let elevation = number(CMD, p, "elevation")?.map(|m| m * 1000.0);
    let rotation = number(CMD, p, "rotation")?.map(f64::to_radians);
    let label = number(CMD, p, "labelHeight")?;
    let show = p.get("showLabel").and_then(Value::as_bool);
    let mut next: Vec<(Handle, StructParams)> = Vec::with_capacity(hs.len());
    for h in &hs {
        let mut o = s.doc()?.entity(*h).and_then(|e| struct_of(e)).ok_or_else(|| other("Select structural objects only."))?;
        if let Some(sec) = section {
            o.section = sec.designation();
        }
        if let Some(n) = str_param(p, "name") {
            o.name = n.trim().to_string();
        }
        if let Some(v) = height {
            if o.kind != StructKind::Column {
                return Err(bad(CMD, "`height` is for a column: a beam's length is its two points"));
            }
            o.height_mm = v;
        }
        if rotation.is_some() && o.kind != StructKind::Column {
            return Err(bad(CMD, "`rotation` is for a column: a beam follows its two points"));
        }
        o.elevation_mm = elevation.unwrap_or(o.elevation_mm);
        o.label_height_mm = label.unwrap_or(o.label_height_mm);
        o.show_label = show.unwrap_or(o.show_label);
        o.validate().map_err(doc_err)?;
        next.push((*h, o));
    }
    let d = s.doc_mut()?;
    for (h, o) in &next {
        // A beam changing section changes its cut ends: they are found again below.
        d.regenerate_struct(*h, o).map_err(doc_err)?;
        if let Some(r) = rotation {
            d.modify_entity(*h, |e| {
                if let cadcraft_doc::EntityKind::Insert(i) = &mut e.kind {
                    i.rotation = r;
                }
            })
            .map_err(doc_err)?;
        }
    }
    cleanup(s)?;
    Ok(json!({ "changed": next.len() }))
}

fn run_structtakeoff(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "structtakeoff";
    let comma = match str_param(p, "decimal").map(|t| t.trim().to_ascii_lowercase()).as_deref() {
        None | Some("comma") => true,
        Some("point") => false,
        Some(_) => return Err(bad(CMD, "`decimal` is comma or point")),
    };
    let space = s.space();
    let t = s.doc()?.struct_takeoff(&space);
    let mut out = serde_json::to_value(&t).map_err(|e| other(e.to_string()))?;
    if let Some(path) = str_param(p, "path").map(str::trim).filter(|x| !x.is_empty()) {
        if path.len() > 1024 || path.contains('\0') {
            return Err(bad(CMD, "`path` is not a usable file name"));
        }
        let path = std::path::Path::new(path);
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        }
        let bytes = if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("xlsx")) {
            t.to_xlsx().map_err(doc_err)?
        } else {
            let mut b = vec![0xEF, 0xBB, 0xBF];
            b.extend_from_slice(t.csv(comma).as_bytes());
            b
        };
        std::fs::write(path, bytes).map_err(|e| other(format!("Cannot write {}: {e}", path.display())))?;
        out["path"] = json!(path.display().to_string());
    }
    Ok(out)
}

fn run_structcleanup(s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({ "regenerated": cleanup(s)? }))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    Point,
    Section,
    Height,
    Elevation,
    Rotation,
}

fn echo_section(s: &mut Session, text: &str) {
    if let Some(x) = parse_section(text) {
        s.echo(format!("Section {}: {:.0} x {:.0} mm, {:.1} kg/m", x.designation(), x.h_mm, x.b_mm, x.kg_m));
    }
}

struct ColumnAddM {
    last: StructLastUsed,
    rotation_deg: f64,
    ask: Ask,
}

impl ColumnAddM {
    fn new(s: &Session) -> Self {
        ColumnAddM { last: s.last_used.structure.clone(), rotation_deg: 0.0, ask: Ask::Point }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!(
            "Current settings: Section = {}, Height = {:.0} mm, Elevation = {:.2} m, Rotation = {:.0}",
            self.last.section,
            self.last.column_height_mm,
            self.last.column_elevation_mm / 1000.0,
            self.rotation_deg
        ));
    }
}

fn parse_number(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

impl Interactive for ColumnAddM {
    fn name(&self) -> &'static str {
        "COLUMNADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Section => Prompt::new("Enter the section (e.g. HEA 200)", Accept::TEXT).default(self.last.section.clone()),
            Ask::Height => Prompt::new("Enter the height of the column (mm)", Accept::NUMBER).default(format!("{:.0}", self.last.column_height_mm)),
            Ask::Elevation => {
                Prompt::new("Enter the elevation of the foot (m)", Accept::NUMBER).default(format!("{:.2}", self.last.column_elevation_mm / 1000.0))
            }
            Ask::Rotation => Prompt::new("Enter the rotation (degrees)", Accept::NUMBER).default(format!("{:.0}", self.rotation_deg)),
            Ask::Point => Prompt::new("Specify the position of the column", Accept::POINT).kw(&["Section", "Height", "Elevation", "Rotation"]),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        if self.ask != Ask::Point {
            match (self.ask, text.as_deref()) {
                (Ask::Section, Some(t)) => match parse_section(t) {
                    Some(x) => {
                        self.last.section = x.designation();
                        echo_section(s, &self.last.section);
                    }
                    None => s.echo("Enter a section of the table: IPN, IPE, HEA, HEB or UPN with a size (e.g. HEA 200)."),
                },
                (Ask::Height, Some(t)) => match parse_number(t).filter(|v| *v >= 10.0) {
                    Some(v) => self.last.column_height_mm = v,
                    None => s.echo("Requires a height in mm."),
                },
                (Ask::Elevation, Some(t)) => match parse_number(t) {
                    Some(v) => self.last.column_elevation_mm = v * 1000.0,
                    None => s.echo("Requires an elevation in metres."),
                },
                (Ask::Rotation, Some(t)) => match parse_number(t) {
                    Some(v) => self.rotation_deg = v,
                    None => s.echo("Requires an angle in degrees."),
                },
                _ => {}
            }
            self.ask = Ask::Point;
            self.settings(s);
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "Section" => self.ask = Ask::Section,
                    "Height" => self.ask = Ask::Height,
                    "Elevation" => self.ask = Ask::Elevation,
                    "Rotation" => self.ask = Ask::Rotation,
                    _ => {}
                }
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                let tol = snap_tolerance(s, None)?;
                let at = snapped(s, p, tol)?;
                match add_column(s, &self.last, at, self.rotation_deg.to_radians(), None) {
                    Ok((_, tag)) => {
                        s.last_used.structure = self.last.clone();
                        cleanup(s)?;
                        s.echo(format!("Column {tag} {}", self.last.section));
                    }
                    Err(e) => s.echo(e.to_string()),
                }
                Ok(Step::Continue)
            }
            Input::Enter | Input::Cancel => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        let Some(x) = parse_section(&self.last.section) else { return Vec::new() };
        let Ok(d) = s.doc() else { return Vec::new() };
        let units = HvacUnits::of(d);
        let (c0, s0) = (self.rotation_deg.to_radians().cos(), self.rotation_deg.to_radians().sin());
        let vertices = StructParams::section_outline_mm(x)
            .into_iter()
            .map(|(px, py)| {
                let (u, v) = (units.to_units(px), units.to_units(py));
                cadcraft_geom::PolyVertex { p: Vec2::new(c.x + u * c0 - v * s0, c.y + u * s0 + v * c0), bulge: 0.0, start_width: 0.0, end_width: 0.0 }
            })
            .collect();
        vec![cadcraft_doc::EntityKind::LwPolyline(cadcraft_doc::LwPolyline {
            vertices,
            closed: true,
            const_width: 0.0,
            elevation: 0.0,
            plinegen: false,
        })]
    }
}

struct BeamAddM {
    last: StructLastUsed,
    pts: Vec<Vec2>,
    ask: Ask,
}

impl BeamAddM {
    fn new(s: &Session) -> Self {
        let mut last = s.last_used.structure.clone();
        // A beam is an IPE or an IPN, not the HEA the columns start with: a column's section is kept only when it was chosen.
        if last == StructLastUsed::default() {
            last.section = "IPE 240".into();
        }
        BeamAddM { last, pts: Vec::new(), ask: Ask::Point }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!("Current settings: Section = {}, Underside = {:.2} m", self.last.section, self.last.beam_elevation_mm / 1000.0));
    }
}

impl Interactive for BeamAddM {
    fn name(&self) -> &'static str {
        "BEAMADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            Ask::Section => Prompt::new("Enter the section (e.g. IPE 240)", Accept::TEXT).default(self.last.section.clone()),
            Ask::Elevation => Prompt::new("Enter the elevation of the underside (m)", Accept::NUMBER)
                .default(format!("{:.2}", self.last.beam_elevation_mm / 1000.0)),
            _ => match self.pts.last().copied() {
                None => Prompt::new("Specify start point of beam", Accept::POINT).kw(&["Section", "Elevation"]),
                last => Prompt::new("Specify next point", Accept::POINT).kw(&["Undo"]).base_opt(last),
            },
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        if matches!(self.ask, Ask::Section | Ask::Elevation) {
            match (self.ask, text.as_deref()) {
                (Ask::Section, Some(t)) => match parse_section(t) {
                    Some(x) => {
                        self.last.section = x.designation();
                        echo_section(s, &self.last.section);
                    }
                    None => s.echo("Enter a section of the table: IPN, IPE, HEA, HEB or UPN with a size (e.g. IPE 240)."),
                },
                (Ask::Elevation, Some(t)) => match parse_number(t) {
                    Some(v) => self.last.beam_elevation_mm = v * 1000.0,
                    None => s.echo("Requires an elevation in metres."),
                },
                _ => {}
            }
            self.ask = Ask::Point;
            self.settings(s);
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "Section" => self.ask = Ask::Section,
                    "Elevation" => self.ask = Ask::Elevation,
                    "Undo" => {
                        self.pts.pop();
                    }
                    _ => {}
                }
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                let tol = snap_tolerance(s, None)?;
                let p = snapped(s, p, tol)?;
                if let Some(prev) = self.pts.last().copied() {
                    match add_beam(s, &self.last, prev, p, None) {
                        Ok((_, tag)) => {
                            s.last_used.structure = self.last.clone();
                            cleanup(s)?;
                            s.echo(format!("Beam {tag} {}", self.last.section));
                        }
                        Err(e) => {
                            s.echo(e.to_string());
                            return Ok(Step::Continue);
                        }
                    }
                }
                self.pts.push(p);
                Ok(Step::Continue)
            }
            Input::Enter | Input::Cancel => Ok(Step::Done),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        match self.pts.last() {
            Some(p) => vec![cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: p.to3(0.0), b: c.to3(0.0) })],
            None => Vec::new(),
        }
    }
}
