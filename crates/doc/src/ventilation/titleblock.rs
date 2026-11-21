//! The title block of a sheet: a column of cells (project, the parties with their logos, a picture of the project, the sheet's title, a table
//! of revisions and the code of the sheet). Like the other plan objects it is a block reference to an anonymous block, its data travelling as
//! extended data of its own registered application. Every size is in millimetres on paper and scales with the width of the column.

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::part::{line, poly};
use super::{HvacPiece, HvacUnits, check_name, invalid, v3, xdata};
use crate::{EntityKind, HAlign, Image, RawTag, Result, Text, VAlign};

/// The registered application name of the data of a title block.
pub const TITLEBLOCK_APP: &str = "CADCRAFT_TITLEBLOCK";
/// The layer of a title block.
pub const TITLEBLOCK_LAYER: &str = "A-ANNO-TTLB";
const TITLEBLOCK_FORMAT: &str = "TTLB1";
/// The start of the key a title block's pictures are kept under in the drawing's rasters (they are saved with the drawing).
pub const TITLEBLOCK_IMAGE_PREFIX: &str = "titleblock-image:";
/// Most parties, revisions and codes of a title block, and most characters of one text.
pub const MAX_PARTIES: usize = 12;
pub const MAX_REVISIONS: usize = 40;
pub const MAX_CODES: usize = 16;
const MAX_TEXT_CHARS: usize = 200;
/// The width of the column the sizes below are drawn for, mm; the others scale with `width / REFERENCE_WIDTH_MM`.
pub const REFERENCE_WIDTH_MM: f64 = 190.0;
/// The margin between the edge of a sheet and its frame, mm.
pub const SHEET_MARGIN_MM: f64 = 10.0;

/// A picture kept in the drawing: the key of its pixels in the rasters and their size.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pic {
    pub key: String,
    pub width: u32,
    pub height: u32,
}

impl Pic {
    fn validate(&self) -> Result<()> {
        if !self.key.starts_with(TITLEBLOCK_IMAGE_PREFIX) || self.key.chars().count() > 80 || self.key.chars().any(char::is_control) {
            return Err(invalid("A title block picture is not a usable key."));
        }
        if self.width == 0 || self.height == 0 || self.width > 16_384 || self.height > 16_384 {
            return Err(invalid("A title block picture is 1 to 16384 pixels on each side."));
        }
        Ok(())
    }
}

/// One of the parties of the project (client, designer, inspector, contractor…): its role, its details and its logo.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    pub role: String,
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<Pic>,
    /// Drawn under the picture of the project instead of above it (the contractor).
    #[serde(default)]
    pub after_image: bool,
}

/// One line of the table of revisions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub index: String,
    pub date: String,
    pub nature: String,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub checked: String,
}

/// One cell of the code of the sheet: its label and its value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Code {
    pub label: String,
    #[serde(default)]
    pub value: String,
}

/// The title block of a sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleBlockParams {
    pub name: String,
    /// The paper the column is drawn for ("A4" to "A0") and whether it is landscape: they give the height of the column.
    pub paper: String,
    pub landscape: bool,
    /// The width of the column, mm; its height is the sheet's less its margins.
    pub width_mm: f64,
    pub project: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub parties: Vec<Party>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Pic>,
    /// The title of the sheet.
    #[serde(default)]
    pub title: String,
    /// The revisions, the oldest first; the table shows at least `revision_rows` lines.
    #[serde(default)]
    pub revisions: Vec<Revision>,
    pub revision_rows: usize,
    #[serde(default)]
    pub codes: Vec<Code>,
    #[serde(default)]
    pub units: HvacUnits,
}

/// The paper sizes, mm (portrait: width, height).
pub const PAPERS: [(&str, f64, f64); 5] =
    [("A4", 210.0, 297.0), ("A3", 297.0, 420.0), ("A2", 420.0, 594.0), ("A1", 594.0, 841.0), ("A0", 841.0, 1189.0)];

/// The cells of the code of a sheet, the labels of a French study office's (company, lot, type, zone, level, phase, sheet number, index, date, scale).
pub const DEFAULT_CODE_LABELS: [&str; 10] = ["Société", "Lot", "Type", "Zone", "Niveau", "Phase", "N° plan", "Ind.", "Date", "Échelle"];
/// The relative widths of the cells of the code.
const CODE_WEIGHTS: [f64; 10] = [1.0, 0.9, 0.9, 0.9, 1.0, 1.0, 1.3, 0.8, 1.3, 1.1];

/// The width of the column the paper starts with, mm.
pub fn default_width_mm(paper: &str) -> f64 {
    match paper {
        "A4" => 100.0,
        "A3" => 130.0,
        "A2" => 160.0,
        "A1" => 190.0,
        _ => 220.0,
    }
}

impl TitleBlockParams {
    /// A title block for `paper` ("A4" to "A0") with the usual cells: three parties above the picture and the contractor below it, four empty
    /// lines of revisions and the ten cells of the code.
    pub fn new(name: &str, paper: &str, landscape: bool, units: HvacUnits) -> TitleBlockParams {
        let party = |role: &str, after_image: bool| Party { role: role.into(), after_image, ..Party::default() };
        TitleBlockParams {
            name: name.trim().to_string(),
            paper: paper.to_string(),
            landscape,
            width_mm: default_width_mm(paper),
            project: String::new(),
            description: String::new(),
            address: String::new(),
            parties: vec![
                party("Maîtrise d'ouvrage", false),
                party("Maîtrise d'œuvre", false),
                party("Bureau de contrôle", false),
                party("Entreprise", true),
            ],
            image: None,
            title: String::new(),
            revisions: Vec::new(),
            revision_rows: 4,
            codes: DEFAULT_CODE_LABELS.iter().map(|l| Code { label: (*l).into(), value: String::new() }).collect(),
            units,
        }
    }

    /// The sheet's sides, mm.
    pub fn sheet_mm(&self) -> Option<(f64, f64)> {
        let (_, w, h) = PAPERS.iter().find(|(n, _, _)| *n == self.paper)?;
        Some(if self.landscape { (*h, *w) } else { (*w, *h) })
    }

    /// The height of the column, mm: the sheet's height less the two margins.
    pub fn height_mm(&self) -> f64 {
        self.sheet_mm().map_or(0.0, |(_, h)| h - 2.0 * SHEET_MARGIN_MM)
    }

    fn scale(&self) -> f64 {
        self.width_mm / REFERENCE_WIDTH_MM
    }

    fn above(&self) -> Vec<&Party> {
        self.parties.iter().filter(|p| !p.after_image).collect()
    }

    fn below(&self) -> Vec<&Party> {
        self.parties.iter().filter(|p| p.after_image).collect()
    }

    /// The heights of the rows, mm, from the top: the header, the parties above, the picture, the parties below, the title, the table of
    /// revisions and the code. The picture takes what the fixed rows leave.
    fn rows(&self) -> Result<Rows> {
        let k = self.scale();
        let revision_lines = self.revision_rows.max(self.revisions.len());
        let rows = Rows {
            header: 24.0 * k,
            party: 22.0 * k,
            below: 26.0 * k,
            title: 26.0 * k,
            revision_line: 5.5 * k,
            revision_head: 6.0 * k,
            codes: 13.0 * k,
            image: 0.0,
        };
        let fixed = rows.header
            + rows.party * self.above().len() as f64
            + rows.below * self.below().len() as f64
            + rows.title
            + rows.revision_head
            + rows.revision_line * revision_lines as f64
            + rows.codes;
        let image = self.height_mm() - fixed;
        if image < 20.0 * k {
            return Err(invalid("The title block is too tall for the sheet: take fewer parties or revision lines, or a bigger sheet."));
        }
        Ok(Rows { image, ..rows })
    }

    pub fn validate(&self) -> Result<()> {
        check_name("title block", &self.name)?;
        self.units.validate()?;
        if self.sheet_mm().is_none() {
            return Err(invalid("A title block's paper is A4, A3, A2, A1 or A0."));
        }
        if !(self.width_mm.is_finite() && (60.0..=400.0).contains(&self.width_mm) && self.width_mm < self.sheet_mm().map_or(0.0, |(w, _)| w)) {
            return Err(invalid("A title block is 60 to 400 mm wide, and narrower than its sheet."));
        }
        if self.parties.len() > MAX_PARTIES
            || self.revisions.len() > MAX_REVISIONS
            || self.codes.len() > MAX_CODES
            || self.revision_rows > MAX_REVISIONS
        {
            return Err(invalid(format!("A title block has at most {MAX_PARTIES} parties, {MAX_REVISIONS} revisions and {MAX_CODES} codes.")));
        }
        let long = |s: &str| s.chars().count() > MAX_TEXT_CHARS || s.chars().any(|c| c.is_control() && c != '\n');
        let texts = self
            .parties
            .iter()
            .flat_map(|p| [&p.role, &p.name, &p.address, &p.phone, &p.email])
            .chain(self.revisions.iter().flat_map(|r| [&r.index, &r.date, &r.nature, &r.by, &r.checked]))
            .chain(self.codes.iter().flat_map(|c| [&c.label, &c.value]))
            .chain([&self.project, &self.description, &self.address, &self.title]);
        if texts.into_iter().any(|t| long(t)) {
            return Err(invalid(format!("A title block text has at most {MAX_TEXT_CHARS} characters, on one line.")));
        }
        for pic in self.image.iter().chain(self.parties.iter().filter_map(|p| p.logo.as_ref())) {
            pic.validate()?;
        }
        self.rows()?;
        Ok(())
    }

    /// The keys of the pictures the title block shows.
    pub fn picture_keys(&self) -> Vec<&str> {
        self.image.iter().chain(self.parties.iter().filter_map(|p| p.logo.as_ref())).map(|p| p.key.as_str()).collect()
    }

    /// The title block drawn in the block's coordinates (drawing units), its lower left corner at the origin.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let rows = self.rows()?;
        let mut d = Drawer { out: Vec::new(), units: self.units, k: self.scale() };
        let (w, h) = (self.width_mm, self.height_mm());
        d.rect(0.0, 0.0, w, h);
        let mut top = h;

        // The header: the name of the project, what is built and where.
        let band = top - rows.header;
        d.hline(band, 0.0, w);
        d.text(w / 2.0, top - 7.0 * d.k, &self.project, 5.2 * d.k, w - 6.0 * d.k, (HAlign::Center, true));
        d.text(w / 2.0, top - 14.5 * d.k, &self.description, 3.6 * d.k, w - 6.0 * d.k, (HAlign::Center, false));
        d.text(w / 2.0, top - 20.0 * d.k, &self.address, 3.2 * d.k, w - 6.0 * d.k, (HAlign::Center, false));
        top = band;

        // The parties above the picture.
        for p in self.above() {
            d.party(p, top, rows.party, w);
            top -= rows.party;
            d.hline(top, 0.0, w);
        }

        // The picture of the project.
        let bottom = top - rows.image;
        if let Some(pic) = &self.image {
            d.picture(pic, 3.0 * d.k, bottom + 2.0 * d.k, w - 6.0 * d.k, rows.image - 4.0 * d.k);
        }
        d.hline(bottom, 0.0, w);
        top = bottom;

        // The parties below it.
        for p in self.below() {
            d.party(p, top, rows.below, w);
            top -= rows.below;
            d.hline(top, 0.0, w);
        }

        // The title of the sheet.
        d.text(w / 2.0, top - 4.0 * d.k, "LIBELLÉ", 2.6 * d.k, w, (HAlign::Center, false));
        d.text(w / 2.0, top - 15.0 * d.k, &self.title, 7.0 * d.k, w - 8.0 * d.k, (HAlign::Center, false));
        top -= rows.title;
        d.hline(top, 0.0, w);

        // The table of revisions: the newest line on top, the oldest next to the head of the table.
        let lines = self.revision_rows.max(self.revisions.len());
        let cols = [0.07, 0.15, 0.45, 0.16, 0.17];
        let xs = Drawer::edges(&cols, w);
        let table_top = top;
        for i in 0..lines {
            let y = top - rows.revision_line * (i as f64 + 1.0);
            d.hline(y, 0.0, w);
            // The line `i` from the top is revision `lines - 1 - i` counted from the oldest.
            let from_bottom = lines - 1 - i;
            if let Some(r) = self.revisions.get(from_bottom) {
                let cells = [&r.index, &r.date, &r.nature, &r.by, &r.checked];
                for (c, text) in cells.iter().enumerate() {
                    d.text_left(xs[c] + 1.2 * d.k, y + rows.revision_line / 2.0, text, 3.0 * d.k, xs[c + 1] - xs[c] - 2.4 * d.k);
                }
            }
        }
        top -= rows.revision_line * lines as f64;
        let heads = ["IND.", "DATE", "NATURE DE LA MODIFICATION", "ÉTABLI PAR", "VÉRIFIÉ PAR"];
        for (c, text) in heads.iter().enumerate() {
            d.text_left(xs[c] + 1.2 * d.k, top - rows.revision_head / 2.0, text, 2.6 * d.k, xs[c + 1] - xs[c] - 2.4 * d.k);
        }
        for x in &xs[1..xs.len() - 1] {
            d.vline(*x, top - rows.revision_head, table_top);
        }
        top -= rows.revision_head;
        d.hline(top, 0.0, w);

        // The code of the sheet: the labels over the values.
        let n = self.codes.len().max(1);
        let weights: Vec<f64> = (0..n).map(|i| CODE_WEIGHTS.get(i).copied().unwrap_or(1.0)).collect();
        let sum: f64 = weights.iter().sum();
        let cols: Vec<f64> = weights.iter().map(|x| x / sum).collect();
        let xs = Drawer::edges(&cols, w);
        let mid = top - 4.5 * d.k;
        d.hline(mid, 0.0, w);
        for (i, c) in self.codes.iter().enumerate() {
            let width = xs[i + 1] - xs[i] - 1.6 * d.k;
            d.text(xs[i] + (xs[i + 1] - xs[i]) / 2.0, top - 2.3 * d.k, &c.label, 2.3 * d.k, width, (HAlign::Center, false));
            d.text(xs[i] + (xs[i + 1] - xs[i]) / 2.0, mid - 4.2 * d.k, &c.value, 3.6 * d.k, width, (HAlign::Center, false));
            if i > 0 {
                d.vline(xs[i], 0.0, top);
            }
        }
        Ok(d.out)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(TITLEBLOCK_APP, TITLEBLOCK_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<TitleBlockParams> {
        let p: TitleBlockParams = xdata::from_xdata(TITLEBLOCK_APP, TITLEBLOCK_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

struct Rows {
    header: f64,
    party: f64,
    below: f64,
    title: f64,
    revision_line: f64,
    revision_head: f64,
    codes: f64,
    image: f64,
}

/// Draws in millimetres on paper and hands the pieces out in drawing units.
struct Drawer {
    out: Vec<HvacPiece>,
    units: HvacUnits,
    k: f64,
}

impl Drawer {
    fn u(&self, mm: f64) -> f64 {
        self.units.to_units(mm)
    }

    fn push(&mut self, kind: EntityKind) {
        self.out.push(HvacPiece { layer: TITLEBLOCK_LAYER, kind });
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (x0, y0, x1, y1) = (self.u(x), self.u(y), self.u(x + w), self.u(y + h));
        self.push(poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true));
    }

    fn hline(&mut self, y: f64, x0: f64, x1: f64) {
        let (a, b, y) = (self.u(x0), self.u(x1), self.u(y));
        self.push(line((a, y), (b, y)));
    }

    fn vline(&mut self, x: f64, y0: f64, y1: f64) {
        let (x, a, b) = (self.u(x), self.u(y0), self.u(y1));
        self.push(line((x, a), (x, b)));
    }

    /// The x of each edge of columns of relative widths `parts` (summing to 1) across `w`.
    fn edges(parts: &[f64], w: f64) -> Vec<f64> {
        let mut xs = vec![0.0];
        let mut x = 0.0;
        for p in parts {
            x += p * w;
            xs.push(x);
        }
        xs
    }

    /// A text whose height shrinks so that it fits in `width` mm (about 0.6 of its height per character).
    fn text(&mut self, x: f64, y: f64, value: &str, height: f64, width: f64, (halign, bold): (HAlign, bool)) {
        let value = value.trim();
        if value.is_empty() || width <= 0.0 {
            return;
        }
        let chars = value.chars().count() as f64;
        let mut h = height.min(width / (0.62 * chars)).max(0.5);
        if bold {
            h = h.min(width / (0.7 * chars)).max(0.5);
        }
        let (px, py) = (self.u(x), self.u(y));
        let at = Vec2::new(px, py);
        self.push(EntityKind::Text(Text {
            insert: v3(at),
            align_pt: Some(v3(at)),
            height: self.u(h),
            value: value.to_string(),
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style: "Standard".into(),
            halign,
            valign: VAlign::Middle,
        }));
    }

    fn text_left(&mut self, x: f64, y: f64, value: &str, height: f64, width: f64) {
        self.text(x, y, value, height, width, (HAlign::Left, false));
    }

    /// A party: its role at the left over its logo, its name, address, phone and mail at the right.
    fn party(&mut self, p: &Party, top: f64, height: f64, w: f64) {
        let left = 0.31 * w;
        self.vline(left, top - height, top);
        let k = self.k;
        self.text_left(2.0 * k, top - 4.0 * k, &p.role, 3.0 * k, left - 3.0 * k);
        if let Some(pic) = &p.logo {
            self.picture(pic, 2.0 * k, top - height + 1.5 * k, left - 4.0 * k, height - 8.0 * k);
        }
        let x = left + 2.5 * k;
        let width = w - left - 5.0 * k;
        let step = 4.4 * k;
        let mut y = top - 4.0 * k;
        self.text(x, y, &p.name, 3.6 * k, width, (HAlign::Left, true));
        for line in [&p.address, &p.phone, &p.email] {
            y -= step;
            self.text_left(x, y, line, 3.0 * k, width);
        }
    }

    /// A picture in the box `x`, `y`, `w` by `h` (mm), as large as fits it and centred.
    fn picture(&mut self, pic: &Pic, x: f64, y: f64, w: f64, h: f64) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (pw, ph) = (f64::from(pic.width), f64::from(pic.height));
        let s = (w / pw).min(h / ph);
        let (iw, ih) = (pw * s, ph * s);
        let (ix, iy) = (x + (w - iw) / 2.0, y + (h - ih) / 2.0);
        let px = self.u(s);
        self.push(EntityKind::Image(Image {
            insert: v3(Vec2::new(self.u(ix), self.u(iy))),
            u: cadcraft_geom::Vec3::new(px, 0.0, 0.0),
            v: cadcraft_geom::Vec3::new(0.0, px, 0.0),
            size: Vec2::new(pw, ph),
            path: pic.key.clone(),
            name: pic.key.clone(),
            clip: Vec::new(),
            clipping: false,
            display: 7,
            brightness: 50,
            contrast: 50,
            fade: 0,
            pixel_size: Vec2::new(1.0, 1.0),
            resolution_units: 0,
        }));
    }
}

/// The title block an object is, when it is a block reference carrying usable data.
pub fn titleblock_of(e: &crate::Entity) -> Option<TitleBlockParams> {
    match &e.kind {
        EntityKind::Insert(_) => TitleBlockParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour of the title block's layer.
pub(super) fn titleblock_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    (name == TITLEBLOCK_LAYER).then_some(cadcraft_color::Color::WHITE)
}

impl crate::Drawing {
    /// Adds a title block, its lower left corner at `at` (drawing coordinates).
    pub fn add_titleblock(&mut self, space: &crate::Space, mut common: crate::Common, at: Vec2, p: TitleBlockParams) -> Result<crate::Handle> {
        let pieces = p.generate()?;
        common.xdata = xdata::replace(TITLEBLOCK_APP, &common.xdata, p.to_xdata()?);
        common.layer = TITLEBLOCK_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(TITLEBLOCK_LAYER);
        let insert = crate::Insert { block, insert: v3(at), ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives title block `h` the parameters `p`, redrawing it in place.
    pub fn regenerate_titleblock(&mut self, h: crate::Handle, p: &TitleBlockParams) -> Result<()> {
        let pieces = p.generate()?;
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let tags = xdata::replace(TITLEBLOCK_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)?;
        self.ensure_layer(TITLEBLOCK_LAYER);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Common, Drawing, Space};

    fn units() -> HvacUnits {
        HvacUnits { mm_per_unit: 1.0 }
    }

    fn sample() -> TitleBlockParams {
        let mut p = TitleBlockParams::new("Cartouche", "A3", false, units());
        p.project = "NOM DU PROJET".into();
        p.title = "Plan des réseaux".into();
        p.parties[0].name = "Société A".into();
        p.revisions.push(Revision { index: "A".into(), date: "30/11/2023".into(), nature: "Première diffusion".into(), ..Revision::default() });
        p
    }

    #[test]
    fn the_default_title_block_fits_every_paper() {
        for (name, ..) in PAPERS {
            for landscape in [false, true] {
                let p = TitleBlockParams::new("C", name, landscape, units());
                let pieces = p.generate().unwrap_or_else(|e| panic!("{name} {landscape}: {e}"));
                assert!(pieces.len() > 20, "{name}");
            }
        }
    }

    #[test]
    fn the_picture_takes_what_the_rows_leave_and_more_revisions_take_from_it() {
        let a = sample();
        let mut b = sample();
        b.revision_rows = 12;
        assert!(a.rows().unwrap().image > b.rows().unwrap().image);
        let total = |p: &TitleBlockParams| {
            let r = p.rows().unwrap();
            r.header
                + r.party * p.above().len() as f64
                + r.image
                + r.below * p.below().len() as f64
                + r.title
                + r.revision_head
                + r.revision_line * 12.0_f64.max(p.revisions.len() as f64)
                + r.codes
        };
        assert!((total(&b) - b.height_mm()).abs() < 1e-6);
    }

    #[test]
    fn too_many_rows_are_refused_and_bad_data_is_refused() {
        let mut p = TitleBlockParams::new("C", "A4", true, units());
        p.revision_rows = MAX_REVISIONS;
        assert!(p.validate().is_err(), "a tall table on a small sheet");
        let mut ok = sample();
        ok.width_mm = f64::NAN;
        assert!(ok.validate().is_err());
        let mut ok = sample();
        ok.paper = "B5".into();
        assert!(ok.validate().is_err());
        let mut ok = sample();
        ok.project = "x".repeat(MAX_TEXT_CHARS + 1);
        assert!(ok.validate().is_err());
        let mut ok = sample();
        ok.image = Some(Pic { key: "somewhere".into(), width: 10, height: 10 });
        assert!(ok.validate().is_err());
        let mut ok = sample();
        ok.image = Some(Pic { key: format!("{TITLEBLOCK_IMAGE_PREFIX}1"), width: 0, height: 10 });
        assert!(ok.validate().is_err());
    }

    #[test]
    fn a_title_block_round_trips_through_its_extended_data_and_redraws() {
        let mut d = Drawing::new_metric();
        let mut p = sample();
        p.image = Some(Pic { key: format!("{TITLEBLOCK_IMAGE_PREFIX}1"), width: 800, height: 450 });
        p.parties[3].logo = Some(Pic { key: format!("{TITLEBLOCK_IMAGE_PREFIX}2"), width: 100, height: 60 });
        let h = d.add_titleblock(&Space::Model, Common::default(), Vec2::new(100.0, 50.0), p.clone()).unwrap();
        let back = titleblock_of(d.entity(h).unwrap()).expect("a title block");
        assert_eq!(back, p);
        // Two pictures are drawn, inside the column.
        let block = match &d.entity(h).unwrap().kind {
            EntityKind::Insert(i) => d.block(&i.block).unwrap().clone(),
            _ => panic!("an insert"),
        };
        let images: Vec<&Image> = block.entities.iter().filter_map(|e| if let EntityKind::Image(i) = &e.kind { Some(i) } else { None }).collect();
        assert_eq!(images.len(), 2);
        for i in images {
            let (w, h2) = (i.u.x * i.size.x, i.v.y * i.size.y);
            assert!(i.insert.x >= 0.0 && i.insert.x + w <= p.width_mm + 1e-6 && i.insert.y >= 0.0 && i.insert.y + h2 <= p.height_mm() + 1e-6);
        }
        let mut q = p.clone();
        q.revisions.push(Revision { index: "B".into(), ..Revision::default() });
        q.revision_rows = 6;
        d.regenerate_titleblock(h, &q).unwrap();
        assert_eq!(titleblock_of(d.entity(h).unwrap()).map(|x| x.revisions.len()), Some(2));
    }

    #[test]
    fn long_texts_shrink_to_their_cell() {
        let mut p = sample();
        p.project = "UN NOM DE PROJET TRÈS LONG QUI NE TIENT PAS À LA HAUTEUR NORMALE DE L'EN-TÊTE".into();
        let pieces = p.generate().unwrap();
        for piece in pieces {
            if let EntityKind::Text(t) = piece.kind
                && t.value.starts_with("UN NOM")
            {
                assert!(t.height < 5.2 * p.scale(), "the text was made smaller");
                assert!(t.height * 0.6 * t.value.chars().count() as f64 <= p.width_mm, "and fits the column");
            }
        }
    }
}
