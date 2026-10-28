"""Presentation recording orchestration.

NIRUPRES owns the presentation; NIRUCLIP owns the capture process and its
versioned state. This module only starts/stops the CLI, observes its public
state file and writes chapter sidecars next to the recording.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from PySide6.QtCore import QObject, QTimer, Signal

from .theme import command_path

ACTIVE = {"countdown", "starting", "recording", "finalizing"}
DONE = {"finished", "failed", "interrupted", "cancelled"}


def state_path() -> Path:
    if os.environ.get("NIRUCLIP_STATE_DIR"):
        return Path(os.environ["NIRUCLIP_STATE_DIR"]).expanduser() / "session.json"
    base = os.environ.get("XDG_STATE_HOME")
    root = Path(base).expanduser() if base else Path.home() / ".local/state"
    return root / "niruclip/session.json"


def read_state() -> dict:
    try:
        value = json.loads(state_path().read_text(encoding="utf-8"))
        return value if isinstance(value, dict) else {}
    except (OSError, json.JSONDecodeError):
        return {}


def timestamp() -> str:
    return datetime.now().strftime("%Y%m%d-%H%M%S")


def safe_stem(value: str) -> str:
    text = "".join(ch if ch.isalnum() or ch in "._- " else "-" for ch in value.strip())
    text = " ".join(text.split()).strip(" .-_")
    return text[:80] or "presentation"


def clock(seconds: float) -> str:
    total = max(0, int(seconds))
    return f"{total // 3600:02d}:{total % 3600 // 60:02d}:{total % 60:02d}"


def srt_time(seconds: float) -> str:
    milliseconds = max(0, int(round(seconds * 1000)))
    hours, milliseconds = divmod(milliseconds, 3_600_000)
    minutes, milliseconds = divmod(milliseconds, 60_000)
    seconds, milliseconds = divmod(milliseconds, 1_000)
    return f"{hours:02d}:{minutes:02d}:{seconds:02d},{milliseconds:03d}"


def srt_text(markers: list[dict], duration: float) -> str:
    rows = []
    markers = sorted(markers, key=lambda marker: float(marker.get("seconds", 0)))
    for index, marker in enumerate(markers):
        start = float(marker.get("seconds", 0))
        end = float(markers[index + 1].get("seconds", duration)) if index + 1 < len(markers) else duration
        end = max(start + 0.6, end)
        title = str(marker.get("title") or f"Slide {marker.get('slide', index + 1)}").strip()
        number = int(marker.get("slide", index + 1))
        rows.append(f"{index + 1}\n{srt_time(start)} --> {srt_time(end)}\n{number:02d} · {title}\n")
    return "\n".join(rows)


@dataclass
class TalkResult:
    recording: Path
    chapters: Path
    subtitles: Path
    markers: list[dict]
    duration: float
    profile: str = ""
    warnings: list[str] = field(default_factory=list)


class TalkSession(QObject):
    started = Signal(str)
    markerAdded = Signal(dict)
    failed = Signal(str)
    finished = Signal(object)

    def __init__(self, deck_path: Path | None, slides: list[dict], profile: str = "screen-mic", parent=None):
        super().__init__(parent)
        self.deck_path = Path(deck_path) if deck_path else None
        self.slides = list(slides) if isinstance(slides, list) else []
        self.profile = profile or "screen-mic"
        self.process: subprocess.Popen | None = None
        self.log = None
        self.started_at = 0.0
        self.markers: list[dict] = []
        self.last_slide: int | None = None
        self.recording: Path | None = None
        self.output: Path | None = None
        self._start_deadline = 0.0
        self._finishing = False
        self._closed = False
        prefix = safe_stem(self.deck_path.stem if self.deck_path else "Untitled")
        videos = Path(os.environ.get("NIRUPRES_TALK_DIR", "")).expanduser() if os.environ.get("NIRUPRES_TALK_DIR") else Path.home() / "Videos"
        videos.mkdir(parents=True, exist_ok=True)
        self.output = videos / f"NIRUPRES-{prefix}-{timestamp()}.mkv"
        self.timer = QTimer(self)
        self.timer.setInterval(150)
        self.timer.timeout.connect(self._poll_start)

    def start(self) -> bool:
        current = read_state()
        if current.get("state") in ACTIVE:
            self.failed.emit("NIRUCLIP is already recording. Stop it before starting Talk mode.")
            return False
        command = command_path("niruclip")
        if not command:
            self.failed.emit("NIRUCLIP is not installed or not in PATH.")
            return False
        self.log = open(self.output.with_suffix(".log"), "w", encoding="utf-8")
        args = [command, "--profile", self.profile, "--countdown", "3", "--output", str(self.output)]
        try:
            self.process = subprocess.Popen(
                args,
                stdin=subprocess.DEVNULL,
                stdout=self.log,
                stderr=self.log,
                start_new_session=True,
            )
        except OSError as error:
            self.log.close(); self.log = None
            self.failed.emit(f"Could not start NIRUCLIP: {error}")
            return False
        self._start_deadline = time.monotonic() + 15.0
        self.timer.start()
        return True

    def _fail(self, message: str):
        if self._closed:
            return
        self._closed = True
        self.timer.stop()
        self._stop_process()
        self.failed.emit(message)

    def _poll_start(self):
        if self._closed:
            return
        state = read_state()
        kind = str(state.get("state", ""))
        if kind == "recording":
            self.started_at = float(state.get("started", time.time()))
            self.recording = Path(str(state.get("output") or self.output))
            self.timer.stop()
            self.started.emit(str(self.recording))
            return
        if kind in ("failed", "interrupted", "cancelled"):
            self._fail(str(state.get("reason") or f"NIRUCLIP state: {kind}"))
            return
        if self.process is not None and self.process.poll() is not None:
            self._fail(self._log_tail("NIRUCLIP exited before recording started"))
            return
        if time.monotonic() > self._start_deadline:
            self._stop_process()
            self._fail("NIRUCLIP did not start recording within 15 seconds.")
        # Continue with the countdown; the state file is authoritative.

    def _log_tail(self, fallback: str) -> str:
        try:
            if self.log:
                self.log.flush()
            text = self.output.with_suffix(".log").read_text(encoding="utf-8", errors="replace")
            lines = [line.strip() for line in text.splitlines() if line.strip()]
            return " ".join(lines[-4:])[:500] or fallback
        except OSError:
            return fallback

    def mark_slide(self, index: int):
        if self._closed or not self.started_at:
            return
        if index == self.last_slide:
            return
        if not (0 <= index < len(self.slides)):
            return
        title = str(self.slides[index].get("title") or self.slides[index].get("layout") or f"Slide {index + 1}")
        marker = {
            "slide": index + 1,
            "title": title,
            "seconds": max(0.0, time.time() - self.started_at),
        }
        self.last_slide = index
        self.markers.append(marker)
        self.markerAdded.emit(marker)

    def finish(self):
        if self._closed or self._finishing:
            return
        self._finishing = True
        self.timer.stop()
        command = command_path("niruclip")
        if command:
            try:
                subprocess.run([command, "--stop"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=25, check=False)
            except (OSError, subprocess.TimeoutExpired):
                pass
        deadline = time.monotonic() + 25
        state = {}
        while time.monotonic() < deadline:
            state = read_state()
            if state.get("state") in DONE:
                break
            time.sleep(0.2)
        if self.process is not None:
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.terminate()
        if self.log:
            try: self.log.close()
            except OSError: pass
            self.log = None
        kind = str(state.get("state", ""))
        recording = Path(str(state.get("output") or self.recording or self.output))
        if kind != "finished" or not recording.is_file():
            reason = str(state.get("reason") or kind or "recording was not finalized")
            self._fail(f"Talk recording did not finalize: {reason}")
            return
        duration = self._duration(recording)
        if duration <= 0:
            duration = max([marker["seconds"] for marker in self.markers] + [1.0])
        chapters = recording.with_suffix(".chapters.json")
        subtitles = recording.with_suffix(".srt")
        payload = {
            "schema": 1,
            "recording": str(recording),
            "deck": str(self.deck_path) if self.deck_path else "",
            "profile": self.profile,
            "started": self.started_at,
            "duration": duration,
            "markers": self.markers,
        }
        warnings = []
        try:
            temporary = chapters.with_suffix(".tmp")
            temporary.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            temporary.replace(chapters)
            subtitles.write_text(srt_text(self.markers, duration), encoding="utf-8")
        except OSError as error:
            warnings.append(f"Chapters could not be written: {error}")
        self._closed = True
        self.finished.emit(TalkResult(recording, chapters, subtitles, self.markers, duration, self.profile, warnings))

    def _duration(self, recording: Path) -> float:
        probe = command_path("ffprobe")
        if not probe:
            return 0.0
        try:
            result = subprocess.run(
                [probe, "-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1", str(recording)],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=4, check=False,
            )
            return max(0.0, float(result.stdout.decode("utf-8", "replace").strip() or 0))
        except (OSError, ValueError, subprocess.TimeoutExpired):
            return 0.0

    def _stop_process(self):
        if self.process is not None and self.process.poll() is None:
            try:
                self.process.terminate()
                self.process.wait(timeout=2)
            except (OSError, subprocess.TimeoutExpired):
                try: self.process.kill()
                except OSError: pass
        if self.log:
            try: self.log.close()
            except OSError: pass
            self.log = None

    def abort(self):
        if self._closed:
            return
        self._closed = True
        self.timer.stop()
        command = command_path("niruclip")
        if command:
            try:
                subprocess.run([command, "--stop"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=8, check=False)
            except (OSError, subprocess.TimeoutExpired):
                pass
        self._stop_process()
