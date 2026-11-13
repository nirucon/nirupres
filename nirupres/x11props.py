"""Small X11 helpers for presentation windows (2.3).

Qt gives every window of an application the same WM_CLASS. NIRUPRES sets
per-window classes so the window manager, NIRUBAR and screen-share pickers
can tell the audience window from the private presenter view, and marks the
audience window with ``_NIRUWM_KEEP_MAPPED`` so NIRUWM keeps it mapped (and
therefore live in a screen share) when its workspace is hidden.

Everything is best effort: on Wayland, offscreen or without libX11 the
functions do nothing and return None.
"""

import ctypes
import ctypes.util

from PySide6.QtGui import QGuiApplication

_X = None
_LOADED = False
_ERR_T = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
_HANDLER = None


def _lib():
    global _X, _LOADED
    if _LOADED:
        return _X
    _LOADED = True
    try:
        app = QGuiApplication.instance()
        if app is None or app.platformName() != "xcb":
            return None
        name = ctypes.util.find_library("X11") or "libX11.so.6"
        x = ctypes.cdll.LoadLibrary(name)
        x.XOpenDisplay.restype = ctypes.c_void_p
        x.XOpenDisplay.argtypes = [ctypes.c_char_p]
        x.XCloseDisplay.argtypes = [ctypes.c_void_p]
        x.XInternAtom.restype = ctypes.c_ulong
        x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
        x.XChangeProperty.argtypes = [
            ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong,
            ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int,
        ]
        x.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
        x.XGetWindowAttributes.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_void_p]
        x.XGetWindowAttributes.restype = ctypes.c_int
        # A window that vanished must not abort the process through Xlib's
        # default error handler (it calls exit()). Errors are ignored here.
        global _HANDLER
        _HANDLER = _ERR_T(lambda d, e: 0)
        x.XSetErrorHandler.argtypes = [_ERR_T]
        x.XSetErrorHandler.restype = ctypes.c_void_p
        x.XSetErrorHandler(_HANDLER)
        _X = x
    except Exception:
        _X = None
    return _X


class _Attrs(ctypes.Structure):
    _fields_ = [
        ("x", ctypes.c_int), ("y", ctypes.c_int), ("width", ctypes.c_int),
        ("height", ctypes.c_int), ("border_width", ctypes.c_int), ("depth", ctypes.c_int),
        ("visual", ctypes.c_void_p), ("root", ctypes.c_ulong), ("c_class", ctypes.c_int),
        ("bit_gravity", ctypes.c_int), ("win_gravity", ctypes.c_int),
        ("backing_store", ctypes.c_int), ("backing_planes", ctypes.c_ulong),
        ("backing_pixel", ctypes.c_ulong), ("save_under", ctypes.c_int),
        ("colormap", ctypes.c_ulong), ("map_installed", ctypes.c_int),
        ("map_state", ctypes.c_int), ("all_event_masks", ctypes.c_long),
        ("your_event_mask", ctypes.c_long), ("do_not_propagate_mask", ctypes.c_long),
        ("override_redirect", ctypes.c_int), ("screen", ctypes.c_void_p),
    ]


def _with_display(fn):
    x = _lib()
    if x is None:
        return None
    d = x.XOpenDisplay(None)
    if not d:
        return None
    try:
        return fn(x, d)
    finally:
        x.XCloseDisplay(d)


def set_window_identity(widget, instance, cls, keep_mapped=False, opaque=False):
    """Set WM_CLASS (instance, class) and optionally _NIRUWM_KEEP_MAPPED=1.

    ``opaque`` sets _NET_WM_WINDOW_OPACITY to fully opaque, so a compositor
    with an inactive-window opacity (picom ``inactive-opacity``) never makes
    the slides or the presenter view see-through when they lose focus.
    """
    wid = int(widget.winId())

    def run(x, d):
        data = instance.encode() + b"\0" + cls.encode() + b"\0"
        buf = ctypes.create_string_buffer(data, len(data))
        string = x.XInternAtom(d, b"STRING", 0)
        x.XChangeProperty(d, wid, x.XInternAtom(d, b"WM_CLASS", 0), string, 8, 0, buf, len(data))
        if keep_mapped:
            card = x.XInternAtom(d, b"CARDINAL", 0)
            one = (ctypes.c_long * 1)(1)
            x.XChangeProperty(
                d, wid, x.XInternAtom(d, b"_NIRUWM_KEEP_MAPPED", 0), card, 32, 0, one, 1
            )
        if opaque:
            card = x.XInternAtom(d, b"CARDINAL", 0)
            full = (ctypes.c_ulong * 1)(0xFFFFFFFF)
            x.XChangeProperty(
                d, wid, x.XInternAtom(d, b"_NET_WM_WINDOW_OPACITY", 0), card, 32, 0, full, 1
            )
        x.XSync(d, 0)
        return True

    return _with_display(run)


def is_viewable(widget):
    """True when the window is mapped and viewable, False when not, None if unknown."""
    try:
        wid = int(widget.winId())
    except Exception:
        return None

    def run(x, d):
        attrs = _Attrs()
        if not x.XGetWindowAttributes(d, wid, ctypes.byref(attrs)):
            return None
        return attrs.map_state == 2  # IsViewable

    return _with_display(run)
