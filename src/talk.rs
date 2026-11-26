//! Talk mode: NIRUCLIP owns the capture process and its state file;
//! NIRUPRES starts and stops it, marks each slide change and writes the
//! chapters (`.chapters.json`) and subtitles (`.srt`) beside the recording.
//!
//! The recording is the audience picture only: NIRUCLIP (0.14 or newer)
//! records the audience window by its X id (`--window-id`), so the
//! presenter view with notes, timer and next slide can never be in it, on
//! one screen or several. On one screen the audience window is an
//! off-screen recording stage (see `present.rs`).
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const ACTIVE: [&str; 4] = ["countdown", "starting", "recording", "finalizing"];
const DONE: [&str; 4] = ["finished", "failed", "interrupted", "cancelled"];

pub fn state_path() -> PathBuf {
    if let Some(d) = std::env::var_os("NIRUCLIP_STATE_DIR").filter(|v| !v.is_empty()) {
        return crate::prefs::expand(&d.to_string_lossy()).join("session.json");
    }
    let root = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| crate::prefs::expand(&v))
        .unwrap_or_else(|| crate::prefs::home().join(".local/state"));
    root.join("niruclip/session.json")
}

pub fn read_state() -> serde_json::Map<String, serde_json::Value> {
    crate::prefs::read_obj(&state_path())
}

/// The recorder named in the state file still runs (a crashed or killed
/// NIRUCLIP leaves an active state behind; NIRUCLIP itself ignores it).
fn recorder_alive(st: &serde_json::Map<String, serde_json::Value>) -> bool {
    let Some(pid) = crate::model::to_f64(st.get("pid")).filter(|p| *p > 0.0) else {
        return false;
    };
    std::fs::read(format!("/proc/{}/cmdline", pid as u32))
        .map(|c| String::from_utf8_lossy(&c).contains("niruclip"))
        .unwrap_or(false)
}

fn kind(st: &serde_json::Map<String, serde_json::Value>) -> String {
    st.get("state")
        .map(|v| crate::model::py_str(Some(v)))
        .unwrap_or_default()
}

pub fn timestamp() -> String {
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

pub fn safe_stem(v: &str) -> String {
    let t: String = v
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "._- ".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_matches(|c| " .-_".contains(c));
    let s: String = t.chars().take(80).collect();
    if s.is_empty() {
        "presentation".into()
    } else {
        s
    }
}

pub fn srt_time(seconds: f64) -> String {
    let ms = (seconds * 1000.0).round().max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        ms % 3_600_000 / 60_000,
        ms % 60_000 / 1000,
        ms % 1000
    )
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub slide: usize,
    pub title: String,
    pub seconds: f64,
}

pub fn srt_text(markers: &[Marker], duration: f64) -> String {
    let mut m = markers.to_vec();
    m.sort_by(|a, b| {
        a.seconds
            .partial_cmp(&b.seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut rows = Vec::new();
    for (i, k) in m.iter().enumerate() {
        let start = k.seconds;
        let end = m
            .get(i + 1)
            .map(|n| n.seconds)
            .unwrap_or(duration)
            .max(start + 0.6);
        let title = if k.title.trim().is_empty() {
            format!("Slide {}", k.slide)
        } else {
            k.title.trim().to_string()
        };
        rows.push(format!(
            "{}\n{} --> {}\n{:02} · {title}\n",
            i + 1,
            srt_time(start),
            srt_time(end),
            k.slide
        ));
    }
    rows.join("\n")
}

/// What Talk mode records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The slides, the microphone and a small camera bottom right.
    SlidesCamera,
    /// The slides and the microphone.
    Slides,
    /// The camera and the microphone (no slides).
    Camera,
}

/// A Talk mode choice (dialog, palette or `--talk FILE [PROFILE]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub source: Source,
    /// Also record the computer's sound (videos in the slides).
    pub video_sound: bool,
}

/// The camera overlay size NIRUCLIP draws (`--webcam-size tiny`): 16 % of
/// the picture height, about 16 % of the width on a 16:9 slide.
pub const CAMERA_SHARE: f32 = 0.16;
/// Its margin from the edges, as a share of the picture height.
pub const CAMERA_MARGIN: f32 = 0.025;

/// The oldest NIRUCLIP that can record one window's content.
pub const NIRUCLIP_MIN: (u32, u32) = (0, 14);

/// True when a regular video capture device is present (the first node of
/// a camera has index 0; infrared sensors are skipped by name).
pub fn camera_present() -> bool {
    if let Ok(v) = std::env::var("NIRUPRES_TALK_CAMERA") {
        return v == "1";
    }
    let Ok(rd) = std::fs::read_dir("/sys/class/video4linux") else {
        return false;
    };
    rd.flatten().any(|e| {
        let p = e.path();
        let index = std::fs::read_to_string(p.join("index")).unwrap_or_default();
        let name = std::fs::read_to_string(p.join("name"))
            .unwrap_or_default()
            .to_lowercase();
        index.trim() == "0" && !name.contains("infrared") && !name.contains(" ir ")
    })
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Self::SlidesCamera => "slides-camera",
            Self::Slides => "slides",
            Self::Camera => "camera",
        }
    }

    pub fn records_slides(self) -> bool {
        self != Self::Camera
    }
}

impl Choice {
    /// `slides-camera`, `slides`, `camera` or `auto` (also the NIRUCLIP
    /// profile names of earlier versions: screen-camera, screen-mic,
    /// camera-mic). `auto` picks the camera when one is connected.
    pub fn parse(profile: &str, video_sound: bool) -> Option<Self> {
        let source = match profile.trim() {
            "" | "auto" => {
                if camera_present() {
                    Source::SlidesCamera
                } else {
                    Source::Slides
                }
            }
            "slides-camera" | "screen-camera" => Source::SlidesCamera,
            "slides" | "slides-mic" | "screen-mic" => Source::Slides,
            "camera" | "camera-mic" => Source::Camera,
            _ => return None,
        };
        Some(Self {
            source,
            video_sound,
        })
    }

    pub fn label(self) -> String {
        let base = match self.source {
            Source::SlidesCamera => "slides + microphone + camera",
            Source::Slides => "slides + microphone",
            Source::Camera => "camera + microphone",
        };
        if self.video_sound {
            format!("{base} + video sound")
        } else {
            base.into()
        }
    }

    /// The NIRUCLIP options. Everything is explicit, so a NIRUCLIP default
    /// profile cannot turn this into a full-screen recording.
    pub fn niruclip_args(self, window: Option<u32>, output: &Path) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        match (self.source, window) {
            (Source::Camera, _) => v.push("--webcam-only".into()),
            (_, Some(id)) => {
                v.push("--window-id".into());
                v.push(format!("0x{id:x}"));
                if self.source == Source::SlidesCamera {
                    for a in [
                        "--webcam",
                        "corner",
                        "--corner",
                        "bottom-right",
                        "--webcam-size",
                        "tiny",
                        "--camera-preview",
                    ] {
                        v.push(a.into());
                    }
                } else {
                    v.push("--webcam".into());
                    v.push("off".into());
                }
            }
            (_, None) => {}
        }
        v.push("--audio".into());
        v.push(if self.video_sound { "both" } else { "mic" }.into());
        v.push("--countdown".into());
        v.push("3".into());
        v.push("--output".into());
        v.push(output.display().to_string());
        v
    }
}

/// `NIRUCLIP 0.14.0` → (0, 14).
pub fn parse_version(text: &str) -> Option<(u32, u32)> {
    let v = text
        .split_whitespace()
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()))?;
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn niruclip_version(cmd: &Path) -> Option<(u32, u32)> {
    let out = std::process::Command::new(cmd)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    parse_version(&String::from_utf8_lossy(&out.stdout))
}

/// What the presenter view shows about the recording.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    /// countdown, starting, recording, finalizing, finished, failed, …
    pub state: String,
    pub reason: String,
    /// The live camera picture NIRUCLIP keeps (`--camera-preview`).
    pub preview: Option<PathBuf>,
    /// The camera status (ok, missing, privacy, …) and its message.
    pub camera: String,
    pub camera_message: String,
}

pub fn live_state() -> Live {
    let st = read_state();
    let s = |k: &str| {
        st.get(k)
            .map(|v| crate::model::py_str(Some(v)))
            .unwrap_or_default()
    };
    let cam = st.get("camera").and_then(|v| v.as_object());
    let cs = |k: &str| {
        cam.and_then(|c| c.get(k))
            .map(|v| crate::model::py_str(Some(v)))
            .unwrap_or_default()
    };
    Live {
        state: s("state"),
        reason: s("reason"),
        preview: st
            .get("camera_preview")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        camera: cs("status"),
        camera_message: cs("message"),
    }
}

pub enum Poll {
    Waiting,
    Started(String),
    Failed(String),
}

pub struct Talk {
    pub deck: Option<PathBuf>,
    titles: Vec<String>,
    pub choice: Choice,
    process: Option<std::process::Child>,
    pub output: PathBuf,
    log: PathBuf,
    started_at: f64,
    pub markers: Vec<Marker>,
    last: Option<usize>,
    pub recording: Option<PathBuf>,
    deadline: Instant,
    /// Recording has started (the REC clock runs from here).
    pub since: Option<Instant>,
}

pub struct TalkResult {
    pub recording: PathBuf,
    pub chapters: PathBuf,
    pub subtitles: PathBuf,
    pub markers: usize,
    pub duration: f64,
    pub warnings: Vec<String>,
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

impl Talk {
    pub fn new(deck: Option<&Path>, titles: Vec<String>, choice: Choice) -> Self {
        let prefix = safe_stem(
            &deck
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into()),
        );
        let videos = std::env::var("NIRUPRES_TALK_DIR")
            .ok()
            .filter(|v| !v.is_empty())
            .map(|v| crate::prefs::expand(&v))
            .unwrap_or_else(|| crate::prefs::home().join("Videos"));
        let _ = std::fs::create_dir_all(&videos);
        let output = videos.join(format!("NIRUPRES-{prefix}-{}.mkv", timestamp()));
        Self {
            deck: deck.map(Path::to_path_buf),
            titles,
            choice,
            process: None,
            log: output.with_extension("log"),
            output,
            started_at: 0.0,
            markers: Vec::new(),
            last: None,
            recording: None,
            deadline: Instant::now(),
            since: None,
        }
    }

    /// Everything that can be checked before the presentation opens.
    pub fn check(&self) -> Result<PathBuf, String> {
        let st = read_state();
        if ACTIVE.contains(&kind(&st).as_str()) && recorder_alive(&st) {
            return Err("NIRUCLIP is already recording. Stop it before starting Talk mode.".into());
        }
        let cmd = crate::presenting::command_path("niruclip")
            .ok_or("NIRUCLIP is not installed or not in PATH.")?;
        if self.choice.source.records_slides() {
            // Never fall back to a screen recording: it could show notes.
            match niruclip_version(&cmd) {
                Some(v) if v >= NIRUCLIP_MIN => {}
                v => {
                    return Err(format!(
                        "Talk mode records only the slides and needs NIRUCLIP {}.{} or newer (found {}). Update NIRUCLIP; nothing was recorded.",
                        NIRUCLIP_MIN.0,
                        NIRUCLIP_MIN.1,
                        v.map(|(a, b)| format!("{a}.{b}")).unwrap_or_else(|| "an unknown version".into())
                    ))
                }
            }
        }
        Ok(cmd)
    }

    /// Start NIRUCLIP. `window` is the audience window: the only thing
    /// recorded besides camera and sound.
    pub fn start(&mut self, window: Option<u32>) -> Result<(), String> {
        let cmd = self.check()?;
        if self.choice.source.records_slides() && window.is_none() {
            return Err("The audience window is missing; nothing was recorded.".into());
        }
        let log = std::fs::File::create(&self.log)
            .map_err(|e| format!("Could not start NIRUCLIP: {e}"))?;
        let log2 = log.try_clone().map_err(|e| e.to_string())?;
        use std::os::unix::process::CommandExt;
        let mut c = std::process::Command::new(cmd);
        c.args(self.choice.niruclip_args(window, &self.output))
            .stdin(std::process::Stdio::null())
            .stdout(log)
            .stderr(log2);
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let child = c
            .spawn()
            .map_err(|e| format!("Could not start NIRUCLIP: {e}"))?;
        self.process = Some(child);
        self.deadline = Instant::now() + Duration::from_secs(15);
        Ok(())
    }

    fn log_tail(&self, fallback: &str) -> String {
        let t = std::fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = t.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let tail = lines[lines.len().saturating_sub(4)..].join(" ");
        let tail: String = tail.chars().take(500).collect();
        if tail.is_empty() {
            fallback.into()
        } else {
            tail
        }
    }

    /// Called every 150 ms until recording starts.
    pub fn poll(&mut self) -> Poll {
        let st = read_state();
        let k = kind(&st);
        if k == "recording" {
            // The recording's zero (NIRUCLIP 0.14); `started` is when the
            // countdown began.
            self.started_at = crate::model::to_f64(st.get("capture_started"))
                .or_else(|| crate::model::to_f64(st.get("started")))
                .unwrap_or_else(now_secs);
            let out = st
                .get("output")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| self.output.clone());
            self.recording = Some(out.clone());
            self.since = Some(Instant::now());
            return Poll::Started(out.display().to_string());
        }
        if ["failed", "interrupted", "cancelled"].contains(&k.as_str()) {
            let reason = st
                .get("reason")
                .map(|v| crate::model::py_str(Some(v)))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("NIRUCLIP state: {k}"));
            self.stop_process();
            return Poll::Failed(reason);
        }
        if let Some(p) = self.process.as_mut() {
            if let Ok(Some(_)) = p.try_wait() {
                let m = self.log_tail("NIRUCLIP exited before recording started");
                return Poll::Failed(m);
            }
        }
        if Instant::now() > self.deadline {
            self.stop_process();
            return Poll::Failed("NIRUCLIP did not start recording within 15 seconds.".into());
        }
        Poll::Waiting
    }

    /// The slide on screen when the recording started: chapter 1 at 0:00.
    pub fn mark_start(&mut self, index: usize) {
        if self.started_at == 0.0 || index >= self.titles.len() {
            return;
        }
        self.last = Some(index);
        self.markers.push(Marker {
            slide: index + 1,
            title: self.titles[index].clone(),
            seconds: 0.0,
        });
    }

    /// NIRUCLIP's process has ended (a crash, a kill, or it finished on
    /// its own, for example when the disk filled up).
    pub fn process_ended(&mut self) -> bool {
        self.process
            .as_mut()
            .is_some_and(|p| matches!(p.try_wait(), Ok(Some(_))))
    }

    pub fn mark_slide(&mut self, index: usize) {
        if self.started_at == 0.0 || self.last == Some(index) || index >= self.titles.len() {
            return;
        }
        self.last = Some(index);
        self.markers.push(Marker {
            slide: index + 1,
            title: self.titles[index].clone(),
            seconds: (now_secs() - self.started_at).max(0.0),
        });
    }

    fn stop_process(&mut self) {
        if let Some(mut p) = self.process.take() {
            if let Ok(None) = p.try_wait() {
                unsafe { libc::kill(p.id() as i32, libc::SIGTERM) };
                let t = Instant::now();
                while t.elapsed() < Duration::from_secs(2) {
                    if let Ok(Some(_)) = p.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                let _ = p.kill();
            }
            let _ = p.wait();
        }
    }

    fn niruclip_stop(timeout: u64) {
        if let Some(cmd) = crate::presenting::command_path("niruclip") {
            if let Ok(mut c) = std::process::Command::new(cmd)
                .arg("--stop")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                let t = Instant::now();
                while t.elapsed() < Duration::from_secs(timeout) {
                    if let Ok(Some(_)) = c.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }

    /// Stop recording and write the chapters (runs in a worker thread).
    pub fn finish(mut self) -> Result<TalkResult, String> {
        Self::niruclip_stop(25);
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut st = read_state();
        while Instant::now() < deadline {
            st = read_state();
            if DONE.contains(&kind(&st).as_str()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if let Some(mut p) = self.process.take() {
            let t = Instant::now();
            loop {
                if let Ok(Some(_)) = p.try_wait() {
                    break;
                }
                if t.elapsed() > Duration::from_secs(3) {
                    unsafe { libc::kill(p.id() as i32, libc::SIGTERM) };
                    let _ = p.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let k = kind(&st);
        let recording = st
            .get("output")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or(self.recording.clone())
            .unwrap_or(self.output.clone());
        if k != "finished" || !recording.is_file() {
            let reason = st
                .get("reason")
                .map(|v| crate::model::py_str(Some(v)))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    if k.is_empty() {
                        "recording was not finalized".into()
                    } else {
                        k.clone()
                    }
                });
            let hint = if ["interrupted", "failed"].contains(&k.as_str()) || reason.contains("gone")
            {
                "\n\nWhat was recorded so far can usually be saved with: niruclip --recover"
            } else {
                ""
            };
            return Err(format!("Talk recording did not finalize: {reason}{hint}"));
        }
        let mut duration = probe_duration(&recording);
        if duration <= 0.0 {
            duration = self.markers.iter().map(|m| m.seconds).fold(1.0, f64::max);
        }
        let chapters = recording.with_extension("chapters.json");
        let subtitles = recording.with_extension("srt");
        let payload = json!({
            "schema": 1,
            "recording": recording.display().to_string(),
            "deck": self.deck.as_ref().map(|d| d.display().to_string()).unwrap_or_default(),
            "profile": self.choice.source.name(),
            "videoSound": self.choice.video_sound,
            "started": self.started_at,
            "duration": duration,
            "markers": self.markers.iter().map(|m| json!({"slide": m.slide, "title": m.title, "seconds": m.seconds})).collect::<Vec<_>>(),
        });
        let mut warnings = Vec::new();
        let tmp = chapters.with_extension("tmp");
        let r = std::fs::write(&tmp, crate::package::py_json_project(&payload) + "\n")
            .and_then(|_| std::fs::rename(&tmp, &chapters))
            .and_then(|_| std::fs::write(&subtitles, srt_text(&self.markers, duration)));
        if let Err(e) = r {
            warnings.push(format!("Chapters could not be written: {e}"));
        }
        Ok(TalkResult {
            recording,
            chapters,
            subtitles,
            markers: self.markers.len(),
            duration,
            warnings,
        })
    }

    pub fn abort(mut self) {
        Self::niruclip_stop(8);
        self.stop_process();
    }
}

fn probe_duration(p: &Path) -> f64 {
    let Some(probe) = crate::presenting::command_path("ffprobe") else {
        return 0.0;
    };
    std::process::Command::new(probe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(p)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(0.0)
        .max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_record_the_window_never_the_screen() {
        let out = Path::new("/v/t.mkv");
        let c = Choice::parse("slides-camera", false).unwrap();
        let a = c.niruclip_args(Some(0x1a00003), out).join(" ");
        assert!(a.starts_with("--window-id 0x1a00003 --webcam corner --corner bottom-right --webcam-size tiny --camera-preview"));
        assert!(a.ends_with("--audio mic --countdown 3 --output /v/t.mkv"));
        assert!(!a.contains("--full") && !a.contains("--profile"));
        let s = Choice::parse("screen-mic", true).unwrap();
        assert_eq!(s.source, Source::Slides);
        let a = s.niruclip_args(Some(42), out).join(" ");
        assert!(a.contains("--window-id 0x2a --webcam off --audio both"));
        let cam = Choice::parse("camera-mic", false).unwrap();
        assert!(cam
            .niruclip_args(None, out)
            .join(" ")
            .starts_with("--webcam-only --audio mic"));
        assert!(Choice::parse("everything", false).is_none());
        assert_eq!(parse_version("NIRUCLIP 0.14.0\n"), Some((0, 14)));
        assert_eq!(parse_version("NIRUCLIP 0.13.1"), Some((0, 13)));
        assert!(parse_version("NIRUCLIP 0.13.1").unwrap() < NIRUCLIP_MIN);
    }

    #[test]
    fn srt_and_names() {
        assert_eq!(srt_time(3661.5), "01:01:01,500");
        assert_eq!(safe_stem("  My deck / v2 "), "My deck - v2");
        let m = vec![
            Marker {
                slide: 1,
                title: "Intro".into(),
                seconds: 0.0,
            },
            Marker {
                slide: 2,
                title: "".into(),
                seconds: 5.0,
            },
        ];
        let s = srt_text(&m, 9.0);
        assert!(s.starts_with("1\n00:00:00,000 --> 00:00:05,000\n01 · Intro\n"));
        assert!(s.contains("02 · Slide 2"));
    }
}
