//! `render_slide`: the 28 layouts, 17 themes, logo, footer and numbers,
//! ported from the 2.4 renderer so decks look the same.
use crate::images::{self, Treatment};
use crate::model::*;
use crate::paint::{Img, Painter, RectF, Rgba};
use crate::textfit::{self, Align, HAlign, VAlign, W_BLACK, W_BOLD, W_LIGHT, W_MEDIUM, W_NORMAL};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

const PT_TO_PX: f32 = 96.0 / 72.0;
pub const MONO_FONT: &str = "Noto Sans Mono";

const L: HAlign = HAlign::Left;
const HC: HAlign = HAlign::Center;
const R: HAlign = HAlign::Right;
const T: VAlign = VAlign::Top;
const VC: VAlign = VAlign::Center;
const B: VAlign = VAlign::Bottom;

fn al(h: HAlign, v: VAlign) -> Align {
    Align::new(h, v)
}

/// Deck-level values a slide render needs.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub theme: String,
    pub index: usize,
    pub total: usize,
    pub show_numbers: bool,
    pub font: String,
    pub show_logo: bool,
    pub footer: String,
    pub footer_align: String,
    pub footer_size: String,
}

impl Ctx {
    pub fn new(deck: &DeckStyle, index: usize, show_numbers: bool) -> Self {
        Self {
            theme: deck.theme.clone(),
            index,
            total: deck.total,
            show_numbers,
            font: deck.font.clone(),
            show_logo: deck.show_logo,
            footer: deck.footer.clone(),
            footer_align: deck.footer_align.clone(),
            footer_size: deck.footer_size.clone(),
        }
    }
}

struct Colors {
    bg: Rgba,
    fg: Rgba,
    muted: Rgba,
    accent: Rgba,
    rule: Rgba,
}

struct R<'a> {
    p: &'a mut dyn Painter,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    t: Colors,
    sans: String,
    scale: f32,
    base_align: HAlign,
    hero_weight: u16,
    img: Treatment,
    mode_fill: bool,
}

impl R<'_> {
    fn px(&self, frac: f32, floor: f32) -> f32 {
        floor.max(self.h * frac * PT_TO_PX)
    }

    #[allow(clippy::too_many_arguments)]
    fn text_full(
        &mut self,
        r: RectF,
        txt: &str,
        maxp: f32,
        minp: f32,
        color: Rgba,
        weight: u16,
        align: Align,
        lines: Option<usize>,
        label: &str,
        family: Option<&str>,
        markup: bool,
        wrap: bool,
        size: Option<i32>,
        scaled: bool,
    ) -> i32 {
        let k = if scaled { self.scale } else { 1.0 };
        let fam = family.unwrap_or(&self.sans).to_string();
        let maxpx = self.px(maxp * k, 1.0);
        let minpx = self.px(minp * k, 1.0);
        textfit::fit_draw(
            self.p, r, txt, &fam, weight, maxpx, minpx, color, align, lines, label, markup, wrap,
            size,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        r: RectF,
        txt: &str,
        maxp: f32,
        minp: f32,
        color: Rgba,
        weight: u16,
        align: Align,
        lines: Option<usize>,
        label: &str,
    ) -> i32 {
        self.text_full(
            r, txt, maxp, minp, color, weight, align, lines, label, None, true, true, None, true,
        )
    }

    fn label_text(&mut self, r: RectF, txt: &str, frac: f32, color: Rgba) {
        let maxpx = self.px(frac, 6.0);
        let minpx = self.px(frac * 0.6, 5.0);
        textfit::fit_draw(
            self.p,
            r,
            txt,
            MONO_FONT,
            W_NORMAL,
            maxpx,
            minpx,
            color,
            al(L, VC),
            Some(1),
            "label",
            false,
            true,
            None,
        );
    }

    fn hero(
        &mut self,
        r: RectF,
        txt: &str,
        maxp: f32,
        minp: f32,
        align: Option<Align>,
        lines: usize,
    ) {
        let a = align.unwrap_or(al(self.base_align, VC));
        let (fg, hw) = (self.t.fg, self.hero_weight);
        self.text(r, txt, maxp, minp, fg, hw, a, Some(lines), "title");
    }

    #[allow(clippy::too_many_arguments)]
    fn body(
        &mut self,
        r: RectF,
        txt: &str,
        align: Option<Align>,
        maxp: f32,
        minp: f32,
        color: Option<Rgba>,
        lines: Option<usize>,
    ) {
        let mut a = align.unwrap_or(al(L, VC));
        if align.is_none() && self.base_align != L {
            a = al(self.base_align, VC);
        }
        let c = color.unwrap_or(self.t.muted);
        self.text(r, txt, maxp, minp, c, W_NORMAL, a, lines, "body");
    }

    fn more(&mut self, r: RectF, hidden: usize, align: Align) {
        if hidden > 0 {
            let m = self.t.muted;
            self.text_full(
                r,
                &textfit::more_label(hidden),
                0.020,
                0.012,
                m,
                W_MEDIUM,
                align,
                Some(1),
                "more",
                None,
                false,
                true,
                None,
                false,
            );
        }
    }

    fn paired(
        &self,
        texts: &[String],
        rects: &[RectF],
        maxp: f32,
        minp: f32,
        weight: u16,
        lines: Option<usize>,
    ) -> i32 {
        let maxpx = self.px(maxp * self.scale, 1.0);
        let minpx = self.px(minp * self.scale, 1.0);
        let mut sizes = Vec::new();
        for (tx, r) in texts.iter().zip(rects) {
            if tx.trim().is_empty() {
                continue;
            }
            sizes.push(textfit::fit_size(
                tx, &self.sans, weight, maxpx, minpx, r, lines, true, true,
            ));
        }
        if sizes.is_empty() {
            return maxpx as i32;
        }
        if sizes.iter().any(|s| s.is_none()) {
            return minpx as i32;
        }
        sizes.into_iter().flatten().min().unwrap_or(maxpx as i32)
    }

    fn fill(&mut self, r: RectF, c: Rgba) {
        self.p.fill_rect(r, c);
    }

    fn image(&mut self, r: RectF, path: &str, fill: Option<bool>, opacity: f32) -> bool {
        let mut t = self.img;
        t.fill = fill.unwrap_or(self.mode_fill);
        draw_image(self.p, r, path, &t, opacity)
    }
}

/// `draw_image`: false when there is no image to draw.
pub fn draw_image(
    p: &mut dyn Painter,
    rect: RectF,
    path: &str,
    t: &Treatment,
    opacity: f32,
) -> bool {
    if path.is_empty() || !Path::new(path).exists() {
        return false;
    }
    let (tx, ty, tw, th) = rect.to_rect();
    if tw <= 0 || th <= 0 {
        return false;
    }
    let density = p.image_density();
    if density == 0.0 {
        return true;
    }
    match images::prepare(Path::new(path), tw, th, t, density) {
        Some((dx, dy, dw, dh, img)) => {
            p.image(
                RectF::new((tx + dx) as f32, (ty + dy) as f32, dw as f32, dh as f32),
                &img,
                opacity,
            );
            true
        }
        None => false,
    }
}

fn logo_png(name: &str) -> &'static [u8] {
    match name {
        "light" => include_bytes!("../assets/branding/uddevalla-light.png"),
        "disc" => include_bytes!("../assets/branding/uddevalla-white-disc.png"),
        _ => include_bytes!("../assets/branding/uddevalla-dark.png"),
    }
}

/// The logo scaled (KeepAspectRatio, smooth) to `w` x `h`.
fn logo_image(name: &str, w: u32, h: u32) -> Option<Arc<Img>> {
    static CACHE: OnceLock<Mutex<Vec<(String, u32, u32, Arc<Img>)>>> = OnceLock::new();
    static SRC: OnceLock<Mutex<Vec<(String, Arc<Img>)>>> = OnceLock::new();
    let c = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Some(hit) = c
        .lock()
        .ok()?
        .iter()
        .find(|(n, a, b, _)| n == name && *a == w && *b == h)
    {
        return Some(hit.3.clone());
    }
    let src = {
        let s = SRC.get_or_init(|| Mutex::new(Vec::new()));
        let mut s = s.lock().ok()?;
        if let Some((_, i)) = s.iter().find(|(n, _)| n == name) {
            i.clone()
        } else {
            let i = Arc::new(images::decode(logo_png(name), None)?);
            s.push((name.to_string(), i.clone()));
            i
        }
    };
    let (sw, sh) = images::qt_scaled(src.w, src.h, w.max(1), h.max(1), false);
    let img = Arc::new(images::resize(&src, sw, sh));
    let mut c = c.lock().ok()?;
    if c.len() > 24 {
        c.remove(0);
    }
    c.push((name.to_string(), w, h, img.clone()));
    Some(img)
}

fn luminance_f(c: u32) -> f32 {
    let r = ((c >> 16) & 0xff) as f32 / 255.0;
    let g = ((c >> 8) & 0xff) as f32 / 255.0;
    let b = (c & 0xff) as f32 / 255.0;
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// `str.expandtabs(4)`.
fn expandtabs(s: &str) -> String {
    let mut out = String::new();
    let mut col = 0;
    for c in s.chars() {
        match c {
            '\t' => {
                let n = 4 - col % 4;
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            }
            '\n' | '\r' => {
                out.push(c);
                col = 0;
            }
            _ => {
                out.push(c);
                col += 1;
            }
        }
    }
    out
}

/// The SECTION title pattern `^\s*(\d{1,2})[. :–—-]+(.*)$`.
fn section_number(title: &str) -> Option<(String, String)> {
    let t = title.trim_start();
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < chars.len() && i < 2 && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let num: String = chars[..i].iter().collect();
    let mut j = i;
    while j < chars.len() && ". :–—-".contains(chars[j]) {
        j += 1;
    }
    if j == i {
        return None;
    }
    let rest: String = chars[j..].iter().collect();
    let body = rest.strip_suffix('\n').unwrap_or(&rest);
    if body.contains('\n') {
        return None;
    }
    Some((format!("{num:0>2}"), body.trim().to_string()))
}

pub fn render_slide(p: &mut dyn Painter, rect: RectF, s: &Obj, ctx: &Ctx) {
    let ov_theme = s_get(s, "themeOverride", "DECK");
    let theme_name = normalize_theme(if ov_theme != "DECK" {
        &ov_theme
    } else {
        &ctx.theme
    });
    let th = theme_or_bw(&theme_name);
    let accent =
        accent_override(&s_get(s, "accentOverride", "AUTO").to_uppercase()).unwrap_or(th.accent);
    let colors = Colors {
        bg: Rgba::hex(th.bg),
        fg: Rgba::hex(th.fg),
        muted: Rgba::hex(th.muted),
        accent: Rgba::hex(accent),
        rule: Rgba::hex(th.rule),
    };
    p.fill_rect(rect, colors.bg);
    let (x, y, w, h) = (rect.x, rect.y, rect.w, rect.h);
    let m = w * 0.055;
    let top = h * 0.055;
    let bgimg = s_or(s, "backgroundImage", "");
    if !bgimg.is_empty() {
        let t = Treatment {
            fill: true,
            mono: b_get(s, "backgroundMono"),
            fx: f_get(s, "backgroundFocalX", 0.5) as f32,
            fy: f_get(s, "backgroundFocalY", 0.5) as f32,
            brightness: 0,
            contrast: 0,
            overlay: 0,
            blur: i_get(s, "backgroundBlur", 0).clamp(0, 20) as i32,
            zoom: {
                let z = f_get(s, "_backgroundZoom", 1.0) as f32;
                if z == 0.0 {
                    1.0
                } else {
                    z
                }
            },
        };
        draw_image(p, rect, &bgimg, &t, 1.0);
        let dim = i_get(s, "backgroundDim", 75).clamp(0, 90);
        p.fill_rect(rect, colors.bg.with_alpha((255 * dim / 100) as u8));
    }
    let layout = s_get(s, "layout", "TITLE");
    let title = s_get(s, "title", "");
    let body = s_get(s, "body", "");
    let img = s_get(s, "image", "");
    let treatment = if s.contains_key("imageTreatment") {
        s_get(s, "imageTreatment", "NATURAL")
    } else if b_get(s, "mono") {
        "MONO".to_string()
    } else {
        "NATURAL".to_string()
    }
    .to_uppercase();
    let mono = treatment == "MONO" || th.image_mono;
    let mode_fill = s_get(s, "imageMode", "FILL") == "FILL";
    let mut br = i_get(s, "brightness", 0) as i32;
    let mut ct = i_get(s, "contrast", 0) as i32;
    let mut ov = i_get(s, "overlay", 0) as i32;
    let bl = i_get(s, "blur", 0) as i32;
    if treatment == "DIM" {
        br -= 10;
        ov = ov.max(18);
    }
    if treatment == "CONTRAST" {
        ct = ct.max(24);
    }
    let sans_raw = s_get(s, "fontFamily", "DECK");
    let sans = if sans_raw == "DECK" {
        ctx.font.clone()
    } else {
        sans_raw
    };
    let scale = {
        let v = if truthy(s.get("fontScale")) {
            to_f64(s.get("fontScale")).unwrap_or(1.0)
        } else {
            1.0
        };
        v.clamp(0.5, 2.0) as f32
    };
    let base_align = match s_get(s, "align", "LEFT").as_str() {
        "CENTER" => HC,
        "RIGHT" => R,
        _ => L,
    };
    let hero_weight = match s_get(s, "weight", "AUTO").as_str() {
        "LIGHT" => W_LIGHT,
        "REGULAR" => W_NORMAL,
        "BOLD" => W_BOLD,
        _ => W_BLACK,
    };
    let img_t = Treatment {
        fill: mode_fill,
        mono,
        fx: f_get(s, "focalX", 0.5) as f32,
        fy: f_get(s, "focalY", 0.5) as f32,
        brightness: br,
        contrast: ct,
        overlay: ov,
        blur: bl,
        zoom: 1.0,
    };
    let mut r = R {
        p,
        x,
        y,
        w,
        h,
        t: colors,
        sans,
        scale,
        base_align,
        hero_weight,
        img: img_t,
        mode_fill,
    };
    let fg = r.t.fg;
    let muted = r.t.muted;
    let acc = r.t.accent;
    let rule = r.t.rule;

    if ctx.show_numbers && layout != "FULL BLEED" {
        r.label_text(
            RectF::new(x + m, y + top, w * 0.3, h * 0.035),
            &format!("{:02} / {:02}", ctx.index + 1, ctx.total),
            0.013,
            muted,
        );
    }

    match layout.as_str() {
        "VIDEO" | "VIDEO + TEXT" => {
            r.hero(
                RectF::new(x + m, y + h * 0.08, w * 0.82, h * 0.12),
                &title,
                0.045,
                0.024,
                None,
                2,
            );
            let vr = if layout == "VIDEO" {
                RectF::new(x + m, y + h * 0.23, w - 2.0 * m, h * 0.58)
            } else {
                RectF::new(x + m, y + h * 0.25, w * 0.56, h * 0.52)
            };
            let poster = s_or(s, "videoPoster", "");
            let poster = if poster.is_empty() {
                img.clone()
            } else {
                poster
            };
            if !poster.is_empty() {
                let mut t = r.img;
                t.fill = true;
                draw_image(r.p, vr, &poster, &t, 1.0);
            } else {
                r.fill(vr, rule);
            }
            r.p.stroke_rect(vr, fg, 1.0);
            let (cx, cy) = vr.center();
            r.text_full(
                RectF::new(cx - w * 0.04, cy - h * 0.05, w * 0.08, h * 0.10),
                "▶",
                0.055,
                0.03,
                fg,
                W_BOLD,
                al(HC, VC),
                Some(1),
                "label",
                None,
                false,
                true,
                None,
                false,
            );
            if layout == "VIDEO + TEXT" {
                r.body(
                    RectF::new(x + w * 0.66, y + h * 0.25, w * 0.285, h * 0.52),
                    &body,
                    Some(al(L, T)),
                    0.024,
                    0.014,
                    Some(fg),
                    None,
                );
            } else if !body.is_empty() {
                r.body(
                    RectF::new(x + m, y + h * 0.825, w - 2.0 * m, h * 0.10),
                    &body,
                    Some(al(L, VC)),
                    0.020,
                    0.012,
                    None,
                    Some(3),
                );
            }
        }
        "FULL BLEED" => {
            r.image(rect, &img, Some(true), 1.0);
            r.fill(rect, Rgba(0, 0, 0, 105));
            let ba = r.base_align;
            r.hero(
                RectF::new(x + m, y + h * 0.45, w - 2.0 * m, h * 0.34),
                &title,
                0.085,
                0.036,
                Some(al(ba, B)),
                3,
            );
            if !body.is_empty() {
                r.text(
                    RectF::new(x + m, y + h * 0.81, w - 2.0 * m, h * 0.11),
                    &body,
                    0.025,
                    0.014,
                    Rgba::hex(0xE8E8E8),
                    W_NORMAL,
                    al(ba, T),
                    Some(3),
                    "body",
                );
            }
        }
        "AGENDA" | "BULLETS" => {
            let agenda = layout == "AGENDA";
            let (top_y, area_h, cap);
            if agenda {
                r.hero(
                    RectF::new(x + m, y + h * 0.10, w * 0.80, h * 0.15),
                    &title,
                    0.052,
                    0.028,
                    None,
                    2,
                );
                top_y = y + h * 0.30;
                area_h = h * 0.58;
                cap = 6;
            } else {
                r.hero(
                    RectF::new(x + m, y + h * 0.11, w * 0.82, h * 0.17),
                    &title,
                    0.058,
                    0.028,
                    None,
                    2,
                );
                r.fill(
                    RectF::new(x + m, y + h * 0.30, w * 0.055, (h * 0.0035).max(2.0)),
                    acc,
                );
                top_y = y + h * 0.345;
                area_h = h * 0.545;
                cap = 7;
            }
            let (items, hidden) = take_items(clean_items(&body, true), cap);
            if hidden > 0 {
                textfit::note("hidden", if agenda { "items" } else { "bullets" }, hidden);
            }
            let slots = items.len() + usize::from(hidden > 0);
            let step = (h * if agenda { 0.092 } else { 0.079 }).min(area_h / slots.max(1) as f32);
            let style = s_get(s, "listStyle", "NUMBERS").to_uppercase();
            let marker_w = w * if agenda { 0.07 } else { 0.045 };
            let has_marker = style != "NONE";
            let tx = x
                + m
                + if has_marker {
                    w * if agenda { 0.13 } else { 0.055 }
                } else {
                    0.0
                };
            let tw = (x + w - m) - tx - w * if agenda { 0.08 } else { 0.06 };
            let boxes: Vec<RectF> = (0..items.len())
                .map(|n| RectF::new(tx, top_y + n as f32 * step, tw, step * 0.86))
                .collect();
            let iw = if agenda { W_MEDIUM } else { W_NORMAL };
            let size = r.paired(&items, &boxes, 0.032, 0.017, iw, Some(3));
            for (n, item) in items.iter().enumerate() {
                let yy = top_y + n as f32 * step;
                let marker = match style.as_str() {
                    "DOTS" => "•".to_string(),
                    "DASHES" => "—".to_string(),
                    "NONE" => String::new(),
                    _ => format!("{:02}", n + 1),
                };
                if !marker.is_empty() {
                    r.label_text(
                        RectF::new(x + m, yy, marker_w, step * 0.86),
                        &marker,
                        if agenda { 0.025 } else { 0.016 },
                        acc,
                    );
                    if agenda {
                        r.fill(
                            RectF::new(
                                x + m + w * 0.075,
                                yy + step * 0.43,
                                w * 0.035,
                                (h * 0.002).max(1.0),
                            ),
                            rule,
                        );
                    }
                }
                r.text_full(
                    boxes[n],
                    item,
                    0.032,
                    0.017,
                    fg,
                    iw,
                    al(L, VC),
                    Some(3),
                    "item",
                    None,
                    true,
                    true,
                    Some(size),
                    true,
                );
            }
            if hidden > 0 {
                r.more(
                    RectF::new(tx, top_y + items.len() as f32 * step, tw, step * 0.86),
                    hidden,
                    al(L, VC),
                );
            }
        }
        "HERO IMAGE" => {
            let ir = RectF::new(x + w * 0.40, y, w * 0.60, h);
            if r.image(ir, &img, Some(true), 1.0) {
                let bg = r.t.bg;
                r.fill(RectF::new(x + w * 0.40, y, w * 0.10, h), bg);
            }
            let ba = r.base_align;
            r.hero(
                RectF::new(x + m, y + h * 0.16, w * 0.39, h * 0.38),
                &title,
                0.075,
                0.032,
                Some(al(ba, B)),
                4,
            );
            r.fill(
                RectF::new(x + m, y + h * 0.57, w * 0.08, (h * 0.005).max(2.0)),
                acc,
            );
            r.body(
                RectF::new(x + m, y + h * 0.61, w * 0.31, h * 0.27),
                &body,
                Some(al(L, T)),
                0.024,
                0.014,
                None,
                None,
            );
        }
        "COMPARE" | "TWO COLUMN" | "THREE COLUMN" => {
            let three = layout == "THREE COLUMN";
            r.hero(
                RectF::new(x + m, y + h * 0.10, w * 0.82, h * 0.15),
                &title,
                0.052,
                0.028,
                None,
                2,
            );
            let texts: Vec<String> = if three {
                parse_columns(&body, 3)
            } else if (b_get(s, "sideStructured") || b_get(s, "twoColumnStructured"))
                && s.get("leftColumn").is_some_and(|v| !v.is_null())
                && s.get("rightColumn").is_some_and(|v| !v.is_null())
            {
                vec![
                    s_or(s, "leftColumn", "").trim().to_string(),
                    s_or(s, "rightColumn", "").trim().to_string(),
                ]
            } else {
                let (l, rr, _) = parse_two_column_body(&body);
                vec![l, rr]
            };
            let n = texts.len();
            let gutter = w * if three {
                0.045
            } else if layout == "COMPARE" {
                0.060
            } else {
                0.070
            };
            let cw = (w - 2.0 * m - gutter * (n as f32 - 1.0)) / n as f32;
            let ctop = y + h * if layout != "TWO COLUMN" { 0.33 } else { 0.345 };
            let ch = h * 0.55;
            let rects: Vec<RectF> = (0..n)
                .map(|i| RectF::new(x + m + i as f32 * (cw + gutter), ctop, cw, ch))
                .collect();
            let size = r.paired(&texts, &rects, 0.027, 0.0145, W_NORMAL, None);
            for (tx_, rc) in texts.iter().zip(&rects) {
                r.text_full(
                    *rc,
                    tx_,
                    0.027,
                    0.0145,
                    fg,
                    W_NORMAL,
                    al(L, T),
                    None,
                    "column",
                    None,
                    true,
                    true,
                    Some(size),
                    true,
                );
            }
            if layout == "TWO COLUMN" {
                r.fill(
                    RectF::new(x + m, y + h * 0.285, w * 0.065, (h * 0.0035).max(2.0)),
                    acc,
                );
                r.fill(
                    RectF::new(x + w * 0.5 - w * 0.0006, ctop, (w * 0.0012).max(1.0), ch),
                    rule,
                );
            } else {
                for rc in &rects {
                    r.fill(
                        RectF::new(rc.x, ctop - h * 0.022, rc.w, (h * 0.004).max(2.0)),
                        acc,
                    );
                }
            }
        }
        "DATA / KPI" => {
            r.hero(
                RectF::new(x + m, y + h * 0.08, w * 0.80, h * 0.12),
                &title,
                0.043,
                0.024,
                None,
                2,
            );
            let (items, hidden) = take_items(nonempty_lines(&body), 4);
            if hidden > 0 {
                textfit::note("hidden", "metrics", hidden);
            }
            let (num, lab) = match items.first() {
                Some(i) => split_pipe(i),
                None => ("—".to_string(), "Primary metric".to_string()),
            };
            r.text_full(
                RectF::new(x + m, y + h * 0.24, w * 0.48, h * 0.25),
                &num,
                0.20,
                0.035,
                acc,
                W_BLACK,
                al(L, VC),
                Some(1),
                "number",
                None,
                false,
                false,
                None,
                true,
            );
            r.body(
                RectF::new(x + m, y + h * 0.50, w * 0.45, h * 0.18),
                &lab,
                Some(al(L, T)),
                0.025,
                0.014,
                Some(fg),
                Some(4),
            );
            let rest = if items.is_empty() {
                &[][..]
            } else {
                &items[1..]
            };
            for (n, item) in rest.iter().enumerate() {
                let (val, label) = split_pipe(item);
                let ry = y + h * (0.25 + n as f32 * 0.17);
                r.text_full(
                    RectF::new(x + w * 0.60, ry, w * 0.16, h * 0.12),
                    &val,
                    0.036,
                    0.016,
                    fg,
                    W_BOLD,
                    al(L, VC),
                    Some(1),
                    "number",
                    None,
                    false,
                    false,
                    None,
                    true,
                );
                r.body(
                    RectF::new(x + w * 0.775, ry, w * 0.17, h * 0.12),
                    &label,
                    Some(al(L, VC)),
                    0.017,
                    0.0115,
                    None,
                    Some(4),
                );
                if n + 1 < rest.len() || hidden > 0 {
                    r.fill(
                        RectF::new(x + w * 0.60, ry + h * 0.14, w * 0.345, (h * 0.002).max(1.0)),
                        rule,
                    );
                }
            }
            if hidden > 0 {
                r.more(
                    RectF::new(
                        x + w * 0.60,
                        y + h * (0.25 + rest.len() as f32 * 0.17),
                        w * 0.345,
                        h * 0.06,
                    ),
                    hidden,
                    al(L, VC),
                );
            }
        }
        "PROCESS" => {
            r.hero(
                RectF::new(x + m, y + h * 0.09, w * 0.80, h * 0.15),
                &title,
                0.050,
                0.026,
                None,
                2,
            );
            let (items, hidden) = take_items(nonempty_lines(&body), 5);
            if hidden > 0 {
                textfit::note("hidden", "steps", hidden);
            }
            let n = items.len().max(1) as f32;
            let gap = w * 0.018;
            let cell = (w - 2.0 * m - gap * (n - 1.0)) / n;
            let y0 = y + h * 0.33;
            let (heads, descs): (Vec<String>, Vec<String>) =
                items.iter().map(|i| split_pipe(i)).unzip();
            let hrects: Vec<RectF> = (0..items.len())
                .map(|j| {
                    RectF::new(
                        x + m + j as f32 * (cell + gap),
                        y0 + h * 0.11,
                        cell,
                        h * 0.12,
                    )
                })
                .collect();
            let drects: Vec<RectF> = hrects
                .iter()
                .map(|rc| RectF::new(rc.x, y0 + h * 0.24, cell, h * 0.30))
                .collect();
            let hs = r.paired(&heads, &hrects, 0.030, 0.016, W_BOLD, Some(3));
            let ds = r.paired(&descs, &drects, 0.019, 0.0115, W_NORMAL, None);
            for j in 0..items.len() {
                let rx = hrects[j].x;
                r.label_text(
                    RectF::new(rx, y0, cell, h * 0.06),
                    &format!("{:02}", j + 1),
                    0.018,
                    acc,
                );
                r.fill(
                    RectF::new(rx, y0 + h * 0.08, cell, (h * 0.004).max(2.0)),
                    acc,
                );
                r.text_full(
                    hrects[j],
                    &heads[j],
                    0.030,
                    0.016,
                    fg,
                    W_BOLD,
                    al(L, T),
                    Some(3),
                    "step",
                    None,
                    true,
                    true,
                    Some(hs),
                    true,
                );
                r.text_full(
                    drects[j],
                    &descs[j],
                    0.019,
                    0.0115,
                    muted,
                    W_NORMAL,
                    al(L, T),
                    None,
                    "step",
                    None,
                    true,
                    true,
                    Some(ds),
                    true,
                );
            }
            if hidden > 0 {
                r.more(
                    RectF::new(x + m, y + h * 0.86, w - 2.0 * m, h * 0.05),
                    hidden,
                    al(R, VC),
                );
            }
        }
        "MATRIX" => {
            r.hero(
                RectF::new(x + m, y + h * 0.07, w * 0.80, h * 0.14),
                &title,
                0.046,
                0.024,
                None,
                2,
            );
            let (items, hidden) = take_items(nonempty_lines(&body), 4);
            if hidden > 0 {
                textfit::note("hidden", "cells", hidden);
            }
            for (n, item) in items.iter().enumerate() {
                let (head, desc) = split_pipe(item);
                let (col, row) = ((n % 2) as f32, (n / 2) as f32);
                let rx = x + m + col * w * 0.445;
                let ry = y + h * (0.25 + row * 0.33);
                let rw = w * 0.40;
                r.fill(RectF::new(rx, ry, w * 0.035, (h * 0.004).max(2.0)), acc);
                r.text(
                    RectF::new(rx, ry + h * 0.02, rw, h * 0.085),
                    &head,
                    0.025,
                    0.015,
                    fg,
                    W_BOLD,
                    al(L, VC),
                    Some(2),
                    "cell",
                );
                r.body(
                    RectF::new(rx, ry + h * 0.11, rw, h * 0.19),
                    &desc,
                    Some(al(L, T)),
                    0.020,
                    0.012,
                    None,
                    None,
                );
            }
            if hidden > 0 {
                r.more(
                    RectF::new(x + m, y + h * 0.89, w - 2.0 * m, h * 0.04),
                    hidden,
                    al(R, VC),
                );
            }
        }
        "TABLE" => {
            r.hero(
                RectF::new(x + m, y + h * 0.07, w * 0.80, h * 0.14),
                &title,
                0.046,
                0.024,
                None,
                2,
            );
            render_table(&mut r, &body);
        }
        "IMAGE" => {
            let has_title = !title.trim().is_empty();
            if has_title {
                r.hero(
                    RectF::new(x + m, y + h * 0.065, w * 0.80, h * 0.10),
                    &title,
                    0.040,
                    0.022,
                    None,
                    2,
                );
            }
            let ir = RectF::new(
                x + m,
                y + h * if has_title { 0.18 } else { 0.10 },
                w - 2.0 * m,
                h * if has_title { 0.62 } else { 0.70 },
            );
            r.image(ir, &img, None, 1.0);
            let cap = s_or(s, "caption", "");
            let bw = if cap.is_empty() {
                w - 2.0 * m
            } else {
                w * 0.58
            };
            r.body(
                RectF::new(x + m, y + h * 0.82, bw, h * 0.095),
                &body,
                Some(al(L, VC)),
                0.024,
                0.013,
                None,
                Some(3),
            );
            if !cap.is_empty() {
                r.text(
                    RectF::new(x + m + w * 0.60, y + h * 0.82, w * 0.29, h * 0.095),
                    &cap,
                    0.016,
                    0.011,
                    muted,
                    W_NORMAL,
                    al(R, VC),
                    Some(3),
                    "caption",
                );
            }
        }
        "IMAGE + CAPTION" => {
            let ir = RectF::new(x + m, y + h * 0.07, w - 2.0 * m, h * 0.66);
            r.image(ir, &img, None, 1.0);
            r.fill(
                RectF::new(x + m, y + h * 0.765, (w * 0.003).max(2.0), h * 0.13),
                acc,
            );
            let cx = x + m + w * 0.018;
            r.hero(
                RectF::new(cx, y + h * 0.755, w * 0.80, h * 0.07),
                &title,
                0.034,
                0.018,
                Some(al(L, VC)),
                1,
            );
            let cap = [body.clone(), s_or(s, "caption", "")]
                .into_iter()
                .filter(|q| !q.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            r.body(
                RectF::new(cx, y + h * 0.83, w * 0.80, h * 0.075),
                &cap,
                Some(al(L, T)),
                0.019,
                0.0115,
                None,
                Some(3),
            );
        }
        "SPLIT" => {
            let left = RectF::new(x + m, y + h * 0.14, w * 0.36, h * 0.74);
            let right = RectF::new(x + w * 0.50, y + h * 0.12, w * 0.445, h * 0.76);
            let has_body = !body.is_empty();
            let tr = RectF::new(
                left.x,
                left.y,
                left.w,
                left.h * if has_body { 0.54 } else { 1.0 },
            );
            let ba = r.base_align;
            r.hero(
                tr,
                &title,
                0.064,
                0.030,
                if has_body { Some(al(ba, B)) } else { None },
                5,
            );
            if has_body {
                r.body(
                    RectF::new(
                        left.x,
                        tr.bottom() + h * 0.03,
                        left.w,
                        left.bottom() - tr.bottom() - h * 0.03,
                    ),
                    &body,
                    Some(al(L, T)),
                    0.025,
                    0.014,
                    None,
                    None,
                );
            }
            r.image(right, &img, None, 1.0);
            r.fill(
                RectF::new(x + w * 0.465, y + h * 0.27, (w * 0.0015).max(1.0), h * 0.46),
                rule,
            );
        }
        "PHOTO" => {
            let ir = RectF::new(x + m, y + h * 0.085, w - 2.0 * m, h * 0.715);
            r.image(ir, &img, None, 1.0);
            r.hero(
                RectF::new(x + m, y + h * 0.815, w * 0.50, h * 0.075),
                &title,
                0.032,
                0.018,
                Some(al(L, VC)),
                2,
            );
            r.body(
                RectF::new(x + m + w * 0.53, y + h * 0.815, w * 0.36, h * 0.075),
                &body,
                Some(al(R, VC)),
                0.019,
                0.0115,
                None,
                Some(3),
            );
            let cap = s_or(s, "caption", "");
            if !cap.is_empty() {
                r.text(
                    RectF::new(x + m, y + h * 0.893, w - 2.0 * m, h * 0.035),
                    &cap,
                    0.013,
                    0.010,
                    muted,
                    W_NORMAL,
                    al(R, VC),
                    Some(1),
                    "caption",
                );
            }
        }
        "IMAGE + QUOTE" => {
            let ir = RectF::new(x + m, y + h * 0.11, w * 0.49, h * 0.75);
            r.image(ir, &img, None, 1.0);
            r.fill(
                RectF::new(x + w * 0.555, y + h * 0.25, (w * 0.0014).max(1.0), h * 0.43),
                rule,
            );
            let ba = r.base_align;
            r.hero(
                RectF::new(x + w * 0.59, y + h * 0.13, w * 0.35, h * 0.44),
                &title,
                0.056,
                0.026,
                Some(al(ba, B)),
                6,
            );
            r.body(
                RectF::new(x + w * 0.59, y + h * 0.60, w * 0.35, h * 0.26),
                &body,
                Some(al(L, T)),
                0.022,
                0.013,
                None,
                None,
            );
        }
        "NUMBER GRID" => {
            r.hero(
                RectF::new(x + m, y + h * 0.08, w * 0.82, h * 0.14),
                &title,
                0.05,
                0.026,
                None,
                2,
            );
            let (items, hidden) = take_items(nonempty_lines(&body), 4);
            if hidden > 0 {
                textfit::note("hidden", "numbers", hidden);
            }
            for (n, item) in items.iter().enumerate() {
                let (num, label) = split_pipe(item);
                let (col, row) = ((n % 2) as f32, (n / 2) as f32);
                let rx = x + m + col * w * 0.445;
                let ry = y + h * (0.26 + row * 0.31);
                r.text_full(
                    RectF::new(rx, ry, w * 0.40, h * 0.14),
                    &num,
                    0.105,
                    0.03,
                    acc,
                    W_BLACK,
                    al(L, VC),
                    Some(1),
                    "number",
                    None,
                    false,
                    false,
                    None,
                    true,
                );
                r.body(
                    RectF::new(rx, ry + h * 0.145, w * 0.40, h * 0.13),
                    &label,
                    Some(al(L, T)),
                    0.022,
                    0.0125,
                    None,
                    Some(3),
                );
            }
            if hidden > 0 {
                r.more(
                    RectF::new(x + m, y + h * 0.88, w - 2.0 * m, h * 0.04),
                    hidden,
                    al(R, VC),
                );
            }
        }
        "TIMELINE" => {
            r.hero(
                RectF::new(x + m, y + h * 0.10, w * 0.82, h * 0.15),
                &title,
                0.052,
                0.026,
                None,
                2,
            );
            let (items, hidden) = take_items(nonempty_lines(&body), 5);
            if hidden > 0 {
                textfit::note("hidden", "milestones", hidden);
            }
            let n = items.len().max(1);
            let yline = y + h * 0.50;
            let cells = timeline_cells(RectF::new(x + m, y, w - 2.0 * m, h), n, yline);
            if !items.is_empty() {
                let first = cells[0].0;
                let last = cells[cells.len() - 1].0;
                r.fill(
                    RectF::new(first, yline, (last - first).max(1.0), (h * 0.004).max(2.0)),
                    rule,
                );
            }
            let (heads, descs): (Vec<String>, Vec<String>) =
                items.iter().map(|i| split_pipe(i)).unzip();
            let hrects: Vec<RectF> = cells
                .iter()
                .take(items.len())
                .map(|c| RectF::new(c.1.x, yline - h * 0.17, c.1.w, h * 0.14))
                .collect();
            let drects: Vec<RectF> = cells
                .iter()
                .take(items.len())
                .map(|c| RectF::new(c.2.x, yline + h * 0.045, c.2.w, h * 0.33))
                .collect();
            let hs = r.paired(&heads, &hrects, 0.022, 0.013, W_MEDIUM, Some(3));
            let ds = r.paired(&descs, &drects, 0.017, 0.0115, W_NORMAL, None);
            for j in 0..items.len() {
                let xx = cells[j].0;
                r.p.fill_ellipse(
                    RectF::new(xx - w * 0.008, yline - w * 0.008, w * 0.016, w * 0.016),
                    acc,
                );
                r.text_full(
                    hrects[j],
                    &heads[j],
                    0.022,
                    0.013,
                    fg,
                    W_MEDIUM,
                    al(HC, B),
                    Some(3),
                    "milestone",
                    None,
                    true,
                    true,
                    Some(hs),
                    true,
                );
                r.text_full(
                    drects[j],
                    &descs[j],
                    0.017,
                    0.0115,
                    muted,
                    W_NORMAL,
                    al(HC, T),
                    None,
                    "milestone",
                    None,
                    true,
                    true,
                    Some(ds),
                    true,
                );
            }
            if hidden > 0 {
                r.more(
                    RectF::new(x + m, y + h * 0.88, w - 2.0 * m, h * 0.04),
                    hidden,
                    al(R, VC),
                );
            }
        }
        "QUOTE" => {
            r.text_full(
                RectF::new(x + m, y + h * 0.10, w * 0.18, h * 0.20),
                "“",
                0.16,
                0.08,
                rule,
                W_BLACK,
                al(L, T),
                Some(1),
                "label",
                None,
                false,
                true,
                None,
                false,
            );
            r.fill(
                RectF::new(x + m, y + h * 0.26, (w * 0.003).max(2.0), h * 0.45),
                acc,
            );
            let q = RectF::new(x + m + w * 0.045, y + h * 0.20, w * 0.80, h * 0.48);
            r.hero(q, &title, 0.068, 0.028, None, 6);
            r.body(
                RectF::new(q.x, y + h * 0.71, q.w, h * 0.13),
                &body,
                Some(al(L, T)),
                0.022,
                0.013,
                None,
                Some(3),
            );
        }
        "BIG NUMBER" => {
            r.text_full(
                RectF::new(x + m, y + h * 0.16, w - 2.0 * m, h * 0.44),
                &title,
                0.23,
                0.04,
                acc,
                W_BLACK,
                al(L, VC),
                Some(1),
                "number",
                None,
                true,
                false,
                None,
                true,
            );
            r.body(
                RectF::new(x + m, y + h * 0.63, w * 0.78, h * 0.24),
                &body,
                Some(al(L, T)),
                0.027,
                0.014,
                None,
                None,
            );
        }
        "SECTION" => {
            let section_title = match section_number(&title) {
                Some((num, rest)) => {
                    r.text_full(
                        RectF::new(x + m, y + h * 0.10, w * 0.18, h * 0.16),
                        &num,
                        0.10,
                        0.05,
                        acc,
                        W_NORMAL,
                        al(L, VC),
                        Some(1),
                        "label",
                        None,
                        false,
                        true,
                        None,
                        true,
                    );
                    rest
                }
                None => title.clone(),
            };
            r.fill(
                RectF::new(x + m, y + h * 0.53, w * 0.10, (h * 0.006).max(2.0)),
                acc,
            );
            let ba = r.base_align;
            r.hero(
                RectF::new(x + m, y + h * 0.27, w * 0.82, h * 0.24),
                &section_title,
                0.07,
                0.032,
                Some(al(ba, B)),
                3,
            );
            r.body(
                RectF::new(x + m + w * 0.14, y + h * 0.57, w * 0.66, h * 0.27),
                &body,
                Some(al(L, T)),
                0.027,
                0.016,
                None,
                None,
            );
        }
        "END" => {
            r.hero(
                RectF::new(x + m, y + h * 0.20, w - 2.0 * m, h * 0.30),
                &title,
                0.085,
                0.036,
                Some(al(HC, B)),
                3,
            );
            r.body(
                RectF::new(x + m, y + h * 0.52, w - 2.0 * m, h * 0.20),
                &body,
                Some(al(HC, T)),
                0.027,
                0.016,
                None,
                None,
            );
            if !th.uddevalla {
                r.fill(
                    RectF::new(x + w * 0.45, y + h * 0.76, w * 0.10, (h * 0.006).max(2.0)),
                    acc,
                );
            }
        }
        "STATEMENT" => {
            r.hero(
                RectF::new(x + m, y + h * 0.17, w - 2.0 * m, h * 0.47),
                &title,
                0.095,
                0.036,
                None,
                5,
            );
            r.fill(
                RectF::new(x + m, y + h * 0.67, w * 0.10, (h * 0.006).max(2.0)),
                acc,
            );
            r.body(
                RectF::new(x + m, y + h * 0.70, w * 0.80, h * 0.20),
                &body,
                Some(al(L, T)),
                0.027,
                0.016,
                None,
                None,
            );
        }
        "TEXT" => {
            r.hero(
                RectF::new(x + m, y + h * 0.12, w * 0.80, h * 0.20),
                &title,
                0.06,
                0.028,
                None,
                3,
            );
            r.fill(
                RectF::new(x + m, y + h * 0.35, w * 0.07, (h * 0.004).max(2.0)),
                acc,
            );
            r.body(
                RectF::new(x + m, y + h * 0.39, w * 0.80, h * 0.51),
                &body,
                Some(al(L, T)),
                0.027,
                0.016,
                None,
                None,
            );
        }
        "CODE" => {
            let has_title = !title.trim().is_empty();
            if has_title {
                r.hero(
                    RectF::new(x + m, y + h * 0.07, w * 0.80, h * 0.11),
                    &title,
                    0.042,
                    0.022,
                    None,
                    2,
                );
            }
            let panel = RectF::new(
                x + m,
                y + h * if has_title { 0.20 } else { 0.09 },
                w - 2.0 * m,
                h * if has_title { 0.70 } else { 0.81 },
            );
            r.fill(panel, fg.with_alpha(18));
            r.fill(
                RectF::new(panel.x, panel.y, (w * 0.003).max(2.0), panel.h),
                acc,
            );
            let code = expandtabs(&body.replace("\r\n", "\n"));
            let code = code.trim_matches('\n');
            let inner = panel.adjusted(w * 0.022, h * 0.03, -w * 0.018, -h * 0.03);
            r.text_full(
                inner,
                code,
                0.030,
                0.0135,
                fg,
                W_NORMAL,
                al(L, T),
                None,
                "code",
                Some(MONO_FONT),
                false,
                false,
                None,
                true,
            );
        }
        _ => {
            // TITLE (and anything unknown).
            let has_img = !img.is_empty();
            let tw = w * if has_img { 0.53 } else { 0.82 };
            let ba = r.base_align;
            r.hero(
                RectF::new(x + m, y + h * 0.16, tw, h * 0.35),
                &title,
                0.09,
                0.034,
                Some(al(ba, B)),
                4,
            );
            r.fill(
                RectF::new(x + m, y + h * 0.54, w * 0.10, (h * 0.006).max(2.0)),
                acc,
            );
            r.body(
                RectF::new(
                    x + m,
                    y + h * 0.58,
                    w * if has_img { 0.53 } else { 0.75 },
                    h * 0.25,
                ),
                &body,
                Some(al(L, T)),
                0.027,
                0.016,
                None,
                None,
            );
            if has_img {
                let t = r.img;
                draw_image(
                    r.p,
                    RectF::new(x + w * 0.64, y + h * 0.18, w * 0.30, h * 0.62),
                    &img,
                    &t,
                    0.92,
                );
            }
        }
    }
    let req = s_or(s, "imageRequest", "");
    if !req.is_empty() && !truthy(s.get("image")) {
        let f = r.px(0.014, 6.0);
        textfit::fit_draw(
            r.p,
            RectF::new(x + w * 0.60, y + h * 0.90, w * 0.34, h * 0.035),
            &format!("IMAGE REQUEST  ·  {req}"),
            MONO_FONT,
            W_NORMAL,
            f,
            f,
            muted,
            al(R, VC),
            Some(1),
            "request",
            false,
            true,
            None,
        );
    }
    if theme_name == "SATIE" {
        let (x1, y1) = ((x + w * 0.80) as i32 as f32, (y + h * 0.09) as i32 as f32);
        let x2 = (x + w * 0.94) as i32 as f32;
        let y2 = (y + h * 0.18) as i32 as f32;
        r.p.line(x1, y1, x2, y1, rule, 1.0);
        r.p.line(x2, y1, x2, y2, rule, 1.0);
    }
    if theme_name == "CARL LARSSON" {
        let palette = th
            .palette
            .unwrap_or([0xA33E32, 0x557A68, 0x66859A, 0xC39A45]);
        let band_w = w * 0.022;
        let band_h = (h * 0.004).max(2.0);
        for (j, c) in palette.iter().enumerate() {
            r.fill(
                RectF::new(
                    x + m + j as f32 * band_w,
                    y + h * 0.925,
                    band_w * 0.72,
                    band_h,
                ),
                Rgba::hex(*c),
            );
        }
    }
    if th.uddevalla {
        r.fill(RectF::new(x, y, w * 0.010, h), acc);
        let logo_mode = s_get(s, "logoMode", "AUTO").to_uppercase();
        let auto_layout = matches!(layout.as_str(), "TITLE" | "SECTION" | "END");
        let want = ctx.show_logo && logo_mode != "OFF" && (logo_mode == "ON" || auto_layout);
        if want {
            let name = if layout == "FULL BLEED" {
                "disc"
            } else if luminance_f(th.bg) >= 0.55 {
                "light"
            } else {
                "dark"
            };
            let side = h * 0.135;
            let lr = match layout.as_str() {
                "END" => RectF::new(x + w * 0.5 - side * 0.5, y + h * 0.755, side, side),
                "SPLIT" | "IMAGE + QUOTE" => RectF::new(x + m, y + h * 0.80, side, side),
                "IMAGE" | "PHOTO" | "FULL BLEED" | "IMAGE + CAPTION" => {
                    RectF::new(x + w - side - w * 0.035, y + h * 0.035, side, side)
                }
                "TITLE" if !img.is_empty() => RectF::new(x + m, y + h * 0.805, side, side),
                _ => RectF::new(x + w - side - w * 0.038, y + h * 0.805, side, side),
            };
            if r.p.image_density() > 0.0 {
                let d = r.p.image_density();
                let (lw, lh) = (lr.w as i32, lr.h as i32);
                let (pw, ph) = (((lw as f32) * d) as u32, ((lh as f32) * d) as u32);
                if let Some(li) = logo_image(name, pw.max(1), ph.max(1)) {
                    let sw = li.w as f32 / d;
                    let sh = li.h as f32 / d;
                    let dx = (lr.x + (lr.w - sw.round()) / 2.0) as i32 as f32;
                    let dy = (lr.y + (lr.h - sh.round()) / 2.0) as i32 as f32;
                    r.p.image(RectF::new(dx, dy, sw, sh), &li, 1.0);
                }
            }
        }
    }
    if !ctx.footer.is_empty() && layout != "FULL BLEED" {
        let fa = normalize_footer_align(&ctx.footer_align);
        let fs = normalize_footer_size(&ctx.footer_size);
        let factor = match fs.as_str() {
            "SMALL" => 0.010,
            "LARGE" => 0.014,
            _ => 0.012,
        };
        let hal = match fa.as_str() {
            "CENTER" => HC,
            "RIGHT" => R,
            _ => L,
        };
        let f = r.px(factor, 7.0);
        let sans = r.sans.clone();
        textfit::fit_draw(
            r.p,
            RectF::new(x + m, y + h * 0.938, w - 2.0 * m, h * 0.035),
            &ctx.footer,
            &sans,
            W_NORMAL,
            f,
            f,
            muted,
            al(hal, VC),
            Some(1),
            "footer",
            false,
            true,
            None,
        );
    }
}

/// `timeline_cells`: (dot x, head rect, desc rect) for 1–5 milestones.
pub fn timeline_cells(safe: RectF, count: usize, yline: f32) -> Vec<(f32, RectF, RectF)> {
    let count = count.clamp(1, 5);
    let cell_w = safe.w / count as f32;
    let pad = (cell_w * 0.08).min(safe.w * 0.012);
    let head_h = safe.h * 0.075;
    let desc_h = safe.h * 0.125;
    (0..count)
        .map(|i| {
            let left = safe.x + i as f32 * cell_w;
            let iw = (cell_w - 2.0 * pad).max(1.0);
            let xx = left + cell_w * 0.5;
            let head = RectF::new(left + pad, yline - safe.h * 0.17, iw, head_h);
            let desc = RectF::new(left + pad, yline + safe.h * 0.055, iw, desc_h);
            (xx, head, desc)
        })
        .collect()
}

fn render_table(r: &mut R, body: &str) {
    let (x, y, w, h) = (r.x, r.y, r.w, r.h);
    let m = w * 0.055;
    let rows = table_rows(body);
    let cols = rows
        .iter()
        .map(|rw| rw.len())
        .max()
        .unwrap_or(1)
        .clamp(1, 6);
    let rows: Vec<Vec<String>> = rows
        .into_iter()
        .map(|mut rw| {
            rw.resize(cols, String::new());
            rw.truncate(cols);
            rw
        })
        .collect();
    let (tx0, tw) = (x + m, w - 2.0 * m);
    let (ttop, tbot) = (y + h * 0.24, y + h * 0.905);
    let mut weights = Vec::new();
    for c in 0..cols {
        let longest = rows
            .iter()
            .map(|rw| crate::markup::plain(&rw[c]).chars().count())
            .chain(std::iter::once(3))
            .max()
            .unwrap_or(3);
        weights.push((longest.clamp(6, 40) as f32).powf(0.7));
    }
    let tot: f32 = weights.iter().sum::<f32>().max(1.0);
    let widths: Vec<f32> = weights.iter().map(|wv| tw * wv / tot).collect();
    let (pad_x, pad_y) = (w * 0.008, h * 0.010);
    let sans = r.sans.clone();
    let row_heights = |size: f32, rr: &[Vec<String>]| -> Vec<f32> {
        rr.iter()
            .enumerate()
            .map(|(ri, row)| {
                let mut need = 0f32;
                for c in 0..cols {
                    let wt = if ri == 0 || c == 0 { W_BOLD } else { W_NORMAL };
                    let (n, step, tall) =
                        textfit::measure(&row[c], &sans, wt, size, widths[c] - 2.0 * pad_x);
                    let tall = if n <= 3 { tall } else { tall * 3.0 / n as f32 };
                    need = need.max(tall).max(step);
                }
                need + 2.0 * pad_y + 1.0
            })
            .collect()
    };
    let avail = tbot - ttop;
    let max_size = r.px(0.022 * r.scale, 1.0) as i32;
    let min_size = r.px(0.0125 * r.scale, 6.0) as i32;
    let mut shown_rows = rows.clone();
    let mut size = min_size;
    let mut heights: Option<Vec<f32>> = None;
    if !rows.is_empty() {
        let mut sz = max_size;
        while sz >= min_size {
            let hh = row_heights(sz as f32, &rows);
            let ok = hh.iter().sum::<f32>() <= avail
                && rows.iter().all(|rw| {
                    (0..cols).all(|c| {
                        textfit::measure(
                            &rw[c],
                            &sans,
                            W_NORMAL,
                            sz as f32,
                            widths[c] - 2.0 * pad_x,
                        )
                        .0 <= 3
                    })
                });
            if ok {
                size = sz;
                heights = Some(hh);
                break;
            }
            sz -= 1;
        }
        if heights.is_none() {
            let hh = row_heights(min_size as f32, &rows);
            let mut keep = rows.len();
            let more_h = h * 0.05;
            while keep > 1 && hh[..keep].iter().sum::<f32>() > avail - more_h {
                keep -= 1;
            }
            if keep < rows.len() {
                textfit::note("hidden", "rows", rows.len() - keep);
            }
            shown_rows.truncate(keep);
            heights = Some(hh[..keep].to_vec());
        }
    }
    let heights = heights.unwrap_or_default();
    let (fg, muted, acc, rule) = (r.t.fg, r.t.muted, r.t.accent, r.t.rule);
    let mut yy = ttop;
    for (ri, row) in shown_rows.iter().enumerate() {
        let rh = heights[ri].min(tbot - yy);
        if ri % 2 == 0 && ri > 0 {
            r.fill(RectF::new(tx0, yy, tw, rh), rule.with_alpha(70));
        }
        let mut xx = tx0;
        for c in 0..cols {
            let cell = RectF::new(
                xx + pad_x,
                yy + pad_y,
                widths[c] - 2.0 * pad_x,
                (rh - 2.0 * pad_y).max(1.0),
            );
            let head = ri == 0 || c == 0;
            r.text_full(
                cell,
                &row[c],
                0.022,
                0.0125,
                if head { fg } else { muted },
                if head { W_BOLD } else { W_NORMAL },
                al(L, VC),
                Some(3),
                "cell",
                None,
                true,
                true,
                Some(size),
                true,
            );
            xx += widths[c];
        }
        let lw = (h * 0.0015).max(1.0);
        r.fill(
            RectF::new(tx0, yy + rh - lw, tw, lw),
            if ri == 0 { acc } else { rule },
        );
        yy += rh;
    }
    if shown_rows.len() < rows.len() {
        r.more(
            RectF::new(tx0, yy + h * 0.005, tw, h * 0.04),
            rows.len() - shown_rows.len(),
            al(R, VC),
        );
    }
}

/// `slide_fit_issues`: render offscreen at 1920x1080 and name what does not fit.
pub fn slide_fit_issues(s: &Obj, deck: &Obj, index: usize) -> Vec<String> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<String, Vec<String>>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    let d = Deck(deck);
    let key = serde_json::to_string(&serde_json::json!([
        s,
        d.theme(),
        d.font(),
        d.footer(),
        d.footer_size(),
        d.show_numbers()
    ]))
    .unwrap_or_default();
    if let Some(hit) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let style = DeckStyle::of(deck);
    let ctx = Ctx::new(&style, index, style.show_numbers);
    let (_, report, _) = textfit::collect(false, || {
        let mut p = crate::paint::NullPainter;
        render_slide(&mut p, RectF::new(0.0, 0.0, 1920.0, 1080.0), s, &ctx);
    });
    let mut out = Vec::new();
    let mut clipped: Vec<String> = report
        .iter()
        .filter(|r| {
            r.kind == "clipped"
                && !matches!(r.label.as_str(), "footer" | "request" | "label" | "more")
        })
        .map(|r| r.label.clone())
        .collect();
    clipped.sort();
    clipped.dedup();
    if !clipped.is_empty() {
        out.push(format!("text does not fit ({})", clipped.join(", ")));
    }
    for r in report.iter().filter(|r| r.kind == "hidden") {
        if r.count != 1 {
            out.push(format!("{} {} hidden", r.count, r.label));
        } else {
            out.push(format!("1 {} hidden", r.label.trim_end_matches('s')));
        }
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 2000 {
            c.clear();
        }
        c.insert(key, out.clone());
    });
    out
}

/// The letterboxed slide rectangle inside `r` for a deck aspect ratio.
pub fn aspect_rect(r: RectF, aspect: &str) -> RectF {
    let ar = aspect_ratio(aspect) as f32;
    if r.w / r.h.max(1.0) > ar {
        let nw = r.h * ar;
        RectF::new(r.x + (r.w - nw) / 2.0, r.y, nw, r.h)
    } else {
        let nh = r.w / ar;
        RectF::new(r.x, r.y + (r.h - nh) / 2.0, r.w, nh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_numbers_and_tabs() {
        assert_eq!(
            section_number("3. Results"),
            Some(("03".into(), "Results".into()))
        );
        assert_eq!(
            section_number("12 — Plan"),
            Some(("12".into(), "Plan".into()))
        );
        assert_eq!(section_number("123 x"), None);
        assert_eq!(section_number("Plan"), None);
        assert_eq!(expandtabs("a\tb\n\tc"), "a   b\n    c");
    }

    #[test]
    fn timeline_geometry_is_bounded() {
        let c = timeline_cells(RectF::new(0.0, 0.0, 1000.0, 500.0), 9, 250.0);
        assert_eq!(c.len(), 5);
        assert!(c[4].1.right() <= 1000.0);
    }
}
