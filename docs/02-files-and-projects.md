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
├── .gitignore                     ← the writer's own; River never edits it (#62)
├── The Long Night.fountain        ← the story (or several files, e.g. one per chapter)
└── _river/
    ├── scratchpad.md              ← free-form dump
    ├── todos.md                   ← Markdown checkboxes: - [ ] / - [x]
    └── river.json                 ← UI state only (cursor, panes, theme, template)
```

* `_river/` is **visible** (not hidden) and sorts last: honest, easy to back up by hand.
* **Opening a lone file** (`script.fountain`) works without a project folder. River
  creates the sidecar (`script.river/` or a `_river/` next to it) **only when you first
  use the scratchpad or todos**.
* **Opening a folder** makes it a project. Several story files appear in the navigator,
  and the extras are shared across the project.
* Losing `river.json` costs only window state, never words.

## Extras: deliberately dumb
Each extra has its own **activity-bar icon and opens in the left side bar**, never as an
editor tab (tabs are for story files only). On disk they're still plain text in `_river/`,
which the Explorer hides.

| Extra | File | Side-bar behaviour |
|---|---|---|
| Scratchpad | `_river/scratchpad.md` | Implemented (#48): one plain text area, autosaves, no toolbar or story tab. |
| Todos | `_river/todos.md` | Simple Google Tasks-inspired sidebar (#46): type to add, click a title to edit, tick to complete, collapse completed tasks, reopen/delete, drag or Option+↑/↓ to reorder. No dates, priorities, reminders or subtasks. |

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

## Opening River (decision #39)

River launches into a project welcome screen, with **New Project…**, **Open Project…**
and **Recent Projects**. No editor chrome or template selection appears before a project
opens. New Project uses a native Mac name/location dialog and creates a plain folder
containing `<project name>.md`, ready to type. It never replaces an existing folder.
Open Project selects an existing folder; no special project extension is required.

Opening a project restores its story tabs, active file, cursor/scroll positions,
sidebar/preview visibility, split sizes and Light/Dark choice. UI-only session metadata
is saved under `"session"` in `_river/river.json`; this can create the project sidecar
before any extras are used. Existing template settings and unknown JSON keys are kept.
Recent locations are stored in the app's local configuration as `projects.json` (up to
12 projects). River only opens folders it can write to (#74); an entry can be removed
from Recent Projects with its × button, which never touches the folder. Missing folders stay listed as unavailable; failure to open leaves the
welcome screen/current workspace intact. No story content is stored in recents.

File → New Project, Open Project and Close Project also work from the workspace.
Changing/closing projects checks unsaved text, and cancelling leaves it in place.
File → Open remains available for individual story files inside the workspace.
Session restoration reloads **saved files**, not unsaved text: content autosave,
crash recovery and Git are described elsewhere (backups dropped, #50).


## Todos implementation (#46)
Opening Todos reads `_river/todos.md` without creating it. The first saved task
creates the file. A standalone saved story uses `_river/todos.md` in its containing
folder; an untitled story without a project asks the writer to save/open a project.
Tasks stay out of the story-tab strip and Explorer.

The sidebar supports standard `- [ ]` / `- [x]` rows (also uppercase `X` and `*`
bullets imported from another editor). Unknown Markdown is kept, along with BOM and
newline conventions. Reordering moves checkbox-row contents between task positions;
non-task lines retain their positions. This is a flat list, without nested task or
attached-description semantics. External edits are checked on each write and on
idle window focus; conflicts preserve disk content and retain typed input for retry.
A pending quick-entry/title edit is flushed before changing/closing a project or
quitting. Unsaved keystrokes in task inputs are not part of story crash-recovery
journals; tasks persist on Enter, blur or the workspace flush.


## Scratchpad implementation (#48)
Scratchpad saves after one second idle and flushes on blur, sidebar departure, project
changes and quit. Opening it is read-only until text changes; the sidecar/file is
created only on a write. Standalone stories use `_river/scratchpad.md` beside the
story. Existing newline convention/BOM are preserved; the editor uses LF internally.

Dirty text is protected in private app-data recovery journals after 120 ms idle.
After interruption it is offered as an unsaved copy, preserving the source. File
conflicts leave the local text in the sidebar. Save a copy and reload asks for a
new `.md` location; only a successful copy permits reloading. Cancelling or choosing
an existing filename preserves the local text. The copy is never opened as a tab
automatically. Bin was removed by decision #47. Existing files are left alone.


## Standard Git protection (#50–51)
Source Control versions the project as an ordinary Git repository. Commit all changes
includes non-ignored story files, Scratchpad, Todos and other project files; UI state
is omitted. Use `.gitignore` for private material. No separate dated-copy backup
system is planned for v1. Remote operations (v1.1, #64) will provide off-device copies
of committed work; unsaved recovery drafts stay local. There is no private history (#66).
