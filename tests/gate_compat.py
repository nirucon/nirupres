"""Test helpers shared by the NIRUPRES release gate.

* ``isolate_user_data()`` points every XDG root at a throw-away directory, so a
  gate run (including the one ``install.sh`` performs on the user's machine)
  never reads or deletes real autosave data, recents or preferences.
* ``QTest`` is PySide6's QtTest when available. Distribution packages such as
  Debian's ``python3-pyside6`` do not ship QtTest, so an equivalent shim built
  on ``QApplication.sendEvent`` is used there. The gate only needs key clicks,
  mouse clicks and event-loop waits.
"""

import atexit
import os
import shutil
import tempfile
import time

_ISOLATED_ROOT = None


def isolate_user_data():
    """Redirect XDG data/config/state to a private temp root (idempotent)."""
    global _ISOLATED_ROOT
    if _ISOLATED_ROOT is not None:
        return _ISOLATED_ROOT
    root = tempfile.mkdtemp(prefix="nirupres-gate-")
    for var in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_STATE_HOME"):
        path = os.path.join(root, var.lower())
        os.makedirs(path, exist_ok=True)
        os.environ[var] = path
    # Never let a gate run switch Do Not Disturb or presenting mode in the
    # real NIRUWM session; tests that need niructl point this at a fake.
    os.environ["NIRUPRES_NIRUCTL"] = "none"
    atexit.register(shutil.rmtree, root, True)
    _ISOLATED_ROOT = root
    return root


try:  # pragma: no cover - depends on the PySide6 build
    from PySide6.QtTest import QTest  # noqa: F401
except ImportError:  # Debian/Void python3-pyside6 ship without QtTest
    from PySide6.QtCore import QElapsedTimer, QEvent, QPointF, Qt
    from PySide6.QtGui import QKeyEvent, QMouseEvent
    from PySide6.QtWidgets import QApplication

    class QTest:  # minimal, event-equivalent subset used by the gate
        @staticmethod
        def qWait(ms):
            clock = QElapsedTimer()
            clock.start()
            app = QApplication.instance()
            while clock.elapsed() < ms:
                app.processEvents()
                app.sendPostedEvents()
                if clock.elapsed() < ms:
                    # Short sleeps keep timers firing without busy-spinning a core.
                    time.sleep(0.002)

        @staticmethod
        def mouseClick(widget, button=Qt.LeftButton, modifiers=Qt.NoModifier, pos=None):
            local = QPointF(pos) if pos is not None else QPointF(widget.rect().center())
            glob = QPointF(widget.mapToGlobal(local.toPoint()))
            for kind, buttons in (
                (QEvent.Type.MouseButtonPress, button),
                (QEvent.Type.MouseButtonRelease, Qt.NoButton),
            ):
                event = QMouseEvent(kind, local, glob, button, buttons, modifiers)
                QApplication.sendEvent(widget, event)
            QApplication.processEvents()

        @staticmethod
        def keyClick(widget, key, modifiers=Qt.NoModifier):
            key = int(key)
            text = chr(key).lower() if 0x20 <= key <= 0x7E else ""
            if key == int(Qt.Key_Space):
                text = " "
            for kind in (QEvent.Type.KeyPress, QEvent.Type.KeyRelease):
                event = QKeyEvent(kind, key, modifiers, text)
                QApplication.sendEvent(widget, event)
            QApplication.processEvents()
