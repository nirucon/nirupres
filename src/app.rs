//! The event loop: one poll over the X connection, an eventfd for worker
//! results and inotify for the NIRUX appearance files. Nothing wakes the
//! process while it is idle.
use crate::editor::{Editor, PresentReq, Request};
use crate::present::{self, Outcome, Session};
use crate::talk::{Choice, Poll, Talk, TalkResult};
use crate::x::{Ev, X};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub enum Start {
    Editor(Option<PathBuf>),
    Present(PathBuf),
    Talk(PathBuf, Choice),
}

pub struct App {
    x: X,
    ed: Editor,
    session: Option<Session>,
    preview: Option<Session>,
    talk: Option<Talk>,
    talk_poll_at: Option<Instant>,
    talk_rx: Option<mpsc::Receiver<Result<TalkResult, String>>>,
    wake: i32,
    inotify: i32,
    chrome_at: Option<Instant>,
    started: Instant,
    shown_once: bool,
    last_dirty: Option<bool>,
    quit: bool,
    start: Option<Start>,
}

fn wake_fd() -> i32 {
    unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) }
}

impl App {
    pub fn new(start: Start, started: Instant) -> crate::x::Res<Self> {
        let timing = std::env::var_os("NIRUPRES_TIMING").is_some();
        let x = X::open()?;
        if timing {
            eprintln!(
                "nirupres: X open {:.1} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        let ed = Editor::new(&x)?;
        if timing {
            eprintln!(
                "nirupres: editor ready {:.1} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );
        }
        let inotify = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        let mut app = Self {
            x,
            ed,
            session: None,
            preview: None,
            talk: None,
            talk_poll_at: None,
            talk_rx: None,
            wake: wake_fd(),
            inotify,
            chrome_at: None,
            started,
            shown_once: false,
            last_dirty: None,
            quit: false,
            start: Some(start),
        };
        app.watch_chrome();
        Ok(app)
    }

    fn watch_chrome(&mut self) {
        if self.inotify < 0 {
            return;
        }
        let dir = crate::chrome::watch_dir();
        let target = if dir.is_dir() {
            dir
        } else {
            crate::chrome::config_home()
        };
        if let Ok(c) = std::ffi::CString::new(target.to_string_lossy().as_bytes()) {
            unsafe {
                libc::inotify_add_watch(
                    self.inotify,
                    c.as_ptr(),
                    libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_CREATE | libc::IN_DELETE,
                )
            };
        }
    }

    fn drain_inotify(&mut self) {
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe {
                libc::read(
                    self.inotify,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n <= 0 {
                break;
            }
        }
        self.chrome_at = Some(Instant::now() + Duration::from_millis(250));
    }

    pub fn run(&mut self) -> crate::x::Res<()> {
        let xfd = {
            use std::os::fd::AsRawFd;
            self.x.c.stream().as_raw_fd()
        };
        // The editor shows first; the deck from the command line opens in it.
        match self.start.as_ref() {
            Some(Start::Editor(Some(p))) | Some(Start::Present(p)) | Some(Start::Talk(p, _)) => {
                let p = p.clone();
                if let Err(e) = self.ed.open_path(&self.x, &p) {
                    if !matches!(self.start, Some(Start::Editor(_))) {
                        return Err(format!("cannot open {}: {e}", p.display()).into());
                    }
                    crate::dialogs::message(
                        &mut self.ed,
                        "Open failed",
                        &format!("NIRUPRES could not open this presentation.\n\n{e}"),
                    );
                }
            }
            _ => {}
        }
        self.x.map(self.ed.win);
        self.ed.update_title(&self.x);
        while !self.quit {
            while let Some(ev) = self.x.next_event() {
                self.event(ev);
                if self.quit {
                    break;
                }
            }
            if self.quit {
                break;
            }
            let mut next: Option<Instant> = None;
            let mut want = |t: Option<Instant>| {
                if let Some(t) = t {
                    next = Some(next.map_or(t, |n: Instant| n.min(t)));
                }
            };
            want(self.ed.timers());
            if let Some(mut s) = self.session.take() {
                let (o, t) = s.tick(&mut self.x);
                if o == Outcome::Finished {
                    self.session_finished(s);
                } else {
                    want(t);
                    self.session = Some(s);
                }
            }
            if let Some(mut s) = self.preview.take() {
                let (o, t) = s.tick(&mut self.x);
                if o != Outcome::Finished {
                    want(t);
                    self.preview = Some(s);
                }
            }
            if let Some(t) = self.talk_poll_at {
                if Instant::now() >= t {
                    self.poll_talk();
                }
                want(self.talk_poll_at);
            }
            if let Some(t) = self.chrome_at {
                if Instant::now() >= t {
                    self.chrome_at = None;
                    self.ed.refresh_chrome(&self.x);
                } else {
                    want(Some(t));
                }
            }
            self.draw()?;
            // The first frame can start a presentation or Talk mode (a
            // command-line start): their timers count from now on.
            want(self.talk_poll_at);
            if self.x.has_pending() {
                continue;
            }
            use x11rb::connection::Connection;
            self.x.c.flush()?;
            let timeout = next
                .map(|t| t.saturating_duration_since(Instant::now()).as_millis() as i32 + 1)
                .unwrap_or(-1);
            let mut fds = [
                libc::pollfd {
                    fd: xfd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.wake,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.inotify,
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let n = if self.inotify >= 0 { 3 } else { 2 };
            unsafe { libc::poll(fds.as_mut_ptr(), n, timeout) };
            if fds[1].revents & libc::POLLIN != 0 {
                let mut v = 0u64;
                unsafe { libc::read(self.wake, &mut v as *mut u64 as *mut libc::c_void, 8) };
                self.worker_results();
            }
            if n == 3 && fds[2].revents & libc::POLLIN != 0 {
                self.drain_inotify();
            }
        }
        if let Some(mut s) = self.session.take() {
            s.finish(&self.x);
        }
        if let Some(t) = self.talk.take() {
            t.abort();
        }
        Ok(())
    }

    fn draw(&mut self) -> crate::x::Res<()> {
        if self.ed.dirty_draw && !self.ed.presenting && self.ed.mapped {
            self.ed.dirty_draw = false;
            let t0 = Instant::now();
            self.ed.draw();
            if !self.shown_once && std::env::var_os("NIRUPRES_TIMING").is_some() {
                eprintln!(
                    "nirupres: draw {:.1} ms (mapped {})",
                    t0.elapsed().as_secs_f64() * 1000.0,
                    self.ed.mapped
                );
            }
            self.x.present(self.ed.win, &self.ed.cv)?;
            if !self.shown_once && self.ed.mapped {
                self.shown_once = true;
                if std::env::var_os("NIRUPRES_TIMING").is_some() {
                    eprintln!(
                        "nirupres: first frame after {:.1} ms",
                        self.started.elapsed().as_secs_f64() * 1000.0
                    );
                }
                self.after_first_frame();
            }
        }
        if let Some(s) = self.session.as_mut() {
            s.draw(&mut self.x)?;
        }
        if let Some(s) = self.preview.as_mut() {
            s.draw(&mut self.x)?;
        }
        if self.last_dirty != Some(self.ed.dirty) {
            self.last_dirty = Some(self.ed.dirty);
            self.ed.update_title(&self.x);
        }
        Ok(())
    }

    fn after_first_frame(&mut self) {
        self.x.raise_focus(self.ed.win);
        // Undo Do Not Disturb left on by a presentation that crashed.
        std::thread::spawn(crate::presenting::recover_stale);
        if matches!(self.start, Some(Start::Editor(_))) {
            self.ed.offer_recovery();
        }
        match self.start.take() {
            Some(Start::Present(_)) => {
                let r = self
                    .ed
                    .present(&mut self.x, true, None, None, false, false, None);
                self.request(r);
            }
            Some(Start::Talk(_, choice)) => {
                let r = self.ed.start_talk(choice);
                self.request(r);
            }
            _ => {}
        }
    }

    fn event(&mut self, ev: Ev) {
        let win = match &ev {
            Ev::Expose(w)
            | Ev::Close(w)
            | Ev::Leave(w)
            | Ev::Focus(w, _)
            | Ev::Mapped(w, _)
            | Ev::DndLeave(w) => Some(*w),
            Ev::Configure(w, ..) | Ev::DndOver(w, ..) | Ev::Drop(w, ..) => Some(*w),
            Ev::Key { win, .. } | Ev::Button { win, .. } | Ev::Motion { win, .. } => Some(*win),
            Ev::Theme => {
                if self.ed.chrome_follow {
                    self.ed.refresh_chrome(&self.x);
                }
                None
            }
            Ev::Keymap => {
                self.x.reload_keymap();
                None
            }
        };
        let Some(win) = win else { return };
        if self.session.as_ref().is_some_and(|s| s.owns(win)) {
            let mut s = self.session.take().unwrap();
            if s.event(&mut self.x, &ev) == Outcome::Finished {
                self.session_finished(s);
            } else {
                self.mark_slide(&mut s);
                self.session = Some(s);
            }
            return;
        }
        if self.preview.as_ref().is_some_and(|s| s.owns(win)) {
            let mut s = self.preview.take().unwrap();
            if s.event(&mut self.x, &ev) != Outcome::Finished {
                self.preview = Some(s);
            }
            return;
        }
        if win == self.ed.win {
            let r = self.ed.event(&mut self.x, &ev);
            self.request(r);
            if let Some(r) = self.ed.pending.take() {
                self.request(r);
            }
        }
    }

    fn mark_slide(&mut self, s: &mut Session) {
        if let Some(i) = s.slide_changed.take() {
            if let Some(t) = self.talk.as_mut() {
                t.mark_slide(i);
            }
        }
    }

    fn request(&mut self, r: Request) {
        match r {
            Request::None => {}
            Request::Quit => self.quit = true,
            Request::Present(req) => self.present(req),
            Request::TransitionPreview(prev) => {
                if let Some(old) = self.preview.take() {
                    let mut old = old;
                    old.finish(&self.x);
                }
                let mons = self.x.monitors();
                let here = self.ed.editor_monitor(&self.x, &mons);
                let to = self.ed.cur;
                match Session::transition_preview(
                    &mut self.x,
                    self.ed.data.clone(),
                    prev,
                    to,
                    self.ed.c.clone(),
                    mons[here].clone(),
                ) {
                    Ok(s) => self.preview = Some(s),
                    Err(e) => crate::dialogs::message(&mut self.ed, "Preview", &e.to_string()),
                }
            }
            Request::Talk(choice) => self.start_talk(choice),
        }
    }

    /// Talk mode: present first (the audience window must exist), then let
    /// NIRUCLIP record that window, the microphone and the camera. The
    /// presenter view shows REC and the camera; it is never recorded.
    fn start_talk(&mut self, choice: Choice) {
        let titles: Vec<String> = self
            .ed
            .slides()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let s = s.as_object().cloned().unwrap_or_default();
                let t = crate::model::s_or(&s, "title", "");
                if !t.is_empty() {
                    t
                } else {
                    let l = crate::model::s_or(&s, "layout", "");
                    if l.is_empty() {
                        format!("Slide {}", i + 1)
                    } else {
                        l
                    }
                }
            })
            .collect();
        let mut t = Talk::new(self.ed.path.as_deref(), titles, choice);
        if let Err(e) = t.check() {
            self.ed.status = None;
            crate::dialogs::message(&mut self.ed, "Talk mode", &e);
            return;
        }
        match self
            .ed
            .present(&mut self.x, true, None, None, false, true, None)
        {
            Request::Present(req) => self.present(req),
            _ => {
                self.ed.status = None;
                return;
            }
        }
        let Some(window) = self.session.as_ref().map(|s| s.audience.id) else {
            self.ed.status = None;
            return;
        };
        match t.start(Some(window)) {
            Ok(()) => {
                if let Some(s) = self.session.as_mut() {
                    s.rec = Some(crate::present::RecView {
                        state: "starting".into(),
                        camera: choice.source == crate::talk::Source::SlidesCamera,
                        ..Default::default()
                    });
                    if let Some(p) = s.presenter.as_mut() {
                        p.dirty = true;
                    }
                }
                self.talk = Some(t);
                self.talk_poll_at = Some(Instant::now() + Duration::from_millis(150));
            }
            Err(e) => {
                if let Some(mut s) = self.session.take() {
                    s.finish(&self.x);
                    self.session_finished(s);
                }
                self.ed.status = None;
                crate::dialogs::message(&mut self.ed, "Talk mode", &e);
            }
        }
    }

    fn poll_talk(&mut self) {
        let Some(t) = self.talk.as_mut() else {
            self.talk_poll_at = None;
            return;
        };
        if t.since.is_some() {
            // Recording: keep the presenter's REC badge and camera current.
            let mut live = crate::talk::live_state();
            if t.process_ended() && live.state == "recording" {
                // Killed or crashed: the state file was not updated.
                live.state = "interrupted".into();
                live.reason = "NIRUCLIP ended unexpectedly; run niruclip --recover".into();
            }
            let mut stopped = false;
            if let Some(s) = self.session.as_mut() {
                if let Some(rec) = s.rec.as_mut() {
                    let mut next = rec.clone();
                    let active = ["recording", "starting", "countdown", "pausing", "paused"]
                        .contains(&live.state.as_str());
                    if !active {
                        stopped = true;
                        next.state = "stopped".into();
                        next.note = if live.reason.is_empty() {
                            format!("recording {}", live.state)
                        } else {
                            live.reason.clone()
                        };
                    } else {
                        next.preview = live.preview.clone();
                        next.note = match live.camera.as_str() {
                            "" | "ok" | "checking" => String::new(),
                            other if !next.camera => {
                                let _ = other;
                                String::new()
                            }
                            other => format!("CAMERA: {}", other.to_uppercase()),
                        };
                    }
                    if next != *rec {
                        *rec = next;
                        if let Some(p) = s.presenter.as_mut() {
                            p.dirty = true;
                        }
                    }
                }
            }
            self.talk_poll_at = if stopped {
                None
            } else {
                Some(Instant::now() + Duration::from_millis(500))
            };
            return;
        }
        match t.poll() {
            Poll::Waiting => self.talk_poll_at = Some(Instant::now() + Duration::from_millis(150)),
            Poll::Started(rec) => {
                let since = t.since;
                if let Some(s) = self.session.as_mut() {
                    t.mark_start(s.i);
                    s.slide_changed = None;
                    s.restart_clock();
                    let live = crate::talk::live_state();
                    if let Some(r) = s.rec.as_mut() {
                        r.state = "recording".into();
                        r.since = since;
                        r.preview = live.preview;
                    }
                    if let Some(p) = s.presenter.as_mut() {
                        p.dirty = true;
                    }
                }
                eprintln!("nirupres: Talk recording {rec}");
                self.talk_poll_at = Some(Instant::now() + Duration::from_millis(500));
            }
            Poll::Failed(msg) => {
                self.talk_poll_at = None;
                self.talk = None;
                if let Some(mut s) = self.session.take() {
                    s.finish(&self.x);
                    self.session_finished(s);
                }
                self.ed.status = None;
                crate::dialogs::message(&mut self.ed, "Talk mode", &msg);
            }
        }
    }

    fn present(&mut self, req: PresentReq) {
        if let Some(mut s) = self.session.take() {
            s.finish(&self.x);
        }
        let mons = self.x.monitors();
        let p = req.presenter.unwrap_or(0).min(mons.len() - 1);
        let a = req.audience.unwrap_or(p).min(mons.len() - 1);
        let cfg = present::Config {
            start: req.start,
            presenter: mons[p].clone(),
            audience: mons[a].clone(),
            online: req.online,
            talk: req.talk,
            share_check: req.share_check,
        };
        match Session::start(&mut self.x, self.ed.data.clone(), cfg, self.ed.c.clone()) {
            Ok(s) => {
                self.session = Some(s);
                // The editor is hidden while presenting, so it can never be
                // shared or seen behind the presenter view.
                self.ed.presenting = true;
                self.x.unmap(self.ed.win);
            }
            Err(e) => {
                eprintln!("nirupres: could not present: {e}");
                crate::dialogs::message(&mut self.ed, "Present", &e.to_string())
            }
        }
    }

    fn session_finished(&mut self, s: Session) {
        drop(s);
        self.ed.presenting = false;
        self.x.map(self.ed.win);
        self.x.raise_focus(self.ed.win);
        self.ed.dirty_draw = true;
        self.x
            .shrink_shm((self.ed.cv.w * self.ed.cv.h * 4) as usize);
        if let Some(t) = self.talk.take() {
            self.ed.set_status("Finishing the Talk recording…", 30000);
            let (tx, rx) = mpsc::channel();
            let wake = self.wake;
            std::thread::spawn(move || {
                let r = t.finish();
                let _ = tx.send(r);
                let one: u64 = 1;
                unsafe { libc::write(wake, &one as *const u64 as *const libc::c_void, 8) };
            });
            self.talk_rx = Some(rx);
        }
    }

    fn worker_results(&mut self) {
        if let Some(rx) = &self.talk_rx {
            if let Ok(r) = rx.try_recv() {
                self.talk_rx = None;
                self.ed.status = None;
                match r {
                    Ok(res) => {
                        let name = |p: &std::path::Path| {
                            p.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        };
                        let folder = res
                            .recording
                            .parent()
                            .map(|d| {
                                let home = crate::prefs::home();
                                match d.strip_prefix(&home) {
                                    Ok(rel) => format!("~/{}", rel.display()),
                                    Err(_) => d.display().to_string(),
                                }
                            })
                            .unwrap_or_default();
                        let mut text = format!(
                            "Saved in {folder}\n\nRecording: {}\nChapters: {}\nSubtitles: {}\n\nDuration {:02}:{:02} · {} slide marker(s)",
                            name(&res.recording),
                            name(&res.chapters),
                            name(&res.subtitles),
                            (res.duration / 60.0) as u64,
                            (res.duration % 60.0) as u64,
                            res.markers
                        );
                        if !res.warnings.is_empty() {
                            text.push_str(&format!("\n\n{}", res.warnings.join("\n")));
                        }
                        crate::dialogs::message(&mut self.ed, "Talk recording saved", &text);
                    }
                    Err(e) => crate::dialogs::message(&mut self.ed, "Talk mode", &e),
                }
                self.x.raise_focus(self.ed.win);
            }
        }
    }
}
