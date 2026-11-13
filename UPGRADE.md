# Upgrading to NIRUPRES 2.3.0

2.3.0 keeps project format 9 and Markdown format 2; decks open unchanged.

- **Text sizes are now pixels.** Slides look the same as before on a normal
  96 dpi screen and now also the same on HiDPI screens and in exports. Text
  that used to run out of its box now shrinks or ends with an ellipsis; check
  Deck health (`Ctrl+K`) for slides marked **FIT**.
- New layouts CODE, IMAGE + CAPTION and THREE COLUMN are appended to the
  layout list; older versions cannot show slides that use them correctly.
- Window titles changed: share `AUDIENCE · share this · NIRUPRES`. Window
  rules that matched the old audience window title should match the
  WM_CLASS `nirupres-audience` instead.
- F5 on one screen now asks for laptop presenter or slides only.

# Upgrading to NIRUPRES 2.2.x

2.2.1 changes only the installer and adds `uninstall.sh`.

2.2.0 keeps project format 9 and Markdown format 2; no conversion is needed and
decks saved by 2.2 still open in 2.1.x. Run `./install.sh` from the release (or
`nirux extras update`). The installer keeps a rollback snapshot of the
previous code until the new release has passed its checks, and preserves
`~/.local/share/nirupres` user data.

- **Profile:** upgrading from 2.1 or earlier writes
  `~/.local/share/nirupres/profile.json` with the Uddevalla profile, so new
  decks keep the logo default you had. Switch with New → Profile or the command
  palette. Fresh installations start with the neutral profile.
- **Inspector layout** is remembered in `~/.local/share/nirupres/ui.json`;
  deleting that file restores the defaults.

# Upgrading from NIRUPRES 2.0.0 / 1.7.3

NIRUPRES 2.1.0 keeps project format 9 and Markdown format 2. Existing projects,
autosave data, recents and display preferences are preserved. If you set a
non-default `XDG_DATA_HOME`, install NIRUPRES 2.1.0 with the same value so code,
settings and MIME data remain in the intended roots. The previous application
generation is kept by the transactional installer for rollback.

1. Back up important `.nirupres` presentations and local preferences.
2. Extract the 2.0.0 release ZIP.
3. Run `./install.sh` from the extracted directory.
4. Confirm the installer release gate passes and run `nirupres`.
5. Verify existing presentations, speaker notes, PDF/PPTX export and
   dual-screen presentation before relying on the upgrade for live use.

Project format 9 and Markdown format 2 are unchanged. Do not delete
`~/.local/share/nirupres` during the upgrade.
