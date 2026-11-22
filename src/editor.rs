//! The editor window: toolbar, slide list, deck controls, the slide
//! canvas, the inspector and the status bar, with every 2.x command.
use crate::canvas::{Canvas, RasterPainter, Rect};
use crate::chrome::{self, blend, Chrome};
use crate::dialogs::{self, Dialog, Kind, Then};
use crate::ids::*;
use crate::model::*;
use crate::paint::{RectF, Rgba};
use crate::prefs::{self, FilePrefs};
use crate::render::{self, Ctx};
use crate::ui::{self, BtnKind, Edit, EditResult, Fonts, Paint, Popup};
use crate::x::{self, Ev, X};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const HINT: &str = "DOUBLE-CLICK TO EDIT  ·  CTRL+K COMMANDS  ·  F5 PRESENT  ·  ONLINE SHARES AUDIENCE";
const THUMB_W: i32 = 176;
const THUMB_H: i32 = 99;
const ITEM_H: i32 = 135;
const OUTLINE_H: i32 = 52;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PopAct {
    Act(Act),
    Combo(Cid, usize),
}

#[derive(Clone, Debug)]
pub enum Drag {
    Slides { from: usize, start_y: i32, moved: bool },
    Split { left: bool, start_x: i32, start_w: i32 },
    Slider(Sid, Rect),
    Text(Fid),
    ListScroll,
}

/// What the app loop should do after an editor event.
#[derive(Debug, PartialEq)]
pub enum Request {
    None,
    Quit,
    Present(PresentReq),
    TransitionPreview(usize),
    Talk(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct PresentReq {
    pub start: usize,
    pub presenter: Option<usize>,
    pub audience: Option<usize>,
    pub online: bool,
    pub talk: bool,
    pub share_check: bool,
}

pub struct Thumb {
    key: u64,
    cv: Canvas,
    pub issues: Vec<String>,
}

pub struct Editor {
    pub win: u32,
    pub cv: Canvas,
    pub data: Obj,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub extract: crate::tempdir::TempDir,
    history: Vec<String>,
    history_pos: isize,
    slide_clip: Vec<Obj>,
    pub outline: bool,
    pub file_prefs: FilePrefs,
    pub profile: String,
    pub chrome_available: bool,
    pub chrome_follow: bool,
    pub palette: Option<Chrome>,
    pub c: Chrome,
    pub f: Fonts,
    pub cur: usize,
    pub sel: BTreeSet<usize>,
    sel_anchor: usize,
    pub left_visible: bool,
    pub right_visible: bool,
    pub toolbar_visible: bool,
    left_w: i32,
    right_w: i32,
    pub guides: bool,
    pub inspector_open: Vec<bool>,
    insp_scroll: i32,
    insp_content_h: i32,
    list_scroll: i32,
    pub edits: HashMap<Fid, Edit>,
    pub focus: Option<Fid>,
    hover: Option<Hit>,
    pub popup: Option<Popup<PopAct>>,
    popup_owner: Option<Hit>,
    pub dialog: Option<Dialog>,
    pub status: Option<(String, Instant, u64)>,
    pub save_state: String,
    pub hits: Vec<(Rect, Hit)>,
    tips: Vec<(Rect, String)>,
    tip: Option<(String, i32, i32)>,
    hover_at: Option<(Instant, i32, i32)>,
    thumbs: HashMap<usize, Thumb>,
    autosave_at: Option<Instant>,
    history_at: Option<Instant>,
    drag: Option<Drag>,
    pub recents: Vec<PathBuf>,
    families: Option<Vec<String>>,
    last_click: (Instant, i32, i32, u8),
    pub dirty_draw: bool,
    toolbar_hidden: Vec<&'static str>,
    pub mapped: bool,
    mouse: (i32, i32),
    canvas_rect: RectF,
    field_rects: HashMap<Fid, Rect>,
    pub presenting: bool,
    pub quit_after_present: bool,
}

fn now() -> Instant {
    Instant::now()
}

pub fn clock_now() -> String {
    clock_at(unsafe { libc::time(std::ptr::null_mut()) })
}

pub fn clock_at(t: i64) -> String {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

fn hash_str(s: &str) -> u64 {
    // FNV-1a.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

pub const FOCAL_X: [&str; 3] = ["LEFT", "CENTER", "RIGHT"];
pub const FOCAL_Y: [&str; 3] = ["TOP", "CENTER", "BOTTOM"];

fn focal_name(v: f64, names: &[&str; 3]) -> &'static str {
    let names: [&'static str; 3] = if names[0] == "LEFT" { FOCAL_X } else { FOCAL_Y };
    if v == 0.0 {
        names[0]
    } else if v == 1.0 {
        names[2]
    } else {
        "CENTER"
    }
}

fn focal_value(name: &str) -> f64 {
    match name {
        "LEFT" | "TOP" => 0.0,
        "RIGHT" | "BOTTOM" => 1.0,
        _ => 0.5,
    }
}

impl Editor {
    pub fn new(x: &X) -> x::Res<Self> {
        let root = prefs::data_root();
        let _ = std::fs::create_dir_all(&root);
        let profile = prefs::resolve_profile(&root);
        let chrome_available = chrome::detect();
        let follow = match chrome::read_follow() {
            None => chrome_available,
            Some(p) => p && chrome_available,
        };
        let palette = if follow {
            chrome::load_nirux(x.root_theme().as_deref())
        } else {
            None
        };
        let c = palette.clone().unwrap_or_else(Chrome::noir);
        let f = Fonts::load().ok_or("no usable font (install fonts-noto or DejaVu)")?;
        let mut data = default_deck();
        normalize_loaded(&mut data, "Untitled");
        data.insert(
            "showLogo".into(),
            json!(prefs::profile_info(&profile).show_logo_default),
        );
        let (w, h) = (1540, 920);
        let win = x.create(80, 60, w as u16, h as u16, &format!("{APP} {VERSION}"), "nirupres", "NIRUPRES", false, false, true)?;
        x::register_window(win);
        let mut e = Self {
            win,
            cv: Canvas::new(w, h),
            data,
            path: None,
            dirty: false,
            extract: crate::tempdir::TempDir::new("nirupres-")?,
            history: Vec::new(),
            history_pos: -1,
            slide_clip: Vec::new(),
            outline: false,
            file_prefs: FilePrefs::load(),
            profile,
            chrome_available,
            chrome_follow: follow,
            palette,
            c,
            f,
            cur: 0,
            sel: BTreeSet::new(),
            sel_anchor: 0,
            left_visible: true,
            right_visible: true,
            toolbar_visible: true,
            left_w: 230,
            right_w: 370,
            guides: false,
            inspector_open: prefs::inspector_state(),
            insp_scroll: 0,
            insp_content_h: 0,
            list_scroll: 0,
            edits: HashMap::new(),
            focus: None,
            hover: None,
            popup: None,
            popup_owner: None,
            dialog: None,
            status: None,
            save_state: String::new(),
            hits: Vec::new(),
            tips: Vec::new(),
            tip: None,
            hover_at: None,
            thumbs: HashMap::new(),
            autosave_at: None,
            history_at: None,
            drag: None,
            recents: prefs::recents(),
            families: None,
            last_click: (now(), 0, 0, 0),
            dirty_draw: true,
            toolbar_hidden: Vec::new(),
            mapped: false,
            mouse: (-1, -1),
            canvas_rect: RectF::default(),
            field_rects: HashMap::new(),
            presenting: false,
            quit_after_present: false,
        };
        e.sel.insert(0);
        e.load_fields();
        e.push_history();
        Ok(e)
    }

    // ------------------------------------------------------------ data

    pub fn slides(&self) -> &Vec<Value> {
        match self.data.get("slides") {
            Some(Value::Array(a)) => a,
            _ => unreachable!("normalized decks always have slides"),
        }
    }

    fn slides_mut(&mut self) -> &mut Vec<Value> {
        if !matches!(self.data.get("slides"), Some(Value::Array(_))) {
            self.data.insert("slides".into(), json!([]));
        }
        match self.data.get_mut("slides") {
            Some(Value::Array(a)) => a,
            _ => unreachable!(),
        }
    }

    pub fn current(&self) -> Option<&Obj> {
        self.slides().get(self.cur).and_then(|v| v.as_object())
    }

    fn current_mut(&mut self) -> Option<&mut Obj> {
        let i = self.cur;
        self.slides_mut().get_mut(i).and_then(|v| v.as_object_mut())
    }

    pub fn profile_info(&self) -> &'static prefs::Profile {
        prefs::profile_info(&self.profile)
    }

    pub fn style(&self) -> DeckStyle {
        DeckStyle::of(&self.data)
    }

    pub fn set_status(&mut self, msg: &str, ms: u64) {
        self.status = Some((msg.to_string(), now(), ms));
        self.dirty_draw = true;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.save_state = "● Unsaved changes".into();
        self.autosave_at = Some(now() + Duration::from_millis(2500));
        self.dirty_draw = true;
    }

    pub fn push_history(&mut self) {
        let snap = serde_json::to_string(&self.data).unwrap_or_default();
        if self.history_pos >= 0 && self.history.get(self.history_pos as usize) == Some(&snap) {
            return;
        }
        self.history.truncate((self.history_pos + 1) as usize);
        self.history.push(snap);
        if self.history.len() > 60 {
            let extra = self.history.len() - 60;
            self.history.drain(..extra);
        }
        while self.history.len() > 1
            && self.history.iter().map(|x| x.len()).sum::<usize>() > MAX_HISTORY_BYTES
        {
            self.history.remove(0);
        }
        self.history_pos = self.history.len() as isize - 1;
    }

    fn restore_history(&mut self, pos: isize) {
        if pos < 0 || pos as usize >= self.history.len() {
            return;
        }
        if let Ok(Value::Object(o)) = serde_json::from_str(&self.history[pos as usize]) {
            self.data = o;
            self.history_pos = pos;
            let sel = self.cur;
            self.refresh(sel);
            self.mark_dirty();
        }
    }

    pub fn undo(&mut self) {
        self.history_at = None;
        self.restore_history(self.history_pos - 1);
    }

    pub fn redo(&mut self) {
        self.restore_history(self.history_pos + 1);
    }

    /// `refresh`: the list and deck controls changed; select `sel`.
    pub fn refresh(&mut self, sel: usize) {
        let n = self.slides().len();
        self.cur = sel.min(n.saturating_sub(1));
        self.sel.clear();
        self.sel.insert(self.cur);
        self.sel_anchor = self.cur;
        self.thumbs.retain(|i, _| *i < n);
        self.load_fields();
        self.scroll_to_current();
        self.dirty_draw = true;
    }

    fn edit_mut(&mut self, id: Fid) -> &mut Edit {
        self.edits
            .entry(id)
            .or_insert_with(|| Edit::new("", id.multiline()))
    }

    fn set_field(&mut self, id: Fid, s: &str) {
        let e = self.edit_mut(id);
        if e.string() != s {
            e.reset(s);
        }
    }

    pub fn field(&self, id: Fid) -> String {
        self.edits.get(&id).map(|e| e.string()).unwrap_or_default()
    }

    /// `select`: put the current slide's values into the fields.
    pub fn load_fields(&mut self) {
        let Some(s) = self.current().cloned() else { return };
        self.set_field(Fid::Title, &s_get(&s, "title", ""));
        self.set_field(Fid::Body, &s_get(&s, "body", ""));
        self.set_field(Fid::Img, &s_get(&s, "image", ""));
        self.set_field(Fid::BgImg, &s_get(&s, "backgroundImage", ""));
        self.set_field(Fid::Caption, &s_get(&s, "caption", ""));
        self.set_field(Fid::ImageRequest, &s_get(&s, "imageRequest", ""));
        self.set_field(Fid::VideoSource, &s_get(&s, "videoSource", ""));
        self.set_field(Fid::VideoPoster, &s_get(&s, "videoPoster", ""));
        self.set_field(Fid::Notes, &s_get(&s, "notes", ""));
        let footer = s_get(&self.data, "footerText", "");
        self.set_field(Fid::Footer, &footer);
    }

    /// One inspector value changed: write it to the slide (`changed`).
    fn slide_set(&mut self, key: &str, v: Value) {
        let Some(s) = self.current_mut() else { return };
        s.insert(key.to_string(), v);
        self.after_slide_change();
    }

    fn after_slide_change(&mut self) {
        self.mark_dirty();
        let i = self.cur;
        self.thumbs.remove(&i);
        self.history_at = Some(now() + Duration::from_millis(500));
    }

    fn field_changed(&mut self, id: Fid) {
        let v = self.field(id);
        let key = match id {
            Fid::Title => "title",
            Fid::Body => "body",
            Fid::Img => "image",
            Fid::BgImg => "backgroundImage",
            Fid::Caption => "caption",
            Fid::ImageRequest => "imageRequest",
            Fid::VideoSource => "videoSource",
            Fid::VideoPoster => "videoPoster",
            Fid::Notes => "notes",
            Fid::Footer => {
                self.data.insert("footerText".into(), json!(v.trim()));
                self.mark_dirty();
                self.thumbs.clear();
                self.history_at = Some(now() + Duration::from_millis(500));
                return;
            }
            Fid::D(_) => {
                dialogs::field_changed(self, id);
                return;
            }
        };
        self.slide_set(key, json!(v));
    }

    pub fn deck_changed(&mut self) {
        self.mark_dirty();
        self.thumbs.clear();
        let c = self.cur;
        self.refresh(c);
        self.history_at = Some(now() + Duration::from_millis(500));
    }

    fn families(&mut self) -> &Vec<String> {
        if self.families.is_none() {
            self.families = Some(crate::fonts::db().families());
        }
        self.families.as_ref().unwrap()
    }

    // ---------------------------------------------------------- combos

    pub fn combo_items(&mut self, id: Cid) -> Vec<String> {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match id {
            Cid::Theme | Cid::SlideTheme => THEMES.iter().map(|t| t.name.to_string()).collect(),
            Cid::Font | Cid::SlideFont => self.families().clone(),
            Cid::Ratio => v(&ASPECTS),
            Cid::Transition => v(&TRANSITIONS),
            Cid::Recent => {
                let mut r = vec!["RECENT FILES…".to_string()];
                r.extend(self.recents.iter().map(|p| {
                    p.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                }));
                r
            }
            Cid::Layout => v(&LAYOUTS),
            Cid::ListStyle => v(&["NUMBERS", "DOTS", "DASHES", "NONE"]),
            Cid::ImgMode => v(&["FIT", "FILL"]),
            Cid::Treatment => v(&["NATURAL", "MONO", "DIM", "CONTRAST"]),
            Cid::FocalX | Cid::BgFocalX => v(&FOCAL_X),
            Cid::FocalY | Cid::BgFocalY => v(&FOCAL_Y),
            Cid::BgMotion => v(&["NONE", "KEN BURNS"]),
            Cid::Logo => v(&LOGO_MODES),
            Cid::Accent => v(&ACCENTS),
            Cid::FontScale => v(&["−", "AUTO", "+"]),
            Cid::Align => v(&["LEFT", "CENTER", "RIGHT"]),
            Cid::Weight => v(&["AUTO", "LIGHT", "REGULAR", "BOLD"]),
            Cid::SlideTransition => v(&["DECK", "NONE", "FADE", "MORPH", "REVEAL"]),
            Cid::D(_) => self
                .dialog
                .as_ref()
                .map(|d| d.combo_items(id))
                .unwrap_or_default(),
        }
    }

    pub fn combo_value(&self, id: Cid) -> String {
        let s = self.current();
        let g = |k: &str, d: &str| s.map(|s| s_get(s, k, d)).unwrap_or_else(|| d.to_string());
        match id {
            Cid::Theme => s_get(&self.data, "theme", "B&W"),
            Cid::Font => s_get(&self.data, "fontFamily", "Noto Sans"),
            Cid::Ratio => s_get(&self.data, "aspect", "16:9"),
            Cid::Transition => normalize_transition(self.data.get("transition"), false),
            Cid::Recent => "RECENT FILES…".into(),
            Cid::Layout => g("layout", "TITLE"),
            Cid::ListStyle => g("listStyle", "NUMBERS"),
            Cid::ImgMode => g("imageMode", "FILL"),
            Cid::Treatment => {
                let mono = s.map(|s| b_get(s, "mono")).unwrap_or(false);
                g("imageTreatment", if mono { "MONO" } else { "NATURAL" }).to_uppercase()
            }
            Cid::FocalX => focal_name(s.map(|s| f_get(s, "focalX", 0.5)).unwrap_or(0.5), &FOCAL_X).into(),
            Cid::FocalY => focal_name(s.map(|s| f_get(s, "focalY", 0.5)).unwrap_or(0.5), &FOCAL_Y).into(),
            Cid::BgFocalX => focal_name(s.map(|s| f_get(s, "backgroundFocalX", 0.5)).unwrap_or(0.5), &FOCAL_X).into(),
            Cid::BgFocalY => focal_name(s.map(|s| f_get(s, "backgroundFocalY", 0.5)).unwrap_or(0.5), &FOCAL_Y).into(),
            Cid::BgMotion => g("backgroundMotion", "NONE"),
            Cid::SlideTheme => {
                let o = g("themeOverride", "DECK");
                if o == "DECK" {
                    s_get(&self.data, "theme", "B&W")
                } else {
                    o
                }
            }
            Cid::Logo => g("logoMode", "AUTO"),
            Cid::Accent => g("accentOverride", "AUTO"),
            Cid::SlideFont => {
                let o = g("fontFamily", "DECK");
                if o == "DECK" {
                    s_get(&self.data, "fontFamily", "Noto Sans")
                } else {
                    o
                }
            }
            Cid::FontScale => {
                let v = s.map(|s| f_get(s, "fontScale", 1.0)).unwrap_or(1.0);
                if (v - 0.85).abs() < 1e-9 {
                    "−".into()
                } else if (v - 1.15).abs() < 1e-9 {
                    "+".into()
                } else {
                    "AUTO".into()
                }
            }
            Cid::Align => g("align", "LEFT"),
            Cid::Weight => g("weight", "AUTO"),
            Cid::SlideTransition => {
                normalize_transition(s.and_then(|s| s.get("transition")).or(Some(&json!("DECK"))), true)
            }
            Cid::D(_) => self
                .dialog
                .as_ref()
                .map(|d| d.combo_value(id))
                .unwrap_or_default(),
        }
    }

    fn combo_enabled(&self, id: Cid) -> bool {
        let s = self.current();
        match id {
            Cid::SlideTheme => s.is_some_and(|s| s_get(s, "themeOverride", "DECK") != "DECK"),
            Cid::SlideFont => s.is_some_and(|s| s_get(s, "fontFamily", "DECK") != "DECK"),
            Cid::D(_) => self.dialog.as_ref().is_none_or(|d| d.combo_enabled(id)),
            _ => true,
        }
    }

    pub fn combo_chosen(&mut self, x: &mut X, id: Cid, idx: usize) -> Request {
        let items = self.combo_items(id);
        let Some(v) = items.get(idx).cloned() else {
            return Request::None;
        };
        match id {
            Cid::Theme => {
                self.data.insert("theme".into(), json!(v));
                self.deck_changed();
            }
            Cid::Font => {
                self.data.insert("fontFamily".into(), json!(if v.is_empty() { "Noto Sans".into() } else { v }));
                self.deck_changed();
            }
            Cid::Ratio => {
                self.data.insert("aspect".into(), json!(v));
                self.deck_changed();
            }
            Cid::Transition => {
                self.data.insert("transition".into(), json!(v));
                self.deck_changed();
            }
            Cid::Recent => {
                if idx > 0 {
                    if let Some(p) = self.recents.get(idx - 1).cloned() {
                        return self.open_path_checked(x, p);
                    }
                }
            }
            Cid::Layout => self.slide_set("layout", json!(v)),
            Cid::ListStyle => self.slide_set("listStyle", json!(v)),
            Cid::ImgMode => self.slide_set("imageMode", json!(v)),
            Cid::Treatment => {
                if let Some(s) = self.current_mut() {
                    s.insert("mono".into(), json!(v == "MONO"));
                }
                self.slide_set("imageTreatment", json!(v));
            }
            Cid::FocalX => self.slide_set("focalX", json!(focal_value(&v))),
            Cid::FocalY => self.slide_set("focalY", json!(focal_value(&v))),
            Cid::BgFocalX => self.slide_set("backgroundFocalX", json!(focal_value(&v))),
            Cid::BgFocalY => self.slide_set("backgroundFocalY", json!(focal_value(&v))),
            Cid::BgMotion => self.slide_set("backgroundMotion", json!(v)),
            Cid::SlideTheme => self.slide_set("themeOverride", json!(v)),
            Cid::Logo => self.slide_set("logoMode", json!(v)),
            Cid::Accent => self.slide_set("accentOverride", json!(v)),
            Cid::SlideFont => self.slide_set("fontFamily", json!(v)),
            Cid::FontScale => {
                let f = match v.as_str() {
                    "−" => 0.85,
                    "+" => 1.15,
                    _ => 1.0,
                };
                self.slide_set("fontScale", json!(f));
            }
            Cid::Align => self.slide_set("align", json!(v)),
            Cid::Weight => self.slide_set("weight", json!(v)),
            Cid::SlideTransition => self.slide_set("transition", json!(v)),
            Cid::D(_) => return dialogs::combo_chosen(self, x, id, idx),
        }
        Request::None
    }

    fn check_value(&self, id: Kid) -> bool {
        let s = self.current();
        let b = |k: &str| s.map(|s| b_get(s, k)).unwrap_or(false);
        match id {
            Kid::Numbers => b_get(&self.data, "showNumbers"),
            Kid::Guides => self.guides,
            Kid::Logo => Deck(&self.data).show_logo(),
            Kid::BgMono => b("backgroundMono"),
            Kid::Autoplay => b("videoAutoplay"),
            Kid::Muted => b("videoMuted"),
            Kid::OverrideTheme => s.is_some_and(|s| s_get(s, "themeOverride", "DECK") != "DECK"),
            Kid::OverrideFont => s.is_some_and(|s| s_get(s, "fontFamily", "DECK") != "DECK"),
            Kid::Hidden => b("hidden"),
            Kid::D(_) => self.dialog.as_ref().is_some_and(|d| d.check_value(id)),
        }
    }

    fn check_enabled(&self, id: Kid) -> bool {
        match id {
            Kid::Logo => theme(&s_get(&self.data, "theme", "B&W")).is_some_and(|t| t.uddevalla),
            Kid::D(_) => self.dialog.as_ref().is_none_or(|d| d.check_enabled(id)),
            _ => true,
        }
    }

    fn check_toggled(&mut self, id: Kid) {
        let on = !self.check_value(id);
        match id {
            Kid::Numbers => {
                self.data.insert("showNumbers".into(), json!(on));
                self.deck_changed();
            }
            Kid::Guides => {
                self.guides = on;
                self.dirty_draw = true;
            }
            Kid::Logo => {
                self.data.insert("showLogo".into(), json!(on));
                self.deck_changed();
            }
            Kid::BgMono => self.slide_set("backgroundMono", json!(on)),
            Kid::Autoplay => self.slide_set("videoAutoplay", json!(on)),
            Kid::Muted => self.slide_set("videoMuted", json!(on)),
            Kid::OverrideTheme => {
                let t = if on { s_get(&self.data, "theme", "B&W") } else { "DECK".into() };
                self.slide_set("themeOverride", json!(t));
            }
            Kid::OverrideFont => {
                let t = if on { s_get(&self.data, "fontFamily", "Noto Sans") } else { "DECK".into() };
                self.slide_set("fontFamily", json!(t));
            }
            Kid::Hidden => self.slide_set("hidden", json!(on)),
            Kid::D(_) => dialogs::check_toggled(self, id),
        }
    }

    fn slider_def(&self, id: Sid) -> (i32, i32, i32) {
        let s = self.current();
        let g = |k: &str, d: i64| s.map(|s| i_get(s, k, d)).unwrap_or(d) as i32;
        match id {
            Sid::BgDim => (0, 90, g("backgroundDim", 75)),
            Sid::BgBlur => (0, 20, g("backgroundBlur", 0)),
            Sid::Brightness => (-30, 30, g("brightness", 0)),
            Sid::Contrast => (0, 50, g("contrast", 0)),
            Sid::Overlay => (0, 60, g("overlay", 0)),
            Sid::Blur => (0, 10, g("blur", 0)),
            Sid::D(_) => self
                .dialog
                .as_ref()
                .map(|d| d.slider_def(id))
                .unwrap_or((0, 1, 0)),
        }
    }

    fn slider_set(&mut self, id: Sid, v: i32) {
        let (lo, hi, old) = self.slider_def(id);
        let v = v.clamp(lo, hi);
        if v == old {
            return;
        }
        let key = match id {
            Sid::BgDim => "backgroundDim",
            Sid::BgBlur => "backgroundBlur",
            Sid::Brightness => "brightness",
            Sid::Contrast => "contrast",
            Sid::Overlay => "overlay",
            Sid::Blur => "blur",
            Sid::D(_) => {
                if let Some(d) = self.dialog.as_mut() {
                    d.slider_set(id, v);
                }
                self.dirty_draw = true;
                return;
            }
        };
        self.slide_set(key, json!(v));
    }

    // ------------------------------------------------------- selection

    pub fn select(&mut self, i: usize) {
        if i >= self.slides().len() {
            return;
        }
        self.commit_focus();
        self.cur = i;
        self.load_fields();
        self.scroll_to_current();
        self.dirty_draw = true;
    }

    fn scroll_to_current(&mut self) {
        let ih = if self.outline { OUTLINE_H + 5 } else { ITEM_H + 5 };
        let top = self.cur as i32 * ih;
        let view = (self.cv.h - 200).max(ih);
        if top < self.list_scroll {
            self.list_scroll = top;
        } else if top + ih > self.list_scroll + view {
            self.list_scroll = top + ih - view;
        }
        self.list_scroll = self.list_scroll.max(0);
    }

    fn selected_rows(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.sel.iter().copied().filter(|&i| i < self.slides().len()).collect();
        if v.is_empty() {
            v.push(self.cur);
        }
        v
    }

    fn commit_focus(&mut self) {
        // Fields are written as they change; nothing is pending.
    }

    pub fn slide_accessible_text(&self, i: usize) -> String {
        let n = self.slides().len();
        let s = self.slides().get(i).and_then(|v| v.as_object());
        let title = s.map(|s| crate::markup::plain(&s_get(s, "title", ""))).unwrap_or_default();
        let title = if title.is_empty() { "untitled".to_string() } else { title };
        let hidden = if s.is_some_and(|s| b_get(s, "hidden")) { ", hidden" } else { "" };
        let layout = s.map(|s| s_get(s, "layout", "TITLE")).unwrap_or_default();
        format!("Slide {} of {n}: {title} ({layout}{hidden})", i + 1)
    }

    // ------------------------------------------------------- thumbnails

    fn thumb(&mut self, i: usize) -> Option<&Thumb> {
        let s = self.slides().get(i)?.as_object()?.clone();
        let style = self.style();
        let key = hash_str(&format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            serde_json::to_string(&s).unwrap_or_default(),
            style.theme,
            style.font,
            style.show_logo,
            style.footer,
            style.footer_align,
            style.footer_size,
            style.total,
            i,
            style.show_numbers
        ));
        if self.thumbs.get(&i).is_none_or(|t| t.key != key) {
            let mut cv = Canvas::new(THUMB_W, THUMB_H);
            cv.fill(cv.bounds(), 0x0b0b0b);
            let ctx = Ctx::new(&style, i, false);
            let r = render::aspect_rect(RectF::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32), &s_get(&self.data, "aspect", "16:9"));
            {
                let mut p = RasterPainter { cv: &mut cv };
                render::render_slide(&mut p, r, &s, &ctx);
            }
            crate::trace("thumb rendered");
            let issues = render::slide_fit_issues(&s, &self.data, i);
            crate::trace("fit issues");
            self.thumbs.insert(i, Thumb { key, cv, issues });
        }
        self.thumbs.get(&i)
    }

    // ------------------------------------------------------------ draw

    fn hit(&mut self, r: Rect, h: Hit) {
        self.hits.push((r, h));
    }

    fn tipz(&mut self, r: Rect, s: &str) {
        self.tips.push((r, s.to_string()));
    }

    fn hov(&self, h: Hit) -> bool {
        self.hover == Some(h)
    }

    pub fn is_hover(&self, h: Hit) -> bool {
        self.hover == Some(h)
    }

    fn btn(&mut self, r: Rect, label: &str, a: Act, kind: BtnKind, tip: &str) {
        let hv = self.hov(Hit::Act(a));
        let checked = match a {
            Act::ChromeToggle => self.chrome_follow,
            Act::ToggleLeft => self.left_visible,
            Act::ToggleRight => self.right_visible,
            Act::ToggleOutline => self.outline,
            _ => false,
        };
        let enabled = !(a == Act::ChromeToggle && !self.chrome_available);
        let mut p = Paint { cv: &mut self.cv, c: &self.c, f: &self.f, px: self.c.font_px };
        p.button(r, label, kind, hv, false, enabled, checked);
        if matches!(a, Act::ExportMenu | Act::TalkMenu | Act::PresetsMenu) {
            let col = if kind == BtnKind::Primary { self.c.on_accent } else { self.c.fg };
            let mut p = Paint { cv: &mut self.cv, c: &self.c, f: &self.f, px: self.c.font_px };
            p.chevron(r.x + r.w - 14, r.y + r.h / 2 - 1, col);
        }
        if enabled {
            self.hit(r, Hit::Act(a));
        }
        if !tip.is_empty() {
            self.tipz(r, tip);
        }
    }

    fn combo(&mut self, r: Rect, id: Cid) {
        let v = self.combo_value(id);
        let en = self.combo_enabled(id);
        let open = self.popup_owner == Some(Hit::Combo(id));
        let hv = self.hov(Hit::Combo(id));
        let mut p = Paint { cv: &mut self.cv, c: &self.c, f: &self.f, px: self.c.font_px };
        p.combo(r, &v, hv, false, en, open);
        if en {
            self.hit(r, Hit::Combo(id));
        }
    }

    fn check(&mut self, r: Rect, id: Kid, label: &str) {
        let on = self.check_value(id);
        let en = self.check_enabled(id);
        let hv = self.hov(Hit::Check(id));
        let mut p = Paint { cv: &mut self.cv, c: &self.c, f: &self.f, px: self.c.font_px };
        p.check(r, label, on, hv, en);
        if en {
            self.hit(r, Hit::Check(id));
        }
    }

    fn slider(&mut self, r: Rect, id: Sid) {
        let (lo, hi, v) = self.slider_def(id);
        let hv = self.hov(Hit::Slider(id)) || matches!(self.drag, Some(Drag::Slider(s, _)) if s == id);
        let mut p = Paint { cv: &mut self.cv, c: &self.c, f: &self.f, px: self.c.font_px };
        p.slider(r, lo, hi, v, hv, false);
        self.hit(r, Hit::Slider(id));
    }

    pub fn field_draw(&mut self, r: Rect, id: Fid, placeholder: &str) {
        let focused = self.focus == Some(id);
        let hv = self.hov(Hit::Field(id));
        let c = self.c.clone();
        let border = if focused {
            c.focus
        } else if hv {
            blend(c.border_strong, c.fg, 0.15)
        } else {
            c.border
        };
        self.cv.fill(r, if focused { blend(c.input, c.fg, 0.01) } else { c.input });
        self.cv.frame(r, border);
        self.field_rects.insert(id, r);
        self.hit(r, Hit::Field(id));
        let px = c.font_px;
        let face = self.f.sans.clone();
        let inner = Rect::new(r.x + 8, r.y + 4, r.w - 16, r.h - 8);
        let e = self.edit_mut(id).clone();
        let lh = ui::line_h(&face, px).ceil();
        self.cv.push_clip_rect(inner);
        if e.text.is_empty() && !placeholder.is_empty() {
            let y = if id.multiline() { inner.y as f32 } else { inner.y as f32 + (inner.h as f32 - lh) / 2.0 };
            ui::text(&mut self.cv, &face, px, inner.x as f32, y, placeholder, blend(c.input, c.muted, 0.55));
        }
        if id.multiline() {
            let lay = ui::area_layout(&face, px, &e.text, inner.w as f32);
            let mut scroll = e.scroll;
            // Keep the cursor visible.
            let cl = lay.line_of(e.cur) as f32 * lay.line_h;
            if focused {
                if cl < scroll {
                    scroll = cl;
                } else if cl + lay.line_h > scroll + inner.h as f32 {
                    scroll = cl + lay.line_h - inner.h as f32;
                }
            }
            let maxs = (lay.lines.len() as f32 * lay.line_h - inner.h as f32).max(0.0);
            scroll = scroll.clamp(0.0, maxs);
            self.edit_mut(id).scroll = scroll;
            let (sa, sb) = e.sel();
            for (k, (a, b)) in lay.lines.iter().enumerate() {
                let y = inner.y as f32 + k as f32 * lay.line_h - scroll;
                if y + lay.line_h < inner.y as f32 || y > (inner.y + inner.h) as f32 {
                    continue;
                }
                if focused && sa != sb && sb > *a && sa <= *b {
                    let x0: f32 = lay.adv[*a..sa.max(*a)].iter().sum();
                    let x1: f32 = lay.adv[*a..sb.min(*b)].iter().sum();
                    let extra = if sb > *b { 5.0 } else { 0.0 };
                    self.cv.fill(Rect::new(inner.x + x0 as i32, y as i32, (x1 - x0 + extra).ceil() as i32, lay.line_h as i32), 0xeeeeee.min(c.focus.max(0x555555)));
                }
                let s: String = e.text[*a..*b].iter().collect();
                ui::text(&mut self.cv, &face, px, inner.x as f32, y, &s, c.fg);
            }
            if focused {
                let l = lay.line_of(e.cur);
                let x = lay.x_of(e.cur);
                let y = inner.y as f32 + l as f32 * lay.line_h - scroll;
                self.cv.fill(Rect::new(inner.x + x as i32, y as i32 + 1, 1, lay.line_h as i32 - 2), c.fg);
            }
        } else {
            let s: String = e.text.iter().collect();
            let pre: String = e.text[..e.cur].iter().collect();
            let cx = ui::text_w(&face, px, &pre, 0.0);
            let mut scroll = e.scroll;
            if focused {
                if cx - scroll > inner.w as f32 - 2.0 {
                    scroll = cx - inner.w as f32 + 2.0;
                } else if cx < scroll {
                    scroll = cx;
                }
            } else {
                scroll = 0.0;
            }
            self.edit_mut(id).scroll = scroll;
            let y = inner.y as f32 + (inner.h as f32 - lh) / 2.0;
            if focused && e.has_sel() {
                let (a, b) = e.sel();
                let xa = ui::text_w(&face, px, &e.text[..a].iter().collect::<String>(), 0.0);
                let xb = ui::text_w(&face, px, &e.text[..b].iter().collect::<String>(), 0.0);
                self.cv.fill(Rect::new(inner.x + (xa - scroll) as i32, y as i32, (xb - xa).ceil() as i32, lh as i32), c.focus.max(0x555555));
            }
            ui::text(&mut self.cv, &face, px, inner.x as f32 - scroll, y, &s, c.fg);
            if focused {
                self.cv.fill(Rect::new(inner.x + (cx - scroll) as i32, y as i32, 1, lh as i32), c.fg);
            }
        }
        self.cv.pop_clip_rect();
    }

    pub fn draw(&mut self) {
        self.hits.clear();
        self.tips.clear();
        self.field_rects.clear();
        let (w, h) = (self.cv.w, self.cv.h);
        let c = self.c.clone();
        self.cv.reset_clip();
        self.cv.fill(Rect::new(0, 0, w, h), c.bg);
        let mut top = 0;
        crate::trace("draw start");
        if self.toolbar_visible {
            top = self.draw_toolbar();
        }
        crate::trace("toolbar");
        let status_h = 26;
        let body = Rect::new(0, top, w, h - top - status_h);
        let mut x0 = 0;
        let mut x1 = w;
        if self.left_visible {
            let lw = self.left_w.clamp(200, (w / 3).max(200));
            self.draw_left(Rect::new(0, body.y, lw, body.h));
            self.cv.fill(Rect::new(lw, body.y, 1, body.h), 0x181818.max(c.border));
            self.hit(Rect::new(lw - 2, body.y, 5, body.h), Hit::SplitLeft);
            x0 = lw + 1;
        }
        if self.right_visible {
            let rw = self.right_w.clamp(300, (w / 2).max(300));
            self.cv.fill(Rect::new(w - rw - 1, body.y, 1, body.h), 0x181818.max(c.border));
            self.hit(Rect::new(w - rw - 3, body.y, 5, body.h), Hit::SplitRight);
            self.draw_inspector(Rect::new(w - rw, body.y, rw, body.h));
            x1 = w - rw - 1;
        }
        crate::trace("left+inspector");
        self.draw_center(Rect::new(x0, body.y, x1 - x0, body.h));
        crate::trace("center");
        self.draw_status(Rect::new(0, h - status_h, w, status_h));
        if let Some(d) = self.dialog.take() {
            self.cv.reset_clip();
            let mut d = d;
            dialogs::draw(self, &mut d);
            self.dialog = Some(d);
        }
        if let Some(p) = &self.popup {
            self.cv.reset_clip();
            p.draw(&mut self.cv, &c, &self.f, c.font_px);
            let r = p.rect;
            self.hits.push((r, Hit::Popup));
        }
        if let Some((s, x, y)) = self.tip.clone() {
            ui::tooltip(&mut self.cv, &c, &self.f, c.font_px, x, y, &s);
        }
    }

    fn toolbar_items(&self) -> Vec<(&'static str, Vec<(String, Act, BtnKind, &'static str, i32)>)> {
        let b = |s: &str, a: Act, k: BtnKind, t: &'static str, minw: i32| (s.to_string(), a, k, t, minw);
        let n = BtnKind::Normal;
        let aux = BtnKind::Aux;
        vec![
            ("File", vec![
                b("NEW", Act::New, n, "New · Ctrl+N", 0),
                b("OPEN", Act::Open, n, "Open · Ctrl+O", 0),
                b("SAVE", Act::Save, n, "Save · Ctrl+S", 0),
                b("EXPORT", Act::ExportMenu, n, "Export presentation…", 0),
                b("IMPORT MD", Act::ImportMd, n, "Import NIRUPRES Markdown", 0),
            ]),
            ("Slide", vec![
                b("+ SLIDE", Act::AddSlide, n, "Add slide · Ctrl+Enter", 0),
                b("DUPLICATE", Act::Duplicate, n, "Duplicate slide · Ctrl+D", 0),
                b("DELETE", Act::Delete, n, "Delete selected slides · Delete", 0),
                b("↶", Act::Undo, n, "Undo · Ctrl+Z", 0),
                b("↷", Act::Redo, n, "Redo · Ctrl+Shift+Z", 0),
            ]),
            ("View", vec![
                b("FOCUS", Act::Focus, n, "Focus mode · Tab", 0),
                b("NIRUX", Act::ChromeToggle, n, "Follow NIRUX editor chrome · slide themes stay independent", 0),
                b("⌘", Act::Palette, n, "Command palette · Ctrl+K", 0),
                b("?", Act::About, n, "About NIRUPRES / diagnostics", 34),
            ]),
            ("Present", vec![
                b("▶ PRESENT", Act::Present, BtnKind::Primary, "Present from current slide · F5", 0),
                b("|▶", Act::PresentStart, aux, "Present from beginning · Shift+F5", 42),
                b("▣", Act::PresenterView, aux, "Presenter View / display", 42),
                b("● TALK", Act::TalkMenu, aux, "Present and record with NIRUCLIP · slide chapters and subtitles", 84),
                b("◉ ONLINE", Act::Online, aux, "Online presentation · opens a clean Audience window for Teams/Meet/Zoom sharing", 86),
                b("▦", Act::Overview, aux, "Presentation overview", 42),
            ]),
        ]
    }

    fn draw_toolbar(&mut self) -> i32 {
        let c = self.c.clone();
        let w = self.cv.w;
        let px = c.font_px;
        let bh = (ui::line_h(&self.f.sans, px).ceil() as i32 + 16).max(32);
        let th = bh + 12;
        self.cv.fill(Rect::new(0, 0, w, th), c.toolbar);
        self.cv.fill(Rect::new(0, th - 1, w, 1), blend(c.border, c.bg, 0.2));
        let groups = self.toolbar_items();
        let widths: Vec<(&str, i32)> = groups
            .iter()
            .map(|(name, items)| {
                let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &self.f, px };
                let wsum: i32 = items
                    .iter()
                    .map(|(l, a, _, _, m)| {
                        let extra = if matches!(a, Act::ExportMenu | Act::TalkMenu) { 20 } else { 0 };
                        (p.button_w(l) + extra).max(*m) + 4
                    })
                    .sum();
                (*name, wsum + 15)
            })
            .collect();
        // Groups move into ⋯ when the window is narrow: View, Slide, File.
        let mut hidden: Vec<&'static str> = Vec::new();
        let mut need: i32 = widths.iter().map(|(_, w)| *w).sum::<i32>() + 20;
        for name in ["View", "Slide", "File"] {
            if need <= w {
                break;
            }
            hidden.push(name);
            need -= widths.iter().find(|(n, _)| *n == name).map(|(_, w)| *w).unwrap_or(0);
            need += 50;
        }
        self.toolbar_hidden = hidden.clone();
        let mut x = 6;
        let y = 6;
        let mut first = true;
        for (name, items) in groups {
            if hidden.contains(&name) {
                continue;
            }
            if !first {
                self.cv.fill(Rect::new(x + 3, y + 5, 1, bh - 10), c.border_strong);
                x += 11;
            }
            first = false;
            for (label, a, kind, tip, minw) in items {
                let extra = if matches!(a, Act::ExportMenu | Act::TalkMenu) { 20 } else { 0 };
                let bw = {
                    let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &self.f, px };
                    (p.button_w(&label) + extra).max(minw)
                };
                let r = Rect::new(x, y, bw, bh);
                let label = if extra > 0 { format!("{label}   ") } else { label };
                self.btn(r, &label, a, kind, tip);
                x += bw + 4;
            }
        }
        if !hidden.is_empty() {
            x += 7;
            self.btn(Rect::new(x, y, 40, bh), "⋯", Act::Overflow, BtnKind::Normal, "More toolbar actions");
        }
        th
    }

    fn draw_left(&mut self, r: Rect) {
        let c = self.c.clone();
        let px = c.font_px;
        let pad = 10;
        let bh = (ui::line_h(&self.f.sans, px).ceil() as i32 + 14).max(30);
        let hy = r.y + pad;
        let n = self.slides().len();
        {
            let mut p = Paint { cv: &mut self.cv, c: &c, f: &self.f, px };
            p.label(Rect::new(r.x + pad, hy, 80, bh), "SLIDES", c.muted);
        }
        let ow = {
            let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &self.f, px };
            p.button_w("OUTLINE").max(72)
        };
        let collapse = Rect::new(r.x + r.w - 6 - 28, hy, 28, bh);
        let outline = Rect::new(collapse.x - 4 - ow, hy, ow, bh);
        {
            let mut p = Paint { cv: &mut self.cv, c: &c, f: &self.f, px };
            let cnt = n.to_string();
            let cw = ui::text_w(&self.f.sans, px, &cnt, 0.0).ceil() as i32;
            p.label(Rect::new(outline.x - 10 - cw, hy, cw + 2, bh), &cnt, c.muted);
        }
        self.btn(outline, "OUTLINE", Act::ToggleOutline, BtnKind::Normal, "Switch between slide thumbnails and compact outline");
        self.btn(collapse, "‹", Act::HideLeft, BtnKind::Normal, "Hide Slides panel");
        let list = Rect::new(r.x + pad, hy + bh + 8, r.w - pad - 6, r.h - (bh + 8) - pad * 2 - 2 * (bh + 6));
        self.cv.fill(list, c.input);
        self.cv.frame(list, c.border);
        self.hit(list, Hit::SlideList);
        let ih = if self.outline { OUTLINE_H } else { ITEM_H };
        let step = ih + 5;
        let max_scroll = (n as i32 * step + 10 - list.h).max(0);
        self.list_scroll = self.list_scroll.clamp(0, max_scroll);
        self.cv.push_clip_rect(list.inset(1));
        let first = (self.list_scroll / step).max(0) as usize;
        let mut i = first;
        while i < n {
            let iy = list.y + 5 + i as i32 * step - self.list_scroll;
            if iy > list.y + list.h {
                break;
            }
            let row = Rect::new(list.x + 5, iy, list.w - 10, ih);
            let selected = self.sel.contains(&i);
            if selected {
                self.cv.fill(row, blend(c.input, c.fg, 0.04));
                self.cv.frame(row, c.focus.min(0x777777).max(c.focus));
            }
            if let Some(Drag::Slides { from, moved: true, .. }) = &self.drag {
                if *from != i && self.mouse.1 >= iy && self.mouse.1 < iy + step {
                    self.cv.fill(Rect::new(row.x, iy - 3, row.w, 2), c.fg);
                }
            }
            self.hit(row, Hit::Slide(i));
            let s = self.slides()[i].as_object().cloned().unwrap_or_default();
            let title = s_get(&s, "title", "");
            if self.outline {
                let mut flags = String::new();
                if b_get(&s, "hidden") {
                    flags.push_str("  [HIDDEN]");
                }
                if truthy(s.get("imageRequest")) {
                    flags.push_str("  [IMAGE?]");
                }
                let t1 = format!("{:02}  {}", i + 1, if title.is_empty() { "(untitled)" } else { &title });
                let t2 = format!("     {}{flags}", s_get(&s, "layout", "TITLE"));
                let face = self.f.sans.clone();
                ui::text_in(&mut self.cv, &face, px, Rect::new(row.x + 6, row.y + 4, row.w - 12, 22), &t1, c.fg, 0, 0.0);
                ui::text_in(&mut self.cv, &face, px, Rect::new(row.x + 6, row.y + 26, row.w - 12, 20), &t2, c.muted, 0, 0.0);
            } else {
                let tx = row.x + 4;
                let ty = row.y + 4;
                let issues = {
                    let t = self.thumb(i);
                    t.map(|t| t.issues.clone()).unwrap_or_default()
                };
                if let Some(t) = self.thumbs.get(&i) {
                    let src = &t.cv;
                    let dst = RectF::new(tx as f32, ty as f32, THUMB_W as f32, THUMB_H as f32);
                    // Copy the cached thumbnail.
                    self.cv.draw_canvas(src, RectF::new(0.0, 0.0, THUMB_W as f32, THUMB_H as f32), dst, 255);
                }
                if !issues.is_empty() {
                    let bf = self.f.black.clone();
                    let bw = ui::text_w(&bf, 9.0, "FIT", 0.0).ceil() as i32 + 8;
                    let br = Rect::new(tx + 3, ty + 3, bw, 14);
                    self.cv.round_rect(br, 2, 0xE8A33D);
                    ui::text_in(&mut self.cv, &bf, 9.0, br, "FIT", 0x111111, 1, 0.0);
                    let tip = format!("Text does not fit:\n{}", issues.join("\n"));
                    self.tipz(br, &tip);
                    let rtip = format!("Deck health: {}", issues.join("; "));
                    self.tipz(row, &rtip);
                }
                let mut flags = String::new();
                if truthy(s.get("notes")) {
                    flags.push_str("  ◇");
                }
                if b_get(&s, "hidden") {
                    flags.push_str("  ⊘");
                }
                if s_get(&s, "themeOverride", "DECK") != "DECK" {
                    flags.push_str("  ●");
                }
                if !issues.is_empty() {
                    flags.push_str("  ⚠");
                }
                let name = if title.is_empty() { s_get(&s, "layout", "") } else { title };
                let short: String = name.chars().take(20).collect();
                let label = format!("{:02}  {short}{flags}", i + 1);
                let face = self.f.sans.clone();
                ui::text_in(&mut self.cv, &face, 10.0, Rect::new(tx, ty + THUMB_H + 3, row.w - 8, 18), &label, 0xaaaaaa.min(c.muted.max(0x888888)), 0, 0.0);
            }
            i += 1;
        }
        self.cv.pop_clip_rect();
        if max_scroll > 0 {
            let track = Rect::new(list.x + list.w - 5, list.y + 2, 3, list.h - 4);
            let th = ((list.h as f32 / (max_scroll + list.h) as f32) * track.h as f32).max(20.0) as i32;
            let ty = track.y + ((self.list_scroll as f32 / max_scroll as f32) * (track.h - th) as f32) as i32;
            self.cv.fill(Rect::new(track.x, ty, track.w, th), c.border_strong);
        }
        let by = list.y + list.h + 6;
        self.btn(Rect::new(r.x + pad, by, r.w - pad - 6, bh), "+ ADD SLIDE", Act::AddSlide, BtnKind::Normal, "");
        self.btn(Rect::new(r.x + pad, by + bh + 6, r.w - pad - 6, bh), "PRESETS", Act::PresetsMenu, BtnKind::Normal, "Insert a ready-made slide pattern");
    }

    fn draw_center(&mut self, r: Rect) {
        let c = self.c.clone();
        let px = c.font_px;
        let bh = (ui::line_h(&self.f.sans, px).ceil() as i32 + 14).max(30);
        let x0 = r.x + 12;
        let w = r.w - 24;
        let mut y = r.y + 10;
        let pw = |e: &Self, s: &str| {
            let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &e.f, px };
            p.button_w(s)
        };
        // Header: SLIDES, recent files, the document, INSPECTOR.
        let sw = pw(self, "SLIDES").max(70);
        self.btn(Rect::new(x0, y, sw, bh), "SLIDES", Act::ToggleLeft, BtnKind::Normal, if self.left_visible { "Hide Slides panel" } else { "Show Slides panel" });
        self.combo(Rect::new(x0 + sw + 8, y, 170, bh), Cid::Recent);
        self.tipz(Rect::new(x0 + sw + 8, y, 170, bh), "Open a recent presentation");
        let iw = pw(self, "INSPECTOR").max(86);
        self.btn(Rect::new(x0 + w - iw, y, iw, bh), "INSPECTOR", Act::ToggleRight, BtnKind::Normal, if self.right_visible { "Hide Inspector panel" } else { "Show Inspector panel" });
        let (display, tip) = match &self.path {
            Some(p) => (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), p.display().to_string()),
            None => {
                let t = s_get(&self.data, "title", "Untitled");
                (if t.is_empty() { "Untitled".into() } else { t }, "Unsaved presentation".into())
            }
        };
        let df = self.f.bold.clone();
        let shown = ui::elide(&df, 12.0, &display, 360.0, 0.5);
        let label = format!("{shown}{}", if self.dirty { "  •" } else { "" });
        let dw = ui::text_w(&df, 12.0, &label, 0.5).ceil() as i32 + 30;
        let dr = Rect::new(x0 + (w - dw) / 2, y + 2, dw, bh - 4);
        let hv = self.hov(Hit::Act(Act::DocTitle));
        self.cv.round_rect(dr, 3, if hv { blend(c.border, c.fg, 0.2) } else { c.border });
        self.cv.round_rect(dr.inset(1), 3, if hv { blend(c.toolbar, c.fg, 0.03) } else { c.toolbar });
        ui::text_in(&mut self.cv, &df, 12.0, dr, &label, if hv { 0xffffff.min(c.fg.max(0xdddddd)) } else { blend(c.fg, c.bg, 0.05) }, 1, 0.5);
        self.hit(dr, Hit::Act(Act::DocTitle));
        self.tipz(dr, &format!("{tip}{} · click for deck settings", if self.dirty { " · unsaved changes" } else { "" }));
        y += bh + 8;
        // Deck controls.
        let mut xx = x0;
        let cap = |e: &mut Self, x: i32, y: i32, s: &str| -> i32 {
            let mut p = Paint { cv: &mut e.cv, c: &c, f: &e.f, px };
            let cw = p.cap_w(s);
            p.cap(Rect::new(x, y, cw, bh), s);
            cw
        };
        xx += cap(self, xx, y, "THEME") + 8;
        let rest = (w - (xx - x0) - 160).max(300);
        let tw = (rest * 2 / 5).max(170);
        self.combo(Rect::new(xx, y, tw, bh), Cid::Theme);
        self.tipz(Rect::new(xx, y, tw, bh), "Deck theme");
        xx += tw + 16;
        xx += cap(self, xx, y, "FONT") + 8;
        let fw = (rest * 3 / 5 - 40).max(190);
        self.combo(Rect::new(xx, y, fw, bh), Cid::Font);
        self.tipz(Rect::new(xx, y, fw, bh), "Deck typeface");
        xx += fw + 16;
        xx += cap(self, xx, y, "RATIO") + 8;
        let rw = 104.min((x0 + w - xx).max(96));
        self.combo(Rect::new(xx, y, rw, bh), Cid::Ratio);
        self.tipz(Rect::new(xx, y, rw, bh), "Presentation aspect ratio");
        y += bh + 7;
        let mut xx = x0;
        xx += cap(self, xx, y, "TRANSITION") + 10;
        self.combo(Rect::new(xx, y, 140, bh), Cid::Transition);
        self.tipz(Rect::new(xx, y, 140, bh), "Audience transition: NONE, FADE, MORPH or REVEAL");
        xx += 140 + 18;
        for (id, label, tip) in [
            (Kid::Numbers, "NUMBERS", "Show slide numbers on audience output"),
            (Kid::Guides, "GUIDES", "Show editor alignment guides · never shown to audience"),
            (Kid::Logo, "LOGO", self.profile_info().logo_tooltip),
        ] {
            let cw = {
                let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &self.f, px };
                p.check_w(label)
            };
            self.check(Rect::new(xx, y, cw, bh), id, label);
            self.tipz(Rect::new(xx, y, cw, bh), tip);
            xx += cw + 10;
        }
        y += bh + 7;
        let mut xx = x0;
        xx += cap(self, xx, y, "FOOTER") + 8;
        let dsw = pw(self, "DECK SETTINGS…");
        let ph = self.profile_info().footer_placeholder;
        self.field_draw(Rect::new(xx, y, x0 + w - dsw - 12 - xx, bh), Fid::Footer, ph);
        self.tipz(Rect::new(xx, y, x0 + w - dsw - 12 - xx, bh), "Optional deck footer · alignment and size are available in Deck Settings");
        self.btn(Rect::new(x0 + w - dsw, y, dsw, bh), "DECK SETTINGS…", Act::DeckSettings, BtnKind::Normal, "Open all deck-level settings in one dialog");
        y += bh + 10;
        // The slide.
        let hint_h = 24;
        let view = Rect::new(x0, y, w, r.y + r.h - 10 - hint_h - y);
        self.draw_slide_view(view);
        let mf = self.f.mono.clone();
        ui::text_in(&mut self.cv, &mf, 10.0, Rect::new(x0, view.y + view.h + 6, w, hint_h - 6), HINT, 0x666666.min(c.label.max(0x555555)), 0, 0.0);
    }

    fn draw_slide_view(&mut self, view: Rect) {
        if view.w < 20 || view.h < 20 {
            return;
        }
        self.cv.fill(view, 0x0B0B0B);
        let aspect = s_get(&self.data, "aspect", "16:9");
        let r = render::aspect_rect(view.f(), &aspect);
        self.canvas_rect = r;
        self.hit(view, Hit::Canvas);
        self.cv.fill_aa(r.adjusted(-1.0, -1.0, 1.0, 1.0), Rgba::hex(0x2A2A2A));
        let style = self.style();
        let ctx = Ctx::new(&style, self.cur, style.show_numbers);
        if let Some(s) = self.current().cloned() {
            self.cv.push_clip_rect(view);
            {
                let mut p = RasterPainter { cv: &mut self.cv };
                render::render_slide(&mut p, r, &s, &ctx);
            }
            self.cv.pop_clip_rect();
        }
        if self.guides {
            let g = Rgba(255, 255, 255, 55);
            let safe = r.adjusted(r.w * 0.055, r.h * 0.055, -r.w * 0.055, -r.h * 0.055);
            let mut p = RasterPainter { cv: &mut self.cv };
            use crate::paint::Painter;
            p.stroke_rect(safe, g, 1.0);
            let (cx, cy) = r.center();
            p.line(cx.trunc(), r.y, cx.trunc(), r.bottom(), g, 1.0);
            p.line(r.x, cy.trunc(), r.right(), cy.trunc(), g, 1.0);
        }
    }

    fn summary(&self, section: &str) -> String {
        let Some(s) = self.current() else { return String::new() };
        let focal = |xk: &str, yk: &str| {
            let fx = match f_get(s, xk, 0.5) {
                v if v == 0.0 => "left",
                v if v == 0.5 => "center",
                v if v == 1.0 => "right",
                _ => "custom",
            };
            let fy = match f_get(s, yk, 0.5) {
                v if v == 0.0 => "top",
                v if v == 0.5 => "center",
                v if v == 1.0 => "bottom",
                _ => "custom",
            };
            if fx == "center" && fy == "center" {
                "center".to_string()
            } else {
                format!("{fx}/{fy}")
            }
        };
        match section {
            "CONTENT" => {
                let t = s_get(s, "title", "");
                let t = if t.trim().is_empty() { "untitled".to_string() } else { t.trim().to_string() };
                format!("{} · {t}", s_get(s, "layout", "TITLE").to_lowercase())
            }
            "IMAGE" => {
                let mut parts = Vec::new();
                if truthy(s.get("image")) {
                    parts.push(s_get(s, "imageMode", "FILL").to_lowercase());
                    parts.push(focal("focalX", "focalY"));
                    let t = s_get(s, "imageTreatment", "NATURAL").to_lowercase();
                    if t != "natural" {
                        parts.push(t);
                    }
                }
                if truthy(s.get("backgroundImage")) {
                    parts.push("background".into());
                    if s_get(s, "backgroundMotion", "NONE") != "NONE" {
                        parts.push("ken burns".into());
                    }
                }
                if parts.is_empty() {
                    "none".into()
                } else {
                    parts.join(" · ")
                }
            }
            "VIDEO" => {
                let src = s_get(s, "videoSource", "");
                if src.trim().is_empty() {
                    return "none".into();
                }
                let l = src.trim().to_lowercase();
                let mut parts = vec![if l.starts_with("http://") || l.starts_with("https://") { "web" } else { "local" }.to_string()];
                if b_get(s, "videoAutoplay") {
                    parts.push("autoplay".into());
                }
                if b_get(s, "videoMuted") {
                    parts.push("muted".into());
                }
                parts.join(" · ")
            }
            "STYLE" => {
                let ov = s_get(s, "themeOverride", "DECK");
                let mut parts = vec![
                    if ov == "DECK" { "deck theme".into() } else { ov.to_lowercase() },
                    s_get(s, "align", "LEFT").to_lowercase(),
                ];
                let ff = s_get(s, "fontFamily", "DECK");
                if ff != "DECK" {
                    parts.push(ff);
                }
                let lm = s_get(s, "logoMode", "AUTO");
                if lm != "AUTO" {
                    parts.push(format!("logo {}", lm.to_lowercase()));
                }
                parts.join(" · ")
            }
            "PRESENTATION" => {
                let mut parts = vec![if b_get(s, "hidden") { "hidden" } else { "shown" }.to_string()];
                if !s_get(s, "notes", "").trim().is_empty() {
                    parts.push("notes".into());
                }
                let t = normalize_transition(s.get("transition").or(Some(&json!("DECK"))), true);
                parts.push(if t == "DECK" { "deck transition".into() } else { t.to_lowercase() });
                parts.join(" · ")
            }
            _ => String::new(),
        }
    }

    fn draw_inspector(&mut self, r: Rect) {
        let c = self.c.clone();
        let px = c.font_px;
        let bh = (ui::line_h(&self.f.sans, px).ceil() as i32 + 14).max(30);
        let pad = 10;
        {
            let mut p = Paint { cv: &mut self.cv, c: &c, f: &self.f, px };
            p.label(Rect::new(r.x + 6, r.y + pad, 120, bh), "INSPECTOR", c.muted);
        }
        self.btn(Rect::new(r.x + r.w - pad - 28, r.y + pad, 28, bh), "›", Act::HideRight, BtnKind::Normal, "Hide Inspector panel");
        let area = Rect::new(r.x + 6, r.y + pad + bh + 6, r.w - 6 - pad, r.h - pad * 2 - bh - 6);
        self.hit(area, Hit::Inspector);
        let max_scroll = (self.insp_content_h - area.h).max(0);
        self.insp_scroll = self.insp_scroll.clamp(0, max_scroll);
        self.cv.push_clip_rect(area);
        let x0 = area.x;
        let w = area.w - 8;
        let lab_w = 92;
        let fx = x0 + lab_w;
        let fw = w - lab_w;
        let mut y = area.y - self.insp_scroll;
        let start_y = y;
        let sections = prefs::INSPECTOR_SECTIONS;
        for (si, name) in sections.iter().enumerate() {
            let open = self.inspector_open[si];
            // Section header with a rule and, when collapsed, the summary.
            y += 12;
            let head = Rect::new(x0, y - 8, w, 26);
            self.cv.fill(Rect::new(x0, y, w, 1), blend(c.border, c.bg, 0.1));
            let mut title = format!("{}  {name}", if open { "▼" } else { "▶" });
            let mut tip = String::new();
            if !open {
                let summ = self.summary(name);
                if !summ.is_empty() {
                    let shown: String = if summ.chars().count() > 30 {
                        summ.chars().take(29).collect::<String>().trim_end().to_string() + "…"
                    } else {
                        summ.clone()
                    };
                    title = format!("{title}  ·  {shown}");
                    let mut n = name.to_lowercase();
                    if let Some(f) = n.get_mut(0..1) {
                        f.make_ascii_uppercase();
                    }
                    tip = format!("{n}: {summ}");
                }
            }
            let bf = self.f.bold.clone();
            let tw = ui::text_w(&bf, px, &title, 0.0).ceil() as i32 + 12;
            self.cv.fill(Rect::new(x0, y - 8, tw.min(w), 18), c.bg);
            ui::text_in(&mut self.cv, &bf, px, Rect::new(x0 + 2, y - 9, w - 4, 20), &title, blend(c.fg, c.bg, 0.15), 0, 0.0);
            self.hit(head, Hit::Section(si));
            if !tip.is_empty() {
                self.tipz(head, &tip);
            }
            y += 18;
            if !open {
                continue;
            }
            let row = |y: &mut i32, h: i32| -> Rect {
                let r = Rect::new(fx, *y, fw, h);
                *y += h + 6;
                r
            };
            let lab = |e: &mut Self, y: i32, s: &str| {
                let face = e.f.sans.clone();
                ui::text_in(&mut e.cv, &face, px, Rect::new(x0 + 4, y, lab_w - 8, bh), s, c.muted, 0, 0.0);
            };
            match *name {
                "CONTENT" => {
                    lab(self, y, "Layout");
                    let rr = row(&mut y, bh);
                    self.combo(rr, Cid::Layout);
                    lab(self, y, "Title");
                    let rr = row(&mut y, bh);
                    self.field_draw(rr, Fid::Title, "");
                    lab(self, y, "Body");
                    let rr = row(&mut y, 220);
                    self.field_draw(rr, Fid::Body, "");
                    lab(self, y, "List style");
                    let rr = row(&mut y, bh);
                    self.combo(rr, Cid::ListStyle);
                    self.tipz(rr, "Marker style for BULLETS and AGENDA layouts");
                }
                "IMAGE" => {
                    let cw = 84;
                    let full = Rect::new(x0 + 4, y, w - 4, bh);
                    self.field_draw(Rect::new(full.x, y, full.w - cw - 6, bh), Fid::Img, "Drop image here or choose…");
                    self.btn(Rect::new(full.x + full.w - cw, y, cw, bh), "CHOOSE", Act::ChooseImage, BtnKind::Normal, "");
                    y += bh + 6;
                    let mut xx = x0 + 4;
                    xx += self.small_cap(xx, y, bh, "MODE");
                    self.combo(Rect::new(xx, y, 86, bh), Cid::ImgMode);
                    self.tipz(Rect::new(xx, y, 86, bh), "FIT shows the whole image · FILL crops to the image frame");
                    xx += 86 + 10;
                    xx += self.small_cap(xx, y, bh, "TREATMENT");
                    let rw = 84;
                    let tw2 = (x0 + w - rw - 6 - xx).max(90);
                    self.combo(Rect::new(xx, y, tw2, bh), Cid::Treatment);
                    self.tipz(Rect::new(xx, y, tw2, bh), "Non-destructive image treatment");
                    self.btn(Rect::new(x0 + w - rw, y, rw, bh), "REMOVE", Act::RemoveImage, BtnKind::Normal, "");
                    y += bh + 6;
                    let mut xx = x0 + 4;
                    xx += self.small_cap(xx, y, bh, "FOCAL X");
                    let half = (w - 150) / 2;
                    self.combo(Rect::new(xx, y, half, bh), Cid::FocalX);
                    xx += half + 10;
                    xx += self.small_cap(xx, y, bh, "FOCAL Y");
                    self.combo(Rect::new(xx, y, (x0 + w - xx).max(60), bh), Cid::FocalY);
                    y += bh + 8;
                    self.small_cap(x0 + 4, y, 16, "BACKGROUND IMAGE");
                    y += 20;
                    let bw2 = 78;
                    self.field_draw(Rect::new(x0 + 4, y, w - 4 - 2 * (bw2 + 6), bh), Fid::BgImg, "Optional full-slide background…");
                    self.btn(Rect::new(x0 + w - 2 * bw2 - 6, y, bw2, bh), "CHOOSE", Act::ChooseBg, BtnKind::Normal, "");
                    self.btn(Rect::new(x0 + w - bw2, y, bw2, bh), "REMOVE", Act::RemoveBg, BtnKind::Normal, "");
                    y += bh + 6;
                    let mut xx = x0 + 4;
                    let mw = {
                        let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &self.f, px };
                        p.check_w("MONO")
                    };
                    self.check(Rect::new(xx, y, mw, bh), Kid::BgMono, "MONO");
                    xx += mw + 6;
                    let sl = (x0 + w - xx - 90) / 2;
                    let face = self.f.sans.clone();
                    ui::text_in(&mut self.cv, &face, px, Rect::new(xx, y, 34, bh), "DIM", c.muted, 0, 0.0);
                    xx += 34;
                    self.slider(Rect::new(xx, y, sl, bh), Sid::BgDim);
                    self.tipz(Rect::new(xx, y, sl, bh), "Theme-coloured dim veil · 75% is a safe readable default");
                    xx += sl + 6;
                    ui::text_in(&mut self.cv, &face, px, Rect::new(xx, y, 40, bh), "BLUR", c.muted, 0, 0.0);
                    xx += 40;
                    self.slider(Rect::new(xx, y, (x0 + w - xx).max(30), bh), Sid::BgBlur);
                    self.tipz(Rect::new(xx, y, (x0 + w - xx).max(30), bh), "Background-only blur");
                    y += bh + 6;
                    let mut xx = x0 + 4;
                    xx += self.small_cap(xx, y, bh, "BG FOCAL X");
                    let half = (x0 + w - xx - 30) / 2;
                    self.combo(Rect::new(xx, y, half, bh), Cid::BgFocalX);
                    xx += half + 8;
                    xx += self.small_cap(xx, y, bh, "Y");
                    self.combo(Rect::new(xx, y, (x0 + w - xx).max(60), bh), Cid::BgFocalY);
                    y += bh + 6;
                    let mut xx = x0 + 4;
                    xx += self.small_cap(xx, y, bh, "BG MOTION");
                    self.combo(Rect::new(xx, y, x0 + w - xx, bh), Cid::BgMotion);
                    self.tipz(Rect::new(xx, y, x0 + w - xx, bh), "Subtle background-only pan during presentation · disabled by Reduce motion");
                    y += bh + 6;
                    self.field_draw(Rect::new(x0 + 4, y, w - 4, bh), Fid::Caption, "Caption / attribution");
                    y += bh + 6;
                    self.field_draw(Rect::new(x0 + 4, y, w - 4, bh), Fid::ImageRequest, "AI image request / desired image…");
                    y += bh + 6;
                }
                "VIDEO" => {
                    self.field_draw(Rect::new(x0 + 4, y, w - 4, bh), Fid::VideoSource, "YouTube / Vimeo URL or local video path…");
                    y += bh + 6;
                    let b1 = 86;
                    let b2 = 120;
                    self.field_draw(Rect::new(x0 + 4, y, w - 4 - b1 - b2 - 12, bh), Fid::VideoPoster, "Optional poster image…");
                    self.btn(Rect::new(x0 + w - b1 - b2 - 6, y, b1, bh), "POSTER…", Act::ChoosePoster, BtnKind::Normal, "");
                    self.btn(Rect::new(x0 + w - b2, y, b2, bh), "LOCAL VIDEO…", Act::ChooseVideo, BtnKind::Normal, "");
                    y += bh + 6;
                    let aw = {
                        let p = Paint { cv: &mut Canvas::new(1, 1), c: &c, f: &self.f, px };
                        p.check_w("AUTOPLAY")
                    };
                    self.check(Rect::new(x0 + 4, y, aw, bh), Kid::Autoplay, "AUTOPLAY");
                    self.check(Rect::new(x0 + 4 + aw + 12, y, 120, bh), Kid::Muted, "MUTED");
                    y += bh + 6;
                }
                "STYLE" => {
                    let rr = row(&mut y, bh);
                    self.check(rr, Kid::OverrideTheme, "Override deck theme");
                    lab(self, y, "Slide theme");
                    let rr = row(&mut y, bh);
                    self.combo(rr, Cid::SlideTheme);
                    lab(self, y, "Logo");
                    let rr = row(&mut y, bh);
                    self.combo(rr, Cid::Logo);
                    self.tipz(rr, "AUTO follows layout rules · ON forces logo · OFF hides logo on this slide");
                    lab(self, y, "Accent");
                    let rr = row(&mut y, bh);
                    self.combo(rr, Cid::Accent);
                    let tip = self.profile_info().accent_tooltip;
                    self.tipz(rr, tip);
                    let rr = row(&mut y, bh);
                    self.check(rr, Kid::OverrideFont, "Override deck font");
                    lab(self, y, "Slide font");
                    let rr = row(&mut y, bh);
                    self.combo(rr, Cid::SlideFont);
                    lab(self, y, "Typography");
                    let face = self.f.sans.clone();
                    let half = (fw - 100) / 2;
                    ui::text_in(&mut self.cv, &face, px, Rect::new(fx, y, 44, bh), "Size", c.muted, 0, 0.0);
                    self.combo(Rect::new(fx + 44, y, half, bh), Cid::FontScale);
                    ui::text_in(&mut self.cv, &face, px, Rect::new(fx + 50 + half, y, 50, bh), "Align", c.muted, 0, 0.0);
                    self.combo(Rect::new(fx + 100 + half, y, fw - 100 - half, bh), Cid::Align);
                    y += bh + 6;
                    ui::text_in(&mut self.cv, &face, px, Rect::new(fx, y, 50, bh), "Weight", c.muted, 0, 0.0);
                    self.combo(Rect::new(fx + 56, y, fw - 56, bh), Cid::Weight);
                    y += bh + 6;
                    for (label, id, tip) in [
                        ("Brightness", Sid::Brightness, "Image brightness · 0 = unchanged"),
                        ("Contrast", Sid::Contrast, "Image contrast · 0 = unchanged"),
                        ("Overlay", Sid::Overlay, "Dark overlay strength · 0 = none"),
                        ("Blur", Sid::Blur, "Image blur · 0 = none"),
                    ] {
                        lab(self, y, label);
                        let rr = row(&mut y, bh);
                        self.slider(rr, id);
                        self.tipz(rr, tip);
                    }
                    let rr = row(&mut y, bh);
                    self.btn(rr, "APPLY STYLE TO SELECTED", Act::ApplyStyle, BtnKind::Normal, "Copy this slide’s visual style to all selected slides without replacing their content");
                }
                "PRESENTATION" => {
                    self.check(Rect::new(x0 + 4, y, w - 4, bh), Kid::Hidden, "HIDDEN SLIDE — skip while presenting");
                    y += bh + 6;
                    let mut xx = x0 + 4;
                    xx += self.small_cap(xx, y, bh, "TRANSITION");
                    let pv = 100;
                    self.combo(Rect::new(xx, y, x0 + w - xx - pv - 6, bh), Cid::SlideTransition);
                    self.tipz(Rect::new(xx, y, x0 + w - xx - pv - 6, bh), "Override the deck transition for this slide");
                    self.btn(Rect::new(x0 + w - pv, y, pv, bh), "▶ PREVIEW", Act::PreviewTransition, BtnKind::Normal, "Preview the transition into the current slide");
                    y += bh + 8;
                    let face = self.f.sans.clone();
                    ui::text_in(&mut self.cv, &face, px, Rect::new(x0 + 4, y, w, 18), "SPEAKER NOTES", c.muted, 0, 0.0);
                    y += 22;
                    self.field_draw(Rect::new(x0 + 4, y, w - 4, 160), Fid::Notes, "Private notes — never shown to audience");
                    y += 166;
                }
                _ => {}
            }
        }
        self.insp_content_h = y - start_y + 10;
        self.cv.pop_clip_rect();
        if max_scroll > 0 {
            let track = Rect::new(area.x + area.w - 4, area.y, 3, area.h);
            let th = ((area.h as f32 / self.insp_content_h as f32) * track.h as f32).max(24.0) as i32;
            let ty = track.y + ((self.insp_scroll as f32 / max_scroll as f32) * (track.h - th) as f32) as i32;
            self.cv.fill(Rect::new(track.x, ty, track.w, th), c.border_strong);
        }
    }

    fn small_cap(&mut self, x: i32, y: i32, h: i32, s: &str) -> i32 {
        let c = self.c.clone();
        let mut p = Paint { cv: &mut self.cv, c: &c, f: &self.f, px: c.font_px };
        let w = p.cap_w(s);
        p.cap(Rect::new(x, y, w, h), s);
        w + 6
    }

    fn draw_status(&mut self, r: Rect) {
        let c = self.c.clone();
        self.cv.fill(r, c.bg);
        let px = c.font_px;
        let face = self.f.sans.clone();
        let sw = ui::text_w(&face, px, &self.save_state, 0.0).ceil() as i32 + 16;
        if let Some((m, _, _)) = &self.status {
            let m = m.clone();
            ui::text_in(&mut self.cv, &face, px, Rect::new(r.x + 8, r.y, r.w - sw - 24, r.h), &m, c.muted, 0, 0.0);
        }
        let st = self.save_state.clone();
        ui::text_in(&mut self.cv, &face, px, Rect::new(r.x + r.w - sw - 8, r.y, sw, r.h), &st, c.muted, 2, 0.0);
        self.tipz(Rect::new(r.x + r.w - sw - 8, r.y, sw, r.h), "Unsaved edits are autosaved 2.5 s after the last change and offered for recovery after a crash");
    }

    // ---------------------------------------------------------- popups

    fn open_popup(&mut self, owner: Hit, anchor: Rect, items: Vec<(String, Option<PopAct>)>, current: Option<usize>) {
        let row_h = (ui::line_h(&self.f.sans, self.c.font_px).ceil() as i32 + 12).max(26);
        let maxw = items
            .iter()
            .map(|(l, _)| ui::text_w(&self.f.sans, self.c.font_px, l, 0.0).ceil() as i32 + 48)
            .max()
            .unwrap_or(120);
        let bounds = Rect::new(0, 0, self.cv.w, self.cv.h);
        let n = items.len().min(18);
        let rect = ui::place_popup(anchor, n, row_h, maxw.min(520), bounds);
        let vis = ((rect.h - 8) / row_h).max(1) as usize;
        let scroll = current.map(|c| c.saturating_sub(vis / 2)).unwrap_or(0).min(items.len().saturating_sub(vis));
        self.popup = Some(Popup {
            items,
            rect,
            hover: current,
            scroll,
            row_h,
            filter: String::new(),
            sections: Vec::new(),
        });
        self.popup_owner = Some(owner);
        self.dirty_draw = true;
    }

    fn open_combo(&mut self, id: Cid, anchor: Rect) {
        let items = self.combo_items(id);
        let cur_v = self.combo_value(id);
        let cur = items.iter().position(|i| *i == cur_v);
        let list = items
            .into_iter()
            .enumerate()
            .map(|(i, s)| (s, Some(PopAct::Combo(id, i))))
            .collect();
        self.open_popup(Hit::Combo(id), anchor, list, cur);
    }

    fn menu_items(&self, a: Act) -> Vec<(String, Option<PopAct>)> {
        let it = |s: &str, a: Act| (s.to_string(), Some(PopAct::Act(a)));
        let sep = || (String::new(), None);
        match a {
            Act::ExportMenu => vec![
                it("Current slide → PNG", Act::ExportPng),
                it("All slides → PNG", Act::ExportAllPng),
                it("Deck → PDF", Act::ExportPdf),
                it("Deck → PPTX (pixel-perfect)", Act::ExportPptx),
                sep(),
                it("NIRUPRES Markdown…", Act::ExportMd),
                it("AI brief (current deck)…", Act::ExportAiBrief),
                it("Copy AI prompt", Act::CopyAiPrompt),
                it("AI template/spec…", Act::ExportAiTemplate),
            ],
            Act::TalkMenu => {
                let mut v: Vec<_> = TALK_PROFILES
                    .iter()
                    .enumerate()
                    .map(|(i, (l, _))| it(l, Act::Talk(i as u8)))
                    .collect();
                v.push(sep());
                v.push(it("Talk helps / profiles…", Act::TalkHelp));
                v
            }
            Act::PresetsMenu => {
                let names = [
                    self.profile_info().title_preset,
                    "Agenda",
                    "Section divider",
                    "Key message",
                    "Three points",
                    "Image + message",
                    "Result / KPI",
                    "Thank you",
                ];
                names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| it(n, Act::Preset(i as u8)))
                    .collect()
            }
            Act::Overflow => {
                let mut v = Vec::new();
                for g in ["File", "Slide", "View"] {
                    if !self.toolbar_hidden.contains(&g) {
                        continue;
                    }
                    v.push((g.to_uppercase(), None));
                    match g {
                        "File" => {
                            v.push(it("NEW", Act::New));
                            v.push(it("OPEN", Act::Open));
                            v.push(it("SAVE", Act::Save));
                            for x in self.menu_items(Act::ExportMenu) {
                                if x.1.is_some() {
                                    v.push((format!("Export · {}", x.0), x.1));
                                }
                            }
                            v.push(it("IMPORT MD", Act::ImportMd));
                        }
                        "Slide" => {
                            v.push(it("+ SLIDE", Act::AddSlide));
                            v.push(it("DUPLICATE", Act::Duplicate));
                            v.push(it("DELETE", Act::Delete));
                            v.push(it("↶ Undo", Act::Undo));
                            v.push(it("↷ Redo", Act::Redo));
                        }
                        _ => {
                            v.push(it("FOCUS", Act::Focus));
                            v.push(it("NIRUX", Act::ChromeToggle));
                            v.push(it("⌘ Command palette", Act::Palette));
                            v.push(it("About NIRUPRES / diagnostics", Act::About));
                        }
                    }
                }
                v
            }
            _ => Vec::new(),
        }
    }

    // ----------------------------------------------------------- events

    fn hit_at(&self, x: i32, y: i32) -> Option<Hit> {
        if let Some(p) = &self.popup {
            if p.rect.contains(x, y) {
                return Some(Hit::Popup);
            }
        }
        if self.dialog.is_some() {
            // Only the dialog (recorded last) receives the mouse.
            for (r, h) in self.hits.iter().rev() {
                if r.contains(x, y) && matches!(h, Hit::D(_) | Hit::DList(_) | Hit::DListArea | Hit::Field(Fid::D(_)) | Hit::Combo(Cid::D(_)) | Hit::Check(Kid::D(_)) | Hit::Slider(Sid::D(_)) | Hit::Modal) {
                    return Some(*h);
                }
            }
            return Some(Hit::Modal);
        }
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(x, y))
            .map(|(_, h)| *h)
    }

    pub fn event(&mut self, x: &mut X, ev: &Ev) -> Request {
        match ev {
            Ev::Configure(_, _, _, w, h) => {
                if *w != self.cv.w || *h != self.cv.h {
                    self.cv.resize(*w, *h);
                    self.popup = None;
                    self.popup_owner = None;
                    self.dirty_draw = true;
                }
            }
            Ev::Expose(_) => self.dirty_draw = true,
            Ev::Close(_) => return self.request_close(x),
            Ev::Mapped(_, m) => {
                self.mapped = *m;
                self.dirty_draw = true;
            }
            Ev::Motion { x: mx, y: my, .. } => return self.motion(x, *mx, *my),
            Ev::Leave(_) => {
                self.hover = None;
                self.tip = None;
                self.hover_at = None;
                self.dirty_draw = true;
            }
            Ev::Button { x: bx, y: by, button, state, press, .. } => {
                return self.button(x, *bx, *by, *button, *state, *press);
            }
            Ev::Key { sym, state, ch, .. } => return self.key(x, *sym, *state, *ch),
            Ev::Drop(_, dx, dy, paths) => self.dropped(*dx, *dy, paths),
            _ => {}
        }
        Request::None
    }

    fn motion(&mut self, x: &mut X, mx: i32, my: i32) -> Request {
        self.mouse = (mx, my);
        if let Some(d) = self.drag.clone() {
            match d {
                Drag::Split { left, start_x, start_w } => {
                    let dx = mx - start_x;
                    if left {
                        self.left_w = (start_w + dx).clamp(200, self.cv.w / 3);
                    } else {
                        self.right_w = (start_w - dx).clamp(300, self.cv.w / 2);
                    }
                }
                Drag::Slider(id, r) => {
                    let (lo, hi, _) = self.slider_def(id);
                    let v = Paint::slider_value(r, lo, hi, mx);
                    self.slider_set(id, v);
                }
                Drag::Slides { from, start_y, moved } => {
                    if !moved && (my - start_y).abs() > 6 {
                        self.drag = Some(Drag::Slides { from, start_y, moved: true });
                    }
                }
                Drag::Text(id) => {
                    if let Some(i) = self.index_at(id, mx, my) {
                        self.edit_mut(id).cur = i;
                    }
                }
                Drag::ListScroll => {}
            }
            self.dirty_draw = true;
            return Request::None;
        }
        if let Some(p) = &mut self.popup {
            let h = p.row_at(mx, my);
            if h != p.hover && h.is_some() {
                p.hover = h;
                self.dirty_draw = true;
            }
        }
        let h = self.hit_at(mx, my);
        if h != self.hover {
            self.hover = h;
            self.tip = None;
            self.dirty_draw = true;
        }
        let cursor = match h {
            Some(Hit::Field(_)) => x.ibeam_cursor,
            _ => x.arrow_cursor,
        };
        x.set_cursor(self.win, cursor);
        if self.tip.is_none() {
            self.hover_at = Some((now(), mx, my));
        }
        Request::None
    }

    /// Tooltip after resting the pointer.
    pub fn tick_tooltip(&mut self) -> Option<Instant> {
        let (t, mx, my) = self.hover_at?;
        let due = t + Duration::from_millis(700);
        if now() >= due {
            self.hover_at = None;
            if self.popup.is_none() && self.drag.is_none() {
                if let Some((_, s)) = self.tips.iter().rev().find(|(r, _)| r.contains(mx, my)) {
                    self.tip = Some((s.clone(), mx, my));
                    self.dirty_draw = true;
                }
            }
            return None;
        }
        Some(due)
    }

    fn index_at(&mut self, id: Fid, mx: i32, my: i32) -> Option<usize> {
        let r = *self.field_rects.get(&id)?;
        let inner = Rect::new(r.x + 8, r.y + 4, r.w - 16, r.h - 8);
        let face = self.f.sans.clone();
        let px = self.c.font_px;
        let e = self.edits.get(&id)?;
        if id.multiline() {
            let lay = ui::area_layout(&face, px, &e.text, inner.w as f32);
            let line = (((my - inner.y) as f32 + e.scroll) / lay.line_h).max(0.0) as usize;
            Some(lay.index_at(line, (mx - inner.x) as f32))
        } else {
            let x = (mx - inner.x) as f32 + e.scroll;
            let mut w = 0.0;
            for i in 0..e.text.len() {
                let a = ui::text_w(&face, px, &e.text[i..i + 1].iter().collect::<String>(), 0.0);
                if w + a / 2.0 > x {
                    return Some(i);
                }
                w += a;
            }
            Some(e.text.len())
        }
    }

    fn button(&mut self, x: &mut X, mx: i32, my: i32, button: u8, state: u16, press: bool) -> Request {
        self.tip = None;
        self.hover_at = None;
        if button == 4 || button == 5 {
            if press {
                self.wheel(mx, my, if button == 4 { -1 } else { 1 });
            }
            return Request::None;
        }
        if !press {
            let d = self.drag.take();
            if let Some(Drag::Slides { from, moved: true, .. }) = d {
                if let Some(Hit::Slide(to)) = self.hit_at(mx, my) {
                    self.drag_reorder(from, to);
                }
            }
            if matches!(d, Some(Drag::Slider(..))) {
                self.push_history();
            }
            self.dirty_draw = true;
            return Request::None;
        }
        if button != 1 {
            return Request::None;
        }
        let dbl = {
            let (t, lx, ly, n) = self.last_click;
            let close = t.elapsed() < Duration::from_millis(400) && (lx - mx).abs() < 5 && (ly - my).abs() < 5;
            let n = if close { n + 1 } else { 1 };
            self.last_click = (now(), mx, my, n);
            n
        };
        let hit = self.hit_at(mx, my);
        // An open popup takes the click.
        if let Some(p) = self.popup.take() {
            let owner = self.popup_owner.take();
            self.dirty_draw = true;
            if hit == Some(Hit::Popup) {
                if let Some(i) = p.row_at(mx, my) {
                    if let Some(a) = p.items[i].1 {
                        return self.pop_act(x, a);
                    }
                }
                self.popup = Some(p);
                self.popup_owner = owner;
                return Request::None;
            }
            if hit == owner {
                return Request::None;
            }
        }
        let Some(hit) = hit else {
            self.blur();
            return Request::None;
        };
        if !matches!(hit, Hit::Field(_)) {
            self.blur();
        }
        self.dirty_draw = true;
        match hit {
            Hit::Act(a) => return self.act(x, a, Some(self.hit_rect(hit))),
            Hit::Combo(id) => {
                let r = self.hit_rect(hit);
                self.open_combo(id, r);
            }
            Hit::Check(id) => self.check_toggled(id),
            Hit::Slider(id) => {
                let r = self.hit_rect(hit);
                self.drag = Some(Drag::Slider(id, r));
                let (lo, hi, _) = self.slider_def(id);
                self.slider_set(id, Paint::slider_value(r, lo, hi, mx));
            }
            Hit::Field(id) => {
                self.focus = Some(id);
                if let Some(i) = self.index_at(id, mx, my) {
                    let shift = state & x::SHIFT != 0;
                    let e = self.edit_mut(id);
                    if dbl == 2 {
                        e.word_at(i);
                    } else if dbl >= 3 {
                        e.select_all();
                    } else {
                        e.cur = i;
                        if !shift {
                            e.anchor = i;
                        }
                        self.drag = Some(Drag::Text(id));
                    }
                }
            }
            Hit::Slide(i) => {
                let ctrl = state & x::CTRL != 0;
                let shift = state & x::SHIFT != 0;
                if ctrl {
                    if !self.sel.insert(i) && self.sel.len() > 1 {
                        self.sel.remove(&i);
                    }
                    self.select(i);
                } else if shift {
                    let (a, b) = (self.sel_anchor.min(i), self.sel_anchor.max(i));
                    self.sel = (a..=b).collect();
                    self.select(i);
                } else {
                    if !self.sel.contains(&i) || self.sel.len() == 1 {
                        self.sel.clear();
                        self.sel.insert(i);
                    }
                    self.sel_anchor = i;
                    self.select(i);
                    self.drag = Some(Drag::Slides { from: i, start_y: my, moved: false });
                }
            }
            Hit::Section(si) => {
                self.inspector_open[si] = !self.inspector_open[si];
                prefs::save_inspector_state(&self.inspector_open);
            }
            Hit::Canvas => {
                if dbl >= 2 && self.canvas_rect.contains(mx as f32, my as f32) {
                    dialogs::open_text_editor(self);
                }
            }
            Hit::SplitLeft => {
                self.drag = Some(Drag::Split { left: true, start_x: mx, start_w: self.left_w });
            }
            Hit::SplitRight => {
                self.drag = Some(Drag::Split { left: false, start_x: mx, start_w: self.right_w });
            }
            Hit::D(_) | Hit::DList(_) | Hit::DListArea | Hit::Modal => {
                return dialogs::click(self, x, hit, mx, my, dbl);
            }
            _ => {}
        }
        Request::None
    }

    fn hit_rect(&self, h: Hit) -> Rect {
        self.hits
            .iter()
            .rev()
            .find(|(_, x)| *x == h)
            .map(|(r, _)| *r)
            .unwrap_or_default()
    }

    fn blur(&mut self) {
        if self.focus.take().is_some() {
            self.push_history();
            self.dirty_draw = true;
        }
    }

    fn wheel(&mut self, mx: i32, my: i32, d: i32) {
        self.dirty_draw = true;
        if let Some(p) = &mut self.popup {
            if p.rect.contains(mx, my) {
                let n = p.filtered().len();
                let vis = p.visible();
                let s = (p.scroll as i32 + d * 3).clamp(0, n.saturating_sub(vis) as i32);
                p.scroll = s as usize;
                return;
            }
        }
        match self.hit_at(mx, my) {
            Some(Hit::DList(_)) | Some(Hit::DListArea) => dialogs::wheel(self, d),
            Some(Hit::Field(id)) if id.multiline() => {
                let e = self.edit_mut(id);
                e.scroll = (e.scroll + d as f32 * 40.0).max(0.0);
            }
            Some(h) if self.dialog.is_none() => {
                let in_list = matches!(h, Hit::Slide(_) | Hit::SlideList);
                let in_insp = self.hits.iter().any(|(r, hh)| *hh == Hit::Inspector && r.contains(mx, my));
                if in_list {
                    self.list_scroll = (self.list_scroll + d * 70).max(0);
                } else if in_insp {
                    self.insp_scroll = (self.insp_scroll + d * 60).max(0);
                }
            }
            _ => {}
        }
    }

    fn dropped(&mut self, dx: i32, dy: i32, paths: &[String]) {
        let Some(p) = paths.iter().find(|p| {
            let l = p.to_lowercase();
            [".png", ".jpg", ".jpeg", ".webp", ".bmp"].iter().any(|e| l.ends_with(e))
        }) else {
            return;
        };
        let target = self
            .field_rects
            .iter()
            .find(|(_, r)| r.contains(dx, dy))
            .map(|(id, _)| *id);
        match target {
            Some(Fid::Img) | Some(Fid::BgImg) | Some(Fid::VideoPoster) => {
                let id = target.unwrap_or(Fid::Img);
                self.set_field(id, p);
                self.field_changed(id);
                self.push_history();
            }
            Some(Fid::D(n)) => {
                self.set_field(Fid::D(n), p);
                self.dirty_draw = true;
            }
            _ => {}
        }
    }

    fn key(&mut self, x: &mut X, sym: u32, state: u16, ch: Option<char>) -> Request {
        use crate::keys::*;
        self.tip = None;
        self.dirty_draw = true;
        let ctrl = state & x::CTRL != 0;
        let shift = state & x::SHIFT != 0;
        let alt = state & x::ALT != 0;
        let sym = normalize(sym);
        // The open popup: arrows, Enter, Escape and type-to-find.
        if let Some(p) = &mut self.popup {
            let f = p.filtered();
            let pos = p.hover.and_then(|h| f.iter().position(|&i| i == h));
            match sym {
                K_ESCAPE => {
                    if !p.filter.is_empty() {
                        p.filter.clear();
                    } else {
                        self.popup = None;
                        self.popup_owner = None;
                    }
                }
                K_UP | K_DOWN | K_PRIOR | K_NEXT => {
                    let d: i32 = match sym {
                        K_UP => -1,
                        K_DOWN => 1,
                        K_PRIOR => -10,
                        _ => 10,
                    };
                    let mut k = pos.map(|p| p as i32 + d).unwrap_or(0).clamp(0, f.len() as i32 - 1);
                    while k >= 0 && (k as usize) < f.len() && p.items[f[k as usize]].1.is_none() {
                        k += d.signum();
                    }
                    if k >= 0 && (k as usize) < f.len() {
                        p.hover = Some(f[k as usize]);
                        let vis = p.visible();
                        if (k as usize) < p.scroll {
                            p.scroll = k as usize;
                        } else if k as usize >= p.scroll + vis {
                            p.scroll = k as usize + 1 - vis;
                        }
                    }
                }
                K_RETURN => {
                    let a = p.hover.and_then(|h| p.items.get(h)).and_then(|i| i.1);
                    self.popup = None;
                    self.popup_owner = None;
                    if let Some(a) = a {
                        return self.pop_act(x, a);
                    }
                }
                K_BACKSPACE => {
                    p.filter.pop();
                }
                _ => {
                    if let Some(c) = ch.filter(|c| !c.is_control()) {
                        p.filter.push(c);
                        let f = p.filtered();
                        p.scroll = 0;
                        p.hover = f.iter().copied().find(|&i| p.items[i].1.is_some());
                    }
                }
            }
            return Request::None;
        }
        if self.dialog.is_some() {
            return dialogs::key(self, x, sym, state, ch);
        }
        // A focused text field gets the keys first (as Qt's widgets do).
        if let Some(id) = self.focus {
            if sym == K_ESCAPE {
                self.blur();
                return Request::None;
            }
            let is_shortcut = ctrl
                && matches!(sym, 0x6e | 0x4e | 0x6f | 0x4f | 0x73 | 0x53 | 0x6b | 0x4b | 0x64 | 0x44 | K_RETURN | 0x65 | 0x45)
                || sym == K_F5
                || (sym == K_TAB && !id.multiline());
            if !is_shortcut {
                let r = self.edit_key(x, id, sym, state, ch);
                if r != EditResult::Unhandled {
                    return Request::None;
                }
            }
        }
        // Shortcuts.
        let a = match (ctrl, shift, alt, sym) {
            (true, false, false, 0x6e) | (true, false, false, 0x4e) => Some(Act::New),
            (true, false, false, 0x6f) | (true, false, false, 0x4f) => Some(Act::Open),
            (true, false, false, 0x73) | (true, false, false, 0x53) => Some(Act::Save),
            (true, true, false, 0x73) | (true, true, false, 0x53) => Some(Act::SaveAs),
            (true, false, false, K_RETURN) => Some(Act::AddSlide),
            (true, false, false, 0x64) | (true, false, false, 0x44) => Some(Act::Duplicate),
            (true, true, false, 0x64) | (true, true, false, 0x44) => Some(Act::SameLayout),
            (true, true, false, 0x63) | (true, true, false, 0x43) => Some(Act::CopySlides),
            (true, true, false, 0x76) | (true, true, false, 0x56) => Some(Act::PasteSlides),
            (true, false, false, 0x7a) | (true, false, false, 0x5a) => Some(Act::Undo),
            (true, true, false, 0x7a) | (true, true, false, 0x5a) => Some(Act::Redo),
            (true, false, false, 0x6b) | (true, false, false, 0x4b) => Some(Act::Palette),
            (true, true, false, 0x65) | (true, true, false, 0x45) => Some(Act::ExportPdf),
            (true, false, true, 0x65) | (true, false, true, 0x45) => Some(Act::ExportPng),
            (false, false, false, K_F5) => Some(Act::Present),
            (false, true, false, K_F5) => Some(Act::PresentStart),
            (false, false, false, K_TAB) => Some(Act::Focus),
            (false, false, true, K_UP) | (true, false, false, K_UP) => Some(Act::MoveUp),
            (false, false, true, K_DOWN) | (true, false, false, K_DOWN) => Some(Act::MoveDown),
            (false, false, false, K_DELETE) => Some(Act::Delete),
            _ => None,
        };
        if let Some(a) = a {
            return self.act(x, a, None);
        }
        // The slide list: arrows move the current slide.
        if !ctrl && !alt && self.focus.is_none() {
            let n = self.slides().len();
            let target = match sym {
                K_UP => Some(self.cur.saturating_sub(1)),
                K_DOWN => Some((self.cur + 1).min(n.saturating_sub(1))),
                K_PRIOR => Some(self.cur.saturating_sub(5)),
                K_NEXT => Some((self.cur + 5).min(n.saturating_sub(1))),
                K_HOME => Some(0),
                K_END => Some(n.saturating_sub(1)),
                _ => None,
            };
            if let Some(t) = target {
                if shift {
                    let (a, b) = (self.sel_anchor.min(t), self.sel_anchor.max(t));
                    self.sel = (a..=b).collect();
                } else {
                    self.sel.clear();
                    self.sel.insert(t);
                    self.sel_anchor = t;
                }
                self.select(t);
            }
        }
        Request::None
    }

    pub fn edit_key(&mut self, x: &mut X, id: Fid, sym: u32, state: u16, ch: Option<char>) -> EditResult {
        let face = self.f.sans.clone();
        let px = self.c.font_px;
        let width = self.field_rects.get(&id).map(|r| (r.w - 16) as f32).unwrap_or(300.0);
        let text = self.edit_mut(id).text.clone();
        let lay = ui::area_layout(&face, px, &text, width);
        let vmove = |cur: usize, d: i32, goal: Option<f32>| -> (usize, f32) {
            let l = lay.line_of(cur) as i32;
            let gx = goal.unwrap_or_else(|| lay.x_of(cur));
            let nl = (l + d).clamp(0, lay.lines.len() as i32 - 1) as usize;
            (lay.index_at(nl, gx), gx)
        };
        let r = self.edit_mut(id).key(sym, state, ch, &vmove);
        match r {
            EditResult::Changed => self.field_changed(id),
            EditResult::Copy | EditResult::Cut => {
                let s = self.edit_mut(id).selected();
                if !s.is_empty() {
                    x.set_clipboard(self.win, &s);
                    if r == EditResult::Cut && self.edit_mut(id).delete_sel() {
                        self.field_changed(id);
                    }
                }
            }
            EditResult::Paste => {
                if let Some(t) = x.paste(self.win) {
                    self.edit_mut(id).insert(&t);
                    self.field_changed(id);
                }
            }
            EditResult::Submit => {
                if let Fid::D(_) = id {
                    dialogs::submit(self, x, id);
                } else {
                    self.blur();
                }
            }
            _ => {}
        }
        self.dirty_draw = true;
        r
    }

    fn pop_act(&mut self, x: &mut X, a: PopAct) -> Request {
        self.popup = None;
        self.popup_owner = None;
        self.dirty_draw = true;
        match a {
            PopAct::Act(a) => self.act(x, a, None),
            PopAct::Combo(id, i) => {
                let r = self.combo_chosen(x, id, i);
                if !matches!(id, Cid::D(_)) {
                    self.push_history();
                }
                r
            }
        }
    }

    // ---------------------------------------------------------- actions

    pub fn act(&mut self, x: &mut X, a: Act, anchor: Option<Rect>) -> Request {
        self.dirty_draw = true;
        match a {
            Act::ExportMenu | Act::TalkMenu | Act::PresetsMenu | Act::Overflow => {
                let anchor = anchor.unwrap_or_else(|| self.hit_rect(Hit::Act(a)));
                let items = self.menu_items(a);
                self.open_popup(Hit::Act(a), anchor, items, None);
            }
            Act::New => return self.confirm_then(x, Then::New),
            Act::Open => return self.confirm_then(x, Then::Open),
            Act::Save => {
                self.save(x, None);
            }
            Act::SaveAs => dialogs::file_dialog(self, dialogs::FilePurpose::SaveAs(None)),
            Act::ExportPng => {
                let name = format!("slide-{:02}.png", self.cur + 1);
                dialogs::file_dialog(self, dialogs::FilePurpose::ExportPng(name));
            }
            Act::ExportAllPng => dialogs::file_dialog(self, dialogs::FilePurpose::ExportAllPng),
            Act::ExportPdf => dialogs::file_dialog(self, dialogs::FilePurpose::ExportPdf),
            Act::ExportPptx => dialogs::file_dialog(self, dialogs::FilePurpose::ExportPptx),
            Act::ExportMd => dialogs::file_dialog(self, dialogs::FilePurpose::ExportMd),
            Act::ExportAiBrief => dialogs::file_dialog(self, dialogs::FilePurpose::ExportAiBrief),
            Act::ExportAiTemplate => dialogs::file_dialog(self, dialogs::FilePurpose::ExportAiTemplate),
            Act::CopyAiPrompt => {
                let t = crate::markdown::ai_brief(&self.data);
                x.set_clipboard(self.win, &t);
                self.set_status("AI prompt + current deck copied to clipboard", 3000);
            }
            Act::ImportMd => return self.confirm_then(x, Then::Import),
            Act::AddSlide => dialogs::open(self, Kind::LayoutPicker),
            Act::Duplicate => {
                if let Some(s) = self.current().cloned() {
                    let i = self.cur;
                    let d = slide_defaults(Some(&s));
                    self.slides_mut().insert(i + 1, Value::Object(d));
                    self.thumbs.clear();
                    self.refresh(i + 1);
                    self.mark_dirty();
                    self.push_history();
                }
            }
            Act::Delete => {
                let rows = self.selected_rows();
                if self.slides().len() - rows.len() < 1 {
                    return Request::None;
                }
                let target = *rows.iter().min().unwrap_or(&0);
                for i in rows.iter().rev() {
                    self.slides_mut().remove(*i);
                }
                self.thumbs.clear();
                let n = self.slides().len();
                self.refresh(target.min(n - 1));
                self.mark_dirty();
                self.push_history();
            }
            Act::Undo => self.undo(),
            Act::Redo => self.redo(),
            Act::Focus => {
                let entering = self.left_visible || self.right_visible;
                self.left_visible = !entering;
                self.right_visible = !entering;
                self.toolbar_visible = !entering;
            }
            Act::ChromeToggle => {
                let f = !self.chrome_follow;
                self.set_chrome_follow(x, f);
            }
            Act::ChromeFollow(f) => self.set_chrome_follow(x, f),
            Act::Palette => dialogs::open(self, Kind::Palette),
            Act::About => dialogs::open_about(self, x),
            Act::Present => return self.present(x, false, None, None, false, false, None),
            Act::PresentStart => return self.present(x, true, None, None, false, false, None),
            Act::PresenterView => return self.presenter_view(x),
            Act::Talk(i) => {
                let p = TALK_PROFILES.get(i as usize).map(|p| p.1).unwrap_or("screen-mic");
                return self.start_talk(p);
            }
            Act::TalkHelp => dialogs::message(
                self,
                "NIRUPRES Talk mode",
                "Talk mode presents the deck from the beginning and records it with NIRUCLIP.\n\nNIRUCLIP owns the capture process and state. NIRUPRES writes slide chapters as .chapters.json and .srt beside the recording.\n\nProfiles: screen-mic, screen-camera and camera-mic. Edit them with niruclip --save-profile and choose the closest profile here.",
            ),
            Act::Online => return self.online_present(x, false),
            Act::OnlineStart => return self.online_present(x, true),
            Act::Overview => dialogs::open(self, Kind::Overview),
            Act::CopySlides => {
                let rows = self.selected_rows();
                self.slide_clip = rows
                    .iter()
                    .filter_map(|&i| self.slides().get(i).and_then(|v| v.as_object()).cloned())
                    .collect();
                let n = self.slide_clip.len();
                self.set_status(&format!("Copied {n} slide(s)"), 1800);
            }
            Act::PasteSlides => {
                if self.slide_clip.is_empty() {
                    return Request::None;
                }
                let at = self.cur + 1;
                let clips = self.slide_clip.clone();
                for (n, s) in clips.iter().enumerate() {
                    let mut d = slide_defaults(Some(s));
                    d.insert("id".into(), json!(uuid4()));
                    self.slides_mut().insert(at + n, Value::Object(d));
                }
                self.thumbs.clear();
                self.refresh(at);
                self.mark_dirty();
                self.push_history();
            }
            Act::SameLayout => {
                let cur = self.current().cloned();
                let i = self.cur + 1;
                let layout = cur.as_ref().map(|c| s_get(c, "layout", "STATEMENT")).unwrap_or_else(|| "STATEMENT".into());
                let th = cur.as_ref().map(|c| s_get(c, "themeOverride", "DECK")).unwrap_or_else(|| "DECK".into());
                let d = slide_defaults(json!({"layout": layout, "themeOverride": th}).as_object());
                self.slides_mut().insert(i, Value::Object(d));
                self.thumbs.clear();
                self.refresh(i);
                self.mark_dirty();
                self.push_history();
            }
            Act::MoveUp => self.move_slide_keyboard(-1),
            Act::MoveDown => self.move_slide_keyboard(1),
            Act::DeckSettings | Act::DocTitle => {
                if anchor.is_some() || a == Act::DeckSettings {
                    dialogs::open(self, Kind::DeckSettings);
                }
            }
            Act::FileWorkflow => dialogs::open(self, Kind::FileWorkflow),
            Act::DeckHealth => {
                let issues = self.validate_deck();
                dialogs::open(self, Kind::Health(issues));
            }
            Act::AssetManager => dialogs::open_assets(self),
            Act::Profile(i) => {
                if let Some(p) = prefs::PROFILES.get(i as usize) {
                    self.set_profile(p.key);
                }
            }
            Act::PresentationCheck => self.presentation_check(),
            Act::ApplyLogo(i) => self.apply_to_selected("logoMode", LOGO_MODES[i as usize % 3]),
            Act::ApplyAccent(i) => self.apply_to_selected("accentOverride", ACCENTS[i as usize % 7]),
            Act::ToggleOutline => {
                self.outline = !self.outline;
                let c = self.cur;
                self.refresh(c);
            }
            Act::HideLeft => self.left_visible = false,
            Act::HideRight => self.right_visible = false,
            Act::ToggleLeft => self.left_visible = !self.left_visible,
            Act::ToggleRight => self.right_visible = !self.right_visible,
            Act::Preset(k) => self.add_preset(k),
            Act::ChooseImage => dialogs::file_dialog(self, dialogs::FilePurpose::Image(Fid::Img)),
            Act::RemoveImage => {
                self.set_field(Fid::Img, "");
                self.field_changed(Fid::Img);
                self.push_history();
            }
            Act::ChooseBg => dialogs::file_dialog(self, dialogs::FilePurpose::Image(Fid::BgImg)),
            Act::RemoveBg => {
                self.set_field(Fid::BgImg, "");
                self.field_changed(Fid::BgImg);
                self.push_history();
            }
            Act::ChoosePoster => dialogs::file_dialog(self, dialogs::FilePurpose::Image(Fid::VideoPoster)),
            Act::ChooseVideo => dialogs::file_dialog(self, dialogs::FilePurpose::Video),
            Act::ApplyStyle => self.apply_style_to_selected(),
            Act::PreviewTransition => return self.preview_transition(),
        }
        Request::None
    }

    fn set_profile(&mut self, name: &str) {
        if let Err(e) = prefs::set_profile(&prefs::data_root(), name) {
            self.set_status(&format!("Profile could not be saved: {e}"), 4000);
        }
        self.profile = name.to_string();
        let label = self.profile_info().label;
        self.set_status(&format!("Profile: {label} · applies to new decks; open decks keep their logo setting"), 4000);
    }

    pub fn set_chrome_follow(&mut self, x: &X, follow: bool) {
        let follow = follow && self.chrome_available;
        self.chrome_follow = follow;
        chrome::write_follow(follow);
        self.apply_chrome(x);
    }

    pub fn apply_chrome(&mut self, x: &X) {
        self.palette = if self.chrome_follow {
            chrome::load_nirux(x.root_theme().as_deref())
        } else {
            None
        };
        self.c = self.palette.clone().unwrap_or_else(Chrome::noir);
        let s = chrome::status_text(self.palette.as_ref(), self.chrome_follow);
        self.set_status(&s, 2200);
        self.dirty_draw = true;
    }

    pub fn refresh_chrome(&mut self, x: &X) {
        self.chrome_available = chrome::detect();
        self.chrome_follow = self.chrome_follow && self.chrome_available;
        self.apply_chrome(x);
    }

    fn move_slide_keyboard(&mut self, d: i32) {
        let i = self.cur as i32;
        let j = i + d;
        if i < 0 || j < 0 || j as usize >= self.slides().len() {
            return;
        }
        self.slides_mut().swap(i as usize, j as usize);
        self.thumbs.clear();
        self.refresh(j as usize);
        self.mark_dirty();
        self.push_history();
        let t = self.slide_accessible_text(j as usize);
        self.set_status(&format!("Moved to {t}"), 2500);
    }

    fn drag_reorder(&mut self, old: usize, new: usize) {
        if old == new {
            return;
        }
        let s = self.slides_mut().remove(old);
        self.slides_mut().insert(new, s);
        self.thumbs.clear();
        self.refresh(new);
        self.mark_dirty();
        self.push_history();
    }

    pub fn add_layout(&mut self, layout: &str) {
        let (t, b): (&str, &str) = match layout {
            "TITLE" => ("Title", "Subtitle"),
            "STATEMENT" => ("New statement", "Short supporting line"),
            "TEXT" => ("Heading", "Concise narrative text"),
            "BULLETS" => ("Key points", "First point\nSecond point\nThird point"),
            "AGENDA" => ("Agenda", "Opening\nContext\nDecision\nNext steps"),
            "IMAGE" => ("", ""),
            "PHOTO" => ("Photo title", "Short caption"),
            "HERO IMAGE" => ("A visual story", "Short supporting line"),
            "SPLIT" => ("Heading", "Supporting text"),
            "TWO COLUMN" => ("Heading", "Left column\n\n|||\n\nRight column"),
            "COMPARE" => ("Compare", "Current state\nKey limitation\n\n|||\n\nDesired state\nKey benefit"),
            "IMAGE + QUOTE" => ("A strong quote", "Attribution"),
            "QUOTE" => ("Quote", "Attribution"),
            "BIG NUMBER" => ("80%", "Key result or context"),
            "DATA / KPI" => ("Results", "80% | Faster\n12 | Systems\n47 | Risks\n2027 | Production"),
            "NUMBER GRID" => ("Key figures", "80% | Faster\n12 | Systems\n47 | Risks\n2027 | Production"),
            "PROCESS" => ("Process", "ANALYZE | Understand the need\nPILOT | Test safely\nDELIVER | Put into use\nFOLLOW UP | Measure and improve"),
            "TIMELINE" => ("Timeline", "SEP | Pilot\nOCT | Test\nNOV | Release\nJAN | Production"),
            "MATRIX" => ("Framework", "PEOPLE | Roles and capability\nPROCESS | Clear ways of working\nTECHNOLOGY | Fit-for-purpose tools\nGOVERNANCE | Ownership and control"),
            "TABLE" => ("Overview", "AREA | STATUS | OWNER\nSecurity | Ready | IT\nTraining | In progress | HR\nLaunch | Planned | Team"),
            "SECTION" => ("Section", "Short context"),
            "FULL BLEED" => ("Title", "Subtitle"),
            "VIDEO" => ("Video", "Optional caption"),
            "VIDEO + TEXT" => ("Video + context", "Key context or talking points"),
            "END" => ("Thank you", "Questions / contact"),
            "CODE" => ("Example", "def greet(name):\n    return f\"Hello, {name}\"\n\nprint(greet(\"NIRUPRES\"))"),
            "IMAGE + CAPTION" => ("Figure title", "What the picture shows and why it matters"),
            "THREE COLUMN" => ("Three options", "**Option A**\nFast to start\n\n|||\n\n**Option B**\nBalanced\n\n|||\n\n**Option C**\nMost complete"),
            _ => ("", ""),
        };
        let i = self.cur + 1;
        let d = slide_defaults(json!({"layout": layout, "title": t, "body": b}).as_object());
        self.slides_mut().insert(i, Value::Object(d));
        self.thumbs.clear();
        self.refresh(i);
        self.mark_dirty();
        self.push_history();
    }

    fn add_preset(&mut self, k: u8) {
        let (layout, title, body) = match k {
            0 => ("TITLE", "Presentation title", "Short subtitle"),
            1 | 4 => ("BULLETS", "Agenda", "First point\nSecond point\nThird point"),
            2 => ("SECTION", "01 · Section", "Short context"),
            3 => ("STATEMENT", "One clear message", "Short supporting sentence"),
            5 => ("SPLIT", "Image + message", "Supporting text"),
            6 => ("BIG NUMBER", "80%", "Key result or KPI"),
            _ => ("END", "Thank you", "Questions / contact"),
        };
        let i = self.cur + 1;
        let d = slide_defaults(json!({"layout": layout, "title": title, "body": body}).as_object());
        self.slides_mut().insert(i, Value::Object(d));
        self.thumbs.clear();
        self.refresh(i);
        self.mark_dirty();
        self.push_history();
    }

    fn apply_to_selected(&mut self, field: &str, value: &str) {
        let rows = self.selected_rows();
        let mut changed = 0;
        for &i in &rows {
            if let Some(Value::Object(s)) = self.slides_mut().get_mut(i) {
                s.insert(field.into(), json!(value));
                changed += 1;
            }
        }
        if changed > 0 {
            self.thumbs.clear();
            self.refresh(rows[0]);
            self.mark_dirty();
            self.push_history();
            self.set_status(&format!("Applied to {changed} selected slide(s)"), 1800);
        }
    }

    fn apply_style_to_selected(&mut self) {
        let Some(src) = self.current().cloned() else { return };
        let rows: Vec<usize> = self.sel.iter().copied().collect();
        if rows.len() < 2 {
            self.set_status("Select two or more slides to apply style", 2500);
            return;
        }
        let keys = [
            "themeOverride", "logoMode", "accentOverride", "fontScale", "align", "weight", "fontFamily",
            "brightness", "contrast", "overlay", "blur", "imageMode", "imageTreatment", "mono", "focalX",
            "focalY", "listStyle", "backgroundImage", "backgroundMono", "backgroundDim", "backgroundBlur",
            "backgroundFocalX", "backgroundFocalY", "transition",
        ];
        for &i in &rows {
            if let Some(Value::Object(d)) = self.slides_mut().get_mut(i) {
                for k in keys {
                    d.insert(k.into(), src.get(k).cloned().unwrap_or(Value::Null));
                }
            }
        }
        self.thumbs.clear();
        let c = self.cur;
        self.refresh(c);
        self.mark_dirty();
        self.push_history();
        self.set_status(&format!("Applied style to {} selected slides", rows.len()), 2500);
    }

    fn preview_transition(&mut self) -> Request {
        let cur = self.cur;
        let mut prev = cur as i64 - 1;
        while prev >= 0 && self.slides().get(prev as usize).and_then(|v| v.as_object()).is_some_and(|s| b_get(s, "hidden")) {
            prev -= 1;
        }
        if prev < 0 {
            self.set_status("Transition preview needs a previous visible slide", 2500);
            return Request::None;
        }
        Request::TransitionPreview(prev as usize)
    }

    /// Deck health: (slide, kind, message).
    pub fn validate_deck(&self) -> Vec<(usize, String, String)> {
        let mut issues = Vec::new();
        for (i, s) in self.slides().iter().enumerate() {
            let Some(s) = s.as_object() else { continue };
            let layout = s_get(s, "layout", "TITLE");
            let mut push = |k: &str, m: String| issues.push((i, k.to_string(), m));
            let title = s_get(s, "title", "");
            let words = format!("{title} {}", s_get(s, "body", "")).split_whitespace().count();
            if !LAYOUTS.contains(&layout.as_str()) {
                push("ERROR", format!("Unknown layout: {layout}"));
            }
            if !(truthy(s.get("title")) || truthy(s.get("body")) || truthy(s.get("image")) || truthy(s.get("imageRequest"))) {
                push("WARN", "Empty slide".into());
            }
            let img = s_get(s, "image", "");
            if !img.is_empty() && !Path::new(&img).exists() {
                push("ERROR", format!("Missing image: {img}"));
            }
            if truthy(s.get("imageRequest")) && img.is_empty() {
                push("TODO", format!("Unresolved image request: {}", s_get(s, "imageRequest", "")));
            }
            for problem in render::slide_fit_issues(s, &self.data, i) {
                let mut c = problem.chars();
                let p = match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                };
                push("WARN", p);
            }
            if ["IMAGE", "PHOTO", "HERO IMAGE", "IMAGE + QUOTE", "IMAGE + CAPTION", "FULL BLEED"].contains(&layout.as_str())
                && !(truthy(s.get("image")) || truthy(s.get("imageRequest")))
            {
                push("WARN", format!("{layout} layout has no image"));
            }
            if layout == "FULL BLEED" && !img.is_empty() && i_get(s, "overlay", 0) < 12 && (truthy(s.get("title")) || truthy(s.get("body"))) {
                push("WARN", "FULL BLEED text may need more overlay for image contrast".into());
            }
            let limit = if ["TITLE", "STATEMENT", "PHOTO", "HERO IMAGE", "IMAGE + QUOTE", "BIG NUMBER", "DATA / KPI"].contains(&layout.as_str()) {
                70
            } else {
                120
            };
            if words > limit {
                push("WARN", format!("Text-heavy slide: {words} words"));
            }
            if title.chars().count() > 120 {
                push("WARN", "Very long title; may reach minimum text size".into());
            }
            let ov = s_get(s, "themeOverride", "DECK");
            let deck_theme = s_get(&self.data, "theme", "B&W");
            let eff = normalize_theme(if ov != "DECK" { &ov } else { &deck_theme });
            let udd = theme(&eff).is_some_and(|t| t.uddevalla);
            if udd && !Deck(&self.data).show_logo() && s_get(s, "logoMode", "AUTO") == "ON" {
                push("WARN", "Slide requests logo ON but deck LOGO master switch is OFF".into());
            }
            if s_get(s, "accentOverride", "AUTO") != "AUTO" && !udd {
                push("WARN", "Uddevalla accent override is set on a non-Uddevalla theme".into());
            }
            if layout == "FULL BLEED" && s_get(s, "logoMode", "AUTO") == "ON" && img.is_empty() {
                push("WARN", "FULL BLEED logo is ON but no background image is set".into());
            }
        }
        issues
    }

    fn presentation_check(&mut self) {
        let issues = self.validate_deck();
        if issues.is_empty() {
            dialogs::message(self, "Presentation Check", "Ready to present.\n\nNo deck-health issues were found.");
            return;
        }
        let cnt = |k: &str| issues.iter().filter(|x| x.1 == k).count();
        let blocking = cnt("ERROR") + cnt("TODO") > 0;
        let mut msg = format!(
            "{} note(s): {} error(s), {} warning(s), {} unresolved image request(s).",
            issues.len(),
            cnt("ERROR"),
            cnt("WARN"),
            cnt("TODO")
        );
        if blocking {
            msg.push_str("\n\nThe presentation can still run, but audience output may be incomplete.");
        }
        msg.push_str("\n\nOpen Deck Health?");
        dialogs::ask(self, "Presentation Check", &msg, vec![("Yes", 1), ("No", 0)], Then::Health);
    }

    // --------------------------------------------------- files and saving

    pub fn dialog_start(&self, filename: &str, prefer_current: bool) -> PathBuf {
        let folder = self.file_prefs.preferred(self.path.as_deref(), prefer_current);
        if filename.is_empty() {
            folder
        } else {
            folder.join(filename)
        }
    }

    pub fn remember_dialog(&mut self, p: &Path) {
        self.file_prefs.remember(p, "");
    }

    /// Ask to save unsaved changes, then continue with `then`.
    pub fn confirm_then(&mut self, x: &mut X, then: Then) -> Request {
        if !self.dirty {
            return dialogs::run_then(self, x, then);
        }
        dialogs::open(self, Kind::Confirm(then));
        Request::None
    }

    pub fn request_close(&mut self, x: &mut X) -> Request {
        if self.presenting {
            return Request::None;
        }
        self.confirm_then(x, Then::Quit)
    }

    pub fn update_title(&self, x: &X) {
        let display = match &self.path {
            Some(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            None => {
                let t = s_get(&self.data, "title", "Untitled");
                if t.is_empty() {
                    "Untitled".into()
                } else {
                    t
                }
            }
        };
        let _ = x.set_title(self.win, &format!("{display}{} — {APP} {VERSION}", if self.dirty { " *" } else { "" }));
    }

    /// Save to the current path (or ask for one); `then` runs afterwards.
    pub fn save(&mut self, x: &mut X, then: Option<Then>) -> bool {
        let Some(path) = self.path.clone() else {
            dialogs::file_dialog(self, dialogs::FilePurpose::SaveAs(then));
            return false;
        };
        let quality = self.file_prefs.asset_quality.clone();
        match crate::package::save_project(&self.data, &path, &quality, Some(self.extract.path())) {
            Ok(stats) => {
                prefs::add_recent(&path);
                self.recents = prefs::recents();
                self.dirty = false;
                self.save_state = format!("Saved {}", clock_now());
                self.clear_autosave();
                self.update_title(x);
                self.set_status(
                    &format!(
                        "Saved portable project · {} image(s) · saved {:.1} MB: {}",
                        stats.images,
                        stats.saved_bytes as f64 / MB as f64,
                        path.display()
                    ),
                    3200,
                );
                if let Some(t) = then {
                    let r = dialogs::run_then(self, x, t);
                    if r == Request::Quit {
                        self.quit_after_present = true;
                    }
                }
                true
            }
            Err(e) => {
                dialogs::message(
                    self,
                    "Save failed",
                    &format!("NIRUPRES could not save this presentation.\n\nYour previous file has not been replaced.\n\n{e}"),
                );
                self.mark_dirty();
                false
            }
        }
    }

    pub fn open_path_checked(&mut self, x: &mut X, p: PathBuf) -> Request {
        self.confirm_then(x, Then::OpenPath(p))
    }

    /// Open a deck (no unsaved-changes check).
    pub fn open_path(&mut self, x: &X, p: &Path) -> Result<(), String> {
        let dir = crate::tempdir::TempDir::new("nirupres-")?;
        let data = crate::package::load_project(p, dir.path())?;
        crate::images::clear_cache();
        self.extract = dir;
        self.data = data;
        self.path = Some(p.to_path_buf());
        self.dirty = false;
        self.save_state.clear();
        self.history.clear();
        self.history_pos = -1;
        self.thumbs.clear();
        self.list_scroll = 0;
        self.refresh(0);
        self.push_history();
        self.clear_autosave();
        prefs::add_recent(p);
        self.recents = prefs::recents();
        self.remember_dialog(p);
        self.update_title(x);
        Ok(())
    }

    pub fn new_deck(&mut self, x: &X, theme_name: &str) {
        let mut d = default_deck();
        normalize_loaded(&mut d, "Untitled");
        d.insert("showLogo".into(), json!(self.profile_info().show_logo_default));
        d.insert("theme".into(), json!(theme_name));
        self.data = d;
        self.path = None;
        self.dirty = false;
        self.save_state.clear();
        self.history.clear();
        self.history_pos = -1;
        self.thumbs.clear();
        self.refresh(0);
        self.push_history();
        self.update_title(x);
    }

    pub fn import_markdown(&mut self, x: &X, p: &Path) {
        let text = match std::fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) => {
                dialogs::message(self, "Markdown import failed", &e.to_string());
                return;
            }
        };
        match crate::markdown::import_report(&text, p.parent()) {
            Ok((data, report)) => {
                self.data = data;
                self.path = None;
                self.dirty = true;
                self.thumbs.clear();
                self.refresh(0);
                self.history.clear();
                self.history_pos = -1;
                self.push_history();
                self.update_title(x);
                let n = self.slides().len();
                self.set_status(&format!("Imported {n} slides · {} normalization(s)", report.len()), 4000);
                let issues = self.validate_deck();
                if !issues.is_empty() {
                    self.set_status(&format!("{} deck health note(s) · Ctrl+K → Deck health", issues.len()), 4500);
                }
                if !report.is_empty() {
                    let lines: Vec<String> = report.iter().take(12).map(|r| format!("• {r}")).collect();
                    dialogs::message(self, "Markdown import", &format!("Imported {n} slides.\n\nNormalized:\n{}", lines.join("\n")));
                }
            }
            Err(e) => dialogs::message(self, "Markdown import failed", &e),
        }
    }

    // ---------------------------------------------------------- autosave

    pub fn autosave(&mut self) {
        if !self.dirty {
            return;
        }
        let payload = json!({
            "source": self.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            "data": Value::Object(self.data.clone()),
        });
        let path = prefs::file("autosave.json");
        let tmp = path.with_extension("tmp");
        let r = std::fs::create_dir_all(prefs::data_root())
            .and_then(|_| std::fs::write(&tmp, crate::package::py_json_utf8(&payload)))
            .and_then(|_| std::fs::rename(&tmp, &path));
        match r {
            Ok(_) => self.save_state = format!("● Autosaved {}", clock_now()),
            Err(e) => self.set_status(&format!("Autosave failed: {e}"), 4000),
        }
        self.dirty_draw = true;
    }

    pub fn clear_autosave(&self) {
        let _ = std::fs::remove_file(prefs::file("autosave.json"));
    }

    /// Unsaved work found at start: offer Restore / Discard.
    pub fn offer_recovery(&mut self) {
        let path = prefs::file("autosave.json");
        if !path.exists() {
            return;
        }
        let Some(v) = prefs::read_json(&path) else {
            self.clear_autosave();
            return;
        };
        let when = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| clock_at(d.as_secs() as i64))
            .unwrap_or_default();
        let source = v
            .get("source")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| Path::new(s).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
            .unwrap_or_else(|| "an untitled presentation".into());
        if !v.get("data").is_some_and(|d| d.is_object()) {
            self.clear_autosave();
            return;
        }
        let info = format!("Unsaved NIRUPRES work from {when} was found ({source}).\n\nRestore it, or discard the autosave?");
        dialogs::open(self, Kind::Recovery { info, when });
    }

    pub fn restore_autosave(&mut self, x: &X, when: &str) {
        let Some(v) = prefs::read_json(&prefs::file("autosave.json")) else {
            return;
        };
        let Some(Value::Object(mut data)) = v.get("data").cloned() else {
            self.clear_autosave();
            return;
        };
        normalize_loaded(&mut data, "Untitled");
        self.data = data;
        self.path = v
            .get("source")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        self.dirty = true;
        self.thumbs.clear();
        self.refresh(0);
        self.history.clear();
        self.history_pos = -1;
        self.push_history();
        self.update_title(x);
        self.save_state = "● Recovered from autosave".into();
        self.set_status(&format!("Recovered from autosave ({when}) · save to keep it"), 5000);
    }

    // ------------------------------------------------------- presenting

    pub fn start_talk(&mut self, profile: &str) -> Request {
        if self.presenting {
            self.set_status("End the current presentation before starting Talk mode", 2500);
            return Request::None;
        }
        if !self.slides().iter().any(|s| !s.as_object().is_some_and(|s| b_get(s, "hidden"))) {
            dialogs::message(self, "Talk mode", "Add at least one visible slide before recording.");
            return Request::None;
        }
        self.set_status(&format!("Starting NIRUCLIP profile {profile}…"), 15000);
        Request::Talk(profile.to_string())
    }

    pub fn editor_monitor(&self, x: &X, mons: &[x::Monitor]) -> usize {
        let pos = x.root_pos(self.win).unwrap_or((0, 0));
        let (cx, cy) = (pos.0 + self.cv.w / 2, pos.1 + self.cv.h / 2);
        mons.iter()
            .position(|m| cx >= m.x && cy >= m.y && cx < m.x + m.w && cy < m.y + m.h)
            .or_else(|| mons.iter().position(|m| m.primary))
            .unwrap_or(0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(&mut self, x: &mut X, from_start: bool, presenter: Option<usize>, audience: Option<usize>, online: bool, talk: bool, single: Option<&str>) -> Request {
        let slides = self.slides();
        if slides.is_empty() {
            return Request::None;
        }
        if !slides.iter().any(|s| !s.as_object().is_some_and(|s| b_get(s, "hidden"))) {
            dialogs::message(self, "Cannot present", "All slides are hidden. Unhide at least one slide first.");
            return Request::None;
        }
        let start = if from_start { 0 } else { self.cur };
        let mons = x.monitors();
        let mut online = online;
        if mons.len() < 2 && !online && !talk {
            let choice = match single {
                Some(c) => Some(c.to_string()),
                None => prefs::display_prefs()
                    .get("singleScreen")
                    .and_then(|v| v.as_str())
                    .filter(|v| *v == "laptop" || *v == "slides")
                    .map(str::to_string),
            };
            match choice.as_deref() {
                None => {
                    dialogs::open(self, Kind::OneScreen { ask: false, from_start });
                    return Request::None;
                }
                Some("laptop") => online = true,
                _ => {}
            }
        }
        let here = self.editor_monitor(x, &mons);
        let presenter = presenter.unwrap_or(here);
        let audience = match audience {
            Some(a) => a,
            None => {
                let pref = prefs::display_prefs().get("audience").and_then(|v| v.as_str()).map(str::to_string);
                pref.and_then(|n| mons.iter().position(|m| m.name == n))
                    .filter(|&a| a != presenter || mons.len() < 2)
                    .or_else(|| (0..mons.len()).find(|&i| i != presenter))
                    .unwrap_or(presenter)
            }
        };
        let audience = if online { presenter } else { audience };
        Request::Present(PresentReq {
            start,
            presenter: Some(presenter),
            audience: Some(audience),
            online,
            talk,
            share_check: online && !talk,
        })
    }

    pub fn online_present(&mut self, x: &mut X, from_start: bool) -> Request {
        let mons = x.monitors();
        let here = self.editor_monitor(x, &mons);
        self.present(x, from_start, Some(here), Some(here), true, false, None)
    }

    fn presenter_view(&mut self, x: &mut X) -> Request {
        let mons = x.monitors();
        if mons.len() < 2 {
            dialogs::open(self, Kind::OneScreen { ask: true, from_start: false });
            return Request::None;
        }
        dialogs::open_display_setup(self, mons);
        Request::None
    }

    // ------------------------------------------------------------ timers

    /// Run due timers; returns the next deadline.
    pub fn timers(&mut self) -> Option<Instant> {
        let t = now();
        let mut next: Option<Instant> = None;
        let mut upd = |d: Instant| {
            next = Some(next.map_or(d, |n: Instant| n.min(d)));
        };
        if let Some(a) = self.history_at {
            if t >= a {
                self.history_at = None;
                self.push_history();
            } else {
                upd(a);
            }
        }
        if let Some(a) = self.autosave_at {
            if t >= a {
                self.autosave_at = None;
                self.autosave();
            } else {
                upd(a);
            }
        }
        if let Some((_, at, ms)) = &self.status {
            let due = *at + Duration::from_millis(*ms);
            if t >= due {
                self.status = None;
                self.dirty_draw = true;
            } else {
                upd(due);
            }
        }
        if let Some(d) = self.tick_tooltip() {
            upd(d);
        }
        next
    }
}

