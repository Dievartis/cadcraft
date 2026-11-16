//! SVG export of a space (vector, for the web and documents).

use cadcraft_color::Rgb;
use cadcraft_doc::{Drawing, Space};
use cadcraft_geom::Bounds2;
use cadcraft_render::Kind;

/// Export with a white background (colour 7 prints black), fitted to the extents.
pub fn export(d: &Drawing, space: &Space) -> String {
    export_window(d, space, None)
}

/// Export showing exactly `window` (the `viewBox`; geometry outside it is clipped by the viewer),
/// or the extents with a 2% margin when `window` is `None`. Objects are drawn with their
/// transparency, as on screen.
pub fn export_window(d: &Drawing, space: &Space, window: Option<Bounds2>) -> String {
    export_window_with(d, space, window, true)
}

/// [`export_window`], drawing transparency only when `transparency` is on (plots).
pub fn export_window_with(d: &Drawing, space: &Space, window: Option<Bounds2>, transparency: bool) -> String {
    export_window_from(d, space, window, transparency, None)
}

/// [`export_window_with`] for the drawing saved at `document` (PDF underlays' relative paths
/// are found from its folder). PDF underlays' pages are drawn as images, about 3200 pixels
/// across the view.
pub(crate) fn export_window_from(d: &Drawing, space: &Space, window: Option<Bounds2>, transparency: bool, document: Option<&str>) -> String {
    let list = cadcraft_render::build(d, space, &cadcraft_render::Options { transparency, ..cadcraft_render::Options::default() });
    let b = window.filter(|w| !w.is_empty()).unwrap_or_else(|| crate::underlay_plot::plotted_bounds(&list));
    let px_per_unit = 3200.0 / b.width().max(b.height()).max(1e-9);
    let images = crate::underlay_plot::images(d, &list, document, px_per_unit, None);
    export_list_over(&list, window, &images)
}

/// Opacity for a transparent primitive (none for an opaque one or a wipeout's mask).
fn opacity(p: &cadcraft_render::DPrim) -> Option<String> {
    (p.kind != Kind::Mask && p.transparency != 0).then(|| format!("{:.2}", f64::from(p.alpha()) / 255.0))
}

/// Write a display list (as [`export_window`] does for a space's own).
pub fn export_list(list: &cadcraft_render::DisplayList, window: Option<Bounds2>) -> String {
    export_list_over(list, window, &[])
}

/// [`export_list`] with PDF underlays' pages `images`, each drawn before the primitive it comes
/// before.
pub(crate) fn export_list_over(list: &cadcraft_render::DisplayList, window: Option<Bounds2>, images: &[crate::underlay_plot::PlotImage]) -> String {
    let window = window.filter(|w| !w.is_empty());
    let b = window.unwrap_or_else(|| crate::underlay_plot::plotted_bounds(list));
    // A single point (or a zero-size window) has no extent: show a unit box centred on it.
    let point = !b.is_empty() && b.width().max(b.height()) <= 1e-9;
    let (w, h) = if b.is_empty() {
        (1.0, 1.0)
    } else if point {
        (0.0, 0.0)
    } else {
        (b.width().max(1e-9), b.height().max(1e-9))
    };
    let m = if point {
        0.5
    } else if window.is_some() {
        0.0
    } else {
        (w.max(h)) * 0.02
    };
    let (x0, y1) = if b.is_empty() { (0.0, 1.0) } else { (b.min.x - m, b.max.y + m) };
    let vw = w + 2.0 * m;
    let vh = h + 2.0 * m;
    let stroke = vw.max(vh) / 1500.0;
    // Decimals: at least 6, more when the stroke would otherwise lose its leading digits.
    let dp = (3.0 - stroke.log10().floor()).clamp(6.0, 15.0) as usize;
    use craft_svg::write::{SvgWriter, num};
    let mut writer = SvgWriter::fragment();
    writer
        .root()
        .attr("viewBox", format!("0 0 {} {}", num(vw, dp), num(vh, dp)))
        .attr("width", format!("{:.0}", 1600.0))
        .attr("height", format!("{:.0}", 1600.0 * vh / vw))
        .open();
    writer.tag("rect").attr("width", "100%").attr("height", "100%").attr("fill", "white").empty();
    let white = Rgb(255, 255, 255);
    let tx = |x: f64| x - x0;
    let ty = |y: f64| y1 - y;
    let mut images: Vec<&crate::underlay_plot::PlotImage> = images.iter().collect();
    images.sort_by_key(|im| im.at);
    let mut images = images.into_iter().enumerate().peekable();
    let image = |n: usize, im: &crate::underlay_plot::PlotImage| -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let Some(png) = cadcraft_render::raster::pixels_png(im.width, im.height, &im.rgb, &im.alpha) else { return s };
        // The unit square (its top row at y = 0) onto the page's parallelogram, in SVG units.
        let top = im.origin + im.v;
        let m = [im.u.x, -im.u.y, -im.v.x, im.v.y, tx(top.x), ty(top.y)];
        if m.iter().any(|v| !v.is_finite()) {
            return s;
        }
        // Seen through a viewport: clipped to it.
        let clipped = !im.clip.is_empty();
        if clipped {
            let _ = write!(s, r#"<clipPath id="cc-underlay-{n}"><path d=""#);
            for t in im.clip.chunks(3) {
                if let [a, b2, cc] = t {
                    let _ = write!(s, "M{:.dp$} {:.dp$}L{:.dp$} {:.dp$}L{:.dp$} {:.dp$}Z", tx(a.x), ty(a.y), tx(b2.x), ty(b2.y), tx(cc.x), ty(cc.y));
                }
            }
            let _ = write!(s, r#""/></clipPath><g clip-path="url(#cc-underlay-{n})">"#);
        }
        let _ = write!(
            s,
            r#"<image width="1" height="1" preserveAspectRatio="none" transform="matrix({:.dp$} {:.dp$} {:.dp$} {:.dp$} {:.dp$} {:.dp$})" href="data:image/png;base64,{}"/>"#,
            m[0],
            m[1],
            m[2],
            m[3],
            m[4],
            m[5],
            base64(&png)
        );
        if clipped {
            s.push_str("</g>");
        }
        s
    };
    for (i, p) in list.prims.iter().enumerate() {
        while let Some((n, im)) = images.next_if(|(_, im)| im.at <= i) {
            writer.raw(&image(n, im));
        }
        let c = p.display_rgb(white).hex();
        let op = opacity(p);
        let pts = list.points(p);
        match p.kind {
            Kind::Polyline => {
                let points = craft_svg::drawing_write::points(pts.iter().map(|q| [tx(q.x), ty(q.y)]), dp);
                let mut tag = writer.tag("polyline").attr("fill", "none").attr("stroke", &c);
                if let Some(op) = &op {
                    tag = tag.attr("opacity", op);
                }
                tag.attr("stroke-width", num(stroke, dp))
                    .attr("stroke-linecap", "round")
                    .attr("stroke-linejoin", "round")
                    .attr("points", points)
                    .empty();
            }
            Kind::Tris | Kind::Mask => {
                let c = if p.kind == Kind::Mask { white.hex() } else { c };
                let triangles = pts
                    .chunks(3)
                    .filter_map(|t| if let [a, b, c] = t { Some([[tx(a.x), ty(a.y)], [tx(b.x), ty(b.y)], [tx(c.x), ty(c.y)]]) } else { None });
                let path = craft_svg::drawing_write::triangles(triangles, dp);
                let mut tag = writer.tag("path").attr("fill", &c);
                if let Some(op) = &op {
                    tag = tag.attr("opacity", op);
                }
                tag.attr("d", path).empty();
            }
            Kind::Image { .. } => {}
            Kind::Point => {
                if let Some(q) = pts.first() {
                    let mut tag =
                        writer.tag("circle").attr("cx", num(tx(q.x), dp)).attr("cy", num(ty(q.y), dp)).attr("r", num(stroke, dp)).attr("fill", &c);
                    if let Some(op) = &op {
                        tag = tag.attr("opacity", op);
                    }
                    tag.empty();
                }
            }
            Kind::Infinite { ray } => {
                if let (Some(bp), Some(dir)) = (pts.first(), pts.get(1)) {
                    let far = (vw + vh) * 2.0 + if window.is_some() { bp.dist(b.center()) } else { 0.0 };
                    let a = if ray { *bp } else { *bp - *dir * far };
                    let e = *bp + *dir * far;
                    let mut tag = writer
                        .tag("line")
                        .attr("x1", num(tx(a.x), dp))
                        .attr("y1", num(ty(a.y), dp))
                        .attr("x2", num(tx(e.x), dp))
                        .attr("y2", num(ty(e.y), dp))
                        .attr("stroke", &c);
                    if let Some(op) = &op {
                        tag = tag.attr("opacity", op);
                    }
                    tag.attr("stroke-width", num(stroke, dp)).empty();
                }
            }
        }
    }
    // Pages drawn after the last primitive.
    for (n, im) in images {
        writer.raw(&image(n, im));
    }
    writer.close();
    writer.finish()
}

/// `bytes` in base64 (RFC 4648, with padding), for a data URL.
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let sym = |i: u32| char::from(ABC.get((i & 63) as usize).copied().unwrap_or(b'A'));
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = |k: usize| u32::from(chunk.get(k).copied().unwrap_or(0));
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        out.push(sym(n >> 18));
        out.push(sym(n >> 12));
        out.push(if chunk.len() > 1 { sym(n >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { sym(n) } else { '=' });
    }
    out
}
