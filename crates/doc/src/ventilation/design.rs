//! Design objects of a plan: the grid of a false ceiling, a straight stair and a railing. Like the other plan objects each is a block reference to
//! an anonymous block, its data travelling as extended data of its own registered application.

use cadcraft_geom::{PolyVertex, Vec2, point_in_polygon, polygon_centroid, shoelace};
use serde::{Deserialize, Serialize};

use super::part::{line, poly};
use super::slab::{block_to_world, world_to_block};
use super::{HvacPiece, HvacUnits, check_label_height, check_name, invalid, label, v3, xdata};
use crate::{EntityKind, LwPolyline, RawTag, Result};

/// The registered application names of the data of a ceiling grid, a stair and a railing.
pub const CEILING_APP: &str = "CADCRAFT_CEILING";
pub const STAIR_APP: &str = "CADCRAFT_STAIR";
pub const RAIL_APP: &str = "CADCRAFT_RAIL";
/// The layers of a ceiling grid and its outline, of the stairs and of the railings.
pub const CEILING_LAYER: &str = "A-CLNG";
pub const CEILING_GRID_LAYER: &str = "A-CLNG-GRID";
pub const STAIR_LAYER: &str = "A-FLOR-STRS";
pub const RAIL_LAYER: &str = "A-FLOR-HRAL";
const CEILING_FORMAT: &str = "CEILING1";
const STAIR_FORMAT: &str = "STAIR1";
const RAIL_FORMAT: &str = "RAIL1";
/// Most vertices of a ceiling's outline or of a railing's path.
pub const MAX_DESIGN_VERTICES: usize = 256;
/// Most grid lines of a ceiling in each direction, and most tiles it counts.
const MAX_GRID_LINES: usize = 4000;
const MAX_TILES: usize = 400_000;
/// Most risers of a stair, and posts of a railing.
pub const MAX_RISERS: usize = 200;
const MAX_POSTS: usize = 2000;

fn default_label_mm() -> f64 {
    150.0
}

fn yes() -> bool {
    true
}

// ---------------------------------------------------------------------------------------------------------------
// Ceiling grid
// ---------------------------------------------------------------------------------------------------------------

/// The grid of a false ceiling over a room: the lines between the tiles, clipped to the room's outline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeilingParams {
    pub name: String,
    /// The room's outline in the block's coordinates (drawing units), without repeating the first point.
    pub outline: Vec<Vec2>,
    /// A tile's two sides, mm: 600 x 600 by default, 600 x 1200 or 1200 x 600 for a long tile.
    pub tile_x_mm: f64,
    pub tile_y_mm: f64,
    /// The turn of the grid, degrees, and where one of its lines crosses another (the block's coordinates): a tile corner.
    #[serde(default)]
    pub angle_deg: f64,
    #[serde(default)]
    pub origin: Vec2,
    /// The height of the underside of the false ceiling, mm.
    #[serde(default = "default_ceiling_height")]
    pub elevation_mm: f64,
    #[serde(default = "yes")]
    pub show_label: bool,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

fn default_ceiling_height() -> f64 {
    2600.0
}

/// How many tiles a ceiling has: whole ones, and ones cut by the walls (a tile is cut when it is not whole but part of it is in the room).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileCount {
    pub whole: usize,
    pub cut: usize,
    pub area_m2: f64,
}

impl CeilingParams {
    /// A grid of 600 x 600 tiles over `outline`, its lines crossing at the outline's lower left corner.
    pub fn new(name: &str, outline: Vec<Vec2>, units: HvacUnits) -> CeilingParams {
        let origin = outline.iter().fold(Vec2::new(f64::MAX, f64::MAX), |m, p| Vec2::new(m.x.min(p.x), m.y.min(p.y)));
        CeilingParams {
            name: name.trim().to_string(),
            outline,
            tile_x_mm: 600.0,
            tile_y_mm: 600.0,
            angle_deg: 0.0,
            origin,
            elevation_mm: default_ceiling_height(),
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("ceiling", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.outline.len() < 3 || self.outline.len() > MAX_DESIGN_VERTICES {
            return Err(invalid(format!("A ceiling's outline has 3 to {MAX_DESIGN_VERTICES} points.")));
        }
        if self.outline.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12) || !self.origin.is_finite() {
            return Err(invalid("A ceiling's outline has a point that is not a usable number."));
        }
        if shoelace(&self.outline).abs() <= 1e-12 {
            return Err(invalid("A ceiling's outline encloses no area."));
        }
        if ![self.tile_x_mm, self.tile_y_mm].iter().all(|t| t.is_finite() && (100.0..=5000.0).contains(t)) {
            return Err(invalid("A ceiling tile is 100 to 5000 mm on each side."));
        }
        if !(self.angle_deg.is_finite() && self.elevation_mm.is_finite() && self.elevation_mm.abs() <= 1.0e5) {
            return Err(invalid("A ceiling's angle and height are not usable numbers."));
        }
        let (min, max) = self.frame_bounds();
        let (nx, ny) = ((max.x - min.x) / self.tile_x(), (max.y - min.y) / self.tile_y());
        if !(nx.is_finite() && ny.is_finite()) || nx > MAX_GRID_LINES as f64 || ny > MAX_GRID_LINES as f64 || nx * ny > MAX_TILES as f64 {
            return Err(invalid(format!("That ceiling has too many tiles (at most {MAX_TILES}): use a bigger tile.")));
        }
        Ok(())
    }

    /// A tile's sides in drawing units.
    fn tile_x(&self) -> f64 {
        self.units.to_units(self.tile_x_mm)
    }

    fn tile_y(&self) -> f64 {
        self.units.to_units(self.tile_y_mm)
    }

    /// A point of the block in the grid's own frame: its origin, turned so that the grid's lines are the axes.
    fn to_frame(&self, p: Vec2) -> Vec2 {
        let (c, s) = (self.angle_deg.to_radians().cos(), self.angle_deg.to_radians().sin());
        let d = p - self.origin;
        Vec2::new(d.x * c + d.y * s, -d.x * s + d.y * c)
    }

    /// The inverse of [`Self::to_frame`].
    fn out_of_frame(&self, q: Vec2) -> Vec2 {
        let (c, s) = (self.angle_deg.to_radians().cos(), self.angle_deg.to_radians().sin());
        self.origin + Vec2::new(q.x * c - q.y * s, q.x * s + q.y * c)
    }

    fn frame_outline(&self) -> Vec<Vec2> {
        self.outline.iter().map(|p| self.to_frame(*p)).collect()
    }

    /// The bounds of the outline in the grid's frame.
    fn frame_bounds(&self) -> (Vec2, Vec2) {
        self.frame_outline().iter().fold((Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN)), |(lo, hi), p| {
            (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y)))
        })
    }

    /// The pieces of the grid lines along one axis of the frame: the segments of the lines `axis = k * pitch` inside the outline.
    fn lines(&self, vertical: bool) -> Vec<(Vec2, Vec2)> {
        let poly = self.frame_outline();
        let (min, max) = self.frame_bounds();
        let (pitch, lo, hi) = if vertical { (self.tile_x(), min.x, max.x) } else { (self.tile_y(), min.y, max.y) };
        let first = (lo / pitch).ceil() as i64;
        let last = (hi / pitch).floor() as i64;
        let mut out = Vec::new();
        for k in first..=last {
            let at = k as f64 * pitch;
            // A line on the edge of the room's extent is the outline itself: only the lines between the walls are drawn.
            if at - lo < 1e-9 * pitch || hi - at < 1e-9 * pitch {
                continue;
            }
            // Where the line meets the outline: the other coordinate of each crossing, in order; pairs of them bound what is inside.
            let mut cuts: Vec<f64> = Vec::new();
            for i in 0..poly.len() {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                let (pa, pb, qa, qb) = if vertical { (a.x, b.x, a.y, b.y) } else { (a.y, b.y, a.x, b.x) };
                // A half-open rule so that a corner on the line is counted once.
                if (pa <= at && pb > at) || (pb <= at && pa > at) {
                    cuts.push(qa + (at - pa) / (pb - pa) * (qb - qa));
                }
            }
            cuts.sort_by(f64::total_cmp);
            for &[u, v] in cuts.as_chunks::<2>().0 {
                if v - u > 1e-9 {
                    let (a, b) = if vertical { (Vec2::new(at, u), Vec2::new(at, v)) } else { (Vec2::new(u, at), Vec2::new(v, at)) };
                    out.push((self.out_of_frame(a), self.out_of_frame(b)));
                }
            }
        }
        out
    }

    /// The tiles: those whose centre is in the room, and among them those that are whole (their four corners are in it too).
    pub fn tiles(&self) -> TileCount {
        let poly = self.frame_outline();
        let (min, max) = self.frame_bounds();
        let (tx, ty) = (self.tile_x(), self.tile_y());
        let (i0, i1) = ((min.x / tx).floor() as i64, (max.x / tx).ceil() as i64);
        let (j0, j1) = ((min.y / ty).floor() as i64, (max.y / ty).ceil() as i64);
        let mut n = TileCount::default();
        if (i1 - i0).saturating_mul(j1 - j0) > MAX_TILES as i64 {
            return n;
        }
        for i in i0..i1 {
            for j in j0..j1 {
                let (x, y) = (i as f64 * tx, j as f64 * ty);
                // The corners a hair inside the tile: a tile against a wall has its corners on the outline, which is not outside it.
                let (ex, ey) = (tx * 1e-6, ty * 1e-6);
                let corners =
                    [Vec2::new(x + ex, y + ey), Vec2::new(x + tx - ex, y + ey), Vec2::new(x + tx - ex, y + ty - ey), Vec2::new(x + ex, y + ty - ey)];
                let inside = corners.iter().filter(|c| point_in_polygon(&poly, **c)).count();
                if inside == 4 {
                    n.whole += 1;
                } else if inside > 0
                    || point_in_polygon(&poly, Vec2::new(x + tx / 2.0, y + ty / 2.0))
                    || poly.iter().any(|v| v.x > x && v.x < x + tx && v.y > y && v.y < y + ty)
                {
                    // Cut by a wall: it is part in the room.
                    n.cut += 1;
                }
            }
        }
        n.area_m2 = self.units.area_m2(shoelace(&self.outline).abs());
        n
    }

    /// The centre of the tile that holds `p` (the block's coordinates), when its centre is in the room.
    pub fn tile_centre(&self, p: Vec2) -> Option<Vec2> {
        let q = self.to_frame(p);
        let (tx, ty) = (self.tile_x(), self.tile_y());
        let c = Vec2::new(((q.x / tx).floor() + 0.5) * tx, ((q.y / ty).floor() + 0.5) * ty);
        point_in_polygon(&self.frame_outline(), c).then(|| self.out_of_frame(c))
    }

    /// The text written in the room: its name, the tile and the height of the false ceiling.
    pub fn label_text(&self) -> String {
        format!("{} {:.0}x{:.0} {:+.2} m", self.name, self.tile_x_mm, self.tile_y_mm, self.elevation_mm / 1000.0)
    }

    /// The room's outline, the grid lines inside it and the label at its centre.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let vertices: Vec<PolyVertex> = self.outline.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        let mut pieces = vec![HvacPiece {
            layer: CEILING_LAYER,
            kind: EntityKind::LwPolyline(LwPolyline { vertices, closed: true, const_width: 0.0, elevation: 0.0, plinegen: false }),
        }];
        for vertical in [true, false] {
            for (a, b) in self.lines(vertical) {
                pieces.push(HvacPiece { layer: CEILING_GRID_LAYER, kind: line((a.x, a.y), (b.x, b.y)) });
            }
        }
        if self.show_label {
            let c = polygon_centroid(&self.outline).unwrap_or_else(|| self.outline.first().copied().unwrap_or(Vec2::ZERO));
            pieces.push(HvacPiece { layer: CEILING_LAYER, kind: label(c, self.units.to_units(self.label_height_mm), self.label_text()) });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(CEILING_APP, CEILING_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<CeilingParams> {
        let p: CeilingParams = xdata::from_xdata(CEILING_APP, CEILING_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The ceiling grid an object is, when it is a block reference carrying usable data.
pub fn ceiling_of(e: &crate::Entity) -> Option<CeilingParams> {
    match &e.kind {
        EntityKind::Insert(_) => CeilingParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Stair
// ---------------------------------------------------------------------------------------------------------------

/// A straight stair: the plan of one flight, its risers across the width, the walking line with an arrow towards the way up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StairParams {
    pub name: String,
    /// The width of the flight, mm.
    pub width_mm: f64,
    /// The depth of a tread, mm.
    pub going_mm: f64,
    /// The height the stair climbs, mm, from one floor to the next.
    pub rise_mm: f64,
    /// The highest a riser may be, mm: the number of risers is the smallest that keeps them under it.
    pub riser_max_mm: f64,
    /// The elevation of the lower floor, mm.
    #[serde(default)]
    pub elevation_mm: f64,
    /// Whether the arrow points the way up (along the flight) or down.
    #[serde(default = "yes")]
    pub up: bool,
    #[serde(default = "yes")]
    pub show_label: bool,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl StairParams {
    /// A stair 1 m wide climbing 3 m, with treads of 280 mm and risers of at most 180 mm.
    pub fn new(name: &str, units: HvacUnits) -> StairParams {
        StairParams {
            name: name.trim().to_string(),
            width_mm: 1000.0,
            going_mm: 280.0,
            rise_mm: 3000.0,
            riser_max_mm: 180.0,
            elevation_mm: 0.0,
            up: true,
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("stair", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !(ok(self.width_mm, 400.0, 5000.0)
            && ok(self.going_mm, 100.0, 600.0)
            && ok(self.rise_mm, 100.0, 20_000.0)
            && ok(self.riser_max_mm, 100.0, 250.0))
        {
            return Err(invalid(
                "A stair is 400 to 5000 mm wide, with treads of 100 to 600 mm, climbing 100 to 20000 mm with risers of at most 100 to 250 mm.",
            ));
        }
        if self.elevation_mm.abs() > 1.0e5 || !self.elevation_mm.is_finite() {
            return Err(invalid("A stair's elevation is not a usable number."));
        }
        if self.risers() > MAX_RISERS {
            return Err(invalid(format!("A stair has at most {MAX_RISERS} risers.")));
        }
        Ok(())
    }

    /// The number of risers: the fewest that keep each under the greatest height allowed.
    pub fn risers(&self) -> usize {
        ((self.rise_mm / self.riser_max_mm).ceil() as usize).max(1)
    }

    /// The height of one riser, mm.
    pub fn riser_mm(&self) -> f64 {
        self.rise_mm / self.risers() as f64
    }

    /// The length of the flight in plan, mm: one tread less than there are risers, the last riser being the floor above.
    pub fn run_mm(&self) -> f64 {
        (self.risers() - 1) as f64 * self.going_mm
    }

    /// Blondel's rule of the step: twice the riser and the tread add up to 600 to 650 mm for a comfortable stair.
    pub fn comfortable(&self) -> bool {
        (600.0..=650.0).contains(&(2.0 * self.riser_mm() + self.going_mm))
    }

    pub fn label_text(&self) -> String {
        format!("{} {} x {:.0} / {:.0} mm", self.name, self.risers(), self.riser_mm(), self.going_mm)
    }

    /// The flight along +x from its first riser, centred on the x axis: the outline, a line at each riser, the walking line and its arrow.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let u = |mm: f64| self.units.to_units(mm);
        let (len, half) = (u(self.run_mm()), u(self.width_mm / 2.0));
        let mut pieces = vec![HvacPiece { layer: STAIR_LAYER, kind: poly(&[(0.0, -half), (len, -half), (len, half), (0.0, half)], true) }];
        for k in 1..self.risers().saturating_sub(1) + 1 {
            let x = u(k as f64 * self.going_mm);
            if x < len - 1e-9 {
                pieces.push(HvacPiece { layer: STAIR_LAYER, kind: line((x, -half), (x, half)) });
            }
        }
        // The walking line from the middle of the first tread to the middle of the last, with an arrowhead at the end the way goes to.
        let (x0, x1) = (u(self.going_mm / 2.0).min(len / 2.0), (len - u(self.going_mm / 2.0)).max(len / 2.0));
        if x1 > x0 {
            let (to, dir) = if self.up { (x1, 1.0) } else { (x0, -1.0) };
            let head = u(150.0).min((x1 - x0) / 3.0);
            pieces.push(HvacPiece { layer: STAIR_LAYER, kind: line((x0, 0.0), (x1, 0.0)) });
            for side in [-1.0, 1.0] {
                pieces.push(HvacPiece { layer: STAIR_LAYER, kind: line((to, 0.0), (to - dir * head, side * head / 2.0)) });
            }
        }
        if self.show_label {
            let h = u(self.label_height_mm);
            pieces.push(HvacPiece { layer: STAIR_LAYER, kind: label(Vec2::new(len / 2.0, half + 1.1 * h), h, self.label_text()) });
            // "UP" or "DN" at the start of the walking line.
            pieces.push(HvacPiece {
                layer: STAIR_LAYER,
                kind: label(Vec2::new(u(self.going_mm / 2.0), -half - 1.1 * h), h, if self.up { "UP" } else { "DN" }.into()),
            });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(STAIR_APP, STAIR_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<StairParams> {
        let p: StairParams = xdata::from_xdata(STAIR_APP, STAIR_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The stair an object is, when it is a block reference carrying usable data.
pub fn stair_of(e: &crate::Entity) -> Option<StairParams> {
    match &e.kind {
        EntityKind::Insert(_) => StairParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Railing
// ---------------------------------------------------------------------------------------------------------------

/// A railing along a path: the rail, a thick line, and posts at an even spacing, with its height written on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RailParams {
    pub name: String,
    /// The path in the block's coordinates (drawing units), open, the first point at the origin.
    pub path: Vec<Vec2>,
    /// The height of the rail above the floor, mm (1000 for a stair, 1100 above 1 m of drop in many codes).
    pub height_mm: f64,
    /// The spacing of the posts along the path, mm, and their side.
    pub post_spacing_mm: f64,
    pub post_mm: f64,
    /// The width of the rail drawn in plan, mm.
    pub rail_mm: f64,
    #[serde(default)]
    pub elevation_mm: f64,
    #[serde(default = "yes")]
    pub show_label: bool,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl RailParams {
    /// A railing of 1000 mm with a post every 1200 mm along `path`.
    pub fn new(name: &str, path: Vec<Vec2>, units: HvacUnits) -> RailParams {
        RailParams {
            name: name.trim().to_string(),
            path,
            height_mm: 1000.0,
            post_spacing_mm: 1200.0,
            post_mm: 60.0,
            rail_mm: 40.0,
            elevation_mm: 0.0,
            show_label: true,
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_name("railing", &self.name)?;
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        if self.path.len() < 2
            || self.path.len() > MAX_DESIGN_VERTICES
            || self.path.iter().any(|p| !p.is_finite() || p.x.abs() > 1e12 || p.y.abs() > 1e12)
        {
            return Err(invalid(format!("A railing's path has 2 to {MAX_DESIGN_VERTICES} usable points.")));
        }
        let ok = |v: f64, lo: f64, hi: f64| v.is_finite() && (lo..=hi).contains(&v);
        if !(ok(self.height_mm, 300.0, 3000.0)
            && ok(self.post_spacing_mm, 200.0, 5000.0)
            && ok(self.post_mm, 10.0, 300.0)
            && ok(self.rail_mm, 5.0, 300.0))
        {
            return Err(invalid(
                "A railing is 300 to 3000 mm high, with posts 200 to 5000 mm apart, 10 to 300 mm across, and a rail of 5 to 300 mm.",
            ));
        }
        if !(self.elevation_mm.is_finite() && self.elevation_mm.abs() <= 1.0e5) {
            return Err(invalid("A railing's elevation is not a usable number."));
        }
        if self.length_mm() <= 0.0 || self.posts().len() > MAX_POSTS {
            return Err(invalid(format!("A railing has a length and at most {MAX_POSTS} posts.")));
        }
        Ok(())
    }

    /// The length of the path, mm.
    pub fn length_mm(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>() * self.units.mm_per_unit
    }

    /// The places of the posts, in the block's coordinates: one at each end and one every `post_spacing_mm` along the path, spaced evenly.
    pub fn posts(&self) -> Vec<Vec2> {
        let total = self.length_mm();
        if !(total.is_finite() && total > 0.0) || self.post_spacing_mm <= 0.0 {
            return Vec::new();
        }
        let n = ((total / self.post_spacing_mm).ceil() as usize).max(1);
        if n > MAX_POSTS {
            return vec![Vec2::ZERO; MAX_POSTS + 1];
        }
        (0..=n).filter_map(|k| self.at_mm(total * k as f64 / n as f64)).collect()
    }

    /// The point `s` millimetres along the path.
    fn at_mm(&self, s: f64) -> Option<Vec2> {
        let mut left = s / self.units.mm_per_unit;
        for w in self.path.windows(2) {
            let len = w[0].dist(w[1]);
            if left <= len + 1e-9 {
                return Some(if len > 1e-12 { w[0] + (w[1] - w[0]) * (left / len).clamp(0.0, 1.0) } else { w[0] });
            }
            left -= len;
        }
        self.path.last().copied()
    }

    pub fn label_text(&self) -> String {
        format!("{} h={:.2} m", self.name, self.height_mm / 1000.0)
    }

    /// The rail as a polyline of the rail's width, a square at each post, and the label.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let u = |mm: f64| self.units.to_units(mm);
        let vertices: Vec<PolyVertex> = self.path.iter().map(|p| PolyVertex { p: *p, bulge: 0.0, start_width: 0.0, end_width: 0.0 }).collect();
        let mut pieces = vec![HvacPiece {
            layer: RAIL_LAYER,
            kind: EntityKind::LwPolyline(LwPolyline { vertices, closed: false, const_width: u(self.rail_mm), elevation: 0.0, plinegen: false }),
        }];
        let half = u(self.post_mm / 2.0);
        for p in self.posts() {
            pieces.push(HvacPiece {
                layer: RAIL_LAYER,
                kind: poly(&[(p.x - half, p.y - half), (p.x + half, p.y - half), (p.x + half, p.y + half), (p.x - half, p.y + half)], true),
            });
        }
        if self.show_label
            && let Some(mid) = self.at_mm(self.length_mm() / 2.0)
        {
            let h = u(self.label_height_mm);
            pieces.push(HvacPiece { layer: RAIL_LAYER, kind: label(mid + Vec2::new(0.0, 1.5 * h), h, self.label_text()) });
        }
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(RAIL_APP, RAIL_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<RailParams> {
        let p: RailParams = xdata::from_xdata(RAIL_APP, RAIL_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

/// The railing an object is, when it is a block reference carrying usable data.
pub fn rail_of(e: &crate::Entity) -> Option<RailParams> {
    match &e.kind {
        EntityKind::Insert(_) => RailParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour of a design layer.
pub(super) fn design_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    use cadcraft_color::{Color, Rgb};
    match name {
        CEILING_LAYER => Some(Color::True(Rgb(0x70, 0x90, 0xb0))),
        CEILING_GRID_LAYER => Some(Color::True(Rgb(0x60, 0x68, 0x78))),
        STAIR_LAYER => Some(Color::True(Rgb(0xb0, 0x90, 0x60))),
        RAIL_LAYER => Some(Color::True(Rgb(0x90, 0x70, 0x50))),
        _ => None,
    }
}

impl crate::Drawing {
    /// The outline of space `h` (a room of the HVAC tab) in the drawing's coordinates.
    pub fn space_outline_world(&self, h: crate::Handle) -> Option<Vec<Vec2>> {
        let e = self.entity(h)?;
        match (&e.kind, super::space_of(e)) {
            (EntityKind::Insert(i), Some(sp)) => Some(sp.outline.iter().map(|p| block_to_world(i, *p)).collect()),
            _ => None,
        }
    }

    /// Adds ceiling grid `p` to `space`, its outline given in drawing coordinates.
    pub fn add_ceiling(&mut self, space: &crate::Space, mut common: crate::Common, mut p: CeilingParams) -> Result<crate::Handle> {
        let at = p.outline.first().copied().ok_or_else(|| invalid("A ceiling needs an outline."))?;
        for v in p.outline.iter_mut() {
            *v -= at;
        }
        p.origin -= at;
        let pieces = p.generate()?;
        common.xdata = xdata::replace(CEILING_APP, &common.xdata, p.to_xdata()?);
        common.layer = CEILING_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(CEILING_LAYER);
        self.ensure_layer(CEILING_GRID_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives ceiling grid `h` the parameters `p` (in the block's coordinates), redrawing it in place.
    pub fn regenerate_ceiling(&mut self, h: crate::Handle, p: &CeilingParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(CEILING_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        self.ensure_layer(CEILING_LAYER);
        self.ensure_layer(CEILING_GRID_LAYER);
        Ok(())
    }

    /// The centre of the ceiling tile that holds `p` (drawing coordinates), in the nearest ceiling grid of `space` that has `p` in a tile whose centre
    /// is in its room within `tol` of `p`: where a diffuser or a luminaire of a false ceiling goes.
    pub fn ceiling_tile_centre(&self, space: &crate::Space, p: Vec2) -> Option<Vec2> {
        let store = self.space(space)?;
        let mut best: Option<(f64, Vec2)> = None;
        for e in store.iter() {
            let (EntityKind::Insert(i), Some(c)) = (&e.kind, ceiling_of(e)) else { continue };
            let Some(local) = world_to_block(i, p) else { continue };
            if !point_in_polygon(&c.outline, local) {
                continue;
            }
            if let Some(centre) = c.tile_centre(local) {
                let world = block_to_world(i, centre);
                if best.is_none_or(|(d, _)| world.dist(p) < d) {
                    best = Some((world.dist(p), world));
                }
            }
        }
        best.map(|(_, c)| c)
    }

    /// Adds a stair at `at` (the middle of its first riser), its flight turned `rotation` radians.
    pub fn add_stair(&mut self, space: &crate::Space, mut common: crate::Common, p: StairParams, at: Vec2, rotation: f64) -> Result<crate::Handle> {
        if !at.is_finite() || !rotation.is_finite() {
            return Err(invalid("A stair needs a usable place."));
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(STAIR_APP, &common.xdata, p.to_xdata()?);
        common.layer = STAIR_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(STAIR_LAYER);
        let insert = crate::Insert { block, insert: v3(at), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives stair `h` the parameters `p`, redrawing it in place.
    pub fn regenerate_stair(&mut self, h: crate::Handle, p: &StairParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(STAIR_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        self.ensure_layer(STAIR_LAYER);
        Ok(())
    }

    /// Adds a railing along `path` (drawing coordinates).
    pub fn add_rail(&mut self, space: &crate::Space, mut common: crate::Common, mut p: RailParams) -> Result<crate::Handle> {
        let at = p.path.first().copied().ok_or_else(|| invalid("A railing needs a path."))?;
        for v in p.path.iter_mut() {
            *v -= at;
        }
        let pieces = p.generate()?;
        common.xdata = xdata::replace(RAIL_APP, &common.xdata, p.to_xdata()?);
        common.layer = RAIL_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(RAIL_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives railing `h` the parameters `p` (path in the block's coordinates), redrawing it in place.
    pub fn regenerate_rail(&mut self, h: crate::Handle, p: &RailParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(RAIL_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        self.ensure_layer(RAIL_LAYER);
        Ok(())
    }
}
