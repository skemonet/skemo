//! PNG without a browser (F-RENDER-3): SVG → resvg, with the embedded fonts only.
//! Deterministic: same bytes for the same input.

use crate::fonts::FILES;
use resvg::{tiny_skia, usvg};
use skemo_lang::tr;
use std::sync::{Arc, Mutex, OnceLock};

/// The embedded fonts and the registered ones (`fonts::register`); rebuilt when a font is added.
fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Mutex<(usize, Arc<usvg::fontdb::Database>)>> = OnceLock::new();
    let user = crate::fonts::registered();
    let cell = DB.get_or_init(|| Mutex::new((usize::MAX, Arc::new(usvg::fontdb::Database::new()))));
    let Ok(mut held) = cell.lock() else {
        return Arc::new(usvg::fontdb::Database::new());
    };
    if held.0 != user.len() {
        let mut db = usvg::fontdb::Database::new();
        for f in FILES.iter().chain(user.iter().map(|f| f.file)) {
            db.load_font_data(f.data.to_vec());
        }
        db.set_sans_serif_family("Inter");
        db.set_monospace_family("JetBrains Mono");
        *held = (user.len(), Arc::new(db));
    }
    held.1.clone()
}

/// Renders an SVG (produced by [`crate::svg::render`]) as PNG, at scale `scale`.
pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let opt = usvg::Options {
        fontdb: fontdb(),
        // image icons come as `data:` URLs (D-046); a path or URL in an SVG someone handed over is
        // never resolved, at any depth (nested SVGs use these options too)
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &opt)
        .map_err(|e| tr!("unreadable SVG: {e}", "SVG illisible : {e}"))?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or_else(|| tr!("invalid scale", "échelle invalide"))?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| tr!("image too large", "image trop grande"))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| tr!("PNG encoding: {e}", "encodage PNG : {e}"))
}

/// Renders the diagram as PNG.
pub fn render(d: &crate::Diagram, scale: f32) -> Result<Vec<u8>, String> {
    svg_to_png(&crate::svg::render_for_raster(d), scale)
}
