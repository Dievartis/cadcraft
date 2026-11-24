//! Raster images (IMAGEATTACH, IMAGE). An IMAGE entity names its file; the pixels are decoded
//! when the drawing opens or the image is attached and kept with the open drawing
//! ([`Drawing::rasters`]), never in the file. An image whose file can't be found or read stays in
//! the drawing as a crossed-out frame.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use cadcraft_doc::{Drawing, EntityKind, Image, Raster};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::file::base64_decode;
use super::{CommandSpec, bad, f64_or, point_param, str_param};
use crate::{Result, Session};

/// Most image files loaded per drawing.
const MAX_IMAGE_FILES: usize = 1000;
/// Largest image file read (bytes).
const MAX_IMAGE_BYTES: u64 = 256 << 20;
/// Longest base64 text taken for `data`: what a file of [`MAX_IMAGE_BYTES`] encodes to.
const MAX_IMAGE_BASE64: u64 = MAX_IMAGE_BYTES.div_ceil(3) * 4;
/// Longest `data` text with the line breaks and spaces base64 may carry (MIME breaks every 76
/// characters, under 3% more): the characters past [`MAX_IMAGE_BASE64`] are counted only up to
/// here, which also bounds what decoding sets aside.
const MAX_IMAGE_TEXT: u64 = MAX_IMAGE_BASE64 + MAX_IMAGE_BASE64 / 16;
/// Most decoded pixel bytes kept per drawing; images past it stay unloaded.
const MAX_LOADED_BYTES: u64 = 2 << 30;
/// Most bytes one decoded image takes: the image reader decodes at most 128 million pixels, four
/// bytes each (`cadcraft_io::raster::MAX_IMAGE_PIXELS`).
const MAX_DECODED_BYTES: u64 = 512_000_000;
/// Most image files decoded at the same time when a drawing opens.
const MAX_LOADERS: usize = 4;
/// Longest image path taken from a caller.
const MAX_PATH: usize = 4096;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("imageattach", "Raster Image Reference...", run_imageattach).alias(&["iat"]).params(
            "{path: PNG/JPEG/BMP/TIFF file (saved as given; a relative path is relative to this drawing's folder), data?: base64 file contents \
             (instead of reading `path`), at?: [x,y] lower-left corner (default 0,0), scale?: number (default 1: the image is one drawing unit \
             wide), width?: width in drawing units (instead of `scale`), rotation? (degrees), name? (default: the file name)} → {name, handle, \
             path, pixels: [w, h], size: [w, h]}",
        ),
        CommandSpec::new("image", "Raster Images", run_image)
            .menu(&["Insert", "Raster Images"])
            .alias(&["im"])
            .params("{} → {images: [{name, path, foundAt, status: Loaded|Not Found|Not loaded, references, pixels?: [w, h]}]}")
            .noundo(),
        CommandSpec::new("imageedit", "Edit Raster Image", run_imageedit)
            .params("{handle?, brightness?, contrast?, fade?, transparency?, clipping?, clip?, insert?, width?, height?, rotation?} → {handle}"),
        CommandSpec::new("imagereload", "Reload Raster Image", run_imagereload).params("{handle?} → {handle}"),
    ]
}

fn run_imagereload(s: &mut Session, p: &Value) -> Result<Value> {
    let h = image_handle(s, p)?;
    let path = match s.doc()?.entity(h).map(|e| &e.kind) {
        Some(EntityKind::Image(i)) => i.path.clone(),
        _ => return Err(bad("imagereload", "object is not a raster image")),
    };
    let host = s.state()?.path.clone();
    let raster = read_file(&path, host.as_deref()).and_then(|bytes| decode(&bytes)).map_err(|e| bad("imagereload", e))?;
    if bytes_of(&raster) > MAX_LOADED_BYTES {
        return Err(bad("imagereload", "image exceeds the memory limit"));
    }
    s.doc_mut()?.rasters.insert(&path, Arc::new(raster));
    Ok(json!({"handle": h.hex()}))
}

fn image_handle(s: &Session, p: &Value) -> Result<cadcraft_doc::Handle> {
    let explicit = p.get("handle").and_then(|v| v.as_str()).and_then(cadcraft_doc::Handle::parse_hex);
    explicit
        .or_else(|| {
            s.selection().into_iter().find(|h| s.doc().ok().and_then(|d| d.entity(*h)).is_some_and(|e| matches!(e.kind, EntityKind::Image(_))))
        })
        .ok_or_else(|| bad("imageedit", "`handle` of an image is required"))
}

fn percent(p: &Value, key: &str, current: u8) -> Result<u8> {
    let Some(v) = p.get(key) else { return Ok(current) };
    let n = v.as_f64().ok_or_else(|| bad("imageedit", format!("`{key}` must be a number")))?;
    if !n.is_finite() || !(0.0..=100.0).contains(&n) {
        return Err(bad("imageedit", format!("`{key}` must be between 0 and 100")));
    }
    Ok(n.round() as u8)
}

fn run_imageedit(s: &mut Session, p: &Value) -> Result<Value> {
    let h = image_handle(s, p)?;
    let old = match s.doc()?.entity(h).map(|e| &e.kind) {
        Some(EntityKind::Image(i)) => i.clone(),
        _ => return Err(bad("imageedit", "object is not a raster image")),
    };
    let mut image = old;
    image.brightness = percent(p, "brightness", image.brightness)?;
    image.contrast = percent(p, "contrast", image.contrast)?;
    image.fade = percent(p, "fade", image.fade)?;
    if let Some(v) = p.get("transparency") {
        let enabled = v.as_bool().ok_or_else(|| bad("imageedit", "`transparency` must be boolean"))?;
        if enabled {
            image.display |= 8;
        } else {
            image.display &= !8;
        }
    }
    if let Some(v) = p.get("clipping") {
        image.clipping = v.as_bool().ok_or_else(|| bad("imageedit", "`clipping` must be boolean"))?;
    }
    if let Some(v) = p.get("clip") {
        image.clip = serde_json::from_value(v.clone()).map_err(|_| bad("imageedit", "`clip` must be an array of points"))?;
    }
    if let Some(v) = p.get("insert") {
        image.insert = serde_json::from_value(v.clone()).map_err(|_| bad("imageedit", "`insert` must be [x,y]"))?;
    }
    if let Some(width) = p.get("width").and_then(Value::as_f64) {
        if width <= 0.0 || !width.is_finite() {
            return Err(bad("imageedit", "`width` must be positive"));
        }
        let factor = width / (image.size.x * image.u.xy().len()).max(f64::MIN_POSITIVE);
        image.u = image.u * factor;
        if p.get("height").is_none() {
            image.v = image.v * factor;
        }
    }
    if let Some(height) = p.get("height").and_then(Value::as_f64) {
        if height <= 0.0 || !height.is_finite() {
            return Err(bad("imageedit", "`height` must be positive"));
        }
        let factor = height / (image.size.y * image.v.xy().len()).max(f64::MIN_POSITIVE);
        image.v = image.v * factor;
    }
    if let Some(deg) = p.get("rotation").and_then(Value::as_f64) {
        if !deg.is_finite() {
            return Err(bad("imageedit", "`rotation` must be finite"));
        }
        let angle = deg.to_radians() - image.u.y.atan2(image.u.x);
        let (sin, cos) = angle.sin_cos();
        let u = image.u.xy();
        let v = image.v.xy();
        image.u = Vec2::new(u.x * cos - u.y * sin, u.x * sin + u.y * cos).to3(0.0);
        image.v = Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos).to3(0.0);
    }
    s.doc_mut()?.modify_entity(h, |e| e.kind = EntityKind::Image(image))?;
    Ok(json!({"handle": h.hex()}))
}

/// Whether `ext` (lower case, no dot) is an image file type IMAGEATTACH reads.
pub(crate) fn is_image_ext(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "bmp" | "tif" | "tiff")
}

/// Every IMAGE entity of `d`: in model space, on the layouts and in the blocks.
fn images(d: &Drawing) -> impl Iterator<Item = &Image> {
    let model = std::iter::once(&d.model);
    let layouts = d.layouts.iter().map(|l| &l.entities);
    let blocks = d.blocks.values().map(|b| &b.entities);
    model.chain(layouts).chain(blocks).flat_map(|store| store.iter()).filter_map(|e| match &e.kind {
        EntityKind::Image(i) => Some(i),
        _ => None,
    })
}

/// The image files `d` names, each once, in the order they are first used.
fn paths(d: &Drawing) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut paths = Vec::new();
    for i in images(d) {
        if !i.path.trim().is_empty() && seen.insert(i.path.as_str()) {
            paths.push(i.path.clone());
        }
    }
    paths
}

/// Whether `path` is a full path here, or on Windows where a drawing may have been written (a
/// drive, `C:\maps\a.png`, or a share, `\\server\maps`), which other systems read as relative.
#[cfg(not(target_arch = "wasm32"))]
fn full_path(path: &str) -> bool {
    let b = path.trim().as_bytes();
    std::path::Path::new(path.trim()).is_absolute() || b.starts_with(b"\\\\") || matches!(b, [d, b':', b'\\' | b'/', ..] if d.is_ascii_alphabetic())
}

/// The contents of the image file saved as `path` in a drawing at `host`.
fn read_file(path: &str, host: Option<&str>) -> std::result::Result<Vec<u8>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = super::blocks::xref::find_file(path, host).ok_or_else(|| {
            // An unsaved drawing has no folder for a relative path to be relative to.
            if host.is_none() && !full_path(path) { "file not found (save the drawing first, or give a full path)" } else { "file not found" }
        })?;
        crate::file_access::check_read(&file)?;
        let len = std::fs::metadata(&file).map_err(|e| e.to_string())?.len();
        if len > MAX_IMAGE_BYTES {
            return Err(format!("the file is larger than {} MB", MAX_IMAGE_BYTES >> 20));
        }
        // Read no more than the limit: the file can grow or change after the check above.
        let limit = usize::try_from(MAX_IMAGE_BYTES).unwrap_or(usize::MAX);
        crate::capped_read::read_capped(&file, limit).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData { format!("the file is larger than {} MB", MAX_IMAGE_BYTES >> 20) } else { e.to_string() }
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (path, host);
        Err("files are not available on the web".into())
    }
}

/// Decode an image file. A decoder that panics on a damaged file fails this image only: the
/// drawing it belongs to must still open.
fn decode(bytes: &[u8]) -> std::result::Result<Raster, String> {
    let hook = super::file::image_decoder().ok_or("image files can't be read in this build")?;
    match std::panic::catch_unwind(|| hook(bytes)) {
        Ok(r) => r,
        Err(_) => Err("the file is damaged and can't be read".into()),
    }
}

fn bytes_of(r: &Raster) -> u64 {
    u64::try_from(r.rgba().len()).unwrap_or(u64::MAX)
}

/// Load the images of drawing `i`, just opened. Those that can't be loaded are reported and
/// stay crossed-out frames; opening never fails over an image.
pub(crate) fn after_open(s: &mut Session, i: usize) {
    let Some(st) = s.docs.get_mut(i) else { return };
    let paths = paths(&st.doc);
    if paths.is_empty() {
        return;
    }
    let mut rasters = st.doc.rasters.clone();
    let notes = load_missing(&paths, st.path.as_deref(), &mut rasters);
    if !rasters.is_empty() {
        // Loading pixels is not a change to the drawing. A drawing counts as unchanged while
        // it is the very one that was saved, so the saved one follows.
        let unchanged = Arc::ptr_eq(&st.doc, &st.saved);
        Arc::make_mut(&mut st.doc).rasters = rasters;
        if unchanged {
            st.saved = st.doc.clone();
        }
    }
    for n in notes {
        s.echo(n);
    }
}

/// Read and decode into `rasters` the files of `paths` that are not in it yet, for a drawing at
/// `host`. Returns one message per file that could not be loaded.
fn load_missing(paths: &[String], host: Option<&str>, rasters: &mut cadcraft_doc::Rasters) -> Vec<String> {
    let mut notes = Vec::new();
    let mut missing: Vec<&str> = paths.iter().map(String::as_str).filter(|p| rasters.get(p).is_none()).collect();
    if missing.len() > MAX_IMAGE_FILES {
        missing.truncate(MAX_IMAGE_FILES);
        notes.push(format!("Images: only the first {MAX_IMAGE_FILES} image files are loaded"));
    }
    let decoded = decode_files(&missing, host);
    // Kept in the order the drawing uses them, up to the memory limit.
    let mut kept_bytes = 0u64;
    for (path, result) in missing.iter().zip(decoded) {
        match result {
            Ok(r) => {
                kept_bytes = kept_bytes.saturating_add(bytes_of(&r));
                if kept_bytes > MAX_LOADED_BYTES {
                    notes.push(over_limit(path));
                    break;
                }
                rasters.insert(path, Arc::new(r));
            }
            Err(e) => notes.push(format!("Image {path}: {e}")),
        }
    }
    notes
}

fn over_limit(path: &str) -> String {
    format!("Image {path}: not loaded (more than {} MB of images)", MAX_LOADED_BYTES >> 20)
}

/// Read and decode the image files `paths` of a drawing at `host`, a few at a time (decoding is
/// what opening a drawing with large images waits for). The results are in the order of `paths`.
fn decode_files(paths: &[&str], host: Option<&str>) -> Vec<std::result::Result<Raster, String>> {
    // Decoded bytes so far, shared by the loaders: past the limit no further file is started.
    let budget = Budget::default();
    let load = |path: &str| -> std::result::Result<Raster, String> {
        let mut decoding = budget.start().ok_or_else(|| format!("not loaded (more than {} MB of images)", MAX_LOADED_BYTES >> 20))?;
        let r = read_file(path, host).and_then(|bytes| decode(&bytes))?;
        decoding.bytes = bytes_of(&r);
        Ok(r)
    };
    let mut results: Vec<Option<std::result::Result<Raster, String>>> = paths.iter().map(|_| None).collect();
    let loaders = loader_count(paths.len());
    if loaders > 1 {
        // Each loader takes the next file nobody has started.
        let next = AtomicUsize::new(0);
        // The automation roots are per thread, so they go along with each loader.
        let roots = crate::file_access::current();
        let work = || {
            crate::file_access::confine(roots.as_ref(), || {
                let mut done = Vec::new();
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(index) else { break };
                    done.push((index, load(path)));
                }
                done
            })
        };
        let finished: Vec<_> = std::thread::scope(|scope| {
            let started: Vec<_> = (0..loaders).filter_map(|_| std::thread::Builder::new().spawn_scoped(scope, work).ok()).collect();
            started.into_iter().filter_map(|loader| loader.join().ok()).flatten().collect()
        });
        for (index, result) in finished {
            if let Some(slot) = results.get_mut(index) {
                *slot = Some(result);
            }
        }
    }
    // One loader, or files a loader that could not start or finish left behind.
    results.into_iter().zip(paths).map(|(result, path)| result.unwrap_or_else(|| load(path))).collect()
}

/// The decoded bytes the loaders of one drawing share.
#[derive(Default)]
struct Budget {
    spent: Mutex<Spent>,
    /// Signaled when a file finishes decoding.
    finished: Condvar,
}

/// Decoded bytes so far, and how many files are being decoded.
#[derive(Default)]
struct Spent {
    decoded: u64,
    decoding: u64,
}

/// Whether a loader may start decoding a file.
#[derive(Debug, PartialEq)]
enum Admit {
    Start,
    /// Not until another file finishes.
    Wait,
    /// The images decoded so far are past the limit.
    Full,
}

impl Spent {
    /// A file starts while the images decoded so far, and the largest image each file being
    /// decoded can become, fit under `limit`. A file always starts when no other one is being
    /// decoded, so that the images run past `limit` by one image at most.
    fn admit(&mut self, limit: u64, largest: u64) -> Admit {
        if self.decoded > limit {
            return Admit::Full;
        }
        let reserved = self.decoding.saturating_add(1).saturating_mul(largest);
        if self.decoding > 0 && self.decoded.saturating_add(reserved) > limit {
            return Admit::Wait;
        }
        self.decoding = self.decoding.saturating_add(1);
        Admit::Start
    }

    /// A file finished decoding into `bytes` (0 when it failed).
    fn finish(&mut self, bytes: u64) {
        self.decoding = self.decoding.saturating_sub(1);
        self.decoded = self.decoded.saturating_add(bytes);
    }
}

impl Budget {
    /// Wait until a file may start decoding; `None` once the images decoded so far are past the
    /// limit.
    fn start(&self) -> Option<Decoding<'_>> {
        let mut spent = self.spent.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            match spent.admit(MAX_LOADED_BYTES, MAX_DECODED_BYTES) {
                Admit::Start => return Some(Decoding { budget: self, bytes: 0 }),
                Admit::Full => return None,
                Admit::Wait => spent = self.finished.wait(spent).unwrap_or_else(PoisonError::into_inner),
            }
        }
    }
}

/// A file being decoded. Dropping it counts `bytes` as decoded and lets a waiting loader check
/// again, also when the decoding failed.
struct Decoding<'a> {
    budget: &'a Budget,
    bytes: u64,
}

impl Drop for Decoding<'_> {
    fn drop(&mut self) {
        self.budget.spent.lock().unwrap_or_else(PoisonError::into_inner).finish(self.bytes);
        self.budget.finished.notify_all();
    }
}

/// How many files are decoded at once: one per processor, at most [`MAX_LOADERS`] (each large
/// image needs several hundred megabytes while it is decoded) and no more than there are files.
fn loader_count(files: usize) -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    let processors = std::thread::available_parallelism().map(std::num::NonZero::get).unwrap_or(1);
    #[cfg(target_arch = "wasm32")]
    let processors = 1;
    processors.min(MAX_LOADERS).min(files)
}

/// The file name without folders and extension (either separator).
fn stem(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match file.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => file.to_string(),
    }
}

/// The definition name for a new image of `path`: the name that file already has in `d`, else
/// `wanted`, numbered (`wanted_2`, `wanted_3`, …) when another file has taken it.
fn def_name(d: &Drawing, path: &str, wanted: &str) -> String {
    if let Some(i) = images(d).find(|i| i.path == path && !i.name.is_empty()) {
        return i.name.clone();
    }
    // The names other files have, compared without regard to case.
    let taken: BTreeSet<String> = images(d).filter(|i| i.path != path).map(|i| i.name.to_ascii_lowercase()).collect();
    let free = |name: &str| !taken.contains(&name.to_ascii_lowercase());
    if free(wanted) {
        return wanted.to_string();
    }
    // One of these is free: there are more candidates than names taken.
    let candidates = 2..taken.len().saturating_add(3);
    candidates.map(|n| format!("{wanted}_{n}")).find(|name| free(name)).unwrap_or_else(|| wanted.to_string())
}

const ID: &str = "imageattach";

pub(crate) fn run_imageattach(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").map(str::trim).filter(|x| !x.is_empty()).ok_or_else(|| bad(ID, "`path` is required"))?.to_string();
    if path.len() > MAX_PATH {
        return Err(bad(ID, "`path` is too long"));
    }
    let mut place = placement(p)?;
    let raster = pixels(s, p, &path)?;
    let (w, h) = (f64::from(raster.width()), f64::from(raster.height()));
    if let Some(value) = p.get("dpi") {
        let fallback = value.as_f64().filter(|x| x.is_finite() && *x > 0.0).ok_or_else(|| bad(ID, "`dpi` must be positive"))?;
        let auto = p.get("autoDpi").and_then(Value::as_bool).unwrap_or(false);
        let dpi = if auto { raster.dpi().map_or(fallback, |v| v[0]) } else { fallback };
        if auto {
            s.echo(format!("Image resolution: {dpi:.2} DPI ({})", if raster.dpi().is_some() { "file metadata" } else { "fallback" }));
        }
        let unit_mm = p
            .get("unitMm")
            .map_or(Some(1.0), Value::as_f64)
            .filter(|x| x.is_finite() && *x > 0.0)
            .ok_or_else(|| bad(ID, "`unitMm` must be positive"))?;
        if p.get("width").is_some() || p.get("scale").is_some() {
            return Err(bad(ID, "use dpi or width/scale, not both"));
        }
        place.width = w / dpi * 25.4 / unit_mm;
    }
    // One pixel in drawing units, along the image's bottom edge (`u`) and its left edge (`v`).
    let pixel = place.width / w;
    let u = Vec2::from_angle(place.rotation) * pixel;
    let v = u.perp();
    if !(pixel.is_normal() && (pixel * h).is_normal() && u.x.is_finite() && u.y.is_finite()) {
        return Err(bad(ID, "the image would be too small or too large to draw"));
    }
    let wanted = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| stem(&path));
    let name = def_name(s.doc()?, &path, &wanted);
    let image = Image {
        insert: place.at.to3(0.0),
        u: u.to3(0.0),
        v: v.to3(0.0),
        size: Vec2::new(w, h),
        path: path.clone(),
        name: name.clone(),
        ..Image::default()
    };
    s.doc_mut()?.rasters.insert(&path, raster);
    let handle = s.add_entity(EntityKind::Image(image))?;
    Ok(json!({ "name": name, "handle": handle.hex(), "path": path, "pixels": [w, h], "size": [place.width, pixel * h] }))
}

/// Where and how large IMAGEATTACH puts the image.
struct Placement {
    /// The lower-left corner.
    at: Vec2,
    /// Radians, counter-clockwise.
    rotation: f64,
    /// The image's width in drawing units.
    width: f64,
}

/// `at`, `rotation` and the width (`width`, else `scale`: an image is one unit wide at scale 1).
fn placement(p: &Value) -> Result<Placement> {
    let at = point_param(p, "at").unwrap_or(Vec2::ZERO);
    if !(at.x.is_finite() && at.y.is_finite()) {
        return Err(bad(ID, "`at` must be a point"));
    }
    let width = match p.get("width") {
        Some(v) => v.as_f64().filter(|w| w.is_finite()).ok_or_else(|| bad(ID, "`width` must be a number"))?,
        None => f64_or(p, "scale", 1.0),
    };
    if !(width.is_finite() && width > 0.0) {
        return Err(bad(ID, "`scale` and `width` must be greater than zero"));
    }
    Ok(Placement { at, rotation: f64_or(p, "rotation", 0.0).to_radians(), width })
}

/// The pixels of the image at `path`: decoded from `data` when the caller hands the file over,
/// else the ones already loaded for that path, else read from the file.
fn pixels(s: &Session, p: &Value, path: &str) -> Result<Arc<Raster>> {
    let bytes = match p.get("data") {
        Some(data) => {
            let text = data.as_str().ok_or_else(|| bad(ID, "`data` must be base64 text"))?;
            // Refused before decoding, which would hold all of it in memory. Line breaks and
            // spaces, which decoding skips, don't count.
            let len = u64::try_from(text.len()).unwrap_or(u64::MAX);
            let digits = || u64::try_from(text.bytes().filter(|b| !matches!(b, b'\n' | b'\r' | b' ')).count()).unwrap_or(u64::MAX);
            if len > MAX_IMAGE_TEXT || (len > MAX_IMAGE_BASE64 && digits() > MAX_IMAGE_BASE64) {
                return Err(bad(ID, format!("`data` is larger than {} MB", MAX_IMAGE_BYTES >> 20)));
            }
            let bytes = base64_decode(text).ok_or_else(|| bad(ID, "`data` is not valid base64"))?;
            // The longest text allowed can still hold a byte or two more than the limit.
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_IMAGE_BYTES {
                return Err(bad(ID, format!("`data` is larger than {} MB", MAX_IMAGE_BYTES >> 20)));
            }
            bytes
        }
        None => {
            if let Some(loaded) = s.doc()?.rasters.get(path) {
                return Ok(loaded.clone());
            }
            let host = s.state()?.path.clone();
            read_file(path, host.as_deref()).map_err(|e| bad(ID, format!("{path}: {e}")))?
        }
    };
    Ok(Arc::new(decode(&bytes).map_err(|e| bad(ID, format!("{path}: {e}")))?))
}

fn run_image(s: &mut Session, _p: &Value) -> Result<Value> {
    let host = s.state()?.path.clone();
    let d = s.doc()?;
    // Per file: its definition name and how many images show it.
    let mut uses: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    for i in images(d) {
        uses.entry(i.path.as_str()).or_insert((i.name.as_str(), 0)).1 += 1;
    }
    let rows: Vec<Value> = paths(d)
        .iter()
        .map(|path| {
            let (name, references) = uses.get(path.as_str()).copied().unwrap_or_default();
            let mut row = describe(d, path, host.as_deref());
            row["name"] = json!(name);
            row["references"] = json!(references);
            row
        })
        .collect();
    Ok(json!({ "images": rows }))
}

/// The state of image file `path` of drawing `d` (saved at `host`): where it is and whether its
/// pixels are loaded.
fn describe(d: &Drawing, path: &str, host: Option<&str>) -> Value {
    let loaded = d.rasters.get(path);
    let found_at = super::blocks::xref::find_file(path, host).map(|f| f.to_string_lossy().into_owned());
    let status = match (loaded, &found_at) {
        (Some(_), _) => "Loaded",
        (None, None) => "Not Found",
        // The file is there but could not be read as an image.
        (None, Some(_)) => "Not loaded",
    };
    json!({ "path": path, "foundAt": found_at, "status": status, "pixels": loaded.map(|r| json!([r.width(), r.height()])) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file starts only while the largest image each file being decoded can become still fits,
    /// one file always starts when none is being decoded, and none starts past the limit.
    #[test]
    fn loaders_start_files_within_the_limit() {
        let mut s = Spent::default();
        // Room for two of the largest images.
        assert_eq!(s.admit(10, 4), Admit::Start);
        assert_eq!(s.admit(10, 4), Admit::Start);
        assert_eq!(s.admit(10, 4), Admit::Wait);
        s.finish(3);
        assert_eq!(s.admit(10, 4), Admit::Wait, "3 decoded and two files that can become 4 each are past 10");
        s.finish(5);
        // Over the limit only by the one file being decoded.
        assert_eq!(s.admit(10, 4), Admit::Start);
        s.finish(4);
        assert_eq!(s.admit(10, 4), Admit::Full);
        // A failed file counts nothing.
        let mut s = Spent::default();
        assert_eq!(s.admit(10, 4), Admit::Start);
        s.finish(0);
        assert_eq!((s.decoded, s.decoding), (0, 0));
    }

    /// Files that fail still let the other loaders go on: every file gets a result.
    #[test]
    fn decode_files_finishes_when_every_file_fails() {
        let paths = ["a.png", "b.png", "c.png", "d.png", "e.png", "f.png"];
        let results = decode_files(&paths, None);
        assert_eq!(results.len(), paths.len());
        assert!(results.iter().all(std::result::Result::is_err), "none of these files exists");
    }
}
