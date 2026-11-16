//! Design objects in plan: CEILINGADD draws the grid of a false ceiling over a room, STAIRADD a straight stair, RAILADD a railing, DESIGNEDIT changes
//! them. The tile centres of a ceiling grid are where a diffuser or a luminaire of the false ceiling is put (`ceilingSnap` of the air terminals and of the
//! electrical devices).

use cadcraft_doc::{CeilingParams, Handle, HvacUnits, RailParams, StairParams, ceiling_of, rail_of, stair_of};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

/// Most objects one command draws or changes, and most vertices of an outline or a path.
const MAX_OBJECTS: usize = 10_000;
const MAX_POINTS: usize = cadcraft_doc::MAX_DESIGN_VERTICES;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("ceilingadd", "Ceiling Grid", run_ceilingadd)
            .menu(&["Draw", "Architecture", "Ceiling Grid"])
            .params(
                "{points (3 or more) | handle (a closed polyline) | inside ([x, y]: a point in a room closed by walls) | space (a room of the HVAC tab: its outline), tile? (\"600x600\", \"1200x600\"…, mm), angle? (degrees), \
                 elevation? (m, the underside of the false ceiling), name?, labelHeight? (mm)} → {handle, tiles: {whole, cut}, areaM2}",
            )
            .interactive(|s| Ok(Box::new(CeilingAddM::new(s)))),
        CommandSpec::new("stairadd", "Stair", run_stairadd)
            .menu(&["Draw", "Architecture", "Stair"])
            .params(
                "{at ([x, y]: the middle of the first riser), angle? (degrees: the way up along the flight), rise? (mm, floor to floor, default 3000), riserMax? (mm, default 180), \
                 going? (mm, default 280), width? (mm, default 1000), up? (false: the arrow points down), elevation? (m, the lower floor), name?, labelHeight? (mm)} \
                 → {handle, risers, riser (mm), run (mm), comfortable (Blondel: 2 riser + going within 600 to 650 mm)}",
            )
            .interactive(|s| Ok(Box::new(StairAddM::new(s)))),
        CommandSpec::new("railadd", "Railing", run_railadd)
            .menu(&["Draw", "Architecture", "Railing"])
            .params(
                "{points (2 or more), height? (mm, default 1000), postSpacing? (mm, default 1200), elevation? (m), name?, labelHeight? (mm)} → {handle, posts, length (m)}",
            )
            .interactive(|s| Ok(Box::new(RailAddM::new(s)))),
        CommandSpec::new("designedit", "Edit Design Object", run_designedit).params(
            "{handle | handles: ceiling grids, stairs or railings, name?, elevation? (m), showLabel?; ceiling: tile? (\"600x1200\"), angle? (degrees); stair: rise?, riserMax?, going?, width?, up?; \
             railing: height?, postSpacing?} → {changed}",
        ),
    ]
}

/// What STAIRADD remembers between uses.
#[derive(Clone, Debug, PartialEq)]
pub struct DesignLastUsed {
    /// The tile of a ceiling grid, mm, and the height of the false ceiling, mm.
    pub tile_mm: (f64, f64),
    pub ceiling_mm: f64,
    /// A stair's rise, riser, tread and width, mm.
    pub rise_mm: f64,
    pub riser_max_mm: f64,
    pub going_mm: f64,
    pub width_mm: f64,
    /// A railing's height, mm.
    pub rail_height_mm: f64,
}

impl Default for DesignLastUsed {
    fn default() -> Self {
        DesignLastUsed {
            tile_mm: (600.0, 600.0),
            ceiling_mm: 2600.0,
            rise_mm: 3000.0,
            riser_max_mm: 180.0,
            going_mm: 280.0,
            width_mm: 1000.0,
            rail_height_mm: 1000.0,
        }
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

fn flag(cmd: &str, p: &Value, key: &str) -> Result<Option<bool>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| bad(cmd, format!("`{key}` must be true or false"))),
    }
}

/// A tile typed as "600x600", "600 x 1200", "60x120" (centimetres when a side is under 100) or a single side for a square one.
pub fn parse_tile(text: &str) -> Option<(f64, f64)> {
    let t = text.trim().to_ascii_lowercase().replace(['×', '*'], "x").replace(',', ".");
    let sides: Vec<f64> = t.split('x').map(|v| v.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0)).collect::<Option<_>>()?;
    // A side under 100 means the tile is told in centimetres (a tile is never that small in millimetres): both sides are.
    let k = if sides.iter().any(|v| *v < 100.0) { 10.0 } else { 1.0 };
    match sides[..] {
        [a] => Some((a * k, a * k)),
        [a, b] => Some((a * k, b * k)),
        _ => None,
    }
}

fn tile_param(cmd: &str, p: &Value) -> Result<Option<(f64, f64)>> {
    match p.get("tile") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) => parse_tile(t).map(Some).ok_or_else(|| bad(cmd, "`tile` is like \"600x600\" or \"600x1200\" (mm)")),
        Some(Value::Number(n)) => n.as_f64().map(|v| Some((v, v))).ok_or_else(|| bad(cmd, "`tile` is like \"600x600\"")),
        Some(_) => Err(bad(cmd, "`tile` is like \"600x600\" or \"600x1200\" (mm)")),
    }
}

fn next_name(s: &Session, prefix: &str, count: impl Fn(&cadcraft_doc::Entity) -> bool) -> Result<String> {
    let n = s.doc()?.model.iter().filter(|e| count(e)).count() + 1;
    Ok(format!("{prefix}{n}"))
}

fn add_ceiling(s: &mut Session, p: CeilingParams) -> Result<(Handle, cadcraft_doc::TileCount)> {
    p.validate().map_err(doc_err)?;
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_ceiling(&space, common, p).map_err(doc_err)?;
    let tiles = s.doc()?.entity(h).and_then(|e| ceiling_of(e)).map(|c| c.tiles()).unwrap_or_default();
    Ok((h, tiles))
}

fn run_ceilingadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "ceilingadd";
    let outline = if let Some(h) = super::curves::handle_param(p, "space") {
        s.doc()?.space_outline_world(h).ok_or_else(|| other("Select a room of the HVAC tab (a space)."))?
    } else if let Some(h) = super::curves::handle_param(p, "handle") {
        super::hvac::polyline_outline(s.doc()?, h)?
    } else if let Some(q) = p.get("inside").and_then(super::point_value) {
        super::slab::room_outline_at(s, q)?
    } else if let Some(Value::Array(raw)) = p.get("points") {
        if raw.len() > MAX_POINTS + 1 {
            return Err(bad(CMD, format!("a ceiling has at most {MAX_POINTS} points")));
        }
        let pts: Option<Vec<Vec2>> = raw.iter().map(super::point_value).collect();
        let mut v = super::distinct_points(&pts.ok_or_else(|| bad(CMD, "`points` must be [x, y] points"))?);
        if v.len() > 3 && v.first().zip(v.last()).is_some_and(|(a, b)| a.near(*b, 1e-12)) {
            v.pop();
        }
        v
    } else {
        return Err(bad(
            CMD,
            "`points` (3 or more), `handle` (a closed polyline) or `space` (a room) or `inside` (a point in a room closed by walls) is required",
        ));
    };
    let mut last = DesignLastUsed::default();
    if let Some(t) = tile_param(CMD, p)? {
        last.tile_mm = t;
    }
    last.ceiling_mm = number(CMD, p, "elevation")?.map_or(last.ceiling_mm, |m| m * 1000.0);
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => next_name(s, "FP", |e| ceiling_of(e).is_some())?,
    };
    let mut c = CeilingParams::new(&name, outline, HvacUnits::of(s.doc()?));
    (c.tile_x_mm, c.tile_y_mm, c.elevation_mm) = (last.tile_mm.0, last.tile_mm.1, last.ceiling_mm);
    c.angle_deg = number(CMD, p, "angle")?.unwrap_or(0.0);
    c.label_height_mm = number(CMD, p, "labelHeight")?.unwrap_or(c.label_height_mm);
    let (h, tiles) = add_ceiling(s, c)?;
    Ok(json!({ "handle": h.hex(), "tiles": { "whole": tiles.whole, "cut": tiles.cut }, "areaM2": tiles.area_m2 }))
}

fn add_stair(s: &mut Session, p: StairParams, at: Vec2, rotation: f64) -> Result<Handle> {
    p.validate().map_err(doc_err)?;
    let (space, common) = (s.space(), s.current_common());
    s.doc_mut()?.add_stair(&space, common, p, at, rotation).map_err(doc_err)
}

fn stair_from(s: &Session, last: &DesignLastUsed, name: &str) -> Result<StairParams> {
    let mut st = StairParams::new(name, HvacUnits::of(s.doc()?));
    (st.rise_mm, st.riser_max_mm, st.going_mm, st.width_mm) = (last.rise_mm, last.riser_max_mm, last.going_mm, last.width_mm);
    Ok(st)
}

fn run_stairadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "stairadd";
    let at = super::point_param(p, "at").ok_or_else(|| bad(CMD, "`at` ([x, y]) is required"))?;
    let mut last = DesignLastUsed::default();
    last.rise_mm = number(CMD, p, "rise")?.unwrap_or(last.rise_mm);
    last.riser_max_mm = number(CMD, p, "riserMax")?.unwrap_or(last.riser_max_mm);
    last.going_mm = number(CMD, p, "going")?.unwrap_or(last.going_mm);
    last.width_mm = number(CMD, p, "width")?.unwrap_or(last.width_mm);
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => next_name(s, "E", |e| stair_of(e).is_some())?,
    };
    let mut st = stair_from(s, &last, &name)?;
    st.up = flag(CMD, p, "up")?.unwrap_or(true);
    st.elevation_mm = number(CMD, p, "elevation")?.map_or(0.0, |m| m * 1000.0);
    st.label_height_mm = number(CMD, p, "labelHeight")?.unwrap_or(st.label_height_mm);
    let angle = number(CMD, p, "angle")?.unwrap_or(0.0).to_radians();
    st.validate().map_err(doc_err)?;
    let (risers, riser, run, comfortable) = (st.risers(), st.riser_mm(), st.run_mm(), st.comfortable());
    let h = add_stair(s, st, at, angle)?;
    Ok(json!({ "handle": h.hex(), "risers": risers, "riser": riser, "run": run, "comfortable": comfortable }))
}

fn add_rail(s: &mut Session, p: RailParams) -> Result<(Handle, usize, f64)> {
    p.validate().map_err(doc_err)?;
    let (posts, length) = (p.posts().len(), p.length_mm() / 1000.0);
    let (space, common) = (s.space(), s.current_common());
    let h = s.doc_mut()?.add_rail(&space, common, p).map_err(doc_err)?;
    Ok((h, posts, length))
}

fn run_railadd(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "railadd";
    let raw = p.get("points").and_then(Value::as_array).ok_or_else(|| bad(CMD, "`points` (2 or more) is required"))?;
    if raw.len() > MAX_POINTS {
        return Err(bad(CMD, format!("a railing has at most {MAX_POINTS} points")));
    }
    let pts: Option<Vec<Vec2>> = raw.iter().map(super::point_value).collect();
    let path = super::distinct_points(&pts.ok_or_else(|| bad(CMD, "`points` must be [x, y] points"))?);
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => next_name(s, "G", |e| rail_of(e).is_some())?,
    };
    let mut r = RailParams::new(&name, path, HvacUnits::of(s.doc()?));
    r.height_mm = number(CMD, p, "height")?.unwrap_or(r.height_mm);
    r.post_spacing_mm = number(CMD, p, "postSpacing")?.unwrap_or(r.post_spacing_mm);
    r.elevation_mm = number(CMD, p, "elevation")?.map_or(0.0, |m| m * 1000.0);
    r.label_height_mm = number(CMD, p, "labelHeight")?.unwrap_or(r.label_height_mm);
    let (h, posts, length) = add_rail(s, r)?;
    Ok(json!({ "handle": h.hex(), "posts": posts, "length": length }))
}

fn run_designedit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "designedit";
    let hs = super::arch::handles_param(CMD, p)?;
    if hs.is_empty() || hs.len() > MAX_OBJECTS {
        return Err(bad(CMD, format!("select 1 to {MAX_OBJECTS} ceiling grids, stairs or railings")));
    }
    let tile = tile_param(CMD, p)?;
    let angle = number(CMD, p, "angle")?;
    let elevation = number(CMD, p, "elevation")?.map(|m| m * 1000.0);
    let (rise, riser_max, going, width) = (number(CMD, p, "rise")?, number(CMD, p, "riserMax")?, number(CMD, p, "going")?, number(CMD, p, "width")?);
    let (height, post_spacing) = (number(CMD, p, "height")?, number(CMD, p, "postSpacing")?);
    let (up, show) = (flag(CMD, p, "up")?, flag(CMD, p, "showLabel")?);
    let name = str_param(p, "name").map(|n| n.trim().to_string());
    enum Next {
        Ceiling(CeilingParams),
        Stair(StairParams),
        Rail(RailParams),
    }
    let mut next: Vec<(Handle, Next)> = Vec::with_capacity(hs.len());
    for h in &hs {
        let e = s.doc()?.entity(*h).ok_or_else(|| other("Cannot find that object."))?;
        let wrong = |what: &str| bad(CMD, format!("`{what}` does not apply to this object"));
        if let Some(mut c) = ceiling_of(e) {
            if rise.is_some()
                || riser_max.is_some()
                || going.is_some()
                || width.is_some()
                || up.is_some()
                || height.is_some()
                || post_spacing.is_some()
            {
                return Err(wrong("rise, riserMax, going, width, up, height, postSpacing"));
            }
            if let Some((x, y)) = tile {
                (c.tile_x_mm, c.tile_y_mm) = (x, y);
            }
            c.angle_deg = angle.unwrap_or(c.angle_deg);
            c.elevation_mm = elevation.unwrap_or(c.elevation_mm);
            c.show_label = show.unwrap_or(c.show_label);
            if let Some(n) = &name {
                c.name = n.clone();
            }
            c.validate().map_err(doc_err)?;
            next.push((*h, Next::Ceiling(c)));
        } else if let Some(mut st) = stair_of(e) {
            if tile.is_some() || angle.is_some() || height.is_some() || post_spacing.is_some() {
                return Err(wrong("tile, angle, height, postSpacing"));
            }
            st.rise_mm = rise.unwrap_or(st.rise_mm);
            st.riser_max_mm = riser_max.unwrap_or(st.riser_max_mm);
            st.going_mm = going.unwrap_or(st.going_mm);
            st.width_mm = width.unwrap_or(st.width_mm);
            st.up = up.unwrap_or(st.up);
            st.elevation_mm = elevation.unwrap_or(st.elevation_mm);
            st.show_label = show.unwrap_or(st.show_label);
            if let Some(n) = &name {
                st.name = n.clone();
            }
            st.validate().map_err(doc_err)?;
            next.push((*h, Next::Stair(st)));
        } else if let Some(mut r) = rail_of(e) {
            if tile.is_some() || angle.is_some() || rise.is_some() || riser_max.is_some() || going.is_some() || width.is_some() || up.is_some() {
                return Err(wrong("tile, angle, rise, riserMax, going, width, up"));
            }
            r.height_mm = height.unwrap_or(r.height_mm);
            r.post_spacing_mm = post_spacing.unwrap_or(r.post_spacing_mm);
            r.elevation_mm = elevation.unwrap_or(r.elevation_mm);
            r.show_label = show.unwrap_or(r.show_label);
            if let Some(n) = &name {
                r.name = n.clone();
            }
            r.validate().map_err(doc_err)?;
            next.push((*h, Next::Rail(r)));
        } else {
            return Err(other("Select ceiling grids, stairs or railings only."));
        }
    }
    let d = s.doc_mut()?;
    for (h, n) in &next {
        match n {
            Next::Ceiling(c) => d.regenerate_ceiling(*h, c),
            Next::Stair(st) => d.regenerate_stair(*h, st),
            Next::Rail(r) => d.regenerate_rail(*h, r),
        }
        .map_err(doc_err)?;
    }
    Ok(json!({ "changed": next.len() }))
}

fn parse_number(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

// ---------------------------------------------------------------------------------------------------------------
// Interactive forms
// ---------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum CeilingAsk {
    Point,
    Object,
    Room,
    Walls,
    Tile,
    Elevation,
}

struct CeilingAddM {
    last: DesignLastUsed,
    pts: Vec<Vec2>,
    ask: CeilingAsk,
}

impl CeilingAddM {
    fn new(s: &Session) -> Self {
        CeilingAddM { last: s.last_used.design.clone(), pts: Vec::new(), ask: CeilingAsk::Point }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!(
            "Current settings: Tile = {:.0} x {:.0} mm, Underside = {:.2} m",
            self.last.tile_mm.0,
            self.last.tile_mm.1,
            self.last.ceiling_mm / 1000.0
        ));
    }
    fn finish(&mut self, s: &mut Session, outline: Vec<Vec2>) -> Result<Step> {
        let name = next_name(s, "FP", |e| ceiling_of(e).is_some())?;
        let mut c = CeilingParams::new(&name, outline, HvacUnits::of(s.doc()?));
        (c.tile_x_mm, c.tile_y_mm, c.elevation_mm) = (self.last.tile_mm.0, self.last.tile_mm.1, self.last.ceiling_mm);
        if let Err(e) = c.validate() {
            s.echo(e.to_string());
            return Ok(Step::Continue);
        }
        s.last_used.design = self.last.clone();
        let (_, t) = add_ceiling(s, c)?;
        s.echo(format!("Ceiling {name}: {} whole tiles, {} cut, {:.2} m²", t.whole, t.cut, t.area_m2));
        Ok(Step::Done)
    }
}

impl Interactive for CeilingAddM {
    fn name(&self) -> &'static str {
        "CEILINGADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            CeilingAsk::Object => return Prompt::new("Select a closed polyline", Accept::SELECT),
            CeilingAsk::Room => return Prompt::new("Select a room (a space)", Accept::SELECT),
            CeilingAsk::Walls => return Prompt::new("Click inside a room closed by walls", Accept::POINT),
            CeilingAsk::Tile => {
                return Prompt::new("Enter the tile (mm, e.g. 600x600)", Accept::TEXT)
                    .default(format!("{:.0}x{:.0}", self.last.tile_mm.0, self.last.tile_mm.1));
            }
            CeilingAsk::Elevation => {
                return Prompt::new("Enter the height of the underside (m)", Accept::NUMBER).default(format!("{:.2}", self.last.ceiling_mm / 1000.0));
            }
            CeilingAsk::Point => {}
        }
        let last = self.pts.last().copied();
        match self.pts.len() {
            0 => Prompt::new("Specify first corner of the ceiling", Accept::POINT).kw(&["Room", "Walls", "OBject", "Tile", "Elevation"]),
            1 | 2 => Prompt::new("Specify next corner", Accept::POINT).kw(&["Undo"]).base_opt(last),
            _ => Prompt::new("Specify next corner", Accept::POINT).kw(&["Close", "Undo"]).base_opt(last),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        match (self.ask, &i) {
            (CeilingAsk::Object, Input::Pick(hs)) => {
                let Some(&h) = hs.first() else { return Ok(Step::Continue) };
                return match super::hvac::polyline_outline(s.doc()?, h) {
                    Ok(o) => self.finish(s, o),
                    Err(e) => {
                        s.echo(e.to_string());
                        Ok(Step::Continue)
                    }
                };
            }
            (CeilingAsk::Room, Input::Pick(hs)) => {
                let Some(o) = hs.iter().find_map(|h| s.doc().ok()?.space_outline_world(*h)) else {
                    s.echo("Select a room of the HVAC tab (a space).");
                    return Ok(Step::Continue);
                };
                return self.finish(s, o);
            }
            (CeilingAsk::Walls, Input::Point(q)) => {
                return match super::slab::room_outline_at(s, *q) {
                    Ok(o) => self.finish(s, o),
                    Err(e) => {
                        s.echo(e.to_string());
                        Ok(Step::Continue)
                    }
                };
            }
            (CeilingAsk::Tile, _) => {
                match text.as_deref().and_then(parse_tile) {
                    Some(t) => self.last.tile_mm = t,
                    None => s.echo("Requires a tile like 600x600 or 600x1200 (mm)."),
                }
                self.ask = CeilingAsk::Point;
                self.settings(s);
                return Ok(Step::Continue);
            }
            (CeilingAsk::Elevation, _) => {
                match text.as_deref().and_then(parse_number) {
                    Some(v) => self.last.ceiling_mm = v * 1000.0,
                    None => s.echo("Requires a height in metres."),
                }
                self.ask = CeilingAsk::Point;
                self.settings(s);
                return Ok(Step::Continue);
            }
            (CeilingAsk::Object | CeilingAsk::Room | CeilingAsk::Walls, Input::Enter | Input::Cancel) => {
                self.ask = CeilingAsk::Point;
                return Ok(Step::Continue);
            }
            (CeilingAsk::Object | CeilingAsk::Room | CeilingAsk::Walls, _) => return Ok(Step::Continue),
            _ => {}
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "Room" => self.ask = CeilingAsk::Room,
                    "Walls" => self.ask = CeilingAsk::Walls,
                    "OBject" => self.ask = CeilingAsk::Object,
                    "Tile" => self.ask = CeilingAsk::Tile,
                    "Elevation" => self.ask = CeilingAsk::Elevation,
                    "Undo" => {
                        self.pts.pop();
                    }
                    "Close" if self.pts.len() >= 3 => return self.finish(s, self.pts.clone()),
                    _ => {}
                }
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                if self.pts.len() < MAX_POINTS && !self.pts.last().is_some_and(|l| l.near(p, 1e-9)) {
                    self.pts.push(p);
                }
                Ok(Step::Continue)
            }
            Input::Enter if self.pts.len() >= 3 => self.finish(s, self.pts.clone()),
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        if self.pts.is_empty() || self.ask != CeilingAsk::Point {
            return Vec::new();
        }
        let mut pts = self.pts.clone();
        pts.push(c);
        pts.push(self.pts[0]);
        pts.windows(2).map(|w| cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: w[0].to3(0.0), b: w[1].to3(0.0) })).collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StairAsk {
    Start,
    Direction,
    Rise,
    Width,
    Going,
}

struct StairAddM {
    last: DesignLastUsed,
    start: Option<Vec2>,
    ask: StairAsk,
}

impl StairAddM {
    fn new(s: &Session) -> Self {
        StairAddM { last: s.last_used.design.clone(), start: None, ask: StairAsk::Start }
    }
    fn settings(&self, s: &mut Session) {
        s.echo(format!(
            "Current settings: Rise = {:.0} mm, Riser = {:.0} mm at most, Tread = {:.0} mm, Width = {:.0} mm",
            self.last.rise_mm, self.last.riser_max_mm, self.last.going_mm, self.last.width_mm
        ));
    }
}

impl Interactive for StairAddM {
    fn name(&self) -> &'static str {
        "STAIRADD"
    }
    fn begin(&mut self, s: &mut Session) -> Result<Step> {
        self.settings(s);
        Ok(Step::Continue)
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        match self.ask {
            StairAsk::Rise => Prompt::new("Enter the height to climb (mm)", Accept::NUMBER).default(format!("{:.0}", self.last.rise_mm)),
            StairAsk::Width => Prompt::new("Enter the width of the flight (mm)", Accept::NUMBER).default(format!("{:.0}", self.last.width_mm)),
            StairAsk::Going => Prompt::new("Enter the depth of a tread (mm)", Accept::NUMBER).default(format!("{:.0}", self.last.going_mm)),
            StairAsk::Direction => Prompt::new("Specify a point along the way up", Accept::POINT).base_opt(self.start),
            StairAsk::Start => Prompt::new("Specify the middle of the first riser", Accept::POINT).kw(&["Rise", "Width", "Going"]),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        let text = match &i {
            Input::Text(t) | Input::Keyword(t) => Some(t.trim().to_string()),
            _ => None,
        };
        if matches!(self.ask, StairAsk::Rise | StairAsk::Width | StairAsk::Going) {
            match (text.as_deref().and_then(parse_number), self.ask) {
                (Some(v), StairAsk::Rise) => self.last.rise_mm = v,
                (Some(v), StairAsk::Width) => self.last.width_mm = v,
                (Some(v), StairAsk::Going) => self.last.going_mm = v,
                _ => s.echo("Requires a number of millimetres."),
            }
            self.ask = StairAsk::Start;
            self.settings(s);
            return Ok(Step::Continue);
        }
        match (self.ask, i) {
            (StairAsk::Start, Input::Keyword(k)) => {
                match k.as_str() {
                    "Rise" => self.ask = StairAsk::Rise,
                    "Width" => self.ask = StairAsk::Width,
                    "Going" => self.ask = StairAsk::Going,
                    _ => {}
                }
                Ok(Step::Continue)
            }
            (StairAsk::Start, Input::Point(p)) => {
                self.start = Some(p);
                self.ask = StairAsk::Direction;
                Ok(Step::Continue)
            }
            (StairAsk::Direction, Input::Point(p)) => {
                let Some(start) = self.start else { return Ok(Step::Cancel) };
                if start.near(p, 1e-9) {
                    s.echo("Specify a point away from the first riser.");
                    return Ok(Step::Continue);
                }
                let name = next_name(s, "E", |e| stair_of(e).is_some())?;
                let st = stair_from(s, &self.last, &name)?;
                let (risers, riser, comfortable) = (st.risers(), st.riser_mm(), st.comfortable());
                match add_stair(s, st, start, (p - start).angle()) {
                    Ok(_) => {
                        s.last_used.design = self.last.clone();
                        s.echo(format!(
                            "Stair {name}: {risers} risers of {riser:.0} mm{}",
                            if comfortable { "" } else { " (Blondel: 2 riser + tread is not within 600 to 650 mm)" }
                        ));
                        Ok(Step::Done)
                    }
                    Err(e) => {
                        s.echo(e.to_string());
                        Ok(Step::Continue)
                    }
                }
            }
            (_, Input::Enter | Input::Cancel) => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RailAsk {
    Point,
    Height,
}

struct RailAddM {
    last: DesignLastUsed,
    pts: Vec<Vec2>,
    ask: RailAsk,
}

impl RailAddM {
    fn new(s: &Session) -> Self {
        RailAddM { last: s.last_used.design.clone(), pts: Vec::new(), ask: RailAsk::Point }
    }
    fn finish(&mut self, s: &mut Session) -> Result<Step> {
        let name = next_name(s, "G", |e| rail_of(e).is_some())?;
        let mut r = RailParams::new(&name, self.pts.clone(), HvacUnits::of(s.doc()?));
        r.height_mm = self.last.rail_height_mm;
        match add_rail(s, r) {
            Ok((_, posts, length)) => {
                s.last_used.design = self.last.clone();
                s.echo(format!("Railing {name}: {length:.2} m, {posts} posts"));
                Ok(Step::Done)
            }
            Err(e) => {
                s.echo(e.to_string());
                Ok(Step::Continue)
            }
        }
    }
}

impl Interactive for RailAddM {
    fn name(&self) -> &'static str {
        "RAILADD"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.ask == RailAsk::Height {
            return Prompt::new("Enter the height of the railing (mm)", Accept::NUMBER).default(format!("{:.0}", self.last.rail_height_mm));
        }
        match self.pts.len() {
            0 => Prompt::new("Specify start point of the railing", Accept::POINT).kw(&["Height"]),
            _ => Prompt::new("Specify next point", Accept::POINT).kw(&["Undo"]).base_opt(self.pts.last().copied()),
        }
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.ask == RailAsk::Height {
            match &i {
                Input::Text(t) | Input::Keyword(t) => match parse_number(t).filter(|v| (300.0..=3000.0).contains(v)) {
                    Some(v) => self.last.rail_height_mm = v,
                    None => s.echo("Requires a height of 300 to 3000 mm."),
                },
                _ => {}
            }
            self.ask = RailAsk::Point;
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) => {
                match k.as_str() {
                    "Height" => self.ask = RailAsk::Height,
                    "Undo" => {
                        self.pts.pop();
                    }
                    _ => {}
                }
                Ok(Step::Continue)
            }
            Input::Point(p) => {
                if self.pts.len() < MAX_POINTS && !self.pts.last().is_some_and(|l| l.near(p, 1e-9)) {
                    self.pts.push(p);
                }
                Ok(Step::Continue)
            }
            Input::Enter if self.pts.len() >= 2 => self.finish(s),
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
    fn preview(&self, _s: &Session, c: Vec2) -> Vec<cadcraft_doc::EntityKind> {
        let mut pts = self.pts.clone();
        pts.push(c);
        pts.windows(2).map(|w| cadcraft_doc::EntityKind::Line(cadcraft_doc::Line { a: w[0].to3(0.0), b: w[1].to3(0.0) })).collect()
    }
}
