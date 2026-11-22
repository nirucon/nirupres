//! Pixel-based text fitting for slide templates, a port of the 2.3 textfit
//! module: text is laid out at a pixel size the way QTextLayout does it
//! (paragraphs, word wrap with a break-anywhere fallback, emphasis runs),
//! fitted to a box and line limit, then clipped with an ellipsis. Overflow
//! is reported to the active report (Deck health).
use crate::fonts::{self, Face, ShapedGlyph};
use crate::markup;
use crate::paint::{GlyphRun, Painter, RectF, Rgba};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub const ELLIPSIS: char = '…';

pub const W_LIGHT: u16 = 300;
pub const W_NORMAL: u16 = 400;
pub const W_MEDIUM: u16 = 500;
pub const W_DEMIBOLD: u16 = 600;
pub const W_BOLD: u16 = 700;
pub const W_BLACK: u16 = 900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Align {
    pub h: HAlign,
    pub v: VAlign,
}

impl Align {
    pub const fn new(h: HAlign, v: VAlign) -> Self {
        Self { h, v }
    }
}

#[derive(Clone, Debug)]
pub struct Issue {
    pub kind: &'static str,
    pub label: String,
    pub count: usize,
}

thread_local! {
    static REPORT: RefCell<Option<Vec<Issue>>> = const { RefCell::new(None) };
    static TRACE: RefCell<Option<Vec<(RectF, RectF, String)>>> = const { RefCell::new(None) };
    static FIT_CACHE: RefCell<HashMap<FitKey, Option<i32>>> = RefCell::new(HashMap::new());
}

type FitKey = (String, Vec<(usize, usize, bool, bool)>, String, u16, i32, i32, i64, i64, usize, bool);

/// Collect issues (and optionally painted line boxes) while `f` runs.
pub fn collect<T>(trace: bool, f: impl FnOnce() -> T) -> (T, Vec<Issue>, Vec<(RectF, RectF, String)>) {
    let old_r = REPORT.with(|r| r.borrow_mut().replace(Vec::new()));
    let old_t = TRACE.with(|t| {
        let mut t = t.borrow_mut();
        let old = t.take();
        if trace {
            *t = Some(Vec::new());
        }
        old
    });
    let v = f();
    let rep = REPORT.with(|r| std::mem::replace(&mut *r.borrow_mut(), old_r)).unwrap_or_default();
    let tr = TRACE.with(|t| std::mem::replace(&mut *t.borrow_mut(), old_t)).unwrap_or_default();
    (v, rep, tr)
}

pub fn note(kind: &'static str, label: &str, count: usize) {
    REPORT.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            r.push(Issue {
                kind,
                label: label.to_string(),
                count,
            });
        }
    });
}

fn trace(boxr: RectF, r: RectF, text: &str) {
    TRACE.with(|t| {
        if let Some(t) = t.borrow_mut().as_mut() {
            t.push((boxr, r, text.to_string()));
        }
    });
}

pub fn more_label(count: usize) -> String {
    format!("+{count} more")
}

/// Formats as (start char, length, bold, italic) over the plain text.
type Formats = Vec<(usize, usize, bool, bool)>;

pub fn plain_and_formats(text: &str, markup_on: bool) -> (String, Formats) {
    let text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ");
    if !markup_on {
        return (text, Vec::new());
    }
    let mut plain = String::new();
    let mut fmts = Vec::new();
    let mut pos = 0;
    for r in markup::runs(&text) {
        let n = r.text.chars().count();
        if r.bold || r.italic {
            fmts.push((pos, n, r.bold, r.italic));
        }
        plain.push_str(&r.text);
        pos += n;
    }
    (plain, fmts)
}

pub fn face(family: &str, weight: u16, italic: bool) -> Option<Arc<Face>> {
    fonts::db().face(family, weight, italic)
}

/// One laid-out line.
#[derive(Clone)]
pub struct Line {
    /// Char range in the plain text.
    pub start: usize,
    pub end: usize,
    pub y: f32,
    pub ascent: f32,
    pub height: f32,
    pub natural: f32,
    pub x_off: f32,
    glyphs: Vec<(ShapedGlyph, f32)>,
}

pub struct Laid {
    pub lines: Vec<Line>,
    pub height: f32,
    pub width_ok: bool,
    pub step: f32,
    pub base: Arc<Face>,
    pub px: f32,
}

struct Seg {
    face: Arc<Face>,
    weight: u16,
    italic: bool,
}

fn seg_for(base: &Arc<Face>, family: &str, weight: u16, b: bool, i: bool) -> Seg {
    if !b && !i {
        return Seg {
            face: base.clone(),
            weight,
            italic: false,
        };
    }
    let w = if b {
        if weight < W_BOLD {
            W_BOLD
        } else {
            W_BLACK
        }
    } else {
        weight
    };
    let f = face(family, w, i).unwrap_or_else(|| base.clone());
    Seg {
        face: f,
        weight: w,
        italic: i,
    }
}

/// Lay out `plain` at `px` into `width` (paragraphs split on newlines).
pub fn layout(
    plain: &str,
    fmts: &Formats,
    family: &str,
    weight: u16,
    px: f32,
    width: f32,
    wrap: bool,
    align: HAlign,
) -> Option<Laid> {
    let base = face(family, weight, false)?;
    let step = base.line_spacing(px);
    let width = width.max(1.0);
    let mut lines = Vec::new();
    let mut y = 0.0f32;
    let mut width_ok = true;
    let chars: Vec<char> = plain.chars().collect();
    let mut pstart = 0usize;
    let mut para_ranges = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '\n' {
            para_ranges.push((pstart, i));
            pstart = i + 1;
        }
    }
    para_ranges.push((pstart, chars.len()));
    for (ps, pe) in para_ranges {
        let para = &chars[ps..pe];
        // Shape in runs of equal emphasis.
        let mut glyphs: Vec<ShapedGlyph> = Vec::new();
        let mut cut = vec![ps];
        for &(fs, fl, _, _) in fmts.iter() {
            cut.push(fs.clamp(ps, pe));
            cut.push((fs + fl).clamp(ps, pe));
        }
        cut.push(pe);
        cut.sort_unstable();
        cut.dedup();
        for w in cut.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a >= b {
                continue;
            }
            let (mut bo, mut it) = (false, false);
            for &(fs, fl, fb, fi) in fmts.iter() {
                if a >= fs && a < fs + fl {
                    bo |= fb;
                    it |= fi;
                }
            }
            let seg = seg_for(&base, family, weight, bo, it);
            let mut g = fonts::shape(&chars[a..b], &seg.face, px, seg.weight, seg.italic);
            for x in g.iter_mut() {
                x.cluster += a;
            }
            glyphs.extend(g);
        }
        // Width per char (cluster start gets the cluster's advance).
        let n = para.len();
        let mut cw = vec![0f32; n + 1];
        for g in &glyphs {
            let ci = g.cluster.saturating_sub(ps).min(n);
            cw[ci] += g.x_advance;
        }
        let breaks = break_points(para);
        let line_ranges = if wrap {
            wrap_ranges(para, &cw[..n], &breaks, width)
        } else {
            vec![(0, n)]
        };
        let mut gi = 0usize;
        for (ls, le) in line_ranges {
            let mut natural_end = le;
            while natural_end > ls && para[natural_end - 1].is_whitespace() {
                natural_end -= 1;
            }
            let natural: f32 = cw[ls..natural_end].iter().sum();
            if natural > width + 0.5 {
                width_ok = false;
            }
            let mut lg = Vec::new();
            let mut x = 0.0f32;
            let mut asc = base.ascent(px);
            let mut desc = base.descent(px);
            while gi < glyphs.len() && glyphs[gi].cluster < ps + le {
                let g = &glyphs[gi];
                if g.cluster >= ps + ls {
                    if g.face.id != base.id {
                        asc = asc.max(g.face.ascent(px));
                        desc = desc.max(g.face.descent(px));
                    }
                    lg.push((g.clone(), x));
                    x += g.x_advance;
                }
                gi += 1;
            }
            let x_off = match align {
                HAlign::Left => 0.0,
                HAlign::Center => (width - natural) / 2.0,
                HAlign::Right => width - natural,
            };
            // QTextLine::height() rounds the line height up.
            let height = (asc + desc).ceil();
            lines.push(Line {
                start: ps + ls,
                end: ps + le,
                y,
                ascent: asc,
                height,
                natural,
                x_off,
                glyphs: lg,
            });
            y += step.max(height);
        }
    }
    Some(Laid {
        lines,
        height: y,
        width_ok,
        step,
        base,
        px,
    })
}

/// Allowed break positions (char indices where a new line may start).
fn break_points(para: &[char]) -> Vec<bool> {
    let s: String = para.iter().collect();
    let mut byte_to_char = vec![0usize; s.len() + 1];
    let mut ci = 0;
    for (bi, _) in s.char_indices() {
        byte_to_char[bi] = ci;
        ci += 1;
    }
    byte_to_char[s.len()] = ci;
    let mut out = vec![false; para.len() + 1];
    for (bi, _) in unicode_linebreak::linebreaks(&s) {
        out[byte_to_char[bi]] = true;
    }
    out
}

/// Greedy line filling: whole words where they fit, else anywhere.
fn wrap_ranges(para: &[char], cw: &[f32], breaks: &[bool], width: f32) -> Vec<(usize, usize)> {
    let n = para.len();
    if n == 0 {
        return vec![(0, 0)];
    }
    let mut out = Vec::new();
    let mut s = 0usize;
    let eps = 0.01;
    while s < n {
        // Width of [s, e) without trailing whitespace.
        let mut best: Option<usize> = None;
        let mut w = 0.0f32;
        let mut w_trim = 0.0f32;
        let mut e = s;
        while e < n {
            w += cw[e];
            if !para[e].is_whitespace() {
                w_trim = w;
            }
            e += 1;
            if w_trim > width + eps {
                break;
            }
            if breaks[e] || e == n {
                best = Some(e);
            }
        }
        let end = match best {
            Some(b) if b > s => b,
            _ => {
                // A word longer than the line: break anywhere.
                let mut w = 0.0f32;
                let mut e = s;
                while e < n {
                    if e > s && w + cw[e] > width + eps && !para[e].is_whitespace() {
                        break;
                    }
                    w += cw[e];
                    e += 1;
                }
                // Keep spaces that follow on this line.
                while e < n && para[e].is_whitespace() && para[e] != '\u{a0}' {
                    e += 1;
                }
                e.max(s + 1)
            }
        };
        out.push((s, end));
        s = end;
    }
    out
}

fn fits(l: &Laid, rect: &RectF, max_lines: Option<usize>) -> bool {
    l.width_ok
        && l.height <= rect.h + 0.5
        && max_lines.is_none_or(|m| m == 0 || l.lines.len() <= m)
}

#[allow(clippy::too_many_arguments)]
pub fn fit_size(
    text: &str,
    family: &str,
    weight: u16,
    max_px: f32,
    min_px: f32,
    rect: &RectF,
    max_lines: Option<usize>,
    markup_on: bool,
    wrap: bool,
) -> Option<i32> {
    let (plain, fmts) = plain_and_formats(text, markup_on);
    let max_px = (max_px as i32).max(1);
    let min_px = (min_px as i32).min(max_px).max(1);
    let key: FitKey = (
        plain.clone(),
        fmts.clone(),
        family.to_string(),
        weight,
        max_px,
        min_px,
        (rect.w * 10.0).round() as i64,
        (rect.h * 10.0).round() as i64,
        max_lines.unwrap_or(0),
        wrap,
    );
    if let Some(v) = FIT_CACHE.with(|c| c.borrow().get(&key).copied()) {
        return v;
    }
    let ok = |px: i32| {
        layout(&plain, &fmts, family, weight, px as f32, rect.w, wrap, HAlign::Left)
            .is_some_and(|l| fits(&l, rect, max_lines))
    };
    let result = if plain.trim().is_empty() || ok(max_px) {
        Some(max_px)
    } else if ok(min_px) {
        let (mut lo, mut hi) = (min_px, max_px);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if ok(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    } else {
        None
    };
    FIT_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 6000 {
            c.clear();
        }
        c.insert(key, result);
    });
    result
}

/// Fit `text` into `rect` and paint it; returns the pixel size used.
/// `px` forces a size (paired columns share one size).
#[allow(clippy::too_many_arguments)]
pub fn fit_draw(
    p: &mut dyn Painter,
    rect: RectF,
    text: &str,
    family: &str,
    weight: u16,
    max_px: f32,
    min_px: f32,
    color: Rgba,
    align: Align,
    max_lines: Option<usize>,
    label: &str,
    markup_on: bool,
    wrap: bool,
    px: Option<i32>,
) -> i32 {
    let (plain, fmts) = plain_and_formats(text, markup_on);
    if plain.trim().is_empty() || rect.w < 2.0 || rect.h < 2.0 {
        return px.unwrap_or(max_px as i32);
    }
    let (mut size, mut clipped) = match px {
        None => match fit_size(text, family, weight, max_px, min_px, &rect, max_lines, markup_on, wrap) {
            Some(s) => (s, false),
            None => (((min_px.min(max_px)) as i32).max(1), true),
        },
        Some(s) => (s.max(1), false),
    };
    let mut wrap = wrap;
    let Some(mut laid) = layout(&plain, &fmts, family, weight, size as f32, rect.w, wrap, align.h) else {
        return size;
    };
    if !wrap && !laid.width_ok {
        // Code still too wide at the minimum wraps rather than overflowing.
        wrap = true;
        clipped = false;
        if let Some(l) = layout(&plain, &fmts, family, weight, size as f32, rect.w, true, align.h) {
            laid = l;
        }
    }
    if !fits(&laid, &rect, max_lines) {
        clipped = true;
    }
    let mut step = laid.step;
    if step > rect.h {
        // Even one line is taller than the box: shrink below the minimum.
        size = ((size as f32 * rect.h / step) as i32).max(1);
        if let Some(l) = layout(&plain, &fmts, family, weight, size as f32, rect.w, wrap, align.h) {
            laid = l;
        }
        step = laid.step;
        clipped = true;
    }
    let total = laid.lines.len();
    let mut shown = total;
    if clipped {
        let mut cap = (((rect.h + 0.5) / step.max(1.0)).floor() as usize).max(1);
        if let Some(m) = max_lines.filter(|m| *m > 0) {
            cap = cap.min(m);
        }
        shown = shown.min(cap);
        while shown > 1 {
            let l = &laid.lines[shown - 1];
            if l.y + l.height > rect.h + 0.5 {
                shown -= 1;
            } else {
                break;
            }
        }
    }
    let truncated = shown < total || !laid.width_ok;
    let used_h = if shown > 0 {
        let l = &laid.lines[shown - 1];
        l.y + step.max(l.height)
    } else {
        0.0
    };
    let mut y0 = rect.y;
    match align.v {
        VAlign::Center => y0 += ((rect.h - used_h) / 2.0).max(0.0),
        VAlign::Bottom => y0 += (rect.h - used_h).max(0.0),
        VAlign::Top => {}
    }
    let chars: Vec<char> = plain.chars().collect();
    p.push_clip(rect);
    for n in 0..shown {
        let line = &laid.lines[n];
        let ly = y0 + line.y;
        if truncated && n == shown - 1 {
            let rest: String = chars[line.start..].iter().map(|&c| if c == '\n' { ' ' } else { c }).collect();
            let elided = elide(&laid.base, laid.px, &rest, rect.w - 1.0);
            let wid = text_width(&laid.base, laid.px, &elided);
            let hx = match align.h {
                HAlign::Left => rect.x,
                HAlign::Right => rect.x + rect.w - wid,
                HAlign::Center => rect.x + rect.w / 2.0 - wid / 2.0,
            };
            let base_y = ly + laid.base.ascent(laid.px);
            draw_plain(p, &laid.base, laid.px, hx, base_y, &elided, color, weight);
            trace(rect, RectF::new(hx, ly, wid, step), &elided);
        } else {
            let base_y = ly + line.ascent;
            let x0 = rect.x + line.x_off;
            emit_line(p, line, &chars, x0, base_y, laid.px, color);
            let s: String = chars[line.start..line.end].iter().collect();
            trace(
                rect,
                RectF::new(x0, ly, line.natural, step.max(line.height)),
                &s,
            );
        }
    }
    p.pop_clip();
    if clipped || truncated {
        note("clipped", label, total - shown);
    }
    size
}

fn emit_line(p: &mut dyn Painter, line: &Line, chars: &[char], x0: f32, base_y: f32, px: f32, color: Rgba) {
    let mut i = 0;
    while i < line.glyphs.len() {
        let fid = line.glyphs[i].0.face.id;
        let face = line.glyphs[i].0.face.clone();
        let mut run = GlyphRun {
            face,
            px,
            glyphs: Vec::new(),
            text: String::new(),
        };
        let mut last_cluster = usize::MAX;
        while i < line.glyphs.len() && line.glyphs[i].0.face.id == fid {
            let (g, x) = &line.glyphs[i];
            run.glyphs
                .push((g.gid, x0 + x + g.x_offset, base_y - g.y_offset));
            if g.cluster != last_cluster {
                if let Some(c) = chars.get(g.cluster) {
                    run.text.push(*c);
                }
                last_cluster = g.cluster;
            }
            i += 1;
        }
        p.glyphs(&run, color);
    }
}

/// Width of plain text in `face` at `px` (shaped, with fallback).
pub fn text_width(base: &Arc<Face>, px: f32, s: &str) -> f32 {
    let chars: Vec<char> = s.chars().collect();
    fonts::shape(&chars, base, px, W_NORMAL, false)
        .iter()
        .map(|g| g.x_advance)
        .sum()
}

/// Draw plain text with its baseline at `base_y`; returns the advance.
#[allow(clippy::too_many_arguments)]
pub fn draw_plain(
    p: &mut dyn Painter,
    base: &Arc<Face>,
    px: f32,
    x: f32,
    base_y: f32,
    s: &str,
    color: Rgba,
    weight: u16,
) -> f32 {
    let chars: Vec<char> = s.chars().collect();
    let gl = fonts::shape(&chars, base, px, weight, false);
    let mut pen = x;
    let mut i = 0;
    while i < gl.len() {
        let fid = gl[i].face.id;
        let mut run = GlyphRun {
            face: gl[i].face.clone(),
            px,
            glyphs: Vec::new(),
            text: String::new(),
        };
        let mut last_cluster = usize::MAX;
        while i < gl.len() && gl[i].face.id == fid {
            let g = &gl[i];
            run.glyphs.push((g.gid, pen + g.x_offset, base_y - g.y_offset));
            if g.cluster != last_cluster {
                if let Some(c) = chars.get(g.cluster) {
                    run.text.push(*c);
                }
                last_cluster = g.cluster;
            }
            pen += g.x_advance;
            i += 1;
        }
        p.glyphs(&run, color);
    }
    pen - x
}

/// `QFontMetrics::elidedText(..., ElideRight, width)` plus the 2.x rule
/// that a clipped line always ends with an ellipsis.
pub fn elide(base: &Arc<Face>, px: f32, s: &str, width: f32) -> String {
    let full = text_width(base, px, s);
    let mut out: String;
    if full <= width {
        out = s.to_string();
    } else {
        let chars: Vec<char> = s.chars().collect();
        let ell = text_width(base, px, "…");
        let gl = fonts::shape(&chars, base, px, W_NORMAL, false);
        let mut w = 0.0;
        let mut cut = 0;
        let mut acc = vec![0f32; chars.len() + 1];
        for g in &gl {
            acc[g.cluster.min(chars.len())] += g.x_advance;
        }
        for (i, a) in acc.iter().take(chars.len()).enumerate() {
            if w + a + ell > width {
                break;
            }
            w += a;
            cut = i + 1;
        }
        out = chars[..cut].iter().collect();
        out.push('…');
    }
    if !out.ends_with('…') {
        out = format!("{}…", out.trim_end());
        while out != "…" && text_width(base, px, &out) > width {
            let mut c: Vec<char> = out.chars().collect();
            let n = c.len();
            if n >= 2 {
                c.remove(n - 2);
            }
            out = c.into_iter().collect();
        }
    }
    out
}

/// (line count, line step, height) of `text` at `px` in `width`.
pub fn measure(text: &str, family: &str, weight: u16, px: f32, width: f32) -> (usize, f32, f32) {
    let (plain, fmts) = plain_and_formats(text, true);
    if plain.trim().is_empty() {
        let step = face(family, weight, false)
            .map(|f| f.line_spacing(px))
            .unwrap_or(px);
        return (0, step, 0.0);
    }
    match layout(&plain, &fmts, family, weight, px, width, true, HAlign::Left) {
        Some(l) => (l.lines.len(), l.step, l.height),
        None => (0, px, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_on_words_then_anywhere() {
        let para: Vec<char> = "aa bb cccccccc".chars().collect();
        let cw = vec![1.0; para.len()];
        let br = break_points(&para);
        let r = wrap_ranges(&para, &cw, &br, 5.0);
        assert_eq!(r, vec![(0, 6), (6, 11), (11, 14)]);
    }

    #[test]
    fn plain_text_and_formats() {
        let (p, f) = plain_and_formats("a **b**\tc", true);
        assert_eq!(p, "a b    c");
        assert_eq!(f, vec![(2, 1, true, false)]);
    }
}
