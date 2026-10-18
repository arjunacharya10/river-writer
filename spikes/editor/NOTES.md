# Editor trial — results and implementation notes

A disposable SolidJS + CodeMirror 6 + Tauri 2 experiment, not the production app.
The local macOS app is built and runs with bundled frontend assets (no web server).

## What works

* Screenplay line decorations: scene headings, cues, parentheticals, dialogue,
  transitions, forced markers and muted notes/boneyard. Manuscript is a plain column
  with heading emphasis. No line numbers or page information in the editor.
* One bundled Courier Prime family at 12 pt; Light/Dark appearance.
* Shared geometry in `../template.json`, also read by the Rust typesetting trial.
* Minimap via `@replit/codemirror-minimap`.
* Common editing shortcuts: undo/redo, find/replace, select all, navigation, indent.
  Full VS Code keymap parity is not implemented; Quick Open/palette are placeholders.
* Template changes preserve the source text. Sample-loading buttons deliberately
  replace the disposable trial document. There is no file saving in this experiment.

## Checks (2026-10-17, Apple M5 / macOS)

Long fixture: **4,920 source lines / 116,291 characters**, 120 scene sections.
It approximates a long script; its exact printed page count is not yet known.
Each benchmark inserts 60 characters, one per animation frame, then makes 30 scroll
jumps across the document. Typing timings include transaction dispatch and forced
DOM layout, not physical keystroke-to-photon latency. The minimap stays enabled.

| Check | Native Tauri / WebKit | Headless Chrome |
|---|---:|---:|
| Typing median | 4 ms | 3.7 ms |
| Typing p95 | 5 ms (5–6 ms observed over runs) | 4.3 ms |
| Frame interval p95 | 17 ms | 16.7 ms |
| Scroll jump + next frame p95 | 18 ms | 17 ms |

Undo/redo, inserted character count, unchanged source during styling, dialogue
indent, action width and dialogue width pass. Browser interaction checks also
exercise actual keyboard input, undo, search, and source-preserving template
changes; there are no console or failed-resource errors. All four Courier Prime faces
are explicitly loaded before browser checks; the dev server allows access only
to the trial folder, shared geometry file and bundled fonts. Native keyboard typing and Command-Z were
also checked through macOS accessibility automation and restored the original text.

Native measurements require the app to remain visible/foreground; WebKit throttles
animation frames in background windows. Checks are started with **Run checks**,
not automatically on launch. Reports are written to ignored `output/` files.
The 16.7 ms typing p95 threshold is an experimental budget, not a product promise.

## Reproduce

Use Homebrew Node 26 on this machine (`PATH=/opt/homebrew/bin:$PATH`). From this folder:

```sh
pnpm install --frozen-lockfile
pnpm build
pnpm tauri dev
```

For a macOS application containing the frontend (no dev server required):

```sh
pnpm tauri build --debug --bundles app
```

The app is under `src-tauri/target/debug/bundle/macos/`. Click **Run checks** while
the app is visible; the report is `output/native-results.json`.

Browser automation uses an installed Chrome and the local Vite server:

```sh
pnpm dev
# In another terminal:
node verify.mjs
```

Screenshots and `browser-results.json` go to `output/`.
The icon is an original temporary SVG, not a proposed product identity.

## Still needed

Windows/WebView2 testing; actual production parser classifications; IME and
accessibility testing; selection/caret edge cases across wrapped indented lines;
continuous scroll profiling and a real 120-page document. The experiment rebuilds
line decorations and word counts on edits; incremental work belongs in the real app.
No file handling, real Explorer/git, preview integration, or production command
palette exists here. The sidebar/chrome is a visual scaffold with placeholder icons.
