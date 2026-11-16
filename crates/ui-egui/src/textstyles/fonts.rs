//! User-selected fonts stay in CADCraft's own folder; document edits remain engine commands.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Bound input before parsing or copying a user-selected font.
const MAX_FONT_BYTES: u64 = 32 << 20;

fn managed_directory() -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
    {
        std::env::var_os("APPDATA").map(PathBuf::from).map(|p| p.join("CADCraft/fonts"))
    }
    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    {
        std::env::var_os("HOME").map(PathBuf::from).map(|p| p.join("Library/Application Support/CADCraft/fonts"))
    }
    #[cfg(all(not(target_arch = "wasm32"), not(any(target_os = "windows", target_os = "macos"))))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from).map(|p| p.join(".local/share")))
            .map(|p| p.join("CADCraft/fonts"))
    }
}

/// Register the managed folder before opening drawings, also after an application restart.
pub(crate) fn initialize_fonts() {
    if let Some(dir) = managed_directory().filter(|p| p.is_dir()) {
        let _ = activate_directory(&dir);
    }
}

fn activate_directory(dir: &Path) -> Result<(), String> {
    let path = dir.to_str().ok_or("Font folder is not valid Unicode")?;
    let mut dirs = cadcraft_fonts::ttf::font_dirs();
    if !dirs.iter().any(|p| p == path) {
        if dirs.len() >= cadcraft_fonts::ttf::MAX_FONT_DIRS {
            return Err("The font folder limit has been reached".into());
        }
        dirs.push(path.to_string());
        cadcraft_fonts::ttf::set_font_dirs(&dirs);
    } else {
        cadcraft_fonts::ttf::rescan();
    }
    Ok(())
}

pub(super) fn import_font(path: &str) -> Result<String, String> {
    let dir = managed_directory().ok_or("Font import is available in the desktop application")?;
    let name = import_into(Path::new(path.trim().trim_matches('"')), &dir)?;
    activate_directory(&dir)?;
    Ok(name)
}

fn font_bytes(src: &Path) -> Result<Vec<u8>, String> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !matches!(ext.as_str(), "ttf" | "otf" | "shx") {
        return Err("Choose a .ttf, .otf or .shx file".into());
    }
    let metadata = std::fs::metadata(src).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_FONT_BYTES {
        return Err("Choose a regular font file no larger than 32 MiB".into());
    }
    let file = std::fs::File::open(src).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_FONT_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FONT_BYTES {
        return Err("Font file exceeds 32 MiB".into());
    }
    let valid = if ext == "shx" {
        cadcraft_fonts::shx::ShxFont::parse(&bytes).is_ok_and(|font| font.is_font())
    } else {
        cadcraft_fonts::ttf::Face::new(Arc::new(bytes.clone()), 0).is_some()
    };
    if !valid {
        return Err("The selected file is not a supported font".into());
    }
    Ok(bytes)
}

/// A file name, rather than an arbitrary path, is how the font lookup APIs resolve a style.
/// Never overwrite a previously imported font: styles may still reference it.
fn import_into(src: &Path, dir: &Path) -> Result<String, String> {
    let bytes = font_bytes(src)?;
    let name = src.file_name().and_then(|s| s.to_str()).ok_or("Invalid font filename")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = dir.join(name);
    if src.canonicalize().ok() == dest.canonicalize().ok() && dest.is_file() {
        return Ok(name.to_string());
    }
    let stem = src.file_stem().and_then(|s| s.to_str()).ok_or("Invalid font filename")?;
    let ext = src.extension().and_then(|s| s.to_str()).ok_or("Invalid font extension")?;
    for suffix in 0..1000 {
        let name = if suffix == 0 { name.to_string() } else { format!("{stem}-{suffix}.{ext}") };
        let dest = dir.join(&name);
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&dest) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        if let Err(e) = file.write_all(&bytes) {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(e.to_string());
        }
        return Ok(name);
    }
    Err("Too many imported fonts with the same name".into())
}

pub(super) fn available_fonts(ctx: &egui::Context) -> Arc<Vec<String>> {
    let id = egui::Id::unique("text-style-font-catalog");
    let generation = (cadcraft_fonts::ttf::generation(), cadcraft_fonts::shx::generation());
    type Catalog = ((u64, u64), Arc<Vec<String>>);
    if let Some((_, fonts)) = ctx.data(|data| data.get_temp::<Catalog>(id)).filter(|(key, _)| *key == generation) {
        return fonts;
    }
    let mut fonts = cadcraft_fonts::ttf::available_fonts();
    for stem in cadcraft_fonts::shx::installed() {
        let name = format!("{stem}.shx");
        if !fonts.iter().any(|font| font.eq_ignore_ascii_case(&name)) {
            fonts.push(name);
        }
    }
    let fonts = Arc::new(fonts);
    ctx.data_mut(|data| data.insert_temp(id, (generation, fonts.clone())));
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!("cadcraft-font-import-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn source(dir: &Temp) -> (PathBuf, Vec<u8>) {
        let bytes = cadcraft_fonts::shx::compile("*0,4,import test\n6,2,0,0\n*65,4,UCA\n1,064,0\n").unwrap();
        let src = dir.0.join("import-test.shx");
        std::fs::write(&src, &bytes).unwrap();
        (src, bytes)
    }

    #[test]
    fn importing_preserves_existing_fonts_and_the_source() {
        let temp = Temp::new();
        let (src, bytes) = source(&temp);
        let dest = temp.0.join("managed");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("import-test.shx"), b"existing font").unwrap();
        let name = import_into(&src, &dest).unwrap();
        assert_eq!(name, "import-test-1.shx");
        assert_eq!(std::fs::read(dest.join("import-test.shx")).unwrap(), b"existing font");
        assert_eq!(std::fs::read(&src).unwrap(), bytes);
        assert_eq!(std::fs::read(dest.join(&name)).unwrap(), bytes);
        assert_eq!(import_into(&dest.join(&name), &dest).unwrap(), name);
    }

    #[test]
    fn imported_font_resolves_after_the_directory_is_activated_again() {
        let temp = Temp::new();
        let (src, _) = source(&temp);
        let dest = temp.0.join("managed");
        let name = import_into(&src, &dest).unwrap();
        activate_directory(&dest).unwrap();
        assert!(cadcraft_fonts::TextFont::resolve(&name).is_shx());
        cadcraft_fonts::ttf::rescan();
        activate_directory(&dest).unwrap();
        assert!(cadcraft_fonts::TextFont::resolve(&name).is_shx());
        // Where a system TrueType file exists, also exercise an actual outline font
        // under a unique imported name. No font binary is bundled with this test.
        let outline = [
            "C:/Windows/Fonts/arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
        ]
        .into_iter()
        .map(Path::new)
        .find(|p| p.is_file());
        if let Some(outline) = outline {
            let src = temp.0.join("cadcraft-import-outline-test.ttf");
            std::fs::copy(outline, &src).unwrap();
            let name = import_into(&src, &dest).unwrap();
            activate_directory(&dest).unwrap();
            assert!(cadcraft_fonts::ttf::find(&name).is_some());
            cadcraft_fonts::ttf::rescan();
            assert!(cadcraft_fonts::ttf::find(&name).is_some());
        }
        // Remove only this test's folder from the process-wide configuration.
        let path = dest.to_str().unwrap();
        let dirs: Vec<_> = cadcraft_fonts::ttf::font_dirs().into_iter().filter(|d| d != path).collect();
        cadcraft_fonts::ttf::set_font_dirs(&dirs);
    }

    #[test]
    fn invalid_and_oversized_fonts_are_rejected_before_copying() {
        let temp = Temp::new();
        let dest = temp.0.join("managed");
        for name in ["bad.ttf", "bad.otf", "bad.shx", "bad.txt"] {
            let src = temp.0.join(name);
            std::fs::write(&src, b"not a font").unwrap();
            assert!(import_into(&src, &dest).is_err());
        }
        let src = temp.0.join("large.ttf");
        std::fs::File::create(&src).unwrap().set_len(MAX_FONT_BYTES + 1).unwrap();
        assert!(import_into(&src, &dest).is_err());
        assert!(!dest.exists());
    }
}
