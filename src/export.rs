//! PNG, PDF and PPTX export.
use crate::canvas::{Canvas, RasterPainter};
use crate::model::*;
use crate::paint::{Painter, RectF, Rgba};
use crate::render::{self, Ctx};
use std::path::{Path, PathBuf};

pub const PREVIEW: (u32, u32) = (1920, 1080);
pub const PPTX: (u32, u32) = (2560, 1440);

/// `_render_export_image`: one slide at the deck aspect within (w, h).
pub fn render_image(data: &Obj, i: usize, width: u32, height: u32) -> Canvas {
    let d = Deck(data);
    let ar = aspect_ratio(&d.aspect());
    let (mut w, mut h) = (width as f64, height as f64);
    if w / h > ar {
        w = (h * ar).floor();
    } else {
        h = (w / ar).floor();
    }
    let mut cv = Canvas::new(w as i32, h as i32);
    let bg = theme_or_bw(&d.theme()).bg;
    cv.fill(cv.bounds(), bg);
    let style = DeckStyle::of(data);
    let ctx = Ctx::new(&style, i, style.show_numbers);
    if let Some(s) = d.slides().get(i).and_then(|v| v.as_object()) {
        let mut p = RasterPainter { cv: &mut cv };
        render::render_slide(&mut p, RectF::new(0.0, 0.0, w as f32, h as f32), s, &ctx);
    }
    cv
}

pub fn png_bytes(cv: &Canvas) -> Option<Vec<u8>> {
    let rgb: Vec<u8> = cv
        .px
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    crate::images::encode_png_rgb(cv.w as u32, cv.h as u32, &rgb)
}

pub fn export_png(data: &Obj, i: usize, path: &Path) -> Result<(), String> {
    let cv = render_image(data, i, PREVIEW.0, PREVIEW.1);
    let bytes = png_bytes(&cv).ok_or("PNG encoding failed")?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn export_all_png(data: &Obj, folder: &Path) -> Result<usize, String> {
    let n = Deck(data).slides().len();
    for i in 0..n {
        export_png(data, i, &folder.join(format!("slide-{:02}.png", i + 1)))?;
    }
    Ok(n)
}

pub fn with_ext(p: &Path, ext: &str) -> PathBuf {
    let s = p.to_string_lossy();
    if s.to_lowercase().ends_with(ext) {
        p.to_path_buf()
    } else {
        PathBuf::from(format!("{s}{ext}"))
    }
}

/// `export_pdf_to`: A4 landscape pages, every slide (hidden ones too, as
/// in 2.x), letterboxed on the deck background.
pub fn export_pdf(data: &Obj, path: &Path) -> Result<PathBuf, String> {
    let path = with_ext(path, ".pdf");
    let (wpt, hpt) = (841.889_76_f32, 595.275_6_f32);
    // QPdfWriter at 144 dpi: the page is 1684 x 1191 device pixels.
    let scale = 72.0 / 144.0;
    let (pw, ph) = ((wpt / scale).round(), (hpt / scale).round());
    let d = Deck(data);
    let ar = aspect_ratio(&d.aspect()) as f32;
    let r = if pw / ph > ar {
        let nw = ph * ar;
        RectF::new((pw - nw) / 2.0, 0.0, nw, ph)
    } else {
        let nh = pw / ar;
        RectF::new(0.0, (ph - nh) / 2.0, pw, nh)
    };
    let bg = Rgba::hex(theme_or_bw(&d.theme()).bg);
    let style = DeckStyle::of(data);
    let mut doc = crate::pdf::PdfDoc::new(wpt, hpt);
    for (i, s) in d.slides().iter().enumerate() {
        let Some(s) = s.as_object() else { continue };
        let mut p = doc.page(scale);
        p.fill_rect(RectF::new(0.0, 0.0, pw, ph), bg);
        let ctx = Ctx::new(&style, i, style.show_numbers);
        render::render_slide(&mut p, r, s, &ctx);
        p.end();
    }
    let bytes = doc.finish(&s_get(data, "title", "Untitled"));
    let tmp = path.with_extension("pdf.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn export_pptx(data: &Obj, path: &Path) -> Result<PathBuf, String> {
    let path = with_ext(path, ".pptx");
    let d = Deck(data);
    let mut slides = Vec::new();
    for (i, s) in d.slides().iter().enumerate() {
        let Some(s) = s.as_object() else { continue };
        if b_get(s, "hidden") {
            continue;
        }
        let cv = render_image(data, i, PPTX.0, PPTX.1);
        let png = png_bytes(&cv).ok_or("PNG encoding failed")?;
        slides.push((png, s_get(s, "notes", "").trim().to_string()));
    }
    crate::pptx::write(&path, &slides, aspect_ratio(&d.aspect()))?;
    Ok(path)
}
