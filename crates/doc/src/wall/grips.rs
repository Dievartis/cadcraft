//! The grips of a wall: one at each vertex of its path (drag it to lengthen or shorten the wall) and one in the middle of each segment (drag it to move that
//! segment, its two ends with it), like the grips of a duct run.

use cadcraft_geom::Vec2;

use super::{WallJoints, WallParams, wall_of};
use crate::{Drawing, Entity, EntityKind, Result};

/// A wall's path in the drawing, and what it takes to turn one into the other.
struct Placed {
    params: WallParams,
    insert: Vec2,
    rotation: f64,
    scale: Vec2,
}

impl Placed {
    fn of(d: &Drawing, e: &Entity) -> Option<Placed> {
        let (EntityKind::Insert(i), Some(params)) = (&e.kind, wall_of(e)) else { return None };
        // A wall edited by hand has no path to follow.
        d.wall_is_intact(e.handle).then(|| Placed {
            params,
            insert: Vec2::new(i.insert.x, i.insert.y),
            rotation: i.rotation,
            scale: Vec2::new(i.scale.x.signum(), i.scale.y.signum()),
        })
    }
    fn world(&self, p: Vec2) -> Vec2 {
        self.insert + Vec2::new(p.x * self.scale.x, p.y * self.scale.y).rotate(self.rotation)
    }
    /// A point of the drawing in the block's coordinates, the block placed at `insert`.
    fn local(&self, insert: Vec2, p: Vec2) -> Vec2 {
        let q = (p - insert).rotate(-self.rotation);
        Vec2::new(q.x * self.scale.x, q.y * self.scale.y)
    }
    fn path(&self) -> Vec<Vec2> {
        self.params.path.iter().map(|p| self.world(*p)).collect()
    }
    /// The ends of segment `k` of `path`.
    fn segment(&self, path: &[Vec2], k: usize) -> Option<(Vec2, Vec2)> {
        Some((*path.get(k)?, *path.get((k + 1) % path.len())?))
    }
    fn segments(&self) -> usize {
        let n = self.params.path.len();
        if self.params.closed { n } else { n.saturating_sub(1) }
    }
}

impl Drawing {
    /// The grips of wall `e`: its vertices, then the middle of each segment; `None` when `e` is not a wall that is as it was drawn.
    pub fn wall_grips(&self, e: &Entity) -> Option<Vec<Vec2>> {
        let w = Placed::of(self, e)?;
        let path = w.path();
        let mids = (0..w.segments()).filter_map(|k| w.segment(&path, k).map(|(a, b)| a.mid(b)));
        Some(path.iter().copied().chain(mids).collect())
    }

    /// The points of the drawing that grip `index` of wall `h` moves: the vertex, or the two ends of the segment.
    pub fn wall_grip_points(&self, h: crate::Handle, index: usize) -> Vec<Vec2> {
        let Some(w) = self.entity(h).and_then(|e| Placed::of(self, e)) else { return Vec::new() };
        let path = w.path();
        match index.checked_sub(path.len()) {
            None => path.get(index).copied().into_iter().collect(),
            Some(k) => w.segment(&path, k).map(|(a, b)| vec![a, b]).unwrap_or_default(),
        }
    }

    /// Drags grip `index` of wall `h` to `to`: a vertex goes there, a segment moves with the grip in its middle. The wall is redrawn (the walls it meets are not:
    /// see [`Drawing::cleanup_walls`]). `Ok(false)` when `h` is not a wall, the grip does not exist or the wall would be degenerate; a wall edited by hand moves.
    pub fn stretch_wall_grip(&mut self, h: crate::Handle, index: usize, to: Vec2) -> Result<bool> {
        let Some(e) = self.entity(h) else { return Ok(false) };
        if !to.is_finite() {
            return Ok(false);
        }
        let Some(w) = Placed::of(self, e) else {
            // A wall edited by hand has the one grip of its block reference, which moves it.
            if index != 0 || !matches!(e.kind, EntityKind::Insert(_)) {
                return Ok(false);
            }
            self.modify_entity(h, |e| {
                if let EntityKind::Insert(i) = &mut e.kind {
                    (i.insert.x, i.insert.y) = (to.x, to.y);
                }
            })?;
            return Ok(true);
        };
        let mut path = w.path();
        let n = path.len();
        match index.checked_sub(n) {
            None => match path.get_mut(index) {
                Some(p) => *p = to,
                None => return Ok(false),
            },
            Some(k) => {
                let Some((a, b)) = w.segment(&path, k) else { return Ok(false) };
                let delta = to - a.mid(b);
                for j in [k, (k + 1) % n] {
                    if let Some(p) = path.get_mut(j) {
                        *p += delta;
                    }
                }
            }
        }
        let Some(first) = path.first().copied() else { return Ok(false) };
        let mut params = w.params.clone();
        params.path = path.iter().map(|p| w.local(first, *p)).collect();
        // What the wall kept about its neighbours and its openings was worked out for the old path.
        params.joints = WallJoints::default();
        if params.validate().is_err() {
            return Ok(false);
        }
        self.regenerate_wall(h, &params)?;
        self.modify_entity(h, |e| {
            if let EntityKind::Insert(i) = &mut e.kind {
                (i.insert.x, i.insert.y) = (first.x, first.y);
            }
        })?;
        Ok(true)
    }
}
