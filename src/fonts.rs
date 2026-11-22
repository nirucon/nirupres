//! Fonts: the fontconfig font list (cached on disk), face selection the way
//! Qt asks fontconfig (family, weight, italic), metrics computed like Qt 6
//! (hhea/OS/2, unrounded with subpixel positioning), HarfBuzz shaping via
//! rustybuzz and glyph coverage masks cached per size.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug)]
struct Entry {
    file: String,
    index: u32,
    families: Vec<String>,
    lower: Vec<String>,
    /// OpenType weight range (100..900); equal ends for a static face.
    wmin: f32,
    wmax: f32,
    italic: bool,
    /// Distance from the normal width (100); 0 for a normal face.
    width_off: f32,
}

pub struct Face {
    pub id: u32,
    pub ttf: ttf_parser::Face<'static>,
    pub rb: rustybuzz::Face<'static>,
    pub upem: f32,
    asc: f32,
    desc: f32,
    gap: f32,
    pub oblique: bool,
    pub family: String,
    /// The variation weight set on a variable font (for the PDF glyph cache key).
    pub wght: Option<f32>,
}

impl Face {
    /// QFixed::fromReal: truncated to 1/64 pixel.
    fn q64(v: f32) -> f32 {
        (v * 64.0).trunc() / 64.0
    }
    pub fn ascent(&self, px: f32) -> f32 {
        Self::q64(self.asc * px / self.upem)
    }
    pub fn descent(&self, px: f32) -> f32 {
        Self::q64(self.desc * px / self.upem)
    }
    pub fn leading(&self, px: f32) -> f32 {
        Self::q64(self.gap * px / self.upem)
    }
    /// `QFontMetricsF::lineSpacing()`.
    pub fn line_spacing(&self, px: f32) -> f32 {
        self.ascent(px) + self.descent(px) + self.leading(px)
    }
    pub fn has(&self, c: char) -> bool {
        self.ttf.glyph_index(c).is_some_and(|g| g.0 != 0)
    }
}

pub struct FontDb {
    entries: Vec<Entry>,
    loaded: HashMap<(usize, i32, bool), Arc<Face>>,
    resolved: HashMap<(String, u16, bool), Arc<Face>>,
    substitutes: HashMap<String, Option<String>>,
    fallback_cache: HashMap<(u16, bool), Vec<Arc<Face>>>,
    next_id: u32,
    refreshed: bool,
}

static DB: OnceLock<Mutex<FontDb>> = OnceLock::new();

pub fn db() -> std::sync::MutexGuard<'static, FontDb> {
    DB.get_or_init(|| Mutex::new(FontDb::load()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn fc_to_ot(w: f32) -> f32 {
    // FcWeightToOpenType, piecewise linear.
    const MAP: [(f32, f32); 11] = [
        (0.0, 100.0),
        (40.0, 200.0),
        (50.0, 300.0),
        (55.0, 350.0),
        (75.0, 380.0),
        (80.0, 400.0),
        (100.0, 500.0),
        (180.0, 600.0),
        (200.0, 700.0),
        (205.0, 800.0),
        (210.0, 900.0),
    ];
    if w >= 210.0 {
        return 900.0 + (w - 210.0).min(5.0) * 10.0;
    }
    for pair in MAP.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if w >= a.0 && w <= b.0 {
            return a.1 + (w - a.0) / (b.0 - a.0) * (b.1 - a.1);
        }
    }
    400.0
}

fn cache_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::prefs::home().join(".cache"));
    base.join("nirupres/fonts-v2")
}

fn stamp() -> String {
    // The font list changes when fontconfig rebuilds its caches or a font
    // directory changes; their modification times are the cache key.
    let home = crate::prefs::home();
    let mut s = String::new();
    for p in [
        std::path::PathBuf::from("/var/cache/fontconfig"),
        home.join(".cache/fontconfig"),
        std::path::PathBuf::from("/usr/share/fonts"),
        std::path::PathBuf::from("/usr/local/share/fonts"),
        home.join(".local/share/fonts"),
        home.join(".fonts"),
    ] {
        let t = std::fs::metadata(&p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        s.push_str(&format!("{t}:"));
    }
    s
}

fn parse_list(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 6 {
            continue;
        }
        let index: u32 = f[1].trim().parse().unwrap_or(0);
        if index >= 0x10000 {
            continue; // a named instance of a variable font
        }
        let file = f[0].to_string();
        let lower = file.to_ascii_lowercase();
        if !(lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".ttc")) {
            continue;
        }
        let families: Vec<String> = f[2]
            .split(',')
            .map(|s| s.replace("\\-", "-").trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let w = f[3].trim().trim_start_matches('[').trim_end_matches(']');
        let mut ws = w.split_whitespace().filter_map(|x| x.parse::<f32>().ok());
        let a = ws.next().unwrap_or(80.0);
        let b = ws.next().unwrap_or(a);
        let slant: f32 = f[4]
            .trim()
            .trim_start_matches('[')
            .split_whitespace()
            .next()
            .and_then(|x| x.trim_end_matches(']').parse().ok())
            .unwrap_or(0.0);
        let wd = f[5].trim().trim_start_matches('[').trim_end_matches(']');
        let mut wds = wd.split_whitespace().filter_map(|x| x.parse::<f32>().ok());
        let w0 = wds.next().unwrap_or(100.0);
        let w1 = wds.next().unwrap_or(w0);
        let width_off = if (w0..=w1).contains(&100.0) {
            0.0
        } else {
            (w0 - 100.0).abs().min((w1 - 100.0).abs())
        };
        let lower = families.iter().map(|f| f.to_lowercase()).collect();
        out.push(Entry {
            file,
            index,
            families,
            lower,
            wmin: fc_to_ot(a),
            wmax: fc_to_ot(b),
            italic: slant >= 100.0,
            width_off,
        });
    }
    out
}

fn run_fc_list() -> String {
    std::process::Command::new("fc-list")
        .args([
            "--format",
            "%{file}\t%{index}\t%{family}\t%{weight}\t%{slant}\t%{width}\n",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

impl FontDb {
    fn load() -> Self {
        crate::trace("fontdb start");
        let st = stamp();
        let path = cache_path();
        let cached = std::fs::read_to_string(&path).ok().and_then(|t| {
            let (head, body) = t.split_once('\n')?;
            (head == st).then(|| body.to_string())
        });
        let (text, refreshed) = match cached {
            Some(t) => (t, false),
            None => {
                let t = run_fc_list();
                if !t.is_empty() {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let tmp = path.with_extension("tmp");
                    if std::fs::write(&tmp, format!("{st}\n{t}")).is_ok() {
                        let _ = std::fs::rename(&tmp, &path);
                    }
                }
                (t, true)
            }
        };
        crate::trace("fontdb text");
        let mut entries = parse_list(&text);
        crate::trace("fontdb parsed");
        if entries.is_empty() {
            entries = fallback_entries();
        }
        Self {
            entries,
            loaded: HashMap::new(),
            resolved: HashMap::new(),
            substitutes: HashMap::new(),
            fallback_cache: HashMap::new(),
            next_id: 1,
            refreshed,
        }
    }

    /// Every installed family, sorted (the font menus).
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.families.first().cloned())
            .collect();
        v.sort_by_key(|a| a.to_lowercase());
        v.dedup();
        v
    }

    fn candidates(&self, family: &str) -> Vec<usize> {
        let fl = family.to_lowercase();
        (0..self.entries.len())
            .filter(|&i| self.entries[i].lower.iter().any(|f| *f == fl))
            .collect()
    }

    fn substitute(&mut self, family: &str) -> Option<String> {
        if let Some(s) = self.substitutes.get(family) {
            return s.clone();
        }
        let out = std::process::Command::new("fc-match")
            .args(["-f", "%{family[0]}", family])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty() && s.to_lowercase() != family.to_lowercase());
        self.substitutes.insert(family.to_string(), out.clone());
        out
    }

    /// The face Qt would use for (family, weight, italic).
    pub fn face(&mut self, family: &str, weight: u16, italic: bool) -> Option<Arc<Face>> {
        let key = (family.to_string(), weight, italic);
        if let Some(f) = self.resolved.get(&key) {
            return Some(f.clone());
        }
        let mut cands = self.candidates(family);
        if cands.is_empty() && !self.refreshed {
            // A font installed since the list was cached.
            self.refreshed = true;
            let t = run_fc_list();
            let fresh = parse_list(&t);
            if !fresh.is_empty() {
                self.entries = fresh;
                cands = self.candidates(family);
            }
        }
        if cands.is_empty() {
            if let Some(sub) = self.substitute(family) {
                cands = self.candidates(&sub);
            }
        }
        for fb in ["Noto Sans", "DejaVu Sans", "Liberation Sans"] {
            if cands.is_empty() {
                cands = self.candidates(fb);
            }
        }
        if cands.is_empty() {
            cands = (0..self.entries.len()).collect();
        }
        let w = weight as f32;
        let score = |e: &Entry| {
            let dw = if w < e.wmin {
                e.wmin - w
            } else if w > e.wmax {
                w - e.wmax
            } else {
                0.0
            };
            // Prefer the heavier face on a tie above 400, the lighter below.
            let tie = if w > 450.0 {
                -e.wmax * 0.001
            } else {
                e.wmin * 0.001
            };
            (if e.italic == italic { 0.0 } else { 10_000.0 }) + e.width_off * 100.0 + dw + tie
        };
        let mut best: Option<(usize, f32)> = None;
        for &i in &cands {
            let s = score(&self.entries[i]);
            if best.is_none_or(|(_, b)| s < b) {
                best = Some((i, s));
            }
        }
        let (i, _) = best?;
        let e = self.entries[i].clone();
        let oblique = italic && !e.italic;
        let wght = (e.wmax > e.wmin).then(|| w.clamp(e.wmin, e.wmax));
        let lkey = (i, wght.map(|x| x as i32).unwrap_or(-1), oblique);
        if let Some(f) = self.loaded.get(&lkey) {
            self.resolved.insert(key, f.clone());
            return Some(f.clone());
        }
        crate::trace("face load start");
        let data = map_file(&e.file)?;
        let mut ttf = ttf_parser::Face::parse(data, e.index).ok()?;
        let mut rb = rustybuzz::Face::from_slice(data, e.index)?;
        if let Some(wv) = wght {
            let tag = ttf_parser::Tag::from_bytes(b"wght");
            ttf.set_variation(tag, wv);
            rb.set_variations(&[rustybuzz::Variation { tag, value: wv }]);
        }
        let upem = ttf.units_per_em() as f32;
        let (asc, desc, gap) = qt_metrics(&ttf);
        let face = Arc::new(Face {
            id: self.next_id,
            ttf,
            rb,
            upem,
            asc,
            desc,
            gap,
            oblique,
            family: e.families.first().cloned().unwrap_or_default(),
            wght,
        });
        self.next_id += 1;
        crate::trace(&format!("face loaded {} {}", e.file, weight));
        self.loaded.insert(lkey, face.clone());
        self.resolved.insert(key, face.clone());
        Some(face)
    }

    /// Faces tried, in order, for a character the primary face lacks.
    pub fn fallbacks(&mut self, weight: u16, italic: bool) -> Vec<Arc<Face>> {
        if let Some(v) = self.fallback_cache.get(&(weight, italic)) {
            return v.clone();
        }
        let mut v = Vec::new();
        for fam in [
            "Noto Sans",
            "DejaVu Sans",
            "Noto Sans Symbols2",
            "Noto Sans Symbols",
            "Noto Sans Math",
            "DejaVu Sans Mono",
            "FreeSerif",
            "Noto Sans CJK SC",
        ] {
            if self.candidates(fam).is_empty() {
                continue;
            }
            if let Some(f) = self.face(fam, weight, italic) {
                if !v.iter().any(|x: &Arc<Face>| x.id == f.id) {
                    v.push(f);
                }
            }
        }
        self.fallback_cache.insert((weight, italic), v.clone());
        v
    }
}

fn fallback_entries() -> Vec<Entry> {
    // No fontconfig: look in the usual places for the fonts NIRUX installs.
    let mut v = Vec::new();
    for (file, fam, w, it) in [
        ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", "Noto Sans", 400.0, false),
        ("/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf", "Noto Sans", 700.0, false),
        ("/usr/share/fonts/noto/NotoSans-Regular.ttf", "Noto Sans", 400.0, false),
        ("/usr/share/fonts/noto/NotoSans-Bold.ttf", "Noto Sans", 700.0, false),
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "DejaVu Sans", 400.0, false),
        ("/usr/share/fonts/TTF/DejaVuSans.ttf", "DejaVu Sans", 400.0, false),
        ("/usr/share/fonts/dejavu/DejaVuSans.ttf", "DejaVu Sans", 400.0, false),
    ] {
        if std::path::Path::new(file).is_file() {
            v.push(Entry {
                file: file.into(),
                index: 0,
                families: vec![fam.into()],
                lower: vec![fam.to_lowercase()],
                wmin: w,
                wmax: w,
                italic: it,
                width_off: 0.0,
            });
        }
    }
    v
}

fn qt_metrics(f: &ttf_parser::Face) -> (f32, f32, f32) {
    // Qt 6: hhea, then OS/2 overrides (typo metrics when USE_TYPO_METRICS
    // is set, else win metrics with no leading).
    let mut asc = f.ascender() as f32;
    let mut desc = -(f.descender() as f32);
    let mut gap = f.line_gap() as f32;
    if let Some(os2) = f.tables().os2 {
        if os2.use_typographic_metrics() {
            asc = os2.typographic_ascender() as f32;
            desc = -(os2.typographic_descender() as f32);
            gap = os2.typographic_line_gap() as f32;
        } else {
            let wa = os2.windows_ascender() as f32;
            let wd = os2.windows_descender() as f32;
            if wa != 0.0 || wd != 0.0 {
                asc = wa;
                desc = wd.abs();
                gap = 0.0;
            }
        }
    }
    (asc, desc, gap)
}

fn map_file(path: &str) -> Option<&'static [u8]> {
    static MAPS: OnceLock<Mutex<HashMap<String, &'static [u8]>>> = OnceLock::new();
    let maps = MAPS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut m = maps.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = m.get(path) {
        return Some(d);
    }
    use std::os::fd::AsRawFd;
    let f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len() as usize;
    if len == 0 {
        return None;
    }
    // Mapped read-only: pages are shared with every other program using
    // the font and only the touched ones count.
    let p = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            f.as_raw_fd(),
            0,
        )
    };
    if p == libc::MAP_FAILED {
        let v = std::fs::read(path).ok()?;
        let s: &'static [u8] = Box::leak(v.into_boxed_slice());
        m.insert(path.to_string(), s);
        return Some(s);
    }
    let s: &'static [u8] = unsafe { std::slice::from_raw_parts(p as *const u8, len) };
    m.insert(path.to_string(), s);
    Some(s)
}

// ------------------------------------------------------------- shaping

#[derive(Clone)]
pub struct ShapedGlyph {
    pub face: Arc<Face>,
    pub gid: u16,
    pub x_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    /// Char index (into the shaped text) of the cluster start.
    pub cluster: usize,
}

type ShapeKey = (u32, u32, u16, bool, Vec<char>);

thread_local! {
    static SHAPES: std::cell::RefCell<HashMap<ShapeKey, Vec<ShapedGlyph>>> = std::cell::RefCell::new(HashMap::new());
}

/// Shape `text` (chars) with `primary`, falling back per character.
/// Short texts (labels, lines) are cached.
pub fn shape(text: &[char], primary: &Arc<Face>, px: f32, weight: u16, italic: bool) -> Vec<ShapedGlyph> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.len() > 240 {
        return shape_uncached(text, primary, px, weight, italic);
    }
    let key: ShapeKey = (primary.id, px.to_bits(), weight, italic, text.to_vec());
    if let Some(v) = SHAPES.with(|c| c.borrow().get(&key).cloned()) {
        return v;
    }
    let v = shape_uncached(text, primary, px, weight, italic);
    SHAPES.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 6000 {
            c.clear();
        }
        c.insert(key, v.clone());
    });
    v
}

fn shape_uncached(text: &[char], primary: &Arc<Face>, px: f32, weight: u16, italic: bool) -> Vec<ShapedGlyph> {
    // Split into runs of one face each.
    let mut runs: Vec<(usize, usize, Arc<Face>)> = Vec::new();
    let mut fallbacks: Option<Vec<Arc<Face>>> = None;
    for (i, &c) in text.iter().enumerate() {
        let face = if primary.has(c) || c.is_whitespace() || c.is_control() || is_joiner(c) {
            primary.clone()
        } else {
            let fbs = fallbacks.get_or_insert_with(|| db().fallbacks(weight, italic));
            fbs.iter()
                .find(|f| f.has(c))
                .cloned()
                .unwrap_or_else(|| primary.clone())
        };
        match runs.last_mut() {
            Some((_, end, f)) if f.id == face.id && *end == i => *end = i + 1,
            _ => runs.push((i, i + 1, face)),
        }
    }
    let mut out = Vec::with_capacity(text.len());
    for (s, e, face) in runs {
        let run: String = text[s..e].iter().collect();
        // Byte offsets of chars inside the run, to map clusters back.
        let mut byte_to_char = Vec::with_capacity(run.len() + 1);
        for (ci, ch) in text[s..e].iter().enumerate() {
            for _ in 0..ch.len_utf8() {
                byte_to_char.push(ci);
            }
        }
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(&run);
        buf.set_direction(rustybuzz::Direction::LeftToRight);
        let gb = rustybuzz::shape(&face.rb, &[], buf);
        let scale = px / face.upem;
        for (info, pos) in gb.glyph_infos().iter().zip(gb.glyph_positions()) {
            let ci = byte_to_char
                .get(info.cluster as usize)
                .copied()
                .unwrap_or(0);
            out.push(ShapedGlyph {
                face: face.clone(),
                gid: info.glyph_id as u16,
                // Qt keeps advances as 26.6 fixed point, truncated.
                x_advance: (pos.x_advance as f32 * scale * 64.0).trunc() / 64.0,
                x_offset: pos.x_offset as f32 * scale,
                y_offset: pos.y_offset as f32 * scale,
                cluster: s + ci,
            });
        }
    }
    out
}

fn is_joiner(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200f}' | '\u{fe0f}' | '\u{fe0e}')
}

// ------------------------------------------------------- rasterization

pub struct Mask {
    pub w: i32,
    pub h: i32,
    pub left: i32,
    pub top: i32,
    pub cov: Vec<u8>,
}

struct Outline<'a> {
    r: &'a mut ab_glyph_rasterizer::Rasterizer,
    scale: f32,
    shear: f32,
    dx: f32,
    dy: f32,
    last: (f32, f32),
    start: (f32, f32),
}

impl Outline<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (
            (x + y * self.shear) * self.scale + self.dx,
            -y * self.scale + self.dy,
        )
    }
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.last = p;
        self.start = p;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.r.draw_line(
            ab_glyph_rasterizer::point(self.last.0, self.last.1),
            ab_glyph_rasterizer::point(p.0, p.1),
        );
        self.last = p;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let c = self.p(x1, y1);
        let p = self.p(x, y);
        self.r.draw_quad(
            ab_glyph_rasterizer::point(self.last.0, self.last.1),
            ab_glyph_rasterizer::point(c.0, c.1),
            ab_glyph_rasterizer::point(p.0, p.1),
        );
        self.last = p;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.p(x1, y1);
        let c2 = self.p(x2, y2);
        let p = self.p(x, y);
        self.r.draw_cubic(
            ab_glyph_rasterizer::point(self.last.0, self.last.1),
            ab_glyph_rasterizer::point(c1.0, c1.1),
            ab_glyph_rasterizer::point(c2.0, c2.1),
            ab_glyph_rasterizer::point(p.0, p.1),
        );
        self.last = p;
    }
    fn close(&mut self) {
        if self.last != self.start {
            self.r.draw_line(
                ab_glyph_rasterizer::point(self.last.0, self.last.1),
                ab_glyph_rasterizer::point(self.start.0, self.start.1),
            );
        }
        self.last = self.start;
    }
}

pub const SHEAR: f32 = 0.2;

/// Rasterize one glyph at `px`, positioned with fractional x offset `fx`
/// (0..1) on the pen position.
pub fn rasterize(face: &Face, gid: u16, px: f32, fx: f32) -> Option<Mask> {
    let bb = face.ttf.glyph_bounding_box(ttf_parser::GlyphId(gid))?;
    let scale = px / face.upem;
    let shear = if face.oblique { SHEAR } else { 0.0 };
    let (x0, x1) = {
        let a = (bb.x_min as f32 + bb.y_min as f32 * shear).min(bb.x_min as f32 + bb.y_max as f32 * shear);
        let b = (bb.x_max as f32 + bb.y_min as f32 * shear).max(bb.x_max as f32 + bb.y_max as f32 * shear);
        (a * scale + fx, b * scale + fx)
    };
    let left = x0.floor() as i32;
    let right = x1.ceil() as i32;
    let top = (-(bb.y_max as f32) * scale).floor() as i32;
    let bottom = (-(bb.y_min as f32) * scale).ceil() as i32;
    let (w, h) = (right - left, bottom - top);
    if w <= 0 || h <= 0 || w > 4096 || h > 4096 {
        return None;
    }
    let mut r = ab_glyph_rasterizer::Rasterizer::new(w as usize, h as usize);
    {
        let mut o = Outline {
            r: &mut r,
            scale,
            shear,
            dx: fx - left as f32,
            dy: -(top as f32),
            last: (0.0, 0.0),
            start: (0.0, 0.0),
        };
        face.ttf.outline_glyph(ttf_parser::GlyphId(gid), &mut o)?;
    }
    let mut cov = vec![0u8; (w * h) as usize];
    r.for_each_pixel(|i, v| {
        if i < cov.len() {
            cov[i] = (v.min(1.0) * 255.0 + 0.5) as u8;
        }
    });
    Some(Mask {
        w,
        h,
        left,
        top,
        cov,
    })
}

type GlyphKey = (u32, u16, u32, u8);

struct GlyphCache {
    map: HashMap<GlyphKey, Option<Arc<Mask>>>,
    bytes: usize,
}

static GLYPHS: OnceLock<Mutex<GlyphCache>> = OnceLock::new();

/// A cached glyph mask; `fx` is quantized to quarter pixels.
pub fn glyph_mask(face: &Face, gid: u16, px: f32, fx: f32) -> Option<Arc<Mask>> {
    let q = ((fx * 4.0).floor() as i32).clamp(0, 3) as u8;
    let key = (face.id, gid, (px * 64.0).round() as u32, q);
    let cache = GLYPHS.get_or_init(|| {
        Mutex::new(GlyphCache {
            map: HashMap::new(),
            bytes: 0,
        })
    });
    {
        let c = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(m) = c.map.get(&key) {
            return m.clone();
        }
    }
    let m = rasterize(face, gid, px, q as f32 / 4.0).map(Arc::new);
    let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
    let size = m.as_ref().map(|m| m.cov.len() + 48).unwrap_or(16);
    if c.bytes + size > 12 * 1024 * 1024 {
        c.map.clear();
        c.bytes = 0;
    }
    c.bytes += size;
    c.map.insert(key, m.clone());
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fontconfig_weights_map_to_opentype() {
        assert_eq!(fc_to_ot(80.0), 400.0);
        assert_eq!(fc_to_ot(200.0), 700.0);
        assert_eq!(fc_to_ot(210.0), 900.0);
        assert_eq!(fc_to_ot(100.0), 500.0);
    }

    #[test]
    fn list_lines_parse() {
        let t = "/f/NotoSans-Black.ttf\t0\tNoto Sans,Noto Sans Black\t210\t0\t100\n\
                 /f/Var.ttf\t0\tVar\t[0 210]\t0\t[75 100]\n/f/Var.ttf\t65537\tVar\t80\t0\t100\n";
        let e = parse_list(t);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].wmin, 900.0);
        assert_eq!((e[1].wmin, e[1].wmax), (100.0, 900.0));
    }
}
