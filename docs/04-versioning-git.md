# 04 — Versioning with git

**Decision:** River uses **real git** and **standard git vocabulary**. Writers learn a
small subset once and can then use GitHub or any git tool. The learning curve is
accepted and softened by in-app teaching. (See the decision log for the alternatives
considered: writer-renamed terms, simpler non-git models.)

## Repository
* A normal git repository at the project root (`.git/`), embedded via a Rust git
  library, so no git install is needed.
* A folder that is already a git repository is simply adopted.
* `_river/river.json` is excluded through `.git/info/exclude`; River never edits
  `.gitignore` (#62).
* **v1 scope (#64):** commit, history, diff, restore and branch create/switch. Remotes,
  revert, branch delete/merge, tags, writer-shaped diff, retention and teaching hints
  are v1.1.

## The visible subset
| Term | In River |
|---|---|
| **commit** | "Commit" button + message field; the log lists commits |
| **branch** / **main** | Branch picker in the status bar; "New branch from here" |
| **checkout** | Switch branches, or open an older commit read-only |
| **diff** | Writer-shaped diff view (see below) |
| **merge** | Merge a branch into the current one; conflicts resolved per scene |
| **cherry-pick** | "Cherry-pick this scene/paragraph" from a diff or another branch |
| **tag** | Milestones (`draft-1`, `sent-to-agent`, `shooting-script`) |
| **revert** / **restore** | Undo a commit / restore a file to an earlier commit |
| **remote** / **origin** | "Add remote" (paste a URL) |
| **push** / **pull** / **fetch** | Toolbar buttons + status badge `↑2 ↓1` |
| **conflict** | "Conflict in Scene 14" → **Keep ours / Keep theirs / Keep both** |
| **clone** | "Clone from URL…" on the welcome screen |
| **.gitignore** | Per-file "Ignore" (e.g. a private scratchpad) |

## Hidden (River handles these silently)
Staging area/index (**commit always includes everything**), stash (auto-handled when
switching branches with uncommitted work: offer "commit" or "carry over"), rebase,
reset modes, detached HEAD, submodules, hooks. SHAs appear only as a short code on hover.

## Autosave history (not git commits)
* River keeps an automatic safety net while you write (~every 1–2 min of idle).
* Stored in a **separate local repository at `.git/river-autosave/`** (shares object
  storage with the main repository), never shown as commits and **never pushed** (doc 05 §12.3).
* Thinned over time: everything today, hourly for a week, daily for a month, weekly after.
* A "Local history" view lets you restore from it. This is the "never lose work" guarantee.

## Diff, built for prose
Git diffs by line, and a prose paragraph is one long line, so River diffs at three levels:
1. **Structure:** scenes/sections added, removed, moved (badges in the navigator)
2. **Paragraph:** which beats changed
3. **Word:** inline additions and deletions

Views: side-by-side (synced scroll), inline, and **page diff** (two PDFs side by side
with changed pages flagged). Gutter change bars show edits since the last commit.

## Merge, scene by scene
* Changes in only one side merge automatically.
* For scenes changed on both sides: side-by-side choice per scene (ours/theirs/both/edit).
* A summary afterwards ("12 scenes merged, 1 conflict resolved").

## Revision marks (production)
Industry colored revision pages and asterisks = **the diff since a tag** (e.g.
`shooting-script`), rendered on the PDF. No separate revision system (Highland's
in-document `Revision` objects were the source of its confusion).

## Remotes and network
* Promise: **River makes no network connections except when you push, pull, fetch
  or clone.** No telemetry, no update checks.
* Any git URL works (GitHub, GitLab, Gitea, self-hosted).
* Credentials come from the user's existing setup (SSH key / system credential
  manager). Optional "Sign in with GitHub" (device flow) later.
* No automatic push; an optional reminder after committing.

## Teaching the vocabulary
* One first-run hint per concept, shown when first relevant
  ("A commit is a snapshot of your whole project. Name it like 'Act 2 done'.")
* A one-sentence tooltip on every git control.
* Help → **Git for Writers**: ~10 short cards with writing examples
  ("Use a branch to try a darker ending").
* Defaults: branch `main`; placeholder "Describe what you changed…"; tag suggestions.

## Panel sketch
```
SOURCE CONTROL                     main ↑2 ↓0
[ Describe what you changed…           ]
[ Commit ]   Push   Pull   ⋯
COMMITS
◆ shooting-script (tag)
● After producer notes         Oct 17
● Act 2 done                   Oct 12
BRANCHES
● main
○ darker-ending                [Diff] [Merge]
```


### Implemented draft protection (2026-10-18, #42)
Named files save after one second of idle typing. Private history checkpoints the
previous disk text and each changed saved text before replacement, deduplicating
unchanged content. This supersedes the proposed 90-second/1–2-minute history timing.
The Timeline bottom panel previews per-file versions in batches of 200 and restores
through an undoable editor edit, preserving the current buffer first.

The private repository reads main objects through alternates; its own new objects
and refs stay private. An independent local `git push --mirror` regression test
checks that neither the private commit nor its draft blob reaches the remote.
Standalone files use app data. Untitled drafts use crash recovery until named.
Real commits in Timeline and retention compaction/collection are still planned.
Separate automatic backups are dropped; push to a Git remote instead (#50). Existing snapshots are not pruned yet.

### Source Control, local slice (2026-10-18, #51)
The Source Control sidebar view initializes Git for the project folder (branch
`main`, `_river/river.json` added to `.gitignore`), asks for a project-local name and
email only when no Git identity is configured, lists every change and shows the
patch in the right pane. **Commit all changes** commits the whole working tree,
ignored files excepted, with no staging step. The commit uses a temporary index and
the standard `index.lock`; it is rejected if HEAD or the files changed since the
review shown, keeping the message and any existing staged index. Merges, conflicts
and detached HEAD block committing with a message pointing to Git. History loads 100
commits at a time; selecting one shows its diff. Remotes, push/pull/fetch, branches
and restore are the next slices.

### Restore (2026-10-18, #53)
Selecting a commit shows its files and diff in the right pane. **Restore** beside a
file, or **Restore project…** in the header, asks for confirmation and then rewrites
working files to match that commit. A file the commit deleted is brought back from
the commit's parent. Restore never commits or moves the branch; the result appears as
ordinary uncommitted changes.

Before writing, River saves named tabs, refuses if a tab still has unsaved changes,
records the current project state as a Git tree and checkpoints replaced story files
in private Timeline history. A banner offers **Undo restore**, which puts the restored
paths back as they were unless one was edited after the restore. The pre-restore tree
is unreferenced, so Undo belongs to the current session; Timeline still holds the
replaced story text. Ignored files, `.git` and `_river/river.json` are never touched;
symbolic-link paths, links and submodules in the commit are refused.

### Branches (2026-10-18, #55)
Source Control lists local branches; the current one is checked and also appears in
the status bar (click it to open Source Control). Clicking another branch switches to
it. **Create and switch** makes a branch at the current commit and moves to it,
keeping any uncommitted writing. Switching to a branch on a different commit first
requires committing, so writing is never lost or silently carried between drafts.
Switching uses libgit2's safe checkout. Unsaved tabs block it; changed tabs, Scratchpad
and Todos reload afterwards; private autosave history follows the active branch.
Deleting, renaming and merging branches are not in River yet.

### Git on by default (2026-10-18, #62)
River projects always have Git. On open or create, `git::ensure_repository` creates a
repository if the folder has none and commits what is already there as "Start project
history" (the writer's Git identity, else "River Writer"). Changes therefore starts empty
and shows only what the writer edits. The rule that keeps `_river/river.json` out of Git
lives in `.git/info/exclude`, so River never touches `.gitignore`. Private history uses
the same function, so the first autosave can no longer create a commit-less repository.
New branches can be created immediately; with no commits the field shows a hint.
