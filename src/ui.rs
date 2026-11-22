//! A small widget kit on the canvas: chrome text, buttons, check boxes,
//! combo boxes, sliders, single and multi-line text editing (with
//! selection, clipboard and undo), popup lists and tooltips.
use crate::canvas::{Canvas, RasterPainter, Rect};
use crate::chrome::{blend, Chrome};
use crate::fonts::{self, Face};
use crate::paint::Rgba;
use crate::textfit;
use std::sync::Arc;

pub struct Fonts {
    pub sans: Arc<Face>,
    pub semi: Arc<Face>,
    pub bold: Arc<Face>,
    pub black: Arc<Face>,
    pub mono: Arc<Face>,
    pub mono_bold: Arc<Face>,
}

impl Fonts {
    pub fn load() -> Option<Self> {
        let f = |fam: &str, w: u16| textfit::face(fam, w, false);
        Some(Self {
            sans: f("Noto Sans", 400)?,
            semi: f("Noto Sans", 600)?,
            bold: f("Noto Sans", 700)?,
            black: f("Noto Sans", 900)?,
            mono: f("Noto Sans Mono", 400)?,
            mono_bold: f("Noto Sans Mono", 700)?,
        })
    }
}

/// Width of `s` in `face` at `px`, with optional letter spacing.
pub fn text_w(face: &Arc<Face>, px: f32, s: &str, spacing: f32) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    textfit::text_width(face, px, s) + spacing * s.chars().count() as f32
}

/// Draw `s` with its top at `y`; returns the width.
pub fn text(
    cv: &mut Canvas,
    face: &Arc<Face>,
    px: f32,
    x: f32,
    y: f32,
    s: &str,
    color: u32,
) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    let mut p = RasterPainter { cv };
    let base = y + face.ascent(px);
    textfit::draw_plain(
        &mut p,
        face,
        px,
        x.round(),
        base.round(),
        s,
        Rgba::hex(color),
        400,
    )
}

/// Letter-spaced text (labels like "SPEAKER NOTES").
#[allow(clippy::too_many_arguments)]
pub fn text_spaced(
    cv: &mut Canvas,
    face: &Arc<Face>,
    px: f32,
    x: f32,
    y: f32,
    s: &str,
    color: u32,
    spacing: f32,
) -> f32 {
    if spacing == 0.0 {
        return text(cv, face, px, x, y, s, color);
    }
    let mut pen = x.round();
    let mut buf = [0u8; 4];
    for c in s.chars() {
        let w = text(cv, face, px, pen, y, c.encode_utf8(&mut buf), color);
        pen += w + spacing;
    }
    pen - x
}

pub fn line_h(face: &Arc<Face>, px: f32) -> f32 {
    face.ascent(px) + face.descent(px)
}

/// Text vertically centred in `r`, aligned left (0), centre (1) or right (2).
#[allow(clippy::too_many_arguments)]
pub fn text_in(
    cv: &mut Canvas,
    face: &Arc<Face>,
    px: f32,
    r: Rect,
    s: &str,
    color: u32,
    align: u8,
    spacing: f32,
) {
    let s = elide(face, px, s, r.w as f32, spacing);
    let w = text_w(face, px, &s, spacing);
    let x = match align {
        1 => r.x as f32 + (r.w as f32 - w) / 2.0,
        2 => r.x as f32 + r.w as f32 - w,
        _ => r.x as f32,
    };
    let y = r.y as f32 + (r.h as f32 - line_h(face, px)) / 2.0;
    cv.push_clip_rect(r);
    text_spaced(cv, face, px, x, y, &s, color, spacing);
    cv.pop_clip_rect();
}

/// Cut `s` with an ellipsis to fit `max` pixels.
pub fn elide(face: &Arc<Face>, px: f32, s: &str, max: f32, spacing: f32) -> String {
    if text_w(face, px, s, spacing) <= max {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let t: String = chars[..mid].iter().collect::<String>() + "…";
        if text_w(face, px, &t, spacing) <= max {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>() + "…"
}

/// Word-wrapped plain text lines for `max` pixels.
pub fn wrap(face: &Arc<Face>, px: f32, s: &str, max: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let cand = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && text_w(face, px, &cand, 0.0) > max {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = cand;
            }
        }
        out.push(line);
    }
    out
}

// ------------------------------------------------------------- widgets

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BtnKind {
    Normal,
    Primary,
    Aux,
    Danger,
    Flat,
}

pub struct Paint<'a> {
    pub cv: &'a mut Canvas,
    pub c: &'a Chrome,
    pub f: &'a Fonts,
    pub px: f32,
}

impl Paint<'_> {
    pub fn label(&mut self, r: Rect, s: &str, color: u32) {
        let (f, px) = (self.f.sans.clone(), self.px);
        text_in(self.cv, &f, px, r, s, color, 0, 0.0);
    }

    /// The small monospace caps label ("THEME", "FOOTER").
    pub fn cap(&mut self, r: Rect, s: &str) {
        let f = self.f.mono_bold.clone();
        let col = self.c.label;
        text_in(self.cv, &f, 10.0, r, s, col, 0, 0.7);
    }

    pub fn cap_w(&self, s: &str) -> i32 {
        text_w(&self.f.mono_bold, 10.0, s, 0.7).ceil() as i32 + 2
    }

    pub fn button_w(&self, s: &str) -> i32 {
        text_w(&self.f.semi, self.px, s, 0.0).ceil() as i32 + 24
    }

    #[allow(clippy::too_many_arguments)]
    pub fn button(
        &mut self,
        r: Rect,
        s: &str,
        kind: BtnKind,
        hover: bool,
        focus: bool,
        enabled: bool,
        checked: bool,
    ) {
        let c = self.c;
        let (mut bg, mut border, mut fg) = match kind {
            BtnKind::Primary => (c.fg, c.fg, c.on_accent),
            BtnKind::Aux => (blend(c.button, c.fg, 0.02), c.border_strong, c.fg),
            BtnKind::Danger => (c.button, 0x5a2a2a, c.fg),
            BtnKind::Flat => (c.toolbar, c.border, c.fg),
            BtnKind::Normal => (c.button, c.border_strong, c.fg),
        };
        if checked {
            bg = blend(c.button, c.fg, 0.16);
            border = c.focus;
        }
        if hover && enabled {
            match kind {
                BtnKind::Primary => bg = blend(c.fg, 0xffffff, 0.5),
                BtnKind::Danger => {
                    bg = 0x2a1212;
                    border = 0xaa3333;
                }
                _ => {
                    bg = c.button_hover;
                    border = c.muted;
                }
            }
        }
        if focus {
            border = c.focus;
        }
        if !enabled {
            fg = c.disabled;
            bg = c.toolbar;
            border = c.border;
        }
        self.cv.fill(r, bg);
        self.cv.frame(r, border);
        let face = if kind == BtnKind::Primary || kind == BtnKind::Aux {
            self.f.black.clone()
        } else {
            self.f.semi.clone()
        };
        let px = self.px;
        text_in(self.cv, &face, px, r.inset(4), s, fg, 1, 0.0);
    }

    pub fn check(&mut self, r: Rect, s: &str, on: bool, hover: bool, enabled: bool) {
        let c = self.c;
        let b = Rect::new(r.x, r.y + (r.h - 14) / 2, 14, 14);
        let (bg, border) = if !enabled {
            (c.toolbar, c.border_strong)
        } else if on {
            (if hover { 0xffffff } else { c.fg }, c.fg)
        } else if hover {
            (c.hover, c.focus)
        } else {
            (c.input, c.label)
        };
        self.cv.round_rect(b, 2, border);
        self.cv.round_rect(b.inset(1), 1, bg);
        if on {
            // The check mark.
            let m = c.on_accent;
            let pts = [
                (3, 7),
                (4, 8),
                (5, 9),
                (6, 10),
                (7, 9),
                (8, 8),
                (9, 7),
                (10, 6),
                (11, 5),
            ];
            for (dx, dy) in pts {
                self.cv.fill(Rect::new(b.x + dx - 1, b.y + dy - 2, 2, 2), m);
            }
        }
        let fg = if enabled {
            blend(c.fg, c.bg, 0.08)
        } else {
            c.disabled
        };
        let px = self.px;
        let f = self.f.sans.clone();
        text_in(
            self.cv,
            &f,
            px,
            Rect::new(r.x + 23, r.y, r.w - 23, r.h),
            s,
            fg,
            0,
            0.0,
        );
    }

    pub fn check_w(&self, s: &str) -> i32 {
        23 + text_w(&self.f.sans, self.px, s, 0.0).ceil() as i32 + 4
    }

    pub fn chevron(&mut self, cx: i32, cy: i32, color: u32) {
        for i in 0..5 {
            self.cv.fill(Rect::new(cx - 5 + i, cy - 2 + i, 2, 2), color);
            self.cv.fill(Rect::new(cx + 4 - i, cy - 2 + i, 2, 2), color);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn combo(&mut self, r: Rect, s: &str, hover: bool, focus: bool, enabled: bool, open: bool) {
        let c = self.c;
        let border = if focus || open {
            c.focus
        } else if hover && enabled {
            blend(c.border_strong, c.fg, 0.15)
        } else if enabled {
            c.border
        } else {
            c.toolbar
        };
        let bg = if enabled { c.input } else { c.toolbar };
        self.cv.fill(r, bg);
        self.cv.frame(r, border);
        let dd = Rect::new(r.x + r.w - 30, r.y + 1, 29, r.h - 2);
        self.cv.fill(
            dd,
            if hover && enabled {
                c.hover
            } else {
                blend(c.input, c.fg, 0.04)
            },
        );
        self.cv
            .fill(Rect::new(dd.x, dd.y, 1, dd.h), c.border_strong);
        self.chevron(
            dd.x + dd.w / 2,
            dd.y + dd.h / 2 - 1,
            if enabled { c.fg } else { c.disabled },
        );
        let fg = if enabled { c.fg } else { c.disabled };
        let f = self.f.sans.clone();
        let px = self.px;
        text_in(
            self.cv,
            &f,
            px,
            Rect::new(r.x + 8, r.y, r.w - 44, r.h),
            s,
            fg,
            0,
            0.0,
        );
    }

    pub fn slider(&mut self, r: Rect, min: i32, max: i32, v: i32, hover: bool, focus: bool) {
        let c = self.c;
        let cy = r.y + r.h / 2;
        let g = Rect::new(r.x + 7, cy - 2, (r.w - 14).max(1), 4);
        self.cv
            .round_rect(g, 2, if hover { 0x414141 } else { 0x333333 });
        let t = if max > min {
            (v - min) as f32 / (max - min) as f32
        } else {
            0.0
        };
        let hx = g.x + (t * g.w as f32) as i32;
        let col = if hover || focus { 0xffffff } else { 0xdddddd };
        self.cv.ellipse_aa(
            crate::paint::RectF::new(hx as f32 - 7.0, cy as f32 - 7.0, 14.0, 14.0),
            Rgba::hex(if focus { 0xffffff } else { 0xeeeeee }),
        );
        self.cv.ellipse_aa(
            crate::paint::RectF::new(hx as f32 - 6.0, cy as f32 - 6.0, 12.0, 12.0),
            Rgba::hex(col),
        );
        let _ = c;
    }

    pub fn slider_value(r: Rect, min: i32, max: i32, x: i32) -> i32 {
        let g0 = r.x + 7;
        let gw = (r.w - 14).max(1);
        let t = ((x - g0) as f32 / gw as f32).clamp(0.0, 1.0);
        min + (t * (max - min) as f32).round() as i32
    }
}

// ----------------------------------------------------------- text edit

#[derive(Clone, Debug, Default)]
pub struct Edit {
    pub text: Vec<char>,
    pub cur: usize,
    pub anchor: usize,
    pub scroll: f32,
    pub multiline: bool,
    pub readonly: bool,
    undo: Vec<(Vec<char>, usize)>,
    redo: Vec<(Vec<char>, usize)>,
    /// Desired x when moving up and down.
    pub goal_x: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditResult {
    None,
    Moved,
    Changed,
    Submit,
    Copy,
    Cut,
    Paste,
    Unhandled,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Edit {
    pub fn new(s: &str, multiline: bool) -> Self {
        let text: Vec<char> = s.chars().collect();
        let n = text.len();
        Self {
            text,
            cur: n,
            anchor: n,
            multiline,
            ..Default::default()
        }
    }

    pub fn string(&self) -> String {
        self.text.iter().collect()
    }

    /// Replace the text from outside (no undo step); keeps the cursor in range.
    pub fn set(&mut self, s: &str) {
        let t: Vec<char> = s.chars().collect();
        if t == self.text {
            return;
        }
        self.text = t;
        self.cur = self.cur.min(self.text.len());
        self.anchor = self.cur;
        self.undo.clear();
        self.redo.clear();
    }

    pub fn reset(&mut self, s: &str) {
        self.text = s.chars().collect();
        self.cur = self.text.len();
        self.anchor = self.cur;
        self.scroll = 0.0;
        self.undo.clear();
        self.redo.clear();
    }

    pub fn has_sel(&self) -> bool {
        self.cur != self.anchor
    }

    pub fn sel(&self) -> (usize, usize) {
        (self.cur.min(self.anchor), self.cur.max(self.anchor))
    }

    pub fn selected(&self) -> String {
        let (a, b) = self.sel();
        self.text[a..b].iter().collect()
    }

    fn snapshot(&mut self) {
        self.undo.push((self.text.clone(), self.cur));
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn insert(&mut self, s: &str) {
        if self.readonly {
            return;
        }
        self.snapshot();
        let (a, b) = self.sel();
        let mut ins: Vec<char> = s.chars().collect();
        if !self.multiline {
            ins.retain(|c| *c != '\n' && *c != '\r');
        } else {
            ins.retain(|c| *c != '\r');
        }
        self.text.splice(a..b, ins.iter().copied());
        self.cur = a + ins.len();
        self.anchor = self.cur;
        self.goal_x = None;
    }

    /// Replace the selection and keep the new text selected (formatting).
    pub fn replace_selection(&mut self, s: &str) {
        let (a, _) = self.sel();
        self.insert(s);
        self.anchor = a;
    }

    pub fn delete_sel(&mut self) -> bool {
        if !self.has_sel() || self.readonly {
            return false;
        }
        self.snapshot();
        let (a, b) = self.sel();
        self.text.drain(a..b);
        self.cur = a;
        self.anchor = a;
        true
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cur = self.text.len();
    }

    pub fn undo(&mut self) -> bool {
        if let Some((t, c)) = self.undo.pop() {
            self.redo.push((self.text.clone(), self.cur));
            self.text = t;
            self.cur = c.min(self.text.len());
            self.anchor = self.cur;
            return true;
        }
        false
    }

    pub fn redo_(&mut self) -> bool {
        if let Some((t, c)) = self.redo.pop() {
            self.undo.push((self.text.clone(), self.cur));
            self.text = t;
            self.cur = c.min(self.text.len());
            self.anchor = self.cur;
            return true;
        }
        false
    }

    pub fn word_at(&mut self, i: usize) {
        let n = self.text.len();
        let i = i.min(n);
        let mut a = i;
        let mut b = i;
        while a > 0 && is_word(self.text[a - 1]) {
            a -= 1;
        }
        while b < n && is_word(self.text[b]) {
            b += 1;
        }
        self.anchor = a;
        self.cur = b;
    }

    fn word_left(&self, mut i: usize) -> usize {
        while i > 0 && !is_word(self.text[i - 1]) {
            i -= 1;
        }
        while i > 0 && is_word(self.text[i - 1]) {
            i -= 1;
        }
        i
    }

    fn word_right(&self, mut i: usize) -> usize {
        let n = self.text.len();
        while i < n && !is_word(self.text[i]) {
            i += 1;
        }
        while i < n && is_word(self.text[i]) {
            i += 1;
        }
        i
    }

    fn line_start(&self, i: usize) -> usize {
        let mut j = i;
        while j > 0 && self.text[j - 1] != '\n' {
            j -= 1;
        }
        j
    }

    fn line_end(&self, i: usize) -> usize {
        let mut j = i;
        while j < self.text.len() && self.text[j] != '\n' {
            j += 1;
        }
        j
    }

    /// Keyboard editing. `vmove` moves the cursor up/down by visual lines
    /// for a multi-line edit (given by the caller's layout).
    pub fn key(
        &mut self,
        sym: u32,
        state: u16,
        ch: Option<char>,
        vmove: &dyn Fn(usize, i32, Option<f32>) -> (usize, f32),
    ) -> EditResult {
        use crate::keys::*;
        let shift = state & crate::x::SHIFT != 0;
        let ctrl = state & crate::x::CTRL != 0;
        let sym = normalize(sym);
        let n = self.text.len();
        let moved = |s: &mut Self, to: usize| {
            s.cur = to.min(n);
            if !shift {
                s.anchor = s.cur;
            }
            s.goal_x = None;
            EditResult::Moved
        };
        if ctrl {
            match sym {
                0x61 | 0x41 => {
                    self.select_all();
                    return EditResult::Moved;
                }
                0x63 | 0x43 => return EditResult::Copy,
                0x78 | 0x58 => return EditResult::Cut,
                0x76 | 0x56 => return EditResult::Paste,
                0x7a | 0x5a if !shift => {
                    return if self.undo() {
                        EditResult::Changed
                    } else {
                        EditResult::Unhandled
                    }
                }
                0x7a | 0x5a | 0x79 | 0x59 => {
                    return if self.redo_() {
                        EditResult::Changed
                    } else {
                        EditResult::Unhandled
                    }
                }
                K_LEFT => {
                    let to = self.word_left(self.cur);
                    return moved(self, to);
                }
                K_RIGHT => {
                    let to = self.word_right(self.cur);
                    return moved(self, to);
                }
                K_HOME => return moved(self, 0),
                K_END => return moved(self, n),
                K_BACKSPACE if !self.readonly => {
                    if !self.delete_sel() {
                        let a = self.word_left(self.cur);
                        self.snapshot();
                        self.text.drain(a..self.cur);
                        self.cur = a;
                        self.anchor = a;
                    }
                    return EditResult::Changed;
                }
                K_DELETE if !self.readonly => {
                    if !self.delete_sel() {
                        let b = self.word_right(self.cur);
                        self.snapshot();
                        self.text.drain(self.cur..b);
                        self.anchor = self.cur;
                    }
                    return EditResult::Changed;
                }
                _ => return EditResult::Unhandled,
            }
        }
        match sym {
            K_LEFT => {
                if self.has_sel() && !shift {
                    let (a, _) = self.sel();
                    return moved(self, a);
                }
                let to = self.cur.saturating_sub(1);
                moved(self, to)
            }
            K_RIGHT => {
                if self.has_sel() && !shift {
                    let (_, b) = self.sel();
                    return moved(self, b);
                }
                let to = (self.cur + 1).min(n);
                moved(self, to)
            }
            K_HOME => {
                let to = if self.multiline {
                    self.line_start(self.cur)
                } else {
                    0
                };
                moved(self, to)
            }
            K_END => {
                let to = if self.multiline {
                    self.line_end(self.cur)
                } else {
                    n
                };
                moved(self, to)
            }
            K_UP | K_DOWN | K_PRIOR | K_NEXT if self.multiline => {
                let d = match sym {
                    K_UP => -1,
                    K_DOWN => 1,
                    K_PRIOR => -10,
                    _ => 10,
                };
                let (to, gx) = vmove(self.cur, d, self.goal_x);
                self.cur = to.min(n);
                if !shift {
                    self.anchor = self.cur;
                }
                self.goal_x = Some(gx);
                EditResult::Moved
            }
            K_BACKSPACE => {
                if self.readonly {
                    return EditResult::None;
                }
                if !self.delete_sel() && self.cur > 0 {
                    self.snapshot();
                    self.cur -= 1;
                    self.text.remove(self.cur);
                    self.anchor = self.cur;
                }
                EditResult::Changed
            }
            K_DELETE => {
                if self.readonly {
                    return EditResult::None;
                }
                if !self.delete_sel() && self.cur < n {
                    self.snapshot();
                    self.text.remove(self.cur);
                }
                EditResult::Changed
            }
            K_RETURN => {
                if self.multiline && !self.readonly {
                    self.insert("\n");
                    EditResult::Changed
                } else {
                    EditResult::Submit
                }
            }
            _ => match ch {
                Some(c) if !c.is_control() && !self.readonly => {
                    let mut b = [0u8; 4];
                    self.insert(c.encode_utf8(&mut b));
                    EditResult::Changed
                }
                _ => EditResult::Unhandled,
            },
        }
    }
}

/// Visual lines of a multi-line edit: (start, end) char ranges (end
/// excludes the newline) wrapped at `width`, with per-char advances.
pub struct AreaLayout {
    pub lines: Vec<(usize, usize)>,
    pub adv: Vec<f32>,
    pub line_h: f32,
}

pub fn area_layout(face: &Arc<Face>, px: f32, text: &[char], width: f32) -> AreaLayout {
    let mut adv = vec![0f32; text.len()];
    let mut start = 0;
    let mut lines = Vec::new();
    let mut i = 0;
    while i <= text.len() {
        if i == text.len() || text[i] == '\n' {
            let para = &text[start..i];
            let gl = fonts::shape(para, face, px, 400, false);
            for g in &gl {
                if let Some(a) = adv.get_mut(start + g.cluster) {
                    *a += g.x_advance;
                }
            }
            // Wrap the paragraph on spaces.
            let mut ls = start;
            let mut w = 0.0;
            let mut last_space: Option<usize> = None;
            let mut j = start;
            while j < i {
                if text[j] == ' ' {
                    last_space = Some(j);
                }
                w += adv[j];
                if w > width && j > ls {
                    let brk = match last_space {
                        Some(sp) if sp >= ls => sp + 1,
                        _ => j,
                    };
                    lines.push((ls, brk));
                    ls = brk;
                    w = adv[ls..=j].iter().sum();
                    last_space = None;
                }
                j += 1;
            }
            lines.push((ls, i));
            start = i + 1;
        }
        i += 1;
    }
    AreaLayout {
        lines,
        adv,
        line_h: (face.line_spacing(px)).ceil(),
    }
}

impl AreaLayout {
    pub fn line_of(&self, i: usize) -> usize {
        let mut best = 0;
        for (k, (a, b)) in self.lines.iter().enumerate() {
            if i >= *a && i <= *b {
                best = k;
                if i < *b {
                    break;
                }
            }
        }
        best
    }

    pub fn x_of(&self, i: usize) -> f32 {
        let l = self.line_of(i);
        let (a, _) = self.lines[l];
        self.adv[a..i.max(a)].iter().sum()
    }

    pub fn index_at(&self, line: usize, x: f32) -> usize {
        let line = line.min(self.lines.len().saturating_sub(1));
        let (a, b) = self.lines[line];
        let mut w = 0.0;
        for i in a..b {
            let half = self.adv[i] / 2.0;
            if w + half > x {
                return i;
            }
            w += self.adv[i];
        }
        b
    }
}

// --------------------------------------------------------------- popup

/// An open list: a combo's choices or a menu.
#[derive(Clone, Debug)]
pub struct Popup<T: Clone> {
    pub items: Vec<(String, Option<T>)>,
    pub rect: Rect,
    pub hover: Option<usize>,
    pub scroll: usize,
    pub row_h: i32,
    pub filter: String,
    pub sections: Vec<usize>,
}

impl<T: Clone> Popup<T> {
    pub fn visible(&self) -> usize {
        ((self.rect.h - 8) / self.row_h).max(1) as usize
    }

    pub fn filtered(&self) -> Vec<usize> {
        let f = self.filter.to_lowercase();
        (0..self.items.len())
            .filter(|&i| f.is_empty() || self.items[i].0.to_lowercase().contains(&f))
            .collect()
    }

    pub fn row_at(&self, x: i32, y: i32) -> Option<usize> {
        if !self.rect.contains(x, y) {
            return None;
        }
        let k = ((y - self.rect.y - 4) / self.row_h).max(0) as usize + self.scroll;
        let f = self.filtered();
        f.get(k).copied().filter(|&i| self.items[i].1.is_some())
    }

    pub fn draw(&self, cv: &mut Canvas, c: &Chrome, f: &Fonts, px: f32) {
        let r = self.rect;
        // Shadow, frame, rows.
        cv.tint(Rect::new(r.x + 3, r.y + 3, r.w, r.h), 0, 90);
        cv.fill(r, blend(c.input, c.fg, 0.02));
        cv.frame(r, 0x454545.max(c.border_strong));
        let f_rows = self.filtered();
        cv.push_clip_rect(r.inset(1));
        for (k, &i) in f_rows
            .iter()
            .skip(self.scroll)
            .take(self.visible())
            .enumerate()
        {
            let row = Rect::new(
                r.x + 4,
                r.y + 4 + k as i32 * self.row_h,
                r.w - 8,
                self.row_h,
            );
            let (label, act) = &self.items[i];
            if act.is_none() {
                if label.is_empty() {
                    cv.fill(
                        Rect::new(row.x + 4, row.y + row.h / 2, row.w - 8, 1),
                        c.border_strong,
                    );
                } else {
                    text_in(
                        cv,
                        &f.mono_bold,
                        10.0,
                        Rect::new(row.x + 6, row.y, row.w - 12, row.h),
                        label,
                        c.label,
                        0,
                        0.7,
                    );
                }
                continue;
            }
            let hot = self.hover == Some(i);
            if hot {
                cv.fill(row, c.fg);
            }
            let col = if hot { c.on_accent } else { c.fg };
            text_in(
                cv,
                &f.sans,
                px,
                Rect::new(row.x + 10, row.y, row.w - 20, row.h),
                label,
                col,
                0,
                0.0,
            );
        }
        cv.pop_clip_rect();
        if f_rows.len() > self.visible() {
            let track = Rect::new(r.x + r.w - 5, r.y + 4, 3, r.h - 8);
            let total = f_rows.len() as f32;
            let th = ((self.visible() as f32 / total) * track.h as f32).max(16.0) as i32;
            let ty = track.y + ((self.scroll as f32 / total) * track.h as f32) as i32;
            cv.fill(Rect::new(track.x, ty, track.w, th), c.muted);
        }
        if !self.filter.is_empty() {
            let fr = Rect::new(r.x, r.y - 24, r.w.min(260), 22);
            cv.fill(fr, c.input);
            cv.frame(fr, c.focus);
            text_in(
                cv,
                &f.sans,
                px,
                fr.inset(4),
                &format!("Find: {}", self.filter),
                c.fg,
                0,
                0.0,
            );
        }
    }
}

/// Place a popup of `n` rows under (or above) `anchor` within `bounds`.
pub fn place_popup(anchor: Rect, n: usize, row_h: i32, width: i32, bounds: Rect) -> Rect {
    let want = n as i32 * row_h + 8;
    let below = bounds.y + bounds.h - (anchor.y + anchor.h) - 4;
    let above = anchor.y - bounds.y - 4;
    let w = width.max(anchor.w).min(bounds.w - 8);
    let x = anchor.x.min(bounds.x + bounds.w - w - 4).max(bounds.x + 4);
    if want <= below || below >= above {
        let h = want.min(below.max(row_h + 8));
        Rect::new(x, anchor.y + anchor.h + 1, w, h)
    } else {
        let h = want.min(above);
        Rect::new(x, anchor.y - h - 1, w, h)
    }
}

/// A tooltip box near (x, y).
pub fn tooltip(cv: &mut Canvas, c: &Chrome, f: &Fonts, px: f32, x: i32, y: i32, s: &str) {
    let lines = wrap(&f.sans, px, s, 420.0);
    let w = lines
        .iter()
        .map(|l| text_w(&f.sans, px, l, 0.0).ceil() as i32)
        .max()
        .unwrap_or(0)
        + 16;
    let lh = line_h(&f.sans, px).ceil() as i32 + 2;
    let h = lines.len() as i32 * lh + 10;
    let bx = x.min(cv.w - w - 4).max(2);
    let by = if y + 22 + h < cv.h {
        y + 22
    } else {
        (y - h - 6).max(2)
    };
    let r = Rect::new(bx, by, w, h);
    cv.reset_clip();
    cv.fill(r, blend(c.bg, c.fg, 0.10));
    cv.frame(r, c.border_strong);
    for (i, l) in lines.iter().enumerate() {
        text(
            cv,
            &f.sans,
            px,
            (bx + 8) as f32,
            (by + 5 + i as i32 * lh) as f32,
            l,
            c.fg,
        );
    }
}
