//! A client-side framebuffer (0x00RRGGBB per pixel) and the raster painter
//! the slides are drawn with. Everything is drawn here and sent to the
//! window in one copy, so a redraw never flickers.
use crate::fonts;
use crate::paint::{GlyphRun, Img, Painter, RectF, Rgba};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }
    pub fn inset(&self, d: i32) -> Rect {
        Rect::new(
            self.x + d,
            self.y + d,
            (self.w - 2 * d).max(0),
            (self.h - 2 * d).max(0),
        )
    }
    pub fn f(&self) -> RectF {
        RectF::new(self.x as f32, self.y as f32, self.w as f32, self.h as f32)
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
    clip: Rect,
    clips: Vec<Rect>,
}

#[inline]
fn blend(d: u32, color: u32, a: u32) -> u32 {
    if a >= 255 {
        return color;
    }
    let inv = 255 - a;
    let ch = |s: u32, d: u32| (s * a + d * inv + 127) / 255;
    (ch((color >> 16) & 0xff, (d >> 16) & 0xff) << 16)
        | (ch((color >> 8) & 0xff, (d >> 8) & 0xff) << 8)
        | ch(color & 0xff, d & 0xff)
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        let w = w.max(1);
        let h = h.max(1);
        Self {
            w,
            h,
            px: vec![0; (w * h) as usize],
            clip: Rect::new(0, 0, w, h),
            clips: Vec::new(),
        }
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        let w = w.max(1);
        let h = h.max(1);
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.px = vec![0; (w * h) as usize];
            self.px.shrink_to_fit();
        }
        self.clip = Rect::new(0, 0, w, h);
        self.clips.clear();
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    pub fn set_clip(&mut self, r: Rect) {
        self.clip = r.intersect(&self.bounds());
    }

    pub fn reset_clip(&mut self) {
        self.clip = self.bounds();
        self.clips.clear();
    }

    pub fn push_clip_rect(&mut self, r: Rect) {
        self.clips.push(self.clip);
        self.clip = self.clip.intersect(&r);
    }

    pub fn pop_clip_rect(&mut self) {
        if let Some(c) = self.clips.pop() {
            self.clip = c;
        }
    }

    pub fn clip(&self) -> Rect {
        self.clip
    }

    pub fn fill(&mut self, r: Rect, color: u32) {
        let r = r.intersect(&self.clip);
        if r.w <= 0 || r.h <= 0 {
            return;
        }
        for y in r.y..r.y + r.h {
            let row = (y * self.w) as usize;
            self.px[row + r.x as usize..row + (r.x + r.w) as usize].fill(color);
        }
    }

    /// A one pixel frame inside `r`.
    pub fn frame(&mut self, r: Rect, color: u32) {
        self.fill(Rect::new(r.x, r.y, r.w, 1), color);
        self.fill(Rect::new(r.x, r.y + r.h - 1, r.w, 1), color);
        self.fill(Rect::new(r.x, r.y, 1, r.h), color);
        self.fill(Rect::new(r.x + r.w - 1, r.y, 1, r.h), color);
    }

    pub fn tint(&mut self, r: Rect, color: u32, a: u32) {
        let r = r.intersect(&self.clip);
        for py in r.y..r.y + r.h {
            let row = (py * self.w) as usize;
            for px in r.x..r.x + r.w {
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], color, a);
            }
        }
    }

    /// Draw a coverage mask (one byte per pixel) in `color` at (x, y).
    pub fn mask(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        coverage: &[u8],
        color: u32,
        alpha: u32,
    ) {
        let area = Rect::new(x, y, w, h).intersect(&self.clip);
        for py in area.y..area.y + area.h {
            let src_row = ((py - y) * w) as usize;
            let dst_row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let a = coverage[src_row + (px - x) as usize] as u32 * alpha / 255;
                if a == 0 {
                    continue;
                }
                let i = dst_row + px as usize;
                self.px[i] = blend(self.px[i], color, a);
            }
        }
    }

    /// Blend an RGBA image (straight alpha) at (x, y) with an opacity 0..=255.
    pub fn blit(&mut self, x: i32, y: i32, img: &Img, alpha: u32) {
        let area = Rect::new(x, y, img.w as i32, img.h as i32).intersect(&self.clip);
        for py in area.y..area.y + area.h {
            let src_row = ((py - y) as usize) * img.w as usize * 4;
            let dst_row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let s = src_row + (px - x) as usize * 4;
                let a = img.data[s + 3] as u32 * alpha / 255;
                if a == 0 {
                    continue;
                }
                let c = ((img.data[s] as u32) << 16)
                    | ((img.data[s + 1] as u32) << 8)
                    | img.data[s + 2] as u32;
                let i = dst_row + px as usize;
                self.px[i] = blend(self.px[i], c, a);
            }
        }
    }

    /// Draw `img` stretched into `dst` with bilinear sampling.
    pub fn blit_scaled(&mut self, img: &Img, dst: Rect, alpha: u32) {
        if dst.w <= 0 || dst.h <= 0 || img.w == 0 || img.h == 0 {
            return;
        }
        if dst.w == img.w as i32 && dst.h == img.h as i32 {
            self.blit(dst.x, dst.y, img, alpha);
            return;
        }
        let area = dst.intersect(&self.clip);
        let sx = img.w as f32 / dst.w as f32;
        let sy = img.h as f32 / dst.h as f32;
        let iw = img.w as usize;
        for py in area.y..area.y + area.h {
            let fy = ((py - dst.y) as f32 + 0.5) * sy - 0.5;
            let y0 = fy.floor().clamp(0.0, img.h as f32 - 1.0) as usize;
            let y1 = (y0 + 1).min(img.h as usize - 1);
            let ty = (fy - y0 as f32).clamp(0.0, 1.0);
            let row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let fx = ((px - dst.x) as f32 + 0.5) * sx - 0.5;
                let x0 = fx.floor().clamp(0.0, img.w as f32 - 1.0) as usize;
                let x1 = (x0 + 1).min(iw - 1);
                let tx = (fx - x0 as f32).clamp(0.0, 1.0);
                let p = |x: usize, y: usize, k: usize| img.data[(y * iw + x) * 4 + k] as f32;
                let mut c = [0f32; 4];
                for (k, v) in c.iter_mut().enumerate() {
                    let a = p(x0, y0, k) + (p(x1, y0, k) - p(x0, y0, k)) * tx;
                    let b = p(x0, y1, k) + (p(x1, y1, k) - p(x0, y1, k)) * tx;
                    *v = a + (b - a) * ty;
                }
                let a = (c[3] as u32) * alpha / 255;
                if a == 0 {
                    continue;
                }
                let col = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], col, a);
            }
        }
    }

    /// Copy another canvas into `dst`, scaled (bilinear), with an opacity.
    pub fn draw_canvas(&mut self, src: &Canvas, src_rect: RectF, dst: RectF, alpha: u32) {
        if src_rect.w <= 0.0 || src_rect.h <= 0.0 || dst.w <= 0.0 || dst.h <= 0.0 {
            return;
        }
        let x0 = dst.x.floor() as i32;
        let y0 = dst.y.floor() as i32;
        let x1 = dst.right().ceil() as i32;
        let y1 = dst.bottom().ceil() as i32;
        let area = Rect::new(x0, y0, x1 - x0, y1 - y0).intersect(&self.clip);
        let sx = src_rect.w / dst.w;
        let sy = src_rect.h / dst.h;
        let same = (sx - 1.0).abs() < 1e-4 && (sy - 1.0).abs() < 1e-4;
        for py in area.y..area.y + area.h {
            let fy = src_rect.y + ((py as f32 + 0.5) - dst.y) * sy - 0.5;
            let row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let fx = src_rect.x + ((px as f32 + 0.5) - dst.x) * sx - 0.5;
                let col = if same {
                    let ix = (fx + 0.5).floor() as i32;
                    let iy = (fy + 0.5).floor() as i32;
                    if ix < 0 || iy < 0 || ix >= src.w || iy >= src.h {
                        continue;
                    }
                    src.px[(iy * src.w + ix) as usize]
                } else {
                    sample(src, fx, fy)
                };
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], col, alpha);
            }
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: u32 has no padding; the slice covers exactly the pixels.
        unsafe { std::slice::from_raw_parts(self.px.as_ptr() as *const u8, self.px.len() * 4) }
    }

    /// RGBA bytes (opaque) for PNG export.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.px.len() * 4);
        for &p in &self.px {
            v.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
        v
    }

    /// Fill a fractional rectangle with antialiased edges.
    pub fn fill_aa(&mut self, r: RectF, c: Rgba) {
        if r.w <= 0.0 || r.h <= 0.0 || c.3 == 0 {
            return;
        }
        let x0 = r.x.floor() as i32;
        let y0 = r.y.floor() as i32;
        let x1 = r.right().ceil() as i32;
        let y1 = r.bottom().ceil() as i32;
        let area = Rect::new(x0, y0, x1 - x0, y1 - y0).intersect(&self.clip);
        let col = c.rgb();
        for py in area.y..area.y + area.h {
            let cy = (r.bottom().min(py as f32 + 1.0) - r.y.max(py as f32)).clamp(0.0, 1.0);
            let row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let cx = (r.right().min(px as f32 + 1.0) - r.x.max(px as f32)).clamp(0.0, 1.0);
                let a = (cx * cy * c.3 as f32 + 0.5) as u32;
                if a == 0 {
                    continue;
                }
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], col, a);
            }
        }
    }

    pub fn ellipse_aa(&mut self, r: RectF, c: Rgba) {
        let (cx, cy) = r.center();
        let (rx, ry) = (r.w / 2.0, r.h / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let area = Rect::new(
            r.x.floor() as i32,
            r.y.floor() as i32,
            r.w.ceil() as i32 + 2,
            r.h.ceil() as i32 + 2,
        )
        .intersect(&self.clip);
        let col = c.rgb();
        for py in area.y..area.y + area.h {
            let row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let mut n = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let x = (px as f32 + (sx as f32 + 0.5) / 4.0 - cx) / rx;
                        let y = (py as f32 + (sy as f32 + 0.5) / 4.0 - cy) / ry;
                        if x * x + y * y <= 1.0 {
                            n += 1;
                        }
                    }
                }
                if n == 0 {
                    continue;
                }
                let a = n * c.3 as u32 / 16;
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], col, a);
            }
        }
    }

    /// Everything outside a circle darkened with `alpha` (Spotlight/Focus).
    pub fn veil_with_hole(&mut self, area: Rect, cx: f32, cy: f32, r: f32, alpha: u32) {
        let area = area.intersect(&self.clip);
        for py in area.y..area.y + area.h {
            let row = (py * self.w) as usize;
            let dy = py as f32 + 0.5 - cy;
            for px in area.x..area.x + area.w {
                let dx = px as f32 + 0.5 - cx;
                let d = (dx * dx + dy * dy).sqrt() - r;
                // About one pixel of antialiasing on the hole edge.
                let cov = (d + 0.5).clamp(0.0, 1.0);
                let a = (cov * alpha as f32) as u32;
                if a == 0 {
                    continue;
                }
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], 0, a);
            }
        }
    }

    /// A circle outline of one pixel (the Focus feather ring).
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        let area = Rect::new(
            (cx - r - 2.0) as i32,
            (cy - r - 2.0) as i32,
            (2.0 * r + 4.0) as i32,
            (2.0 * r + 4.0) as i32,
        )
        .intersect(&self.clip);
        let col = c.rgb();
        for py in area.y..area.y + area.h {
            let row = (py * self.w) as usize;
            let dy = py as f32 + 0.5 - cy;
            for px in area.x..area.x + area.w {
                let dx = px as f32 + 0.5 - cx;
                let d = ((dx * dx + dy * dy).sqrt() - r).abs();
                let cov = (1.0 - d).clamp(0.0, 1.0);
                let a = (cov * c.3 as f32) as u32;
                if a == 0 {
                    continue;
                }
                let i = row + px as usize;
                self.px[i] = blend(self.px[i], col, a);
            }
        }
    }

    pub fn round_rect(&mut self, r: Rect, radius: i32, color: u32) {
        if radius <= 0 {
            self.fill(r, color);
            return;
        }
        let rf = radius as f32;
        let area = r.intersect(&self.clip);
        for py in area.y..area.y + area.h {
            let row = (py * self.w) as usize;
            for px in area.x..area.x + area.w {
                let lx = px - r.x;
                let ly = py - r.y;
                let cx = if lx < radius {
                    radius
                } else if lx >= r.w - radius {
                    r.w - radius - 1
                } else {
                    lx
                };
                let cy = if ly < radius {
                    radius
                } else if ly >= r.h - radius {
                    r.h - radius - 1
                } else {
                    ly
                };
                let i = row + px as usize;
                if cx == lx || cy == ly {
                    self.px[i] = color;
                    continue;
                }
                let dx = (lx - cx) as f32;
                let dy = (ly - cy) as f32;
                let d = (dx * dx + dy * dy).sqrt() - rf + 0.5;
                let cov = (1.0 - d).clamp(0.0, 1.0);
                self.px[i] = blend(self.px[i], color, (cov * 255.0) as u32);
            }
        }
    }
}

fn sample(src: &Canvas, fx: f32, fy: f32) -> u32 {
    let x0 = fx.floor().clamp(0.0, (src.w - 1) as f32) as i32;
    let y0 = fy.floor().clamp(0.0, (src.h - 1) as f32) as i32;
    let x1 = (x0 + 1).min(src.w - 1);
    let y1 = (y0 + 1).min(src.h - 1);
    let tx = ((fx - x0 as f32).clamp(0.0, 1.0) * 256.0) as u32;
    let ty = ((fy - y0 as f32).clamp(0.0, 1.0) * 256.0) as u32;
    let p00 = src.px[(y0 * src.w + x0) as usize];
    let p10 = src.px[(y0 * src.w + x1) as usize];
    let p01 = src.px[(y1 * src.w + x0) as usize];
    let p11 = src.px[(y1 * src.w + x1) as usize];
    let mut out = 0u32;
    for sh in [16, 8, 0] {
        let c = |p: u32| (p >> sh) & 0xff;
        let a = c(p00) * (256 - tx) + c(p10) * tx;
        let b = c(p01) * (256 - tx) + c(p11) * tx;
        let v = (a * (256 - ty) + b * ty) >> 16;
        out |= v.min(255) << sh;
    }
    out
}

/// The raster painter: the slide renderer drawing into a canvas.
pub struct RasterPainter<'a> {
    pub cv: &'a mut Canvas,
}

impl Painter for RasterPainter<'_> {
    fn fill_rect(&mut self, r: RectF, c: Rgba) {
        self.cv.fill_aa(r, c);
    }
    fn fill_ellipse(&mut self, r: RectF, c: Rgba) {
        self.cv.ellipse_aa(r, c);
    }
    fn stroke_rect(&mut self, r: RectF, c: Rgba, width: f32) {
        let h = width / 2.0;
        self.cv
            .fill_aa(RectF::new(r.x - h, r.y - h, r.w + width, width), c);
        self.cv
            .fill_aa(RectF::new(r.x - h, r.bottom() - h, r.w + width, width), c);
        self.cv
            .fill_aa(RectF::new(r.x - h, r.y + h, width, r.h - width), c);
        self.cv
            .fill_aa(RectF::new(r.right() - h, r.y + h, width, r.h - width), c);
    }
    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: Rgba, width: f32) {
        let h = width / 2.0;
        if (y1 - y2).abs() < 1e-3 {
            let (a, b) = (x1.min(x2), x1.max(x2));
            self.cv.fill_aa(RectF::new(a, y1 - h, b - a, width), c);
        } else if (x1 - x2).abs() < 1e-3 {
            let (a, b) = (y1.min(y2), y1.max(y2));
            self.cv.fill_aa(RectF::new(x1 - h, a, width, b - a), c);
        } else {
            let steps = ((x2 - x1).abs().max((y2 - y1).abs()) * 2.0) as i32 + 1;
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let x = x1 + (x2 - x1) * t;
                let y = y1 + (y2 - y1) * t;
                self.cv.fill_aa(
                    RectF::new(x - h, y - h, width, width),
                    c.with_alpha(c.3 / 2),
                );
            }
        }
    }
    fn image(&mut self, dest: RectF, img: &Arc<Img>, opacity: f32) {
        let (x, y, w, h) = dest.to_rect();
        let a = (opacity.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        self.cv.blit_scaled(img, Rect::new(x, y, w, h), a);
    }
    fn glyphs(&mut self, run: &GlyphRun, c: Rgba) {
        let col = c.rgb();
        for &(gid, x, y) in &run.glyphs {
            let fx = x - x.floor();
            if let Some(m) = fonts::glyph_mask(&run.face, gid, run.px, fx) {
                let gx = x.floor() as i32 + m.left;
                let gy = y.round() as i32 + m.top;
                self.cv.mask(gx, gy, m.w, m.h, &m.cov, col, c.3 as u32);
            }
        }
    }
    fn push_clip(&mut self, r: RectF) {
        let (x, y, w, h) = r.to_rect();
        self.cv.push_clip_rect(Rect::new(x, y, w, h));
    }
    fn pop_clip(&mut self) {
        self.cv.pop_clip_rect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_fill_blends_edges() {
        let mut c = Canvas::new(4, 1);
        c.fill_aa(RectF::new(0.5, 0.0, 2.0, 1.0), Rgba::hex(0xffffff));
        assert!((120..=135).contains(&(c.px[0] & 0xff)));
        assert_eq!(c.px[1], 0xffffff);
        assert!((120..=135).contains(&(c.px[2] & 0xff)));
        assert_eq!(c.px[3], 0);
    }

    #[test]
    fn clip_stack_restores() {
        let mut c = Canvas::new(10, 10);
        c.push_clip_rect(Rect::new(0, 0, 5, 5));
        c.fill(Rect::new(0, 0, 10, 10), 1);
        c.pop_clip_rect();
        c.fill(Rect::new(9, 9, 1, 1), 2);
        assert_eq!(c.px[4 * 10 + 4], 1);
        assert_eq!(c.px[5 * 10 + 5], 0);
        assert_eq!(c.px[99], 2);
    }
}
