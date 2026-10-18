# 07 — Prior art: lessons we reuse instead of re-solving

River's philosophy is unchanged (focused, opinionated, no plugins, no cloud). This doc
collects **technical** solutions others already worked out. Obsidian is closed source:
lessons come from its public API docs, community plugins and observable behaviour,
never from inspecting its code.

## From Obsidian (public knowledge)

| Problem | Obsidian's approach | River adopts |
|---|---|---|
| Live styling in CodeMirror 6 without lag | Live Preview decorates only the visible viewport, incrementally | Script-look line decorations for the viewport only, from cached element types |
| Features coupled to the editor library | Plugins use an `Editor` abstraction, not CodeMirror directly | Thin `RiverEditor` interface (cursor, selection, replace, scroll-to-range) |
| Indexing a big folder | Background metadata cache on disk, refreshed by modification time/size | Rust project index (scenes, characters, locations), cached in the app data dir by file hash/mtime; `_river/` stays clean |
| External edits (other editors, git, Dropbox/iCloud) | Debounced watcher, ignores own writes, silent reload when clean | Same; fingerprint own writes; compare prompt when dirty |
| Sync-tool conflict copies | Plain files left to the user | Detect common `(conflicted copy)` patterns → "Compare with original" (diff view) |
| Renames breaking references | Rewrites links on rename | Rewrite `{{INCLUDE: path}}` on rename/move |
| Per-project layout/settings | `.obsidian/workspace.json` + per-vault config | `_river/river.json` + `_river/settings.json` |
| Never losing work | "File recovery" snapshots with retention | Validates autosave in `.git/river-autosave/` (today / hourly / daily / weekly) |
| Theming | CSS variables throughout | Light/Dark as CSS variable sets from VS Code's palettes |
| Commands and hotkeys | Central registry, hotkeys by command ID, conflict detection | VS Code-style registry + conflict warnings in the keybindings editor |
| Undo across tab switches | Keeps editor state per open file | Cache CodeMirror `EditorState` per tab |
| Startup with large projects | UI first, index in background | Editor usable immediately; outline/characters fill in |
| Spellcheck | Chromium's spellchecker via Electron (identical everywhere) | **Gap:** Tauri uses OS webviews (Linux WebKitGTK differs). Open question: accept OS spellcheck, or bundle Hunspell-based checking in Rust (shown as editor decorations) for identical behaviour |
| PDF export | Chromium print-to-PDF | N/A: River's own typesetting engine |
| Mobile | One codebase via Capacitor | Later: Tauri 2 mobile targets + CodeMirror touch support; keep UI free of desktop-only assumptions |

## Open-source code worth studying

| Project | Useful for | License (verify before reuse) |
|---|---|---|
| VS Code | Workbench layout, commands/keybindings/`when` clauses, quick input, settings editor | MIT (reusable with attribution) |
| CodeMirror 6 examples & community | Decorations, merge view, minimap | MIT |
| SilverBullet | CodeMirror 6 Markdown live preview, indexing | MIT |
| Zettlr | CodeMirror 6 writing app, exports, file watching | GPL-3: ideas only |
| afterwriting | Fountain → PDF pagination (cross-check with the Highland spec) | MIT (verify) |
| Beat | Open-source screenwriting app: Fountain parsing, pagination | GPL: ideas only |
| Fountain parsers (e.g. fountain-js) | Parser edge cases, test inputs | Mostly MIT (verify each) |

**License rule:** GPL code is for reading ideas only (copying would force River to be
GPL). MIT/BSD/Apache code may be reused with attribution, recorded in a `THIRD_PARTY`
notices file.
