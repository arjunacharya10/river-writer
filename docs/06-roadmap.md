# 06 — Roadmap

## Version 1 — "write, see the page, keep every version"
Ships on **macOS only** (principle 7, decision #36). Windows is deferred and does not
gate development or release. Linux: later or never.

**Scope is frozen (#64): v1 is what is built today, plus release polish and fixes.**
* VS Code-style workbench: activity bar, Explorer (flat list of story files), Source
  Control, Outline (click to jump), story tabs, command palette, minimap, status bar,
  native Mac menus and dialogs, project welcome with recent projects
* Writing assistance: character/location suggestions, scene prefixes and times, scoped
  capitalization (#43)
* Live paginated preview; PDF export; title page (US Letter)
* Templates: Screenplay (+ multi-cam), Stage Play, Novel/Manuscript
* Extras: Scratchpad (#48) and Todos (#46) as sidebar views (plain files in `_river/`)
* Git, built in (no installed `git`): on by default with a starting snapshot (#62),
  review and commit everything (#51), history and commit diffs, file/project restore
  with Undo (#53), branch create/switch (#55), Mac-account identity (#63)
* Draft protection: one-second autosave and crash recovery (#42). Versions are Git
  commits; no private local history or Timeline (#66). No off-machine copy in v1;
  writers use their own Mac backups (#50, #64)
* Mac release: `app.riverwriter.mac` identity, icon and file associations (#56, #58).
  Distributed as source: writers clone and build it themselves (#65)
* Themes: Light and Dark only, following the system appearance

### Left before releasing v1
* Native checks in the Mac app after the refactor (#61): open, edit/save, preview, PDF,
  Git review/commit/restore/branch, Finder double-click
* Build-from-source steps checked on a clean clone
* Doc and bug fixes found along the way

## Version 1.1
Moved from v1 (#64):
* Git remotes: add remote, push/pull/fetch, clone (in-process, #52); revert a commit;
  branch delete; tags and milestones; first-run Git hints and Help → Git for Writers
* Writer-shaped diff (scene/paragraph/word) and gutter change bars
* Settings UI + `settings.json`/`keybindings.json`
* Quick Open (⌘P, `@` scenes, `#` characters) and breadcrumbs
* Project-wide search/replace
* Explorer tree: folders, new folder, rename/delete, folder watching (New File done, #67)
* Native Print; A4 paper / Page Setup
* Preview sync scroll and click-to-jump; Outline drag-to-reorder scenes
* Split editor groups, pinned tabs; includes resolved from the project root
* CI release workflow, `CHANGELOG.md`, network-promise audit; Help → Check for Updates /
  Subscribe links

Already planned for 1.1:
* Final Draft (FDX) import and export (#54)
* Branch merge with scene-level merge + conflict resolution; cherry-pick scene/paragraph
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

## Closed decisions: license and business model (#40)
* **License: MIT.** The repository's `LICENSE` contains the grant.
* **Business model: none; no monetization planned.** Pricing is not an open product decision.
* Earlier one-time-purchase and paid-build options are superseded.
* Bundled fonts and third-party components retain their own licenses/notices.
