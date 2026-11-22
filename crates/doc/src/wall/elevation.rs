//! The elevation of a wall: the face of the wall seen straight on, as a drawing in model space beside
//! the plan. Its outline is the wall's developed length (along its path, round its bends and arcs)
//! by its height; every opening in it is drawn at its place with its sill and height, and a chain of
//! dimensions below gives the distances between the openings. It is one block reference, so it
//! moves, copies and explodes like any other object, and it is a picture made now: it does not
//! follow later changes to the plan. What stands against the face seen (equipment and furniture, within a metre of it) is drawn too, at its place and height.

use cadcraft_geom::{PolyVertex, Vec2};

use super::{WallParams, joint, wall_of};
use crate::catalogue::{HvacPiece, equipment_of, furniture_of};
use crate::{Common, DocError, Drawing, EntityKind, HAlign, Handle, Line, LwPolyline, OpeningKind, Result, Space, Text, VAlign, opening_of};

/// The layers of an elevation.
pub const ELEVATION_LAYER: &str = "A-ELEV";
pub const ELEVATION_DOOR_LAYER: &str = "A-ELEV-DOOR";
pub const ELEVATION_WINDOW_LAYER: &str = "A-ELEV-GLAZ";
pub const ELEVATION_DIM_LAYER: &str = "A-ELEV-DIMS";
/// What stands in front of the wall: equipment and furniture, seen as they are on it.
pub const ELEVATION_OBJECT_LAYER: &str = "A-ELEV-OBJ";

/// Most objects drawn in front of one wall, and how far from its face (mm) one still counts as standing against it.
const MAX_ELEVATION_OBJECTS: usize = 512;
const OBJECT_DEPTH_MM: f64 = 1000.0;

/// An object standing in front of a wall, seen from the front: where it is along the wall and in height (drawing units) and what it is called.
#[derive(Clone, Debug, PartialEq)]
struct FrontObject {
    from: f64,
    to: f64,
    bottom: f64,
    top: f64,
    name: String,
}

/// Most openings drawn in one elevation.
const MAX_ELEVATION_OPENINGS: usize = 512;

/// The side the elevation is seen from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ElevationSide {
    /// Seen from the right of the wall's path: the start is on the left, as the path runs.
    #[default]
    Right,
    /// Seen from the left of the path: drawn the other way round, the start on the right.
    Left,
}

impl ElevationSide {
    pub fn parse(text: &str) -> Option<ElevationSide> {
        match text.trim().to_ascii_lowercase().as_str() {
            "right" | "droite" | "r" => Some(ElevationSide::Right),
            "left" | "gauche" | "l" => Some(ElevationSide::Left),
            _ => None,
        }
    }
}

fn v3(p: Vec2) -> cadcraft_geom::Vec3 {
    p.to3(0.0)
}

/// What an elevation is made of, in the block's coordinates: its width (the wall's developed
/// length) and its pieces. `openings` are, for each opening, its kind, its centre along the wall,
/// its width, sill and height (all in drawing units).
fn pieces_of(
    wall: &WallParams,
    openings: &[(OpeningKind, f64, f64, f64, f64)],
    objects: &[FrontObject],
    side: ElevationSide,
) -> (f64, Vec<HvacPiece>) {
    let u = |mm: f64| wall.units.to_units(mm);
    let mm = wall.units.mm_per_unit;
    let path = wall.centreline();
    let n = path.len();
    let count = if wall.closed { n } else { n.saturating_sub(1) };
    let length: f64 = (0..count).filter_map(|i| Some(path.get(i)?.dist(*path.get((i + 1) % n)?))).sum();
    let height = u(wall.height);
    // The picture is mirrored when seen from the left.
    let x = |s: f64| match side {
        ElevationSide::Right => s,
        ElevationSide::Left => length - s,
    };
    let rect = |layer: &'static str, x0: f64, y0: f64, x1: f64, y1: f64| {
        let (a, b) = (x0.min(x1), x0.max(x1));
        let corners = [Vec2::new(a, y0), Vec2::new(b, y0), Vec2::new(b, y1), Vec2::new(a, y1)];
        HvacPiece {
            layer,
            kind: EntityKind::LwPolyline(LwPolyline {
                vertices: corners.iter().map(|p| PolyVertex::new(*p)).collect(),
                closed: true,
                const_width: 0.0,
                elevation: 0.0,
                plinegen: false,
            }),
        }
    };
    let line = |layer: &'static str, a: Vec2, b: Vec2| HvacPiece { layer, kind: EntityKind::Line(Line { a: v3(a), b: v3(b) }) };
    let text_height = u(100.0);
    let text = |layer: &'static str, at: Vec2, value: String| {
        let t = Text {
            insert: v3(at),
            align_pt: Some(v3(at)),
            height: text_height,
            value,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign: HAlign::Center,
            valign: VAlign::Middle,
        };
        HvacPiece { layer, kind: EntityKind::Text(t) }
    };
    let mut out = vec![rect(ELEVATION_LAYER, 0.0, 0.0, length, height)];
    let mut edges = vec![0.0, length];
    for &(kind, centre, w, sill, h) in openings {
        let layer = if kind == OpeningKind::Window { ELEVATION_WINDOW_LAYER } else { ELEVATION_DOOR_LAYER };
        let (left, right, bottom, top) = (x(centre - w / 2.0), x(centre + w / 2.0), sill, sill + h);
        out.push(rect(layer, left, bottom, right, top));
        let mid = (left + right) / 2.0;
        match kind {
            OpeningKind::Window => {
                out.push(line(layer, Vec2::new(mid, bottom), Vec2::new(mid, top)));
                out.push(line(layer, Vec2::new(left, (bottom + top) / 2.0), Vec2::new(right, (bottom + top) / 2.0)));
            }
            OpeningKind::DoubleDoor => out.push(line(layer, Vec2::new(mid, bottom), Vec2::new(mid, top))),
            OpeningKind::Door | OpeningKind::Bay => {}
        }
        // Its size above it, and its sill when it has one.
        let label =
            if sill > 1e-9 { format!("{:.0} x {:.0}  (h {:.0})", w * mm, h * mm, sill * mm) } else { format!("{:.0} x {:.0}", w * mm, h * mm) };
        out.push(text(ELEVATION_DIM_LAYER, Vec2::new(mid, top + 1.5 * text_height), label));
        edges.extend([centre - w / 2.0, centre + w / 2.0]);
    }
    // What stands in front of the wall, with its name above it.
    for o in objects {
        let (a, b) = (x(o.from), x(o.to));
        out.push(rect(ELEVATION_OBJECT_LAYER, a, o.bottom, b, o.top));
        if !o.name.is_empty() {
            out.push(text(ELEVATION_OBJECT_LAYER, Vec2::new((a + b) / 2.0, o.top + 0.8 * text_height), o.name.clone()));
        }
    }
    // The wall's height, on its left, and the chain of dimensions below.
    out.push(text(ELEVATION_DIM_LAYER, Vec2::new(-u(300.0), height / 2.0), format!("{:.0}", wall.height)));
    edges.retain(|e| e.is_finite() && *e >= -1e-9 && *e <= length + 1e-9);
    edges.sort_by(f64::total_cmp);
    edges.dedup_by(|a, b| (*a - *b).abs() < u(1.0));
    let (y, tick) = (-u(400.0), u(60.0));
    for pair in edges.windows(2) {
        let (a, b) = (x(pair[0]), x(pair[1]));
        out.push(line(ELEVATION_DIM_LAYER, Vec2::new(a, y), Vec2::new(b, y)));
        for e in [a, b] {
            out.push(line(ELEVATION_DIM_LAYER, Vec2::new(e, y - tick), Vec2::new(e, y + tick)));
        }
        out.push(text(ELEVATION_DIM_LAYER, Vec2::new((a + b) / 2.0, y - 2.0 * tick - text_height / 2.0), format!("{:.0}", (pair[1] - pair[0]) * mm)));
    }
    (length, out)
}

impl Drawing {
    /// The pieces of the elevation of wall `wall` seen from `side`, with the width of the picture:
    /// the wall's outline from the corner at the origin, and the openings of `space` found in it.
    /// Refuses what is not a wall.
    pub fn wall_elevation_pieces(&self, space: &Space, wall: Handle, side: ElevationSide) -> Result<(f64, Vec<HvacPiece>)> {
        let e = self.entity(wall).ok_or(DocError::NoEntity(wall))?;
        let (EntityKind::Insert(i), Some(params)) = (&e.kind, wall_of(e)) else {
            return Err(DocError::Invalid("The object is not a wall.".into()));
        };
        let place = joint::Placement {
            insert: Vec2::new(i.insert.x, i.insert.y),
            rotation: i.rotation,
            scale: Vec2::new(i.scale.x.signum(), i.scale.y.signum()),
        };
        let placed = joint::Placed { params: params.clone(), at: place };
        let path = params.centreline();
        // Where each segment of the centre line begins along the developed length.
        let mut start = vec![0.0];
        for k in 0..path.len().saturating_sub(1) {
            let a = path.get(k).copied().unwrap_or(Vec2::ZERO);
            let b = path.get(k + 1).copied().unwrap_or(a);
            start.push(start.last().copied().unwrap_or(0.0) + a.dist(b));
        }
        let mut openings = Vec::new();
        for o in self.space(space).into_iter().flat_map(|s| s.iter()) {
            let (EntityKind::Insert(oi), Some(op)) = (&o.kind, opening_of(o)) else { continue };
            if let Some((cut, _)) = placed.host(Vec2::new(oi.insert.x, oi.insert.y), oi.rotation, op.width()) {
                let centre = start.get(cut.segment).copied().unwrap_or(0.0) + (cut.from + cut.to) / 2.0;
                let u = |mm: f64| op.units.to_units(mm);
                openings.push((op.kind, centre, op.width(), u(op.sill_mm), u(op.height_mm)));
                if openings.len() >= MAX_ELEVATION_OPENINGS {
                    break;
                }
            }
        }
        openings.sort_by(|a, b| a.1.total_cmp(&b.1));
        let objects = self.front_objects(space, &placed, &start, side);
        Ok(pieces_of(&params, &openings, &objects, side))
    }

    /// The equipment and furniture of `space` that stand against the face of the wall seen from `side` (within a metre of it,
    /// in front of the segment they are nearest to), as the elevation shows them: their extent along the wall and in height, from the altitude of their
    /// underside (the floor, or 2.50 m for a terminal that has none, as a section puts it).
    fn front_objects(&self, space: &Space, wall: &joint::Placed, start: &[f64], side: ElevationSide) -> Vec<FrontObject> {
        let params = &wall.params;
        let units = params.units;
        let (u, mm) = (|v: f64| units.to_units(v), units.mm_per_unit);
        let path: Vec<Vec2> = params.centreline().iter().map(|p| wall.at.to_world(*p)).collect();
        let n = path.len();
        let segments = if params.closed { n } else { n.saturating_sub(1) };
        let thick = params.style.thickness() / mm;
        let depth = OBJECT_DEPTH_MM / mm;
        let sign = if side == ElevationSide::Right { 1.0 } else { -1.0 };
        let mut out = Vec::new();
        for o in self.space(space).into_iter().flat_map(|s| s.iter()) {
            let EntityKind::Insert(i) = &o.kind else { continue };
            // (half sizes in plan, mm; underside and height, mm; name)
            let (half, bottom, height, name) = if let Some(q) = equipment_of(o) {
                ((q.length_mm / 2.0, q.width_mm / 2.0), q.bottom_mm.unwrap_or(0.0), q.height_mm, q.name)
            } else if let Some(f) = furniture_of(o) {
                ((f.width_mm / 2.0, f.depth_mm / 2.0), f.bottom_mm.unwrap_or(0.0), f.height_mm, f.name)
            } else {
                continue;
            };
            let c = Vec2::new(i.insert.x, i.insert.y);
            // The segment nearest to the object's centre.
            let mut best: Option<(f64, usize, f64, f64, Vec2)> = None;
            for k in 0..segments {
                let (Some(&a), Some(&b)) = (path.get(k), path.get((k + 1) % n)) else { continue };
                let len = a.dist(b);
                if len <= 1e-9 {
                    continue;
                }
                let d = (b - a) / len;
                let t = (c - a).dot(d).clamp(0.0, len);
                let off = (c - (a + d * t)).len();
                if best.as_ref().is_none_or(|x| off < x.0) {
                    best = Some((off, k, t, (c - a).dot(Vec2::new(d.y, -d.x)) * sign, d));
                }
            }
            let Some((_, k, t, side_off, d)) = best else { continue };
            if side_off < -thick || side_off > depth + thick / 2.0 {
                continue;
            }
            // How far the object reaches along the wall: its half sizes turned to the wall's direction.
            let phi = i.rotation - d.angle();
            let (hx, hy) = (half.0 / mm, half.1 / mm);
            let along = hx * phi.cos().abs() + hy * phi.sin().abs();
            let centre = start.get(k).copied().unwrap_or(0.0) + t;
            out.push(FrontObject { from: centre - along, to: centre + along, bottom: u(bottom), top: u(bottom + height), name });
            if out.len() >= MAX_ELEVATION_OBJECTS {
                break;
            }
        }
        out
    }

    /// Draws the elevation of wall `wall` with its lower left corner at `at` (drawing coordinates),
    /// as one block reference; returns it with the width of the picture. Refuses what is not a wall.
    pub fn add_wall_elevation(&mut self, space: &Space, mut common: Common, wall: Handle, at: Vec2, side: ElevationSide) -> Result<(Handle, f64)> {
        if !at.is_finite() {
            return Err(DocError::Invalid("An elevation needs a usable place.".into()));
        }
        let (width, pieces) = self.wall_elevation_pieces(space, wall, side)?;
        for layer in [ELEVATION_LAYER, ELEVATION_DOOR_LAYER, ELEVATION_WINDOW_LAYER, ELEVATION_DIM_LAYER, ELEVATION_OBJECT_LAYER] {
            self.ensure_layer(layer);
        }
        let block = self.hvac_block(pieces);
        common.layer = ELEVATION_LAYER.into();
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        let h = self.add(space, common, EntityKind::Insert(insert))?;
        Ok((h, width))
    }
}

#[cfg(test)]
mod tests;
