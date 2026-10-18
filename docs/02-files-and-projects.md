# 02 — Files and projects

## Story files
* **`.fountain`** for Screenplay and Stage Play.
* **`.md`** for Novel/Manuscript: a small, defined Markdown subset plus notes `[[ ]]`,
  synopses `=` and boneyard `/* */`.
* The story file stays **pure**. Nothing app-specific is written into it, so it opens
  correctly in any Fountain or Markdown tool.
* **Template is per file**, chosen from the dropdown in the editor title bar (doc 03).
  Default by extension: **`.md` → Manuscript, `.fountain` → Screenplay**; "New File"
  creates `.md` unless named `*.fountain`. A changed choice is stored in
  `_river/river.json` (`"templates": {"Act 1.fountain": "stageplay"}`), never in the story file.
* No `.river` content format and no ZIP bundle (unlike Highland's `.highland` ZIP of a
  TextBundle, which hides the text from other tools and makes sync all-or-nothing).

## Project = a folder
```
The Long Night/
├── .git/                          ← real git repository (doc 04)
├── .gitignore                     ← includes _river/river.json and anything marked "ignore"
├── The Long Night.fountain        ← the story (or several files, e.g. one per chapter)
└── _river/
    ├── scratchpad.md              ← free-form dump
    ├── todos.md                   ← Markdown checkboxes: - [ ] / - [x]
    ├── bin/                       ← cut material, one dated file per snippet
    │   └── 2026-10-17 old opening.fountain
    └── river.json                 ← UI state only (cursor, panes, theme, template)
```

* `_river/` is **visible** (not hidden) and sorts last: honest, easy to back up by hand.
* **Opening a lone file** (`script.fountain`) works without a project folder. River
  creates the sidecar (`script.river/` or a `_river/` next to it) **only when you first
  use the scratchpad, todos or bin**.
* **Opening a folder** makes it a project. Several story files appear in the navigator,
  and the extras are shared across the project.
* Losing `river.json` costs only window state, never words.

## Extras: deliberately dumb
Each extra has its own **activity-bar icon and opens in the left side bar**, never as an
editor tab (tabs are for story files only). On disk they're still plain text in `_river/`,
which the Explorer hides.

| Extra | File | Side-bar behaviour |
|---|---|---|
| Scratchpad | `_river/scratchpad.md` | One plain text area, autosaves, no toolbar |
| Todos | `_river/todos.md` | Markdown checkboxes; type to add, click to tick, drag to reorder. No dates, priorities or reminders. |
| Bin | `_river/bin/*` | "Send to Bin" on a selection makes a dated file; list newest first, expand inline, drag back into the editor. Shared across the project. |

* Extras are **shared across branches by default** (option: per-branch).
* Extras are committed and pushed like any other file. Any file can be marked
  **Ignore** (added to `.gitignore`), e.g. a private scratchpad.

## `river.json` (sketch, UI state only)
```json
{
  "version": 1,
  "templates": {"The Long Night.fountain": "screenplay"},  // per file; missing = extension default (.md manuscript, .fountain screenplay)
                                     // manuscript | screenplay | screenplay-multicam | stageplay
  "openFiles": ["The Long Night.fountain"],
  "cursor": {"The Long Night.fountain": 10422},
  "panes": {"preview": true, "extras": "scratchpad"},
  "theme": "light"
}
```
