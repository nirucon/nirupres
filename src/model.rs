//! The deck model: project format 9, exactly as NIRUPRES 2.x reads and
//! writes it. A deck is kept as a JSON object (key order preserved) so that
//! fields this version does not know survive a load and save unchanged.
use serde_json::{json, Map, Value};

pub const APP: &str = "NIRUPRES";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FORMAT: i64 = 9;
pub const MD_FORMAT: i64 = 2;
pub const TAGLINE: &str = "A minimal, suckless presentation app by Nicklas Rudolfsson.";
pub const MB: u64 = 1024 * 1024;
pub const MAX_PROJECT_JSON: u64 = 5 * MB;
pub const MAX_ASSETS: usize = 250;
pub const MAX_PACKAGE_UNPACKED: u64 = 500 * MB;
pub const MAX_IMAGE_PIXELS: u64 = 50_000_000;
pub const MAX_HISTORY_BYTES: usize = 24 * MB as usize;

pub type Obj = Map<String, Value>;

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub bg: u32,
    pub fg: u32,
    pub muted: u32,
    pub accent: u32,
    pub rule: u32,
    pub image_mono: bool,
    pub uddevalla: bool,
    pub palette: Option<[u32; 4]>,
}

const fn th(
    name: &'static str,
    bg: u32,
    fg: u32,
    muted: u32,
    accent: u32,
    rule: u32,
    image_mono: bool,
    uddevalla: bool,
) -> Theme {
    Theme {
        name,
        bg,
        fg,
        muted,
        accent,
        rule,
        image_mono,
        uddevalla,
        palette: None,
    }
}

pub const THEMES: [Theme; 17] = [
    th(
        "B&W", 0x050505, 0xF4F2ED, 0x969696, 0xF4F2ED, 0x2A2A2A, true, false,
    ),
    th(
        "W&B", 0xF4F2ED, 0x090909, 0x66635F, 0x090909, 0xD4D0C8, true, false,
    ),
    th(
        "GRAY", 0x242424, 0xECECEC, 0xA8A8A8, 0xCFCFCF, 0x505050, true, false,
    ),
    th(
        "REDWINE", 0x210B11, 0xF1E8E3, 0xB89DA4, 0x8F263B, 0x54202C, false, false,
    ),
    th(
        "SATIE", 0xEDE7D8, 0x191815, 0x6F695F, 0x2E2923, 0xC7BEAC, true, false,
    ),
    Theme {
        palette: Some([0xA33E32, 0x557A68, 0x66859A, 0xC39A45]),
        ..th(
            "CARL LARSSON",
            0xEFE1BF,
            0x26372F,
            0x78664F,
            0xA33E32,
            0xB8A47B,
            false,
            false,
        )
    },
    th(
        "BEKSINSKI",
        0x0A0807,
        0xE9E0D2,
        0x8F8274,
        0x7B3028,
        0x30251F,
        false,
        false,
    ),
    th(
        "MARTIN", 0x0A0D0E, 0xEEE8D9, 0x92938D, 0xB78C4A, 0x293034, false, false,
    ),
    th(
        "SFUMATO", 0x171713, 0xE8E3D5, 0x9C988C, 0x8A8067, 0x37362F, false, false,
    ),
    th(
        "UDDEVALLA DARK",
        0x07141D,
        0xECEDED,
        0xA9BAC4,
        0x0065A6,
        0x24404F,
        false,
        true,
    ),
    th(
        "UDDEVALLA BLACK",
        0x000000,
        0xFFFFFF,
        0xB8C0C5,
        0x0065A6,
        0x252525,
        false,
        true,
    ),
    th(
        "UDDEVALLA BLUE",
        0xF8FAFB,
        0x173042,
        0x526774,
        0x0065A6,
        0xD8E2E8,
        false,
        true,
    ),
    th(
        "UDDEVALLA GREEN",
        0xF8FAFB,
        0x173042,
        0x526774,
        0x79AC54,
        0xDCE7D5,
        false,
        true,
    ),
    th(
        "UDDEVALLA YELLOW",
        0xF8FAFB,
        0x173042,
        0x526774,
        0xFFCC00,
        0xEEE5BF,
        false,
        true,
    ),
    th(
        "UDDEVALLA RED",
        0xF8FAFB,
        0x173042,
        0x526774,
        0xB51726,
        0xE8D5D8,
        false,
        true,
    ),
    th(
        "UDDEVALLA PINK",
        0xF8FAFB,
        0x173042,
        0x526774,
        0xE0437C,
        0xEED8E1,
        false,
        true,
    ),
    th(
        "UDDEVALLA PURPLE",
        0xF8FAFB,
        0x173042,
        0x526774,
        0x672A61,
        0xE0D5DF,
        false,
        true,
    ),
];

pub fn theme(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.name == name)
}

pub fn theme_or_bw(name: &str) -> &'static Theme {
    theme(name).unwrap_or(&THEMES[0])
}

pub const LAYOUTS: [&str; 28] = [
    "TITLE",
    "STATEMENT",
    "TEXT",
    "BULLETS",
    "AGENDA",
    "IMAGE",
    "PHOTO",
    "HERO IMAGE",
    "SPLIT",
    "TWO COLUMN",
    "COMPARE",
    "IMAGE + QUOTE",
    "QUOTE",
    "BIG NUMBER",
    "DATA / KPI",
    "NUMBER GRID",
    "PROCESS",
    "TIMELINE",
    "MATRIX",
    "TABLE",
    "SECTION",
    "VIDEO",
    "VIDEO + TEXT",
    "FULL BLEED",
    "END",
    "CODE",
    "IMAGE + CAPTION",
    "THREE COLUMN",
];

pub const UDDEVALLA_ACCENTS: [(&str, u32); 6] = [
    ("BLUE", 0x0065A6),
    ("GREEN", 0x79AC54),
    ("YELLOW", 0xFFCC00),
    ("RED", 0xB51726),
    ("PINK", 0xE0437C),
    ("PURPLE", 0x672A61),
];

pub fn accent_override(name: &str) -> Option<u32> {
    UDDEVALLA_ACCENTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}

pub const TRANSITIONS: [&str; 4] = ["NONE", "FADE", "MORPH", "REVEAL"];
pub const ASPECTS: [&str; 4] = ["16:9", "16:10", "4:3", "A4"];

pub fn aspect_ratio(a: &str) -> f64 {
    match a {
        "16:10" => 16.0 / 10.0,
        "4:3" => 4.0 / 3.0,
        "A4" => 297.0 / 210.0,
        _ => 16.0 / 9.0,
    }
}

// ---------------------------------------------------------------- values

/// Python truthiness of a JSON value.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// `str(value)` the way Python prints it.
pub fn py_str(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "None".into(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::Bool(false)) => "False".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => num_str(n),
        Some(other) => other.to_string(),
    }
}

pub fn num_str(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    let f = n.as_f64().unwrap_or(0.0);
    if f.is_finite() && f == f.trunc() && f.abs() < 1e16 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

pub fn s_get(o: &Obj, k: &str, d: &str) -> String {
    match o.get(k) {
        Some(Value::String(s)) => s.clone(),
        None => d.to_string(),
        Some(v) => py_str(Some(v)),
    }
}

/// `str(o.get(k) or d)`: an empty or missing value gives the default.
pub fn s_or(o: &Obj, k: &str, d: &str) -> String {
    if truthy(o.get(k)) {
        py_str(o.get(k))
    } else {
        d.to_string()
    }
}

pub fn f_get(o: &Obj, k: &str, d: f64) -> f64 {
    to_f64(o.get(k)).unwrap_or(d)
}

pub fn to_f64(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Python `int(x)`: floats truncate, numeric strings parse.
pub fn to_i64(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.trunc() as i64)),
        Value::Bool(b) => Some(*b as i64),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub fn i_get(o: &Obj, k: &str, d: i64) -> i64 {
    to_i64(o.get(k)).unwrap_or(d)
}

pub fn b_get(o: &Obj, k: &str) -> bool {
    truthy(o.get(k))
}

pub fn uuid4() -> String {
    let mut b = [0u8; 16];
    random_bytes(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub fn random_bytes(buf: &mut [u8]) {
    let mut filled = 0;
    while filled < buf.len() {
        let n = unsafe {
            libc::getrandom(
                buf[filled..].as_mut_ptr() as *mut libc::c_void,
                buf.len() - filled,
                0,
            )
        };
        if n <= 0 {
            // Very old kernels: fall back to the clock.
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            for (i, x) in buf[filled..].iter_mut().enumerate() {
                *x = (t >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(31);
            }
            return;
        }
        filled += n as usize;
    }
}

// ---------------------------------------------------------- normalizing

pub fn normalize_transition(v: Option<&Value>, allow_deck: bool) -> String {
    let fallback = if allow_deck { "DECK" } else { "NONE" };
    let raw = if truthy(v) {
        py_str(v)
    } else {
        fallback.to_string()
    };
    let mut value = raw.trim().to_uppercase();
    if value == "CUT" {
        value = "NONE".into();
    }
    if TRANSITIONS.contains(&value.as_str()) || (allow_deck && value == "DECK") {
        value
    } else {
        fallback.to_string()
    }
}

pub fn normalize_transition_str(v: &str, allow_deck: bool) -> String {
    normalize_transition(Some(&Value::String(v.to_string())), allow_deck)
}

pub fn normalize_footer_align(v: &str) -> String {
    let v = if v.is_empty() { "LEFT" } else { v };
    let v = v.trim().to_uppercase();
    if ["LEFT", "CENTER", "RIGHT"].contains(&v.as_str()) {
        v
    } else {
        "LEFT".into()
    }
}

pub fn normalize_footer_size(v: &str) -> String {
    let v = if v.is_empty() { "MEDIUM" } else { v };
    let v = v.trim().to_uppercase();
    if ["SMALL", "MEDIUM", "LARGE"].contains(&v.as_str()) {
        v
    } else {
        "MEDIUM".into()
    }
}

pub fn normalize_theme(name: &str) -> String {
    let n = if name.is_empty() { "B&W" } else { name }.to_uppercase();
    match n.as_str() {
        "UDDEVALLA" | "UDDEVALLA LIGHT" => "UDDEVALLA BLUE".into(),
        "CL" => "CARL LARSSON".into(),
        _ => n,
    }
}

pub fn normalize_themes(data: &mut Obj) {
    let t = normalize_theme(&s_or(data, "theme", "B&W"));
    let t = if theme(&t).is_some() { t } else { "B&W".into() };
    data.insert("theme".into(), Value::String(t));
    if let Some(Value::Array(slides)) = data.get_mut("slides") {
        for s in slides.iter_mut() {
            if let Value::Object(s) = s {
                let ov = normalize_theme(&s_or(s, "themeOverride", "DECK"));
                let ok = ov == "DECK" || theme(&ov).is_some();
                s.insert(
                    "themeOverride".into(),
                    Value::String(if ok { ov } else { "DECK".into() }),
                );
            }
        }
    }
}

fn slide_default_fields() -> Obj {
    let v = json!({
        "id": uuid4(),
        "layout": "TITLE",
        "title": "",
        "body": "",
        "image": "",
        "imageMode": "FILL",
        "mono": false,
        "notes": "",
        "focalX": 0.5,
        "focalY": 0.5,
        "imageTreatment": "NATURAL",
        "listStyle": "NUMBERS",
        "backgroundImage": "",
        "backgroundDim": 75,
        "backgroundBlur": 0,
        "backgroundMono": false,
        "backgroundFocalX": 0.5,
        "backgroundFocalY": 0.5,
        "themeOverride": "DECK",
        "fontScale": 1.0,
        "align": "LEFT",
        "weight": "AUTO",
        "brightness": 0,
        "contrast": 0,
        "overlay": 0,
        "blur": 0,
        "fontFamily": "DECK",
        "hidden": false,
        "caption": "",
        "imageRequest": "",
        "logoMode": "AUTO",
        "accentOverride": "AUTO",
        "transition": "DECK",
        "leftColumn": null,
        "rightColumn": null,
        "twoColumnStructured": false,
        "sideStructured": false,
        "backgroundMotion": "NONE",
        "videoSource": "",
        "videoPoster": "",
        "videoAutoplay": false,
        "videoMuted": false
    });
    match v {
        Value::Object(o) => o,
        _ => unreachable!(),
    }
}

fn upper_in(v: Option<&Value>, d: &str, allowed: &[&str]) -> String {
    let s = match v {
        None => d.to_string(),
        Some(v) => py_str(Some(v)),
    }
    .to_uppercase();
    if allowed.contains(&s.as_str()) {
        s
    } else {
        d.to_string()
    }
}

fn clamp_int(v: Option<&Value>, d: i64, lo: i64, hi: i64) -> i64 {
    // int(x or 0): a falsy value counts as 0.
    let x = if truthy(v) { to_i64(v).unwrap_or(d) } else { 0 };
    x.clamp(lo, hi)
}

fn clamp_focal(v: Option<&Value>) -> f64 {
    // float(x or 0.5): 0 counts as missing, as in 2.x.
    let x = if truthy(v) {
        to_f64(v).unwrap_or(0.5)
    } else {
        0.5
    };
    x.clamp(0.0, 1.0)
}

/// `slide_defaults`: fill in every slide field and normalize enumerations.
pub fn slide_defaults(s: Option<&Obj>) -> Obj {
    let mut d = slide_default_fields();
    if let Some(s) = s {
        for (k, v) in s {
            d.insert(k.clone(), v.clone());
        }
    }
    let treatment = if s.is_some_and(|s| !s.contains_key("imageTreatment")) {
        if truthy(d.get("mono")) {
            "MONO".to_string()
        } else {
            "NATURAL".to_string()
        }
    } else {
        py_str(d.get("imageTreatment")).to_uppercase()
    };
    let treatment = if ["NATURAL", "MONO", "DIM", "CONTRAST"].contains(&treatment.as_str()) {
        treatment
    } else {
        "NATURAL".into()
    };
    d.insert("mono".into(), Value::Bool(treatment == "MONO"));
    d.insert("imageTreatment".into(), Value::String(treatment));
    let ls = upper_in(
        d.get("listStyle"),
        "NUMBERS",
        &["NUMBERS", "DOTS", "DASHES", "NONE"],
    );
    d.insert("listStyle".into(), Value::String(ls));
    let dim = clamp_int(d.get("backgroundDim"), 75, 0, 90);
    d.insert("backgroundDim".into(), json!(dim));
    let blur = clamp_int(d.get("backgroundBlur"), 0, 0, 20);
    d.insert("backgroundBlur".into(), json!(blur));
    let bm = truthy(d.get("backgroundMono"));
    d.insert("backgroundMono".into(), Value::Bool(bm));
    let fx = clamp_focal(d.get("backgroundFocalX"));
    d.insert("backgroundFocalX".into(), json!(fx));
    let fy = clamp_focal(d.get("backgroundFocalY"));
    d.insert("backgroundFocalY".into(), json!(fy));
    let tr = normalize_transition(d.get("transition"), true);
    d.insert("transition".into(), Value::String(tr));
    let motion = upper_in(d.get("backgroundMotion"), "NONE", &["NONE", "KEN BURNS"]);
    d.insert("backgroundMotion".into(), Value::String(motion));
    let logo = upper_in(d.get("logoMode"), "AUTO", &["AUTO", "ON", "OFF"]);
    d.insert("logoMode".into(), Value::String(logo));
    let acc = upper_in(
        d.get("accentOverride"),
        "AUTO",
        &["AUTO", "BLUE", "GREEN", "YELLOW", "RED", "PINK", "PURPLE"],
    );
    d.insert("accentOverride".into(), Value::String(acc));
    d
}

pub fn default_deck() -> Obj {
    let first = json!({
        "layout": "TITLE",
        "title": "NIRUPRES",
        "body": "MINIMAL NOIR PRESENTATIONS",
        "image": "",
        "imageMode": "FILL",
        "mono": false,
        "notes": "",
        "focalX": 0.5,
        "focalY": 0.5,
        "imageTreatment": "NATURAL"
    });
    let v = json!({
        "format": FORMAT,
        "appVersion": VERSION,
        "title": "Untitled",
        "theme": "B&W",
        "showNumbers": false,
        "showLogo": true,
        "fontFamily": "Noto Sans",
        "aspect": "16:9",
        "transition": "NONE",
        "reduceMotion": false,
        "cinematicOpen": true,
        "cinematicClose": true,
        "targetMinutes": 0,
        "footerText": "",
        "footerAlign": "LEFT",
        "footerSize": "MEDIUM",
        "slides": [first]
    });
    match v {
        Value::Object(o) => o,
        _ => unreachable!(),
    }
}

/// The deck-level defaults applied after every load (and after recovery).
pub fn normalize_loaded(data: &mut Obj, title_default: &str) {
    let set_default = |data: &mut Obj, k: &str, v: Value| {
        if !data.contains_key(k) {
            data.insert(k.to_string(), v);
        }
    };
    set_default(data, "title", Value::String(title_default.to_string()));
    set_default(data, "theme", json!("B&W"));
    set_default(data, "showNumbers", json!(false));
    set_default(data, "showLogo", json!(true));
    set_default(data, "footerText", json!(""));
    set_default(data, "footerAlign", json!("LEFT"));
    set_default(data, "footerSize", json!("MEDIUM"));
    let fa = normalize_footer_align(&s_or(data, "footerAlign", ""));
    data.insert("footerAlign".into(), Value::String(fa));
    let fs = normalize_footer_size(&s_or(data, "footerSize", ""));
    data.insert("footerSize".into(), Value::String(fs));
    set_default(data, "fontFamily", json!("Noto Sans"));
    set_default(data, "aspect", json!("16:9"));
    set_default(data, "transition", json!("NONE"));
    set_default(data, "reduceMotion", json!(false));
    set_default(data, "cinematicOpen", json!(true));
    set_default(data, "cinematicClose", json!(true));
    let tr = normalize_transition(data.get("transition"), false);
    data.insert("transition".into(), Value::String(tr));
    let mut slides: Vec<Value> = match data.get("slides") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| Value::Object(slide_defaults(x.as_object())))
            .collect(),
        _ => Vec::new(),
    };
    if slides.is_empty() {
        let d = default_deck();
        let first = d["slides"][0].as_object().cloned();
        slides.push(Value::Object(slide_defaults(first.as_ref())));
    }
    data.insert("slides".into(), Value::Array(slides));
    normalize_themes(data);
}

pub fn validate_project_data(data: &Value) -> Result<(), String> {
    let Some(obj) = data.as_object() else {
        return Err("Invalid project: project.json must contain an object.".into());
    };
    let slides = match obj.get("slides") {
        None => return Ok(()),
        Some(Value::Array(a)) => a,
        Some(_) => return Err("Invalid project: slides must be a list.".into()),
    };
    if slides.len() > 2000 {
        return Err("Project contains too many slides (maximum 2000).".into());
    }
    for (i, s) in slides.iter().enumerate() {
        let Some(s) = s.as_object() else {
            return Err(format!("Invalid slide {}.", i + 1));
        };
        for key in [
            "title",
            "body",
            "notes",
            "caption",
            "imageRequest",
            "image",
            "backgroundImage",
            "videoSource",
            "videoPoster",
        ] {
            match s.get(key) {
                None => {}
                Some(Value::String(v)) => {
                    if v.chars().count() > 1_000_000 {
                        return Err(format!(
                            "Slide {} contains an excessively large text field.",
                            i + 1
                        ));
                    }
                }
                Some(_) => return Err(format!("Invalid {key} on slide {}.", i + 1)),
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------ deck view

/// Read-only accessors over a deck object.
pub struct Deck<'a>(pub &'a Obj);

impl Deck<'_> {
    pub fn slides(&self) -> &[Value] {
        self.0
            .get("slides")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    pub fn theme(&self) -> String {
        s_get(self.0, "theme", "B&W")
    }
    pub fn font(&self) -> String {
        s_get(self.0, "fontFamily", "Noto Sans")
    }
    pub fn aspect(&self) -> String {
        s_get(self.0, "aspect", "16:9")
    }
    pub fn show_numbers(&self) -> bool {
        b_get(self.0, "showNumbers")
    }
    pub fn show_logo(&self) -> bool {
        self.0
            .get("showLogo")
            .map(|v| truthy(Some(v)))
            .unwrap_or(true)
    }
    pub fn footer(&self) -> String {
        s_get(self.0, "footerText", "")
    }
    pub fn footer_align(&self) -> String {
        s_get(self.0, "footerAlign", "LEFT")
    }
    pub fn footer_size(&self) -> String {
        s_get(self.0, "footerSize", "MEDIUM")
    }
    pub fn reduce_motion(&self) -> bool {
        b_get(self.0, "reduceMotion")
    }
    pub fn flag_default_true(&self, k: &str) -> bool {
        self.0.get(k).map(|v| truthy(Some(v))).unwrap_or(true)
    }
}

/// What every slide render needs from the deck.
#[derive(Clone, Debug)]
pub struct DeckStyle {
    pub theme: String,
    pub font: String,
    pub show_numbers: bool,
    pub show_logo: bool,
    pub footer: String,
    pub footer_align: String,
    pub footer_size: String,
    pub total: usize,
}

impl DeckStyle {
    pub fn of(data: &Obj) -> Self {
        let d = Deck(data);
        Self {
            theme: d.theme(),
            font: d.font(),
            show_numbers: d.show_numbers(),
            show_logo: d.show_logo(),
            footer: d.footer(),
            footer_align: d.footer_align(),
            footer_size: d.footer_size(),
            total: d.slides().len().max(1),
        }
    }
}

// --------------------------------------------------------- body parsing

pub fn clean_items(body: &str, strip_markers: bool) -> Vec<String> {
    let mut out = Vec::new();
    for q in py_splitlines(body) {
        let q = q.trim();
        if q.is_empty() {
            continue;
        }
        let mut q = q.to_string();
        if strip_markers {
            for m in ["- ", "• ", "– ", "— ", "* "] {
                // `^(?:[-•–—]\s+|\*\s+)`: a marker followed by whitespace.
                let mk = m.trim_end();
                if let Some(rest) = q.strip_prefix(mk) {
                    if rest.starts_with(char::is_whitespace) {
                        q = rest.trim_start().to_string();
                        break;
                    }
                }
            }
        }
        out.push(q);
    }
    out
}

/// Lines of a non-empty body, trimmed (`[q.strip() for q in body.splitlines() if q.strip()]`).
pub fn nonempty_lines(body: &str) -> Vec<String> {
    py_splitlines(body)
        .into_iter()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty())
        .collect()
}

/// Python's `str.splitlines()` for the separators that occur in text.
pub fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        let c = s[i..].chars().next().unwrap();
        let cl = c.len_utf8();
        let is_sep = matches!(
            c,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if is_sep {
            out.push(&s[start..i]);
            if c == '\r' && b.get(i + 1) == Some(&b'\n') {
                i += 1;
            }
            i += cl;
            start = i;
        } else {
            i += cl;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

pub fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// A line that is only `|||` (or `||` when `two` allows it), with spaces.
fn is_bars(line: &str, min: usize, max: usize) -> bool {
    let t = line.trim();
    let n = t.len();
    n >= min && n <= max && t.bytes().all(|b| b == b'|')
}

/// `_parse_two_column_body`: (left, right, explicit).
pub fn parse_two_column_body(body: &str) -> (String, String, bool) {
    let raw = normalize_newlines(body);
    let lines: Vec<&str> = raw.split('\n').collect();
    for (min, max) in [(3, 3), (2, 2)] {
        if let Some(i) = lines.iter().position(|l| is_bars(l, min, max)) {
            let left = lines[..i].join("\n");
            let right = lines[i + 1..].join("\n");
            return (left.trim().to_string(), right.trim().to_string(), true);
        }
    }
    // Legacy decks: the first blank line separates the columns.
    let (l, r) = split_first_blank(&raw);
    (l.trim().to_string(), r.trim().to_string(), false)
}

/// `re.split(r"\n\s*\n", raw, maxsplit=1)`.
fn split_first_blank(raw: &str) -> (String, String) {
    let parts = split_blank(raw, Some(1));
    let l = parts.first().cloned().unwrap_or_default();
    let r = parts.get(1).cloned().unwrap_or_default();
    (l, r)
}

/// `re.split(r"\n\s*\n", raw)`: `\s*` is greedy and may span several lines.
pub fn split_blank(raw: &str, max: Option<usize>) -> Vec<String> {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let mut splits = 0;
    while i < chars.len() {
        if chars[i] == '\n' && max.is_none_or(|m| splits < m) {
            // Greedy \s* then backtrack to the last \n inside it.
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let last_nl = (i + 1..j).rev().find(|&k| chars[k] == '\n');
            if let Some(k) = last_nl {
                out.push(std::mem::take(&mut cur));
                splits += 1;
                i = k + 1;
                continue;
            }
        }
        cur.push(chars[i]);
        i += 1;
    }
    out.push(cur);
    out
}

/// `_parse_columns`: `count` literal columns split on lines that are only `||` or `|||`.
pub fn parse_columns(body: &str, count: usize) -> Vec<String> {
    let raw = normalize_newlines(body);
    let mut parts: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in raw.split('\n') {
        if is_bars(line, 2, 3) {
            parts.push(cur.join("\n"));
            cur.clear();
        } else {
            cur.push(line);
        }
    }
    parts.push(cur.join("\n"));
    let mut parts: Vec<String> = parts.into_iter().map(|p| p.trim().to_string()).collect();
    if parts.len() == 1 && count > 1 {
        parts = split_blank(raw.trim(), None)
            .into_iter()
            .map(|p| p.trim().to_string())
            .collect();
    }
    if parts.len() > count {
        let tail = parts[count - 1..].join("\n\n");
        parts.truncate(count - 1);
        parts.push(tail);
    }
    while parts.len() < count {
        parts.push(String::new());
    }
    parts
}

pub fn table_rows(body: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for q in py_splitlines(body) {
        let q = q.trim();
        if q.is_empty() || q.chars().all(|c| c.is_whitespace() || "|:-+".contains(c)) {
            continue;
        }
        let mut q = q;
        if let Some(r) = q.strip_prefix('|') {
            q = r;
        }
        if q.ends_with('|') && !q.ends_with("\\|") {
            q = &q[..q.len() - 1];
        }
        rows.push(q.split('|').map(|c| c.trim().to_string()).collect());
    }
    rows
}

/// `_take`: show at most `cap` items; the last slot says "+N more".
pub fn take_items(items: Vec<String>, cap: usize) -> (Vec<String>, usize) {
    if items.len() <= cap {
        return (items, 0);
    }
    let shown: Vec<String> = items[..cap.saturating_sub(1)].to_vec();
    let hidden = items.len() - shown.len();
    (shown, hidden)
}

pub fn split_pipe(item: &str) -> (String, String) {
    match item.split_once('|') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (item.trim().to_string(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_defaults_normalize_like_2x() {
        let s = json!({"layout": "BULLETS", "mono": true, "backgroundFocalX": 0.0,
                       "transition": "cut", "logoMode": "on", "backgroundDim": 120});
        let d = slide_defaults(s.as_object());
        assert_eq!(d["imageTreatment"], "MONO");
        assert_eq!(d["backgroundFocalX"], 0.5);
        assert_eq!(d["transition"], "NONE");
        assert_eq!(d["logoMode"], "ON");
        assert_eq!(d["backgroundDim"], 90);
        // Default keys first, in the 2.x order.
        assert_eq!(d.keys().next().unwrap(), "id");
    }

    #[test]
    fn column_parsing() {
        let (l, r, e) = parse_two_column_body("a\nb\n\n|||\n\nc");
        assert_eq!((l.as_str(), r.as_str(), e), ("a\nb", "c", true));
        let (l, r, e) = parse_two_column_body("a\n\n\nb\nc");
        assert_eq!((l.as_str(), r.as_str(), e), ("a", "b\nc", false));
        assert_eq!(
            parse_columns("a\n|||\nb\n||\nc\n|||\nd", 3),
            vec!["a", "b", "c\n\nd"]
        );
        assert_eq!(parse_columns("a\n\nb", 3), vec!["a", "b", ""]);
    }

    #[test]
    fn tables_and_items() {
        let rows = table_rows("| A | B |\n|---|---|\n| 1 | 2 |");
        assert_eq!(rows, vec![vec!["A", "B"], vec!["1", "2"]]);
        assert_eq!(
            clean_items("- one\n•  two\n*three", true),
            vec!["one", "two", "*three"]
        );
        let (shown, hidden) = take_items((0..9).map(|i| i.to_string()).collect(), 7);
        assert_eq!((shown.len(), hidden), (6, 3));
    }

    #[test]
    fn python_str_of_numbers() {
        assert_eq!(py_str(Some(&json!(1.0))), "1.0");
        assert_eq!(py_str(Some(&json!(0.5))), "0.5");
        assert_eq!(py_str(Some(&json!(75))), "75");
    }
}
