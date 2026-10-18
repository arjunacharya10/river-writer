# 03 — Editor and UI: exactly like VS Code

**Decision:** River's UI is a faithful replica of the VS Code workbench: same layout,
same interaction patterns, same default keybindings. Writer-specific behaviour lives
*inside* that structure. **No extensions, ever.** (VS Code's name and logo are Microsoft
trademarks and are not used.)

## Layout
```
┌─ Title bar ─────────── [ Command Center: Search files, scenes, commands ] ───────┐
├──┬──────────────┬────────────────────────────────────────┬──────────────────────┤
│📁│ SIDE BAR     │ EDITOR GROUP 1                          │ EDITOR GROUP 2       │
│🔍│ SCRATCHPAD   │ ┌ The Long Night.fountain × ┐ Act2.md  │ ┌ Preview: The Long… │
│⎇ │ ┌──────────┐ │ Story › Act 1 › INT. LIGHTHOUSE (crumbs)│ │  [ page 12 ]       │
│☰ │ │what if   │ │  INT. LIGHTHOUSE - NIGHT          ▒▒▒  │ │                    │
│👤│ │Mara never│ │  MARA climbs the stairs…  (minimap)▒▒  │ │                    │
│──│ │left?     │ │  MARA                             ░░  │ │                    │
│✎ │ │…         │ │  (whispering)                     ░░  │ │                    │
│☑ │ └──────────┘ ├────────────────────────────────────────┴──────────────────────┤
│🗑 │              │ PANEL:  PROBLEMS │ TIMELINE │ STATS                           │
│⚙ │              │  ⚠ Cue "JON" also written as "JOHN" (12 places)               │
├──┴──────────────┴───────────────────────────────────────────────────────────────┤
│ ⎇ main ↑2 ↓0 · ⚠ 1 · p.12/104 · 23,410 words · ~104 min · Screenplay · Fountain │
└──────────────────────────────────────────────────────────────────────────────────┘
```

Activity bar (top to bottom): Explorer 📁 · Search 🔍 · Source Control ⎇ · Outline ☰ ·
Characters 👤 │ **Scratchpad ✎ · Todos ☑ · Bin 🗑** │ Settings ⚙.

**Rule: editor tabs are only for story files** (`.fountain` / `.md` the writer creates).
The scratchpad, todos and bin each have an activity-bar icon and open **in the left side
bar**. They never open as tabs.

## Mapping
| VS Code area | River |
|---|---|
| Activity bar | Explorer · Search · Source Control · Outline (scenes) · Characters · **Scratchpad · Todos · Bin** · Settings |
| Explorer | Story files only (`.fountain`, `.md`, folders of chapters). `_river/` is hidden (like `files.exclude`) |
| **Scratchpad view** | Side bar: one plain-text editing area, autosaved to `_river/scratchpad.md`. No toolbar. |
| **Todos view** | Side bar: checklist backed by `_river/todos.md` (`- [ ]` / `- [x]`). Type to add, click to tick, drag to reorder. Nothing else. |
| **Bin view** | Side bar: list of cut snippets (`_river/bin/*`), newest first. Select text → "Send to Bin" (also right-click / command). Click a snippet to expand it inline in the side bar; drag it into the editor to reuse; delete. |
| Outline | Scenes, sections, synopses, notes; drag to reorder scenes |
| Timeline view | Local autosave history + git commits for the current file |
| Search | Project find/replace with regex and include/exclude; extra toggles: dialogue only, character, notes |
| Source Control | VS Code SCM view: commit box, changes list (all files, **no staging section**), branches, ⋯ menu for push/pull/fetch/merge/tag (doc 04) |
| Editor groups & tabs | **Story files only** (+ the page preview and diff views of story files). Splits, preview tabs, pinned tabs, drag between groups |
| Template | **Template dropdown in the editor title bar**, next to the preview icon (story tabs only): `[ Manuscript ▾ ] [preview] [⋯]`. Options: Manuscript · Screenplay · Screenplay (Multi-cam) · Stage Play. **Default: Manuscript** for every file. Changing it re-paginates the preview instantly; the choice is remembered per file in `_river/river.json`. No template prompt when creating projects or files. |
| Preview | **One preview icon in the editor title bar** (right end of the tab row, where VS Code shows "Open Preview to the Side"), visible only for story tabs. Click toggles the paginated page preview beside the script; scroll-synced. Keys: `⌘K V` (to the side), `⇧⌘V` (in place). The adjacent **⋯** menu holds Export PDF, Export FDX, Print. |
| Diff editor | Side-by-side and inline, word-level highlighting, scene badges |
| Minimap | Real minimap with scene headers as section markers |
| Breadcrumbs | File › Act/section › Scene |
| Panel (bottom) | **Problems** (script check) · **Timeline** · **Stats** (pages, runtime, words per character). No terminal or output panel. |
| Status bar | Branch + sync, problems, page X/Y, words, runtime (~1 page = 1 min), language mode (template is chosen in the editor title bar, not here) |
| Command palette / Quick Open | `⌘⇧P`, `⌘P`; `@` jumps to a scene, `#` to a character |
| Settings | Settings UI + `settings.json` (user and project scopes) |
| Keybindings | VS Code default keymap; `keybindings.json` |
| Zen mode | VS Code Zen mode + typewriter centering |
| Themes | **Two only: Light and Dark**, modelled on VS Code's default Light Modern / Dark Modern. Follows the system appearance by default (like `window.autoDetectColorScheme`); can be set explicitly. More themes can come later; version 1 ships these two only. The page preview stays white paper in both themes (it shows what prints); PDFs are always white. |
| Extensions | None |

## Writing behaviour inside the editor
* Fountain/Markdown language modes with highlighting and auto-formatting following
  `~/rea/HIGHLAND_SPEC.md` §4 (cues, scene headers, parentheticals, transitions).
* IntelliSense-style autocomplete: character names, scene-header parts (INT./EXT.,
  locations, times of day).
* Autocapitalize scene headers and cues.
* Rename character (like Rename Symbol, F2) across cues, dialogue mentions, title page.
* Gutter change bars since the last commit (click to peek or revert).
* Problems = script check: inconsistent cue spellings, unclosed `[[note`, empty
  scenes, unused characters. Never grammar.

## Implementation notes
* **Monaco** (MIT) for the editor and diff editor gives the identical editing feel.
* Workbench chrome rebuilt in the Tauri webview to VS Code's layout, using **Codicons**.
* **Do not fork VS Code/Code-OSS**: it's Electron and brings the extension host,
  which conflicts with Tauri and the no-extensions principle.
