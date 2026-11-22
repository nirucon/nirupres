# NIRUPRES guide

Reference for the features added in NIRUPRES 2.1 and 2.2. The
[README](../README.md) has the overview; [CHANGELOG.md](../CHANGELOG.md) has
the full history.

## NIRUX editor chrome

The editor, dialogs, menus and focus treatment can follow the active NIRUX
appearance. This never changes slide themes, audience output or PDF/PPTX exports:
a deck must look the same on every machine.

- **Follow NIRUX** is enabled by default when NIRUX is detected.
- The active theme comes from `_NIRUWM_THEME` on the root window (it changes
  live) or `~/.config/niruwm/config.toml`, user themes from
  `~/.config/niruwm/themes`; optional `~/.config/niruwm/appearance.toml`
  overrides are applied.
- Toggle **NIRUX** in the toolbar or use the command palette. The choice is
  persisted. Without NIRUX, the original monochrome Noir chrome is used.
- `nirupres --check-config` reports the effective source and data root.

## Presenting online and in the NIRUX session

See "Presenting in Teams, Meet or Zoom" in the README for the user steps.
Technical details:

- Window identity: the audience window has WM_CLASS
  (`nirupres-audience`, `NIRUPRES`) and title `AUDIENCE · share this · NIRUPRES`;
  the presenter view (`nirupres-presenter`, `NIRUPRES`) and title
  `PRIVATE presenter · do not share`; the editor keeps (`nirupres`, `NIRUPRES`).
  These are set on X11 only (Wayland compositors choose their own picker names).
- The audience window carries `_NIRUWM_KEEP_MAPPED` = 1 (CARDINAL). NIRUWM then
  moves it off screen instead of unmapping it when its workspace is hidden, so a
  window share keeps receiving frames.
- Presenting mode: `niructl present on --pid <pid>` at start and
  `niructl present off` at the end. Without `present` (older NIRUWM) the
  fallback is `niructl dnd on` and, at the end, `niructl dnd off` only if Do Not
  Disturb was off before. The fallback state is kept in
  `~/.local/share/nirupres/presenting.json` and repaired on the next start
  after a crash. `NIRUPRES_NIRUCTL=/path/to/niructl` overrides the binary and
  `NIRUPRES_NIRUCTL=none` disables it (the tests do this).
- Text fitting: every slide text is laid out at a pixel size the way Qt's
  QTextLayout did it in 2.x (HarfBuzz shaping, the same metrics and line
  breaking), shrunk to its layout minimum, then cut with an ellipsis inside its
  box. `nirupres --self-test-fit [DIR]` renders every layout with short,
  normal, long and very long text and fails on any overflow (DIR gets the PNGs).
- Video: local files and web links play in the audience window through mpv
  (embedded with `--wid`); without mpv the poster frame is shown.

## Talk mode

The **TALK** toolbar menu starts a presentation from the beginning and records
it with NIRUCLIP:

- Screen + microphone, screen + camera, or camera + microphone.
- NIRUCLIP owns capture and state; NIRUPRES never starts a second recorder.
- Each visible slide change becomes a chapter marker.
- At the end, NIRUPRES stops NIRUCLIP and writes `.chapters.json` and `.srt`
  beside the recording. The recording itself is never deleted.
- Errors from NIRUCLIP, a busy camera or a missing command are shown instead of
  claiming a successful recording.

Talk mode uses the NIRUCLIP state at
`${XDG_STATE_HOME:-~/.local/state}/niruclip/session.json` and the NIRUCLIP
profiles `screen-mic`, `screen-camera` and `camera-mic`. Recordings are written
to `~/Videos/NIRUPRES-DECK-TIMESTAMP.mkv`; set `NIRUPRES_TALK_DIR` to choose a
different recording directory.

## User data and desktop integration

Autosave, recent files and preferences live under the XDG data root, normally
`~/.local/share/nirupres` (`autosave.json`, `recent.json`, `display.json`,
`file-workflow.json`, `profile.json`, `ui.json`). Your `.nirupres` decks live
wherever you save them. The installer also registers the
`application/x-nirupres` MIME type, a desktop entry and an icon, and checks
the SHA-256 of the binary it downloads.

## Organization profiles

A profile sets defaults and wording for **new** decks only: whether the logo
master switch starts on, the starter themes in **New**, the title preset name and
placeholder texts. Every theme, including the Uddevalla family, is available in
every profile, and each deck keeps its own `showLogo` value, so switching profile
never changes an existing presentation.

- **Neutral** (default for fresh installations): no logo by default, neutral
  starters and placeholders.
- **Uddevalla**: the official logo on by default, Uddevalla starters, the
  "Uddevalla title" preset.

Choose it in **New → Profile** or the command palette (`Ctrl+K` → "Profile").
The choice is stored in `~/.local/share/nirupres/profile.json`. An installation
that already has NIRUPRES settings, or that `install.sh` upgrades from 2.1 or
earlier, keeps the Uddevalla profile automatically.

## Saving, autosave and recovery

- **Save** (`Ctrl+S`) writes the whole deck, with its images, into one
  `.nirupres` archive. The previous file is replaced only after the new one has
  been verified.
- **Autosave** is a crash safety net, not a second save: 2.5 seconds after the
  last edit of an *unsaved* deck, NIRUPRES writes one recovery file,
  `~/.local/share/nirupres/autosave.json`. It never touches your `.nirupres`
  file. Saving, opening another deck or closing NIRUPRES normally removes it.
- The status bar shows the state on the right: `● Unsaved changes`,
  `● Autosaved hh:mm` or `Saved hh:mm`.
- If NIRUPRES did not close normally, the next start shows when the work was
  autosaved and offers **Restore** or **Discard**. Closing that dialog keeps the
  autosave for the next start. A restored deck is marked
  `● Recovered from autosave` and stays unsaved until you save it.

The editor writes no files while idle; autosave runs only after an edit.
