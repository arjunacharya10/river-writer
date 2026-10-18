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
It checks page navigation and all four font faces, and rejects browser errors.
Screenshots go to ignored `app/output/`. The adapter is excluded from production
execution; a packaged app uses Tauri IPC.

Native WKWebView smoke checks separately verified a real Open dialog, screenplay
loading, page raster rendering, typing, Command-S (confirmed on disk), a native
File → New File action and native Quit. These are manual smoke checks, not a
complete automated Mac interaction suite.

## Remaining work

* This is a baseline for phases 2/3, not completion of those phases: incremental
  parsing/pagination, rope patches, generated IPC types, file-system watchers,
  completion, broader syntax highlighting and project-wide search remain.
* Preview currently renders whole pages, not tiles. Changed revisions rerender
  visible pages; bounded tile caching, zoom, editor/page sync scroll, accessible
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
* Extras, settings UI, Git/history/autosave/backups, FDX, print, Mac file associations,
  full font-license presentation, signed packaging and other v1 polish remain later
  phases. Until local history/backups land, unsaved text is in memory only.
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
keeps this UI state without replacing existing template/unknown keys. No story text is
autosaved. Unsaved text can cancel closing/switching projects.

Recent folders live in the local app configuration's `projects.json`. Only paths,
names and last-opened timestamps are recorded. Missing locations remain visible, and
cancelled/failed choices keep the current screen. Close Project returns to the welcome
screen. The old empty-editor startup is superseded; the workspace remains VS Code-style.

The verification adapter now exercises real project creation, cancelled creation,
existing-folder open, recents, tab/active-file/layout restoration and dirty-project
protection alongside the writing checks. Eight service tests cover document safety,
project creation and state/recents boundaries. Native WKWebView launch and cancellation
of the New Project dialog were checked. Screenshots are in ignored `app/output/`.
