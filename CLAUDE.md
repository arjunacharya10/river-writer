# River Writer — guide for Claude

River Writer is an offline, open-source writing app for screenwriters, playwrights and
novelists. You write plain `.fountain` / `.md` text, and River produces perfectly
formatted pages. It's a focused, anti-bloat take on Highland Pro: craft first, no cloud,
no accounts, no AI, no plugins.

## Read first
1. `docs/decision-log.md` — every decision (37 so far with reasoning and reversals.
   **Locked decisions are not to be relitigated** unless the user raises them.
2. `docs/01-product-brief.md` (principles), then `02`–`09` as relevant.
3. Formatting-engine behaviour (parser rules, template values, pagination,
   (MORE)/(CONT'D)): `~/rea/HIGHLAND_SPEC.md` (research workspace: `~/rea/README.md`).

## Locked essentials
* **Stack:** Tauri 2 · Rust engine crates (no Tauri dependency) · TypeScript + SolidJS
  workbench · CodeMirror 6 editor · Parley (text layout) · krilla (PDF) · git via git2.
* **UI components (#37):** Kobalte behind River wrappers; Corvu resizable panels;
  shared CSS tokens; native Mac menus/dialogs via Tauri. CodeMirror owns editing.
* **Platforms:** macOS only now and for v1 (#36). Windows deferred; no parity or
  simultaneous-release gate. Prioritize Mac quality. Linux later or never.
* **Files:** pure `.fountain`/`.md`; project = folder; extras (scratchpad, todos, bin) are
  plain text in a visible `_river/`; `_river/river.json` holds UI state only.
* **UI:** exactly like VS Code. Editor tabs only for story files. Extras live in
  activity-bar side-bar views. Template dropdown + preview icon in the editor title bar.
  Template default by extension (`.md` → Manuscript, `.fountain` → Screenplay). Two
  themes (Light/Dark). Minimap kept.
* **Editor vs page:** no page info and no line numbers in the editor. Screenplay-family
  files get a "script look" (indents/widths from template data, visual only).
  Pages exist only in the preview/PDF, which come from one Rust display list.
* **Git:** standard git vocabulary, no staging, autosave in `.git/river-autosave/`
  (never pushed), scene-level diff/merge.
* **Network:** none except explicit git push/pull/fetch/clone. No auto-update or update
  checks (but keep the door open; doc 08).

## Current status and next step
Product definition complete (37 decisions). **The first engine CLI baseline is implemented**
in the root Rust workspace (`crates/`). Real `.fountain`/`.md` files export to PDF/PNG
with all four templates, inline styles, source maps, includes, title pages and dialogue
continuations. See `docs/09-engine-cli.md` for commands, verification and limitations.

Next: production Mac workbench integration; incremental parsing/pagination and broader
formatting fixtures remain engine follow-up. Kobalte/Corvu are approved for that shell
(#37). The editor in `spikes/` is still a disposable experiment. Windows is deferred (#36).
Open (non-blocking) decisions: license and business model (before the first public push),
unified syntax vs separate, `.highland` import, autosave timing (`docs/06-roadmap.md`).

## Working with the user
* Show the plan (paths, options, what changes) and **wait for approval** before system
  changes or big edits; present 2–3 lettered options with a recommendation.
* When asked to "ideate", don't write code; keep answers short when asked.
* **Keep docs current:** record every new decision in `docs/decision-log.md` (mark
  superseded ones) and update the affected doc; commit doc changes.
* Reimplement Highland's *behaviour* only. Never copy Highland code, fonts, sample scripts
  or assets. GPL projects (Beat, Zettlr) are for ideas only unless the license changes.
* Toolchain here: Rust 1.96, Xcode 27.1, pnpm 9. The default `node` is nvm 22.12; use
  Homebrew Node 26 (`/opt/homebrew/bin`) if tooling needs newer Node.
