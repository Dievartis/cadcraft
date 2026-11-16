//! Font-family and variant discovery adapted from @davidhooey's CADCraft PR #2462.
//! Keep naming-table I/O and picker indexing here; rendering and document commands stay unchanged.

#[cfg(not(target_arch = "wasm32"))]
use super::MAX_FILE_BYTES;
use std::collections::HashMap;

use super::{Db, Face, MAX_FACES_PER_FILE, Source, is_stroke_name, key, lock, norm};

/// Common font families offered at the top of font pickers (after [`crate::BUILTIN_FONT`]) when
/// installed or registered.
const COMMON_FAMILIES: &[&str] = &["Arial", "Calibri", "Courier New", "Georgia", "Times New Roman", "Verdana"];
/// Maximum `name` table size read when scanning installed fonts for family names.
const MAX_NAME_TABLE_BYTES: usize = 256 << 10;
/// Maximum sfnt tables per face directory inspected during the family-name scan.
const MAX_SFNT_TABLES: usize = 128;
// Bound the complete discovery pass, not only each individual font file.
const MAX_CATALOG_FILES: usize = 4096;
const MAX_CATALOG_BYTES: usize = 16 << 20;

struct NameReader<'a, R> {
    inner: &'a mut R,
    remaining: &'a mut usize,
}
impl<R: std::io::Read> std::io::Read for NameReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let limit = buffer.len().min(*self.remaining);
        let read = self.inner.read(&mut buffer[..limit])?;
        *self.remaining = self.remaining.saturating_sub(read);
        Ok(read)
    }
}
impl<R: std::io::Seek> std::io::Seek for NameReader<'_, R> {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

/// One style face of a discovered font family.
#[derive(Clone, Debug)]
struct FamilyStyle {
    /// Subfamily/style name from the `name` table (`"Regular"`, `"Bold"`, `"Italic"`, etc.).
    style: String,
    /// Lower-case file stem in `Db::paths` or `Db::files`.
    stem: String,
    /// Face index inside the font file (`0` for a single-face sfnt, `0..N` for `.ttc`/`.otc`).
    index: u32,
    priority: u8,
}

/// A font family discovered from the `name` tables of installed or registered fonts.
#[derive(Clone, Debug)]
pub(super) struct FamilyEntry {
    /// Family name from the `name` table (`"Arial"`, `"Agency FB"`, `"Times New Roman"`, etc.).
    family: String,
    /// Styles belonging to this family, ordered with Regular first.
    styles: Vec<FamilyStyle>,
    // Exact file names remain identifiable when several files share a family/style.
    sources: HashMap<String, String>,
}

/// Read `(face_index, family_name, subfamily_name)` for each face of an sfnt font or TTC
/// collection from a seekable reader, reading only the table directories and `name` tables.
fn read_reader_names(input: &mut (impl std::io::Read + std::io::Seek), budget: &mut usize) -> Vec<(u32, String, String)> {
    use std::io::{Read, Seek};
    let r = &mut NameReader { inner: input, remaining: budget };
    let mut hdr = [0u8; 12];
    if r.seek(std::io::SeekFrom::Start(0)).is_err() || r.read_exact(&mut hdr).is_err() {
        return Vec::new();
    }
    if &hdr[0..4] == b"ttcf" {
        let count = u32::from_be_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]).min(MAX_FACES_PER_FILE);
        let Ok(byte_len) = usize::try_from(count).map(|n| n.saturating_mul(4)) else {
            return Vec::new();
        };
        let mut offs = vec![0u8; byte_len];
        if r.read_exact(&mut offs).is_err() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (i, chunk) in offs.as_chunks::<4>().0.iter().enumerate() {
            let off = u64::from(u32::from_be_bytes(*chunk));
            let idx = u32::try_from(i).unwrap_or(0);
            if let Some((fam, sub)) = read_sfnt_name(r, off) {
                out.push((idx, fam, sub));
            }
        }
        out
    } else if let Some((fam, sub)) = read_sfnt_name(r, 0) {
        vec![(0, fam, sub)]
    } else {
        Vec::new()
    }
}

fn read_sfnt_name(r: &mut (impl std::io::Read + std::io::Seek), sfnt_offset: u64) -> Option<(String, String)> {
    r.seek(std::io::SeekFrom::Start(sfnt_offset)).ok()?;
    let mut hdr = [0u8; 12];
    r.read_exact(&mut hdr).ok()?;
    if !matches!(&hdr[0..4], b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"typ1") {
        return None;
    }
    let num_tables = usize::from(u16::from_be_bytes([hdr[4], hdr[5]])).min(MAX_SFNT_TABLES);
    let mut dir = vec![0u8; num_tables.saturating_mul(16)];
    r.read_exact(&mut dir).ok()?;
    let mut name_loc = None;
    for rec in dir.as_chunks::<16>().0 {
        if &rec[0..4] == b"name" {
            let offset = u64::from(u32::from_be_bytes([rec[8], rec[9], rec[10], rec[11]]));
            let length = usize::try_from(u32::from_be_bytes([rec[12], rec[13], rec[14], rec[15]])).ok()?;
            name_loc = Some((offset, length));
            break;
        }
    }
    let (offset, length) = name_loc?;
    if length == 0 || length > MAX_NAME_TABLE_BYTES {
        return None;
    }
    r.seek(std::io::SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; length];
    r.read_exact(&mut buf).ok()?;
    parse_name_table(&buf)
}

fn pick_name_string(name: &skrifa::raw::tables::name::Name<'_>, id: skrifa::string::StringId) -> Option<String> {
    let data = name.string_data();
    let mut best: Option<(u8, String)> = None;
    for rec in name.name_record() {
        if rec.name_id() != id {
            continue;
        }
        let rank = match (rec.platform_id(), rec.language_id()) {
            (3, 0x0409) => 0,
            (0, _) => 1,
            (1, 0) => 2,
            _ => 3,
        };
        if best.as_ref().is_some_and(|(b, _)| *b <= rank) {
            continue;
        }
        let Ok(s) = rec.string(data) else { continue };
        let text: String = s.chars().take(128).collect();
        let trimmed = text.trim();
        if usable_font_name(trimmed) {
            best = Some((rank, trimmed.to_string()));
            if rank == 0 {
                break;
            }
        }
    }
    best.map(|(_, s)| s)
}

fn parse_name_table(buf: &[u8]) -> Option<(String, String)> {
    use skrifa::raw::{FontData, FontRead, tables::name::Name};
    use skrifa::string::StringId;
    let name = Name::read(FontData::new(buf)).ok()?;
    let family = pick_name_string(&name, StringId::TYPOGRAPHIC_FAMILY_NAME).or_else(|| pick_name_string(&name, StringId::FAMILY_NAME))?;
    let style = pick_name_string(&name, StringId::TYPOGRAPHIC_SUBFAMILY_NAME)
        .or_else(|| pick_name_string(&name, StringId::SUBFAMILY_NAME))
        .unwrap_or_else(|| "Regular".to_string());
    Some((family, style))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_path_names(path: &std::path::Path, budget: &mut usize) -> Vec<(u32, String, String)> {
    if std::fs::metadata(path).ok().is_none_or(|m| !m.is_file() || m.len() > MAX_FILE_BYTES) {
        return Vec::new();
    }
    let Ok(mut file) = std::fs::File::open(path) else { return Vec::new() };
    read_reader_names(&mut file, budget)
}

#[cfg(target_arch = "wasm32")]
fn read_path_names(_path: &std::path::Path, _budget: &mut usize) -> Vec<(u32, String, String)> {
    Vec::new()
}

fn style_rank(style: &str) -> (u8, String) {
    let lower = style.trim().to_ascii_lowercase();
    let rank = match lower.as_str() {
        "regular" | "roman" | "normal" | "book" | "plain" => 0,
        "italic" | "oblique" => 1,
        "bold" => 2,
        "bold italic" | "bold oblique" => 3,
        _ => 4,
    };
    (rank, lower)
}

fn insert_family_style(families: &mut Vec<FamilyEntry>, family: String, style: String, stem: String, index: u32, priority: u8) {
    if !usable_font_name(&family) || !usable_font_name(&style) {
        return;
    }
    if let Some(entry) = families.iter_mut().find(|e| e.family.eq_ignore_ascii_case(&family)) {
        if index == 0 {
            entry.sources.insert(stem.clone(), style.clone());
        }
        if let Some(existing) = entry.styles.iter_mut().find(|s| s.style.eq_ignore_ascii_case(&style)) {
            if (priority, stem.len(), &stem, index) < (existing.priority, existing.stem.len(), &existing.stem, existing.index) {
                existing.stem = stem;
                existing.index = index;
                existing.priority = priority;
            }
        } else {
            entry.styles.push(FamilyStyle { style, stem, index, priority });
        }
    } else {
        let sources = if index == 0 { HashMap::from([(stem.clone(), style.clone())]) } else { HashMap::new() };
        families.push(FamilyEntry { family, styles: vec![FamilyStyle { style, stem, index, priority }], sources });
    }
}

impl Db {
    fn face_at(&mut self, stem: &str, index: u32) -> Option<Face> {
        let bytes = self.file(stem)?;
        Face::new(bytes, index)
    }

    fn stem_file_name(&self, stem: &str) -> String {
        if let Some(Source::File(path)) = self.paths.get(stem)
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            return name.to_string();
        }
        let extension = self.files.get(stem).map(|bytes| bytes.get(..4));
        let ext = match extension.flatten() {
            Some(b"OTTO") => "otf",
            Some(b"ttcf") => "ttc",
            _ => "ttf",
        };
        format!("{stem}.{ext}")
    }

    fn source_names(&self, stem: &str, budget: &mut usize) -> Vec<(u32, String, String)> {
        if let Some(bytes) = self.files.get(stem) {
            return read_reader_names(&mut std::io::Cursor::new(bytes.as_slice()), budget);
        }
        match self.paths.get(stem) {
            Some(Source::File(path)) => read_path_names(path, budget),
            Some(Source::Static(bytes)) => read_reader_names(&mut std::io::Cursor::new(*bytes), budget),
            None => Vec::new(),
        }
    }

    fn add_source_families(&self, families: &mut Vec<FamilyEntry>, stems: &[String], budget: &mut usize) {
        for stem in stems {
            if *budget == 0 {
                break;
            }
            let priority = u8::from(!self.user.contains(stem) && !self.registered.contains(stem));
            for (index, family, style) in self.source_names(stem, budget) {
                insert_family_style(families, family, style, stem.clone(), index, priority);
            }
        }
    }

    fn ensure_families(&mut self) -> &[FamilyEntry] {
        if self.families.is_none() {
            let mut families = Vec::new();
            let mut budget = MAX_CATALOG_BYTES;
            for f in crate::CRAFT_FONTS {
                insert_family_style(&mut families, f.family.to_string(), f.style.to_string(), f.stem(), 0, 2);
            }
            let mut memory: Vec<_> = self.files.keys().take(MAX_CATALOG_FILES).cloned().collect();
            memory.sort();
            self.add_source_families(&mut families, &memory, &mut budget);
            // User folders are considered before the bounded installed-font list.
            let mut paths: Vec<_> = self
                .user
                .iter()
                .chain(self.paths.keys().filter(|key| !self.user.contains(*key)))
                .filter(|stem| !self.files.contains_key(*stem) && !crate::craft_fonts::is_embedded(stem))
                .take(MAX_CATALOG_FILES)
                .cloned()
                .collect();
            paths.sort();
            self.add_source_families(&mut families, &paths, &mut budget);
            for entry in &mut families {
                entry.styles.sort_by(|a, b| style_rank(&a.style).cmp(&style_rank(&b.style)).then_with(|| a.stem.cmp(&b.stem)));
            }
            families.sort_by(|a, b| a.family.to_lowercase().cmp(&b.family.to_lowercase()).then_with(|| a.family.cmp(&b.family)));
            self.families = Some(families);
        }
        self.families.as_deref().unwrap_or(&[])
    }

    pub(super) fn by_family_table(&mut self, k: &str) -> Option<Face> {
        let want = norm(k);
        if want.is_empty() {
            return None;
        }
        let mut target: Option<(String, u32)> = None;
        for entry in self.ensure_families() {
            let fam_norm = norm(&entry.family);
            if (fam_norm == want || format!("{fam_norm}regular") == want)
                && let Some(first) = entry.styles.first()
            {
                target = Some((first.stem.clone(), first.index));
                break;
            }
            for st in &entry.styles {
                if norm(&format!("{} {}", entry.family, st.style)) == want {
                    target = Some((st.stem.clone(), st.index));
                    break;
                }
            }
            if target.is_some() {
                break;
            }
        }
        let (stem, index) = target?;
        self.face_at(&stem, index)
    }
}
/// Whether `s` is a visible font name suitable for a font picker (not a hidden dotfile or
/// the built-in stroke font, which is always placed first).
fn usable_font_name(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && !s.starts_with('.') && !s.eq_ignore_ascii_case(crate::BUILTIN_FONT) && !s.chars().any(char::is_control)
}

/// Font family names for a Text Style font picker: [`crate::BUILTIN_FONT`] (`"CADCraft Stroke"`)
/// first, followed by installed [`COMMON_FAMILIES`], embedded [`crate::CRAFT_FONTS`] families, and
/// family names discovered from each font's `name` table, sorted and deduplicated case-insensitively.
pub fn available_fonts() -> Vec<String> {
    let mut g = lock();
    if let Some(names) = &g.names {
        return names.clone();
    }
    let families: Vec<String> = g.ensure_families().iter().map(|e| e.family.clone()).collect();
    let mut out: Vec<String> = Vec::with_capacity(families.len() + 1);
    out.push(crate::BUILTIN_FONT.to_string());
    for &common in COMMON_FAMILIES {
        let found = families.iter().any(|f| f.eq_ignore_ascii_case(common)) || g.lookup(&key(common)).is_some();
        if found && !out.iter().any(|existing| existing.eq_ignore_ascii_case(common)) {
            out.push(common.to_string());
        }
    }
    for f in crate::CRAFT_FONTS {
        if usable_font_name(f.family) && !out.iter().any(|existing| existing.eq_ignore_ascii_case(f.family)) {
            out.push(f.family.to_string());
        }
    }
    for fam in families {
        if !out.iter().any(|existing| existing.eq_ignore_ascii_case(&fam)) {
            out.push(fam);
        }
    }
    g.names = Some(out.clone());
    out
}

/// Split a style font spec (`"Arial"`, `"Arial Bold"`, `"arialbd.ttf"`) into
/// `(family_name, current_style, family_styles)` using the scanned `name` tables. Returns `None`
/// for stroke/SHX fonts or unknown font names.
pub fn describe_font(name: &str) -> Option<(String, String, Vec<String>)> {
    let k = key(name);
    if is_stroke_name(&k) {
        return None;
    }
    let want = norm(&k);
    let mut g = lock();
    let entries = g.ensure_families();
    for entry in entries {
        if let Some(style) = entry.sources.get(&k) {
            return Some((entry.family.clone(), style.clone(), entry.styles.iter().map(|s| s.style.clone()).collect()));
        }
    }
    for entry in entries {
        let styles = || entry.styles.iter().map(|s| s.style.clone()).collect::<Vec<_>>();
        for st in &entry.styles {
            if norm(&format!("{} {}", entry.family, st.style)) == want || (st.stem.eq_ignore_ascii_case(&k) && st.index == 0) {
                return Some((entry.family.clone(), st.style.clone(), styles()));
            }
        }
        let fam_norm = norm(&entry.family);
        if fam_norm == want || format!("{fam_norm}regular") == want {
            let cur = entry.styles.first().map_or_else(|| "Regular".to_string(), |s| s.style.clone());
            return Some((entry.family.clone(), cur, styles()));
        }
    }
    None
}

/// Resolve a font family and subfamily style (`"Arial"` + `"Regular"` → `"arial.ttf"`,
/// `"Arial"` + `"Bold"` → `"arialbd.ttf"`) to the face's font file name for DXF `STYLE` group 3.
pub fn compose_font(family: &str, style: &str) -> String {
    let fam = family.trim();
    let st = style.trim();
    let mut g = lock();
    g.ensure_families();
    let target_stem = g.families.as_deref().unwrap_or(&[]).iter().find(|e| e.family.eq_ignore_ascii_case(fam)).and_then(|entry| {
        let chosen = if st.is_empty() || st.eq_ignore_ascii_case("Regular") {
            entry.styles.iter().find(|s| s.style.eq_ignore_ascii_case("Regular")).or_else(|| entry.styles.first())
        } else {
            entry.styles.iter().find(|s| s.style.eq_ignore_ascii_case(st))
        }?;
        (chosen.index == 0).then(|| chosen.stem.clone())
    });
    if let Some(stem) = target_stem {
        return g.stem_file_name(&stem);
    }
    if st.is_empty() || st.eq_ignore_ascii_case("Regular") { fam.to_string() } else { format!("{fam} {st}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_and_budget_exhausted_fonts_do_not_produce_names() {
        let mut budget = 8;
        assert!(read_reader_names(&mut std::io::Cursor::new(vec![0; 1024]), &mut budget).is_empty());
        assert_eq!(budget, 0);
        let mut budget = 1024;
        assert!(read_reader_names(&mut std::io::Cursor::new(b"not a font"), &mut budget).is_empty());
    }

    #[test]
    fn duplicate_files_keep_their_identity_and_user_fonts_win() {
        let mut entries = Vec::new();
        insert_family_style(&mut entries, "Family".into(), "Bold".into(), "system".into(), 0, 1);
        insert_family_style(&mut entries, "Family".into(), "Bold".into(), "imported-long-name".into(), 0, 0);
        let family = &entries[0];
        assert_eq!(family.styles.len(), 1);
        assert_eq!(family.styles[0].stem, "imported-long-name");
        assert_eq!(family.sources.get("system").map(String::as_str), Some("Bold"));
        assert_eq!(family.sources.get("imported-long-name").map(String::as_str), Some("Bold"));
    }

    #[test]
    fn collection_alias_resolves_the_selected_face_with_a_cold_index() {
        let mut italic = crate::ttf_tests::sfnt_with_names("Collection Picker Test", "Italic");
        let mut regular = crate::ttf_tests::sfnt_with_names("Collection Picker Test", "Regular");
        let regular_offset = 20 + italic.len() as u32;
        // Collection table offsets are absolute from the beginning of the file.
        italic[20..24].copy_from_slice(&(20u32 + 28).to_be_bytes());
        regular[20..24].copy_from_slice(&(regular_offset + 28).to_be_bytes());
        let mut collection = b"ttcf".to_vec();
        for value in [0x00010000u32, 2, 20, regular_offset] {
            collection.extend_from_slice(&value.to_be_bytes());
        }
        collection.extend(italic);
        collection.extend(regular);
        let mut db = Db::default();
        db.files.insert("collection-picker".into(), std::sync::Arc::new(collection));
        assert!(db.families.is_none());
        assert_eq!(db.by_family("Collection Picker Test").unwrap().index, 1);
        assert_eq!(db.by_family("Collection Picker Test Italic").unwrap().index, 0);
    }

    #[test]
    fn unicode_names_are_accepted_and_controls_are_rejected() {
        assert!(usable_font_name("Écriture 日本語"));
        assert!(!usable_font_name("hidden\nfont"));
        assert!(!usable_font_name(".hidden"));
    }
}
