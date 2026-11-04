# Agent assignment: NIRUPRES 2.2.0, a one-shot repository, UX and maintainability update

## 0. Quick start

1. Read this file, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `UPGRADE.md`,
   `NIRUPRES-MARKDOWN-SPEC.md`, `NIRUPRES-AI-GUIDE.md`, `nirupres/*.py` and
   `tests/release_gate.py`. The audit baseline is `ba02c7d` (2.1.4, pinned as a NIRUX
   extra). Revalidate each item against current HEAD and skip items that are already
   fixed, noting that you did.
2. Implement **R1–R5, then U1–U5, then M1 last**. Commit per item. Keep
   `QT_QPA_PLATFORM=offscreen PYTHONPATH=. python3 tests/release_gate.py` passing (it
   passes at baseline when run with the installer's venv: `~/.local/share/nirupres/.venv`).
3. Pass the gates in §4, release in §5 and report in §6.

This is an **execution** assignment. Do not stop after a plan or a branch. Make sensible
decisions, document them, and continue without routine questions.

## 1. Invariants

- **Project format 9 and Markdown format 2 stay readable and writable.** Every existing
  presentation must open unchanged. Bump a format only if unavoidable, with a migration
  and a backup copy.
- Exports stay as they are: PDF, and PowerPoint as rasterized slides. Presenter and
  audience windows, transitions and the online (screen-share) audience window keep
  their behavior.
- Keep the zip safety in the project archive handling (`_safe_zip_name`, validation).
- The requirements in `requirements.txt` keep their ranges. Add no new runtime dependency.
- The owner's local checkout `~/Git/nirupres` has an uncommitted change to `install.sh`
  (a file mode change). Never touch the owner's checkout. Work in your own clone.

## 2. Findings (verified 2026-11-04 on niruxwork, in a separate Xvfb with a temporary HOME)

### R1: There is no CI on push or pull request
Only `release.yml` exists, so regressions are found at release time. **Fix:** add
`ci.yml` that runs on push and pull request: the Qt runtime packages (same as
`release.yml`), `pip install -r requirements.txt`, `compileall` and the offscreen
release gate.

### R2: The tests do not run on a plain Debian system
On Debian 13 with system PySide6, `tests/release_gate.py` fails at import: Debian's
package has no `PySide6.QtTest`, and `python-pptx` is not installed. **Fix:** document
in `CONTRIBUTING.md` that tests use the installer venv, and add
`scripts/test.sh` that creates or reuses a venv from `requirements.txt` and runs the
gate. Where a `QTest` helper is only a convenience (key clicks), consider
`QApplication.sendEvent` so the gate also runs on distro PySide6.

### R3: A stale branch
`chore/mit-license-2026` is not merged. `main` already has `LICENSE`, so check that it is
MIT. If it is, delete the branch. Otherwise merge only the license change.

### R4: Organization-specific defaults in a public app
New decks default `showLogo=True` ("Show official Uddevalla logo"). The footer placeholder
says "e.g. Socialtjänsten", and the preset list starts with "Uddevalla title". For anyone
else the app looks like an internal tool.
**Fix:**
- Move the organization identity (logo, profile colors, title preset, placeholder text)
  into an opt-in **profile**, with the Uddevalla profile kept and selectable.
- The default profile is neutral.
- Existing decks that already use the logo keep it, because it is stored per deck.
- The owner's installation is migrated so that their default profile stays Uddevalla
  (detect an existing settings file), so the owner sees no change.

### R5: Labels are clipped at normal widths
At 1600 px wide, the left panel's **OUTLINE** tab label was clipped ("OUTLINI"), and
several toolbar or inspector labels sit at their limit. **Fix:** set minimum widths
from font metrics, elide with a tooltip only as a last resort, and test at 1366x768 and
1920x1080 with the theme fonts.

## 3. UX

### U1: Inspector density
The inspector shows content, image, background, video, style and presentation together.
Collapse all sections except **Content** by default, remember the open/closed state,
and show a one-line summary on collapsed sections ("Image: fill · center").

### U2: Toolbar grouping
Group the top toolbar into File (New/Open/Save/Export/Import), Slide (+Slide,
Duplicate, Delete, Undo/Redo) and Present (Present, from current, Talk, Online). Keep
every action and shortcut, add visible separators, and add an overflow `⋯` menu at narrow
widths.

### U3: Presenter idle cost
While presenting, measure CPU in presenter view and audience view with and without
`BG MOTION` and transitions. The presenter view has a 100 ms precise timer and the
clock has a 1 s timer. With no motion, the target is ≤ 1 % CPU. Pause motion timers
when the audience window is covered by the "black/white/curtain" modes.

### U4: Autosave transparency
Show "Saved hh:mm" or "Autosaved hh:mm" in the status area, and "Recovered from
autosave" with Restore/Discard after a crash, as long as that matches the existing
autosave model. Document the model in the README.

### U5: Accessibility of the slide list
Add keyboard reordering of slides (Alt+Up/Down), and announce the slide number and title
as accessible names.

### M1: Readable source (last, behavior-neutral)
`main.py` has 44 lines over 300 characters, the longest about 3 300. `foundation.py`,
`editor.py` and `markdown_io.py` have 6–7 each, with whole widget trees on one line.
Reformat in mechanical commits (`ruff format`/`black` as a dev-only tool), then split the
`main.py` shell into toolbar, inspector and slide-list modules where it is natural. The
release gate passes after every commit.

Out of scope: native editable PPTX export, collaboration and cloud, and new transition
types.

## 4. Gates

Automated:
- R1 CI green.
- The release gate.
- New tests: the R4 profile defaults and the owner migration (fixture settings), the R5
  minimum widths (assert that no label is elided at 1366 px with the default font), U1
  collapse persistence, and U5 reorder.
- Round-trip every bundled example and fixture project through open → save → reopen with
  byte-equivalent JSON for unchanged decks.

Native, on **niruxwork** (the owner allows local testing):
- Use a temporary HOME. Open a copy of one of the owner's real decks only if the owner
  provides one; otherwise use fixtures.
- Present to a second virtual screen (Xvfb) or the real external screen if connected.
- Measure U3 CPU.
- Export PDF and PPTX and open them.
- Do not change the owner's settings or decks.

## 5. Release

Bump to **2.2.0** (`nirupres/__init__.py`, the release gate's expected version,
`CHANGELOG.md`, `UPGRADE.md`). Tag `v2.2.0`, let `release.yml` publish, and verify the ZIP
and checksum. Update the NIRUX extras pin in `nirucon/nirux`: `docs/EXTRAS.md` and the
version/checksum line in `nirux`. Install through the NIRUX extras update, verify
`nirupres --version`, and open a fixture deck.

## 6. Final report

For each item R1–R5, U1–U5 and M1: the change, the tests and the evidence (screenshots
before and after at 1366 and 1920 px, presenter CPU, round-trip results). Include release
URLs, pinned versions and remaining limits. Mark anything unfinished as **PARTIAL** and
give the exact remaining steps.
