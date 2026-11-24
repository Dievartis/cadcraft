//! A building section: the drawing cut along a line, seen straight on, as a picture in model space beside the plan. The walls the line crosses are
//! drawn with their openings (sill and head), the slabs with their thickness and openings, the steel columns and beams, and the false ceilings, each
//! at its elevation, with the levels in the margin. The air terminals, the ducts, the air handling units, the equipment and the pipes the line crosses are
//! drawn too, each at the altitude of its underside (the terminals at 2.50 m and the ducts at 2.50 m where none was given). It is one block reference, so it moves, copies and explodes like any other object, and it is a
//! picture made now: it does not follow later changes to the plan. Roofs are drawn as the line of their surface (with the overhang, the ridge and the slope) and the thickness of 200 mm; slopes of slabs are not drawn.

use cadcraft_geom::{PolyVertex, Vec2};

use super::{WallParams, joint, wall_of};
use crate::ventilation::{HvacPiece, HvacUnits, ahu_of, air_terminal_of, ceiling_of, duct_of, equipment_of, pipe_of, roof_of, slab_of, struct_of};
use crate::{
    Common, DocError, Drawing, EntityKind, HAlign, Handle, Hatch, HatchLoop, Insert, Line, LwPolyline, OpeningKind, Result, Space, Text, VAlign,
    opening_of,
};

/// The layers of a section: what is cut, the levels and notes, and the cut line in plan.
pub const SECTION_LAYER: &str = "A-SECT";
pub const SECTION_DIM_LAYER: &str = "A-SECT-DIMS";
pub const SECTION_LINE_LAYER: &str = "A-SECT-LINE";

/// Most objects one section looks at, and most pieces it draws.
const MAX_SECTION_OBJECTS: usize = 200_000;
const MAX_SECTION_PIECES: usize = 50_000;

/// What a section cut through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionCount {
    /// The cuts through walls (a wall the line crosses twice is cut twice).
    pub walls: usize,
    pub openings: usize,
    pub slabs: usize,
    pub columns: usize,
    pub beams: usize,
    pub ceilings: usize,
    /// The roofs the line crosses.
    pub roofs: usize,
    /// The ducts, the air terminals, the units and equipment, and the pipes the line goes through (a duct it crosses twice counts twice).
    pub ducts: usize,
    pub terminals: usize,
    pub equipment: usize,
    pub pipes: usize,
}

/// The cut line from `a`, `len` long, and what is measured along and across it.
struct Cut {
    a: Vec2,
    d: Vec2,
    n: Vec2,
    len: f64,
}

impl Cut {
    fn new(a: Vec2, b: Vec2) -> Option<Cut> {
        let len = a.dist(b);
        if !(a.is_finite() && b.is_finite() && len > 1e-9) {
            return None;
        }
        let d = (b - a) * (1.0 / len);
        Some(Cut { a, d, n: Vec2::new(-d.y, d.x), len })
    }

    fn station(&self, p: Vec2) -> f64 {
        (p - self.a).dot(self.d)
    }

    fn offset(&self, p: Vec2) -> f64 {
        (p - self.a).dot(self.n)
    }

    /// The stretches of the cut line that lie inside `polygon`, as (from, to) distances along it.
    fn inside(&self, polygon: &[Vec2]) -> Vec<(f64, f64)> {
        let mut crossings = Vec::new();
        for (p, q) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
            let (op, oq) = (self.offset(*p), self.offset(*q));
            if (op <= 0.0) != (oq <= 0.0) {
                let t = op / (op - oq);
                crossings.push(self.station(*p) + t * (self.station(*q) - self.station(*p)));
            }
        }
        crossings.sort_by(f64::total_cmp);
        crossings
            .as_chunks::<2>()
            .0
            .iter()
            .filter_map(|[a, b]| {
                let (from, to) = (a.max(0.0), b.min(self.len));
                (to - from > 1e-9).then_some((from, to))
            })
            .collect()
    }
}

/// The stretches of `a` that `b` does not cover.
fn subtract(a: Vec<(f64, f64)>, b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = a;
    for &(b0, b1) in b {
        out = out
            .into_iter()
            .flat_map(|(from, to)| {
                if b1 <= from || b0 >= to {
                    vec![(from, to)]
                } else {
                    let mut parts = Vec::new();
                    if b0 > from + 1e-9 {
                        parts.push((from, b0));
                    }
                    if b1 < to - 1e-9 {
                        parts.push((b1, to));
                    }
                    parts
                }
            })
            .collect();
    }
    out
}

fn v3(p: Vec2) -> cadcraft_geom::Vec3 {
    p.to3(0.0)
}

/// A point of a block reference's block in the drawing.
fn to_world(i: &Insert, p: Vec2) -> Vec2 {
    let (c, s) = (i.rotation.cos(), i.rotation.sin());
    let (x, y) = (p.x * i.scale.x, p.y * i.scale.y);
    Vec2::new(i.insert.x + x * c - y * s, i.insert.y + x * s + y * c)
}

fn solid_hatch(pts: &[Vec2]) -> Hatch {
    Hatch {
        pattern: "SOLID".into(),
        solid: true,
        loops: vec![HatchLoop { vertices: pts.iter().map(|&p| PolyVertex::new(p)).collect(), outer: true, sources: Vec::new() }],
        scale: 1.0,
        angle: 0.0,
        associative: false,
        style: 0,
        elevation: 0.0,
        gradient: None,
        origin: Vec2::ZERO,
        background: None,
        pattern_lines: Vec::new(),
    }
}

/// What is drawn, in the block's coordinates: the cut line's length along x, the elevations (drawing units) up y.
struct Picture {
    units: HvacUnits,
    pieces: Vec<HvacPiece>,
    count: SectionCount,
    /// The tops of the slabs (mm), for the levels in the margin.
    levels: Vec<f64>,
    full: bool,
}

impl Picture {
    fn u(&self, mm: f64) -> f64 {
        self.units.to_units(mm)
    }

    fn push(&mut self, piece: HvacPiece) {
        if self.pieces.len() >= MAX_SECTION_PIECES {
            self.full = true;
        } else {
            self.pieces.push(piece);
        }
    }

    fn line(&mut self, layer: &'static str, a: Vec2, b: Vec2) {
        self.push(HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(a), b: v3(b) }) });
    }

    /// A rectangle from station `x0` to `x1` and from elevation `y0` to `y1` (mm), outlined, and filled when `fill`.
    fn rect(&mut self, x0: f64, x1: f64, y0_mm: f64, y1_mm: f64, fill: bool) {
        if !(x1 - x0 > 1e-9 && y1_mm - y0_mm > 1e-9) {
            return;
        }
        let (y0, y1) = (self.u(y0_mm), self.u(y1_mm));
        let corners = [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)];
        self.push(HvacPiece {
            layer: SECTION_LAYER,
            kind: EntityKind::LwPolyline(LwPolyline {
                vertices: corners.iter().map(|p| PolyVertex::new(*p)).collect(),
                closed: true,
                const_width: 0.0,
                elevation: 0.0,
                plinegen: false,
            }),
        });
        if fill {
            self.push(HvacPiece { layer: SECTION_LAYER, kind: EntityKind::Hatch(solid_hatch(&corners)) });
        }
    }

    /// A rectangle in the layer of an object of the HVAC family, from station `x0` to `x1` and elevation `y0` to `y1` (mm), outlined only.
    fn rect_on(&mut self, layer: &'static str, x0: f64, x1: f64, y0_mm: f64, y1_mm: f64) {
        if !(x1 - x0 > 1e-9 && y1_mm - y0_mm > 1e-9) {
            return;
        }
        let (y0, y1) = (self.u(y0_mm), self.u(y1_mm));
        let corners = [Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)];
        self.push(HvacPiece {
            layer,
            kind: EntityKind::LwPolyline(LwPolyline {
                vertices: corners.iter().map(|p| PolyVertex::new(*p)).collect(),
                closed: true,
                const_width: 0.0,
                elevation: 0.0,
                plinegen: false,
            }),
        });
    }

    /// A circle at station `x` and elevation `y_mm`, `d_mm` across, in the layer of an object of the HVAC family.
    fn circle_on(&mut self, layer: &'static str, x: f64, y_mm: f64, d_mm: f64) {
        let r = self.u(d_mm) / 2.0;
        if r > 1e-9 {
            self.push(HvacPiece { layer, kind: EntityKind::Circle(crate::Circle { center: v3(Vec2::new(x, self.u(y_mm))), radius: r }) });
        }
    }

    fn text(&mut self, at: Vec2, height_mm: f64, value: String, halign: HAlign) {
        let t = Text {
            insert: v3(at),
            align_pt: Some(v3(at)),
            height: self.u(height_mm),
            value,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign,
            valign: VAlign::Middle,
        };
        self.push(HvacPiece { layer: SECTION_DIM_LAYER, kind: EntityKind::Text(t) });
    }
}

impl Drawing {
    /// The picture of the section along `a` to `b` of what `space` holds: its pieces and what it cut through. Refuses a cut line with no length.
    fn section_picture(&self, space: &Space, a: Vec2, b: Vec2) -> Result<(f64, Picture)> {
        let cut = Cut::new(a, b).ok_or_else(|| DocError::Invalid("A section needs a cut line with a length.".into()))?;
        let mut pic = Picture { units: HvacUnits::of(self), pieces: Vec::new(), count: SectionCount::default(), levels: vec![0.0], full: false };
        let Some(store) = self.space(space) else { return Ok((cut.len, pic)) };
        let entities: Vec<&crate::Entity> = store.iter().map(|e| &**e).filter(|e| self.is_visible(e)).take(MAX_SECTION_OBJECTS).collect();
        // The openings, with where they stand, for the walls to look at.
        let openings: Vec<_> = entities
            .iter()
            .filter_map(|e| match (&e.kind, opening_of(e)) {
                (EntityKind::Insert(i), Some(op)) => Some((Vec2::new(i.insert.x, i.insert.y), i.rotation, op)),
                _ => None,
            })
            .collect();
        for e in &entities {
            let EntityKind::Insert(i) = &e.kind else { continue };
            if let Some(w) = wall_of(e) {
                section_wall(&mut pic, &cut, i, &w, &openings);
            } else if let Some(p) = slab_of(e) {
                let outline: Vec<Vec2> = p.outline.iter().map(|q| to_world(i, *q)).collect();
                let mut holes = Vec::new();
                for hole in &p.holes {
                    holes.extend(cut.inside(&hole.iter().map(|q| to_world(i, *q)).collect::<Vec<_>>()));
                }
                let stretches = subtract(cut.inside(&outline), &holes);
                if !stretches.is_empty() {
                    pic.count.slabs += 1;
                    pic.levels.push(p.elevation_mm);
                }
                for (x0, x1) in stretches {
                    pic.rect(x0, x1, p.elevation_mm - p.thickness_mm, p.elevation_mm, true);
                }
            } else if let Some(s) = struct_of(e) {
                let Some(sec) = s.profile() else { continue };
                let u = |mm: f64| s.units.to_units(mm);
                let local: Vec<Vec2> = match s.kind {
                    // A column is drawn over its whole width, as seen, not just where the line goes through the web of its profile.
                    crate::ventilation::StructKind::Column => {
                        let (hx, hy) = (u(sec.b_mm / 2.0), u(sec.h_mm / 2.0));
                        vec![Vec2::new(-hx, -hy), Vec2::new(hx, -hy), Vec2::new(hx, hy), Vec2::new(-hx, hy)]
                    }
                    crate::ventilation::StructKind::Beam => {
                        let (x0, x1, half) = (u(s.trim_start_mm), s.length - u(s.trim_end_mm), u(sec.b_mm / 2.0));
                        vec![Vec2::new(x0, -half), Vec2::new(x1, -half), Vec2::new(x1, half), Vec2::new(x0, half)]
                    }
                };
                let world: Vec<Vec2> = local.iter().map(|q| to_world(i, *q)).collect();
                for (x0, x1) in cut.inside(&world) {
                    match s.kind {
                        crate::ventilation::StructKind::Column => {
                            pic.count.columns += 1;
                            pic.rect(x0, x1, s.elevation_mm, s.elevation_mm + s.height_mm, true);
                        }
                        crate::ventilation::StructKind::Beam => {
                            pic.count.beams += 1;
                            pic.rect(x0, x1, s.elevation_mm, s.elevation_mm + sec.h_mm, true);
                        }
                    }
                }
            } else if let Some(r) = roof_of(e) {
                section_roof(&mut pic, &cut, i, &r);
            } else if let Some(c) = ceiling_of(e) {
                let outline: Vec<Vec2> = c.outline.iter().map(|q| to_world(i, *q)).collect();
                let stretches = cut.inside(&outline);
                if !stretches.is_empty() {
                    pic.count.ceilings += 1;
                }
                for (x0, x1) in stretches {
                    pic.rect(x0, x1, c.elevation_mm - 20.0, c.elevation_mm, false);
                }
            }
        }
        self.section_hvac(&mut pic, &cut, &entities);
        // The ground line and the levels in the margin.
        pic.levels.sort_by(f64::total_cmp);
        pic.levels.dedup_by(|p, q| (*p - *q).abs() < 1.0);
        let levels = pic.levels.clone();
        let margin = pic.u(600.0);
        for mm in levels {
            let y = pic.u(mm);
            pic.line(SECTION_DIM_LAYER, Vec2::new(-margin, y), Vec2::new(cut.len + margin / 2.0, y));
            pic.text(Vec2::new(-margin - pic.u(150.0), y), 150.0, format!("{:+.2}", mm / 1000.0), HAlign::Right);
        }
        let title = format!("{:.0}", cut.len * pic.units.mm_per_unit);
        pic.text(Vec2::new(cut.len / 2.0, -pic.u(500.0)), 200.0, title, HAlign::Center);
        Ok((cut.len, pic))
    }

    /// The air terminals, ducts, units, equipment and pipes the cut line goes through, each at the altitude of its underside.
    fn section_hvac(&self, pic: &mut Picture, cut: &Cut, entities: &[&crate::Entity]) {
        use crate::ventilation::{AHU_LAYER, AIR_TERMINAL_LAYER, EQUIPMENT_LAYER};
        // Where nothing was given: a duct has its underside at 2.50 m, an air terminal is at 2.50 m.
        const DEFAULT_DUCT_BOTTOM_MM: f64 = 2500.0;
        // The thickness of a diffuser or a grille, mm, and the height of a unit that has none.
        const TERMINAL_THICKNESS_MM: f64 = 60.0;
        const UNIT_HEIGHT_MM: f64 = 1000.0;
        // A plan rectangle of half sizes (`hx` along its axis, `hy` across) turned like the object it belongs to.
        let rotated = |i: &Insert, centre: Vec2, hx: f64, hy: f64| -> Vec<Vec2> {
            let (c, s) = (i.rotation.cos(), i.rotation.sin());
            [(-hx, -hy), (hx, -hy), (hx, hy), (-hx, hy)].iter().map(|(x, y)| centre + Vec2::new(x * c - y * s, x * s + y * c)).collect()
        };
        for e in entities {
            let EntityKind::Insert(i) = &e.kind else { continue };
            let at = Vec2::new(i.insert.x, i.insert.y);
            if let Some(t) = air_terminal_of(e) {
                let (half, bottom) = (t.half(), t.bottom_mm.unwrap_or(crate::ventilation::DEFAULT_TERMINAL_BOTTOM_MM));
                for (x0, x1) in cut.inside(&rotated(i, at, half.x, half.y)) {
                    pic.count.terminals += 1;
                    pic.rect_on(AIR_TERMINAL_LAYER, x0, x1, bottom, bottom + TERMINAL_THICKNESS_MM);
                }
            } else if let Some(p) = duct_of(e) {
                let (length, half) = (p.length * i.scale.x, p.units.to_units(p.plan_width_mm()) / 2.0);
                let axis = Vec2::from_angle(i.rotation);
                let local = [at, at + axis * length];
                let corners =
                    [local[0] + axis.perp() * half, local[1] + axis.perp() * half, local[1] - axis.perp() * half, local[0] - axis.perp() * half];
                let bottom = p.altitude_mm().unwrap_or(DEFAULT_DUCT_BOTTOM_MM);
                let tall = match p.shape {
                    crate::ventilation::DuctShape::Round => p.diameter_mm,
                    crate::ventilation::DuctShape::Rectangular => p.height_mm,
                };
                for (x0, x1) in cut.inside(&corners) {
                    pic.count.ducts += 1;
                    let wide = (x1 - x0) * pic.units.mm_per_unit;
                    // A round duct the line crosses squarely is a circle; one it follows, or crosses aslant, is the strip it makes.
                    if p.shape == crate::ventilation::DuctShape::Round && wide <= 1.3 * p.diameter_mm {
                        pic.circle_on(p.layer(), (x0 + x1) / 2.0, bottom + tall / 2.0, tall);
                    } else {
                        pic.rect_on(p.layer(), x0, x1, bottom, bottom + tall);
                    }
                }
            } else if let Some(a) = ahu_of(e) {
                let half = a.half();
                let bottom = a.bottom_mm.unwrap_or(0.0);
                for (x0, x1) in cut.inside(&rotated(i, at, half.x, half.y)) {
                    pic.count.equipment += 1;
                    pic.rect_on(AHU_LAYER, x0, x1, bottom, bottom + a.height_mm.unwrap_or(UNIT_HEIGHT_MM));
                }
            } else if let Some(q) = equipment_of(e) {
                let (hx, hy) = (q.units.to_units(q.length_mm) / 2.0, q.units.to_units(q.width_mm) / 2.0);
                let bottom = q.bottom_mm.unwrap_or(0.0);
                for (x0, x1) in cut.inside(&rotated(i, at, hx, hy)) {
                    pic.count.equipment += 1;
                    pic.rect_on(EQUIPMENT_LAYER, x0, x1, bottom, bottom + q.height_mm);
                }
            } else if let Some((p, a, b)) = pipe_of(e).and_then(|p| self.pipe_ends(e.handle).map(|(_, a, b)| (p, a, b))) {
                // A pipe is drawn where it has an altitude: as a circle across it, or a strip along it.
                let Some(bottom) = p.level_mm else { continue };
                let (half, axis) = (p.units.to_units(p.plan_width_mm()) / 2.0, (b - a).normalized());
                let corners = [a + axis.perp() * half, b + axis.perp() * half, b - axis.perp() * half, a - axis.perp() * half];
                for (x0, x1) in cut.inside(&corners) {
                    pic.count.pipes += 1;
                    let wide = (x1 - x0) * pic.units.mm_per_unit;
                    if wide <= 1.3 * p.plan_width_mm() {
                        pic.circle_on(p.layer(), (x0 + x1) / 2.0, bottom + p.plan_width_mm() / 2.0, p.plan_width_mm());
                    } else {
                        pic.rect_on(p.layer(), x0, x1, bottom, bottom + p.plan_width_mm());
                    }
                }
            }
        }
    }

    /// Draws the section along `a` to `b` named `name`, its ground level at `at` (drawing coordinates; to the right of the drawing when `None`),
    /// as one block reference, and the cut line with its letters in plan. Returns the picture, what it cut through and its width.
    pub fn add_section(
        &mut self,
        space: &Space,
        mut common: Common,
        a: Vec2,
        b: Vec2,
        name: &str,
        at: Option<Vec2>,
    ) -> Result<(Handle, SectionCount, f64)> {
        if at.is_some_and(|p| !p.is_finite()) {
            return Err(DocError::Invalid("A section needs a usable place.".into()));
        }
        let name = name.trim();
        if name.chars().count() > 32 || name.chars().any(char::is_control) {
            return Err(DocError::Invalid("A section's name is a short text.".into()));
        }
        let (width, mut pic) = self.section_picture(space, a, b)?;
        if pic.full {
            return Err(DocError::Invalid("The section cuts through too many objects.".into()));
        }
        let at = at.unwrap_or_else(|| {
            let ext = self.extents(space);
            if ext.min.is_finite() && ext.max.is_finite() {
                Vec2::new(ext.max.x + pic.units.to_units(3000.0), ext.min.y)
            } else {
                Vec2::new(0.0, 0.0)
            }
        });
        pic.text(Vec2::new(width / 2.0, -pic.u(900.0)), 300.0, format!("Section {name}"), HAlign::Center);
        for layer in [SECTION_LAYER, SECTION_DIM_LAYER, SECTION_LINE_LAYER] {
            self.ensure_layer(layer);
        }
        let count = pic.count;
        let block = self.hvac_block(pic.pieces);
        common.layer = SECTION_LAYER.into();
        let h = self.add(space, common.clone(), EntityKind::Insert(Insert { block, insert: v3(at), ..Default::default() }))?;
        // The cut line in plan, with the letters at its ends.
        let mut mark = common;
        mark.layer = SECTION_LINE_LAYER.into();
        mark.xdata = Vec::new();
        self.add(space, mark.clone(), EntityKind::Line(Line { a: v3(a), b: v3(b) }))?;
        let height = HvacUnits::of(self).to_units(250.0);
        let d = (b - a) * (1.0 / a.dist(b).max(1e-12));
        for (p, away) in [(a, -d), (b, d)] {
            let t = Text {
                insert: v3(p + away * (1.5 * height)),
                align_pt: Some(v3(p + away * (1.5 * height))),
                height,
                value: name.to_string(),
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                style: "Standard".into(),
                halign: HAlign::Center,
                valign: VAlign::Middle,
            };
            self.add(space, mark.clone(), EntityKind::Text(t))?;
        }
        Ok((h, count, width))
    }
}

/// The cut of one wall: for each segment the line crosses, the wall from the ground to its height, with a gap where an opening stands.
/// A point of the drawing in the block of a block reference: what [`to_world`] undoes.
fn to_local(i: &Insert, p: Vec2) -> Vec2 {
    let (c, s) = (i.rotation.cos(), i.rotation.sin());
    let (dx, dy) = (p.x - i.insert.x, p.y - i.insert.y);
    let (x, y) = (dx * c + dy * s, -dx * s + dy * c);
    Vec2::new(if i.scale.x.abs() > 1e-12 { x / i.scale.x } else { 0.0 }, if i.scale.y.abs() > 1e-12 { y / i.scale.y } else { 0.0 })
}

/// How thick a roof is drawn in a section, mm (a roof has no thickness of its own).
const SECTION_ROOF_THICKNESS_MM: f64 = 200.0;

/// A roof the cut line crosses: the line of its surface, with the slopes where the planes meet (the ridge) and where the overhang ends, and the same line
/// a little lower, closed at both ends.
fn section_roof(pic: &mut Picture, cut: &Cut, i: &Insert, r: &crate::ventilation::RoofParams) {
    let eaves: Vec<Vec2> = r.eaves_outline().iter().map(|q| to_world(i, *q)).collect();
    let terms = r.height_terms();
    let stretches = cut.inside(&eaves);
    if stretches.is_empty() || terms.is_empty() {
        return;
    }
    pic.count.roofs += 1;
    for (x0, x1) in stretches {
        let (la, lb) = (to_local(i, cut.a + cut.d * x0), to_local(i, cut.a + cut.d * x1));
        // Each plane's height at the two ends of the stretch: it is a straight line between them, and the roof is the lowest plane.
        let at = |p: Vec2| terms.iter().map(|(g, c)| g.dot(p) + c).collect::<Vec<f64>>();
        let (va, vb) = (at(la), at(lb));
        let mut steps = vec![0.0, 1.0];
        for a in 0..terms.len() {
            for b in a + 1..terms.len() {
                let (da, db) = (va[a] - va[b], vb[a] - vb[b]);
                if (da - db).abs() > 1e-12 {
                    let s = da / (da - db);
                    if s > 1e-9 && s < 1.0 - 1e-9 {
                        steps.push(s);
                    }
                }
            }
        }
        steps.sort_by(f64::total_cmp);
        steps.dedup_by(|p, q| (*p - *q).abs() < 1e-9);
        let height = |s: f64| (0..terms.len()).map(|n| va[n] + (vb[n] - va[n]) * s).fold(f64::MAX, f64::min);
        let top: Vec<Vec2> = steps.iter().map(|s| Vec2::new(x0 + s * (x1 - x0), pic.u(height(*s)))).collect();
        let low: Vec<Vec2> = top.iter().map(|p| Vec2::new(p.x, p.y - pic.u(SECTION_ROOF_THICKNESS_MM))).collect();
        for w in top.windows(2).chain(low.windows(2)) {
            if let [a, b] = w {
                pic.line(SECTION_LAYER, *a, *b);
            }
        }
        if let (Some(a), Some(b), Some(c), Some(d)) = (top.first(), low.first(), top.last(), low.last()) {
            pic.line(SECTION_LAYER, *a, *b);
            pic.line(SECTION_LAYER, *c, *d);
        }
        // The level of the top, in the margin.
        if let Some(y) = top.iter().map(|p| p.y).reduce(f64::max) {
            pic.levels.push(y * pic.units.mm_per_unit);
        }
    }
}

fn section_wall(pic: &mut Picture, cut: &Cut, i: &Insert, wall: &WallParams, openings: &[(Vec2, f64, crate::OpeningParams)]) {
    let place = joint::Placement {
        insert: Vec2::new(i.insert.x, i.insert.y),
        rotation: i.rotation,
        scale: Vec2::new(i.scale.x.signum(), i.scale.y.signum()),
    };
    let placed = joint::Placed { params: wall.clone(), at: place };
    let path = wall.centreline();
    let bounds = wall.boundaries();
    let (Some(&hi), Some(&lo)) = (bounds.first(), bounds.last()) else { return };
    let n = path.len();
    let segs = if wall.closed { n } else { n.saturating_sub(1) };
    let height = wall.height;
    for seg in 0..segs {
        let (Some(&la), Some(&lb)) = (path.get(seg), path.get((seg + 1) % n)) else { continue };
        let along = lb - la;
        if along.len() <= 1e-9 {
            continue;
        }
        let ln = Vec2::new(-along.y, along.x).normalized();
        let corners = [place.to_world(la + ln * hi), place.to_world(lb + ln * hi), place.to_world(lb + ln * lo), place.to_world(la + ln * lo)];
        let (wa, wb) = (place.to_world(la), place.to_world(lb));
        let wd = (wb - wa).normalized();
        for (x0, x1) in cut.inside(&corners) {
            pic.count.walls += 1;
            // Where the cut line crosses the wall's own line, to see if an opening stands there.
            let mid = cut.a + cut.d * ((x0 + x1) / 2.0);
            let t = (mid - wa).dot(wd);
            let found = openings.iter().find_map(|(at, rotation, op)| {
                let (c, _) = placed.host(*at, *rotation, op.width())?;
                (c.segment == seg && c.from <= t && t <= c.to).then_some(op)
            });
            match found {
                Some(op) => {
                    pic.count.openings += 1;
                    let (sill, head) = (op.sill_mm, op.sill_mm + op.height_mm);
                    pic.rect(x0, x1, 0.0, sill, true);
                    pic.rect(x0, x1, head, height, true);
                    // The joinery: a line at the head, and for a window the glass with its sill.
                    let (y_head, y_sill) = (pic.u(head), pic.u(sill));
                    pic.line(SECTION_LAYER, Vec2::new(x0, y_head), Vec2::new(x1, y_head));
                    if matches!(op.kind, OpeningKind::Window) {
                        pic.line(SECTION_LAYER, Vec2::new(x0, y_sill), Vec2::new(x1, y_sill));
                        let mid_y = (y_head + y_sill) / 2.0;
                        pic.line(SECTION_DIM_LAYER, Vec2::new(x0, mid_y), Vec2::new(x1, mid_y));
                    }
                }
                None => pic.rect(x0, x1, 0.0, height, true),
            }
        }
    }
}
