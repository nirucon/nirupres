//! The X11 side: windows (editor, audience, presenter, preview), the
//! keyboard map, monitors (RandR), the clipboard, file drops (XDND) and
//! presenting a canvas with MIT-SHM (one shared copy per frame) or, without
//! it, PutImage in slices.
use crate::canvas::Canvas;
use std::collections::VecDeque;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::composite::ConnectionExt as _;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shm::ConnectionExt as _;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

pub type Res<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Atoms {
    pub wm_protocols: Atom,
    pub wm_delete: Atom,
    pub net_wm_name: Atom,
    pub net_wm_icon: Atom,
    pub net_wm_pid: Atom,
    pub utf8: Atom,
    pub niruwm_theme: Atom,
    pub keep_mapped: Atom,
    pub opacity: Atom,
    pub clipboard: Atom,
    pub targets: Atom,
    pub uri_list: Atom,
    pub sel_prop: Atom,
    pub net_wm_state: Atom,
    pub fullscreen: Atom,
    pub max_v: Atom,
    pub max_h: Atom,
    pub net_active: Atom,
    pub xdnd_aware: Atom,
    pub xdnd_enter: Atom,
    pub xdnd_position: Atom,
    pub xdnd_status: Atom,
    pub xdnd_leave: Atom,
    pub xdnd_drop: Atom,
    pub xdnd_finished: Atom,
    pub xdnd_selection: Atom,
    pub xdnd_copy: Atom,
    pub dnd_prop: Atom,
    pub net_wm_window_type: Atom,
    pub type_normal: Atom,
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub primary: bool,
}

#[derive(Debug)]
pub enum Ev {
    Expose(Window),
    Configure(Window, i32, i32, i32, i32),
    Key {
        win: Window,
        sym: u32,
        state: u16,
        ch: Option<char>,
    },
    Button {
        win: Window,
        x: i32,
        y: i32,
        button: u8,
        state: u16,
        press: bool,
        time: u32,
    },
    Motion {
        win: Window,
        x: i32,
        y: i32,
        state: u16,
    },
    Leave(Window),
    Focus(Window, bool),
    Close(Window),
    Mapped(Window, bool),
    Theme,
    DndOver(Window, i32, i32),
    DndLeave(Window),
    Drop(Window, i32, i32, Vec<String>),
    Keymap,
}

struct Shm {
    seg: u32,
    addr: *mut u8,
    size: usize,
}

struct Dnd {
    source: Window,
    target: Window,
    pos: (i32, i32),
}

pub struct X {
    pub c: RustConnection,
    pub root: Window,
    pub depth: u8,
    visual: Visualid,
    gc: Gcontext,
    pub atoms: Atoms,
    shm: Option<Shm>,
    shm_ok: bool,
    min_keycode: u8,
    per: u8,
    keysyms: Vec<u32>,
    pending: VecDeque<Ev>,
    pub clip_text: Option<String>,
    pub blank_cursor: Cursor,
    pub ibeam_cursor: Cursor,
    pub arrow_cursor: Cursor,
    dnd: Option<Dnd>,
    pub dead: Option<u32>,
    pub screen_w: i32,
    pub screen_h: i32,
}

fn atom(c: &RustConnection, name: &str) -> Res<Atom> {
    Ok(c.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

impl X {
    pub fn open() -> Res<Self> {
        let (c, screen) = x11rb::connect(None)?;
        let s = &c.setup().roots[screen];
        let root = s.root;
        let depth = s.root_depth;
        let visual = s.root_visual;
        let (screen_w, screen_h) = (s.width_in_pixels as i32, s.height_in_pixels as i32);
        // Ask for every atom first, then read the answers: one round trip.
        let names = [
            "WM_PROTOCOLS",
            "WM_DELETE_WINDOW",
            "_NET_WM_NAME",
            "_NET_WM_ICON",
            "_NET_WM_PID",
            "UTF8_STRING",
            "_NIRUWM_THEME",
            "_NIRUWM_KEEP_MAPPED",
            "_NET_WM_WINDOW_OPACITY",
            "CLIPBOARD",
            "TARGETS",
            "text/uri-list",
            "_NIRUPRES_SELECTION",
            "_NET_WM_STATE",
            "_NET_WM_STATE_FULLSCREEN",
            "_NET_WM_STATE_MAXIMIZED_VERT",
            "_NET_WM_STATE_MAXIMIZED_HORZ",
            "_NET_ACTIVE_WINDOW",
            "XdndAware",
            "XdndEnter",
            "XdndPosition",
            "XdndStatus",
            "XdndLeave",
            "XdndDrop",
            "XdndFinished",
            "XdndSelection",
            "XdndActionCopy",
            "_NIRUPRES_DND",
            "_NET_WM_WINDOW_TYPE",
            "_NET_WM_WINDOW_TYPE_NORMAL",
        ];
        let cookies: Vec<_> = names
            .iter()
            .map(|n| c.intern_atom(false, n.as_bytes()))
            .collect::<Result<_, _>>()?;
        let mut a = Vec::with_capacity(names.len());
        for ck in cookies {
            a.push(ck.reply()?.atom);
        }
        let atoms = Atoms {
            wm_protocols: a[0],
            wm_delete: a[1],
            net_wm_name: a[2],
            net_wm_icon: a[3],
            net_wm_pid: a[4],
            utf8: a[5],
            niruwm_theme: a[6],
            keep_mapped: a[7],
            opacity: a[8],
            clipboard: a[9],
            targets: a[10],
            uri_list: a[11],
            sel_prop: a[12],
            net_wm_state: a[13],
            fullscreen: a[14],
            max_v: a[15],
            max_h: a[16],
            net_active: a[17],
            xdnd_aware: a[18],
            xdnd_enter: a[19],
            xdnd_position: a[20],
            xdnd_status: a[21],
            xdnd_leave: a[22],
            xdnd_drop: a[23],
            xdnd_finished: a[24],
            xdnd_selection: a[25],
            xdnd_copy: a[26],
            dnd_prop: a[27],
            net_wm_window_type: a[28],
            type_normal: a[29],
        };
        c.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?;
        let gc = c.generate_id()?;
        c.create_gc(gc, root, &CreateGCAux::new().graphics_exposures(0))?;
        let shm_ok = c
            .extension_information(x11rb::protocol::shm::X11_EXTENSION_NAME)?
            .is_some()
            && std::env::var_os("NIRUPRES_NO_SHM").is_none();
        let setup = c.setup();
        let (min_keycode, max_keycode) = (setup.min_keycode, setup.max_keycode);
        let map = c
            .get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?
            .reply()?;
        // Cursors: blank (presenting), arrow and the text I-beam.
        let font = c.generate_id()?;
        c.open_font(font, b"cursor")?;
        let arrow = c.generate_id()?;
        c.create_glyph_cursor(arrow, font, font, 68, 69, 0, 0, 0, 0xffff, 0xffff, 0xffff)?;
        let ibeam = c.generate_id()?;
        c.create_glyph_cursor(ibeam, font, font, 152, 153, 0, 0, 0, 0xffff, 0xffff, 0xffff)?;
        let pm = c.generate_id()?;
        c.create_pixmap(1, pm, root, 1, 1)?;
        let blank = c.generate_id()?;
        c.create_cursor(blank, pm, pm, 0, 0, 0, 0, 0, 0, 0, 0)?;
        c.free_pixmap(pm)?;
        c.close_font(font)?;
        Ok(X {
            c,
            root,
            depth,
            visual,
            gc,
            atoms,
            shm: None,
            shm_ok,
            min_keycode,
            per: map.keysyms_per_keycode,
            keysyms: map.keysyms,
            pending: VecDeque::new(),
            clip_text: None,
            blank_cursor: blank,
            ibeam_cursor: ibeam,
            arrow_cursor: arrow,
            dnd: None,
            dead: None,
            screen_w,
            screen_h,
        })
    }

    pub fn reload_keymap(&mut self) {
        let setup = self.c.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        if let Ok(Ok(m)) = self
            .c
            .get_keyboard_mapping(min, max - min + 1)
            .map(|c| c.reply())
        {
            self.min_keycode = min;
            self.per = m.keysyms_per_keycode;
            self.keysyms = m.keysyms;
        }
    }

    /// A top-level window with its WM_CLASS (instance, class).
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
        title: &str,
        instance: &str,
        class: &str,
        keep_mapped: bool,
        opaque: bool,
        dnd: bool,
    ) -> Res<Window> {
        let win = self.c.generate_id()?;
        self.c.create_window(
            self.depth,
            win,
            self.root,
            x as i16,
            y as i16,
            w.max(1),
            h.max(1),
            0,
            WindowClass::INPUT_OUTPUT,
            self.visual,
            &CreateWindowAux::new()
                .background_pixel(0)
                .bit_gravity(Gravity::NORTH_WEST)
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::KEY_PRESS
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::POINTER_MOTION
                        | EventMask::LEAVE_WINDOW
                        | EventMask::STRUCTURE_NOTIFY
                        | EventMask::FOCUS_CHANGE
                        | EventMask::PROPERTY_CHANGE,
                ),
        )?;
        let wm_class = format!("{instance}\0{class}\0");
        self.c.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            wm_class.as_bytes(),
        )?;
        self.c.change_property32(
            PropMode::REPLACE,
            win,
            self.atoms.wm_protocols,
            AtomEnum::ATOM,
            &[self.atoms.wm_delete],
        )?;
        self.c.change_property32(
            PropMode::REPLACE,
            win,
            self.atoms.net_wm_pid,
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )?;
        self.c.change_property32(
            PropMode::REPLACE,
            win,
            self.atoms.net_wm_window_type,
            AtomEnum::ATOM,
            &[self.atoms.type_normal],
        )?;
        if keep_mapped {
            // NIRUWM keeps this window mapped on a hidden workspace, so a
            // screen share keeps receiving frames.
            self.c.change_property32(
                PropMode::REPLACE,
                win,
                self.atoms.keep_mapped,
                AtomEnum::CARDINAL,
                &[1],
            )?;
        }
        if opaque {
            // Never see-through under a compositor's inactive-opacity rule.
            self.c.change_property32(
                PropMode::REPLACE,
                win,
                self.atoms.opacity,
                AtomEnum::CARDINAL,
                &[0xFFFF_FFFF],
            )?;
        }
        if dnd {
            self.c.change_property32(
                PropMode::REPLACE,
                win,
                self.atoms.xdnd_aware,
                AtomEnum::ATOM,
                &[5],
            )?;
        }
        self.set_title(win, title)?;
        let icon = crate::logo::icon();
        let mut data = Vec::new();
        for (w, h, px) in &icon {
            data.push(*w);
            data.push(*h);
            data.extend_from_slice(px);
        }
        self.c.change_property32(
            PropMode::REPLACE,
            win,
            self.atoms.net_wm_icon,
            AtomEnum::CARDINAL,
            &data,
        )?;
        Ok(win)
    }

    /// Turn a new (unmapped) window into the Talk mode recording stage: not
    /// managed by the window manager (override-redirect, so it stays where
    /// it is put, outside every screen) and redirected with Composite, so
    /// everything drawn into it is kept in its own pixmap although no part
    /// of it is on screen. NIRUCLIP records that pixmap. False when the X
    /// server has no Composite extension.
    pub fn make_stage(&self, win: Window) -> bool {
        let _ = self
            .c
            .change_window_attributes(win, &ChangeWindowAttributesAux::new().override_redirect(1));
        let has = self
            .c
            .extension_information(x11rb::protocol::composite::X11_EXTENSION_NAME)
            .ok()
            .flatten()
            .is_some()
            && self
                .c
                .composite_query_version(0, 4)
                .ok()
                .and_then(|c| c.reply().ok())
                .is_some();
        if !has {
            return false;
        }
        let ok = self
            .c
            .composite_redirect_window(win, x11rb::protocol::composite::Redirect::AUTOMATIC)
            .ok()
            .and_then(|c| c.check().ok())
            .is_some();
        let _ = self.c.flush();
        ok
    }

    /// A child window (the embedded video player draws into it).
    pub fn create_child(&self, parent: Window, x: i32, y: i32, w: u16, h: u16) -> Res<Window> {
        let win = self.c.generate_id()?;
        self.c.create_window(
            self.depth,
            win,
            parent,
            x as i16,
            y as i16,
            w.max(1),
            h.max(1),
            0,
            WindowClass::INPUT_OUTPUT,
            self.visual,
            &CreateWindowAux::new().background_pixel(0),
        )?;
        self.c.map_window(win)?;
        self.c.flush()?;
        Ok(win)
    }

    pub fn configure(&self, win: Window, x: i32, y: i32, w: i32, h: i32) {
        let _ = self.c.configure_window(
            win,
            &ConfigureWindowAux::new()
                .x(x)
                .y(y)
                .width(w.max(1) as u32)
                .height(h.max(1) as u32),
        );
    }

    pub fn destroy(&self, win: Window) {
        let _ = self.c.destroy_window(win);
        let _ = self.c.flush();
    }

    pub fn set_title(&self, win: Window, title: &str) -> Res<()> {
        self.c.change_property8(
            PropMode::REPLACE,
            win,
            self.atoms.net_wm_name,
            self.atoms.utf8,
            title.as_bytes(),
        )?;
        let latin: Vec<u8> = title
            .chars()
            .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
            .collect();
        self.c.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_NAME,
            AtomEnum::STRING,
            &latin,
        )?;
        Ok(())
    }

    /// Ask for full screen or maximised before mapping (EWMH state list).
    pub fn initial_state(&self, win: Window, fullscreen: bool, maximized: bool) {
        let mut v = Vec::new();
        if fullscreen {
            v.push(self.atoms.fullscreen);
        }
        if maximized {
            v.push(self.atoms.max_v);
            v.push(self.atoms.max_h);
        }
        let _ = self.c.change_property32(
            PropMode::REPLACE,
            win,
            self.atoms.net_wm_state,
            AtomEnum::ATOM,
            &v,
        );
    }

    /// Toggle an EWMH state on a mapped window.
    pub fn set_state(&self, win: Window, on: bool, a: Atom, b: Atom) {
        let ev = ClientMessageEvent::new(
            32,
            win,
            self.atoms.net_wm_state,
            [u32::from(on), a, b, 1, 0],
        );
        let _ = self.c.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            ev,
        );
    }

    pub fn map(&self, win: Window) {
        let _ = self.c.map_window(win);
        let _ = self.c.flush();
    }

    pub fn unmap(&self, win: Window) {
        let _ = self.c.unmap_window(win);
        let _ = self.c.flush();
    }

    pub fn raise_focus(&self, win: Window) {
        let _ = self
            .c
            .configure_window(win, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
        let ev = ClientMessageEvent::new(
            32,
            win,
            self.atoms.net_active,
            [1, x11rb::CURRENT_TIME, 0, 0, 0],
        );
        let _ = self.c.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            ev,
        );
        let _ = self
            .c
            .set_input_focus(InputFocus::PARENT, win, x11rb::CURRENT_TIME);
        let _ = self.c.flush();
    }

    pub fn set_cursor(&self, win: Window, cursor: Cursor) {
        let _ = self
            .c
            .change_window_attributes(win, &ChangeWindowAttributesAux::new().cursor(cursor));
        let _ = self.c.flush();
    }

    /// True when mapped and viewable, false when not, None if unknown.
    pub fn is_viewable(&self, win: Window) -> Option<bool> {
        let r = self.c.get_window_attributes(win).ok()?.reply().ok()?;
        Some(r.map_state == MapState::VIEWABLE)
    }

    pub fn root_theme(&self) -> Option<String> {
        let r = self
            .c
            .get_property(
                false,
                self.root,
                self.atoms.niruwm_theme,
                AtomEnum::ANY,
                0,
                64,
            )
            .ok()?
            .reply()
            .ok()?;
        let s = String::from_utf8(r.value).ok()?;
        (!s.is_empty()).then_some(s)
    }

    /// Window position on the root (for "which screen am I on").
    pub fn root_pos(&self, win: Window) -> Option<(i32, i32)> {
        let r = self
            .c
            .translate_coordinates(win, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        Some((r.dst_x as i32, r.dst_y as i32))
    }

    pub fn monitors(&self) -> Vec<Monitor> {
        // NIRUPRES_MONITORS="NAME:WxH+X+Y,..." describes the screens by hand
        // (end-to-end tests on a single virtual screen).
        if let Ok(spec) = std::env::var("NIRUPRES_MONITORS") {
            let v: Vec<Monitor> = spec
                .split(',')
                .enumerate()
                .filter_map(|(i, m)| {
                    let (name, geo) = m.split_once(':')?;
                    let (wh, rest) = geo.split_once('+')?;
                    let (w, h) = wh.split_once('x')?;
                    let (x, y) = rest.split_once('+')?;
                    Some(Monitor {
                        name: name.to_string(),
                        x: x.parse().ok()?,
                        y: y.parse().ok()?,
                        w: w.parse().ok()?,
                        h: h.parse().ok()?,
                        primary: i == 0,
                    })
                })
                .collect();
            if !v.is_empty() {
                return v;
            }
        }
        let mut out = Vec::new();
        if let Ok(Ok(r)) = self
            .c
            .randr_get_monitors(self.root, true)
            .map(|c| c.reply())
        {
            for m in r.monitors {
                let name = self
                    .c
                    .get_atom_name(m.name)
                    .ok()
                    .and_then(|c| c.reply().ok())
                    .map(|n| String::from_utf8_lossy(&n.name).into_owned())
                    .unwrap_or_else(|| "screen".into());
                out.push(Monitor {
                    name,
                    x: m.x as i32,
                    y: m.y as i32,
                    w: m.width as i32,
                    h: m.height as i32,
                    primary: m.primary,
                });
            }
        }
        if out.is_empty() {
            out.push(Monitor {
                name: "screen".into(),
                x: 0,
                y: 0,
                w: self.screen_w,
                h: self.screen_h,
                primary: true,
            });
        }
        if !out.iter().any(|m| m.primary) {
            out[0].primary = true;
        }
        out
    }

    /// The keysym of a key, with Shift (column 1) or AltGr (columns 4/5).
    pub fn keysym(&self, keycode: u8, state: KeyButMask) -> u32 {
        let idx = (keycode.saturating_sub(self.min_keycode)) as usize * self.per as usize;
        let shift = state.contains(KeyButMask::SHIFT) ^ state.contains(KeyButMask::LOCK);
        let altgr = state.contains(KeyButMask::MOD5);
        let col = match (altgr, shift) {
            (true, false) => 4,
            (true, true) => 5,
            (false, true) => 1,
            (false, false) => 0,
        };
        let get = |c: usize| self.keysyms.get(idx + c).copied().unwrap_or(0);
        // Keypad keys follow Num Lock (Mod2).
        let numlock = state.contains(KeyButMask::MOD2);
        let k1 = get(1);
        if numlock && (0xff80..=0xffbd).contains(&k1) {
            return if shift { get(0) } else { k1 };
        }
        let k = get(col);
        if k != 0 {
            return k;
        }
        let base = get(if altgr { 4 } else { 0 });
        if base != 0 {
            if shift && (0x61..=0x7a).contains(&base) {
                return base - 0x20;
            }
            return base;
        }
        get(0)
    }

    fn ensure_shm(&mut self, bytes: usize) -> bool {
        if !self.shm_ok {
            return false;
        }
        if self.shm.as_ref().is_some_and(|s| s.size >= bytes) {
            return true;
        }
        self.drop_shm();
        let size = bytes.next_multiple_of(1 << 20);
        unsafe {
            let id = libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600);
            if id < 0 {
                self.shm_ok = false;
                return false;
            }
            let addr = libc::shmat(id, std::ptr::null(), 0);
            if addr as isize == -1 {
                libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut());
                self.shm_ok = false;
                return false;
            }
            let Ok(seg) = self.c.generate_id() else {
                return false;
            };
            let attached = self
                .c
                .shm_attach(seg, id as u32, false)
                .ok()
                .and_then(|ck| ck.check().ok())
                .is_some();
            libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut());
            if !attached {
                libc::shmdt(addr);
                self.shm_ok = false;
                return false;
            }
            self.shm = Some(Shm {
                seg,
                addr: addr as *mut u8,
                size,
            });
        }
        true
    }

    fn drop_shm(&mut self) {
        if let Some(s) = self.shm.take() {
            let _ = self.c.shm_detach(s.seg);
            unsafe { libc::shmdt(s.addr as *const libc::c_void) };
        }
    }

    /// Release the shared segment (the big windows are gone).
    pub fn shrink_shm(&mut self, keep: usize) {
        if self
            .shm
            .as_ref()
            .is_some_and(|s| s.size > keep.next_multiple_of(1 << 20) * 2)
        {
            self.drop_shm();
        }
    }

    /// Copy the canvas to the window and wait until the server has read it.
    pub fn present(&mut self, win: Window, cv: &Canvas) -> Res<()> {
        let bytes = cv.as_bytes();
        let (w, h) = (cv.w as u16, cv.h as u16);
        if self.ensure_shm(bytes.len()) {
            let s = self.shm.as_ref().unwrap();
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), s.addr, bytes.len()) };
            self.c.shm_put_image(
                win,
                self.gc,
                w,
                h,
                0,
                0,
                w,
                h,
                0,
                0,
                self.depth,
                ImageFormat::Z_PIXMAP.into(),
                false,
                s.seg,
                0,
            )?;
        } else {
            let max = self.c.maximum_request_bytes().saturating_sub(64).max(4096);
            let row_bytes = cv.w as usize * 4;
            let rows = (max / row_bytes).max(1);
            let mut y = 0usize;
            while y < cv.h as usize {
                let n = rows.min(cv.h as usize - y);
                self.c.put_image(
                    ImageFormat::Z_PIXMAP,
                    win,
                    self.gc,
                    w,
                    n as u16,
                    0,
                    y as i16,
                    0,
                    self.depth,
                    &bytes[y * row_bytes..(y + n) * row_bytes],
                )?;
                y += n;
            }
        }
        // A round trip: the next frame may overwrite the shared buffer.
        self.c.get_input_focus()?.reply()?;
        Ok(())
    }

    /// Copy one rectangle of the canvas to the window (the presenter's live
    /// camera picture between full redraws).
    pub fn present_rect(&mut self, win: Window, cv: &Canvas, r: crate::canvas::Rect) -> Res<()> {
        let r = r.intersect(&cv.bounds());
        if r.w <= 0 || r.h <= 0 {
            return Ok(());
        }
        let bytes = cv.as_bytes();
        if !self.ensure_shm(bytes.len()) {
            return self.present(win, cv);
        }
        let s = self.shm.as_ref().unwrap();
        let row = cv.w as usize * 4;
        for y in r.y..r.y + r.h {
            let off = y as usize * row + r.x as usize * 4;
            let n = r.w as usize * 4;
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().add(off), s.addr.add(off), n) };
        }
        self.c.shm_put_image(
            win,
            self.gc,
            cv.w as u16,
            cv.h as u16,
            r.x as u16,
            r.y as u16,
            r.w as u16,
            r.h as u16,
            r.x as i16,
            r.y as i16,
            self.depth,
            ImageFormat::Z_PIXMAP.into(),
            false,
            s.seg,
            0,
        )?;
        self.c.get_input_focus()?.reply()?;
        Ok(())
    }

    // ------------------------------------------------------- clipboard

    pub fn set_clipboard(&mut self, win: Window, text: &str) {
        self.clip_text = Some(text.to_string());
        let _ = self
            .c
            .set_selection_owner(win, self.atoms.clipboard, x11rb::CURRENT_TIME);
        let _ = self.c.flush();
    }

    /// The clipboard text (waits up to half a second for the owner).
    pub fn paste(&mut self, win: Window) -> Option<String> {
        let owner = self
            .c
            .get_selection_owner(self.atoms.clipboard)
            .ok()?
            .reply()
            .ok()?
            .owner;
        if owner == x11rb::NONE {
            return None;
        }
        if owner == win || OWN_WINDOWS.with(|w| w.borrow().contains(&owner)) {
            return self.clip_text.clone();
        }
        let _ = self.c.convert_selection(
            win,
            self.atoms.clipboard,
            self.atoms.utf8,
            self.atoms.sel_prop,
            x11rb::CURRENT_TIME,
        );
        let _ = self.c.flush();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            match self.c.poll_for_event() {
                Ok(Some(Event::SelectionNotify(e))) if e.requestor == win => {
                    if e.property == x11rb::NONE {
                        return None;
                    }
                    let r = self
                        .c
                        .get_property(true, win, self.atoms.sel_prop, AtomEnum::ANY, 0, 1 << 24)
                        .ok()?
                        .reply()
                        .ok()?;
                    return Some(String::from_utf8_lossy(&r.value).into_owned());
                }
                Ok(Some(other)) => {
                    if let Some(ev) = self.translate(other) {
                        self.pending.push_back(ev);
                    }
                }
                Ok(None) => {
                    if std::time::Instant::now() > deadline {
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(_) => return None,
            }
        }
    }

    fn answer(&self, e: &SelectionRequestEvent) {
        let a = &self.atoms;
        let mut prop = if e.property == x11rb::NONE {
            e.target
        } else {
            e.property
        };
        match &self.clip_text {
            Some(text) if e.selection == a.clipboard => {
                if e.target == a.targets {
                    let t = [a.targets, a.utf8, u32::from(AtomEnum::STRING)];
                    let _ = self.c.change_property32(
                        PropMode::REPLACE,
                        e.requestor,
                        prop,
                        AtomEnum::ATOM,
                        &t,
                    );
                } else if e.target == a.utf8 || e.target == u32::from(AtomEnum::STRING) {
                    let _ = self.c.change_property8(
                        PropMode::REPLACE,
                        e.requestor,
                        prop,
                        e.target,
                        text.as_bytes(),
                    );
                } else {
                    prop = x11rb::NONE;
                }
            }
            _ => prop = x11rb::NONE,
        }
        let ev = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: e.time,
            requestor: e.requestor,
            selection: e.selection,
            target: e.target,
            property: prop,
        };
        let _ = self
            .c
            .send_event(false, e.requestor, EventMask::NO_EVENT, ev);
        let _ = self.c.flush();
    }

    // ----------------------------------------------------------- events

    pub fn next_event(&mut self) -> Option<Ev> {
        if let Some(e) = self.pending.pop_front() {
            return Some(e);
        }
        loop {
            let ev = self.c.poll_for_event().ok()??;
            if let Some(e) = self.translate(ev) {
                return Some(e);
            }
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn translate(&mut self, ev: Event) -> Option<Ev> {
        match ev {
            Event::Expose(e) if e.count == 0 => Some(Ev::Expose(e.window)),
            Event::ConfigureNotify(e) => Some(Ev::Configure(
                e.window,
                e.x as i32,
                e.y as i32,
                e.width as i32,
                e.height as i32,
            )),
            Event::KeyPress(e) => {
                let sym = self.keysym(e.detail, e.state);
                if std::env::var_os("NIRUPRES_KEYLOG").is_some() {
                    eprintln!(
                        "nirupres: key {} state {:#x} sym {sym:#x} win {}",
                        e.detail,
                        u16::from(e.state),
                        e.event
                    );
                }
                let st = u16::from(e.state);
                let ctrl = st & u16::from(KeyButMask::CONTROL) != 0;
                let mut ch = crate::keys::keysym_char(sym).filter(|_| !ctrl);
                if (0xfe50..=0xfe60).contains(&sym) {
                    self.dead = Some(sym);
                    return None;
                }
                if let (Some(d), Some(c)) = (self.dead, ch) {
                    self.dead = None;
                    ch = Some(crate::keys::compose(d, c).unwrap_or(c));
                }
                Some(Ev::Key {
                    win: e.event,
                    sym,
                    state: st,
                    ch,
                })
            }
            Event::ButtonPress(e) => Some(Ev::Button {
                win: e.event,
                x: e.event_x as i32,
                y: e.event_y as i32,
                button: e.detail,
                state: u16::from(e.state),
                press: true,
                time: e.time,
            }),
            Event::ButtonRelease(e) => Some(Ev::Button {
                win: e.event,
                x: e.event_x as i32,
                y: e.event_y as i32,
                button: e.detail,
                state: u16::from(e.state),
                press: false,
                time: e.time,
            }),
            Event::MotionNotify(e) => Some(Ev::Motion {
                win: e.event,
                x: e.event_x as i32,
                y: e.event_y as i32,
                state: u16::from(e.state),
            }),
            Event::LeaveNotify(e) => Some(Ev::Leave(e.event)),
            Event::FocusIn(e) => Some(Ev::Focus(e.event, true)),
            Event::FocusOut(e) => Some(Ev::Focus(e.event, false)),
            Event::MapNotify(e) => Some(Ev::Mapped(e.window, true)),
            Event::UnmapNotify(e) => Some(Ev::Mapped(e.window, false)),
            Event::MappingNotify(_) => Some(Ev::Keymap),
            Event::PropertyNotify(e)
                if e.window == self.root && e.atom == self.atoms.niruwm_theme =>
            {
                Some(Ev::Theme)
            }
            Event::SelectionRequest(e) => {
                self.answer(&e);
                None
            }
            Event::SelectionClear(e) if e.selection == self.atoms.clipboard => {
                self.clip_text = None;
                None
            }
            Event::SelectionNotify(e) if e.selection == self.atoms.xdnd_selection => {
                let d = self.dnd.take()?;
                let mut paths = Vec::new();
                if e.property != x11rb::NONE {
                    if let Ok(Ok(r)) = self
                        .c
                        .get_property(
                            true,
                            d.target,
                            self.atoms.dnd_prop,
                            AtomEnum::ANY,
                            0,
                            1 << 22,
                        )
                        .map(|c| c.reply())
                    {
                        paths = parse_uris(&r.value);
                    }
                }
                let fin = ClientMessageEvent::new(
                    32,
                    d.source,
                    self.atoms.xdnd_finished,
                    [
                        d.target,
                        u32::from(!paths.is_empty()),
                        self.atoms.xdnd_copy,
                        0,
                        0,
                    ],
                );
                let _ = self.c.send_event(false, d.source, EventMask::NO_EVENT, fin);
                let _ = self.c.flush();
                Some(Ev::Drop(d.target, d.pos.0, d.pos.1, paths))
            }
            Event::ClientMessage(e) => self.client_message(e),
            _ => None,
        }
    }

    fn client_message(&mut self, e: ClientMessageEvent) -> Option<Ev> {
        let a = &self.atoms;
        let d = e.data.as_data32();
        if e.type_ == a.wm_protocols && d[0] == a.wm_delete {
            return Some(Ev::Close(e.window));
        }
        if e.type_ == a.xdnd_enter {
            self.dnd = Some(Dnd {
                source: d[0],
                target: e.window,
                pos: (0, 0),
            });
            return None;
        }
        if e.type_ == a.xdnd_position {
            let (rx, ry) = ((d[2] >> 16) as i16, (d[2] & 0xffff) as i16);
            let (x, y) = self
                .c
                .translate_coordinates(self.root, e.window, rx, ry)
                .ok()
                .and_then(|c| c.reply().ok())
                .map(|r| (r.dst_x as i32, r.dst_y as i32))
                .unwrap_or((0, 0));
            if let Some(dd) = self.dnd.as_mut() {
                dd.pos = (x, y);
                dd.source = d[0];
            }
            let st =
                ClientMessageEvent::new(32, d[0], a.xdnd_status, [e.window, 1, 0, 0, a.xdnd_copy]);
            let _ = self.c.send_event(false, d[0], EventMask::NO_EVENT, st);
            let _ = self.c.flush();
            return Some(Ev::DndOver(e.window, x, y));
        }
        if e.type_ == a.xdnd_leave {
            self.dnd = None;
            return Some(Ev::DndLeave(e.window));
        }
        if e.type_ == a.xdnd_drop {
            let _ =
                self.c
                    .convert_selection(e.window, a.xdnd_selection, a.uri_list, a.dnd_prop, d[2]);
            let _ = self.c.flush();
            return None;
        }
        None
    }
}

thread_local! {
    pub static OWN_WINDOWS: std::cell::RefCell<Vec<Window>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub fn register_window(w: Window) {
    OWN_WINDOWS.with(|v| v.borrow_mut().push(w));
}

pub fn decode_path(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Local paths from a text/uri-list.
pub fn parse_uris(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut v = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("file://") {
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            v.push(decode_path(rest));
        } else if line.starts_with('/') {
            v.push(line.to_string());
        }
    }
    v
}

impl Drop for X {
    fn drop(&mut self) {
        self.drop_shm();
        let _ = self.c.flush();
    }
}

pub const SHIFT: u16 = 1;
pub const CTRL: u16 = 4;
pub const ALT: u16 = 8;
pub const SUPER: u16 = 64;
