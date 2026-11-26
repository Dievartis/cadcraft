//! Browser uploads: File > Open… on the web, through web-sys only. There is no file system to pick
//! a path from, so the picker is a hidden `<input type="file">`; the chosen file (or one dropped on
//! the page) is read in the background and handed to the app on its next frame, which opens it
//! with OPEN `{data, name}` ([`CadApp::open_bytes`]).
//!
//! SHX fonts (text fonts, big fonts, shape files) chosen or dropped with a drawing are the web
//! build's font folder: they are registered with the font table
//! ([`cadcraft_fonts::shx::register`]) under their file name, so the styles and linetypes that
//! name them draw with them, and the drawings are regenerated.

use std::cell::Cell;
use std::rc::Rc;

use cadcraft_ui_egui::CadApp;
use wasm_bindgen::JsCast as _;
use web_sys::js_sys;

/// The hidden file input (one at a time; a cancelled pick leaves it until the next one).
const INPUT_ID: &str = "cadcraft_open_input";
/// What the picker offers: the formats OPEN reads, and the SHX fonts drawings name.
const ACCEPT: &str = ".dxf,.dwg,.shx,.pdf";
/// What PDFATTACH's picker offers: one PDF.
const ACCEPT_PDF: &str = ".pdf";
/// What the Title Block dialog's picker offers: the pictures it reads.
const ACCEPT_IMAGE: &str = ".png,.jpg,.jpeg,.bmp,.tif,.tiff";

/// Most files taken from one pick.
const MAX_PICKED: u32 = 64;
/// Most bytes being read at once, all files together: a large pick must not exhaust the page's
/// memory (each file is copied once more into the app).
const MAX_READING_BYTES: u64 = 512 << 20;

/// Apply the PDF byte limit before reading or copying files from any browser source.
pub fn size_error(name: &str, len: u64) -> Option<String> {
    if is_pdf(name) && len > cadcraft_engine::cmd::MAX_PDF_BYTES as u64 {
        Some(format!("{name}: PDF exceeds the 64 MiB input limit"))
    } else {
        CadApp::open_size_error(name, len)
    }
}

/// Placement supplied by the host; picked PDF files use the command's defaults.
pub struct PdfPlacement {
    pub page: u32,
    pub insertion: [f64; 3],
    pub width: f64,
    pub height: f64,
}

/// A file read in the browser: its name, its contents or why it couldn't be read, whether the
/// page embedding CADCraft sent it (that page is told when it doesn't open), and the placement
/// of a PDF page the host asked to attach.
struct Upload {
    name: String,
    bytes: Result<Vec<u8>, String>,
    from_host: bool,
    pdf: Option<PdfPlacement>,
}

impl Upload {
    fn picked(name: String, bytes: Result<Vec<u8>, String>) -> Self {
        Upload { name, bytes, from_host: false, pdf: None }
    }
}

/// Files read in the background, waiting for the app's next frame.
#[derive(Clone)]
pub struct Inbox {
    files: Rc<Cell<Vec<Upload>>>,
    /// Bytes of the files being read now.
    reading: Rc<Cell<u64>>,
    ctx: egui::Context,
}

impl Inbox {
    pub fn new(ctx: &egui::Context) -> Self {
        Inbox { files: Rc::default(), reading: Rc::default(), ctx: ctx.clone() }
    }

    /// Show the browser's file picker and return at once. A chosen file arrives through
    /// [`Self::drain`]; cancelling the picker does nothing.
    pub fn pick(&self) {
        if let Err(e) = self.show_picker(ACCEPT, true) {
            self.push(Upload::picked(String::new(), Err(format!("Open: the browser's file picker is not available ({e})"))));
        }
    }

    /// Show the picker of the Title Block dialog's pictures; the file chosen reaches the dialog through [`Self::drain`].
    pub fn pick_image(&self) {
        if let Err(e) = self.show_picker(ACCEPT_IMAGE, false) {
            self.push(Upload::picked(String::new(), Err(format!("Title block: the browser's file picker is not available ({e})"))));
        }
    }

    /// Show the picker of PDFATTACH, limited to one PDF; the file chosen is attached under the drawing (`attach_pdf`) when it arrives through
    /// [`Self::drain`], as one that was opened.
    pub fn pick_pdf(&self) {
        if let Err(e) = self.show_picker(ACCEPT_PDF, false) {
            self.push(Upload::picked(String::new(), Err(format!("PDF underlay: the browser's file picker is not available ({e})"))));
        }
    }

    fn show_picker(&self, accept: &str, multiple: bool) -> Result<(), String> {
        let document = web_sys::window().and_then(|w| w.document()).ok_or("no browser document")?;
        let body = document.body().ok_or("the page has no body")?;
        if let Some(old) = document.get_element_by_id(INPUT_ID) {
            old.remove();
        }
        let input = document
            .create_element("input")
            .map_err(js_error)?
            .dyn_into::<web_sys::HtmlInputElement>()
            .map_err(|_| "could not create a file input".to_string())?;
        input.set_type("file");
        input.set_accept(accept);
        // A drawing and the fonts it uses can be chosen together; a PDF underlay is one file.
        input.set_multiple(multiple);
        input.set_id(INPUT_ID);
        input.set_attribute("style", "display:none").map_err(js_error)?;
        let (inbox, chosen) = (self.clone(), input.clone());
        // Called once, when files were chosen (browsers don't fire `change` on cancel).
        let on_change = wasm_bindgen::closure::Closure::once_into_js(move || {
            if let Some(files) = chosen.files() {
                for i in 0..files.length().min(MAX_PICKED) {
                    if let Some(file) = files.get(i) {
                        inbox.read(file);
                    }
                }
                let skipped: Vec<String> = (MAX_PICKED..files.length()).filter_map(|i| files.get(i)).map(|f| f.name()).collect();
                if !skipped.is_empty() {
                    inbox.push(Upload::picked(
                        String::new(),
                        Err(format!("Open: at most {MAX_PICKED} files at a time; not opened: {}", skipped.join(", "))),
                    ));
                }
            }
            chosen.remove();
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        body.append_child(&input).map_err(js_error)?;
        input.click();
        Ok(())
    }

    /// Read the files dropped on the page this frame (eframe hands them over unread on the web).
    /// The list is taken, not cloned: a later pass or hidden tick must not read the same drop.
    pub fn take_dropped(&self, ctx: &egui::Context) {
        for f in ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files)) {
            if let Some(file) = f.web_file() {
                self.read(file.clone());
            }
        }
    }

    /// Read `file` in the background; it arrives through [`Self::drain`]. A file too large to
    /// open is refused before it is read.
    fn read(&self, file: web_sys::File) {
        let name = file.name();
        // `File.size` is a whole number of bytes; anything else counts as too large.
        let size = file.size();
        let len = if size.is_finite() && size >= 0.0 { size as u64 } else { u64::MAX };
        if let Some(e) = size_error(&name, len) {
            return self.push(Upload::picked(name, Err(e)));
        }
        let reading = self.reading.get();
        if reading.saturating_add(len) > MAX_READING_BYTES {
            return self.push(Upload::picked(
                name.clone(),
                Err(format!("Open: {name} was not read: the files being read together are too large; open it on its own")),
            ));
        }
        self.reading.set(reading.saturating_add(len));
        let inbox = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let bytes = match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                Ok(buf) => Ok(js_sys::Uint8Array::new(&buf).to_vec()),
                Err(e) => Err(format!("Open: could not read {name}: {}", js_error(e))),
            };
            inbox.reading.set(inbox.reading.get().saturating_sub(len));
            inbox.push(Upload::picked(name, bytes));
        });
    }

    /// A file sent by the page embedding CADCraft: opened on the next frame like a picked file, or
    /// refused when too large to open. Either way that page is told if it doesn't open.
    pub fn receive(&self, name: String, bytes: Vec<u8>) {
        let bytes = match size_error(&name, bytes.len() as u64) {
            Some(e) => Err(e),
            None => Ok(bytes),
        };
        self.push(Upload { name, bytes, from_host: true, pdf: None });
    }

    /// A PDF page the host asked to attach, kept in arrival order with opened drawings and
    /// attached on the next frame.
    pub fn attach_pdf(&self, name: String, bytes: Vec<u8>, placement: PdfPlacement) {
        self.push(Upload { name, bytes: Ok(bytes), from_host: true, pdf: Some(placement) });
    }

    /// A file the embedding page sent, refused before it was read (`error` says why).
    pub fn refuse(&self, name: String, error: String) {
        self.push(Upload { name, bytes: Err(error), from_host: true, pdf: None });
    }

    fn push(&self, upload: Upload) {
        let mut files = self.files.take();
        files.push(upload);
        self.files.set(files);
        self.ctx.request_repaint();
    }

    /// Open what arrived since the last frame (or report why it couldn't be read). Fonts are
    /// registered first, so drawings opened with them draw with them. A drawing the embedding page
    /// sent that doesn't open is reported to that page as well.
    pub fn drain(&self, app: &mut CadApp) {
        let (fonts, files): (Vec<Upload>, Vec<Upload>) =
            self.files.take().into_iter().partition(|u| u.bytes.is_ok() && u.pdf.is_none() && is_shx(&u.name));
        let mut registered = false;
        for font in fonts {
            let Ok(bytes) = font.bytes else { continue };
            match cadcraft_fonts::shx::register(&font.name, &bytes) {
                Ok(()) => registered = true,
                Err(e) => app.open_failed(format!("{}: {e}", font.name)),
            }
        }
        if registered {
            let _ = app.run("regenall", serde_json::json!({}));
        }
        for upload in files {
            // A picture chosen for the Title Block dialog is not a drawing to open.
            if let Ok(bytes) = &upload.bytes
                && !is_pdf(&upload.name)
                && upload.pdf.is_none()
                && app.title_picture_arrived(&upload.name, bytes)
            {
                continue;
            }
            let opened = match upload.bytes {
                Ok(bytes) if upload.pdf.is_some() || is_pdf(&upload.name) => attach_pdf(app, &upload.name, &bytes, upload.pdf),
                Ok(bytes) => app.open_bytes(&upload.name, &bytes),
                Err(e) => {
                    app.open_failed(e.clone());
                    Err(e)
                }
            };
            if let (true, Err(e)) = (upload.from_host, opened) {
                crate::host::post_open_failed(&upload.name, &e);
            }
        }
    }
}

/// Whether `name` is an SHX font or shape file.
fn is_shx(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".shx")
}

fn is_pdf(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".pdf")
}

/// Attach a PDF's page under the drawing (PDFATTACH), at the host's placement or the command's
/// defaults. A host's attachment that fails is told to the host like a drawing that didn't open.
fn attach_pdf(app: &mut CadApp, name: &str, bytes: &[u8], placement: Option<PdfPlacement>) -> Result<(), String> {
    if bytes.len() > cadcraft_engine::cmd::MAX_PDF_BYTES {
        let e = format!("{name}: PDF exceeds the 64 MiB input limit");
        app.open_failed(e.clone());
        return Err(e);
    }
    let mut params = serde_json::json!({ "name": name, "data": cadcraft_engine::cmd::file::base64_encode(bytes) });
    // Chosen in the browser without a placement, the PDF takes the place of the drawing's one PDF page (a revised plan).
    if placement.is_none() {
        params["replace"] = true.into();
    }
    if let Some(p) = placement {
        params["page"] = p.page.into();
        params["insertion"] = serde_json::json!(p.insertion);
        params["width"] = p.width.into();
        params["height"] = p.height.into();
    }
    // RUN shows its own error on the command line.
    app.run("pdfattach", params).map(|_| ())
}

fn js_error(e: wasm_bindgen::JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}
