//! The drawing interface the slide renderer paints through. The screen and
//! PNG export use a raster painter; PDF export writes vectors; Deck health
//! measures with a painter that draws nothing.
use crate::fonts::Face;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    /// `QRectF::adjusted`.
    pub fn adjusted(&self, l: f32, t: f32, r: f32, b: f32) -> Self {
        Self::new(self.x + l, self.y + t, self.w - l + r, self.h - t + b)
    }
    pub fn intersect(&self, o: &RectF) -> RectF {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        RectF::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }
    /// `QRectF::toRect()`: rounded edges.
    pub fn to_rect(&self) -> (i32, i32, i32, i32) {
        let x = self.x.round() as i32;
        let y = self.y.round() as i32;
        let w = (self.w.round() as i32).max(0);
        let h = (self.h.round() as i32).max(0);
        (x, y, w, h)
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }
}

/// Straight (not premultiplied) colour with alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const fn hex(c: u32) -> Self {
        Rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255)
    }
    pub const fn with_alpha(self, a: u8) -> Self {
        Rgba(self.0, self.1, self.2, a)
    }
    pub fn rgb(self) -> u32 {
        ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | self.2 as u32
    }
}

/// Glyphs of one face at one size: (glyph id, pen x, baseline y).
pub struct GlyphRun {
    pub face: Arc<Face>,
    pub px: f32,
    pub glyphs: Vec<(u16, f32, f32)>,
    /// The characters the glyphs show (for the PDF text layer).
    pub text: String,
}

/// An RGBA image (straight alpha), row-major.
#[derive(Clone)]
pub struct Img {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Img {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            data: vec![0; (w * h * 4) as usize],
        }
    }
    pub fn has_alpha(&self) -> bool {
        self.data.chunks_exact(4).any(|p| p[3] != 255)
    }
}

pub trait Painter {
    /// Antialiased rectangle fill (fractional edges blend).
    fn fill_rect(&mut self, r: RectF, c: Rgba);
    fn fill_ellipse(&mut self, r: RectF, c: Rgba);
    /// `drawRect` with a one pixel pen centred on the edge.
    fn stroke_rect(&mut self, r: RectF, c: Rgba, width: f32);
    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: Rgba, width: f32);
    /// Draw a prepared image into `dest` with an opacity.
    fn image(&mut self, dest: RectF, img: &Arc<Img>, opacity: f32);
    fn glyphs(&mut self, run: &GlyphRun, c: Rgba);
    fn push_clip(&mut self, r: RectF);
    fn pop_clip(&mut self);
    /// Pixels per slide unit for image preparation; 0 for a painter that
    /// does not draw (measuring only).
    fn image_density(&self) -> f32 {
        1.0
    }
}

/// Paints nothing: used to measure text fit (Deck health).
pub struct NullPainter;

impl Painter for NullPainter {
    fn fill_rect(&mut self, _: RectF, _: Rgba) {}
    fn fill_ellipse(&mut self, _: RectF, _: Rgba) {}
    fn stroke_rect(&mut self, _: RectF, _: Rgba, _: f32) {}
    fn line(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba, _: f32) {}
    fn image(&mut self, _: RectF, _: &Arc<Img>, _: f32) {}
    fn glyphs(&mut self, _: &GlyphRun, _: Rgba) {}
    fn push_clip(&mut self, _: RectF) {}
    fn pop_clip(&mut self) {}
    fn image_density(&self) -> f32 {
        0.0
    }
}
