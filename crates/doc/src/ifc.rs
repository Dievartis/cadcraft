//! IFC export: the drawing's building objects written as an IFC4 file (ISO 16739-1, STEP physical file, the schema `IFC4`), for the programs of the BIM
//! family to open and check. What is written: the walls (one extruded footprint per straight piece of their centre line, arcs as short pieces), the
//! slabs (with their holes), the columns and beams, the doors and windows, the spaces, the ducts, the pipes, the cables, the air handling units, the
//! equipment, the air terminals, the furniture, the sanitary fixtures and the electrical devices, each a swept solid (an extruded outline, or a profile
//! along a run) in millimetres, with a property set that says its layer and its handle. They sit on one building storey of one building on one site.
//!
//! What is not: the roofs (their planes are not solids here), the junctions of walls (each piece is written as it is, overlapping at the corners), the
//! openings as voids in the walls (the doors and windows are written in their place, the wall is not cut), the slopes of slabs and ducts, materials and
//! the other property sets. The file is written from the public description of the schema; it was not checked by a viewer, and says so in its header.
//!
//! The entity numbers, the coordinates and the identifiers are made from the drawing alone: the same drawing gives the same file.

use std::fmt::Write as _;

use cadcraft_geom::Vec2;

use crate::ventilation::{HvacUnits, ahu_of, air_terminal_of, device_of, equipment_of, furniture_of, sanitary_of, slab_of, space_of, struct_of};
use crate::{Drawing, EntityKind, Handle, OpeningKind, Result, opening_of, wall_of};

/// Most building elements one file holds.
const MAX_ELEMENTS: usize = 200_000;
/// The altitude a terminal or a run is given when nothing says where it is, mm (as a section puts it), and the height of a space, mm.
const DEFAULT_ALTITUDE_MM: f64 = 2500.0;
const DEFAULT_SPACE_HEIGHT_MM: f64 = 2500.0;

/// The IFC alphabet of the 22-character identifiers.
const GUID_CHARS: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// The 22-character IFC identifier made from 128 bits.
fn guid_of(bits: u128) -> String {
    let mut out = String::with_capacity(22);
    // The first character holds the two highest bits, the 21 others six bits each.
    out.push(char::from(GUID_CHARS.get(((bits >> 126) & 0x3) as usize).copied().unwrap_or(b'0')));
    for k in (0..21).rev() {
        out.push(char::from(GUID_CHARS.get(((bits >> (6 * k)) & 0x3f) as usize).copied().unwrap_or(b'0')));
    }
    out
}

/// A repeatable 128-bit number from a few words (two rounds of FNV-1a with different seeds).
fn bits_of(parts: &[&str], n: u64) -> u128 {
    let round = |seed: u64| {
        let mut h = seed;
        for p in parts.iter().map(|s| s.as_bytes()).chain(std::iter::once(&n.to_le_bytes()[..])) {
            for b in p {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            h ^= 0xff;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    };
    (u128::from(round(0xcbf2_9ce4_8422_2325)) << 64) | u128::from(round(0x8422_2325_cbf2_9ce4))
}

/// `text` as a STEP string: in quotes, a quote doubled, what is not ASCII as `\X2\…\X0\` (UTF-16, four hex digits each).
fn step_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    let mut wide = false;
    let mut units = [0u16; 2];
    for c in text.chars().filter(|c| !c.is_control()) {
        if c.is_ascii() {
            if wide {
                out.push_str("\\X0\\");
                wide = false;
            }
            if c == '\'' {
                out.push('\'');
            }
            if c == '\\' {
                out.push('\\');
            }
            out.push(c);
        } else {
            if !wide {
                out.push_str("\\X2\\");
                wide = true;
            }
            for u in c.encode_utf16(&mut units) {
                let _ = write!(out, "{u:04X}");
            }
        }
    }
    if wide {
        out.push_str("\\X0\\");
    }
    out.push('\'');
    out
}

/// A real number as STEP writes it: always with a decimal point, never an exponent.
fn real(v: f64) -> String {
    if !v.is_finite() {
        return "0.".into();
    }
    let t = format!("{v:.4}");
    let t = t.trim_end_matches('0');
    let t = if t == "-0." { "0." } else { t };
    t.to_string()
}

/// The numbering of the entities and the text of the file.
struct Step {
    data: String,
    n: usize,
}

impl Step {
    fn add(&mut self, entity: String) -> usize {
        self.n += 1;
        let _ = writeln!(self.data, "#{}={entity};", self.n);
        self.n
    }

    fn list(ids: &[usize]) -> String {
        ids.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",")
    }

    fn point3(&mut self, x: f64, y: f64, z: f64) -> usize {
        self.add(format!("IFCCARTESIANPOINT(({},{},{}))", real(x), real(y), real(z)))
    }

    fn point2(&mut self, x: f64, y: f64) -> usize {
        self.add(format!("IFCCARTESIANPOINT(({},{}))", real(x), real(y)))
    }

    fn direction(&mut self, x: f64, y: f64, z: f64) -> usize {
        self.add(format!("IFCDIRECTION(({},{},{}))", real(x), real(y), real(z)))
    }

    /// A placement at a point; with an axis (the local Z) and a reference direction (the local X) when they are given.
    fn axis3(&mut self, at: (f64, f64, f64), axis: Option<(f64, f64, f64)>, reference: Option<(f64, f64, f64)>) -> usize {
        let p = self.point3(at.0, at.1, at.2);
        let a = axis.map_or("$".to_string(), |d| format!("#{}", self.direction(d.0, d.1, d.2)));
        let r = reference.map_or("$".to_string(), |d| format!("#{}", self.direction(d.0, d.1, d.2)));
        self.add(format!("IFCAXIS2PLACEMENT3D(#{p},{a},{r})"))
    }

    /// An extruded outline: the polygon (millimetres, closed by this) from `z0` up by `height`, with holes when it has some.
    fn extruded_outline(&mut self, outline: &[Vec2], holes: &[Vec<Vec2>], z0: f64, height: f64) -> Option<usize> {
        if outline.len() < 3 || !(height.is_finite() && height > 0.0) {
            return None;
        }
        let ring = |s: &mut Step, pts: &[Vec2]| {
            let mut ids: Vec<usize> = pts.iter().map(|p| s.point2(p.x, p.y)).collect();
            if let Some(first) = ids.first().copied() {
                ids.push(first);
            }
            s.add(format!("IFCPOLYLINE(({}))", Step::list(&ids)))
        };
        let outer = ring(self, outline);
        let profile = if holes.iter().any(|h| h.len() >= 3) {
            let inner: Vec<usize> = holes.iter().filter(|h| h.len() >= 3).map(|h| ring(self, h)).collect();
            self.add(format!("IFCARBITRARYPROFILEDEFWITHVOIDS(.AREA.,$,#{outer},({}))", Step::list(&inner)))
        } else {
            self.add(format!("IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{outer})"))
        };
        let place = self.axis3((0.0, 0.0, z0), None, None);
        let up = self.direction(0.0, 0.0, 1.0);
        Some(self.add(format!("IFCEXTRUDEDAREASOLID(#{profile},#{place},#{up},{})", real(height))))
    }

    /// A profile along a run from `a` to `b` (millimetres, the centre of the section at height `z`): round, or `across` wide and `tall` high.
    fn run_solid(&mut self, a: Vec2, b: Vec2, z: f64, round: Option<f64>, rect: (f64, f64)) -> Option<usize> {
        let length = a.dist(b);
        if !(length.is_finite() && length > 1.0) {
            return None;
        }
        let d = (b - a) / length;
        let origin = self.point2(0.0, 0.0);
        let at2 = self.add(format!("IFCAXIS2PLACEMENT2D(#{origin},$)"));
        let profile = match round {
            Some(diameter) if diameter > 0.0 => self.add(format!("IFCCIRCLEPROFILEDEF(.AREA.,$,#{at2},{})", real(diameter / 2.0))),
            _ if rect.0 > 0.0 && rect.1 > 0.0 => {
                // The section's X is vertical, so its X dimension is the height and its Y the width.
                self.add(format!("IFCRECTANGLEPROFILEDEF(.AREA.,$,#{at2},{},{})", real(rect.1), real(rect.0)))
            }
            _ => return None,
        };
        let place = self.axis3((a.x, a.y, z), Some((d.x, d.y, 0.0)), Some((0.0, 0.0, 1.0)));
        let along = self.direction(0.0, 0.0, 1.0);
        Some(self.add(format!("IFCEXTRUDEDAREASOLID(#{profile},#{place},#{along},{})", real(length))))
    }
}

/// A rectangle `across` long (along `angle`) and `deep` wide, centred at `c`, as four corners.
fn rectangle(c: Vec2, angle: f64, across: f64, deep: f64) -> Vec<Vec2> {
    let (s, k) = angle.sin_cos();
    let at = |x: f64, y: f64| c + Vec2::new(x * k - y * s, x * s + y * k);
    let (hx, hy) = (across / 2.0, deep / 2.0);
    vec![at(-hx, -hy), at(hx, -hy), at(hx, hy), at(-hx, hy)]
}

/// The footprints of a wall in the drawing, one quadrilateral for each straight piece of its centre line, between the faces it has (drawing units).
pub(crate) fn wall_footprints(wall: &crate::WallParams, at: crate::Placement) -> Vec<Vec<Vec2>> {
    let path: Vec<Vec2> = wall.centreline().iter().map(|p| at.to_world(*p)).collect();
    let bounds = wall.boundaries();
    let (Some(&o0), Some(&o1)) = (bounds.first(), bounds.last()) else { return Vec::new() };
    // A mirrored wall has its left and its right turned round.
    let side = if at.scale.x * at.scale.y < 0.0 { -1.0 } else { 1.0 };
    let n = path.len();
    let pieces = if wall.closed { n } else { n.saturating_sub(1) };
    (0..pieces)
        .filter_map(|k| {
            let (a, b) = (*path.get(k)?, *path.get((k + 1) % n)?);
            let len = a.dist(b);
            if len <= 1e-9 {
                return None;
            }
            let d = (b - a) / len;
            let left = Vec2::new(-d.y, d.x) * side;
            Some(vec![a + left * o0, b + left * o0, b + left * o1, a + left * o1])
        })
        .collect()
}

/// What the entities of one file are written with: the numbering, the shared placements and the body context.
struct Ctx {
    s: Step,
    mm: f64,
    world: usize,
    body: usize,
    storey_place: usize,
}

impl Ctx {
    /// A building element made of `solids`: the entity, its placement and its shape, and its property set (layer and handle). `tail` is what comes after the
    /// tag, `after` what some classes (doors, windows) have beyond it.
    fn element(&mut self, (class, tail, after): (&str, &str, &str), name: &str, h: Handle, layer: &str, solids: &[usize]) -> Option<usize> {
        if solids.is_empty() {
            return None;
        }
        let rep = self.s.add(format!("IFCSHAPEREPRESENTATION(#{},'Body','SweptSolid',({}))", self.body, Step::list(solids)));
        let shape = self.s.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"));
        let place = self.s.add(format!("IFCLOCALPLACEMENT(#{},#{})", self.storey_place, self.world));
        let id = guid_of(bits_of(&[class, name, layer], h.0));
        let e = self.s.add(format!("{class}('{id}',$,{},$,$,#{place},#{shape},$,{tail}{after})", step_text(name)));
        let layer_p = self.s.add(format!("IFCPROPERTYSINGLEVALUE('Layer',$,IFCLABEL({}),$)", step_text(layer)));
        let handle_p = self.s.add(format!("IFCPROPERTYSINGLEVALUE('Handle',$,IFCLABEL({}),$)", step_text(&h.hex())));
        let pset = self.s.add(format!("IFCPROPERTYSET('{}',$,'Pset_CADCRAFT',$,(#{layer_p},#{handle_p}))", guid_of(bits_of(&["pset", class], h.0))));
        self.s.add(format!("IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#{e}),#{pset})", guid_of(bits_of(&["rel", class], h.0))));
        Some(e)
    }

    /// A box on the plan: `across` long along `angle` and `deep` wide, centred at `at` (millimetres), from `z0` up by `tall`.
    fn boxed(&mut self, at: Vec2, angle: f64, across: f64, deep: f64, z0: f64, tall: f64) -> Vec<usize> {
        self.s.extruded_outline(&rectangle(at, angle, across, deep), &[], z0, tall).into_iter().collect()
    }
}

impl Drawing {
    /// The drawing's building objects as an IFC4 file (see the module), `name` being the file's name as the header says it.
    pub fn to_ifc(&self, name: &str) -> Result<String> {
        let mut s = Step { data: String::new(), n: 0 };
        // The context and the units: millimetres, square metres, cubic metres, radians.
        let world = s.axis3((0.0, 0.0, 0.0), None, None);
        let context = s.add(format!("IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#{world},$)"));
        let body = s.add(format!("IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{context},$,.MODEL_VIEW.,$)"));
        let units: Vec<usize> = [
            "IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)",
            "IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)",
            "IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)",
            "IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)",
        ]
        .iter()
        .map(|u| s.add((*u).to_string()))
        .collect();
        let assignment = s.add(format!("IFCUNITASSIGNMENT(({}))", Step::list(&units)));
        let stem = name.rsplit(['/', '\\']).next().unwrap_or(name).trim_end_matches(".ifc").to_string();
        let project =
            s.add(format!("IFCPROJECT('{}',$,{},$,$,$,$,(#{context}),#{assignment})", guid_of(bits_of(&["project", &stem], 0)), step_text(&stem)));
        let site_place = s.add(format!("IFCLOCALPLACEMENT($,#{world})"));
        let site = s.add(format!("IFCSITE('{}',$,'Site',$,$,#{site_place},$,$,.ELEMENT.,$,$,$,$,$)", guid_of(bits_of(&["site"], 0))));
        let building_place = s.add(format!("IFCLOCALPLACEMENT(#{site_place},#{world})"));
        let building = s.add(format!("IFCBUILDING('{}',$,'Building',$,$,#{building_place},$,$,.ELEMENT.,$,$,$)", guid_of(bits_of(&["building"], 0))));
        let storey_place = s.add(format!("IFCLOCALPLACEMENT(#{building_place},#{world})"));
        let storey = s.add(format!("IFCBUILDINGSTOREY('{}',$,'Level 0',$,$,#{storey_place},$,$,.ELEMENT.,0.)", guid_of(bits_of(&["storey"], 0))));
        for (k, (from, to)) in [(project, site), (site, building), (building, storey)].into_iter().enumerate() {
            s.add(format!("IFCRELAGGREGATES('{}',$,$,$,#{from},(#{to}))", guid_of(bits_of(&["aggregates"], k as u64))));
        }
        let mut cx = Ctx { s, mm: HvacUnits::of(self).mm_per_unit, world, body, storey_place };
        let mut elements: Vec<usize> = Vec::new();
        for e in self.model.iter() {
            if elements.len() >= MAX_ELEMENTS {
                break;
            }
            if let Some(id) = self.ifc_element(&mut cx, e) {
                elements.push(id);
            }
        }
        if !elements.is_empty() {
            cx.s.add(format!(
                "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,({}),#{storey})",
                guid_of(bits_of(&["contained"], 0)),
                Step::list(&elements)
            ));
        }
        let mut out = String::with_capacity(cx.s.data.len() + 512);
        out.push_str("ISO-10303-21;\nHEADER;\n");
        out.push_str(
            "FILE_DESCRIPTION(('ViewDefinition [CoordinationView]','Written from a description of the schema, not checked by a viewer'),'2;1');\n",
        );
        let _ = writeln!(out, "FILE_NAME({},'2000-01-01T00:00:00',('CADCraft'),(''),'CADCraft','CADCraft','');", step_text(name));
        out.push_str("FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n");
        out.push_str(&cx.s.data);
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        Ok(out)
    }

    /// The IFC element an object of the drawing is, written to `cx`: its entity, when it is one the export knows.
    fn ifc_element(&self, cx: &mut Ctx, e: &crate::Entity) -> Option<usize> {
        let EntityKind::Insert(i) = &e.kind else { return None };
        let (h, layer, mm) = (e.handle, e.common.layer.as_str(), cx.mm);
        let (at, rot) = (Vec2::new(i.insert.x, i.insert.y) * mm, i.rotation);
        if let Some(w) = wall_of(e) {
            let place = crate::Placement {
                insert: Vec2::new(i.insert.x, i.insert.y),
                rotation: i.rotation,
                scale: Vec2::new(i.scale.x.signum(), i.scale.y.signum()),
            };
            let solids: Vec<usize> = wall_footprints(&w, place)
                .iter()
                .filter_map(|f| cx.s.extruded_outline(&f.iter().map(|p| *p * mm).collect::<Vec<_>>(), &[], 0.0, w.height))
                .collect();
            cx.element(("IFCWALL", "$", ""), &w.style.name, h, layer, &solids)
        } else if let Some(sl) = slab_of(e) {
            let world_pts = |pts: &[Vec2]| -> Vec<Vec2> { pts.iter().map(|p| world_point(i, *p) * mm).collect() };
            let holes: Vec<Vec<Vec2>> = sl.holes.iter().map(|hole| world_pts(hole)).collect();
            let solids: Vec<usize> =
                cx.s.extruded_outline(&world_pts(&sl.outline), &holes, sl.elevation_mm - sl.thickness_mm, sl.thickness_mm).into_iter().collect();
            cx.element(("IFCSLAB", "$", ""), &sl.name, h, layer, &solids)
        } else if let Some(st) = struct_of(e) {
            let sec = st.profile()?;
            let u = |v: f64| st.units.to_units(v);
            let column = st.kind == crate::ventilation::StructKind::Column;
            let local: Vec<Vec2> = if column {
                let (hx, hy) = (sec.b_mm / 2.0 / mm, sec.h_mm / 2.0 / mm);
                vec![Vec2::new(-hx, -hy), Vec2::new(hx, -hy), Vec2::new(hx, hy), Vec2::new(-hx, hy)]
            } else {
                let (x0, x1, half) = (u(st.trim_start_mm), st.length - u(st.trim_end_mm), sec.b_mm / 2.0 / mm);
                vec![Vec2::new(x0, -half), Vec2::new(x1, -half), Vec2::new(x1, half), Vec2::new(x0, half)]
            };
            let pts: Vec<Vec2> = local.iter().map(|p| world_point(i, *p) * mm).collect();
            let (class, height) = if column { ("IFCCOLUMN", st.height_mm) } else { ("IFCBEAM", sec.h_mm) };
            let solids: Vec<usize> = cx.s.extruded_outline(&pts, &[], st.elevation_mm, height).into_iter().collect();
            cx.element((class, "$", ""), &st.name, h, layer, &solids)
        } else if let Some(op) = opening_of(e) {
            let solids = cx.boxed(at, rot, op.width_mm, 150.0, op.sill_mm, op.height_mm);
            // A window and a door each take their own optional attributes after the tag.
            let tail = format!("{},{}", real(op.height_mm), real(op.width_mm));
            let class = if op.kind == OpeningKind::Window { "IFCWINDOW" } else { "IFCDOOR" };
            cx.element((class, &tail, ",$,$,$"), if op.kind == OpeningKind::Window { "Window" } else { "Door" }, h, layer, &solids)
        } else if let Some((p, a, b)) = self.duct_ends(h) {
            let (w, hgt) = match p.shape {
                crate::ventilation::DuctShape::Round => (p.diameter_mm, p.diameter_mm),
                crate::ventilation::DuctShape::Rectangular => (p.width_mm, p.height_mm),
            };
            let z = p.altitude_mm().unwrap_or(DEFAULT_ALTITUDE_MM) + hgt / 2.0;
            let round = (p.shape == crate::ventilation::DuctShape::Round).then_some(p.diameter_mm);
            let solids: Vec<usize> = cx.s.run_solid(a * mm, b * mm, z, round, (w, hgt)).into_iter().collect();
            cx.element(("IFCDUCTSEGMENT", "$", ""), &format!("Duct {}", p.size_label()), h, layer, &solids)
        } else if let Some((p, a, b)) = self.pipe_ends(h) {
            let diameter = p.dn_mm + 2.0 * p.insulation_mm;
            let z = p.level_mm.unwrap_or(DEFAULT_ALTITUDE_MM) + diameter / 2.0;
            let solids: Vec<usize> = cx.s.run_solid(a * mm, b * mm, z, Some(diameter), (0.0, 0.0)).into_iter().collect();
            cx.element(("IFCPIPESEGMENT", "$", ""), p.service.name(), h, layer, &solids)
        } else if let Some((p, a, b)) = self.cable_ends(h) {
            let solids: Vec<usize> = cx.s.run_solid(a * mm, b * mm, DEFAULT_ALTITUDE_MM + 100.0, Some(20.0), (0.0, 0.0)).into_iter().collect();
            cx.element(("IFCCABLESEGMENT", "$", ""), &p.circuit, h, layer, &solids)
        } else {
            self.ifc_equipment(cx, e, h, layer, (at, rot))
        }
    }

    /// The equipment, units, terminals, furniture, fixtures, devices and spaces among the objects.
    fn ifc_equipment(&self, cx: &mut Ctx, e: &crate::Entity, h: Handle, layer: &str, (at, rot): (Vec2, f64)) -> Option<usize> {
        if let Some(q) = equipment_of(e) {
            let solids = cx.boxed(at, rot, q.length_mm, q.width_mm, q.bottom_mm.unwrap_or(0.0), q.height_mm);
            cx.element((if q.kind.is_rack() { "IFCBUILDINGELEMENTPROXY" } else { "IFCUNITARYEQUIPMENT" }, "$", ""), &q.name, h, layer, &solids)
        } else if let Some(a) = ahu_of(e) {
            let solids = cx.boxed(at, rot, a.width_mm, a.depth_mm, a.bottom_mm.unwrap_or(0.0), a.height_mm.unwrap_or(1000.0));
            cx.element(("IFCUNITARYEQUIPMENT", "$", ""), &a.name, h, layer, &solids)
        } else if let Some(t) = air_terminal_of(e) {
            let solids = cx.boxed(at, rot, t.model.width_mm, t.model.depth_mm, t.bottom_mm.unwrap_or(DEFAULT_ALTITUDE_MM), 100.0);
            cx.element(("IFCAIRTERMINAL", "$", ""), &t.model.name, h, layer, &solids)
        } else if let Some(f) = furniture_of(e) {
            let solids = cx.boxed(at, rot, f.width_mm, f.depth_mm, f.bottom_mm.unwrap_or(0.0), f.height_mm);
            cx.element(("IFCFURNITURE", "$", ""), &f.name, h, layer, &solids)
        } else if let Some(f) = sanitary_of(e) {
            let solids = cx.boxed(at, rot, f.width_mm, f.depth_mm, f.bottom_mm.unwrap_or(0.0), 800.0);
            cx.element(("IFCSANITARYTERMINAL", "$", ""), &f.name, h, layer, &solids)
        } else if let Some(dv) = device_of(e) {
            let (class, w, deep, tall) = if dv.kind.is_light() {
                ("IFCLIGHTFIXTURE", 200.0, 200.0, 50.0)
            } else if matches!(dv.kind, crate::ventilation::DeviceKind::Panel) {
                (
                    "IFCELECTRICDISTRIBUTIONBOARD",
                    dv.board_width_mm.unwrap_or(400.0),
                    dv.board_depth_mm.unwrap_or(150.0),
                    dv.board_height_mm.unwrap_or(600.0),
                )
            } else {
                ("IFCOUTLET", 80.0, 80.0, 80.0)
            };
            let solids = cx.boxed(at, rot, w, deep, dv.height_mm, tall);
            cx.element((class, "$", ""), &dv.name, h, layer, &solids)
        } else if space_of(e).is_some() {
            let (sp, outline) = self.space_outline(h)?;
            let pts: Vec<Vec2> = outline.iter().map(|p| *p * cx.mm).collect();
            let solids: Vec<usize> = cx.s.extruded_outline(&pts, &[], 0.0, DEFAULT_SPACE_HEIGHT_MM).into_iter().collect();
            if solids.is_empty() {
                return None;
            }
            // A space has no tag: its long name, its composition and its predefined type come where the others have theirs.
            let rep = cx.s.add(format!("IFCSHAPEREPRESENTATION(#{},'Body','SweptSolid',({}))", cx.body, Step::list(&solids)));
            let shape = cx.s.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"));
            let place = cx.s.add(format!("IFCLOCALPLACEMENT(#{},#{})", cx.storey_place, cx.world));
            let id = guid_of(bits_of(&["IFCSPACE", &sp.name], h.0));
            Some(cx.s.add(format!("IFCSPACE('{id}',$,{},$,$,#{place},#{shape},$,.ELEMENT.,$,0.)", step_text(&sp.name))))
        } else {
            None
        }
    }
}

/// A point of a block reference's block in the drawing (the reference's position, turn and scale applied).
fn world_point(i: &crate::Insert, p: Vec2) -> Vec2 {
    let (s, c) = i.rotation.sin_cos();
    let (x, y) = (p.x * i.scale.x, p.y * i.scale.y);
    Vec2::new(i.insert.x + x * c - y * s, i.insert.y + x * s + y * c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_has_22_characters_of_the_ifc_alphabet_and_is_repeatable() {
        let g = guid_of(bits_of(&["wall", "x"], 7));
        assert_eq!(g.len(), 22);
        assert!(g.bytes().all(|b| GUID_CHARS.contains(&b)), "{g}");
        assert_eq!(g, guid_of(bits_of(&["wall", "x"], 7)));
        assert_ne!(g, guid_of(bits_of(&["wall", "x"], 8)));
        // The first character holds two bits only: it is one of the first four.
        assert!(b"0123".contains(&g.as_bytes()[0]), "{g}");
        assert_eq!(guid_of(0).len(), 22);
        assert_eq!(guid_of(u128::MAX).len(), 22);
    }

    #[test]
    fn text_and_numbers_are_written_as_step_wants_them() {
        assert_eq!(step_text("Mur 'A'"), "'Mur ''A'''");
        assert_eq!(step_text("é"), "'\\X2\\00E9\\X0\\'");
        assert_eq!(step_text("a\\b"), "'a\\\\b'");
        assert_eq!(real(1000.0), "1000.");
        assert_eq!(real(-0.00001), "0.");
        assert_eq!(real(2.5), "2.5");
        assert_eq!(real(f64::NAN), "0.");
        assert!(!real(1.0e-9).contains('e'));
    }

    #[test]
    fn a_wall_is_written_between_its_faces_on_the_side_the_drawing_has_them() {
        let mut d = Drawing::new_metric();
        let style = crate::standard_wall_styles().into_iter().next().unwrap();
        let thick = style.thickness();
        let p = crate::WallParams {
            style,
            path: vec![Vec2::ZERO, Vec2::new(5000.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            justification: crate::WallJustification::Left,
            height: 2500.0,
            units: crate::WallUnits::default(),
            joints: Default::default(),
            start_free: false,
            end_free: false,
        };
        let h = d.add_wall(&crate::Space::Model, crate::Common::default(), p.clone()).unwrap();
        let wall = wall_of(d.entity(h).unwrap()).unwrap();
        let at = crate::Placement { insert: Vec2::ZERO, rotation: 0.0, scale: Vec2::new(1.0, 1.0) };
        let feet = wall_footprints(&wall, at);
        assert_eq!(feet.len(), 1);
        let ys: Vec<f64> = feet[0].iter().map(|q| q.y).collect();
        let (lo, hi) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
        // The drawing's lines of the wall span the same band across it.
        let EntityKind::Insert(i) = &d.entity(h).unwrap().kind else { panic!("a block reference") };
        let lines: Vec<f64> = d
            .block(&i.block)
            .unwrap()
            .entities
            .iter()
            .filter_map(|e| if let EntityKind::Line(l) = &e.kind { Some([l.a.y, l.b.y]) } else { None })
            .flatten()
            .collect();
        let (blo, bhi) = (lines.iter().cloned().fold(f64::MAX, f64::min), lines.iter().cloned().fold(f64::MIN, f64::max));
        assert!((lo - blo).abs() < 1e-6 && (hi - bhi).abs() < 1e-6, "{lo} {hi} against {blo} {bhi}");
        assert!((hi - lo - thick).abs() < 1e-6);
    }
}
