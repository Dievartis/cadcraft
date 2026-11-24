//! Slabs: a floor, roof or foundation slab drawn as a closed outline, hatched, with its thickness and
//! the elevation of its top written on it. Like the other plan objects it is a block reference to an
//! anonymous block, its data travelling as extended data of its own registered application.

use cadcraft_geom::{PolyVertex, Vec2, point_in_polygon, shoelace};
use serde::{Deserialize, Serialize};

use super::{HvacPiece, HvacUnits, check_label_height, check_name, invalid, label, v3, xdata};
use crate::{EntityKind, Hatch, HatchLoop, LwPolyline, RawTag, Result};

/// The registered application name of a slab's extended data.
pub const SLAB_APP: &str = "CADCRAFT_SLAB";
/// The layers of floor, roof and foundation slabs.
pub const SLAB_FLOOR_LAYER: &str = "A-FLOR-SLAB";
pub const SLAB_ROOF_LAYER: &str = "A-ROOF-SLAB";
pub const SLAB_FOUNDATION_LAYER: &str = "A-FNDN-SLAB";
/// The layer of a slab's hatch.
pub const SLAB_PATTERN_LAYER: &str = "A-SLAB-PATT";
/// Most vertices of a slab's outline.
pub const MAX_SLAB_VERTICES: usize = 256;
/// Most openings (trémies) in one slab, and most vertices of one.
pub const MAX_SLAB_HOLES: usize = 64;
/// Largest slope, per cent.
pub const MAX_SLAB_SLOPE_PCT: f64 = 100.0;
/// Largest thickness and elevation, mm.
pub const MAX_SLAB_MM: f64 = 10_000.0;
const XDATA_FORMAT: &str = "SLAB1";

fn default_label_mm() -> f64 {
    150.0
}

/// About how wide one repeat of a pattern is at scale 1, mm, and the most repeats a slab's hatch may have.
const PATTERN_TILE_MM: f64 = 508.0;
const MAX_PATTERN_TILES: f64 = 400.0;

fn default_hatch_scale() -> f64 {
    1.0
}

/// What a slab is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlabKind {
    #[default]
    Floor,
    Roof,
    Foundation,
}

impl SlabKind {
    pub const ALL: [SlabKind; 3] = [SlabKind::Floor, SlabKind::Roof, SlabKind::Foundation];

    pub fn name(self) -> &'static str {
        match self {
            SlabKind::Floor => "floor",
            SlabKind::Roof => "roof",
            SlabKind::Foundation => "foundation",
        }
    }

    pub fn parse(text: &str) -> Option<SlabKind> {
        let t: String = text.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        match t.as_str() {
            "floor" | "slab" | "plancher" | "dalle" => Some(SlabKind::Floor),
            "roof" | "toit" | "toiture" | "toitureterrasse" => Some(SlabKind::Roof),
            "foundation" | "fondation" | "radier" => Some(SlabKind::Foundation),
            _ => None,
        }
    }

    pub fn layer(self) -> &'static str {
        match self {
            SlabKind::Floor => SLAB_FLOOR_LAYER,
            SlabKind::Roof => SLAB_ROOF_LAYER,
            SlabKind::Foundation => SLAB_FOUNDATION_LAYER,
        }
    }

    /// The thickness a new slab of the kind starts with, mm.
    pub fn default_thickness_mm(self) -> f64 {
        match self {
            SlabKind::Floor => 200.0,
            SlabKind::Roof => 250.0,
            SlabKind::Foundation => 400.0,
        }
    }
}

/// A slab: what its block reference keeps to regenerate it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlabParams {
    pub name: String,
    #[serde(default)]
    pub kind: SlabKind,
    pub thickness_mm: f64,
    /// The elevation of the slab's top, mm, from the drawing's zero.
    #[serde(default)]
    pub elevation_mm: f64,
    /// The closed outline in the block's coordinates (drawing units), without repeating the first point.
    pub outline: Vec<Vec2>,
    /// The openings (trémies) cut through the slab, in the block's coordinates: closed outlines inside the slab's, without repeating the first point.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Vec2>>,
    /// The slope of the top, per cent (0: flat), and the direction it falls towards, degrees counter-clockwise from the drawing's x axis.
    /// The elevation is that of the highest point.
    #[serde(default, skip_serializing_if = "slope_is_flat")]
    pub slope_pct: f64,
    #[serde(default)]
    pub slope_direction_deg: f64,
    /// The hatch pattern; empty for none.
    #[serde(default = "default_pattern")]
    pub pattern: String,
    /// The pattern's scale in drawing units.
    #[serde(default = "default_hatch_scale")]
    pub hatch_scale: f64,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

fn slope_is_flat(v: &f64) -> bool {
    *v == 0.0
}

fn default_pattern() -> String {
    // Diagonal lines: light to draw. AR-CONC (random stones and dots) is tens of thousands of marks on a room-sized slab.
    "ANSI31".into()
}

impl SlabParams {
    /// A slab of `kind` over `outline`, with the kind's thickness and the concrete hatch.
    pub fn new(name: &str, kind: SlabKind, outline: Vec<Vec2>, units: HvacUnits) -> SlabParams {
        SlabParams {
            name: name.to_string(),
            kind,
            thickness_mm: kind.default_thickness_mm(),
            elevation_mm: 0.0,
            outline,
            holes: Vec::new(),
            slope_pct: 0.0,
            slope_direction_deg: 0.0,
            pattern: default_pattern(),
            hatch_scale: default_hatch_scale(),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("slab", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if !(self.thickness_mm.is_finite() && (10.0..=MAX_SLAB_MM).contains(&self.thickness_mm)) {
            return Err(invalid(format!("A slab's thickness is 10 to {MAX_SLAB_MM} mm.")));
        }
        if !(self.elevation_mm.is_finite() && self.elevation_mm.abs() <= 1.0e6) {
            return Err(invalid("A slab's elevation is not a usable number."));
        }
        if !(self.hatch_scale.is_finite() && self.hatch_scale > 0.0 && self.hatch_scale <= 1.0e6)
            || self.pattern.chars().count() > 32
            || self.pattern.chars().any(char::is_control)
        {
            return Err(invalid("A slab's pattern is not usable."));
        }
        if self.outline.len() < 3 || self.outline.len() > MAX_SLAB_VERTICES {
            return Err(invalid(format!("A slab's outline has 3 to {MAX_SLAB_VERTICES} points.")));
        }
        if self.outline.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12) {
            return Err(invalid("A slab's outline has a point that is not a usable number."));
        }
        if shoelace(&self.outline).abs() <= 1e-12 {
            return Err(invalid("A slab's outline encloses no area."));
        }
        if !(self.slope_pct.is_finite() && (0.0..=MAX_SLAB_SLOPE_PCT).contains(&self.slope_pct) && self.slope_direction_deg.is_finite()) {
            return Err(invalid(format!("A slab's slope is 0 to {MAX_SLAB_SLOPE_PCT} per cent.")));
        }
        if self.holes.len() > MAX_SLAB_HOLES {
            return Err(invalid(format!("A slab has at most {MAX_SLAB_HOLES} openings.")));
        }
        for hole in &self.holes {
            if hole.len() < 3 || hole.len() > MAX_SLAB_VERTICES || hole.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12) {
                return Err(invalid(format!("An opening in a slab has 3 to {MAX_SLAB_VERTICES} points that are usable numbers.")));
            }
            if shoelace(hole).abs() <= 1e-12 {
                return Err(invalid("An opening in a slab encloses no area."));
            }
            if !hole.iter().all(|p| point_in_polygon(&self.outline, *p)) {
                return Err(invalid("An opening in a slab is inside its outline."));
            }
        }
        Ok(())
    }

    /// The area, m², without the openings.
    /// The scale the pattern is drawn at: the slab's, made coarser when a big slab would hold more than [`MAX_PATTERN_TILES`] repeats of the pattern
    /// (about 508 mm across at scale 1), which a web build cannot draw or hold.
    pub fn pattern_scale(&self) -> f64 {
        let k = self.units.mm_per_unit;
        let area_mm2 = shoelace(&self.outline).abs() * k * k;
        let tile_mm = PATTERN_TILE_MM * self.hatch_scale * k;
        let tiles = if tile_mm > 0.0 { area_mm2 / (tile_mm * tile_mm) } else { 0.0 };
        if tiles.is_finite() && tiles > MAX_PATTERN_TILES { self.hatch_scale * (tiles / MAX_PATTERN_TILES).sqrt() } else { self.hatch_scale }
    }

    pub fn area_m2(&self) -> f64 {
        let holes: f64 = self.holes.iter().map(|h| shoelace(h).abs()).sum();
        self.units.area_m2((shoelace(&self.outline).abs() - holes).max(0.0))
    }

    /// The area of the openings, m².
    pub fn holes_area_m2(&self) -> f64 {
        self.units.area_m2(self.holes.iter().map(|h| shoelace(h).abs()).sum())
    }

    /// The volume, m³.
    pub fn volume_m3(&self) -> f64 {
        self.area_m2() * self.thickness_mm / 1000.0
    }

    /// The text written on the slab: its name, thickness and the elevation of its top.
    pub fn label_text(&self) -> String {
        let slope = if self.slope_pct > 0.0 { format!(" {:.1} %", self.slope_pct) } else { String::new() };
        format!("{} {:.0} mm {:+.2} m{slope}", self.name, self.thickness_mm, self.elevation_mm / 1000.0)
    }

    /// The outline, its hatch and the label at the outline's first point's centroid.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let vertices: Vec<PolyVertex> = self.outline.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        let outline = LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false };
        let mut pieces = vec![HvacPiece { layer: self.kind.layer(), kind: EntityKind::LwPolyline(outline) }];
        let pattern = self.pattern.trim();
        if !pattern.is_empty() {
            let solid = pattern.eq_ignore_ascii_case("SOLID");
            let mut loops =
                vec![HatchLoop { vertices: self.outline.iter().copied().map(PolyVertex::new).collect(), outer: true, sources: Vec::new() }];
            // The openings are islands: the hatch goes round them.
            loops.extend(self.holes.iter().map(|h| HatchLoop {
                vertices: h.iter().copied().map(PolyVertex::new).collect(),
                outer: false,
                sources: Vec::new(),
            }));
            let hatch = Hatch {
                pattern: pattern.to_ascii_uppercase(),
                solid,
                loops,
                scale: if solid { 1.0 } else { self.pattern_scale() },
                angle: 0.0,
                associative: false,
                style: 0,
                elevation: 0.0,
                gradient: None,
                origin: Vec2::ZERO,
                background: None,
                pattern_lines: Vec::new(),
            };
            pieces.push(HvacPiece { layer: SLAB_PATTERN_LAYER, kind: EntityKind::Hatch(hatch) });
        }
        // The openings, with a cross through each, as they are drawn on a plan.
        for hole in &self.holes {
            let vertices: Vec<PolyVertex> = hole.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
            pieces.push(HvacPiece {
                layer: self.kind.layer(),
                kind: EntityKind::LwPolyline(LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false }),
            });
            if let [a, b, c, d] = hole[..] {
                for (p, q) in [(a, c), (b, d)] {
                    pieces.push(HvacPiece { layer: self.kind.layer(), kind: line_between(p, q) });
                }
            }
        }
        let c = cadcraft_geom::polygon_centroid(&self.outline).unwrap_or_else(|| self.outline.first().copied().unwrap_or(Vec2::ZERO));
        // The slope: an arrow along the direction it falls towards, from the centre.
        if self.slope_pct > 0.0 {
            let (min, max) = self.outline.iter().fold((Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN)), |(lo, hi), p| {
                (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y)))
            });
            let len = ((max.x - min.x).min(max.y - min.y) / 4.0).max(self.units.to_units(300.0));
            let dir = Vec2::new(self.slope_direction_deg.to_radians().cos(), self.slope_direction_deg.to_radians().sin());
            let tip = c + dir * len;
            let wing = self.units.to_units(150.0).min(len / 3.0);
            for end in [tip - dir * wing + dir.perp() * (wing / 2.0), tip - dir * wing - dir.perp() * (wing / 2.0)] {
                pieces.push(HvacPiece { layer: self.kind.layer(), kind: line_between(tip, end) });
            }
            pieces.push(HvacPiece { layer: self.kind.layer(), kind: line_between(c, tip) });
        }
        pieces.push(HvacPiece { layer: self.kind.layer(), kind: label(c, self.units.to_units(self.label_height_mm), self.label_text()) });
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(SLAB_APP, XDATA_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<SlabParams> {
        let p: SlabParams = xdata::from_xdata(SLAB_APP, XDATA_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The slab an object is, when it is a block reference carrying usable slab data.
pub fn slab_of(e: &crate::Entity) -> Option<SlabParams> {
    match &e.kind {
        EntityKind::Insert(_) => SlabParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

fn line_between(a: Vec2, b: Vec2) -> EntityKind {
    EntityKind::Line(crate::Line { a: v3(a), b: v3(b) })
}

/// The colour of a slab layer: a mid grey for a floor, brown for a roof, dark for a foundation.
pub(super) fn slab_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        SLAB_FLOOR_LAYER => Some(Color::True(Rgb(0xa0, 0xa8, 0xb4))),
        SLAB_ROOF_LAYER => Some(Color::True(Rgb(0xc0, 0x7a, 0x4a))),
        SLAB_FOUNDATION_LAYER => Some(Color::True(Rgb(0x7a, 0x70, 0x68))),
        SLAB_PATTERN_LAYER => Some(Color::True(Rgb(0x6e, 0x76, 0x80))),
        _ => None,
    }
}

/// A point of a block reference's block in the drawing: scaled, turned and moved by the reference.
pub(super) fn block_to_world(i: &crate::Insert, p: Vec2) -> Vec2 {
    let (c, s) = (i.rotation.cos(), i.rotation.sin());
    let (x, y) = (p.x * i.scale.x, p.y * i.scale.y);
    Vec2::new(i.insert.x + x * c - y * s, i.insert.y + x * s + y * c)
}

/// A point of the drawing in a block reference's block: the inverse of [`block_to_world`]; `None` for a reference scaled to nothing.
pub(super) fn world_to_block(i: &crate::Insert, p: Vec2) -> Option<Vec2> {
    if i.scale.x.abs() < 1e-12 || i.scale.y.abs() < 1e-12 {
        return None;
    }
    let (c, s) = (i.rotation.cos(), i.rotation.sin());
    let (dx, dy) = (p.x - i.insert.x, p.y - i.insert.y);
    Some(Vec2::new((dx * c + dy * s) / i.scale.x, (-dx * s + dy * c) / i.scale.y))
}

impl crate::Drawing {
    /// The shaft outlines of the risers of `space` (their trémies), in the drawing's coordinates.
    pub fn riser_shafts(&self, space: &crate::Space) -> Vec<Vec<Vec2>> {
        let Some(store) = self.space(space) else { return Vec::new() };
        store
            .iter()
            .filter_map(|e| match (&e.kind, super::any_riser_of(e)) {
                (EntityKind::Insert(i), Some(r)) => Some(r.outline.iter().map(|p| block_to_world(i, *p)).collect()),
                _ => None,
            })
            .collect()
    }

    /// Slab `h` with an opening more, given in the drawing's coordinates (3 or more points); the slab is redrawn.
    pub fn add_slab_hole(&mut self, h: crate::Handle, outline: &[Vec2]) -> Result<()> {
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let (EntityKind::Insert(i), Some(mut p)) = (&e.kind, slab_of(e)) else { return Err(invalid("The object is not a slab.")) };
        let local: Option<Vec<Vec2>> = outline.iter().map(|q| world_to_block(i, *q)).collect();
        p.holes.push(local.ok_or_else(|| invalid("The slab is scaled to nothing."))?);
        p.validate()?;
        self.regenerate_slab(h, &p)
    }

    /// The openings of slab `h` in the drawing's coordinates.
    pub fn slab_holes_world(&self, h: crate::Handle) -> Vec<Vec<Vec2>> {
        let Some(e) = self.entity(h) else { return Vec::new() };
        match (&e.kind, slab_of(e)) {
            (EntityKind::Insert(i), Some(p)) => p.holes.iter().map(|hole| hole.iter().map(|q| block_to_world(i, *q)).collect()).collect(),
            _ => Vec::new(),
        }
    }

    /// Whether every vertex of `outline` (drawing coordinates) is inside slab `h`'s outline.
    pub fn slab_contains(&self, h: crate::Handle, outline: &[Vec2]) -> bool {
        let Some(e) = self.entity(h) else { return false };
        let (EntityKind::Insert(i), Some(p)) = (&e.kind, slab_of(e)) else { return false };
        outline.iter().all(|q| world_to_block(i, *q).is_some_and(|l| point_in_polygon(&p.outline, l)))
    }

    /// Adds slab `p` to `space`, its outline given in drawing coordinates.
    pub fn add_slab(&mut self, space: &crate::Space, mut common: crate::Common, mut p: SlabParams) -> Result<crate::Handle> {
        let origin = p.outline.first().copied().ok_or_else(|| invalid("A slab needs an outline."))?;
        for v in p.outline.iter_mut().chain(p.holes.iter_mut().flatten()) {
            *v -= origin;
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(SLAB_APP, &common.xdata, p.to_xdata()?);
        common.layer = p.kind.layer().into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(p.kind.layer());
        let insert = crate::Insert { block, insert: v3(origin), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives slab `h` the parameters `p` (outline in the block's coordinates), redrawing it in place.
    pub fn regenerate_slab(&mut self, h: crate::Handle, p: &SlabParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(SLAB_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        let layer = p.kind.layer();
        self.ensure_layer(layer);
        self.modify_entity(h, |e| e.common.layer = layer.into())?;
        Ok(())
    }
}
