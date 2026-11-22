//! Modal dialogs of the editor, drawn over it: messages, the layout
//! picker, deck settings, file workflow, the text editor, the command
//! palette, about, new presentation, assets, deck health, overview,
//! display setup, one-screen choice, recovery and the file chooser.
use crate::canvas::{Canvas, Rect};
use crate::chrome::blend;
use crate::editor::{Editor, PresentReq, Request};
use crate::ids::*;
use crate::model::*;
use crate::prefs;
use crate::ui::{self, BtnKind, Paint};
use crate::x::{self, X};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    New,
    Open,
    OpenPath(PathBuf),
    Import,
    Quit,
    Health,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilePurpose {
    Open,
    SaveAs(Option<Then>),
    ExportPng(String),
    ExportAllPng,
    ExportPdf,
    ExportPptx,
    ExportMd,
    ExportAiBrief,
    ExportAiTemplate,
    ImportMd,
    Image(Fid),
    Video,
    DefaultFolder,
    ReplaceAsset(String),
}

#[derive(Clone, Debug)]
pub struct FileState {
    pub purpose: FilePurpose,
    pub dir: PathBuf,
    pub entries: Vec<(String, bool)>,
    pub exts: Vec<&'static str>,
    pub save: bool,
    pub folder: bool,
    pub filter_name: String,
    pub overwrite: Option<PathBuf>,
    pub hidden: bool,
}

#[derive(Clone, Debug)]
pub enum Kind {
    Message,
    Ask(Then),
    Confirm(Then),
    LayoutPicker,
    DeckSettings,
    FileWorkflow,
    TextEditor { two: bool, hint: String },
    Palette,
    About(String),
    New,
    Assets(Vec<String>),
    Health(Vec<(usize, String, String)>),
    Overview,
    DisplaySetup(Vec<x::Monitor>),
    OneScreen { ask: bool, from_start: bool },
    Recovery { info: String, when: String },
    File(FileState),
}

pub struct Dialog {
    pub kind: Kind,
    pub title: String,
    pub text: String,
    pub buttons: Vec<(String, u16)>,
    pub width: i32,
    combos: HashMap<u8, (Vec<String>, usize)>,
    combo_off: Vec<u8>,
    checks: HashMap<u8, bool>,
    check_off: Vec<u8>,
    sliders: HashMap<u8, (i32, i32, i32)>,
    pub items: Vec<String>,
    pub item_codes: Vec<usize>,
    pub sel: usize,
    pub scroll: i32,
    list_rect: Rect,
    list_row_h: i32,
    grid_room: i32,
    parent: Option<Box<Dialog>>,
    pub fields: Vec<u8>,
}

impl Dialog {
    fn new(kind: Kind, title: &str, width: i32) -> Self {
        Self {
            kind,
            title: title.to_string(),
            text: String::new(),
            buttons: Vec::new(),
            width,
            combos: HashMap::new(),
            combo_off: Vec::new(),
            checks: HashMap::new(),
            check_off: Vec::new(),
            sliders: HashMap::new(),
            items: Vec::new(),
            item_codes: Vec::new(),
            sel: 0,
            scroll: 0,
            list_rect: Rect::default(),
            list_row_h: 28,
            grid_room: 0,
            parent: None,
            fields: Vec::new(),
        }
    }

    pub fn combo_items(&self, id: Cid) -> Vec<String> {
        match id {
            Cid::D(n) => self.combos.get(&n).map(|c| c.0.clone()).unwrap_or_default(),
            _ => Vec::new(),
        }
    }
    pub fn combo_value(&self, id: Cid) -> String {
        match id {
            Cid::D(n) => self
                .combos
                .get(&n)
                .and_then(|(v, i)| v.get(*i).cloned())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
    fn combo_index(&self, n: u8) -> usize {
        self.combos.get(&n).map(|c| c.1).unwrap_or(0)
    }
    pub fn combo_enabled(&self, id: Cid) -> bool {
        !matches!(id, Cid::D(n) if self.combo_off.contains(&n))
    }
    pub fn check_value(&self, id: Kid) -> bool {
        matches!(id, Kid::D(n) if self.checks.get(&n).copied().unwrap_or(false))
    }
    pub fn check_enabled(&self, id: Kid) -> bool {
        !matches!(id, Kid::D(n) if self.check_off.contains(&n))
    }
    pub fn slider_def(&self, id: Sid) -> (i32, i32, i32) {
        match id {
            Sid::D(n) => self.sliders.get(&n).copied().unwrap_or((0, 1, 0)),
            _ => (0, 1, 0),
        }
    }
    pub fn slider_set(&mut self, id: Sid, v: i32) {
        if let Sid::D(n) = id {
            if let Some(s) = self.sliders.get_mut(&n) {
                s.2 = v;
            }
        }
    }
    fn combo(&mut self, n: u8, items: Vec<String>, value: &str) {
        let i = items.iter().position(|x| x == value).unwrap_or(0);
        self.combos.insert(n, (items, i));
    }
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ----------------------------------------------------------- opening

pub fn open(e: &mut Editor, kind: Kind) {
    let d = build(e, kind);
    e.dialog = Some(d);
    e.focus = e
        .dialog
        .as_ref()
        .and_then(|d| d.fields.first().map(|f| Fid::D(*f)));
    e.popup = None;
    e.dirty_draw = true;
}

/// Open a dialog over the current one; closing it returns to the first.
pub fn open_child(e: &mut Editor, kind: Kind) {
    let parent = e.dialog.take().map(Box::new);
    let mut d = build(e, kind);
    d.parent = parent;
    e.dialog = Some(d);
    e.focus = e
        .dialog
        .as_ref()
        .and_then(|d| d.fields.first().map(|f| Fid::D(*f)));
    e.dirty_draw = true;
}

pub fn close(e: &mut Editor) {
    if let Some(d) = e.dialog.take() {
        // Dialog fields are private to it.
        for f in &d.fields {
            e.edits.remove(&Fid::D(*f));
        }
        e.dialog = d.parent.map(|b| *b);
        if let Some(p) = &e.dialog {
            restore_parent_fields(e, p.kind.clone());
        }
    }
    e.focus = None;
    e.dirty_draw = true;
}

fn restore_parent_fields(_e: &mut Editor, _k: Kind) {}

pub fn message(e: &mut Editor, title: &str, text: &str) {
    let mut d = Dialog::new(Kind::Message, title, 460);
    d.text = text.to_string();
    d.buttons = vec![("OK".into(), 1)];
    if e.dialog.is_some() {
        d.parent = e.dialog.take().map(Box::new);
    }
    e.dialog = Some(d);
    e.focus = None;
    e.dirty_draw = true;
}

pub fn ask(e: &mut Editor, title: &str, text: &str, buttons: Vec<(&str, u16)>, then: Then) {
    let mut d = Dialog::new(Kind::Ask(then), title, 460);
    d.text = text.to_string();
    d.buttons = buttons
        .into_iter()
        .map(|(s, c)| (s.to_string(), c))
        .collect();
    e.dialog = Some(d);
    e.focus = None;
    e.dirty_draw = true;
}

fn set(e: &mut Editor, n: u8, s: &str) {
    let mut ed = ui::Edit::new(s, Fid::D(n).multiline());
    ed.cur = ed.text.len();
    ed.anchor = ed.cur;
    e.edits.insert(Fid::D(n), ed);
}

fn get(e: &Editor, n: u8) -> String {
    e.field(Fid::D(n))
}

fn build(e: &mut Editor, kind: Kind) -> Dialog {
    match kind {
        Kind::LayoutPicker => Dialog::new(Kind::LayoutPicker, "Choose layout", 780),
        Kind::DeckSettings => {
            let mut d = Dialog::new(Kind::DeckSettings, "Deck settings", 520);
            let fams = crate::fonts::db().families();
            d.combo(
                0,
                THEMES.iter().map(|t| t.name.to_string()).collect(),
                &normalize_theme(&s_get(&e.data, "theme", "B&W")),
            );
            d.combo(1, fams, &s_get(&e.data, "fontFamily", "Noto Sans"));
            d.combo(2, strs(&ASPECTS), &s_get(&e.data, "aspect", "16:9"));
            d.combo(
                3,
                strs(&TRANSITIONS),
                &normalize_transition(e.data.get("transition"), false),
            );
            d.combo(
                4,
                strs(&["LEFT", "CENTER", "RIGHT"]),
                &normalize_footer_align(&s_get(&e.data, "footerAlign", "LEFT")),
            );
            d.combo(
                5,
                strs(&["SMALL", "MEDIUM", "LARGE"]),
                &normalize_footer_size(&s_get(&e.data, "footerSize", "MEDIUM")),
            );
            let deck = Deck(&e.data);
            d.checks.insert(0, deck.reduce_motion());
            d.checks.insert(1, deck.flag_default_true("cinematicOpen"));
            d.checks.insert(2, deck.flag_default_true("cinematicClose"));
            d.checks.insert(3, deck.show_numbers());
            d.checks.insert(4, deck.show_logo());
            if !theme(&s_get(&e.data, "theme", "B&W")).is_some_and(|t| t.uddevalla) {
                d.check_off.push(4);
            }
            d.sliders
                .insert(0, (0, 120, i_get(&e.data, "targetMinutes", 0) as i32));
            set(e, 0, &s_get(&e.data, "footerText", ""));
            d.fields = vec![0];
            d
        }
        Kind::FileWorkflow => {
            let mut d = Dialog::new(Kind::FileWorkflow, "File workflow", 640);
            let fp = e.file_prefs.clone();
            d.checks.insert(0, fp.remember_last);
            d.combo(
                0,
                strs(&["OPTIMIZED", "HIGH", "ORIGINAL"]),
                &fp.asset_quality,
            );
            set(e, 0, &fp.default_folder);
            set(e, 1, &fp.last_folder);
            d.fields = vec![0, 1];
            d
        }
        Kind::Palette => {
            let mut d = Dialog::new(Kind::Palette, "NIRUPRES — Command Palette", 640);
            set(e, 0, "");
            d.fields = vec![0];
            d.items = palette_commands()
                .iter()
                .map(|(n, _)| n.to_string())
                .collect();
            d.item_codes = (0..d.items.len()).collect();
            d
        }
        Kind::New => {
            let mut d = Dialog::new(Kind::New, "New presentation", 540);
            let labels: Vec<String> = prefs::PROFILES
                .iter()
                .map(|p| p.label.to_string())
                .collect();
            let cur = prefs::profile_info(&e.profile).label;
            d.combo(0, labels, cur);
            d
        }
        Kind::Overview => Dialog::new(Kind::Overview, "Presentation overview", 1000),
        Kind::Health(issues) => {
            let mut d = Dialog::new(Kind::Health(issues.clone()), "Deck Health", 760);
            d.items = issues
                .iter()
                .map(|(i, k, m)| format!("{k}   {:02}   {m}", i + 1))
                .collect();
            d.item_codes = issues.iter().map(|(i, _, _)| *i).collect();
            d
        }
        Kind::DisplaySetup(mons) => {
            let mut d = Dialog::new(
                Kind::DisplaySetup(mons.clone()),
                "NIRUPRES — Display Setup",
                600,
            );
            let labels: Vec<String> = mons
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    format!(
                        "{}. {} — {}×{} @ {},{}{}",
                        i + 1,
                        m.name,
                        m.w,
                        m.h,
                        m.x,
                        m.y,
                        if m.primary { " — PRIMARY" } else { "" }
                    )
                })
                .collect();
            let prefs = prefs::display_prefs();
            let find = |name: Option<&str>, default: usize| -> usize {
                name.and_then(|n| mons.iter().position(|m| m.name == n))
                    .unwrap_or(default)
            };
            let primary = mons.iter().position(|m| m.primary).unwrap_or(0);
            let pi = find(prefs.get("presenter").and_then(|v| v.as_str()), primary);
            let other = (0..mons.len()).find(|&i| i != pi).unwrap_or(pi);
            let ai = find(prefs.get("audience").and_then(|v| v.as_str()), other);
            d.combos.insert(0, (labels.clone(), pi));
            d.combos.insert(1, (labels, ai));
            d
        }
        Kind::OneScreen { ask, from_start } => {
            let mut d = Dialog::new(
                Kind::OneScreen { ask, from_start },
                "Present on one screen",
                560,
            );
            d.checks.insert(0, false);
            d
        }
        Kind::Recovery { info, when } => {
            let mut d = Dialog::new(
                Kind::Recovery {
                    info: info.clone(),
                    when,
                },
                "Recover autosave",
                460,
            );
            d.text = info;
            d.buttons = vec![("Restore".into(), 1), ("Discard".into(), 2)];
            d
        }
        Kind::Confirm(then) => {
            let mut d = Dialog::new(Kind::Confirm(then), "Unsaved changes", 460);
            d.text = "Save changes before continuing?".into();
            d.buttons = vec![
                ("Save".into(), 1),
                ("Discard".into(), 2),
                ("Cancel".into(), 3),
            ];
            d
        }
        Kind::Assets(paths) => {
            let mut d = Dialog::new(Kind::Assets(paths.clone()), "Assets", 800);
            d.items = asset_rows(e, &paths);
            d.item_codes = (0..paths.len()).collect();
            d
        }
        k => Dialog::new(k, "", 520),
    }
}

pub fn open_text_editor(e: &mut Editor) {
    let Some(s) = e.current().cloned() else {
        return;
    };
    let layout = s_get(&s, "layout", "");
    let two = layout == "TWO COLUMN" || layout == "COMPARE";
    let hint = match layout.as_str() {
        "PROCESS" => "One step per line: HEADING | description",
        "TIMELINE" => "One milestone per line: MILESTONE | description",
        "NUMBER GRID" => "One figure per line: VALUE | label · up to four",
        "DATA / KPI" => "One KPI per line: VALUE | label",
        "MATRIX" => "One quadrant per line: HEADING | description · up to four",
        "TABLE" => "One row per line · separate columns with |",
        "BULLETS" => "One list item per line",
        "AGENDA" => "One agenda item per line",
        "CODE" => "Code exactly as written · no **bold** parsing · shrinks to fit",
        "THREE COLUMN" => "Separate the three columns with a line that is only |||",
        "IMAGE + CAPTION" => "Caption text under the image",
        _ => "",
    }
    .to_string();
    let mut d = Dialog::new(
        Kind::TextEditor { two, hint },
        "Edit slide text",
        if two { 800 } else { 660 },
    );
    set(e, 0, &s_get(&s, "title", ""));
    if two {
        let structured = b_get(&s, "sideStructured") || b_get(&s, "twoColumnStructured");
        let (l, r) = if structured
            && s.get("leftColumn").is_some_and(|v| !v.is_null())
            && s.get("rightColumn").is_some_and(|v| !v.is_null())
        {
            (s_or(&s, "leftColumn", ""), s_or(&s, "rightColumn", ""))
        } else {
            let (l, r, _) = parse_two_column_body(&s_get(&s, "body", ""));
            (l, r)
        };
        set(e, 101, &l);
        set(e, 102, &r);
        d.fields = vec![0, 101, 102];
    } else {
        set(e, 100, &s_get(&s, "body", ""));
        d.fields = vec![0, 100];
    }
    e.dialog = Some(d);
    e.focus = Some(Fid::D(0));
    e.dirty_draw = true;
}

pub fn open_about(e: &mut Editor, x: &X) {
    let mons = x.monitors();
    let screens = if mons.is_empty() {
        "  none".to_string()
    } else {
        mons.iter()
            .enumerate()
            .map(|(i, m)| {
                format!(
                    "  {}. {} — {}×{} @ {},{}",
                    i + 1,
                    m.name,
                    m.w,
                    m.h,
                    m.x,
                    m.y
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut uts: libc::utsname = unsafe { std::mem::zeroed() };
    unsafe { libc::uname(&mut uts) };
    let cs = |a: &[libc::c_char]| {
        unsafe { std::ffi::CStr::from_ptr(a.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    };
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let text = format!(
        "NIRUPRES {VERSION}\n{TAGLINE}\n\nProject format: {FORMAT} · NIRUPRES Markdown spec: {MD_FORMAT}\nEngine: Rust, X11 (x11rb), own canvas\nPlatform: {} {}\nSession: {} · Wayland: {}\nDesktop: {} · Portal: {}\nEditor chrome: {}\nScreens:\n{screens}",
        cs(&uts.sysname),
        cs(&uts.release),
        env("XDG_SESSION_TYPE", "unknown"),
        env("WAYLAND_DISPLAY", "-"),
        env("XDG_CURRENT_DESKTOP", "unknown"),
        env("XDG_DESKTOP_PORTAL_DIR", "auto"),
        crate::chrome::status_text(e.palette.as_ref(), e.chrome_follow)
    );
    let mut d = Dialog::new(Kind::About(text.clone()), &format!("About {APP}"), 680);
    set(e, 100, &text);
    if let Some(ed) = e.edits.get_mut(&Fid::D(100)) {
        ed.readonly = true;
        ed.cur = 0;
        ed.anchor = 0;
    }
    d.fields = vec![100];
    e.dialog = Some(d);
    e.focus = None;
    e.dirty_draw = true;
}

fn asset_rows(e: &Editor, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|p| {
            let path = Path::new(p);
            let ok = path.exists();
            let (w, h) = if ok {
                crate::images::dimensions(path)
            } else {
                (0, 0)
            };
            let size = if ok {
                std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) as f64 / MB as f64
            } else {
                0.0
            };
            let mut roles = Vec::new();
            let mut slides = Vec::new();
            for (i, s) in e.slides().iter().enumerate() {
                let Some(s) = s.as_object() else { continue };
                for (f, label) in [
                    ("image", "IMAGE"),
                    ("backgroundImage", "BACKGROUND"),
                    ("videoPoster", "VIDEO POSTER"),
                    ("videoSource", "VIDEO"),
                ] {
                    if s_get(s, f, "") == *p {
                        if !roles.contains(&label) {
                            roles.push(label);
                        }
                        slides.push((i + 1).to_string());
                    }
                }
            }
            roles.sort();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!(
                "{}{name}    · {} · {w}×{h} · {size:.1} MB · slides {}",
                if ok { "✓ " } else { "! MISSING  " },
                roles.join("/"),
                slides.join(", ")
            )
        })
        .collect()
}

pub fn open_assets(e: &mut Editor) {
    let mut paths: Vec<String> = Vec::new();
    for s in e.slides() {
        let Some(s) = s.as_object() else { continue };
        for f in ["image", "backgroundImage", "videoPoster", "videoSource"] {
            let p = s_get(s, f, "");
            if !p.is_empty() && !paths.contains(&p) {
                paths.push(p);
            }
        }
    }
    open(e, Kind::Assets(paths));
}

pub fn open_display_setup(e: &mut Editor, mons: Vec<x::Monitor>) {
    open(e, Kind::DisplaySetup(mons));
}

// --------------------------------------------------------- file dialog

fn exts_for(p: &FilePurpose) -> Vec<&'static str> {
    match p {
        FilePurpose::Open => vec!["nirupres", "json"],
        FilePurpose::SaveAs(_) => vec!["nirupres"],
        FilePurpose::ExportPng(_) => vec!["png"],
        FilePurpose::ExportPdf => vec!["pdf"],
        FilePurpose::ExportPptx => vec!["pptx"],
        FilePurpose::ExportMd | FilePurpose::ExportAiBrief | FilePurpose::ExportAiTemplate => {
            vec!["md"]
        }
        FilePurpose::ImportMd => vec!["md", "markdown"],
        FilePurpose::Image(_) | FilePurpose::ReplaceAsset(_) => {
            vec!["png", "jpg", "jpeg", "webp", "bmp"]
        }
        FilePurpose::Video => vec!["mp4", "webm", "mov", "mkv"],
        FilePurpose::ExportAllPng | FilePurpose::DefaultFolder => vec![],
    }
}

fn file_title(p: &FilePurpose) -> (&'static str, &'static str) {
    // (title, filter description)
    match p {
        FilePurpose::Open => ("Open NIRUPRES", "NIRUPRES (*.nirupres *.json)"),
        FilePurpose::SaveAs(_) => ("Save NIRUPRES As", "NIRUPRES (*.nirupres)"),
        FilePurpose::ExportPng(_) => ("Export current slide PNG", "PNG (*.png)"),
        FilePurpose::ExportAllPng => ("Export all slides as PNG", "Folders"),
        FilePurpose::ExportPdf => ("Export PDF", "PDF (*.pdf)"),
        FilePurpose::ExportPptx => ("Export pixel-perfect PPTX", "PowerPoint (*.pptx)"),
        FilePurpose::ExportMd => ("Export NIRUPRES Markdown", "Markdown (*.md)"),
        FilePurpose::ExportAiBrief => ("Export AI brief", "Markdown (*.md)"),
        FilePurpose::ExportAiTemplate => ("Export AI template", "Markdown (*.md)"),
        FilePurpose::ImportMd => ("Import NIRUPRES Markdown", "Markdown (*.md *.markdown)"),
        FilePurpose::Image(Fid::BgImg) => (
            "Choose background image",
            "Images (*.png *.jpg *.jpeg *.webp *.bmp)",
        ),
        FilePurpose::Image(Fid::VideoPoster) => (
            "Choose video poster",
            "Images (*.png *.jpg *.jpeg *.webp *.bmp)",
        ),
        FilePurpose::Image(_) => ("Choose image", "Images (*.png *.jpg *.jpeg *.webp *.bmp)"),
        FilePurpose::ReplaceAsset(_) => {
            ("Replace image", "Images (*.png *.jpg *.jpeg *.webp *.bmp)")
        }
        FilePurpose::Video => ("Choose local video", "Video (*.mp4 *.webm *.mov *.mkv)"),
        FilePurpose::DefaultFolder => ("Choose default presentation folder", "Folders"),
    }
}

pub fn file_dialog(e: &mut Editor, purpose: FilePurpose) {
    let save = matches!(
        purpose,
        FilePurpose::SaveAs(_)
            | FilePurpose::ExportPng(_)
            | FilePurpose::ExportPdf
            | FilePurpose::ExportPptx
            | FilePurpose::ExportMd
            | FilePurpose::ExportAiBrief
            | FilePurpose::ExportAiTemplate
    );
    let folder = matches!(
        purpose,
        FilePurpose::ExportAllPng | FilePurpose::DefaultFolder
    );
    let name = match &purpose {
        FilePurpose::SaveAs(_) => e
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "presentation.nirupres".into()),
        FilePurpose::ExportPng(n) => n.clone(),
        FilePurpose::ExportPdf => "presentation.pdf".into(),
        FilePurpose::ExportPptx => "presentation.pptx".into(),
        FilePurpose::ExportMd => "presentation.md".into(),
        FilePurpose::ExportAiBrief => "NIRUPRES-AI-BRIEF.md".into(),
        FilePurpose::ExportAiTemplate => "NIRUPRES-AI-TEMPLATE.md".into(),
        _ => String::new(),
    };
    let prefer_current = !matches!(purpose, FilePurpose::Open | FilePurpose::ImportMd);
    let dir = match &purpose {
        FilePurpose::DefaultFolder => {
            let f = get(e, 0);
            if !f.is_empty() && Path::new(&f).is_dir() {
                PathBuf::from(f)
            } else {
                e.file_prefs.preferred(None, false)
            }
        }
        _ => e.dialog_start("", prefer_current),
    };
    let (title, filt) = file_title(&purpose);
    let st = FileState {
        exts: exts_for(&purpose),
        purpose,
        dir: dir.clone(),
        entries: Vec::new(),
        save,
        folder,
        filter_name: filt.to_string(),
        overwrite: None,
        hidden: false,
    };
    let nested = e.dialog.is_some();
    let mut d = Dialog::new(Kind::File(st), title, 760);
    d.fields = vec![90, 91];
    let parent = if nested {
        e.dialog.take().map(Box::new)
    } else {
        None
    };
    d.parent = parent;
    set(e, 90, &name);
    set(e, 91, &dir.to_string_lossy());
    e.dialog = Some(d);
    list_dir(e);
    e.focus = Some(Fid::D(if save { 90 } else { 91 }));
    if save {
        if let Some(ed) = e.edits.get_mut(&Fid::D(90)) {
            // Select the name without the extension, as file dialogs do.
            let stem = name.rfind('.').unwrap_or(name.len());
            ed.anchor = 0;
            ed.cur = name[..stem].chars().count();
        }
    }
    e.dirty_draw = true;
}

fn list_dir(e: &mut Editor) {
    let Some(Dialog {
        kind: Kind::File(st),
        ..
    }) = e.dialog.as_mut()
    else {
        return;
    };
    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&st.dir) {
        for de in rd.flatten() {
            let name = de.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') && !st.hidden {
                continue;
            }
            let is_dir = de.path().is_dir();
            if !is_dir {
                if st.folder {
                    continue;
                }
                let lower = name.to_lowercase();
                if !st.exts.is_empty() && !st.exts.iter().any(|x| lower.ends_with(&format!(".{x}")))
                {
                    continue;
                }
            }
            entries.push((name, is_dir));
        }
    }
    entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    st.entries = entries;
    let dir = st.dir.to_string_lossy().into_owned();
    if let Some(d) = e.dialog.as_mut() {
        d.items = match &d.kind {
            Kind::File(st) => st
                .entries
                .iter()
                .map(|(n, dir)| {
                    if *dir {
                        format!("▸  {n}/")
                    } else {
                        format!("    {n}")
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        d.item_codes = (0..d.items.len()).collect();
        d.sel = 0;
        d.scroll = 0;
    }
    set(e, 91, &dir);
}

fn file_accept(e: &mut Editor, x: &mut X) -> Request {
    let Some(Dialog {
        kind: Kind::File(st),
        ..
    }) = e.dialog.as_ref()
    else {
        return Request::None;
    };
    let st = st.clone();
    let name = get(e, 90);
    let sel = e.dialog.as_ref().map(|d| d.sel).unwrap_or(0);
    let chosen: Option<PathBuf> = if st.folder {
        // The selected folder, or the current one.
        let sub = st
            .entries
            .get(sel)
            .filter(|en| en.1)
            .map(|en| st.dir.join(&en.0));
        Some(if name.is_empty() {
            sub.unwrap_or(st.dir.clone())
        } else {
            resolve_name(&st.dir, &name)
        })
    } else if st.save {
        if name.trim().is_empty() {
            None
        } else {
            Some(resolve_name(&st.dir, name.trim()))
        }
    } else {
        let typed = name.trim();
        if !typed.is_empty() {
            Some(resolve_name(&st.dir, typed))
        } else {
            st.entries.get(sel).map(|(n, _)| st.dir.join(n))
        }
    };
    let Some(path) = chosen else {
        return Request::None;
    };
    if path.is_dir() && !st.folder {
        if let Some(Dialog {
            kind: Kind::File(s2),
            ..
        }) = e.dialog.as_mut()
        {
            s2.dir = path;
        }
        set(e, 90, if st.save { &name } else { "" });
        list_dir(e);
        return Request::None;
    }
    if !st.save && !st.folder && !path.is_file() {
        e.set_status(&format!("Not found: {}", path.display()), 3000);
        return Request::None;
    }
    let mut path = path;
    if st.save {
        let ext = st.exts.first().copied().unwrap_or("");
        if !ext.is_empty()
            && !path
                .to_string_lossy()
                .to_lowercase()
                .ends_with(&format!(".{ext}"))
        {
            path = PathBuf::from(format!("{}.{ext}", path.display()));
        }
        if path.exists() && st.overwrite.as_ref() != Some(&path) {
            if let Some(Dialog {
                kind: Kind::File(s2),
                ..
            }) = e.dialog.as_mut()
            {
                s2.overwrite = Some(path.clone());
            }
            e.dirty_draw = true;
            return Request::None;
        }
    }
    close(e);
    e.remember_dialog(&path);
    file_chosen(e, x, st.purpose, path)
}

fn resolve_name(dir: &Path, name: &str) -> PathBuf {
    let p = prefs::expand(name);
    if p.is_absolute() {
        p
    } else {
        dir.join(p)
    }
}

fn file_chosen(e: &mut Editor, x: &mut X, purpose: FilePurpose, p: PathBuf) -> Request {
    let with = |p: &Path, ext: &str| crate::export::with_ext(p, ext);
    match purpose {
        FilePurpose::Open => {
            if let Err(err) = e.open_path(x, &p) {
                message(
                    e,
                    "Open failed",
                    &format!("NIRUPRES could not open this presentation.\n\n{err}"),
                );
            }
        }
        FilePurpose::SaveAs(then) => {
            let old = e.path.clone();
            e.path = Some(with(&p, ".nirupres"));
            if !e.save(x, then) {
                if e.dialog.is_none() {
                    e.path = old;
                }
            } else if e.quit_after_present {
                e.quit_after_present = false;
                return Request::Quit;
            }
            e.update_title(x);
        }
        FilePurpose::ExportPng(_) => {
            let p = with(&p, ".png");
            match crate::export::export_png(&e.data, e.cur, &p) {
                Ok(_) => e.set_status(&format!("PNG exported: {}", p.display()), 2500),
                Err(err) => message(e, "Export failed", &err),
            }
        }
        FilePurpose::ExportAllPng => match crate::export::export_all_png(&e.data, &p) {
            Ok(n) => e.set_status(&format!("Exported {n} PNG slides"), 2500),
            Err(err) => message(e, "Export failed", &err),
        },
        FilePurpose::ExportPdf => match crate::export::export_pdf(&e.data, &p) {
            Ok(p) => e.set_status(&format!("PDF exported: {}", p.display()), 3000),
            Err(err) => message(e, "Export failed", &err),
        },
        FilePurpose::ExportPptx => match crate::export::export_pptx(&e.data, &p) {
            Ok(p) => e.set_status(
                &format!("Pixel-perfect PPTX exported: {}", p.display()),
                3500,
            ),
            Err(err) => message(e, "PPTX export", &err),
        },
        FilePurpose::ExportMd | FilePurpose::ExportAiBrief | FilePurpose::ExportAiTemplate => {
            let p = with(&p, ".md");
            let (text, what) = match purpose {
                FilePurpose::ExportMd => (crate::markdown::export(&e.data), "Markdown exported"),
                FilePurpose::ExportAiBrief => {
                    (crate::markdown::ai_brief(&e.data), "AI brief exported")
                }
                _ => (
                    crate::markdown::AI_TEMPLATE.to_string(),
                    "AI template exported",
                ),
            };
            match std::fs::write(&p, text) {
                Ok(_) => e.set_status(&format!("{what}: {}", p.display()), 3000),
                Err(err) => message(e, "Export failed", &err.to_string()),
            }
        }
        FilePurpose::ImportMd => e.import_markdown(x, &p),
        FilePurpose::Image(id) => {
            let s = p.to_string_lossy().into_owned();
            e.edits.entry(id).or_default().reset(&s);
            let key = match id {
                Fid::BgImg => "backgroundImage",
                Fid::VideoPoster => "videoPoster",
                _ => "image",
            };
            set_slide(e, key, &s);
        }
        FilePurpose::Video => {
            let s = p.to_string_lossy().into_owned();
            e.edits.entry(Fid::VideoSource).or_default().reset(&s);
            set_slide(e, "videoSource", &s);
        }
        FilePurpose::DefaultFolder => {
            set(e, 0, &p.to_string_lossy());
        }
        FilePurpose::ReplaceAsset(old) => {
            let new = p.to_string_lossy().into_owned();
            let slides: Vec<usize> = (0..e.slides().len()).collect();
            for i in slides {
                if let Some(s) = e
                    .data
                    .get_mut("slides")
                    .and_then(|v| v.get_mut(i))
                    .and_then(|v| v.as_object_mut())
                {
                    for f in ["image", "backgroundImage", "videoPoster", "videoSource"] {
                        if s_get(s, f, "") == old {
                            s.insert(f.into(), json!(new));
                        }
                    }
                }
            }
            e.mark_dirty();
            let c = e.cur;
            e.refresh(c);
            close(e);
            open_assets(e);
        }
    }
    Request::None
}

fn set_slide(e: &mut Editor, key: &str, v: &str) {
    let i = e.cur;
    if let Some(s) = e
        .data
        .get_mut("slides")
        .and_then(|s| s.get_mut(i))
        .and_then(|s| s.as_object_mut())
    {
        s.insert(key.to_string(), json!(v));
    }
    e.mark_dirty();
    e.refresh(i);
    e.push_history();
}

// -------------------------------------------------------- continuation

pub fn run_then(e: &mut Editor, x: &mut X, then: Then) -> Request {
    match then {
        Then::New => {
            open(e, Kind::New);
            Request::None
        }
        Then::Open => {
            file_dialog(e, FilePurpose::Open);
            Request::None
        }
        Then::OpenPath(p) => {
            if let Err(err) = e.open_path(x, &p) {
                message(
                    e,
                    "Open failed",
                    &format!("NIRUPRES could not open this presentation.\n\n{err}"),
                );
            }
            Request::None
        }
        Then::Import => {
            file_dialog(e, FilePurpose::ImportMd);
            Request::None
        }
        Then::Quit => {
            e.clear_autosave();
            Request::Quit
        }
        Then::Health => {
            let issues = e.validate_deck();
            open(e, Kind::Health(issues));
            Request::None
        }
    }
}

// ------------------------------------------------------------- palette

pub fn palette_commands() -> Vec<(String, Act)> {
    let mut v: Vec<(String, Act)> = [
        ("Add slide", Act::AddSlide),
        ("Duplicate slide", Act::Duplicate),
        ("Delete selected slides", Act::Delete),
        ("Copy slide", Act::CopySlides),
        ("Paste slide", Act::PasteSlides),
        ("Add same layout", Act::SameLayout),
        ("Move slide up · Alt+Up", Act::MoveUp),
        ("Move slide down · Alt+Down", Act::MoveDown),
        ("Deck settings", Act::DeckSettings),
        ("File workflow settings", Act::FileWorkflow),
        ("Import NIRUPRES Markdown", Act::ImportMd),
        ("Export NIRUPRES Markdown", Act::ExportMd),
        ("Export AI brief", Act::ExportAiBrief),
        ("Copy AI prompt", Act::CopyAiPrompt),
        ("Export AI template", Act::ExportAiTemplate),
        ("Deck health", Act::DeckHealth),
        ("Asset manager", Act::AssetManager),
        ("Presentation overview", Act::Overview),
        ("Export current slide to PNG", Act::ExportPng),
        ("Export all slides to PNG", Act::ExportAllPng),
        ("Export deck to PDF", Act::ExportPdf),
        ("Export pixel-perfect PPTX", Act::ExportPptx),
        ("Focus mode", Act::Focus),
        ("Editor chrome · Follow NIRUX", Act::ChromeFollow(true)),
        ("Editor chrome · Noir", Act::ChromeFollow(false)),
    ]
    .iter()
    .map(|(n, a)| (n.to_string(), *a))
    .collect();
    for (i, p) in prefs::PROFILES.iter().enumerate() {
        v.push((
            format!("Profile · {} (new decks)", p.label),
            Act::Profile(i as u8),
        ));
    }
    for (n, a) in [
        ("Talk · screen + microphone", Act::Talk(0)),
        ("Talk · screen + camera", Act::Talk(1)),
        ("Talk · camera + microphone", Act::Talk(2)),
        ("Present from current", Act::Present),
        ("Present from beginning", Act::PresentStart),
        ("Online presentation", Act::Online),
        ("Online presentation from beginning", Act::OnlineStart),
        ("Presentation check", Act::PresentationCheck),
    ] {
        v.push((n.into(), a));
    }
    for (i, m) in LOGO_MODES.iter().enumerate() {
        v.push((
            format!("Selected slides · Logo {m}"),
            Act::ApplyLogo(i as u8),
        ));
    }
    for (i, m) in ACCENTS.iter().enumerate() {
        v.push((
            format!("Selected slides · Accent {m}"),
            Act::ApplyAccent(i as u8),
        ));
    }
    for (n, a) in [
        ("Save", Act::Save),
        ("Save as", Act::SaveAs),
        ("Open", Act::Open),
        ("About / diagnostics", Act::About),
    ] {
        v.push((n.into(), a));
    }
    v
}

pub fn field_changed(e: &mut Editor, id: Fid) {
    if let (
        Fid::D(0),
        Some(Dialog {
            kind: Kind::Palette,
            ..
        }),
    ) = (id, e.dialog.as_ref())
    {
        let term = get(e, 0).to_lowercase();
        let term = term.trim().to_string();
        let cmds = palette_commands();
        if let Some(d) = e.dialog.as_mut() {
            d.item_codes = cmds
                .iter()
                .enumerate()
                .filter(|(_, (n, _))| term.is_empty() || n.to_lowercase().contains(&term))
                .map(|(i, _)| i)
                .collect();
            d.items = d.item_codes.iter().map(|&i| cmds[i].0.clone()).collect();
            d.sel = 0;
            d.scroll = 0;
        }
    }
    e.dirty_draw = true;
}

pub fn combo_chosen(e: &mut Editor, _x: &mut X, id: Cid, idx: usize) -> Request {
    let Cid::D(n) = id else { return Request::None };
    if let Some(d) = e.dialog.as_mut() {
        if let Some(c) = d.combos.get_mut(&n) {
            c.1 = idx;
        }
        if let Kind::DeckSettings = d.kind {
            if n == 0 {
                let t = d.combo_value(Cid::D(0));
                d.check_off.retain(|x| *x != 4);
                if !theme(&t).is_some_and(|t| t.uddevalla) {
                    d.check_off.push(4);
                }
            }
        }
    }
    e.dirty_draw = true;
    Request::None
}

pub fn check_toggled(e: &mut Editor, id: Kid) {
    if let (Kid::D(n), Some(d)) = (id, e.dialog.as_mut()) {
        let v = d.checks.entry(n).or_insert(false);
        *v = !*v;
    }
    e.dirty_draw = true;
}

pub fn submit(e: &mut Editor, x: &mut X, id: Fid) {
    let kind = e.dialog.as_ref().map(|d| d.kind.clone());
    match (kind, id) {
        (Some(Kind::Palette), _) => {
            let r = run_palette(e, x);
            if r != Request::None {
                e.pending = Some(r);
            }
        }
        (Some(Kind::File(_)), Fid::D(91)) => {
            let p = prefs::expand(get(e, 91).trim());
            if p.is_dir() {
                if let Some(Dialog {
                    kind: Kind::File(st),
                    ..
                }) = e.dialog.as_mut()
                {
                    st.dir = p;
                }
                list_dir(e);
            }
        }
        (Some(Kind::File(_)), _) => {
            let r = file_accept(e, x);
            if r != Request::None {
                e.pending = Some(r);
            }
        }
        (Some(_), _) => {
            let r = button(e, x, default_code(e));
            if r != Request::None {
                e.pending = Some(r);
            }
        }
        _ => {}
    }
}

fn default_code(e: &Editor) -> u16 {
    match e.dialog.as_ref().map(|d| &d.kind) {
        Some(Kind::DeckSettings) | Some(Kind::FileWorkflow) | Some(Kind::TextEditor { .. }) => 1,
        Some(Kind::DisplaySetup(_)) => 1,
        Some(Kind::OneScreen { .. }) => 21,
        Some(Kind::Message) | Some(Kind::Recovery { .. }) | Some(Kind::Confirm(_)) => 1,
        Some(Kind::Ask(_)) => e
            .dialog
            .as_ref()
            .and_then(|d| d.buttons.last().map(|b| b.1))
            .unwrap_or(0),
        Some(Kind::About(_)) => 2,
        _ => 0,
    }
}

fn run_palette(e: &mut Editor, x: &mut X) -> Request {
    let Some(d) = e.dialog.as_ref() else {
        return Request::None;
    };
    let Some(&code) = d.item_codes.get(d.sel) else {
        return Request::None;
    };
    let cmds = palette_commands();
    close(e);
    match cmds.get(code) {
        Some((_, a)) => e.act(x, *a, None),
        None => Request::None,
    }
}

// --------------------------------------------------------------- input

pub fn key(e: &mut Editor, x: &mut X, sym: u32, state: u16, ch: Option<char>) -> Request {
    use crate::keys::*;
    let kind = e.dialog.as_ref().map(|d| d.kind.clone());
    if sym == K_ESCAPE {
        if let Some(Kind::File(st)) = &kind {
            if st.overwrite.is_some() {
                if let Some(Dialog {
                    kind: Kind::File(s2),
                    ..
                }) = e.dialog.as_mut()
                {
                    s2.overwrite = None;
                }
                e.dirty_draw = true;
                return Request::None;
            }
        }
        return cancel(e, x);
    }
    let list_kind = matches!(
        kind,
        Some(Kind::Palette) | Some(Kind::Health(_)) | Some(Kind::Assets(_)) | Some(Kind::File(_))
    );
    if list_kind && matches!(sym, K_UP | K_DOWN | K_PRIOR | K_NEXT) {
        if let Some(d) = e.dialog.as_mut() {
            let n = d.items.len() as i32;
            let step = match sym {
                K_UP => -1,
                K_DOWN => 1,
                K_PRIOR => -10,
                _ => 10,
            };
            d.sel = (d.sel as i32 + step).clamp(0, (n - 1).max(0)) as usize;
            ensure_visible(d);
        }
        e.dirty_draw = true;
        return Request::None;
    }
    if let Some(Kind::TextEditor { .. }) = kind {
        let ctrl = state & x::CTRL != 0;
        if ctrl && matches!(sym, 0x62 | 0x42 | 0x69 | 0x49 | 0x30) {
            let k = match sym {
                0x62 | 0x42 => "bold",
                0x69 | 0x49 => "italic",
                _ => "normal",
            };
            apply_format(e, k);
            return Request::None;
        }
    }
    if let Some(id) = e.focus {
        if sym == K_TAB || sym == K_ISO_LEFT_TAB {
            // Move between the dialog's fields.
            if let Some(d) = e.dialog.as_ref() {
                let fs = d.fields.clone();
                if let Some(pos) = fs.iter().position(|f| Fid::D(*f) == id) {
                    let back = sym == K_ISO_LEFT_TAB || state & x::SHIFT != 0;
                    let n = fs.len();
                    let next = if back {
                        (pos + n - 1) % n
                    } else {
                        (pos + 1) % n
                    };
                    e.focus = Some(Fid::D(fs[next]));
                    e.dirty_draw = true;
                    return Request::None;
                }
            }
        }
        let r = e.edit_key(x, id, sym, state, ch);
        if r != ui::EditResult::Unhandled {
            return Request::None;
        }
    }
    if sym == K_RETURN {
        if list_kind {
            return activate_item(e, x);
        }
        let c = default_code(e);
        if c != 0 {
            return button(e, x, c);
        }
    }
    Request::None
}

fn ensure_visible(d: &mut Dialog) {
    let rh = d.list_row_h.max(1);
    let vis = (d.list_rect.h / rh).max(1);
    let s = d.sel as i32;
    let first = d.scroll / rh;
    if s < first {
        d.scroll = s * rh;
    } else if s >= first + vis {
        d.scroll = (s - vis + 1) * rh;
    }
}

pub fn wheel(e: &mut Editor, d: i32) {
    if let Some(dl) = e.dialog.as_mut() {
        let total = dl.items.len() as i32 * dl.list_row_h;
        let max = if dl.items.is_empty() {
            dl.grid_room
        } else {
            (total - dl.list_rect.h).max(0)
        };
        dl.scroll = (dl.scroll + d * 60).clamp(0, max.max(0));
    }
    e.dirty_draw = true;
}

fn cancel(e: &mut Editor, x: &mut X) -> Request {
    let kind = e.dialog.as_ref().map(|d| d.kind.clone());
    match kind {
        Some(Kind::Confirm(_)) => {
            close(e);
            Request::None
        }
        Some(Kind::Ask(_)) => button(e, x, 0),
        _ => {
            close(e);
            Request::None
        }
    }
}

pub fn click(e: &mut Editor, x: &mut X, hit: Hit, mx: i32, my: i32, clicks: u8) -> Request {
    match hit {
        Hit::D(code) => button(e, x, code),
        Hit::DList(i) => {
            if let Some(d) = e.dialog.as_mut() {
                d.sel = i;
            }
            e.dirty_draw = true;
            let single = matches!(
                e.dialog.as_ref().map(|d| &d.kind),
                Some(Kind::Health(_)) | Some(Kind::Palette)
            );
            if clicks >= 2 || single {
                return activate_item(e, x);
            }
            Request::None
        }
        Hit::Modal => {
            let _ = (mx, my);
            Request::None
        }
        _ => Request::None,
    }
}

fn activate_item(e: &mut Editor, x: &mut X) -> Request {
    let Some(d) = e.dialog.as_ref() else {
        return Request::None;
    };
    let sel = d.sel;
    match d.kind.clone() {
        Kind::Palette => run_palette(e, x),
        Kind::Health(_) => {
            let target = d.item_codes.get(sel).copied();
            close(e);
            if let Some(t) = target {
                e.select(t);
                e.sel.clear();
                e.sel.insert(t);
            }
            Request::None
        }
        Kind::File(st) => {
            if let Some((name, is_dir)) = st.entries.get(sel).cloned() {
                if is_dir {
                    if let Some(Dialog {
                        kind: Kind::File(s2),
                        ..
                    }) = e.dialog.as_mut()
                    {
                        s2.dir = st.dir.join(name);
                    }
                    list_dir(e);
                    return Request::None;
                }
                set(e, 90, &name);
                return file_accept(e, x);
            }
            Request::None
        }
        Kind::Assets(_) => button(e, x, 1),
        _ => Request::None,
    }
}

fn apply_format(e: &mut Editor, kind: &str) {
    let target = e
        .focus
        .filter(|f| matches!(f, Fid::D(0) | Fid::D(100) | Fid::D(101) | Fid::D(102)));
    let Some(id) = target else { return };
    let Some(ed) = e.edits.get_mut(&id) else {
        return;
    };
    let text = ed.selected();
    if text.is_empty() {
        return;
    }
    let rep = crate::markup::format_selection(&text, kind);
    ed.replace_selection(&rep);
    e.dirty_draw = true;
}

/// A dialog button (or grid cell) was pressed.
pub fn button(e: &mut Editor, x: &mut X, code: u16) -> Request {
    let Some(kind) = e.dialog.as_ref().map(|d| d.kind.clone()) else {
        return Request::None;
    };
    e.dirty_draw = true;
    match kind {
        Kind::Message => {
            close(e);
        }
        Kind::Ask(then) => {
            close(e);
            if code == 1 {
                return run_then(e, x, then);
            }
        }
        Kind::Confirm(then) => match code {
            1 => {
                close(e);
                if e.save(x, Some(then.clone())) {
                    // Saved: `save` ran `then` already.
                    if e.quit_after_present {
                        e.quit_after_present = false;
                        return Request::Quit;
                    }
                }
            }
            2 => {
                close(e);
                return run_then(e, x, then);
            }
            _ => close(e),
        },
        Kind::Recovery { when, .. } => {
            close(e);
            match code {
                1 => e.restore_autosave(x, &when),
                2 => e.clear_autosave(),
                _ => {}
            }
        }
        Kind::LayoutPicker => {
            if code == 0 {
                close(e);
            } else if let Some(l) = LAYOUTS.get(code as usize - 100) {
                close(e);
                e.add_layout(l);
            }
        }
        Kind::DeckSettings => match code {
            1 => {
                let d = e.dialog.as_ref().unwrap();
                let th = d.combo_value(Cid::D(0));
                let font = d.combo_value(Cid::D(1));
                let aspect = d.combo_value(Cid::D(2));
                let tr = d.combo_value(Cid::D(3));
                let fa = d.combo_value(Cid::D(4));
                let fs = d.combo_value(Cid::D(5));
                let ck = |n: u8| d.check_value(Kid::D(n));
                let (rm, co, cc, nu, lo) = (ck(0), ck(1), ck(2), ck(3), ck(4));
                let tm = d.slider_def(Sid::D(0)).2;
                let footer = get(e, 0).trim().to_string();
                close(e);
                let data = &mut e.data;
                data.insert("theme".into(), json!(th));
                data.insert(
                    "fontFamily".into(),
                    json!(if font.is_empty() {
                        "Noto Sans".to_string()
                    } else {
                        font
                    }),
                );
                data.insert("aspect".into(), json!(aspect));
                data.insert("showNumbers".into(), json!(nu));
                data.insert("showLogo".into(), json!(lo));
                data.insert("transition".into(), json!(tr));
                data.insert("reduceMotion".into(), json!(rm));
                data.insert("cinematicOpen".into(), json!(co));
                data.insert("cinematicClose".into(), json!(cc));
                data.insert("targetMinutes".into(), json!(tm));
                data.insert("footerText".into(), json!(footer.clone()));
                data.insert("footerAlign".into(), json!(normalize_footer_align(&fa)));
                data.insert("footerSize".into(), json!(normalize_footer_size(&fs)));
                e.edits.entry(Fid::Footer).or_default().reset(&footer);
                e.mark_dirty();
                e.deck_changed();
                e.push_history();
                e.set_status("Deck settings updated", 1800);
            }
            2 => open_child(e, Kind::FileWorkflow),
            _ => close(e),
        },
        Kind::FileWorkflow => match code {
            1 => {
                let d = e.dialog.as_ref().unwrap();
                let remember = d.check_value(Kid::D(0));
                let quality = d.combo_value(Cid::D(0));
                let folder = get(e, 0);
                let folder = folder.trim();
                let folder = if !folder.is_empty() && prefs::expand(folder).is_dir() {
                    std::fs::canonicalize(prefs::expand(folder))
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                close(e);
                let fp = &mut e.file_prefs;
                fp.default_folder = folder;
                fp.remember_last = remember;
                fp.asset_quality = quality;
                if !remember {
                    fp.last_folder.clear();
                }
                fp.save();
                e.set_status("File workflow settings updated", 1800);
            }
            2 => file_dialog(e, FilePurpose::DefaultFolder),
            3 => {
                set(e, 0, "");
                set(e, 1, "");
            }
            _ => close(e),
        },
        Kind::TextEditor { two, .. } => match code {
            1 => {
                let title = get(e, 0);
                let i = e.cur;
                let layout = e
                    .current()
                    .map(|s| s_get(s, "layout", ""))
                    .unwrap_or_default();
                if two {
                    let left = get(e, 101).trim().to_string();
                    let right = get(e, 102).trim().to_string();
                    let mirror = if left.is_empty() && right.is_empty() {
                        String::new()
                    } else {
                        format!("{left}\n\n|||\n\n{right}")
                    };
                    if let Some(s) = e
                        .data
                        .get_mut("slides")
                        .and_then(|v| v.get_mut(i))
                        .and_then(|v| v.as_object_mut())
                    {
                        s.insert("title".into(), json!(title));
                        s.insert("leftColumn".into(), json!(left));
                        s.insert("rightColumn".into(), json!(right));
                        s.insert("sideStructured".into(), json!(true));
                        s.insert("twoColumnStructured".into(), json!(layout == "TWO COLUMN"));
                        s.insert("body".into(), json!(mirror));
                    }
                } else {
                    let body = get(e, 100);
                    if let Some(s) = e
                        .data
                        .get_mut("slides")
                        .and_then(|v| v.get_mut(i))
                        .and_then(|v| v.as_object_mut())
                    {
                        s.insert("title".into(), json!(title));
                        s.insert("body".into(), json!(body));
                    }
                }
                close(e);
                e.mark_dirty();
                e.refresh(i);
                e.push_history();
                e.focus = Some(Fid::Title);
            }
            11 => apply_format(e, "bold"),
            12 => apply_format(e, "italic"),
            13 => apply_format(e, "normal"),
            _ => close(e),
        },
        Kind::Palette => {
            close(e);
        }
        Kind::About(text) => match code {
            1 => {
                x.set_clipboard(e.win, &text);
                e.set_status("Diagnostics copied", 1800);
            }
            _ => close(e),
        },
        Kind::New => {
            let d = e.dialog.as_ref().unwrap();
            let pidx = d.combo_index(0);
            let profile = prefs::PROFILES
                .get(pidx)
                .map(|p| p.key)
                .unwrap_or("neutral");
            match code {
                0 => {
                    close(e);
                }
                1 => {
                    close(e);
                    if profile != e.profile {
                        e.act(x, Act::Profile(pidx as u8), None);
                    }
                    file_dialog(e, FilePurpose::ImportMd);
                }
                c if c >= 10 => {
                    let starters = prefs::profile_info(profile).starters;
                    let th = starters
                        .get((c - 10) as usize)
                        .map(|s| s.1)
                        .unwrap_or("B&W");
                    close(e);
                    if profile != e.profile {
                        e.act(x, Act::Profile(pidx as u8), None);
                    }
                    e.new_deck(x, th);
                }
                _ => {}
            }
        }
        Kind::Assets(paths) => match code {
            1 => {
                let sel = e.dialog.as_ref().map(|d| d.sel).unwrap_or(0);
                if let Some(old) = paths.get(sel).cloned() {
                    file_dialog(e, FilePurpose::ReplaceAsset(old));
                }
            }
            _ => close(e),
        },
        Kind::Health(_) => close(e),
        Kind::Overview => {
            close(e);
            if code >= 1000 {
                let i = (code - 1000) as usize;
                e.sel.clear();
                e.sel.insert(i);
                e.select(i);
            }
        }
        Kind::DisplaySetup(mons) => match code {
            1 => {
                let d = e.dialog.as_ref().unwrap();
                let (pi, ai) = (d.combo_index(0), d.combo_index(1));
                close(e);
                if let (Some(p), Some(a)) = (mons.get(pi), mons.get(ai)) {
                    prefs::save_display_prefs(&[
                        ("presenter", Some(&p.name)),
                        ("audience", Some(&a.name)),
                    ]);
                }
                return Request::Present(PresentReq {
                    start: e.cur,
                    presenter: Some(pi),
                    audience: Some(ai),
                    online: pi == ai,
                    talk: false,
                    share_check: pi == ai,
                });
            }
            2 => {
                if let Some(d) = e.dialog.as_mut() {
                    let (pi, ai) = (d.combo_index(0), d.combo_index(1));
                    if let Some(c) = d.combos.get_mut(&0) {
                        c.1 = ai;
                    }
                    if let Some(c) = d.combos.get_mut(&1) {
                        c.1 = pi;
                    }
                }
            }
            _ => close(e),
        },
        Kind::OneScreen { ask, from_start } => {
            let remember = e.dialog.as_ref().is_some_and(|d| d.check_value(Kid::D(0)));
            match code {
                20 | 21 => {
                    let choice = if code == 21 { "laptop" } else { "slides" };
                    close(e);
                    if remember {
                        prefs::save_display_prefs(&[("singleScreen", Some(choice))]);
                    } else if ask {
                        prefs::save_display_prefs(&[("singleScreen", None)]);
                    }
                    if choice == "laptop" {
                        return e.online_present(x, from_start);
                    }
                    return e.present(x, from_start, None, None, false, false, Some("slides"));
                }
                _ => close(e),
            }
        }
        Kind::File(_) => match code {
            1 => return file_accept(e, x),
            2 => {
                let up = match e.dialog.as_ref().map(|d| &d.kind) {
                    Some(Kind::File(st)) => st.dir.parent().map(Path::to_path_buf),
                    _ => None,
                };
                if let (
                    Some(up),
                    Some(Dialog {
                        kind: Kind::File(st),
                        ..
                    }),
                ) = (up, e.dialog.as_mut())
                {
                    st.dir = up;
                }
                list_dir(e);
            }
            3 => {
                if let Some(Dialog {
                    kind: Kind::File(st),
                    ..
                }) = e.dialog.as_mut()
                {
                    st.dir = prefs::home();
                }
                list_dir(e);
            }
            4 => {
                if let Some(Dialog {
                    kind: Kind::File(st),
                    ..
                }) = e.dialog.as_mut()
                {
                    st.hidden = !st.hidden;
                }
                list_dir(e);
            }
            5 => {
                // Replace the existing file.
                return file_accept(e, x);
            }
            6 => {
                if let Some(Dialog {
                    kind: Kind::File(st),
                    ..
                }) = e.dialog.as_mut()
                {
                    st.overwrite = None;
                }
            }
            _ => close(e),
        },
    }
    Request::None
}

// ---------------------------------------------------------------- draw

enum Ctl {
    Field(u8, &'static str),
    Area(u8, i32),
    Combo(u8),
    Check(u8, String),
    Slider(u8, String),
    Btn(String, u16, BtnKind),
}

enum Row {
    Head(String),
    Text(String, u32),
    Form(String, Ctl),
    Full(Ctl),
    Line(Vec<(Ctl, i32)>),
    Buttons(Vec<(String, u16, BtnKind, bool)>),
    Grid(i32, i32, Vec<(String, String, u16)>),
    List(i32),
    Pick(String),
    Space(i32),
}

fn rows(e: &Editor, d: &Dialog) -> Vec<Row> {
    let mut v = Vec::new();
    let c = &e.c;
    let ok_cancel = |ok: &str| {
        Row::Buttons(vec![
            (ok.to_string(), 1, BtnKind::Primary, false),
            ("Cancel".into(), 0, BtnKind::Normal, false),
        ])
    };
    match &d.kind {
        Kind::Message | Kind::Ask(_) | Kind::Confirm(_) | Kind::Recovery { .. } => {
            v.push(Row::Text(d.text.clone(), blend(c.fg, c.bg, 0.1)));
            v.push(Row::Space(8));
            v.push(Row::Buttons(
                d.buttons
                    .iter()
                    .enumerate()
                    .map(|(i, (l, code))| {
                        (
                            l.clone(),
                            *code,
                            if i == 0 {
                                BtnKind::Primary
                            } else {
                                BtnKind::Normal
                            },
                            false,
                        )
                    })
                    .collect(),
            ));
        }
        Kind::LayoutPicker => {
            v.push(Row::Head("CHOOSE LAYOUT".into()));
            let hints = [
                ("TITLE", "Opening / cover"),
                ("STATEMENT", "One strong idea"),
                ("TEXT", "Heading + narrative"),
                ("BULLETS", "Focused key points"),
                ("AGENDA", "Numbered agenda"),
                ("IMAGE", "Image + caption"),
                ("PHOTO", "Editorial photo"),
                ("HERO IMAGE", "Image-led story"),
                ("SPLIT", "Text + image"),
                ("TWO COLUMN", "Two text columns"),
                ("COMPARE", "A vs B / now vs next"),
                ("IMAGE + QUOTE", "Image + quotation"),
                ("QUOTE", "Editorial quotation"),
                ("BIG NUMBER", "One dominant metric"),
                ("DATA / KPI", "Primary + supporting KPIs"),
                ("NUMBER GRID", "Up to four figures"),
                ("PROCESS", "3–5 sequential steps"),
                ("TIMELINE", "Milestones / sequence"),
                ("MATRIX", "Four-part framework"),
                ("TABLE", "Compact structured data"),
                ("SECTION", "Chapter divider"),
                ("VIDEO", "Video + caption"),
                ("VIDEO + TEXT", "Video + talking points"),
                ("FULL BLEED", "Immersive image"),
                ("END", "Closing / contact"),
                ("CODE", "Code, monospace, auto-fit"),
                ("IMAGE + CAPTION", "Figure + longer caption"),
                ("THREE COLUMN", "Three text columns"),
            ];
            let cells = LAYOUTS
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let h = hints
                        .iter()
                        .find(|(n, _)| n == l)
                        .map(|(_, h)| *h)
                        .unwrap_or("");
                    (l.to_string(), h.to_string(), 100 + i as u16)
                })
                .collect();
            v.push(Row::Grid(4, 62, cells));
            v.push(Row::Buttons(vec![(
                "CANCEL".into(),
                0,
                BtnKind::Normal,
                false,
            )]));
        }
        Kind::DeckSettings => {
            let info = e.profile_info();
            v.push(Row::Form("Theme".into(), Ctl::Combo(0)));
            v.push(Row::Form("Typeface".into(), Ctl::Combo(1)));
            v.push(Row::Form("Format".into(), Ctl::Combo(2)));
            v.push(Row::Form("Transition".into(), Ctl::Combo(3)));
            v.push(Row::Form(
                "Reduce motion".into(),
                Ctl::Check(
                    0,
                    "Disable presentation motion without changing saved transitions".into(),
                ),
            ));
            v.push(Row::Form(
                "Opening".into(),
                Ctl::Check(1, "Fade in first slide from black".into()),
            ));
            v.push(Row::Form(
                "Closing".into(),
                Ctl::Check(2, "Fade out to black when ending".into()),
            ));
            let tm = d.slider_def(Sid::D(0)).2;
            v.push(Row::Form(
                "Target time".into(),
                Ctl::Slider(
                    0,
                    if tm == 0 {
                        "OFF".into()
                    } else {
                        format!("{tm} min")
                    },
                ),
            ));
            v.push(Row::Form(
                "Slide numbers".into(),
                Ctl::Check(3, "Show minimal slide numbers".into()),
            ));
            v.push(Row::Form(
                info.logo_row.into(),
                Ctl::Check(4, info.logo_checkbox.into()),
            ));
            v.push(Row::Form(
                "Footer".into(),
                Ctl::Field(0, info.footer_placeholder_short),
            ));
            v.push(Row::Form("Footer alignment".into(), Ctl::Combo(4)));
            v.push(Row::Form("Footer size".into(), Ctl::Combo(5)));
            v.push(Row::Form(
                "Files".into(),
                Ctl::Btn("FILE WORKFLOW…".into(), 2, BtnKind::Normal),
            ));
            v.push(Row::Space(6));
            v.push(ok_cancel("Save"));
        }
        Kind::FileWorkflow => {
            v.push(Row::Form("Default folder".into(), Ctl::Field(0, "")));
            v.push(Row::Form(
                "".into(),
                Ctl::Btn("CHOOSE…".into(), 2, BtnKind::Normal),
            ));
            v.push(Row::Form(
                "Open / save".into(),
                Ctl::Check(0, "Remember the last folder used".into()),
            ));
            v.push(Row::Form("Embedded images".into(), Ctl::Combo(0)));
            v.push(Row::Form(
                "Last folder".into(),
                Ctl::Field(1, "No remembered folder yet"),
            ));
            v.push(Row::Form(
                "".into(),
                Ctl::Btn("RESET TO SYSTEM DOCUMENTS".into(), 3, BtnKind::Normal),
            ));
            v.push(Row::Space(6));
            v.push(ok_cancel("Save"));
        }
        Kind::TextEditor { two, hint } => {
            v.push(Row::Text("TITLE / STATEMENT / NUMBER".into(), c.muted));
            v.push(Row::Full(Ctl::Field(0, "")));
            v.push(Row::Line(vec![
                (Ctl::Btn("B".into(), 11, BtnKind::Normal), 0),
                (Ctl::Btn("I".into(), 12, BtnKind::Normal), 0),
                (Ctl::Btn("N".into(), 13, BtnKind::Normal), 0),
            ]));
            if *two {
                v.push(Row::Text(
                    "CONTENT · edit each side directly — no separator syntax needed".into(),
                    c.muted,
                ));
                v.push(Row::Line(vec![
                    (Ctl::Area(101, 260), 1),
                    (Ctl::Area(102, 260), 1),
                ]));
                v.push(Row::Text("Blank lines add spacing · **bold** and *italic* supported · rendered exactly as written · no automatic headings".into(), c.label));
            } else {
                v.push(Row::Text(
                    "BODY / SUBTITLE / ATTRIBUTION · **bold** · *italic*".into(),
                    c.muted,
                ));
                if !hint.is_empty() {
                    v.push(Row::Text(hint.clone(), c.label));
                }
                v.push(Row::Full(Ctl::Area(100, 260)));
            }
            v.push(ok_cancel("Save"));
        }
        Kind::Palette => {
            v.push(Row::Full(Ctl::Field(0, "Type a command…")));
            v.push(Row::List(330));
        }
        Kind::About(_) => {
            v.push(Row::Head("NIRUPRES".into()));
            v.push(Row::Text(TAGLINE.into(), c.muted));
            v.push(Row::Full(Ctl::Area(100, 300)));
            v.push(Row::Buttons(vec![
                ("COPY DIAGNOSTICS".into(), 1, BtnKind::Normal, true),
                ("CLOSE".into(), 2, BtnKind::Normal, false),
            ]));
        }
        Kind::New => {
            v.push(Row::Head("NEW PRESENTATION".into()));
            v.push(Row::Form("PROFILE".into(), Ctl::Combo(0)));
            let pidx = d.combo_index(0);
            let starters = prefs::PROFILES.get(pidx).map(|p| p.starters).unwrap_or(&[]);
            for (i, (label, _)) in starters.iter().enumerate() {
                v.push(Row::Full(Ctl::Btn(
                    label.to_string(),
                    10 + i as u16,
                    BtnKind::Normal,
                )));
            }
            v.push(Row::Full(Ctl::Btn(
                "FROM MARKDOWN / AI…".into(),
                1,
                BtnKind::Normal,
            )));
            v.push(Row::Full(Ctl::Btn("CANCEL".into(), 0, BtnKind::Normal)));
        }
        Kind::Assets(paths) => {
            let total: u64 = paths
                .iter()
                .filter_map(|p| std::fs::metadata(p).ok())
                .map(|m| m.len())
                .sum();
            v.push(Row::Text(
                format!(
                    "{} unique image asset(s) · {:.1} MB current source data · embed mode {}",
                    paths.len(),
                    total as f64 / MB as f64,
                    e.file_prefs.asset_quality
                ),
                c.fg,
            ));
            v.push(Row::List(340));
            v.push(Row::Buttons(vec![
                ("REPLACE SELECTED…".into(), 1, BtnKind::Normal, true),
                ("CLOSE".into(), 0, BtnKind::Normal, false),
            ]));
        }
        Kind::Health(issues) => {
            let n = e.slides().len();
            let cnt = |k: &str| issues.iter().filter(|x| x.1 == k).count();
            v.push(Row::Text(
                format!(
                    "{n} slides  ·  {} errors  ·  {} warnings  ·  {} image requests",
                    cnt("ERROR"),
                    cnt("WARN"),
                    cnt("TODO")
                ),
                c.fg,
            ));
            if issues.is_empty() {
                v.push(Row::Text("✓ No issues found.".into(), c.fg));
            } else {
                v.push(Row::List(360));
            }
            v.push(Row::Buttons(vec![(
                "Close".into(),
                0,
                BtnKind::Normal,
                false,
            )]));
        }
        Kind::Overview => {
            let n = e.slides().len();
            v.push(Row::Text(
                format!("{}  ·  {n} slides", s_get(&e.data, "title", "Untitled")),
                c.fg,
            ));
            let cells = e
                .slides()
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let s = s.as_object().cloned().unwrap_or_default();
                    let t = s_get(&s, "title", "");
                    (
                        format!(
                            "{:02}  {}",
                            i + 1,
                            if t.is_empty() { "(untitled)".into() } else { t }
                        ),
                        format!(
                            "{}{}",
                            s_get(&s, "layout", "TITLE"),
                            if b_get(&s, "hidden") {
                                "  · HIDDEN"
                            } else {
                                ""
                            }
                        ),
                        1000 + i as u16,
                    )
                })
                .collect();
            v.push(Row::Grid(4, 80, cells));
            v.push(Row::Buttons(vec![(
                "Close".into(),
                0,
                BtnKind::Normal,
                false,
            )]));
        }
        Kind::DisplaySetup(_) => {
            v.push(Row::Head("DISPLAY SETUP".into()));
            v.push(Row::Text(
                "Choose where Presenter View and the audience presentation should appear.".into(),
                c.muted,
            ));
            v.push(Row::Form("Presenter display".into(), Ctl::Combo(0)));
            v.push(Row::Form("Audience display".into(), Ctl::Combo(1)));
            let same = d.combo_index(0) == d.combo_index(1);
            v.push(Row::Text(
                if same {
                    format!("Same display: laptop presenter mode. Presenter View fills this screen and you share the window “{}” in Teams, Meet or Zoom.", crate::present::AUDIENCE_TITLE)
                } else {
                    "Only the slide is shown on Audience. Controls, notes, timer and next-slide preview stay on Presenter.".into()
                },
                c.muted,
            ));
            v.push(Row::Full(Ctl::Btn(
                "⇄  SWAP DISPLAYS".into(),
                2,
                BtnKind::Normal,
            )));
            v.push(ok_cancel("OK"));
        }
        Kind::OneScreen { .. } => {
            v.push(Row::Head("ONE SCREEN".into()));
            v.push(Row::Text(
                format!("Only one display is connected.\n\nLaptop presenter (Teams, Meet, Zoom): your private presenter view with notes, next slide and timer fills the screen. Share the window “{}”.\n\nSlides only: the slides fill the screen, for a projector that mirrors this screen.", crate::present::AUDIENCE_TITLE),
                blend(c.fg, c.bg, 0.1),
            ));
            v.push(Row::Full(Ctl::Check(
                0,
                "Remember my choice (Presenter View… asks again)".into(),
            )));
            v.push(Row::Buttons(vec![
                ("Cancel".into(), 0, BtnKind::Normal, true),
                ("Slides only".into(), 20, BtnKind::Normal, false),
                ("Laptop presenter".into(), 21, BtnKind::Primary, false),
            ]));
        }
        Kind::File(st) => {
            v.push(Row::Line(vec![
                (Ctl::Btn("↑".into(), 2, BtnKind::Normal), 0),
                (Ctl::Btn("HOME".into(), 3, BtnKind::Normal), 0),
                (Ctl::Field(91, ""), 1),
                (
                    Ctl::Btn(
                        if st.hidden {
                            "HIDE DOTFILES".into()
                        } else {
                            "SHOW DOTFILES".into()
                        },
                        4,
                        BtnKind::Normal,
                    ),
                    0,
                ),
            ]));
            v.push(Row::List(330));
            if st.save {
                v.push(Row::Form("Name".into(), Ctl::Field(90, "")));
            } else if !st.folder {
                v.push(Row::Form(
                    "File".into(),
                    Ctl::Field(90, "Pick a file or type a path"),
                ));
            }
            v.push(Row::Text(st.filter_name.clone(), c.label));
            if let Some(p) = &st.overwrite {
                v.push(Row::Text(
                    format!(
                        "{} already exists. Do you want to replace it?",
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    ),
                    0xffb020,
                ));
                v.push(Row::Buttons(vec![
                    ("Replace".into(), 5, BtnKind::Primary, false),
                    ("Cancel".into(), 6, BtnKind::Normal, false),
                ]));
            } else {
                let ok = if st.folder {
                    "Choose"
                } else if st.save {
                    "Save"
                } else {
                    "Open"
                };
                v.push(ok_cancel(ok));
            }
        }
    }
    v
}

pub fn draw(e: &mut Editor, d: &mut Dialog) {
    let c = e.c.clone();
    let px = c.font_px;
    let (cw, ch) = (e.cv.w, e.cv.h);
    e.cv.tint(Rect::new(0, 0, cw, ch), 0, 110);
    let bh = (ui::line_h(&e.f.sans, px).ceil() as i32 + 14).max(30);
    let w = d.width.min(cw - 40);
    let pad = 18;
    let rws = rows(e, d);
    // Measure.
    let inner_w = w - 2 * pad;
    let label_w = rws
        .iter()
        .filter_map(|r| match r {
            Row::Form(l, _) => Some(ui::text_w(&e.f.sans, px, l, 0.0).ceil() as i32 + 16),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let row_h = |r: &Row, f: &crate::ui::Fonts| -> i32 {
        match r {
            Row::Head(_) => 34,
            Row::Text(t, _) => {
                let lines = ui::wrap(&f.sans, px, t, inner_w as f32);
                lines.len() as i32 * (ui::line_h(&f.sans, px).ceil() as i32 + 3) + 4
            }
            Row::Form(_, Ctl::Area(_, h)) | Row::Full(Ctl::Area(_, h)) => *h,
            Row::Line(items) => items
                .iter()
                .map(|(c, _)| if let Ctl::Area(_, h) = c { *h } else { bh })
                .max()
                .unwrap_or(bh),
            Row::Grid(cols, h, cells) => ((cells.len() as i32 + cols - 1) / cols) * (h + 8),
            Row::List(h) => *h,
            Row::Pick(_) => bh + 8,
            Row::Space(h) => *h,
            _ => bh,
        }
    };
    let title_h = 30;
    let mut total: i32 =
        title_h + pad + rws.iter().map(|r| row_h(r, &e.f) + 8).sum::<i32>() + pad - 8;
    let max_h = ch - 40;
    let mut grid_scroll_room = 0;
    if total > max_h {
        grid_scroll_room = total - max_h;
        total = max_h;
    }
    d.grid_room = grid_scroll_room;
    let r = Rect::new((cw - w) / 2, (ch - total) / 2, w, total);
    e.cv.fill(Rect::new(r.x + 4, r.y + 4, r.w, r.h), 0x000000);
    e.cv.fill(r, c.bg);
    e.cv.frame(r, c.border_strong);
    // Title bar.
    e.cv.fill(Rect::new(r.x + 1, r.y + 1, r.w - 2, title_h - 1), c.toolbar);
    e.cv.fill(Rect::new(r.x + 1, r.y + title_h, r.w - 2, 1), c.border);
    let tf = e.f.semi.clone();
    ui::text_in(
        &mut e.cv,
        &tf,
        px,
        Rect::new(r.x + 12, r.y, r.w - 24, title_h),
        &d.title,
        c.muted,
        0,
        0.0,
    );
    e.hits.push((r, Hit::Modal));
    let mut y = r.y + title_h + pad;
    let x0 = r.x + pad;
    e.cv.push_clip_rect(r.inset(1));
    for row in &rws {
        let h = if matches!(row, Row::Grid(..)) {
            row_h(row, &e.f) - grid_scroll_room
        } else {
            row_h(row, &e.f)
        };
        let h = h.max(20);
        match row {
            Row::Head(t) => {
                let f = e.f.black.clone();
                ui::text_in(
                    &mut e.cv,
                    &f,
                    px * 1.5,
                    Rect::new(x0, y, inner_w, h),
                    t,
                    blend(c.fg, 0xffffff, 0.2),
                    0,
                    1.5,
                );
            }
            Row::Text(t, col) => {
                let lines = ui::wrap(&e.f.sans, px, t, inner_w as f32);
                let lh = ui::line_h(&e.f.sans, px).ceil() as i32 + 3;
                let f = e.f.sans.clone();
                for (i, l) in lines.iter().enumerate() {
                    ui::text(
                        &mut e.cv,
                        &f,
                        px,
                        x0 as f32,
                        (y + 2 + i as i32 * lh) as f32,
                        l,
                        *col,
                    );
                }
            }
            Row::Form(l, ctl) => {
                let f = e.f.sans.clone();
                ui::text_in(
                    &mut e.cv,
                    &f,
                    px,
                    Rect::new(x0, y, label_w, bh),
                    l,
                    c.muted,
                    0,
                    0.0,
                );
                draw_ctl(e, d, ctl, Rect::new(x0 + label_w, y, inner_w - label_w, h));
            }
            Row::Full(ctl) => draw_ctl(e, d, ctl, Rect::new(x0, y, inner_w, h)),
            Row::Line(items) => {
                let fixed: i32 = items
                    .iter()
                    .filter(|(_, s)| *s == 0)
                    .map(|(c, _)| ctl_natural_w(e, c))
                    .sum();
                let stretch: i32 = items.iter().map(|(_, s)| *s).sum();
                let gaps = (items.len() as i32 - 1) * 8;
                let free = (inner_w - fixed - gaps).max(0);
                let mut xx = x0;
                for (ctl, s) in items {
                    let cw2 = if *s == 0 {
                        ctl_natural_w(e, ctl)
                    } else {
                        free * s / stretch.max(1)
                    };
                    draw_ctl(e, d, ctl, Rect::new(xx, y, cw2, h));
                    xx += cw2 + 8;
                }
            }
            Row::Buttons(btns) => {
                let mut right = x0 + inner_w;
                let mut left = x0;
                for (label, code, kind, at_left) in btns.iter().rev() {
                    let bw = {
                        let p = Paint {
                            cv: &mut Canvas::new(1, 1),
                            c: &c,
                            f: &e.f,
                            px,
                        };
                        p.button_w(label).max(112)
                    };
                    let rr = if *at_left {
                        let rr = Rect::new(left, y, bw, bh);
                        left += bw + 8;
                        rr
                    } else {
                        right -= bw;
                        let rr = Rect::new(right, y, bw, bh);
                        right -= 8;
                        rr
                    };
                    let hv = e_hover(e, Hit::D(*code));
                    let mut p = Paint {
                        cv: &mut e.cv,
                        c: &c,
                        f: &e.f,
                        px,
                    };
                    p.button(rr, label, *kind, hv, false, true, false);
                    e.hits.push((rr, Hit::D(*code)));
                }
            }
            Row::Grid(cols, ch2, cells) => {
                let gw = (inner_w - (cols - 1) * 8) / cols;
                let area = Rect::new(x0, y, inner_w, h);
                e.cv.push_clip_rect(area);
                for (i, (a, b, code)) in cells.iter().enumerate() {
                    let cx = x0 + (i as i32 % cols) * (gw + 8);
                    let cy = y + (i as i32 / cols) * (ch2 + 8) - d.scroll;
                    let rr = Rect::new(cx, cy, gw, *ch2);
                    if cy + ch2 < y || cy > y + h {
                        continue;
                    }
                    let hv = e_hover(e, Hit::D(*code));
                    e.cv.fill(rr, if hv { c.button_hover } else { c.button });
                    e.cv.frame(rr, if hv { c.muted } else { c.border_strong });
                    let fb = e.f.semi.clone();
                    let fs = e.f.sans.clone();
                    ui::text_in(
                        &mut e.cv,
                        &fb,
                        px,
                        Rect::new(rr.x + 8, rr.y + rr.h / 2 - 22, rr.w - 16, 22),
                        a,
                        c.fg,
                        1,
                        0.0,
                    );
                    ui::text_in(
                        &mut e.cv,
                        &fs,
                        px - 1.0,
                        Rect::new(rr.x + 8, rr.y + rr.h / 2, rr.w - 16, 20),
                        b,
                        c.muted,
                        1,
                        0.0,
                    );
                    e.hits.push((rr.intersect(&area), Hit::D(*code)));
                }
                e.cv.pop_clip_rect();
                e.hits.push((area, Hit::DListArea));
            }
            Row::List(_) => {
                let lr = Rect::new(x0, y, inner_w, h);
                d.list_rect = lr;
                d.list_row_h = (ui::line_h(&e.f.sans, px).ceil() as i32 + 10).max(26);
                e.cv.fill(lr, c.input);
                e.cv.frame(lr, c.border);
                e.hits.push((lr, Hit::DListArea));
                e.cv.push_clip_rect(lr.inset(1));
                let rh = d.list_row_h;
                let f = e.f.sans.clone();
                for (i, it) in d.items.iter().enumerate() {
                    let ry = lr.y + 2 + i as i32 * rh - d.scroll;
                    if ry + rh < lr.y || ry > lr.y + lr.h {
                        continue;
                    }
                    let rr = Rect::new(lr.x + 2, ry, lr.w - 4, rh);
                    let hv = e_hover(e, Hit::DList(i));
                    if i == d.sel {
                        e.cv.fill(rr, c.fg);
                    } else if hv {
                        e.cv.fill(rr, c.hover);
                    }
                    let col = if i == d.sel { c.on_accent } else { c.fg };
                    ui::text_in(
                        &mut e.cv,
                        &f,
                        px,
                        Rect::new(rr.x + 8, rr.y, rr.w - 16, rr.h),
                        it,
                        col,
                        0,
                        0.0,
                    );
                    e.hits.push((rr.intersect(&lr), Hit::DList(i)));
                }
                e.cv.pop_clip_rect();
            }
            Row::Pick(t) => {
                let rr = Rect::new(x0, y, inner_w, h);
                e.cv.fill(rr, 0x19C37D);
                let f = e.f.black.clone();
                ui::text_in(
                    &mut e.cv,
                    &f,
                    px * 1.25,
                    Rect::new(rr.x + 12, rr.y, rr.w - 24, rr.h),
                    t,
                    0x04130C,
                    0,
                    0.0,
                );
            }
            Row::Space(_) => {}
        }
        y += h + 8;
    }
    e.cv.pop_clip_rect();
}

fn e_hover(e: &Editor, h: Hit) -> bool {
    e.is_hover(h)
}

fn ctl_natural_w(e: &Editor, c: &Ctl) -> i32 {
    match c {
        Ctl::Btn(l, _, _) => {
            let p = Paint {
                cv: &mut Canvas::new(1, 1),
                c: &e.c,
                f: &e.f,
                px: e.c.font_px,
            };
            p.button_w(l).max(36)
        }
        Ctl::Check(_, l) => {
            let p = Paint {
                cv: &mut Canvas::new(1, 1),
                c: &e.c,
                f: &e.f,
                px: e.c.font_px,
            };
            p.check_w(l)
        }
        _ => 160,
    }
}

fn draw_ctl(e: &mut Editor, d: &Dialog, ctl: &Ctl, r: Rect) {
    let c = e.c.clone();
    let px = c.font_px;
    let bh = (ui::line_h(&e.f.sans, px).ceil() as i32 + 14).max(30);
    match ctl {
        Ctl::Field(n, ph) => e.field_draw(Rect::new(r.x, r.y, r.w, bh), Fid::D(*n), ph),
        Ctl::Area(n, h) => e.field_draw(Rect::new(r.x, r.y, r.w, *h), Fid::D(*n), ""),
        Ctl::Combo(n) => {
            let id = Cid::D(*n);
            let v = d.combo_value(id);
            let en = d.combo_enabled(id);
            let hv = e.is_hover(Hit::Combo(id));
            let mut p = Paint {
                cv: &mut e.cv,
                c: &c,
                f: &e.f,
                px,
            };
            p.combo(Rect::new(r.x, r.y, r.w, bh), &v, hv, false, en, false);
            if en {
                e.hits.push((Rect::new(r.x, r.y, r.w, bh), Hit::Combo(id)));
            }
        }
        Ctl::Check(n, l) => {
            let id = Kid::D(*n);
            let on = d.check_value(id);
            let en = d.check_enabled(id);
            let hv = e.is_hover(Hit::Check(id));
            let mut p = Paint {
                cv: &mut e.cv,
                c: &c,
                f: &e.f,
                px,
            };
            p.check(Rect::new(r.x, r.y, r.w, bh), l, on, hv, en);
            if en {
                e.hits.push((Rect::new(r.x, r.y, r.w, bh), Hit::Check(id)));
            }
        }
        Ctl::Slider(n, label) => {
            let id = Sid::D(*n);
            let (lo, hi, v) = d.slider_def(id);
            let hv = e.is_hover(Hit::Slider(id));
            let lw = 70;
            let sr = Rect::new(r.x, r.y, r.w - lw - 8, bh);
            let mut p = Paint {
                cv: &mut e.cv,
                c: &c,
                f: &e.f,
                px,
            };
            p.slider(sr, lo, hi, v, hv, false);
            p.label(Rect::new(r.x + r.w - lw, r.y, lw, bh), label, c.fg);
            e.hits.push((sr, Hit::Slider(id)));
        }
        Ctl::Btn(l, code, kind) => {
            let rr = Rect::new(r.x, r.y, r.w, bh);
            let hv = e.is_hover(Hit::D(*code));
            let mut p = Paint {
                cv: &mut e.cv,
                c: &c,
                f: &e.f,
                px,
            };
            p.button(rr, l, *kind, hv, false, true, false);
            e.hits.push((rr, Hit::D(*code)));
        }
    }
}
