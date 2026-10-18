# 10 — First Mac workbench slice

Implemented 2026-10-18 in `app/`. This is the production shell baseline, separate
from the disposable editor trial. It is not a v1 release candidate.

## Run and build

Prerequisites: the existing Rust/Xcode toolchain, pnpm, and Node 26 (use
`PATH=/opt/homebrew/bin:$PATH` on this machine).

```sh
pnpm --dir app install
pnpm --dir app tauri dev
pnpm --dir app tauri build --debug --bundles app
```

The development bundle is `app/src-tauri/target/debug/bundle/macos/River Writer.app`.
Its identifier is `dev.riverwriter.desktop`; release identity, signing and
notarization remain release work. The engine stays in the root Cargo workspace;
the desktop package has its own workspace/lockfile.

## Implemented

* Folder Explorer for `.fountain`/`.md`, excluding `.git`, `_river`, symlinks and
  dependency folders. Explicit refresh after saves; folder watching is pending.
* Story tabs preserve each file's CodeMirror document, undo history, selection and
  scroll position. No line numbers or page markers in the editor. Bundled Courier
  Prime and the minimap are retained.
* Script geometry, semantic emphasis, outline, word/scene totals and diagnostics
  come from the Rust parser/template data. UTF-8 source offsets are converted to
  CodeMirror's UTF-16 positions, including non-ASCII text.
* Native Open, Save As, PDF export and discard dialogs; native application/File/
  Edit/View/Window menus. Save, find, palette, preview and sidebar shortcuts.
* Kobalte controls through `app/ui/src/components/controls.tsx`; Corvu resizable
  sidebar/editor/preview panels, including keyboard resizing. Shared Light/Dark
  CSS tokens follow system appearance until toggled during the session.
* Template dropdown and preview toggle in the editor title bar. A changed template
  is UI state in the project root's `_river/river.json`, keyed by relative file path;
  standalone files use their containing folder. Existing unrelated state is retained.
  Plain saves with the extension's default template do not create a sidecar.
* Revision-ordered document synchronization. Full-buffer updates are serialized per
  file and debounced by 120 ms; save/export flush pending edits first. Old analysis
  and preview replies cannot replace a newer revision.
* Rust pagination returns an immutable display list; visible/nearby pages request
  144-dpi PNGs through binary IPC. PDF uses the same engine/display-list format.
  Unsaved open include buffers take precedence over their disk copies.
* Safe text saves preserve UTF-8 BOM, original newline convention and Unix mode.
  Same-directory temporary writes protect existing files; external changes reject
  save rather than being silently replaced. Save As requires a new path. The active
  file is checked for external changes every two seconds and on window focus, with
  explicit Reload / Save As actions. Dirty close/quit can be cancelled.
* No runtime HTTP service, external fonts, updater, accounts or network requests.
  The local Vite server and browser IPC adapter exist only during development/tests.

`river-service` owns documents, persistence, analysis and engine orchestration without
Tauri. `app/src-tauri` owns OS integration and background execution. `app/ui` owns
presentation and CodeMirror view state. Font OFL text is included in the bundle;
the About panel identifies the font license. River's own code is MIT-licensed (#40); the MIT text is also included in the app bundle.

## Verification

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --manifest-path app/src-tauri/Cargo.toml -- -D warnings
pnpm --dir app build
cargo build -p river-service --example workbench_host
# Terminal 1:
pnpm --dir app dev
# Terminal 2 (uses installed Chrome):
pnpm --dir app verify
```

The browser check uses the real Rust service/parser/layout/rasterizer/PDF writer via
`workbench_host`, with deterministic test substitutes for OS dialogs. It checks open,
edit, save, tab undo, outline, source-preserving template changes, theme, palette,
find, keyboard resizing, cancelling dirty close, PDF and external-change protection.
It checks scrolling through preview pages and all four font faces, and rejects browser errors.
Screenshots go to ignored `app/output/`. The adapter is excluded from production
execution; a packaged app uses Tauri IPC.

Native WKWebView smoke checks separately verified a real Open dialog, screenplay
loading, page raster rendering, typing, Command-S (confirmed on disk), a native
File → New File action and native Quit. These are manual smoke checks, not a
complete automated Mac interaction suite.

## Remaining work

* This is a baseline for phases 2/3, not completion of those phases: incremental
  parsing/pagination, rope patches, generated IPC types, file-system watchers,
  project-wide completion, broader syntax highlighting and project-wide search remain.
  Current-script writing assistance is implemented below (#43).
* Preview currently renders whole pages, not tiles. Changed revisions rerender
  visible pages; bounded tile caching, editor/page sync scroll, accessible
  selectable page text and production performance budgets remain unverified.
* Includes are still constrained to the initial story file's directory, as in the
  CLI baseline. Project-root include resolution and unsaved new include files need
  a later resolver pass. Includes not open in tabs are read from disk.
* Explorer is a flat list of relative story paths, not the final expandable tree.
  Project tabs/layout/split sizes are restored via the project welcome flow below; window geometry is not persisted yet. The palette supports
  filtering, Enter and tab navigation; arrow-key result selection is pending.
* Save As does not replace an existing file. Mixed-newline files normalize to their
  detected convention. Save protection compares contents before replacement;
  a different app can still race a write between that check and the final rename.
* Settings UI, Git remotes/branches/restore, history compaction,
  FDX, print, Mac file associations, signed packaging and other v1 polish remain
  later phases. Crash recovery, automatic disk saves and local Timeline are implemented
  as described below.
* Development icon/identity only. No public push has been made; the MIT license and no-business-model decision are now settled (#40).

## Preview toggle fix (2026-10-18)
Repeated preview/sidebar unmounts exposed Corvu 0.2.5's remaining-panel size
redistribution issue: the editor could reach zero width while its text stayed intact.
River now keeps both panels registered, explicitly sets collapsed/expanded sizes and
remembers the visible split. CSS and splitter constraints enforce 220 px editor/preview
and 170 px sidebar floors (#38). Hidden panels are inert and excluded from focus.
Regression coverage checks real pane bounds and visible text across repeated toggles,
800/1280/1512 px windows, keyboard resizing to both limits, and sidebar toggles.

Native WKWebView also rechecked the reported open → preview → close → preview
sequence: source and rendered pages remain visible side by side. Informational
tooltips no longer intercept neighbouring toolbar buttons.

## Project welcome (2026-10-18)

The app now starts with New Project / Open Project / Recent Projects (#39).
`river-service::projects` owns safe plain-folder creation, recent locations and UI-only
session persistence; native Tauri commands own name/location/folder dialogs. New projects
contain one empty named `.md` file. Existing folders open their saved story tabs/active
file, cursors, scroll positions, sidebar/preview, splits and theme. `_river/river.json`
keeps UI state without replacing existing template/unknown keys. Story autosaving
was added separately below (#42). Unsaved text can cancel closing/switching projects.

Recent folders live in the local app configuration's `projects.json`. Only paths,
names and last-opened timestamps are recorded. Missing locations remain visible, and
cancelled/failed choices keep the current screen. Close Project returns to the welcome
screen. The old empty-editor startup is superseded; the workspace remains VS Code-style.

The verification adapter now exercises real project creation, cancelled creation,
existing-folder open, recents, tab/active-file/layout restoration and dirty-project
protection alongside the writing checks. Eight service tests cover document safety,
project creation and state/recents boundaries. Native WKWebView launch and cancellation
of the New Project dialog were checked. Screenshots are in ignored `app/output/`.


## Crash recovery (2026-10-18)

The first draft-protection slice journals dirty named and untitled documents in the
local app data directory's `recovery/`, outside the project and Git. Each acknowledged
editor sync writes atomically and flushes the journal; on macOS the directory entry
is flushed too. The existing editor sync runs after 120 ms idle, so edits still in
that debounce window or an in-flight write can be lost on an abrupt termination.
Journal failures surface as errors; writing remains available in memory.

After an interrupted session, startup offers each recovered draft. Recovery opens an
unsaved copy with its template, never overwrites the original story, and transfers
journal protection before deleting the previous entry. Saving, reloading from disk,
or explicitly discarding a tab clears that tab's journal. A cancelled close retains
it. Confirmed quitting now closes documents through the same cleanup path.

This journal protects unsaved text independently of the automatic disk saves and
Timeline added below. Separate backups were dropped in favour of a Git remote (#50). Next in the existing flow:
writing polish. Preview zoom/navigation controls are excluded by #44; Explorer
creation tools remain later work.

Verification: service restart tests cover Unicode, untitled and named drafts, external
edits, saved/discarded cleanup and invalid identifiers. `app/verify-recovery.mjs` kills
only a disposable service process, relaunches it, recovers both drafts through the UI,
continues writing, and verifies saving clears protection without touching a newer
external source. The full existing workbench regression suite also passes.


## Automatic saving and local Timeline (2026-10-18)

Named story files save after 1 second of idle typing, including inactive tabs.
Untitled drafts never trigger a save dialog automatically. Autosave uses the same
encoding-preserving, atomic, external-change-checked save path as Cmd+S. Stale
revisions are rejected. A history error blocks replacement and leaves the draft
available in memory/recovery. Successful saves clear that recovery journal. Closing
flows cancel pending timers; queued saves finish before documents are released.

`river-vcs` has no Tauri dependency and uses libgit2 with transport features disabled.
On first history access it adopts the exact project Git repository or initializes
one, preserving existing ignore rules while excluding `_river/river.json`. A bare
private repository at `.git/river-autosave/` uses a separate branch keyed to each
main branch (or detached HEAD). Its alternates read main objects; all new autosave
objects stay in the private repository. No author-identity setup is needed for these
private snapshots. Standalone files use app-data history keyed by source path.

Before each disk replacement, both the old bytes and changed new bytes are protected.
The editor title bar's history button opens a Timeline in the right pane shared with preview (decision #49) for the current
file, showing versions in batches of 200 with a text preview. Restore first checkpoints
the current buffer, then replaces editor text through an isolated undo transaction;
normal autosaving applies afterward. Changing tabs invalidates pending history reads.

Still pending: retention compaction/collection, real
commits in Timeline. No snapshots are deleted in this slice.
History cannot protect against loss of the drive holding the project/app data.

Verification: 13 service tests plus 5 VCS tests cover encoding, revisions, external
edits, history failure safety, previous-content restoration, per-file/per-branch
isolation, linked worktrees, paginated history, standalone storage, restart
persistence and a real local mirror push.
The expanded recovery UI test covers inactive-tab autosave, Timeline preview/restore,
Undo and conflict recovery; the full workbench regression suite passes as well.


## Screenplay typing assistance (2026-10-18)

`app/ui/src/writing.ts` adds CodeMirror’s official autocomplete extension. Per-document
Rust analysis supplies character names and scene locations; built-in prefixes/time
terms cover new scripts. No suggestions are fetched or generated over a network.
Characters from included files are not added in this first slice.

At a paragraph start in `.fountain`, `@ma` becomes `@MA` and offers matching known
characters; capitalized prefixes also offer names and scene-heading prefixes.
Recognized headings capitalize ordinary typed text. Location and time suggestions
replace only their respective heading part. Mouse, Tab or Enter accept; Escape
dismisses; Control-Space manually requests completion. Completed names and times
leave Enter free to create the next line. No element-cycle Tab mode was added.

Automatic assistance stays out of dialogue, ordinary lowercase action, notes and
boneyard. Comment boundaries follow the parser’s non-nesting/escaped-opener rules.
Pasted/multiline and IME composition transactions are not capitalized. Capitalization
is part of the original typing transaction, including Unicode length changes and
mid-line caret mapping, so Undo remains predictable. Markdown files stay prose even
with a screenplay page template; Save As updates the writing mode through analysis.
Template changes alone never rewrite existing source. Courier Prime remains fixed.

Verification: `pnpm --dir app test:writing` checks completion contexts, Unicode and
caret positions, exact matches, comment/paste/composition boundaries, syntax switching
and atomic Undo with real CodeMirror state. The expanded workbench browser test
checks a character suggestion, dialogue continuation, location/time acceptance via
Tab/Enter, typed capitals and Markdown isolation. The autosave/recovery/Timeline
suite also passes. A popup screenshot is in ignored `app/output/writing-completion.png`.
These composition checks use editor transactions; native Mac IME smoke coverage
remains part of the platform verification work. Preview zoom/navigation controls
are excluded by #44; Zen mode is deferred by #45. Broader syntax polish and
other v1 editor work remain.


## Simple preview (2026-10-18, #44)
The preview header now contains only page count/update status. Export, previous/next
chevrons, current-page navigation and the close button have been removed, together
with their unused callbacks/tracking. Pages remain scrollable, including by keyboard
when the preview scroll area is focused. The editor-title preview toggle/shortcut
opens and hides it; existing export actions outside the preview are unchanged.
Zoom controls are no longer planned.


## Simple Todos (2026-10-18, #46)

The checklist activity icon and `View: Todos` command open a Google Tasks-inspired
sidebar without creating an editor tab. Quick entry adds a task on Enter or blur;
click a title to rename it, tick to complete, and expand Completed to reopen/delete
a finished task. Delete and drag controls appear on hover/focus. Option+↑/↓ on a
reorder handle provides a keyboard alternative. Pending task text is flushed before
project changes, closing story tabs or quitting. The Todos view restores with the
project session; the input stays mounted when switching sidebar views.

`river-service::todos` reads/writes `_river/todos.md` as plain Markdown checkboxes.
Loading does not create the sidecar; writes are atomic and compare the exact loaded
snapshot with disk before replacing it. Existing Markdown, BOM and newline style
are preserved. Symlink sidecars/files are rejected. On conflict, the sidebar keeps
the typed input, offers Reload tasks and lets the writer retry against the newer
list. Task edits are persisted on interaction/flush, not in story crash journals.

Verification: three new Rust tests cover CRUD, reorder, reopening, Unicode, BOM/CRLF,
unknown Markdown, stale writes and symlink boundaries (16 service tests total). The
expanded browser suite checks mouse/keyboard completion, inline editing, drag and
Option-arrow reorder, deletion, pending entry on sidebar/project departure, view
restoration, external-conflict retry and no extra editor tabs. A screenshot is in
ignored `app/output/todos.png`. The draft-recovery/Timeline suite also passes.
Zen remains deferred. Scratchpad is implemented below; Bin was removed (#47).


## Scratchpad; Bin removed (2026-10-18, #47/#48)
The pencil activity icon and `View: Scratchpad` command open one plain-text sidebar
area in Courier Prime. No toolbar and no story tab. Todos remains available. There
is no Bin view/action/service or automatic creation of cut-material files. Existing
user files are not removed.

`river-service::extras` reads/writes `_river/scratchpad.md`. Loading does not create
the file. Changes save after one second idle; blur/sidebar/project/quit paths flush
pending text through a shared extras registry. Saves preserve BOM/newlines, use
atomic replacement and compare the loaded snapshot with disk; symlink sidecars and
files are rejected. Native Cmd+S while an extra has focus flushes that extra.

After 120 ms idle, unsaved notes are journaled privately via the existing recovery
service. Journals use separate note keys and are cleared only when the disk text
is saved/acknowledged. Recovery opens an unsaved copy; it never overwrites the
scratchpad. As with story recovery, an unacknowledged debounce/in-flight write can
be lost on abrupt termination.

On an external conflict, the sidebar preserves local text. Save a copy and reload
opens a native Save dialog for a new `.md` copy; the copy never replaces an existing
file. Cancel leaves the local text intact. Only after a successful copy does the
sidebar reload disk and acknowledge/clear the conflicted journal.

Verification: 20 service tests cover scratchpad encoding, conflicts, safe copies,
symlinks and journal restart recovery alongside existing document/Todo protection.
The expanded browser suite checks autosave, view switching, conflict recovery,
cancelled copy, pending notes on project close, session restoration and no extra
editor tabs/Bin button. Existing Todo, story, recovery and Timeline checks pass.
The inspected screenshot is in ignored `app/output/scratchpad.png`.

## Source Control, local slice (2026-10-18, #50/#51)

A source-control activity icon and `View: Source Control` open the Git view for the
open project. It offers Initialize Git, a project-local identity form when needed,
a commit message, **Commit all changes**, the change list and commit history. Review
changes and selecting a commit show the patch in the right pane, which it shares
with preview and Timeline (one at a time). Before committing, pending Scratchpad/
Todos text is flushed and named story tabs are autosaved.

`river-vcs::project` (re-exported as `river_service::git`) uses libgit2 only,
without transport features. Status builds a candidate tree in an in-memory index
(the real index is untouched); commit writes through a temporary index while
holding `index.lock`, compares HEAD and tree with the reviewed values, and installs
the new index only after the commit. `_river/river.json` is never committed. The
private autosave repository is never part of a commit. Separate automatic backups
are dropped from v1 (#50); off-machine safety will come from pushing to a remote.

Verification: three Rust tests cover add/edit/delete, extras, ignore rules, stale
reviews, a preserved staged index, an external `index.lock`, detached/merge blocking
and not adopting a parent repository; a mirror-push test checks a River commit
leaves private autosaves behind. `pnpm --dir app verify:git` exercises init,
identity, ignored files, commits, notes/tasks, side-pane diff, stale-review rejection
and history in the browser against the real service. Remotes, branches, restore and
scene-level diff remain.

## Git restore (2026-10-18, #53)

The commit diff pane lists the commit's files, each with **Restore**, and a **Restore
project…** header button. An inline confirmation precedes every restore. River
autosaves named tabs, refuses if a project tab still has unsaved changes, then calls
`river_vcs::project::restore` while holding the document lock. Open tabs for rewritten
files reload (closing tabs whose file the restore removed); Scratchpad/Todos remount
and reload. A status banner offers **Undo restore** until dismissed.

The backend validates every path (plain components, no symbolic-link ancestors, not
a folder, not `.git` or `_river/river.json`) before touching disk, checkpoints each
replaced story file in private history through `Documents::checkpoint_replaced`,
writes files atomically and rolls back already-applied paths on an I/O error. Undo
compares each restored path with the post-restore tree and refuses rather than
overwrite later edits.

Verification: two new VCS tests cover deleted-file recovery, project restore and undo,
checkpoint callbacks, refusing undo over newer writing, unchanged history, foreign
commits, failed checkpoints and symbolic links. `pnpm --dir app verify:git` now
restores a file and the whole project through the UI with an open tab, undoes both,
checks ignored files survive and that undo refuses after an external edit. Screenshot:
ignored `app/output/git-restore.png`. Branches and reverting commits remain.

## Git branches (2026-10-18, #54/#55)

`river_vcs::project::{branches, create_branch, switch_branch}` add the branch slice.
`switch_branch` refuses during merges/conflicts, compares the working tree with HEAD
(a separate repository handle, since status installs an in-memory index), checkpoints
replaced story files, runs a safe checkout and sets HEAD. Same-commit switches only
move HEAD. The UI's `switchBranch` reuses the restore path: autosave, refuse unsaved
tabs, call the backend, reload rewritten tabs and extras. The detached-HEAD message
now points to switching to a branch in River.

Verification: a new VCS test covers listing (including an unborn branch), invalid and
duplicate names, carrying uncommitted work to a new branch, refusing a diverged switch
with uncommitted work, file rewrite/removal and checkpoint callbacks. `verify:git`
creates a branch through the UI with uncommitted text, commits on it, switches back
with an open tab, checks the status bar and refuses a dirty switch. Screenshot:
ignored `app/output/git-branches.png`. FDX moved to v1.1 (#54).

## Mac release polish (2026-10-18, #56/#57)

Identity is now `app.riverwriter.mac` with the River icon (#58) and macOS 13 minimum. The
generated Info.plist was inspected: `.fountain` is matched by extension only at Default
rank; Markdown uses `net.daringfireball.markdown` at Alternate rank. An early config with
a `text/plain` MIME type produced `public.plain-text` and was corrected before commit.

macOS `RunEvent::Opened` (Finder double-click, Dock drop, Open With) queues paths in the
backend; the UI drains the queue on start-up and on a `river:open-files` event, opening
each file as a story tab one at a time. `scripts/release-mac.sh` covers signing,
notarization, stapling and Gatekeeper checks (doc 08).

Verified: bundle builds, Info.plist identity/category/minimum OS/document types, icon set,
script syntax, and all browser suites. Not yet verified: a real Developer ID signed and
notarized build (needs the writer's notary profile and the x86_64 Rust target), and the
Finder open flow in the native app (a manual check: double-click a `.fountain` file with
River closed and with it open).

## Performance pass (2026-10-18, #59)

**Editor analysis.** `river_service::analyze` now returns `EditorElement`s: kind, `from`/`to`
in UTF-16 units, style `marks`, and text only for cues and scene headings (needed for
completions). Outline items and problems carry a UTF-16 `at`. Rust converts offsets with
one prefix table per analysis; the UI no longer builds a byte table or ships the parser's
per-character source mappings. Measured on the 120-page benchmark script (release host):
reply 15.5 MB → 50 KB, sync round trip 351 ms → 17 ms. Pagination (≈38 ms) is unchanged.

**Git polling.** `git::status` is now cheap: libgit2 status with the index stat cache, no
hashing, no object writes, returning only changed paths. `git::review_changes` builds the
snapshot tree and patch with a mempack backend, so review writes nothing to `.git`; the
UI calls it for **Review changes** and, if no review is pending, at **Commit**. Commit
still re-derives the tree on disk and refuses if it differs. A new test confirms five
rounds of edits, polling and review leave `.git/objects` unchanged.

## Pruning pass (2026-10-18, #60)

Removed: `spikes/` (last present in `5e44904`), the dead `open_folder` command, the
search activity icon (find-in-file stays on ⌘F, Edit → Find and the palette) and the
status bar's UTF-8 label. Status messages fade after 5 s; errors persist until dismissed.
`ignore_ui_state` is the single writer of the `/_river/river.json` ignore rule, matching
existing rules with CRLF or trailing spaces so it is never appended twice. Source Control
takes a `refreshKey` bumped after River rewrites files, so counts update without waiting
for the 3 s poll. `app/test-harness.mjs` holds the shared service/page/`until` setup for
`verify`, `verify:git`, `verify:recovery` and the Scratchpad checks.

Still open from the review: split `main.tsx` and `style.css` by area, one shared command
table for Tauri and the test host, caching the private-history repository per file, and
history retention.

## Structure after the refactor (2026-10-18, #61)

**Commands.** `crates/river-service/src/commands.rs` is the single command table:
`Service::call(name, args)` covers documents, preview/PDF, recovery, history, projects,
extras and Git, with the same locking as before. A `Shell` trait supplies dialogs and the
config folder: `MacShell` in `app/src-tauri/src/main.rs` (native dialogs) and `TestShell`
in `workbench_host.rs` (fixed paths). Tauri registers `river`, `river_bytes`,
`take_opened_files` and `quit_confirmed`; `ui/src/bridge.ts` routes names accordingly.
The Tauri shell went from 840 to 285 lines and no longer depends on the engine crates.

**UI.** `main.tsx` keeps workspace state (tabs, editor, sync, preview, autosave, project
session) and the split layout. Logic with its own state is in `ui/src/features/`:
`timeline.ts`, `git-workflow.ts` (diff pane, restore/undo, branch switch),
`extras-flush.ts`, `commands.ts` (palette list and ⌘ shortcuts). Presentational pieces
are in `ui/src/components/`: activity bar, sidebar views, editor title/tabs, banners,
Git diff pane, Timeline pane, status bar/problems.

**Styles.** `ui/src/styles/` holds `base`, `workbench`, `editor`, `preview`, `history`,
`project-welcome`, `extras` and `git` CSS, imported in that order from `main.tsx`.

Verified by all browser suites plus a pixel comparison of their screenshots against a
pre-refactor baseline. Not covered automatically: Tauri IPC itself; open a project,
preview and export once in the Mac app to confirm.

## Git on by default (2026-10-18, #62)

`river_vcs::project::ensure_repository` runs when a project is opened or created (inside
the document lock) and from `History::open`. There is no Initialize button: if a project
reaches Source Control without a repository, the panel runs setup itself (Refresh retries).
New repositories get `/_river/river.json` in `.git/info/exclude` and a starting snapshot
written through the real index, so status is clean. The old `.gitignore` writer is gone.
A repository with no commits and nothing staged (as older River builds left behind)
also gets the snapshot; anything the writer staged is never committed for them.
Source Control always shows the new-branch field. Verified by a VCS test (clean status,
snapshot contents, untouched `.gitignore`, single exclude rule, immediate branching,
adopting an existing repository) and by `verify:git`, which now starts from a folder
without Git and checks the snapshot, the untouched `.gitignore` and that only the
writer's edit is listed.

Nested projects: a folder that is itself a Git repository inside the project (another
River project, say) is skipped when snapshotting, reviewing or committing, and is not
listed as a change. Previously libgit2 failed on it ("invalid path"), which broke autosave
once River retried the snapshot for commit-less repositories.

## Commit identity (2026-10-18, #63)

`author()` in `river_vcs::project` uses Git's identity, else the Mac account (`/usr/bin/id
-F` for the full name, `-un` for `<user>@localhost`). Status reports that identity, so the
Source Control name/email form only appears when neither exists. Covered by a VCS test
(fallback, no account, configured identity wins) and `verify:git`, which commits with no
Git identity and checks the author is the account's full name and that no form appears.
