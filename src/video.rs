//! Video slides: the video plays in the audience window through mpv, which
//! draws into a child window at the video rectangle (local files and web
//! URLs; mpv uses yt-dlp for YouTube and Vimeo). Space pauses and plays.
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

pub struct Video {
    pub win: u32,
    child: std::process::Child,
    sock: PathBuf,
    pub key: String,
    pub playing: bool,
}

pub fn available() -> bool {
    crate::presenting::command_path("mpv").is_some()
}

impl Video {
    pub fn start(x: &crate::x::X, parent: u32, rect: (i32, i32, i32, i32), source: &str, autoplay: bool, muted: bool, key: String) -> Option<Self> {
        let mpv = crate::presenting::command_path("mpv")?;
        let win = x
            .create_child(parent, rect.0, rect.1, rect.2.max(1) as u16, rect.3.max(1) as u16)
            .ok()?;
        let sock = std::env::temp_dir().join(format!("nirupres-mpv-{}-{win}.sock", std::process::id()));
        let _ = std::fs::remove_file(&sock);
        let mut cmd = std::process::Command::new(mpv);
        cmd.arg(format!("--wid={win}"))
            .arg("--no-terminal")
            .arg("--really-quiet")
            .arg("--keep-open=yes")
            .arg("--force-window=immediate")
            .arg(format!("--input-ipc-server={}", sock.display()));
        if !autoplay {
            cmd.arg("--pause");
        }
        if muted {
            cmd.arg("--mute=yes");
        }
        cmd.arg("--").arg(source);
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        match cmd.spawn() {
            Ok(child) => Some(Self {
                win,
                child,
                sock,
                key,
                playing: autoplay,
            }),
            Err(_) => {
                x.destroy(win);
                None
            }
        }
    }

    fn send(&self, cmd: &str) {
        if let Ok(mut s) = UnixStream::connect(&self.sock) {
            let _ = s.set_write_timeout(Some(std::time::Duration::from_millis(200)));
            let _ = s.write_all(format!("{cmd}\n").as_bytes());
        }
    }

    pub fn toggle(&mut self) {
        self.playing = !self.playing;
        self.set_pause(!self.playing);
    }

    pub fn set_pause(&mut self, pause: bool) {
        self.playing = !pause;
        self.send(&format!(
            r#"{{"command": ["set_property", "pause", {}]}}"#,
            if pause { "true" } else { "false" }
        ));
    }

    pub fn set_mute(&self, mute: bool) {
        self.send(&format!(
            r#"{{"command": ["set_property", "mute", {}]}}"#,
            if mute { "true" } else { "false" }
        ));
    }

    pub fn stop(mut self, x: &crate::x::X) {
        self.send(r#"{"command": ["quit"]}"#);
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.sock);
        x.destroy(self.win);
    }
}
