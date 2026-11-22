//! Images: decoding (JPEG scaled while decoding, PNG, WebP, BMP), EXIF
//! orientation, a bounded cache of decoded sources and of prepared images,
//! and the 2.x image treatment (fill/fit, focal point, mono, blur,
//! brightness, contrast, overlay, zoom).
use crate::model::MAX_IMAGE_PIXELS;
use crate::paint::Img;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub w: u32,
    pub h: u32,
    pub alpha: bool,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Jpeg,
    Png,
    Webp,
    Bmp,
}

fn kind_of(data: &[u8]) -> Option<Kind> {
    if data.starts_with(&[0xFF, 0xD8]) {
        Some(Kind::Jpeg)
    } else if data.starts_with(b"\x89PNG") {
        Some(Kind::Png)
    } else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some(Kind::Webp)
    } else if data.starts_with(b"BM") {
        Some(Kind::Bmp)
    } else {
        None
    }
}

/// EXIF orientation (1..=8) from a JPEG APP1 block.
fn exif_orientation(exif: &[u8]) -> u16 {
    let d = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
    if d.len() < 8 {
        return 1;
    }
    let le = &d[0..2] == b"II";
    let u16_at = |o: usize| -> Option<u16> {
        let b = d.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    };
    let u32_at = |o: usize| -> Option<u32> {
        let b = d.get(o..o + 4)?;
        Some(if le {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        })
    };
    let Some(ifd) = u32_at(4) else { return 1 };
    let ifd = ifd as usize;
    let Some(n) = u16_at(ifd) else { return 1 };
    for i in 0..n as usize {
        let e = ifd + 2 + i * 12;
        if u16_at(e) == Some(0x0112) {
            return u16_at(e + 8).filter(|v| (1..=8).contains(v)).unwrap_or(1);
        }
    }
    1
}

fn orient(img: Img, o: u16) -> Img {
    if o <= 1 {
        return img;
    }
    let (w, h) = (img.w as usize, img.h as usize);
    let swap = o >= 5;
    let (nw, nh) = if swap { (h, w) } else { (w, h) };
    let mut out = Img::new(nw as u32, nh as u32);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (h - 1 - y, x),
                7 => (h - 1 - y, w - 1 - x),
                8 => (y, w - 1 - x),
                _ => (x, y),
            };
            let s = (y * w + x) * 4;
            let d = (dy * nw + dx) * 4;
            out.data[d..d + 4].copy_from_slice(&img.data[s..s + 4]);
        }
    }
    out
}

/// Size (after EXIF orientation) without decoding the pixels.
pub fn info(path: &Path) -> Option<Info> {
    let data = std::fs::read(path).ok()?;
    info_bytes(&data)
}

pub fn info_bytes(data: &[u8]) -> Option<Info> {
    let kind = kind_of(data)?;
    match kind {
        Kind::Jpeg => {
            let mut d = jpeg_decoder::Decoder::new(std::io::Cursor::new(data));
            d.read_info().ok()?;
            let i = d.info()?;
            let o = d.exif_data().map(exif_orientation).unwrap_or(1);
            let (w, h) = if o >= 5 {
                (i.height as u32, i.width as u32)
            } else {
                (i.width as u32, i.height as u32)
            };
            Some(Info {
                w,
                h,
                alpha: false,
                kind,
            })
        }
        Kind::Png => {
            let dec = png::Decoder::new(std::io::Cursor::new(data));
            let r = dec.read_info().ok()?;
            let i = r.info();
            let alpha = matches!(
                i.color_type,
                png::ColorType::Rgba | png::ColorType::GrayscaleAlpha
            ) || i.trns.is_some();
            Some(Info {
                w: i.width,
                h: i.height,
                alpha,
                kind,
            })
        }
        Kind::Webp => {
            let d = image_webp::WebPDecoder::new(std::io::Cursor::new(data)).ok()?;
            let (w, h) = d.dimensions();
            Some(Info {
                w,
                h,
                alpha: d.has_alpha(),
                kind,
            })
        }
        Kind::Bmp => {
            let (w, h, _, _) = bmp_header(data)?;
            Some(Info {
                w,
                h,
                alpha: false,
                kind,
            })
        }
    }
}

fn bmp_header(d: &[u8]) -> Option<(u32, u32, bool, u16)> {
    if d.len() < 54 {
        return None;
    }
    let w = i32::from_le_bytes(d[18..22].try_into().ok()?);
    let h = i32::from_le_bytes(d[22..26].try_into().ok()?);
    let bpp = u16::from_le_bytes(d[28..30].try_into().ok()?);
    let comp = u32::from_le_bytes(d[30..34].try_into().ok()?);
    if w <= 0 || h == 0 || !(comp == 0 || comp == 3) || !(bpp == 24 || bpp == 32) {
        return None;
    }
    Some((w as u32, h.unsigned_abs(), h > 0, bpp))
}

fn decode_bmp(d: &[u8]) -> Option<Img> {
    let (w, h, bottom_up, bpp) = bmp_header(d)?;
    let off = u32::from_le_bytes(d[10..14].try_into().ok()?) as usize;
    let bpp_bytes = (bpp / 8) as usize;
    let stride = (w as usize * bpp_bytes).div_ceil(4) * 4;
    let mut img = Img::new(w, h);
    for y in 0..h as usize {
        let sy = if bottom_up { h as usize - 1 - y } else { y };
        let row = off + sy * stride;
        for x in 0..w as usize {
            let s = row + x * bpp_bytes;
            let p = d.get(s..s + 3)?;
            let o = (y * w as usize + x) * 4;
            img.data[o] = p[2];
            img.data[o + 1] = p[1];
            img.data[o + 2] = p[0];
            img.data[o + 3] = 255;
        }
    }
    Some(img)
}

/// Decode `data`; a JPEG is decoded at the smallest DCT scale that is still
/// at least `want` (w, h) when given.
pub fn decode(data: &[u8], want: Option<(u32, u32)>) -> Option<Img> {
    match kind_of(data)? {
        Kind::Jpeg => {
            let mut d = jpeg_decoder::Decoder::new(std::io::Cursor::new(data));
            d.read_info().ok()?;
            let info = d.info()?;
            let o = d.exif_data().map(exif_orientation).unwrap_or(1);
            if (info.width as u64) * (info.height as u64) > MAX_IMAGE_PIXELS {
                return None;
            }
            if let Some((w, h)) = want {
                let (w, h) = if o >= 5 { (h, w) } else { (w, h) };
                let w = w.clamp(1, u16::MAX as u32) as u16;
                let h = h.clamp(1, u16::MAX as u32) as u16;
                d.scale(w, h).ok()?;
            }
            let px = d.decode().ok()?;
            let info = d.info()?;
            let (w, h) = (info.width as u32, info.height as u32);
            let mut img = Img::new(w, h);
            match info.pixel_format {
                jpeg_decoder::PixelFormat::RGB24 => {
                    for (i, p) in px.chunks_exact(3).enumerate() {
                        img.data[i * 4..i * 4 + 4].copy_from_slice(&[p[0], p[1], p[2], 255]);
                    }
                }
                jpeg_decoder::PixelFormat::L8 => {
                    for (i, &g) in px.iter().enumerate() {
                        img.data[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, 255]);
                    }
                }
                jpeg_decoder::PixelFormat::L16 => {
                    for (i, p) in px.chunks_exact(2).enumerate() {
                        let g = p[0];
                        img.data[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, 255]);
                    }
                }
                jpeg_decoder::PixelFormat::CMYK32 => {
                    for (i, p) in px.chunks_exact(4).enumerate() {
                        // Adobe CMYK JPEGs store inverted values.
                        let (c, m, y, k) = (p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32);
                        img.data[i * 4..i * 4 + 4].copy_from_slice(&[
                            (c * k / 255) as u8,
                            (m * k / 255) as u8,
                            (y * k / 255) as u8,
                            255,
                        ]);
                    }
                }
            }
            Some(orient(img, o))
        }
        Kind::Png => {
            let mut dec = png::Decoder::new(std::io::Cursor::new(data));
            dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
            let mut r = dec.read_info().ok()?;
            let (w, h) = (r.info().width, r.info().height);
            if (w as u64) * (h as u64) > MAX_IMAGE_PIXELS {
                return None;
            }
            let mut buf = vec![0u8; r.output_buffer_size()];
            let fi = r.next_frame(&mut buf).ok()?;
            let mut img = Img::new(w, h);
            let n = (w * h) as usize;
            match fi.color_type {
                png::ColorType::Rgba => img.data.copy_from_slice(&buf[..n * 4]),
                png::ColorType::Rgb => {
                    for i in 0..n {
                        img.data[i * 4..i * 4 + 3].copy_from_slice(&buf[i * 3..i * 3 + 3]);
                        img.data[i * 4 + 3] = 255;
                    }
                }
                png::ColorType::GrayscaleAlpha => {
                    for i in 0..n {
                        let g = buf[i * 2];
                        img.data[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, buf[i * 2 + 1]]);
                    }
                }
                png::ColorType::Grayscale => {
                    for i in 0..n {
                        let g = buf[i];
                        img.data[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, 255]);
                    }
                }
                png::ColorType::Indexed => return None,
            }
            Some(img)
        }
        Kind::Webp => {
            let mut d = image_webp::WebPDecoder::new(std::io::Cursor::new(data)).ok()?;
            let (w, h) = d.dimensions();
            if (w as u64) * (h as u64) > MAX_IMAGE_PIXELS {
                return None;
            }
            let alpha = d.has_alpha();
            let mut buf = vec![0u8; d.output_buffer_size()?];
            d.read_image(&mut buf).ok()?;
            let mut img = Img::new(w, h);
            if alpha {
                img.data.copy_from_slice(&buf[..(w * h * 4) as usize]);
            } else {
                for i in 0..(w * h) as usize {
                    img.data[i * 4..i * 4 + 3].copy_from_slice(&buf[i * 3..i * 3 + 3]);
                    img.data[i * 4 + 3] = 255;
                }
            }
            Some(img)
        }
        Kind::Bmp => decode_bmp(data),
    }
}

// --------------------------------------------------------- resampling

/// High quality resize: area averaging down, bilinear up (Qt's smooth scale).
pub fn resize(src: &Img, w: u32, h: u32) -> Img {
    let w = w.max(1);
    let h = h.max(1);
    if w == src.w && h == src.h {
        return src.clone();
    }
    // Work premultiplied so transparent pixels do not bleed colour.
    let tmp = resample_axis(src, w, true);
    resample_axis(&tmp, h, false)
}

fn resample_axis(src: &Img, n: u32, horizontal: bool) -> Img {
    let (sw, sh) = (src.w as usize, src.h as usize);
    let (len_src, other) = if horizontal { (sw, sh) } else { (sh, sw) };
    let len_dst = n as usize;
    let (ow, oh) = if horizontal { (len_dst, sh) } else { (sw, len_dst) };
    let mut out = Img::new(ow as u32, oh as u32);
    if len_src == 0 {
        return out;
    }
    let scale = len_src as f32 / len_dst as f32;
    // Contributions per destination index.
    let mut taps: Vec<Vec<(usize, f32)>> = Vec::with_capacity(len_dst);
    for d in 0..len_dst {
        let mut v = Vec::new();
        if scale > 1.0 {
            let a = d as f32 * scale;
            let b = a + scale;
            let mut s = a.floor() as usize;
            while (s as f32) < b && s < len_src {
                let lo = (s as f32).max(a);
                let hi = ((s + 1) as f32).min(b);
                if hi > lo {
                    v.push((s, (hi - lo) / scale));
                }
                s += 1;
            }
        } else {
            let f = (d as f32 + 0.5) * scale - 0.5;
            let s0 = f.floor().max(0.0) as usize;
            let s0 = s0.min(len_src - 1);
            let s1 = (s0 + 1).min(len_src - 1);
            let t = (f - s0 as f32).clamp(0.0, 1.0);
            v.push((s0, 1.0 - t));
            if s1 != s0 {
                v.push((s1, t));
            } else {
                v[0].1 = 1.0;
            }
        }
        taps.push(v);
    }
    let get = |line: usize, i: usize| -> usize {
        if horizontal {
            (line * sw + i) * 4
        } else {
            (i * sw + line) * 4
        }
    };
    for line in 0..other {
        for (d, tv) in taps.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(s, wgt) in tv {
                let o = get(line, s);
                let a = src.data[o + 3] as f32;
                acc[0] += src.data[o] as f32 * a * wgt;
                acc[1] += src.data[o + 1] as f32 * a * wgt;
                acc[2] += src.data[o + 2] as f32 * a * wgt;
                acc[3] += a * wgt;
            }
            let o = if horizontal {
                (line * ow + d) * 4
            } else {
                (d * ow + line) * 4
            };
            let a = acc[3];
            if a > 0.0 {
                out.data[o] = (acc[0] / a + 0.5).min(255.0) as u8;
                out.data[o + 1] = (acc[1] / a + 0.5).min(255.0) as u8;
                out.data[o + 2] = (acc[2] / a + 0.5).min(255.0) as u8;
            }
            out.data[o + 3] = (a + 0.5).min(255.0) as u8;
        }
    }
    out
}

/// `QImage::copy(x, y, w, h)` inside bounds.
pub fn crop(src: &Img, x: u32, y: u32, w: u32, h: u32) -> Img {
    let w = w.min(src.w.saturating_sub(x)).max(1);
    let h = h.min(src.h.saturating_sub(y)).max(1);
    let mut out = Img::new(w, h);
    for row in 0..h as usize {
        let s = ((y as usize + row) * src.w as usize + x as usize) * 4;
        let d = row * w as usize * 4;
        out.data[d..d + w as usize * 4].copy_from_slice(&src.data[s..s + w as usize * 4]);
    }
    out
}

/// `QSize::scaled(target, mode)` with Qt's integer arithmetic.
pub fn qt_scaled(sw: u32, sh: u32, tw: u32, th: u32, expand: bool) -> (u32, u32) {
    if sw == 0 || sh == 0 {
        return (tw, th);
    }
    let rw = (th as i64) * (sw as i64) / (sh as i64);
    let use_height = if expand {
        rw >= tw as i64
    } else {
        rw <= tw as i64
    };
    if use_height {
        (rw.max(1) as u32, th.max(1))
    } else {
        (tw.max(1), ((tw as i64) * (sh as i64) / (sw as i64)).max(1) as u32)
    }
}

/// `qGray` greyscale (Format_Grayscale8 is opaque).
pub fn grayscale(src: &Img) -> Img {
    let mut out = src.clone();
    for p in out.data.chunks_exact_mut(4) {
        let a = p[3] as u32;
        let g = ((p[0] as u32 * 11 + p[1] as u32 * 16 + p[2] as u32 * 5) / 32) * a / 255;
        p[0] = g as u8;
        p[1] = g as u8;
        p[2] = g as u8;
        p[3] = 255;
    }
    out
}

/// Gaussian-like blur (three box passes) of a `radius` like
/// QGraphicsBlurEffect; outside the image counts as transparent.
pub fn blur(src: &Img, radius: f32) -> Img {
    if radius <= 0.0 {
        return src.clone();
    }
    let sigma = radius / 2.0;
    let boxes = box_sizes(sigma);
    let (w, h) = (src.w as usize, src.h as usize);
    let mut buf: Vec<[f32; 4]> = src
        .data
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32;
            [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, a]
        })
        .collect();
    let mut tmp = buf.clone();
    for r in boxes {
        box_pass(&buf, &mut tmp, w, h, r, true);
        box_pass(&tmp, &mut buf, w, h, r, false);
    }
    let mut out = Img::new(src.w, src.h);
    for (i, p) in buf.iter().enumerate() {
        let a = p[3];
        if a > 0.0 {
            out.data[i * 4] = (p[0] / a).clamp(0.0, 255.0) as u8;
            out.data[i * 4 + 1] = (p[1] / a).clamp(0.0, 255.0) as u8;
            out.data[i * 4 + 2] = (p[2] / a).clamp(0.0, 255.0) as u8;
        }
        out.data[i * 4 + 3] = a.clamp(0.0, 255.0) as u8;
    }
    out
}

fn box_sizes(sigma: f32) -> [usize; 3] {
    let n = 3.0;
    let wi = ((12.0 * sigma * sigma / n) + 1.0).sqrt().floor();
    let wl = if (wi as i32) % 2 == 0 { wi - 1.0 } else { wi };
    let wu = wl + 2.0;
    let m = ((12.0 * sigma * sigma - n * wl * wl - 4.0 * n * wl - 3.0 * n) / (-4.0 * wl - 4.0)).round();
    let mut out = [0usize; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let size = if (i as f32) < m { wl } else { wu };
        *o = ((size - 1.0) / 2.0).max(0.0) as usize;
    }
    out
}

fn box_pass(src: &[[f32; 4]], dst: &mut [[f32; 4]], w: usize, h: usize, r: usize, horizontal: bool) {
    if r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let idx = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
    let norm = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let mut acc = [0f32; 4];
        for i in 0..=r.min(len.saturating_sub(1)) {
            let p = src[idx(line, i)];
            for k in 0..4 {
                acc[k] += p[k];
            }
        }
        for i in 0..len {
            let o = idx(line, i);
            for k in 0..4 {
                dst[o][k] = acc[k] * norm;
            }
            let add = i + r + 1;
            if add < len {
                let p = src[idx(line, add)];
                for k in 0..4 {
                    acc[k] += p[k];
                }
            }
            if i >= r {
                let p = src[idx(line, i - r)];
                for k in 0..4 {
                    acc[k] -= p[k];
                }
            }
        }
    }
}

// -------------------------------------------------------------- caches

type SrcKey = (PathBuf, i64, u64, u32, u32);

struct Cache {
    sources: Vec<(SrcKey, Arc<Img>)>,
    src_bytes: usize,
    prepared: Vec<(String, Arc<Img>)>,
    prep_bytes: usize,
    dims: HashMap<PathBuf, (u32, u32)>,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
const SOURCE_BUDGET: usize = 64 << 20;
const PREPARED_BUDGET: usize = 48 << 20;

fn cache() -> std::sync::MutexGuard<'static, Cache> {
    CACHE
        .get_or_init(|| {
            Mutex::new(Cache {
                sources: Vec::new(),
                src_bytes: 0,
                prepared: Vec::new(),
                prep_bytes: 0,
                dims: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Drop every cached image (after a deck closes).
pub fn clear_cache() {
    let mut c = cache();
    c.sources.clear();
    c.src_bytes = 0;
    c.prepared.clear();
    c.prepared.shrink_to_fit();
    c.prep_bytes = 0;
}

fn stat_key(path: &Path) -> Option<(PathBuf, i64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(path).ok()?;
    if !m.is_file() {
        return None;
    }
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some((real, m.mtime_nsec() + m.mtime() * 1_000_000_000, m.len()))
}

/// `_cached_pixmap`: the image decoded for a target size, bucketed to 256 px.
pub fn cached_source(path: &Path, tw: u32, th: u32) -> Option<Arc<Img>> {
    let (real, mtime, len) = stat_key(path)?;
    let tw = tw.max(64);
    let th = th.max(64);
    let bw = (tw.div_ceil(256) * 256).min(4096);
    let bh = (th.div_ceil(256) * 256).min(4096);
    let key = (real.clone(), mtime, len, bw, bh);
    {
        let mut c = cache();
        if let Some(i) = c.sources.iter().position(|(k, _)| *k == key) {
            let e = c.sources.remove(i);
            let img = e.1.clone();
            c.sources.push(e);
            return Some(img);
        }
    }
    let data = std::fs::read(&real).ok()?;
    let info = info_bytes(&data)?;
    if info.w == 0 || info.h == 0 || (info.w as u64) * (info.h as u64) > MAX_IMAGE_PIXELS {
        return None;
    }
    let scale = (1.0f64).min((bw as f64 / info.w as f64).max(bh as f64 / info.h as f64) * 1.15);
    let (want_w, want_h) = if scale < 1.0 {
        (
            ((info.w as f64 * scale) as u32).max(1),
            ((info.h as f64 * scale) as u32).max(1),
        )
    } else {
        (info.w, info.h)
    };
    let mut img = decode(&data, Some((want_w, want_h)))?;
    drop(data);
    if img.w != want_w || img.h != want_h {
        img = resize(&img, want_w, want_h);
    }
    let img = Arc::new(img);
    let cost = img.data.len().max(1);
    let mut c = cache();
    c.dims.insert(real, (info.w, info.h));
    c.sources.push((key, img.clone()));
    c.src_bytes += cost;
    while c.src_bytes > SOURCE_BUDGET && c.sources.len() > 1 {
        let (_, old) = c.sources.remove(0);
        c.src_bytes -= old.data.len().max(1);
    }
    Some(img)
}

/// The full size of an image (cheap after the first look).
pub fn dimensions(path: &Path) -> (u32, u32) {
    if let Some(d) = stat_key(path).and_then(|(r, _, _)| cache().dims.get(&r).copied()) {
        return d;
    }
    info(path).map(|i| (i.w, i.h)).unwrap_or((0, 0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Treatment {
    pub fill: bool,
    pub mono: bool,
    pub fx: f32,
    pub fy: f32,
    pub brightness: i32,
    pub contrast: i32,
    pub overlay: i32,
    pub blur: i32,
    pub zoom: f32,
}

/// `draw_image` up to the paint: returns the destination rectangle (in
/// pixels of the target) and the finished image for it. `scale` maps the
/// slide target size to output pixels (PDF uses a higher density).
pub fn prepare(path: &Path, tw: i32, th: i32, t: &Treatment, density: f32) -> Option<(i32, i32, i32, i32, Arc<Img>)> {
    if tw <= 0 || th <= 0 {
        return None;
    }
    let (real, mtime, len) = stat_key(path)?;
    let key = format!(
        "{}|{mtime}|{len}|{tw}x{th}|{:?}|{density}",
        real.display(),
        t
    );
    {
        let mut c = cache();
        if let Some(i) = c.prepared.iter().position(|(k, _)| *k == key) {
            let e = c.prepared.remove(i);
            let img = e.1.clone();
            c.prepared.push(e);
            return Some(placement(tw, th, &img, t, density, img.clone()));
        }
    }
    let src = cached_source(path, tw as u32, th as u32)?;
    let mut pm: Img = if t.mono { grayscale(&src) } else { (*src).clone() };
    if t.blur > 0 {
        pm = blur(&pm, t.blur as f32);
    }
    let zoom = if t.fill {
        t.zoom.clamp(1.0, 1.18)
    } else {
        1.0
    };
    // Slide-space target, then output pixels.
    let sw = ((tw as f32 * zoom) as u32).max(1);
    let sh = ((th as f32 * zoom) as u32).max(1);
    let (scw, sch) = qt_scaled(pm.w, pm.h, sw, sh, t.fill);
    let d = density.max(0.01);
    let (ow, oh) = (
        ((scw as f32 * d).round() as u32).max(1),
        ((sch as f32 * d).round() as u32).max(1),
    );
    let mut scaled = resize(&pm, ow, oh);
    drop(pm);
    if t.fill {
        let maxx = scw.saturating_sub(tw as u32);
        let maxy = sch.saturating_sub(th as u32);
        let sx = (maxx as f32 * t.fx.clamp(0.0, 1.0)) as u32;
        let sy = (maxy as f32 * t.fy.clamp(0.0, 1.0)) as u32;
        let cw = (tw as u32).min(scw - sx);
        let ch = (th as u32).min(sch - sy);
        scaled = crop(
            &scaled,
            (sx as f32 * d).round() as u32,
            (sy as f32 * d).round() as u32,
            ((cw as f32 * d).round() as u32).max(1),
            ((ch as f32 * d).round() as u32).max(1),
        );
    }
    apply_tone(&mut scaled, t);
    let img = Arc::new(scaled);
    {
        let mut c = cache();
        c.prep_bytes += img.data.len();
        c.prepared.push((key, img.clone()));
        while c.prep_bytes > PREPARED_BUDGET && c.prepared.len() > 1 {
            let (_, old) = c.prepared.remove(0);
            c.prep_bytes -= old.data.len();
        }
    }
    Some(placement(tw, th, &img, t, density, img.clone()))
}

fn placement(tw: i32, th: i32, img: &Img, t: &Treatment, density: f32, arc: Arc<Img>) -> (i32, i32, i32, i32, Arc<Img>) {
    let d = density.max(0.01);
    let iw = (img.w as f32 / d).round() as i32;
    let ih = (img.h as f32 / d).round() as i32;
    if t.fill {
        (0, 0, iw.min(tw), ih.min(th), arc)
    } else {
        // Centred: QRectF(...).toRect() of the centred position.
        let x = ((tw - iw) as f32 / 2.0).round() as i32;
        let y = ((th - ih) as f32 / 2.0).round() as i32;
        (x, y, iw, ih, arc)
    }
}

/// Brightness veil, Overlay-mode contrast and the dark overlay, baked in.
fn apply_tone(img: &mut Img, t: &Treatment) {
    let br = t.brightness;
    let bright = if br > 0 {
        Some((255u32, (br as f32 * 1.5).min(150.0) as u32))
    } else if br < 0 {
        Some((0u32, ((-br) as f32 * 1.5).min(150.0) as u32))
    } else {
        None
    };
    let contrast = if t.contrast > 0 {
        Some(t.contrast.min(100) as u32)
    } else {
        None
    };
    let overlay = if t.overlay != 0 {
        Some((t.overlay as f32 * 2.2).clamp(0.0, 220.0) as u32)
    } else {
        None
    };
    if bright.is_none() && contrast.is_none() && overlay.is_none() {
        return;
    }
    for p in img.data.chunks_exact_mut(4) {
        for c in p.iter_mut().take(3) {
            let mut v = *c as u32;
            if let Some((col, a)) = bright {
                v = (col * a + v * (255 - a) + 127) / 255;
            }
            if let Some(a) = contrast {
                // Overlay blend of white: 2b below the midpoint, else white.
                let ov = if v * 2 <= 255 { (2 * v).min(255) } else { 255 };
                v = (ov * a + v * (255 - a) + 127) / 255;
            }
            if let Some(a) = overlay {
                v = (v * (255 - a) + 127) / 255;
            }
            *c = v.min(255) as u8;
        }
    }
}

// ------------------------------------------------------- encode / sha

pub fn encode_png(img: &Img) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, img.w, img.h);
        let alpha = img.has_alpha();
        e.set_color(if alpha {
            png::ColorType::Rgba
        } else {
            png::ColorType::Rgb
        });
        e.set_depth(png::BitDepth::Eight);
        e.set_compression(png::Compression::Default);
        let mut w = e.write_header().ok()?;
        if alpha {
            w.write_image_data(&img.data).ok()?;
        } else {
            let rgb: Vec<u8> = img
                .data
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            w.write_image_data(&rgb).ok()?;
        }
    }
    Some(out)
}

pub fn encode_png_rgb(w: u32, h: u32, rgb: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().ok()?;
        wr.write_image_data(rgb).ok()?;
    }
    Some(out)
}

pub fn encode_jpeg(img: &Img, quality: u8) -> Option<Vec<u8>> {
    let rgb: Vec<u8> = img
        .data
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    encode_jpeg_rgb(img.w, img.h, &rgb, quality)
}

pub fn encode_jpeg_rgb(w: u32, h: u32, rgb: &[u8], quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let enc = jpeg_encoder::Encoder::new(&mut out, quality);
    enc.encode(rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
        .ok()?;
    Some(out)
}

pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn qt_size_scaling() {
        assert_eq!(qt_scaled(1600, 1000, 800, 800, false), (800, 500));
        assert_eq!(qt_scaled(1600, 1000, 800, 800, true), (1280, 800));
    }

    #[test]
    fn png_round_trip_and_resize() {
        let mut img = Img::new(4, 2);
        for p in img.data.chunks_exact_mut(4) {
            p.copy_from_slice(&[200, 100, 50, 255]);
        }
        let bytes = encode_png(&img).unwrap();
        let back = decode(&bytes, None).unwrap();
        assert_eq!((back.w, back.h), (4, 2));
        assert_eq!(&back.data[0..4], &[200, 100, 50, 255]);
        let small = resize(&back, 2, 1);
        assert_eq!(&small.data[0..4], &[200, 100, 50, 255]);
    }
}
