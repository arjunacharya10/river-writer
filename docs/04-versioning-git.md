# 04 — Versioning with git

**Decision:** River uses **real git** and **standard git vocabulary**. Writers learn a
small subset once and can then use GitHub or any git tool. The learning curve is
accepted and softened by in-app teaching. (See the decision log for the alternatives
considered: writer-renamed terms, simpler non-git models.)

## Repository
* A normal git repository at the project root (`.git/`), embedded via a Rust git
  library, so no git install is needed.
* A folder that is already a git repository is simply adopted.
* `.gitignore` always contains `_river/river.json`.

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
