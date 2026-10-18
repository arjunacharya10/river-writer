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
│ ⎇ main ↑2 ↓0 · ⚠ 1 · 23,410 words · 58 scenes · Fountain                         │
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
| Template | **Template dropdown in the editor title bar**, next to the preview icon (story tabs only): `[ Manuscript ▾ ] [preview] [⋯]`. Options: Manuscript · Screenplay · Screenplay (Multi-cam) · Stage Play. **Default by extension: `.md` → Manuscript, `.fountain` → Screenplay.** "New File" creates an `.md` (Manuscript) unless named `*.fountain`. Changing it re-paginates the preview instantly; the choice is remembered per file in `_river/river.json`. No template prompt when creating projects or files. |
| Preview toolbar | Page count + runtime estimate ("104 pages · ~104 min"), zoom, page navigation |
| Preview | **One preview icon in the editor title bar** (right end of the tab row, where VS Code shows "Open Preview to the Side"), visible only for story tabs. Click toggles the paginated page preview beside the script; scroll-synced. Keys: `⌘K V` (to the side), `⇧⌘V` (in place). The adjacent **⋯** menu holds Export PDF, Export FDX, Print. |
| Diff editor | Side-by-side and inline, word-level highlighting, scene badges |
| Minimap | Real minimap with scene headers as section markers |
| Breadcrumbs | File › Act/section › Scene |
| Panel (bottom) | **Problems** (script check) · **Timeline** · **Stats** (pages, runtime, words per character). No terminal or output panel. |
| Status bar | Branch + sync, problems, **words, scenes**, language mode. No page info (pages live in the preview). Template is chosen in the editor title bar. |
| Command palette / Quick Open | `⌘⇧P`, `⌘P`; `@` jumps to a scene, `#` to a character |
| Settings | Settings UI + `settings.json` (user and project scopes) |
| Keybindings | VS Code default keymap; `keybindings.json` |
| Zen mode | VS Code Zen mode + typewriter centering |
| Themes | **Two only: Light and Dark**, modelled on VS Code's default Light Modern / Dark Modern. Follows the system appearance by default (like `window.autoDetectColorScheme`); can be set explicitly. More themes can come later; version 1 ships these two only. The page preview stays white paper in both themes (it shows what prints); PDFs are always white. |
| Extensions | None |

## The editor is a writing surface, not a page
* **Pages are irrelevant to the editor.** No page breaks, headers, footers or page
  numbers. Page geometry is a preview/PDF concern only. Text soft-wraps inside a
  centered column with default side margins.
* **No line numbers.** Navigation is by scene: Outline,
  breadcrumbs, minimap, `⌘P` → `@scene`, "Go to Scene", "Go to Page" (via the preview
  mapping), and clicking a preview page jumps to the text.
* **No page information in the editor:** no page-break markers or page numbers
  (Highland's "show page numbers in editor" is deliberately not copied). Page count and
  runtime appear in the preview toolbar.
* **The editor still looks like what you're writing, without pages:**
  * **Screenplay / Multi-cam / Stage Play → script look** (structure, not pages):

    | Element | In the editor |
    |---|---|
    | Scene heading | Flush left, uppercase, bold |
    | Action | Flush left, full script width (~60 characters) |
    | Character cue | Indented ~2.0″ (≈20 ch), uppercase |
    | Parenthetical | Indented ~1.4″, width ~29 ch |
    | Dialogue | Indented ~1.0″, **wraps at ~35 ch** |
    | Transition | Right-aligned |
    | Centered `> <` | Centered |
    | Lyrics | Dialogue indent, italic |
    | Notes / boneyard / synopses | Muted / dimmed, flush left |

    Indents and widths come from the **same template data as the PDF** (Courier =
    10 ch/inch, so template inches convert to columns). Purely visual: **no spaces are
    inserted into the file.** Forcing characters (`@ . ! > ~`) are shown dimmed. Dual
    dialogue (`^`) shows sequentially with a side marker; side-by-side is preview-only.
    Bonus: screenplay lines wrap in the editor where they'll wrap on the page.
  * **Manuscript → plain writing surface:** one left edge, ~72-character column,
    Courier Prime, gentle highlighting (headings larger/bold, emphasis rendered inline with
    dimmed markers, notes muted, boneyard dimmed).
* **One font, no font settings (decision #35):** the editor and every template use
  Courier Prime 12 pt. The user only types.
* Problems and search results reference scene/paragraph, not line numbers.

## Writing behaviour inside the editor
* Fountain/Markdown language modes with highlighting and auto-formatting following
  `~/rea/HIGHLAND_SPEC.md` §4 (cues, scene headers, parentheticals, transitions).
* **Implemented (#43):** CodeMirror autocomplete for the current `.fountain` file’s
  character names and locations, plus standard scene prefixes/times. Keyboard or
  mouse acceptance; Tab/Enter accept, Escape dismisses, Control-Space requests suggestions.
* **Implemented (#43):** ordinary typing capitalizes recognized scene headings
  and explicitly forced `@` cue names at paragraph boundaries. Automatic character
  suggestions require capitals or `@`; lowercase action is not guessed to be a cue.
  Exact character/time matches allow Enter to continue writing. Paste and IME
  composition are preserved, as are notes/boneyard and Markdown prose. The editor
  extension follows the file syntax, independently of the chosen page template.
* Rename character (like Rename Symbol, F2) across cues, dialogue mentions, title page.
* Gutter: **no line numbers**; a slim strip for change bars since the last commit (click to
  peek or revert) and fold arrows for scenes/sections.
* Problems = script check: inconsistent cue spellings, unclosed `[[note`, empty
  scenes, unused characters. Never grammar.

## Implementation notes
* **CodeMirror 6** (MIT) for the editor and the diff/merge view (decision #27). Chosen
  over Monaco because the script look needs per-line indent/width/alignment and
  prose-friendly soft wrapping; CodeMirror does both natively via
  line decorations + CSS (the same approach Obsidian uses for Live Preview). VS Code feel
  comes from the workbench (built by us either way), a VS Code keymap and theming.
* Workbench chrome rebuilt in the Tauri webview to VS Code's layout, using **Codicons**.
* **Do not fork VS Code/Code-OSS**: it's Electron and brings the extension host,
  which conflicts with Tauri and the no-extensions principle.

## Workbench component foundation (decision #37)
* Kobalte provides accessible controls (menus, dialogs, selects, tooltips, tabs),
  wrapped as River components so styling and upgrades stay centralized.
* Corvu resizable provides keyboard-accessible, collapsible split panels.
* CodeMirror 6 owns the writing surface and its editing interactions.
* Use Tauri native macOS app menus and file/print dialogs where appropriate.
* Shared CSS tokens define River’s two themes, spacing, borders and component states.
  The foundation does not change the locked VS Code workbench layout.
* References: https://kobalte.dev/docs/core/overview/introduction/ and
  https://corvu.dev/docs/primitives/resizable/ . These are build dependencies, not
  network connections made by the released app.

## Implementation progress (2026-10-18)
The first production Mac workbench in `app/` implements Explorer/story tabs,
CodeMirror with Rust-driven script geometry, outline/diagnostics, the template picker,
page preview, PDF export, basic command palette, two themes and Corvu keyboard-resizable
splits. Native menus/dialogs are wired. This is the first shell/preview slice; the full
UI described above is still the target. See [current scope and verification](10-mac-workbench.md).

Pane sizing (#38): editor and preview have a **220 CSS px minimum** each; the sidebar
has a **170 px minimum**, matching VS Code's defaults. Visible panes do not collapse
when dragged below these limits. Explicit hiding returns the space to the remaining
pane; reopening restores the prior split. If minimums cannot fit, the split scrolls
instead of squeezing a pane away.

## Project welcome (decision #39)
Startup is a calm project welcome screen, inspired by Mac creative tools: River name,
New Project, Open Project and recent folders. The writing workbench appears after a
project is selected. The welcome screen follows the same Light/Dark tokens and uses
Kobalte controls; native Mac dialogs handle name/location and folder selection.
Closing a project returns here, preserving its saved tabs/layout for the next open.
