//! Presenting: the audience window (only the slides) and the private
//! presenter view (current and next slide, timer, pace, clock, notes), with
//! fades, transitions, Spotlight, Focus Reveal, Live Zoom, pause, curtain,
//! black/white screen, the share check and the NIRUWM integration.
use crate::canvas::{Canvas, RasterPainter, Rect};
use crate::chrome::{blend, Chrome};
use crate::model::*;
use crate::paint::{RectF, Rgba};
use crate::render::{self, Ctx};
use crate::ui::{self, BtnKind, Fonts, Paint};
use crate::x::{self, Ev, Monitor, X};
use std::time::{Duration, Instant};

pub const AUDIENCE_TITLE: &str = "AUDIENCE · share this · NIRUPRES";
pub const PRESENTER_TITLE: &str = "PRIVATE presenter · do not share";
const PRIVATE_RED: u32 = 0xC4161C;
const SHARE_GREEN: u32 = 0x19C37D;
const OPEN_FADE_MS: u64 = 900;
const CLOSE_FADE_MS: u64 = 800;
const BLACK_HOLD_MS: u64 = 1000;
const OPEN_SETTLE_MS: u64 = 160;
const OPEN_WAIT_MAX_MS: u64 = 1500;
const SHARE_FLASH_MS: u64 = 5000;
const KEN_BURNS_MS: f32 = 18000.0;

fn out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}
fn in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}
fn in_out_sine(t: f32) -> f32 {
    -((std::f32::consts::PI * t).cos() - 1.0) / 2.0
}

fn frac(start: Instant, ms: u64) -> f32 {
    (start.elapsed().as_secs_f32() * 1000.0 / ms.max(1) as f32).clamp(0.0, 1.0)
}

struct Trans {
    snap: Canvas,
    mode: String,
    start: Instant,
    dur: u64,
}

#[derive(Clone, Copy)]
struct Cover {
    from: f32,
    to: f32,
    start: Instant,
    dur: u64,
    black_reached: bool,
}

impl Cover {
    fn value(&self) -> f32 {
        let t = in_out_sine(frac(self.start, self.dur));
        self.from + (self.to - self.from) * t
    }
    fn done(&self) -> bool {
        self.start.elapsed() >= Duration::from_millis(self.dur)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FocusMode {
    Spotlight,
    Focus,
}

struct Zoom {
    snap: Canvas,
    from: RectF,
    to: RectF,
    start: Instant,
    dur: u64,
    out: bool,
}

pub struct Win {
    pub id: u32,
    pub cv: Canvas,
    pub mapped: bool,
    pub dirty: bool,
    exposed_at: Option<Instant>,
}

impl Win {
    fn new(id: u32, w: i32, h: i32) -> Self {
        Self {
            id,
            cv: Canvas::new(w, h),
            mapped: false,
            dirty: true,
            exposed_at: None,
        }
    }
}

#[derive(PartialEq)]
pub enum Outcome {
    Running,
    Finished,
}

pub struct Session {
    pub data: Obj,
    pub i: usize,
    pub online: bool,
    pub talk: bool,
    pub dual: bool,
    pub preview_only: bool,
    pub audience: Win,
    asize: (i32, i32),
    pub presenter: Option<Win>,
    audience_mon: Monitor,
    presenter_mon: Monitor,
    c: Chrome,
    f: Fonts,
    // State.
    blank: Option<char>,
    curtain: bool,
    paused: bool,
    pause_veil: Option<(Instant, bool)>,
    pause_clock: Instant,
    paused_total: Duration,
    pause_started: Option<Instant>,
    clock: Instant,
    jump: String,
    focus_mode: Option<FocusMode>,
    point: Option<(f32, f32)>,
    radius: f32,
    pinned: bool,
    pin_anim: Option<((f32, f32), (f32, f32), Instant)>,
    zoom: Option<Zoom>,
    zoomed: bool,
    trans: Option<Trans>,
    cover: Option<Cover>,
    opening: Option<Instant>,
    opening_wait: Option<Instant>,
    closing_started: bool,
    black_hold: Option<Instant>,
    end_black: bool,
    closing: bool,
    share_checking: bool,
    share_flash: Option<Instant>,
    share_flash_req: Option<Instant>,
    share_status_at: Instant,
    audience_viewable: Option<bool>,
    last_watch: Instant,
    slide_frame: Option<(usize, i32, i32, Canvas)>,
    motion_start: Instant,
    motion_key: String,
    cursor_hide_at: Option<Instant>,
    cursor_hidden: bool,
    pub notes_px: i64,
    notes_scroll: i32,
    pub mode: crate::presenting::PresentingMode,
    pub video: Option<crate::video::Video>,
    pause_video_was_playing: bool,
    hits: Vec<(Rect, PHit)>,
    hover: Option<PHit>,
    pub slide_changed: Option<usize>,
    last_marked: Option<usize>,
    last_second: u64,
    preview_rect: RectF,
    pub transition_only: Option<(usize, Instant)>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum PHit {
    Prev,
    End,
    Next,
    Smaller,
    Larger,
    Current,
    Flash,
    Cancel,
    Start,
    Stop,
}

pub struct Config {
    pub start: usize,
    pub presenter: Monitor,
    pub audience: Monitor,
    pub online: bool,
    pub talk: bool,
    pub share_check: bool,
}

fn visible(data: &Obj, i: usize) -> bool {
    Deck(data)
        .slides()
        .get(i)
        .and_then(|s| s.as_object())
        .is_some_and(|s| !b_get(s, "hidden"))
}

fn nearest_visible(data: &Obj, i: usize, d: i64) -> usize {
    let n = Deck(data).slides().len();
    if n == 0 {
        return 0;
    }
    let i = i.min(n - 1);
    if visible(data, i) {
        return i;
    }
    for step in 1..n as i64 {
        let j = i as i64 + step * d;
        if j >= 0 && (j as usize) < n && visible(data, j as usize) {
            return j as usize;
        }
    }
    (0..n).find(|&j| visible(data, j)).unwrap_or(i)
}

fn next_visible(data: &Obj, i: usize, d: i64) -> usize {
    let n = Deck(data).slides().len() as i64;
    let mut j = i as i64 + d;
    while j >= 0 && j < n {
        if visible(data, j as usize) {
            return j as usize;
        }
        j += d;
    }
    i
}

impl Session {
    pub fn start(x: &mut X, data: Obj, cfg: Config, c: Chrome) -> x::Res<Self> {
        let f = Fonts::load().ok_or("no font")?;
        let dual = cfg.online
            || cfg.presenter.name != cfg.audience.name
            || (cfg.presenter.x, cfg.presenter.y) != (cfg.audience.x, cfg.audience.y);
        let start = nearest_visible(&data, cfg.start, 1);
        // The audience window.
        let am = cfg.audience.clone();
        let (ax, ay, aw, ah) = if cfg.online {
            let aw = (640).max(1280.min((am.w as f32 * 0.86) as i32));
            let mut ah = aw * 9 / 16;
            let mut aw = aw;
            if ah as f32 > am.h as f32 * 0.9 {
                ah = 360.max((am.h as f32 * 0.9) as i32);
                aw = ah * 16 / 9;
            }
            (am.x + (am.w - aw) / 2, am.y + (am.h - ah) / 2, aw, ah)
        } else {
            (am.x, am.y, am.w, am.h)
        };
        let aid = x.create(
            ax,
            ay,
            aw as u16,
            ah as u16,
            AUDIENCE_TITLE,
            "nirupres-audience",
            "NIRUPRES",
            true,
            true,
            false,
        )?;
        x::register_window(aid);
        if !cfg.online {
            x.initial_state(aid, true, false);
        }
        let mut audience = Win::new(aid, aw, ah);
        x.map(aid);
        audience.mapped = false;
        let mut presenter = None;
        if dual {
            let pm = cfg.presenter.clone();
            let pid = x.create(
                pm.x,
                pm.y,
                pm.w.max(640) as u16,
                pm.h.max(400) as u16,
                PRESENTER_TITLE,
                "nirupres-presenter",
                "NIRUPRES",
                false,
                true,
                false,
            )?;
            x::register_window(pid);
            x.initial_state(pid, !cfg.online, cfg.online);
            x.map(pid);
            presenter = Some(Win::new(pid, pm.w, pm.h));
        }
        let default_px = ((cfg.presenter.h as f32 * 0.028).round() as i64).clamp(20, 40);
        let now = Instant::now();
        let mut s = Self {
            data,
            i: start,
            online: cfg.online,
            talk: cfg.talk,
            dual,
            preview_only: false,
            audience,
            asize: (aw, ah),
            presenter,
            audience_mon: cfg.audience,
            presenter_mon: cfg.presenter,
            c,
            f,
            blank: None,
            curtain: false,
            paused: false,
            pause_veil: None,
            pause_clock: now,
            paused_total: Duration::ZERO,
            pause_started: None,
            clock: now,
            jump: String::new(),
            focus_mode: None,
            point: None,
            radius: 165.0,
            pinned: false,
            pin_anim: None,
            zoom: None,
            zoomed: false,
            trans: None,
            cover: None,
            opening: None,
            opening_wait: None,
            closing_started: false,
            black_hold: None,
            end_black: false,
            closing: false,
            share_checking: cfg.share_check,
            share_flash: None,
            share_flash_req: if cfg.share_check {
                Some(now + Duration::from_millis(250))
            } else {
                None
            },
            share_status_at: now,
            audience_viewable: None,
            last_watch: now,
            slide_frame: None,
            motion_start: now,
            motion_key: String::new(),
            cursor_hide_at: if cfg.online {
                None
            } else {
                Some(now + Duration::from_millis(1500))
            },
            cursor_hidden: false,
            notes_px: crate::prefs::notes_px(default_px),
            notes_scroll: 0,
            mode: Default::default(),
            video: None,
            pause_video_was_playing: false,
            hits: Vec::new(),
            hover: None,
            slide_changed: Some(start),
            last_marked: None,
            last_second: u64::MAX,
            preview_rect: RectF::default(),
            transition_only: None,
        };
        let open_now = s.cinematic_open() && !cfg.share_check;
        if open_now {
            s.begin_opening();
        }
        s.mode.start(std::process::id());
        Ok(s)
    }

    /// A windowed audience-only preview of the transition into `to`.
    pub fn transition_preview(
        x: &mut X,
        data: Obj,
        from: usize,
        to: usize,
        c: Chrome,
        mon: Monitor,
    ) -> x::Res<Self> {
        let cfg = Config {
            start: from,
            presenter: mon.clone(),
            audience: mon,
            online: true,
            talk: false,
            share_check: false,
        };
        let f = Fonts::load().ok_or("no font")?;
        let (aw, ah) = (960, 540);
        let id = x.create(
            cfg.audience.x + (cfg.audience.w - aw) / 2,
            cfg.audience.y + (cfg.audience.h - ah) / 2,
            aw as u16,
            ah as u16,
            "NIRUPRES — Transition Preview",
            "nirupres-preview",
            "NIRUPRES",
            false,
            false,
            false,
        )?;
        x::register_window(id);
        x.map(id);
        let now = Instant::now();
        Ok(Self {
            data,
            i: from,
            online: true,
            talk: false,
            dual: false,
            preview_only: true,
            audience: Win::new(id, aw, ah),
            asize: (aw, ah),
            presenter: None,
            audience_mon: cfg.audience.clone(),
            presenter_mon: cfg.audience,
            c,
            f,
            blank: None,
            curtain: false,
            paused: false,
            pause_veil: None,
            pause_clock: now,
            paused_total: Duration::ZERO,
            pause_started: None,
            clock: now,
            jump: String::new(),
            focus_mode: None,
            point: None,
            radius: 165.0,
            pinned: false,
            pin_anim: None,
            zoom: None,
            zoomed: false,
            trans: None,
            cover: None,
            opening: None,
            opening_wait: None,
            closing_started: false,
            black_hold: None,
            end_black: false,
            closing: false,
            share_checking: false,
            share_flash: None,
            share_flash_req: None,
            share_status_at: now,
            audience_viewable: None,
            last_watch: now,
            slide_frame: None,
            motion_start: now,
            motion_key: String::new(),
            cursor_hide_at: None,
            cursor_hidden: false,
            notes_px: 20,
            notes_scroll: 0,
            mode: Default::default(),
            video: None,
            pause_video_was_playing: false,
            hits: Vec::new(),
            hover: None,
            slide_changed: None,
            last_marked: None,
            last_second: u64::MAX,
            preview_rect: RectF::default(),
            transition_only: Some((to, now + Duration::from_millis(250))),
        })
    }

    pub fn owns(&self, win: u32) -> bool {
        self.audience.id == win || self.presenter.as_ref().is_some_and(|p| p.id == win)
    }

    fn deck(&self) -> Deck<'_> {
        Deck(&self.data)
    }

    fn reduce_motion(&self) -> bool {
        self.deck().reduce_motion()
    }

    fn cinematic_open(&self) -> bool {
        self.deck().flag_default_true("cinematicOpen") && !self.reduce_motion()
    }

    fn cinematic_close(&self) -> bool {
        self.deck().flag_default_true("cinematicClose") && !self.reduce_motion()
    }

    fn slide(&self, i: usize) -> Obj {
        self.deck()
            .slides()
            .get(i)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default()
    }

    fn transition_for(&self, i: usize) -> String {
        if self.reduce_motion() {
            return "NONE".into();
        }
        let s = self.slide(i);
        let t = normalize_transition(
            s.get("transition").or(Some(&serde_json::json!("DECK"))),
            true,
        );
        if t == "DECK" {
            normalize_transition(self.data.get("transition"), false)
        } else {
            t
        }
    }

    fn aspect(&self) -> String {
        self.deck().aspect()
    }

    fn view_rect(&self) -> RectF {
        render::aspect_rect(
            RectF::new(0.0, 0.0, self.asize.0 as f32, self.asize.1 as f32),
            &self.aspect(),
        )
    }

    fn motion_wanted(&self) -> bool {
        if self.blank.is_some() || self.paused || self.curtain || self.reduce_motion() {
            return false;
        }
        let s = self.slide(self.i);
        truthy(s.get("backgroundImage"))
            && s_get(&s, "backgroundMotion", "NONE") == "KEN BURNS"
            && self.motion_start.elapsed().as_secs_f32() * 1000.0 < KEN_BURNS_MS
    }

    /// The slide data with the Ken Burns pan applied.
    fn render_slide_data(&mut self) -> Obj {
        let mut s = self.slide(self.i);
        if truthy(s.get("backgroundImage"))
            && s_get(&s, "backgroundMotion", "NONE") == "KEN BURNS"
            && !self.reduce_motion()
        {
            let key = {
                let id = s_get(&s, "id", "");
                if id.is_empty() {
                    self.i.to_string()
                } else {
                    id
                }
            };
            if self.motion_key != key {
                self.motion_key = key.clone();
                self.motion_start = Instant::now();
            }
            let phase =
                (self.motion_start.elapsed().as_secs_f32() * 1000.0 / KEN_BURNS_MS).min(1.0);
            let ease = phase * phase * (3.0 - 2.0 * phase);
            let dir = if key.chars().map(|c| c as u32).sum::<u32>() % 2 == 1 {
                -1.0
            } else {
                1.0
            };
            let drift = (ease - 0.5) * 0.12 * dir;
            let fx = (f_get(&s, "backgroundFocalX", 0.5) as f32 + drift).clamp(0.0, 1.0);
            let fy = (f_get(&s, "backgroundFocalY", 0.5) as f32 - drift * 0.35).clamp(0.0, 1.0);
            s.insert("backgroundFocalX".into(), serde_json::json!(fx));
            s.insert("backgroundFocalY".into(), serde_json::json!(fy));
            s.insert(
                "_backgroundZoom".into(),
                serde_json::json!(1.0 + 0.085 * ease),
            );
        }
        s
    }

    /// The current slide drawn at the audience size (cached).
    fn frame(&mut self) -> &Canvas {
        let (w, h) = self.asize;
        let moving = self.motion_wanted();
        let fresh = self
            .slide_frame
            .as_ref()
            .is_some_and(|(i, fw, fh, _)| *i == self.i && *fw == w && *fh == h);
        if !fresh || moving {
            let s = self.render_slide_data();
            let mut cv = match self.slide_frame.take() {
                Some((_, fw, fh, cv)) if fw == w && fh == h => cv,
                _ => Canvas::new(w, h),
            };
            cv.reset_clip();
            cv.fill(cv.bounds(), 0x0B0B0B);
            let r = self.view_rect();
            cv.fill_aa(r.adjusted(-1.0, -1.0, 1.0, 1.0), Rgba::hex(0x2A2A2A));
            let style = DeckStyle::of(&self.data);
            let ctx = Ctx::new(&style, self.i, style.show_numbers);
            {
                let mut p = RasterPainter { cv: &mut cv };
                render::render_slide(&mut p, r, &s, &ctx);
            }
            self.slide_frame = Some((self.i, w, h, cv));
        }
        &self.slide_frame.as_ref().unwrap().3
    }

    // ------------------------------------------------------------ fades

    fn begin_opening(&mut self) {
        self.cover = Some(Cover {
            from: 1.0,
            to: 1.0,
            start: Instant::now(),
            dur: 1,
            black_reached: true,
        });
        self.opening = Some(Instant::now());
        self.opening_wait = None;
    }

    fn start_opening_fade(&mut self) {
        let from = self.cover.map(|c| c.value()).unwrap_or(1.0);
        self.cover = Some(Cover {
            from,
            to: 0.0,
            start: Instant::now(),
            dur: ((OPEN_FADE_MS as f32 * from).max(1.0)) as u64,
            black_reached: false,
        });
    }

    fn cancel_opening(&mut self) {
        self.opening = None;
        self.opening_wait = None;
        self.cover = None;
    }

    fn begin_closing(&mut self) {
        self.opening = None;
        self.opening_wait = None;
        let from = self.cover.map(|c| c.value()).unwrap_or(0.0);
        self.cover = Some(Cover {
            from,
            to: 1.0,
            start: Instant::now(),
            dur: ((CLOSE_FADE_MS as f32 * (1.0 - from)).max(1.0)) as u64,
            black_reached: false,
        });
    }

    fn fade_in_again(&mut self) {
        let from = self.cover.map(|c| c.value()).unwrap_or(1.0);
        self.cover = Some(Cover {
            from,
            to: 0.0,
            start: Instant::now(),
            dur: ((OPEN_FADE_MS as f32 * from).max(1.0)) as u64,
            black_reached: false,
        });
    }

    // --------------------------------------------------------- control

    fn active_elapsed(&self) -> Duration {
        let cur = if self.paused {
            self.pause_started.map(|p| p.elapsed()).unwrap_or_default()
        } else {
            Duration::ZERO
        };
        self.clock
            .elapsed()
            .saturating_sub(self.paused_total)
            .saturating_sub(cur)
    }

    fn set_index(&mut self, x: &X, new_i: usize) {
        let n = self.deck().slides().len();
        let new_i = new_i.min(n.saturating_sub(1));
        if new_i == self.i {
            return;
        }
        let mode = self.transition_for(new_i);
        let snap = if self.blank.is_none() && mode != "NONE" {
            Some(self.composite_clean())
        } else {
            None
        };
        if self.opening.is_some() {
            self.cancel_opening();
        }
        self.clear_effects();
        self.i = new_i;
        self.motion_start = Instant::now();
        self.motion_key.clear();
        self.sync_video(x);
        if let Some(snap) = snap {
            let dur = match mode.as_str() {
                "MORPH" => 560,
                "REVEAL" => 460,
                _ => 360,
            };
            self.trans = Some(Trans {
                snap,
                mode,
                start: Instant::now(),
                dur,
            });
        }
        self.slide_changed = Some(new_i);
        self.dirty_all();
    }

    fn composite_clean(&mut self) -> Canvas {
        let f = self.frame();
        let mut c = Canvas::new(f.w, f.h);
        c.px.copy_from_slice(&f.px);
        c
    }

    fn clear_effects(&mut self) {
        self.zoom = None;
        self.zoomed = false;
        self.pinned = false;
        self.pin_anim = None;
    }

    fn dirty_all(&mut self) {
        self.audience.dirty = true;
        if let Some(p) = &mut self.presenter {
            p.dirty = true;
        }
    }

    fn navigate(&mut self, x: &X, d: i64) -> Outcome {
        if self.closing_started || self.closing || self.black_hold.is_some() {
            return Outcome::Running;
        }
        if self.end_black {
            if d == 1 {
                return self.finish(x);
            }
            self.end_black = false;
            self.fade_in_again();
            self.dirty_all();
            if d == -1 {
                return Outcome::Running;
            }
        } else if d == 1 && next_visible(&self.data, self.i, 1) == self.i && self.cinematic_close()
        {
            self.end_black = true;
            self.begin_closing();
            self.dirty_all();
            return Outcome::Running;
        }
        let n = self.deck().slides().len();
        let target = if d == -999 {
            nearest_visible(&self.data, 0, 1)
        } else if d == 999 {
            nearest_visible(&self.data, n.saturating_sub(1), -1)
        } else if d >= 10000 {
            nearest_visible(
                &self.data,
                ((d - 10000) as usize).min(n.saturating_sub(1)),
                1,
            )
        } else {
            next_visible(&self.data, self.i, if d > 0 { 1 } else { -1 })
        };
        if self.blank.is_some() {
            self.blank = None;
        }
        self.set_index(x, target);
        self.dirty_all();
        Outcome::Running
    }

    fn set_blank(&mut self, x: &X, m: Option<char>) {
        self.blank = m;
        if m.is_some() {
            if let Some(v) = self.video.take() {
                v.stop(x);
            }
        } else {
            self.sync_video(x);
        }
        self.dirty_all();
    }

    fn toggle_pause(&mut self) {
        if self.closing {
            return;
        }
        if !self.paused {
            self.blank = None;
            self.curtain = false;
            self.paused = true;
            self.pause_started = Some(Instant::now());
            self.pause_clock = Instant::now();
            self.pause_veil = Some((Instant::now(), true));
            self.pause_video_was_playing = false;
            if let Some(v) = &mut self.video {
                self.pause_video_was_playing = v.playing;
                if v.playing {
                    v.set_pause(true);
                }
                v.set_mute(true);
            }
        } else {
            if let Some(p) = self.pause_started.take() {
                self.paused_total += p.elapsed();
            }
            self.paused = false;
            self.pause_veil = Some((Instant::now(), false));
            let muted = b_get(&self.slide(self.i), "videoMuted");
            if let Some(v) = &mut self.video {
                if self.pause_video_was_playing {
                    v.set_pause(false);
                }
                v.set_mute(muted);
            }
            self.pause_video_was_playing = false;
        }
        self.dirty_all();
    }

    fn set_focus_mode(&mut self, m: FocusMode) {
        if self.focus_mode == Some(m) {
            self.focus_mode = None;
        } else {
            self.focus_mode = Some(m);
            self.radius = if m == FocusMode::Spotlight {
                165.0
            } else {
                115.0
            };
            if self.point.is_none() {
                let (w, h) = (self.asize.0 as f32, self.asize.1 as f32);
                self.point = Some((w / 2.0, h / 2.0));
            }
        }
        self.pinned = false;
        self.pin_anim = None;
        self.cursor_activity();
        self.dirty_all();
    }

    fn adjust_radius(&mut self, d: f32) {
        self.radius = (self.radius + d).clamp(65.0, 420.0);
        self.dirty_all();
    }

    fn aim(&mut self, p: (f32, f32)) {
        if self.pinned {
            return;
        }
        self.point = Some(p);
        if self.focus_mode.is_some() {
            self.dirty_all();
        }
    }

    fn pin_or_move(&mut self, p: (f32, f32)) -> bool {
        if self.focus_mode.is_none() {
            return false;
        }
        if !self.pinned {
            self.pinned = true;
            self.point = Some(p);
        } else {
            let from = self.current_point().unwrap_or(p);
            self.pin_anim = Some((from, p, Instant::now()));
        }
        self.dirty_all();
        true
    }

    fn current_point(&self) -> Option<(f32, f32)> {
        if let Some((a, b, t)) = self.pin_anim {
            let k = in_out_cubic(frac(t, 360));
            return Some((a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k));
        }
        self.point
    }

    fn toggle_zoom(&mut self) {
        let r = RectF::new(0.0, 0.0, self.asize.0 as f32, self.asize.1 as f32);
        if self.zoomed {
            if let Some(z) = self.zoom.take() {
                let from = self.zoom_rect_now(&z);
                self.zoom = Some(Zoom {
                    snap: z.snap,
                    from,
                    to: r,
                    start: Instant::now(),
                    dur: 280,
                    out: true,
                });
            }
            self.zoomed = false;
        } else {
            let snap = self.composite_clean();
            let pt = self.point.unwrap_or(r.center());
            let scale = 1.75;
            let nw = (r.w * scale).trunc();
            let nh = (r.h * scale).trunc();
            let mut tx = (r.w / 2.0 - (pt.0 / r.w.max(1.0)) * nw).trunc();
            let mut ty = (r.h / 2.0 - (pt.1 / r.h.max(1.0)) * nh).trunc();
            tx = tx.max(r.w - nw).min(0.0);
            ty = ty.max(r.h - nh).min(0.0);
            self.zoom = Some(Zoom {
                snap,
                from: r,
                to: RectF::new(tx, ty, nw, nh),
                start: Instant::now(),
                dur: 420,
                out: false,
            });
            self.zoomed = true;
        }
        self.dirty_all();
    }

    fn zoom_rect_now(&self, z: &Zoom) -> RectF {
        let k = in_out_cubic(frac(z.start, z.dur));
        let l = |a: f32, b: f32| a + (b - a) * k;
        RectF::new(
            l(z.from.x, z.to.x),
            l(z.from.y, z.to.y),
            l(z.from.w, z.to.w),
            l(z.from.h, z.to.h),
        )
    }

    fn cursor_activity(&mut self) {
        if self.online {
            return;
        }
        self.cursor_hide_at = Some(Instant::now() + Duration::from_millis(1500));
    }

    /// Escape: fade out, then a short black hold; Escape again closes now.
    pub fn close_session(&mut self, x: &X) -> Outcome {
        if self.black_hold.is_some() || self.closing_started || self.end_black {
            return self.finish(x);
        }
        if self.closing {
            return Outcome::Running;
        }
        if self.cinematic_close() {
            self.closing_started = true;
            self.begin_closing();
            self.dirty_all();
            return Outcome::Running;
        }
        self.finish(x)
    }

    pub fn finish(&mut self, x: &X) -> Outcome {
        if self.closing {
            return Outcome::Finished;
        }
        self.closing = true;
        if let Some(v) = self.video.take() {
            v.stop(x);
        }
        x.destroy(self.audience.id);
        if let Some(p) = &self.presenter {
            x.destroy(p.id);
        }
        self.mode.stop();
        Outcome::Finished
    }

    fn notes_zoom(&mut self, d: i64) {
        if !self.dual {
            return;
        }
        self.notes_px = (self.notes_px + d).clamp(12, 64);
        crate::prefs::save_notes_px(self.notes_px);
        self.dirty_all();
    }

    fn share_check_done(&mut self, accepted: bool, x: &X) -> Outcome {
        self.share_checking = false;
        if !accepted {
            return self.finish(x);
        }
        self.clock = Instant::now();
        self.paused_total = Duration::ZERO;
        if self.cinematic_open() {
            self.begin_opening();
        }
        if let Some(p) = &self.presenter {
            x.raise_focus(p.id);
        }
        self.dirty_all();
        Outcome::Running
    }

    fn handle_key(&mut self, x: &X, sym: u32, state: u16, ch: Option<char>) -> Outcome {
        use crate::keys::*;
        let sym = normalize(sym);
        let ctrl = state & x::CTRL != 0;
        if self.share_checking {
            return match sym {
                K_RETURN => self.share_check_done(true, x),
                K_ESCAPE => self.share_check_done(false, x),
                _ => Outcome::Running,
            };
        }
        if ctrl {
            match sym {
                0x2d | 0xffad => self.notes_zoom(-2),
                0x2b | 0x3d | 0xffab => self.notes_zoom(2),
                _ => {}
            }
            return Outcome::Running;
        }
        let c = ch.map(|c| c.to_ascii_lowercase());
        if sym == K_ESCAPE {
            return self.close_session(x);
        }
        if c == Some('p') {
            self.toggle_pause();
            return Outcome::Running;
        }
        if self.paused {
            return Outcome::Running;
        }
        match c {
            Some('b') => {
                let m = if self.blank == Some('B') {
                    None
                } else {
                    Some('B')
                };
                self.set_blank(x, m);
                return Outcome::Running;
            }
            Some('w') => {
                let m = if self.blank == Some('W') {
                    None
                } else {
                    Some('W')
                };
                self.set_blank(x, m);
                return Outcome::Running;
            }
            Some('s') => {
                self.set_focus_mode(FocusMode::Spotlight);
                return Outcome::Running;
            }
            Some('f') => {
                self.set_focus_mode(FocusMode::Focus);
                return Outcome::Running;
            }
            Some('z') => {
                self.toggle_zoom();
                return Outcome::Running;
            }
            Some('c') => {
                self.curtain = !self.curtain;
                self.dirty_all();
                return Outcome::Running;
            }
            Some('[') => {
                self.adjust_radius(-24.0);
                return Outcome::Running;
            }
            Some(']') => {
                self.adjust_radius(24.0);
                return Outcome::Running;
            }
            _ => {}
        }
        if self.blank.is_some() {
            self.set_blank(x, None);
        }
        if let Some(d) = ch.filter(|c| c.is_ascii_digit()) {
            self.jump.push(d);
            return Outcome::Running;
        }
        if sym == K_RETURN && !self.jump.is_empty() {
            let n: i64 = self.jump.parse().unwrap_or(1);
            self.jump.clear();
            return self.navigate(x, 10000 + n - 1);
        }
        if sym == 0x20 {
            if let Some(v) = &mut self.video {
                v.toggle();
                return Outcome::Running;
            }
        }
        match sym {
            K_RIGHT | 0x20 | K_NEXT | K_DOWN => self.navigate(x, 1),
            K_LEFT | K_PRIOR | K_UP => self.navigate(x, -1),
            K_HOME => self.navigate(x, -999),
            K_END => self.navigate(x, 999),
            _ => Outcome::Running,
        }
    }

    fn to_audience(&self, p: (i32, i32)) -> Option<(f32, f32)> {
        let src = self.preview_rect;
        let dst = self.view_rect();
        if src.w < 1.0 || src.h < 1.0 {
            return None;
        }
        let nx = ((p.0 as f32 - src.x) / src.w).clamp(0.0, 1.0);
        let ny = ((p.1 as f32 - src.y) / src.h).clamp(0.0, 1.0);
        Some((dst.x + nx * dst.w, dst.y + ny * dst.h))
    }

    pub fn event(&mut self, x: &mut X, ev: &Ev) -> Outcome {
        match ev {
            Ev::Configure(w, _, _, cw, ch) => {
                let win = if *w == self.audience.id {
                    Some(&mut self.audience)
                } else {
                    self.presenter.as_mut().filter(|p| p.id == *w)
                };
                if let Some(win) = win {
                    if *cw != win.cv.w || *ch != win.cv.h {
                        win.cv.resize(*cw, *ch);
                        win.dirty = true;
                    }
                }
                if *w == self.audience.id {
                    self.asize = (*cw, *ch);
                    self.slide_frame = None;
                    self.zoom = None;
                    self.zoomed = false;
                    self.trans = None;
                    self.sync_video(x);
                }
                self.dirty_all();
            }
            Ev::Expose(w) => {
                if *w == self.audience.id {
                    self.audience.dirty = true;
                    if self.audience.exposed_at.is_none() {
                        self.audience.exposed_at = Some(Instant::now());
                    }
                } else if let Some(p) = &mut self.presenter {
                    p.dirty = true;
                }
            }
            Ev::Mapped(w, m) => {
                if *w == self.audience.id {
                    self.audience.mapped = *m;
                    if *m {
                        if !self.dual {
                            x.raise_focus(self.audience.id);
                        }
                        self.sync_video(x);
                    }
                } else if let Some(p) = &mut self.presenter {
                    if p.id == *w {
                        p.mapped = *m;
                        if *m {
                            x.raise_focus(p.id);
                        }
                    }
                }
                self.dirty_all();
            }
            Ev::Focus(w, true) => {
                if *w == self.audience.id && self.online && self.dual && !self.closing {
                    // The window manager gave focus to the covered audience
                    // window (a workspace switch): give it back.
                    if let Some(p) = &self.presenter {
                        x.raise_focus(p.id);
                    }
                }
            }
            Ev::Close(_) => {
                if self.preview_only {
                    return self.finish(x);
                }
                return self.close_session(x);
            }
            Ev::Key { sym, state, ch, .. } => {
                if self.preview_only {
                    if *sym == crate::keys::K_ESCAPE {
                        return self.finish(x);
                    }
                    return Outcome::Running;
                }
                return self.handle_key(x, *sym, *state, *ch);
            }
            Ev::Motion {
                win, x: mx, y: my, ..
            } => {
                if *win == self.audience.id {
                    self.aim((*mx as f32, *my as f32));
                    self.cursor_activity();
                    if self.cursor_hidden {
                        self.cursor_hidden = false;
                        x.set_cursor(self.audience.id, x.arrow_cursor);
                    }
                } else {
                    let h = self
                        .hits
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains(*mx, *my))
                        .map(|(_, h)| *h);
                    if h != self.hover {
                        self.hover = h;
                        if let Some(p) = &mut self.presenter {
                            p.dirty = true;
                        }
                    }
                    if h == Some(PHit::Current) {
                        if let Some(q) = self.to_audience((*mx, *my)) {
                            self.aim(q);
                        }
                    }
                }
            }
            Ev::Button {
                win,
                x: bx,
                y: by,
                button,
                press: true,
                ..
            } => {
                if self.preview_only {
                    return Outcome::Running;
                }
                if *win == self.audience.id {
                    if *button == 1 && self.focus_mode.is_some() {
                        self.pin_or_move((*bx as f32, *by as f32));
                        self.cursor_activity();
                        return Outcome::Running;
                    }
                    if self.talk {
                        if let Some((_, PHit::Stop)) = self.hits_audience_stop(*bx, *by) {
                            return self.close_session(x);
                        }
                    }
                    if *button == 1 {
                        let left = *bx < self.asize.0 / 2;
                        return self.handle_key(
                            x,
                            if left {
                                crate::keys::K_LEFT
                            } else {
                                crate::keys::K_RIGHT
                            },
                            0,
                            None,
                        );
                    }
                    if *button == 4 || *button == 5 {
                        return Outcome::Running;
                    }
                } else if *button == 4 || *button == 5 {
                    self.notes_scroll =
                        (self.notes_scroll + if *button == 4 { -40 } else { 40 }).max(0);
                    if let Some(p) = &mut self.presenter {
                        p.dirty = true;
                    }
                } else if *button == 1 {
                    let h = self
                        .hits
                        .iter()
                        .rev()
                        .find(|(r, _)| r.contains(*bx, *by))
                        .map(|(_, h)| *h);
                    match h {
                        Some(PHit::Prev) => return self.navigate(x, -1),
                        Some(PHit::Next) => return self.navigate(x, 1),
                        Some(PHit::End) => return self.close_session(x),
                        Some(PHit::Smaller) => self.notes_zoom(-2),
                        Some(PHit::Larger) => self.notes_zoom(2),
                        Some(PHit::Flash) => self.flash(),
                        Some(PHit::Cancel) => return self.share_check_done(false, x),
                        Some(PHit::Start) => return self.share_check_done(true, x),
                        Some(PHit::Current) => {
                            if self.focus_mode.is_some() {
                                if let Some(q) = self.to_audience((*bx, *by)) {
                                    self.pin_or_move(q);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Outcome::Running
    }

    fn hits_audience_stop(&self, x: i32, y: i32) -> Option<(Rect, PHit)> {
        let r = self.stop_rect();
        r.contains(x, y).then_some((r, PHit::Stop))
    }

    fn stop_rect(&self) -> Rect {
        let (w, h) = self.asize;
        Rect::new((w - 172 - 20).max(16), (h - 38 - 20).max(16), 172, 38)
    }

    fn flash(&mut self) {
        self.share_flash = Some(Instant::now());
        self.audience.dirty = true;
        if let Some(p) = &mut self.presenter {
            p.dirty = true;
        }
    }

    // ------------------------------------------------------------ video

    fn video_rect(&self, s: &Obj) -> (i32, i32, i32, i32) {
        let r = self.view_rect();
        let m = r.w * 0.055;
        let vr = if s_get(s, "layout", "") == "VIDEO" {
            RectF::new(r.x + m, r.y + r.h * 0.23, r.w - 2.0 * m, r.h * 0.58)
        } else {
            RectF::new(r.x + m, r.y + r.h * 0.25, r.w * 0.56, r.h * 0.52)
        };
        vr.to_rect()
    }

    fn sync_video(&mut self, x: &X) {
        if self.opening.is_some() || self.preview_only || self.blank.is_some() {
            if self.blank.is_some() {
                if let Some(v) = self.video.take() {
                    v.stop(x);
                }
            }
            return;
        }
        let s = self.slide(self.i);
        let source = s_get(&s, "videoSource", "").trim().to_string();
        let is_video = matches!(s_get(&s, "layout", "").as_str(), "VIDEO" | "VIDEO + TEXT")
            && !source.is_empty();
        if !is_video {
            if let Some(v) = self.video.take() {
                v.stop(x);
            }
            return;
        }
        let key = format!("{}|{source}", self.i);
        let rect = self.video_rect(&s);
        if self.video.as_ref().is_some_and(|v| v.key == key) {
            if let Some(v) = &self.video {
                x.configure(v.win, rect.0, rect.1, rect.2, rect.3);
            }
            return;
        }
        if let Some(v) = self.video.take() {
            v.stop(x);
        }
        self.video = crate::video::Video::start(
            x,
            self.audience.id,
            rect,
            &source,
            b_get(&s, "videoAutoplay"),
            b_get(&s, "videoMuted"),
            key,
        );
    }

    // ------------------------------------------------------------ timers

    /// Advance animations and timers; returns the next wake-up.
    pub fn tick(&mut self, x: &mut X) -> (Outcome, Option<Instant>) {
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        let mut want = |t: Instant| next = Some(next.map_or(t, |n: Instant| n.min(t)));
        let frame = now + Duration::from_millis(16);
        if let Some((to, at)) = self.transition_only {
            if now >= at {
                self.transition_only = None;
                let snap_mode = self.transition_for(to);
                let snap = self.composite_clean();
                self.i = to;
                if snap_mode != "NONE" {
                    let dur = match snap_mode.as_str() {
                        "MORPH" => 560,
                        "REVEAL" => 460,
                        _ => 360,
                    };
                    self.trans = Some(Trans {
                        snap,
                        mode: snap_mode,
                        start: Instant::now(),
                        dur,
                    });
                }
                self.audience.dirty = true;
            } else {
                want(at);
            }
        }
        // The opening fade waits until the audience window is on screen.
        if let Some(t) = self.opening {
            if self.cover.is_some_and(|c| c.to >= 1.0 && c.from >= 1.0) {
                let exposed = self.audience.exposed_at.is_some() && self.audience.mapped;
                if self.opening_wait.is_none()
                    && (exposed || t.elapsed() >= Duration::from_millis(OPEN_WAIT_MAX_MS))
                {
                    self.opening_wait = Some(now + Duration::from_millis(OPEN_SETTLE_MS));
                }
                match self.opening_wait {
                    Some(w) if now >= w => {
                        self.start_opening_fade();
                        self.dirty_all();
                    }
                    Some(w) => want(w),
                    None => want(now + Duration::from_millis(25)),
                }
            }
        }
        if let Some(c) = self.cover {
            if c.done() {
                if c.to <= 0.0 {
                    self.cover = None;
                    if self.opening.take().is_some() {
                        self.sync_video(x);
                    }
                    self.dirty_all();
                } else if c.from != c.to && !c.black_reached {
                    if let Some(cv) = self.cover.as_mut() {
                        cv.black_reached = true;
                    }
                    if self.closing_started && self.black_hold.is_none() {
                        self.black_hold = Some(now + Duration::from_millis(BLACK_HOLD_MS));
                        self.dirty_all();
                    }
                }
            } else {
                self.dirty_all();
                want(frame);
            }
        }
        if let Some(h) = self.black_hold {
            if now >= h {
                return (self.finish(x), None);
            }
            want(h);
        }
        if let Some(t) = &self.trans {
            if t.start.elapsed() >= Duration::from_millis(t.dur) {
                self.trans = None;
            } else {
                want(frame);
            }
            self.dirty_all();
        }
        if let Some(z) = &self.zoom {
            if z.start.elapsed() < Duration::from_millis(z.dur) {
                want(frame);
                self.dirty_all();
            } else if z.out {
                self.zoom = None;
                self.dirty_all();
            }
        }
        if let Some((_, _, t)) = self.pin_anim {
            if t.elapsed() >= Duration::from_millis(360) {
                if let Some((_, b, _)) = self.pin_anim.take() {
                    self.point = Some(b);
                }
            } else {
                want(frame);
            }
            self.dirty_all();
        }
        if let Some((t, shown)) = self.pause_veil {
            let dur = if shown { 380 } else { 300 };
            if t.elapsed() < Duration::from_millis(dur) {
                want(frame);
                self.audience.dirty = true;
                if let Some(p) = &mut self.presenter {
                    p.dirty = true;
                }
            } else if !shown {
                self.pause_veil = None;
                self.dirty_all();
            }
        }
        if self.paused {
            // One repaint per displayed second of the pause clock.
            let ms = self.pause_clock.elapsed().as_millis() as u64;
            let next_sec = self.pause_clock + Duration::from_millis(ms - ms % 1000 + 1005);
            want(next_sec);
            if ms / 1000 != self.last_second {
                self.last_second = ms / 1000;
                self.dirty_all();
            }
        }
        if self.motion_wanted() {
            self.audience.dirty = true;
            if let Some(p) = &mut self.presenter {
                p.dirty = true;
            }
            want(now + Duration::from_millis(40));
        }
        if let Some(r) = self.share_flash_req {
            if now >= r {
                self.share_flash_req = None;
                self.flash();
            } else {
                want(r);
            }
        }
        if let Some(t) = self.share_flash {
            let end = t + Duration::from_millis(SHARE_FLASH_MS);
            if now >= end {
                self.share_flash = None;
                self.dirty_all();
            } else {
                want(end);
            }
        }
        if let Some(t) = self.cursor_hide_at {
            if now >= t {
                self.cursor_hide_at = None;
                self.cursor_hidden = true;
                x.set_cursor(self.audience.id, x.blank_cursor);
            } else {
                want(t);
            }
        }
        if self.dual && !self.closing {
            // The clock (once a second) and the share watch (every 2 s).
            let secs = self.active_elapsed().as_secs();
            if secs != self.last_second && !self.paused {
                self.last_second = secs;
                if let Some(p) = &mut self.presenter {
                    p.dirty = true;
                }
            }
            let el = self.active_elapsed();
            let to_next = 1000 - (el.as_millis() % 1000) as u64 + 5;
            want(now + Duration::from_millis(to_next));
            let watch = if self.share_checking { 1000 } else { 2000 };
            if self.last_watch.elapsed() >= Duration::from_millis(watch) {
                self.last_watch = now;
                let v = x.is_viewable(self.audience.id);
                if v != self.audience_viewable {
                    self.audience_viewable = v;
                    if let Some(p) = &mut self.presenter {
                        p.dirty = true;
                    }
                }
            }
            want(self.last_watch + Duration::from_millis(watch));
        }
        let _ = self.share_status_at;
        (Outcome::Running, next)
    }

    // ------------------------------------------------------------- draw

    pub fn draw(&mut self, x: &mut X) -> x::Res<()> {
        if self.audience.dirty && !self.closing {
            self.audience.dirty = false;
            let mut cv = std::mem::replace(&mut self.audience.cv, Canvas::new(1, 1));
            let r = RectF::new(0.0, 0.0, cv.w as f32, cv.h as f32);
            self.draw_view(&mut cv, r, 1.0, true);
            if self.talk && self.blank.is_none() {
                let sr = self.stop_rect();
                cv.tint(sr, 0x080808, 220);
                cv.frame(sr, 0x777777);
                let f = self.f.black.clone();
                ui::text_in(&mut cv, &f, 13.0, sr, "■  STOP TALK", 0xf4f2ed, 1, 1.0);
            }
            x.present(self.audience.id, &cv)?;
            self.audience.cv = cv;
        }
        if self.presenter.as_ref().is_some_and(|p| p.dirty) && !self.closing {
            let mut p = self.presenter.take().unwrap();
            p.dirty = false;
            self.draw_presenter(&mut p.cv);
            x.present(p.id, &p.cv)?;
            self.presenter = Some(p);
        }
        Ok(())
    }

    /// The audience output (also the CURRENT preview, scaled).
    fn draw_view(&mut self, cv: &mut Canvas, dst: RectF, scale: f32, audience: bool) {
        let (aw, ah) = (self.asize.0.max(1) as f32, self.asize.1.max(1) as f32);
        let full = RectF::new(0.0, 0.0, aw, ah);
        let clip = Rect::new(
            dst.x as i32,
            dst.y as i32,
            dst.w.ceil() as i32,
            dst.h.ceil() as i32,
        );
        cv.push_clip_rect(clip);
        if let Some(b) = self.blank {
            cv.fill(clip, if b == 'W' { 0xffffff } else { 0 });
            cv.pop_clip_rect();
            return;
        }
        let map = |r: RectF| {
            RectF::new(
                dst.x + r.x * scale,
                dst.y + r.y * scale,
                r.w * scale,
                r.h * scale,
            )
        };
        self.frame();
        {
            let f2 = &self.slide_frame.as_ref().unwrap().3;
            if let Some(z) = &self.zoom {
                let zr = self.zoom_rect_now(z);
                cv.fill(clip, 0x0B0B0B);
                cv.draw_canvas(&z.snap, full, map(zr), 255);
            } else {
                cv.draw_canvas(f2, full, dst, 255);
            }
        }
        if let Some(t) = &self.trans {
            let k = out_cubic(frac(t.start, t.dur));
            let alpha = ((1.0 - k) * 255.0) as u32;
            let r = match t.mode.as_str() {
                "MORPH" => {
                    let dx = (aw * 0.014).max(7.0).trunc();
                    let dy = (ah * 0.014).max(4.0).trunc();
                    RectF::new(-dx * k, -dy * k, aw + 2.0 * dx * k, ah + 2.0 * dy * k)
                }
                "REVEAL" => {
                    let dx = (aw * 0.012).max(8.0).trunc();
                    RectF::new(-dx * k, 0.0, aw, ah)
                }
                _ => full,
            };
            if alpha > 0 {
                cv.draw_canvas(&t.snap, full, map(r), alpha);
            }
        }
        // Spotlight / Focus Reveal rest while zoomed.
        if let (Some(m), false) = (self.focus_mode, self.zoom.is_some()) {
            if let Some(pt) = self.current_point() {
                let (cx, cy) = (dst.x + pt.0 * scale, dst.y + pt.1 * scale);
                let rr = self.radius * scale;
                let alpha = if m == FocusMode::Spotlight { 112 } else { 218 };
                cv.veil_with_hole(clip, cx, cy, rr, alpha);
                if m == FocusMode::Focus {
                    cv.ring(cx, cy, rr, Rgba(255, 255, 255, 20));
                }
            }
        }
        if self.curtain {
            cv.fill(clip, 0);
        }
        if let (Some((t, shown)), true) = (self.pause_veil, audience) {
            let dur = if shown { 380 } else { 300 };
            let k = in_out_cubic(frac(t, dur));
            let op = if shown { k } else { 1.0 - k };
            self.draw_pause(cv, dst, op);
        }
        if audience {
            if let Some(_t) = self.share_flash {
                self.draw_share_flash(cv, dst);
            }
        }
        if let Some(c) = self.cover {
            let a = (c.value().clamp(0.0, 1.0) * 255.0) as u32;
            if a > 0 {
                cv.tint(clip, 0, a);
            }
        }
        cv.pop_clip_rect();
    }

    fn draw_pause(&self, cv: &mut Canvas, r: RectF, op: f32) {
        let clip = Rect::new(r.x as i32, r.y as i32, r.w.ceil() as i32, r.h.ceil() as i32);
        cv.tint(clip, 0, (205.0 * op) as u32);
        let m = r.w.min(r.h);
        let tp = (m * 0.055).max(28.0).trunc();
        let (cx, cy) = r.center();
        let tf = self.f.semi.clone();
        let fg = blend(0x000000, 0xF4F2ED, op);
        let tw = ui::text_w(&tf, tp, "PAUS", 4.0);
        let th = ui::line_h(&tf, tp);
        let ty = cy - tp * 1.35 + (tp * 1.5 - th) / 2.0;
        ui::text_spaced(cv, &tf, tp, cx - tw / 2.0, ty, "PAUS", fg, 4.0);
        let sec = self.pause_clock.elapsed().as_secs();
        let clock = format!("{:02}:{:02}", sec / 60, sec % 60);
        let cp = (m * 0.034).max(20.0).trunc();
        let mf = self.f.mono.clone();
        let cw = ui::text_w(&mf, cp, &clock, 0.0);
        let ch = ui::line_h(&mf, cp);
        let yy = cy + tp * 0.35 + (cp * 1.8 - ch) / 2.0;
        ui::text(
            cv,
            &mf,
            cp,
            cx - cw / 2.0,
            yy,
            &clock,
            blend(0, 0xF4F2ED, op * 0.65),
        );
    }

    fn draw_share_flash(&self, cv: &mut Canvas, r: RectF) {
        let bw = (r.w.min(r.h) * 0.018).max(8.0);
        let g = Rgba::hex(SHARE_GREEN);
        let mut p = RasterPainter { cv };
        use crate::paint::Painter;
        let pr = r.adjusted(bw / 2.0, bw / 2.0, -bw / 2.0, -bw / 2.0);
        p.stroke_rect(pr, g, bw);
        let fpx = (r.h * 0.035).max(14.0).trunc();
        let f = self.f.black.clone();
        let label = "✓  SHARED WINDOW · AUDIENCE";
        let w = ui::text_w(&f, fpx, label, 0.0) + fpx * 1.4;
        let h = fpx * 2.0;
        let badge = Rect::new(
            (r.x + bw * 1.6) as i32,
            (r.y + bw * 1.6) as i32,
            w as i32,
            h as i32,
        );
        cv.round_rect(badge, 4, SHARE_GREEN);
        ui::text_in(cv, &f, fpx, badge, label, 0x04130C, 1, 0.0);
    }

    fn draw_presenter(&mut self, cv: &mut Canvas) {
        self.hits.clear();
        let c = self.c.clone();
        let (w, h) = (cv.w, cv.h);
        cv.reset_clip();
        cv.fill(cv.bounds(), 0x080808);
        // PRIVATE bar.
        let bar_h = 34.max((h as f32 * 0.05) as i32);
        cv.fill(Rect::new(0, 0, w, bar_h), PRIVATE_RED);
        let bf = self.f.black.clone();
        let bpx = 14.max((bar_h as f32 * 0.48) as i32) as f32;
        ui::text_in(
            cv,
            &bf,
            bpx,
            Rect::new(16, 0, w - 32, bar_h),
            "PRIVATE  ·  NOT FOR SHARING  ·  PRESENTER VIEW",
            0xffffff,
            1,
            2.0,
        );
        let px = c.font_px;
        let x0 = 16;
        let iw = w - 32;
        let mut y = bar_h + 10;
        // Mode and audience state.
        let mode = if self.online {
            "PRESENTER · ONLINE"
        } else {
            "PRESENTER · DOCKED"
        };
        let mf = self.f.black.clone();
        let mw = ui::text_w(&mf, px, mode, 1.0).ceil() as i32;
        ui::text_in(
            cv,
            &mf,
            px,
            Rect::new(x0, y, mw + 4, 24),
            mode,
            0xe8e8e8,
            0,
            1.0,
        );
        let (state, warn) = self.audience_state();
        let sf = self.f.bold.clone();
        ui::text_in(
            cv,
            &sf,
            px,
            Rect::new(x0 + mw + 28, y, iw - mw - 28, 24),
            &state,
            if warn { 0xffb020 } else { 0xd0d0d0 },
            0,
            0.0,
        );
        y += 24 + 10;
        let bh = 36;
        let controls_y = h - 12 - bh;
        let body = Rect::new(x0, y, iw, controls_y - 10 - y);
        self.draw_body(cv, body);
        // Controls.
        let bw = 170;
        let ew = 230;
        for (r, label, hit, kind) in [
            (
                Rect::new(x0, controls_y, bw, bh),
                "‹  PREVIOUS",
                PHit::Prev,
                BtnKind::Normal,
            ),
            (
                Rect::new(x0 + (iw - ew) / 2, controls_y, ew, bh),
                "✕  END PRESENTATION",
                PHit::End,
                BtnKind::Danger,
            ),
            (
                Rect::new(x0 + iw - bw, controls_y, bw, bh),
                "NEXT  ›",
                PHit::Next,
                BtnKind::Normal,
            ),
        ] {
            let hv = self.hover == Some(hit);
            let mut p = Paint {
                cv,
                c: &c,
                f: &self.f,
                px,
            };
            p.button(r, label, kind, hv, false, true, false);
            self.hits.push((r, hit));
        }
        if self.share_checking {
            self.draw_share_check(cv);
        }
    }

    fn audience_state(&self) -> (String, bool) {
        let warn = self.audience_viewable == Some(false);
        let s = if warn {
            "⚠  AUDIENCE WINDOW HIDDEN · a shared window is frozen · show it again".into()
        } else if self.black_hold.is_some() || self.closing_started {
            "ENDING · press Esc again to close now".into()
        } else if self.end_black {
            "END OF SLIDES · audience sees black · Next or Esc ends · Previous goes back".into()
        } else if self.paused {
            "PAUSED · AUDIENCE DIMMED".into()
        } else if self.curtain {
            "CURTAIN · AUDIENCE SCREEN HIDDEN".into()
        } else if self.blank == Some('B') {
            "AUDIENCE: BLACK".into()
        } else if self.blank == Some('W') {
            "AUDIENCE: WHITE".into()
        } else if self.online {
            format!("SHARE THE WINDOW  “{AUDIENCE_TITLE}”")
        } else {
            format!("AUDIENCE → {}", self.audience_mon.name)
        };
        (s, warn)
    }

    fn draw_body(&mut self, cv: &mut Canvas, b: Rect) {
        let (bw, bh) = (b.w, b.h);
        if bw < 10 || bh < 10 {
            return;
        }
        let gap = 10.max((bw.min(bh) as f32 * 0.016) as i32);
        let total_h = cv.h;
        let notes_h = ((total_h / 3).max(140)).min(bh - 160);
        let top_h = bh - notes_h - gap;
        let label_h = 18;
        let min_right = 240.max((bw as f32 * 0.30) as i32);
        let cw = (bw - min_right - gap)
            .min(((top_h - label_h) as f32 * 16.0 / 9.0) as i32)
            .max(160);
        let ch = (cw as f32 * 9.0 / 16.0) as i32;
        let lf = self.f.mono_bold.clone();
        ui::text_in(
            cv,
            &lf,
            11.0,
            Rect::new(b.x, b.y, cw, label_h),
            "CURRENT",
            0x7a7a7a,
            0,
            1.0,
        );
        let cur = Rect::new(b.x, b.y + label_h, cw, ch);
        // CURRENT mirrors the audience: fades, transitions, focus, zoom.
        cv.fill(cur, 0x0B0B0B);
        let (aw, ah) = (self.asize.0.max(1) as f32, self.asize.1.max(1) as f32);
        let s = (cw as f32 / aw).min(ch as f32 / ah);
        let dst = RectF::new(
            cur.x as f32 + (cw as f32 - aw * s) / 2.0,
            cur.y as f32 + (ch as f32 - ah * s) / 2.0,
            aw * s,
            ah * s,
        );
        self.draw_view(cv, dst, s, false);
        let vr = self.view_rect();
        self.preview_rect = RectF::new(dst.x + vr.x * s, dst.y + vr.y * s, vr.w * s, vr.h * s);
        self.hits.push((cur, PHit::Current));
        let rx = b.x + cw + gap;
        let rw = b.w - cw - gap;
        let timer_px = 30.max(
            120.min((top_h as f32 * 0.17) as i32)
                .min((rw as f32 * 0.16) as i32),
        ) as f32;
        let mono_b = self.f.mono_bold.clone();
        let mono = self.f.mono.clone();
        let mut y = b.y;
        let secs = if self.share_checking {
            0
        } else {
            self.active_elapsed().as_secs()
        };
        let label = if secs < 3600 {
            format!("{:02}:{:02}", secs / 60, secs % 60)
        } else {
            format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
        };
        ui::text_in(
            cv,
            &mono_b,
            timer_px,
            Rect::new(rx, y, rw, (timer_px * 1.25) as i32),
            &label,
            0xf4f2ed,
            0,
            0.0,
        );
        y += (timer_px * 1.25) as i32;
        let target = i_get(&self.data, "targetMinutes", 0);
        if target > 0 {
            let vis: Vec<usize> = (0..self.deck().slides().len())
                .filter(|&j| visible(&self.data, j))
                .collect();
            let pos = vis
                .iter()
                .position(|&j| j == self.i)
                .map(|p| p + 1)
                .unwrap_or(1);
            let expected = (target * 60) as f64 * (pos as f64 / vis.len().max(1) as f64);
            let delta = secs as i64 - expected as i64;
            let sign = if delta >= 0 { "+" } else { "−" };
            let d = delta.unsigned_abs();
            let pace = format!(
                "PACE {sign}{:02}:{:02}  ·  TARGET {target} MIN",
                d / 60,
                d % 60
            );
            let pp = 13.max((timer_px * 0.24) as i32) as f32;
            ui::text_in(
                cv,
                &mono,
                pp,
                Rect::new(rx, y, rw, (pp * 1.5) as i32),
                &pace,
                0xa8a8a8,
                0,
                0.0,
            );
            y += (pp * 1.5) as i32;
        }
        let cp = 16.max((timer_px * 0.42) as i32) as f32;
        let row_h = (cp * 1.5) as i32;
        let clock = crate::editor::clock_now();
        ui::text_in(
            cv,
            &self.f.mono_bold.clone(),
            cp,
            Rect::new(rx, y, rw / 2, row_h),
            &clock,
            0xa8a8a8,
            0,
            0.0,
        );
        let total = self.deck().slides().len();
        let pp = 16.max((timer_px * 0.40) as i32) as f32;
        ui::text_in(
            cv,
            &mono_b,
            pp,
            Rect::new(rx + rw / 2, y, rw - rw / 2, row_h),
            &format!("{} / {total}", self.i + 1),
            0xe8e8e8,
            2,
            0.0,
        );
        y += row_h + gap;
        let avail = top_h - (y - b.y) - label_h;
        let nw = rw.min((avail as f32 * 16.0 / 9.0) as i32);
        let nh = (nw as f32 * 9.0 / 16.0) as i32;
        let ni = next_visible(&self.data, self.i, 1);
        let at_end = ni == self.i;
        let compact = nw < 200 || nh < 100 || at_end;
        ui::text_in(
            cv,
            &lf,
            11.0,
            Rect::new(rx, y, rw, label_h),
            "NEXT",
            0x7a7a7a,
            0,
            1.0,
        );
        if compact {
            let t = if at_end {
                if self.cinematic_close() {
                    "End of presentation · Next fades to black".to_string()
                } else {
                    "End of presentation".to_string()
                }
            } else {
                let ns = self.slide(ni);
                let tt = s_get(&ns, "title", "");
                format!(
                    "{}  ·  {}",
                    ni + 1,
                    if tt.is_empty() {
                        s_get(&ns, "layout", "")
                    } else {
                        tt
                    }
                )
            };
            let tp = 14.max((timer_px * 0.30) as i32) as f32;
            let bf = self.f.bold.clone();
            let lines = ui::wrap(&bf, tp, &t, rw as f32);
            let lh = ui::line_h(&bf, tp).ceil();
            for (k, l) in lines
                .iter()
                .take(((avail.max(30)) as f32 / lh) as usize)
                .enumerate()
            {
                ui::text(
                    cv,
                    &bf,
                    tp,
                    rx as f32,
                    (y + label_h) as f32 + k as f32 * lh,
                    l,
                    0xd8d8d8,
                );
            }
        } else {
            let nr = Rect::new(rx, y + label_h, nw, nh);
            cv.fill(nr, 0x0B0B0B);
            let r = render::aspect_rect(nr.f(), &self.aspect());
            cv.fill_aa(r.adjusted(-1.0, -1.0, 1.0, 1.0), Rgba::hex(0x2A2A2A));
            let style = DeckStyle::of(&self.data);
            let ctx = Ctx::new(&style, ni, false);
            let ns = self.slide(ni);
            cv.push_clip_rect(nr);
            {
                let mut p = RasterPainter { cv };
                render::render_slide(&mut p, r, &ns, &ctx);
            }
            cv.pop_clip_rect();
        }
        // Notes.
        let ny = b.y + top_h + gap;
        let bh2 = 30;
        ui::text_in(
            cv,
            &lf,
            11.0,
            Rect::new(b.x, ny, b.w - 120, bh2),
            "SPEAKER NOTES",
            0x7a7a7a,
            0,
            1.0,
        );
        let c = self.c.clone();
        for (r, label, hit) in [
            (
                Rect::new(b.x + b.w - 104, ny, 48, bh2 - 2),
                "A−",
                PHit::Smaller,
            ),
            (
                Rect::new(b.x + b.w - 52, ny, 52, bh2 - 2),
                "A+",
                PHit::Larger,
            ),
        ] {
            let hv = self.hover == Some(hit);
            let mut p = Paint {
                cv,
                c: &c,
                f: &self.f,
                px: c.font_px,
            };
            p.button(r, label, BtnKind::Normal, hv, false, true, false);
            self.hits.push((r, hit));
        }
        let nr = Rect::new(b.x, ny + bh2 + 4, b.w, (b.y + b.h - ny - bh2 - 4).max(40));
        cv.fill(nr, 0x0e0e0e);
        cv.frame(nr, 0x262626);
        let notes = s_get(&self.slide(self.i), "notes", "").trim().to_string();
        let nf = self.f.sans.clone();
        let npx = self.notes_px as f32;
        let inner = nr.inset(10);
        cv.push_clip_rect(inner);
        if notes.is_empty() {
            ui::text(
                cv,
                &nf,
                npx,
                inner.x as f32,
                inner.y as f32,
                "No speaker notes for this slide.",
                0x5a5a5a,
            );
        } else {
            let lines = ui::wrap(&nf, npx, &notes, inner.w as f32);
            let lh = nf.line_spacing(npx).ceil();
            let max_scroll = ((lines.len() as f32 * lh) as i32 - inner.h).max(0);
            self.notes_scroll = self.notes_scroll.min(max_scroll);
            for (k, l) in lines.iter().enumerate() {
                let yy = inner.y as f32 + k as f32 * lh - self.notes_scroll as f32;
                if yy > (inner.y + inner.h) as f32 {
                    break;
                }
                ui::text(cv, &nf, npx, inner.x as f32, yy, l, 0xececec);
            }
        }
        cv.pop_clip_rect();
    }

    fn draw_share_check(&mut self, cv: &mut Canvas) {
        let c = self.c.clone();
        let (w, h) = (cv.w, cv.h);
        cv.tint(cv.bounds(), 0, 120);
        let dw = 600.min(w - 40);
        let px = 14.0;
        let sans = self.f.sans.clone();
        let bold = self.f.black.clone();
        let lh = ui::line_h(&sans, px).ceil() as i32 + 4;
        let inner = dw - 36;
        let hop = "You can switch workspaces; the audience keeps seeing the slide. Come back here and the arrow keys work at once.";
        let never = format!("Never pick “{PRESENTER_TITLE}” (red bar) or Entire screen.");
        let hidden = self.audience_viewable == Some(false);
        let status = if hidden {
            "⚠  The audience window is hidden (another workspace or minimised). A shared hidden window freezes. Bring it back before you share.".to_string()
        } else if self.share_flash.is_some() {
            "The audience window shows a green border and a SHARED WINDOW badge for 5 seconds. Check that the Teams preview shows it.".to_string()
        } else {
            "Press Flash to mark the audience window again.".to_string()
        };
        let wrap = |s: &str| ui::wrap(&sans, px, s, inner as f32);
        let blocks: Vec<(Vec<String>, u32)> = vec![
            (wrap(&never), 0xff8a80),
            (wrap(hop), 0xd8d8d8),
            (wrap(&status), if hidden { 0xffb020 } else { 0xd8d8d8 }),
        ];
        let text_h: i32 = blocks.iter().map(|(l, _)| l.len() as i32 * lh + 10).sum();
        let dh = 18 + 34 + 3 * lh + 10 + 44 + 10 + text_h + 10 + 38 + 18;
        let r = Rect::new((w - dw) / 2, (h - dh) / 2, dw, dh);
        cv.fill(r, 0x0b0b0b);
        for k in 0..2 {
            cv.frame(r.inset(k), SHARE_GREEN);
        }
        let x0 = r.x + 18;
        let mut y = r.y + 18;
        ui::text_spaced(
            cv,
            &bold,
            20.0,
            x0 as f32,
            y as f32,
            "SHARE CHECK",
            0xffffff,
            1.0,
        );
        y += 34;
        let steps: [&[(&str, bool)]; 3] = [
            &[
                ("1.  In Teams, click ", false),
                ("Share", true),
                (".", false),
            ],
            &[
                ("2.  Open the ", false),
                ("Window", true),
                (" tab (not Entire screen).", false),
            ],
            &[("3.  Pick this entry:", false)],
        ];
        for st in steps {
            let mut xx = x0 as f32;
            for (t, b) in st {
                let f = if *b { bold.clone() } else { sans.clone() };
                xx += ui::text(cv, &f, px, xx, y as f32, t, 0xd8d8d8);
            }
            y += lh;
        }
        y += 10;
        let pick = Rect::new(x0, y, inner, 40);
        cv.fill(pick, SHARE_GREEN);
        ui::text_in(
            cv,
            &bold,
            17.0,
            Rect::new(pick.x + 12, pick.y, pick.w - 24, pick.h),
            AUDIENCE_TITLE,
            0x04130C,
            0,
            0.0,
        );
        y += 44 + 10;
        for (lines, col) in &blocks {
            let f = if *col == 0xff8a80 || *col == 0xffb020 {
                self.f.bold.clone()
            } else {
                sans.clone()
            };
            for l in lines {
                ui::text(cv, &f, px, x0 as f32, y as f32, l, *col);
                y += lh;
            }
            y += 10;
        }
        y += 10;
        let bh = 38;
        let fl = Rect::new(x0, y, 220, bh);
        let st = Rect::new(r.x + r.w - 18 - 180, y, 180, bh);
        let ca = Rect::new(st.x - 10 - 120, y, 120, bh);
        for (rr, label, hit, kind) in [
            (fl, "Flash audience window", PHit::Flash, BtnKind::Normal),
            (ca, "Cancel", PHit::Cancel, BtnKind::Normal),
            (st, "Start presenting", PHit::Start, BtnKind::Primary),
        ] {
            let hv = self.hover == Some(hit);
            let mut p = Paint {
                cv,
                c: &c,
                f: &self.f,
                px,
            };
            p.button(rr, label, kind, hv, false, true, false);
            self.hits.push((rr, hit));
        }
    }
}
