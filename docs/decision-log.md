# Decision log

All decisions were made 2026-10-17 during ideation. Newest decisions on a topic
supersede older ones.

| # | Topic | Decision | Notes / alternatives |
|---|---|---|---|
| 1 | Product | Lean, offline take on Highland Pro for **creative writers only** | Strip feature bloat to a laser-sharp product |
| 2 | Name | **River Writer** | |
| 3 | Positioning | **Lead with craft**; privacy/offline as proof | Alternative: lead with privacy |
| 4 | Formats | Screenplay (+ multi-cam), Stage Play, Novel/Manuscript | **Comics/graphic novels cut**; non-creative templates cut |
| 5 | Stack | **Tauri** (Rust core + webview UI) | |
| 6 | Story files | Pure `.fountain` and `.md` | Rejected: custom `.river` content file, ZIP bundles |
| 7 | Extras | Scratchpad, todos, bin kept **as plain-text dumps only** | Stored as files in the project's `_river/` folder |
| 8 | Extras location | Visible `_river/` folder in the project; sidecar created only when first used | Alternative: hidden `.river/` |
| 9 | Editor | "VS Code for writers"; no extensions | Superseded by #17 |
| 10 | History | Automatic local history on by default | User agreed |
| 11 | Extras vs branches | Shared across branches by default, per-branch optional | User agreed |
| 12 | Versioning engine | **Real git** | Considered: a simple copy-based model ("versions/alternates/take"). User clarified their concern was vocabulary, not machinery. |
| 13 | Remote | **Projects can push to git** | Briefly considered dropping push for "zero network"; user kept push. Network only on push/pull/fetch/clone. |
| 14 | Vocabulary | **Standard git terms** (commit, branch, merge, diff, tag, push, pull…) | Considered: bilingual labels, writer-renamed terms (Version/Alternate/Take/"The River"), river-themed and desk-and-paper metaphors. User chose git terms: "simpler, it will be a learning curve." |
| 15 | Git UX | No staging area; stash/rebase/reset/detached HEAD hidden; autosave never pushed; scene-level diff/merge | Learning softened with hints, tooltips, "Git for Writers" guide |
| 16 | Revision marks | Derived from the diff since a tag | Replaces Highland's confusing in-document revision objects |
| 17 | UI | **Exactly like VS Code**: activity bar, side bar (Explorer/Search/Source Control/Outline/Characters), editor groups and tabs, preview to the side, diff editor, minimap, breadcrumbs, bottom panel (Problems/Timeline/Stats), status bar, command palette, settings.json, VS Code keymap, Zen mode | Extras become plain files in Explorer (no custom panel); Timeline view = autosave + commits; minimap replaces the scene strip. Monaco + Codicons; don't fork Code-OSS (Electron). No VS Code name/logo. |
| 18 | Extras UI | Scratchpad, Todos, Bin each get an **activity-bar icon and open in the left side bar**; **editor tabs are only for story files** (`.fountain`/`.md`) | Supersedes the #17 note that extras open as files/tabs. Files stay in `_river/` on disk; the Explorer hides `_river/`. |
| 19 | Themes | **Two only: Light and Dark** (modelled on VS Code Light/Dark Modern); follow system appearance by default | Version 1 ships these two; **more themes can come later** (user). VS Code theme import deferred, not ruled out. Page preview stays white paper; PDFs always white. |
