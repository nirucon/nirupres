//! Presenting mode for the NIRUX session: `niructl present on --pid PID`
//! (Do Not Disturb, private bar modules hidden; NIRUWM switches it off if
//! NIRUPRES dies). With an older NIRUWM the fallback is `niructl dnd on`
//! and the previous state is restored, also after a crash on the next
//! start. `NIRUPRES_NIRUCTL` overrides the binary; `none` disables it.
use serde_json::json;

fn state_file() -> std::path::PathBuf {
    crate::prefs::file("presenting.json")
}

pub fn command_path(name: &str) -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(name);
            if is_exec(&p) {
                return Some(p);
            }
        }
    }
    let home = crate::prefs::home();
    for p in [
        home.join(".local/bin").join(name),
        std::path::PathBuf::from("/usr/local/bin").join(name),
        std::path::PathBuf::from("/usr/bin").join(name),
    ] {
        if is_exec(&p) {
            return Some(p);
        }
    }
    None
}

fn is_exec(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn niructl() -> Option<std::path::PathBuf> {
    match std::env::var("NIRUPRES_NIRUCTL") {
        Ok(v) if !v.is_empty() => {
            if v.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(v.into())
            }
        }
        _ => command_path("niructl"),
    }
}

/// Run niructl with a 3 second limit: (exit code, trimmed stdout).
fn run(args: &[&str]) -> Option<(i32, String)> {
    let exe = niructl()?;
    let mut child = std::process::Command::new(exe)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let mut out = String::new();
                if let Some(mut so) = child.stdout.take() {
                    use std::io::Read;
                    let _ = so.read_to_string(&mut out);
                }
                return Some((st.code().unwrap_or(-1), out.trim().to_string()));
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn alive(pid: i64) -> bool {
    pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0
}

fn write(v: serde_json::Value) {
    let p = state_file();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, crate::package::py_json(&v));
}

#[derive(Default)]
pub struct PresentingMode {
    mode: Option<&'static str>,
    prior: Option<String>,
}

impl PresentingMode {
    pub fn start(&mut self, pid: u32) {
        if niructl().is_none() {
            return;
        }
        if let Some((0, s)) = run(&["present", "status"]) {
            if s == "on" || s == "off" {
                if let Some((0, _)) = run(&["present", "on", "--pid", &pid.to_string()]) {
                    self.mode = Some("present");
                    write(json!({"mode": "present", "pid": pid}));
                    return;
                }
            }
        }
        if let Some((0, s)) = run(&["dnd", "status"]) {
            if s.ends_with("on") || s.ends_with("off") {
                let prior = if s.ends_with("on") { "on" } else { "off" };
                // Saved first: a crash after this is repaired on the next start.
                write(json!({"mode": "dnd", "pid": pid, "prior": prior}));
                if prior == "off" {
                    run(&["dnd", "on"]);
                }
                self.mode = Some("dnd");
                self.prior = Some(prior.into());
            }
        }
    }

    pub fn stop(&mut self) {
        match self.mode {
            Some("present") => {
                run(&["present", "off"]);
            }
            Some("dnd") if self.prior.as_deref() == Some("off") => {
                run(&["dnd", "off"]);
            }
            _ => {}
        }
        if self.mode.is_some() {
            let _ = std::fs::remove_file(state_file());
        }
        self.mode = None;
        self.prior = None;
    }
}

/// Undo presenting mode left behind by a NIRUPRES that crashed.
pub fn recover_stale() {
    let Some(v) = crate::prefs::read_json(&state_file()) else {
        return;
    };
    let pid = crate::model::to_i64(v.get("pid")).unwrap_or(0);
    if pid != 0 && pid != std::process::id() as i64 && alive(pid) {
        return; // another NIRUPRES is presenting right now
    }
    let mode = v.get("mode").and_then(|m| m.as_str()).unwrap_or("");
    if mode == "dnd" && v.get("prior").and_then(|p| p.as_str()) == Some("off") {
        run(&["dnd", "off"]);
    } else if mode == "present" {
        if let Some((_, s)) = run(&["present", "status"]) {
            if s == "on" {
                run(&["present", "off"]);
            }
        }
    }
    let _ = std::fs::remove_file(state_file());
}
