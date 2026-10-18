# 06 — Roadmap

## Version 1 — "write, see the page, keep every version"
Ships on **macOS only** (principle 7, decision #36). Windows is deferred and does not
gate development or release. Linux: later or never.
* VS Code workbench: activity bar, Explorer, Search, Source Control, Outline, editor groups/tabs,
  command palette, quick open, breadcrumbs, minimap, status bar, settings.json, VS Code keymap (Zen mode deferred, #45)
* Writing assistance: current-script character/location suggestions, scene prefixes
  and times, scoped capitalization implemented (#43); broader editor polish remains.
* Live paginated preview; PDF export; title page
* Templates: Screenplay (+ multi-cam), Stage Play, Novel/Manuscript
* FDX import/export
* Extras: Scratchpad and Todos as activity-bar sidebar views (plain files in `_river/`).
  Bin removed (#47).
  * Todos implemented (#46): quick entry/edit, completion/reopening, collapsed completed
    tasks, delete and drag/keyboard reorder.
  * Scratchpad implemented (#48): one writing area, autosave, recovery, conflict-copy
    handling. No extra editor tabs or toolbar.
* Git: commit (no staging), log, diff (scene/paragraph/word), restore/revert, branch +
  checkout, remote + push/pull/fetch, clone
  * Implemented (#51): Source Control view with init, project identity, review all
    changes, commit everything, commit history and diff. Remaining: branches/checkout,
    restore/revert, scene-level diff. Remotes/push/pull/fetch/clone parked (#52);
    when built they run in-process, with no installed `git` required.
* Local autosave history (never pushed). Separate automatic backups dropped (#50);
  off-machine safety comes from the writer's Git remote
  * Implemented: crash recovery, one-second idle disk saves, private per-branch Git
    history, Timeline preview/restore with Undo (doc 10). Remaining: retention
    compaction, real commits in Timeline.
* Themes: Light and Dark only, following the system appearance

## Version 1.1
* Scene-level merge + conflict resolution; cherry-pick scene/paragraph
* Tags → revision marks (colored pages + asterisks since a tag); page diff
* Problems panel (script check); rename character (F2); Characters view; Stats panel
* Reader-copy export (line numbers, margin notes)
* Optional "Sign in with GitHub"
* Optional daily word-goal ring

## Later (after 1.1)
* Windows port: revisit after the Mac app is excellent; no committed release date.
* More themes (curated additions, or VS Code theme import with Fountain styling)

## Never
Cloud sync, accounts, AI, subscriptions, analytics, extensions/plugins, comments/review
services, non-creative templates.

## Open decisions
1. One unified syntax for screen and prose, or Fountain + Markdown kept separate?
   (Current lean: separate.)
2. Open Highland `.highland` files as a migration path? (ZIP → TextBundle → `text.fountain`/`text.md`)
3. History retention/compaction implementation. The disk-save interval is settled
   at 1 second idle; checkpoints protect each disk replacement (#42).

## Closed decisions: license and business model (#40)
* **License: MIT.** The repository's `LICENSE` contains the grant.
* **Business model: none; no monetization planned.** Pricing is not an open product decision.
* Earlier one-time-purchase and paid-build options are superseded.
* Bundled fonts and third-party components retain their own licenses/notices.
