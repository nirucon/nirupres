//! A small PDF writer for deck export: every slide is drawn as vectors
//! (rectangles, ellipses, glyph outlines as reusable form XObjects) with
//! images embedded as JPEG (or Flate with a soft mask), plus an invisible
//! text layer so the PDF can be searched and copied from.
use crate::fonts::{Face, SHEAR};
use crate::paint::{GlyphRun, Img, Painter, RectF, Rgba};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

pub struct PdfDoc {
    objs: Vec<Option<Vec<u8>>>,
    pages: Vec<usize>,
    glyphs: HashMap<(u32, u16), usize>,
    alphas: HashMap<u8, usize>,
    font_obj: usize,
    pages_obj: usize,
    pub width_pt: f32,
    pub height_pt: f32,
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

fn num(v: f32) -> String {
    if v.is_finite() {
        let s = format!("{v:.3}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s == "-0" {
            "0".into()
        } else {
            s.to_string()
        }
    } else {
        "0".into()
    }
}

fn pdf_string(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        let b = winansi(c);
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7e => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out.push(')');
    out
}

fn winansi(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '€' => 0x80,
        '‚' => 0x82,
        'ƒ' => 0x83,
        '„' => 0x84,
        '…' => 0x85,
        '†' => 0x86,
        '‡' => 0x87,
        'ˆ' => 0x88,
        '‰' => 0x89,
        'Š' => 0x8a,
        '‹' => 0x8b,
        'Œ' => 0x8c,
        'Ž' => 0x8e,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        '˜' => 0x98,
        '™' => 0x99,
        'š' => 0x9a,
        '›' => 0x9b,
        'œ' => 0x9c,
        'ž' => 0x9e,
        'Ÿ' => 0x9f,
        _ => b'?',
    }
}

struct PathOut {
    s: String,
    shear: f32,
    lx: f32,
    ly: f32,
}

impl PathOut {
    fn pt(&self, x: f32, y: f32) -> String {
        format!("{} {}", num(x + y * self.shear), num(y))
    }
}

impl ttf_parser::OutlineBuilder for PathOut {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        let _ = writeln!(self.s, "{p} m");
        (self.lx, self.ly) = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        let _ = writeln!(self.s, "{p} l");
        (self.lx, self.ly) = (x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // Quadratic to cubic: controls at two thirds towards the quad control.
        let (px, py) = (self.lx, self.ly);
        let c1 = self.pt(px + 2.0 / 3.0 * (x1 - px), py + 2.0 / 3.0 * (y1 - py));
        let c2 = self.pt(x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
        let e = self.pt(x, y);
        let _ = writeln!(self.s, "{c1} {c2} {e} c");
        (self.lx, self.ly) = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.pt(x1, y1);
        let c2 = self.pt(x2, y2);
        let e = self.pt(x, y);
        let _ = writeln!(self.s, "{c1} {c2} {e} c");
        (self.lx, self.ly) = (x, y);
    }
    fn close(&mut self) {
        self.s.push_str("h\n");
    }
}

impl PdfDoc {
    /// A document of pages `width_pt` x `height_pt` points.
    pub fn new(width_pt: f32, height_pt: f32) -> Self {
        let mut d = Self {
            objs: vec![None, None, None], // 1 catalog, 2 pages
            pages: Vec::new(),
            glyphs: HashMap::new(),
            alphas: HashMap::new(),
            font_obj: 0,
            pages_obj: 2,
            width_pt,
            height_pt,
        };
        d.font_obj = d.add(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        d
    }

    fn add(&mut self, body: Vec<u8>) -> usize {
        self.objs.push(Some(body));
        self.objs.len() - 1
    }

    fn add_stream(&mut self, dict: &str, data: &[u8], compress: bool) -> usize {
        let (data, filter) = if compress {
            (deflate(data), " /Filter /FlateDecode")
        } else {
            (data.to_vec(), "")
        };
        let mut b = format!("<< {dict}{filter} /Length {} >>\nstream\n", data.len()).into_bytes();
        b.extend_from_slice(&data);
        b.extend_from_slice(b"\nendstream");
        self.add(b)
    }

    fn glyph(&mut self, face: &Face, gid: u16) -> Option<usize> {
        if let Some(&o) = self.glyphs.get(&(face.id, gid)) {
            return Some(o);
        }
        let mut po = PathOut {
            s: String::new(),
            shear: if face.oblique { SHEAR } else { 0.0 },
            lx: 0.0,
            ly: 0.0,
        };
        let bb = face.ttf.outline_glyph(ttf_parser::GlyphId(gid), &mut po)?;
        po.s.push_str("f\n");
        let pad = 0.25 * face.upem;
        let dict = format!(
            "/Type /XObject /Subtype /Form /BBox [{} {} {} {}]",
            num(bb.x_min as f32 - pad),
            num(bb.y_min as f32 - pad),
            num(bb.x_max as f32 + pad),
            num(bb.y_max as f32 + pad)
        );
        let o = self.add_stream(&dict, po.s.as_bytes(), true);
        self.glyphs.insert((face.id, gid), o);
        Some(o)
    }

    fn alpha(&mut self, a: u8) -> usize {
        if let Some(&o) = self.alphas.get(&a) {
            return o;
        }
        let f = a as f32 / 255.0;
        let o =
            self.add(format!("<< /Type /ExtGState /ca {} /CA {} >>", num(f), num(f)).into_bytes());
        self.alphas.insert(a, o);
        o
    }

    fn image(&mut self, img: &Img) -> usize {
        let alpha = img.has_alpha();
        let smask = if alpha {
            let a: Vec<u8> = img.data.chunks_exact(4).map(|p| p[3]).collect();
            Some(self.add_stream(
                &format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8",
                    img.w, img.h
                ),
                &a,
                true,
            ))
        } else {
            None
        };
        let rgb: Vec<u8> = img
            .data
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let sm = smask
            .map(|o| format!(" /SMask {o} 0 R"))
            .unwrap_or_default();
        if !alpha {
            if let Some(jpg) = crate::images::encode_jpeg_rgb(img.w, img.h, &rgb, 90) {
                return self.add_stream(
                    &format!(
                        "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
                        img.w, img.h
                    ),
                    &jpg,
                    false,
                );
            }
        }
        self.add_stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8{sm}",
                img.w, img.h
            ),
            &rgb,
            true,
        )
    }

    /// Start a page drawn in device pixels of `scale` points each (y down).
    pub fn page(&mut self, scale: f32) -> PdfPainter<'_> {
        let mut content = String::new();
        let _ = write!(
            content,
            "{} 0 0 {} 0 {} cm\n",
            num(scale),
            num(-scale),
            num(self.height_pt)
        );
        PdfPainter {
            doc: self,
            content,
            xobjects: Vec::new(),
            gstates: Vec::new(),
            images: HashMap::new(),
            text: String::new(),
            scale,
        }
    }

    pub fn finish(mut self, title: &str) -> Vec<u8> {
        let kids: String = self.pages.iter().map(|p| format!("{p} 0 R ")).collect();
        self.objs[2] = Some(
            format!(
                "<< /Type /Pages /Kids [{kids}] /Count {} >>",
                self.pages.len()
            )
            .into_bytes(),
        );
        self.objs[1] = Some(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let info = self.add(
            format!(
                "<< /Producer {} /Creator {} /Title {} >>",
                pdf_string(&format!("NIRUPRES {}", crate::model::VERSION)),
                pdf_string("NIRUPRES"),
                pdf_string(title)
            )
            .into_bytes(),
        );
        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = vec![0usize; self.objs.len()];
        for (i, o) in self.objs.iter().enumerate().skip(1) {
            offsets[i] = out.len();
            out.extend_from_slice(format!("{i} 0 obj\n").as_bytes());
            out.extend_from_slice(o.as_deref().unwrap_or(b"null"));
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", self.objs.len()).as_bytes(),
        );
        for off in offsets.iter().skip(1) {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                self.objs.len()
            )
            .as_bytes(),
        );
        out
    }
}

pub struct PdfPainter<'a> {
    doc: &'a mut PdfDoc,
    content: String,
    xobjects: Vec<usize>,
    gstates: Vec<usize>,
    images: HashMap<usize, usize>,
    text: String,
    scale: f32,
}

impl PdfPainter<'_> {
    fn color(&mut self, c: Rgba) {
        let _ = write!(
            self.content,
            "{} {} {} rg\n",
            num(c.0 as f32 / 255.0),
            num(c.1 as f32 / 255.0),
            num(c.2 as f32 / 255.0)
        );
        if c.3 < 255 {
            let g = self.doc.alpha(c.3);
            if !self.gstates.contains(&g) {
                self.gstates.push(g);
            }
            let _ = write!(self.content, "/GS{g} gs\n");
        }
    }

    fn reset_alpha(&mut self, c: Rgba) {
        if c.3 < 255 {
            let g = self.doc.alpha(255);
            if !self.gstates.contains(&g) {
                self.gstates.push(g);
            }
            let _ = write!(self.content, "/GS{g} gs\n");
        }
    }

    pub fn end(self) {
        let mut res = String::from("<< /ProcSet [/PDF /Text /ImageB /ImageC]");
        if !self.xobjects.is_empty() {
            res.push_str(" /XObject <<");
            for o in &self.xobjects {
                let _ = write!(res, " /X{o} {o} 0 R");
            }
            res.push_str(" >>");
        }
        if !self.gstates.is_empty() {
            res.push_str(" /ExtGState <<");
            for g in &self.gstates {
                let _ = write!(res, " /GS{g} {g} 0 R");
            }
            res.push_str(" >>");
        }
        let _ = write!(res, " /Font << /F1 {} 0 R >> >>", self.doc.font_obj);
        let mut content = self.content;
        content.push_str(&self.text);
        let doc = self.doc;
        let c = doc.add_stream("", content.as_bytes(), true);
        let page = doc.add(
            format!(
                "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Resources {res} /Contents {c} 0 R >>",
                doc.pages_obj,
                num(doc.width_pt),
                num(doc.height_pt)
            )
            .into_bytes(),
        );
        doc.pages.push(page);
    }
}

impl Painter for PdfPainter<'_> {
    fn fill_rect(&mut self, r: RectF, c: Rgba) {
        if r.w <= 0.0 || r.h <= 0.0 || c.3 == 0 {
            return;
        }
        self.color(c);
        let _ = write!(
            self.content,
            "{} {} {} {} re f\n",
            num(r.x),
            num(r.y),
            num(r.w),
            num(r.h)
        );
        self.reset_alpha(c);
    }

    fn fill_ellipse(&mut self, r: RectF, c: Rgba) {
        self.color(c);
        let k = 0.552_284_8;
        let (cx, cy) = r.center();
        let (rx, ry) = (r.w / 2.0, r.h / 2.0);
        let _ = write!(
            self.content,
            "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c f\n",
            num(cx + rx), num(cy),
            num(cx + rx), num(cy + ry * k), num(cx + rx * k), num(cy + ry), num(cx), num(cy + ry),
            num(cx - rx * k), num(cy + ry), num(cx - rx), num(cy + ry * k), num(cx - rx), num(cy),
            num(cx - rx), num(cy - ry * k), num(cx - rx * k), num(cy - ry), num(cx), num(cy - ry),
            num(cx + rx * k), num(cy - ry), num(cx + rx), num(cy - ry * k), num(cx + rx), num(cy)
        );
        self.reset_alpha(c);
    }

    fn stroke_rect(&mut self, r: RectF, c: Rgba, width: f32) {
        let _ = write!(
            self.content,
            "{} {} {} RG {} w {} {} {} {} re S\n",
            num(c.0 as f32 / 255.0),
            num(c.1 as f32 / 255.0),
            num(c.2 as f32 / 255.0),
            num(width),
            num(r.x),
            num(r.y),
            num(r.w),
            num(r.h)
        );
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: Rgba, width: f32) {
        let _ = write!(
            self.content,
            "{} {} {} RG {} w {} {} m {} {} l S\n",
            num(c.0 as f32 / 255.0),
            num(c.1 as f32 / 255.0),
            num(c.2 as f32 / 255.0),
            num(width),
            num(x1),
            num(y1),
            num(x2),
            num(y2)
        );
    }

    fn image(&mut self, dest: RectF, img: &Arc<Img>, opacity: f32) {
        let key = Arc::as_ptr(img) as usize;
        let o = match self.images.get(&key) {
            Some(&o) => o,
            None => {
                let o = self.doc.image(img);
                self.images.insert(key, o);
                o
            }
        };
        if !self.xobjects.contains(&o) {
            self.xobjects.push(o);
        }
        self.content.push_str("q\n");
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        if a < 255 {
            let g = self.doc.alpha(a);
            if !self.gstates.contains(&g) {
                self.gstates.push(g);
            }
            let _ = write!(self.content, "/GS{g} gs\n");
        }
        let _ = write!(
            self.content,
            "{} 0 0 {} {} {} cm /X{o} Do\nQ\n",
            num(dest.w),
            num(-dest.h),
            num(dest.x),
            num(dest.y + dest.h)
        );
    }

    fn glyphs(&mut self, run: &GlyphRun, c: Rgba) {
        if run.glyphs.is_empty() {
            return;
        }
        self.color(c);
        let k = run.px / run.face.upem;
        for &(gid, x, y) in &run.glyphs {
            if let Some(o) = self.doc.glyph(&run.face, gid) {
                if !self.xobjects.contains(&o) {
                    self.xobjects.push(o);
                }
                let _ = write!(
                    self.content,
                    "q {} 0 0 {} {} {} cm /X{o} Do Q\n",
                    num(k),
                    num(-k),
                    num(x),
                    num(y)
                );
            }
        }
        self.reset_alpha(c);
        // Invisible text, stretched over the glyphs, for search and copy.
        let t = run.text.trim_end();
        if !t.is_empty() {
            let (x0, y0) = (run.glyphs[0].1, run.glyphs[0].2);
            let last = run.glyphs[run.glyphs.len() - 1];
            let width = (last.1 - x0 + run.px * 0.5).max(1.0);
            let natural = t.chars().count() as f32 * run.px * 0.5;
            let tz = (width / natural.max(0.1) * 100.0).clamp(1.0, 1000.0);
            let _ = write!(
                self.text,
                "BT 3 Tr /F1 {} Tf {} Tz 1 0 0 -1 {} {} Tm {} Tj ET\n",
                num(run.px),
                num(tz),
                num(x0),
                num(y0),
                pdf_string(t)
            );
        }
    }

    fn push_clip(&mut self, r: RectF) {
        let _ = write!(
            self.content,
            "q {} {} {} {} re W n\n",
            num(r.x),
            num(r.y),
            num(r.w),
            num(r.h)
        );
    }

    fn pop_clip(&mut self) {
        self.content.push_str("Q\n");
    }

    fn image_density(&self) -> f32 {
        // Images at about 216 dpi on an A4 page (1.5 x the 144 dpi grid).
        let _ = self.scale;
        1.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_with_a_rectangle_is_a_valid_document() {
        let mut d = PdfDoc::new(841.89, 595.28);
        {
            let mut p = d.page(0.5);
            p.fill_rect(RectF::new(0.0, 0.0, 100.0, 50.0), Rgba(255, 0, 0, 128));
            p.end();
        }
        let out = d.finish("Test (1)");
        let s = String::from_utf8_lossy(&out);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 1"));
        assert!(s.contains("startxref"));
        assert!(s.contains("Test \\(1\\)"));
    }
}
