"""Presenting mode for the NIRUX session (2.3).

While a presentation runs NIRUPRES asks NIRUWM for presenting mode:
``niructl present on --pid PID`` turns Do Not Disturb on, hides private bar
modules and is switched off by NIRUWM itself if NIRUPRES dies. With an older
NIRUWM that lacks ``present`` it falls back to ``niructl dnd on`` and restores
the previous DND state afterwards. Without niructl nothing happens.

The state is written to a small file so a crash in the fallback mode is
repaired the next time NIRUPRES starts.

``NIRUPRES_NIRUCTL`` overrides the niructl path; ``none`` disables it.
"""

import json
import os
import shutil
import subprocess

from .foundation import DATA

STATE = DATA / "presenting.json"


def _niructl():
    forced = os.environ.get("NIRUPRES_NIRUCTL")
    if forced:
        return None if forced.lower() == "none" else forced
    return shutil.which("niructl")


def _run(args, timeout=3):
    exe = _niructl()
    if not exe:
        return None
    try:
        r = subprocess.run(
            [exe, *args], capture_output=True, text=True, timeout=timeout,
            stdin=subprocess.DEVNULL,
        )
        return r.returncode, (r.stdout or "").strip()
    except (OSError, subprocess.SubprocessError):
        return None


def _alive(pid):
    try:
        os.kill(int(pid), 0)
        return True
    except (OSError, ValueError, TypeError):
        return False


def _write(state):
    try:
        STATE.parent.mkdir(parents=True, exist_ok=True)
        STATE.write_text(json.dumps(state), encoding="utf-8")
    except OSError:
        pass


def _clear():
    try:
        STATE.unlink()
    except OSError:
        pass


class PresentingMode:
    """Turn presenting mode on for one presentation session."""

    def __init__(self):
        self.mode = None  # "present", "dnd" or None
        self.prior = None

    def start(self, pid=None):
        pid = int(pid or os.getpid())
        if not _niructl():
            return None
        status = _run(["present", "status"])
        if status and status[0] == 0 and status[1] in ("on", "off"):
            r = _run(["present", "on", "--pid", str(pid)])
            if r and r[0] == 0:
                self.mode = "present"
                _write({"mode": "present", "pid": pid})
                return self.mode
        dnd = _run(["dnd", "status"])
        if dnd and dnd[0] == 0 and dnd[1].endswith(("on", "off")):
            self.prior = "on" if dnd[1].endswith("on") else "off"
            # Save first: if NIRUPRES dies after this, the next start restores it.
            _write({"mode": "dnd", "pid": pid, "prior": self.prior})
            if self.prior == "off":
                _run(["dnd", "on"])
            self.mode = "dnd"
            return self.mode
        return None

    def stop(self):
        if self.mode == "present":
            _run(["present", "off"])
        elif self.mode == "dnd" and self.prior == "off":
            _run(["dnd", "off"])
        if self.mode:
            _clear()
        self.mode = None
        self.prior = None


def recover_stale():
    """Undo presenting mode left behind by a NIRUPRES that crashed."""
    try:
        state = json.loads(STATE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    pid = state.get("pid")
    if pid and int(pid) != os.getpid() and _alive(pid):
        return None  # another NIRUPRES is presenting right now
    mode = state.get("mode")
    if mode == "dnd" and state.get("prior") == "off":
        _run(["dnd", "off"])
    elif mode == "present":
        status = _run(["present", "status"])
        if status and status[1] == "on":
            _run(["present", "off"])
    _clear()
    return mode
