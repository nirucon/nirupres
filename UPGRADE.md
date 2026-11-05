# Upgrading to NIRUPRES 2.2.0

2.2.0 keeps project format 9 and Markdown format 2; no conversion is needed and
decks saved by 2.2 still open in 2.1.x. Run `./install.sh` from the release (or
`nirux extras update`). The installer keeps the previous generation for
rollback and preserves `~/.local/share/nirupres` user data.

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
