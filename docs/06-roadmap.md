# 06 — Roadmap

## Version 1 — "write, see the page, keep every version"
Ships on **macOS, Windows and Linux** simultaneously (principle 7: multiplatform, slow features).
* VS Code workbench: activity bar, Explorer, Search, Source Control, Outline, editor groups/tabs,
  command palette, quick open, breadcrumbs, minimap, status bar, settings.json, VS Code keymap, Zen mode
* Live paginated preview; PDF export; title page
* Templates: Screenplay (+ multi-cam), Stage Play, Novel/Manuscript
* FDX import/export
* Extras: Scratchpad, Todos, Bin as activity-bar side-bar views (plain files in `_river/`)
* Git: commit (no staging), log, diff (scene/paragraph/word), restore/revert, branch +
  checkout, remote + push/pull/fetch, clone
* Local autosave history (never pushed); automatic local backups
* Themes: Light and Dark only, following the system appearance

## Version 1.1
* Scene-level merge + conflict resolution; cherry-pick scene/paragraph
* Tags → revision marks (colored pages + asterisks since a tag); page diff
* Problems panel (script check); rename character (F2); Characters view; Stats panel
* Reader-copy export (line numbers, margin notes)
* Optional "Sign in with GitHub"
* Optional daily word-goal ring

## Later (after 1.1)
* More themes (curated additions, or VS Code theme import with Fountain styling)

## Never
Cloud sync, accounts, AI, subscriptions, analytics, extensions/plugins, comments/review
services, non-creative templates.

## Open decisions
1. One unified syntax for screen and prose, or Fountain + Markdown kept separate?
   (Current lean: separate.)
2. Open Highland `.highland` files as a migration path? (ZIP → TextBundle → `text.fountain`/`text.md`)
3. Pricing: one-time purchase (assumed), price point TBD.
4. Exact autosave interval and retention.
5. Spellcheck: OS webview spellcheck (differs on Linux) vs bundled Hunspell-based checking for identical behaviour (doc 07).
