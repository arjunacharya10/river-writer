# 01 — Product brief

## One sentence
**A quiet, offline app where you write your story in plain text and it comes out as a
perfectly formatted script or manuscript, with no accounts, no cloud and no
subscription.**

Positioning: **lead with craft** ("the most beautiful, correct pages"). Privacy and
offline operation are the proof points, not the headline.

Shorthand: **Obsidian's foundations, Final Draft's pages, iA Writer's restraint.**

## Audience
Creative writers only:
* **Screen:** features, TV (with a multi-cam option)
* **Stage:** plays
* **Page:** novels, short fiction, manuscript submissions

Out: comics/graphic novels, essays, academic/MLA, wikis, bulletins, speeches, articles.

## Principles
1. **File over app.** Your words are plain text files you own: readable in any editor, forever.
2. **The page is the truth.** An exact live preview of the printed page; correct
   formatting by default.
3. **Nothing leaves the machine** unless you explicitly push, pull, fetch or clone.
4. **Opinionated over configurable.** Three forms done perfectly beat 14 done adequately.
5. **Calm.** No badges, nags, streaks, pop-ups, surveys, paywalls or callouts.
6. **Laser-sharp.** Every feature must serve the act of writing a story.
7. **Multiplatform, slow features.** **macOS and Windows** from v1. Every feature ships
   on both at the same time, with the same behaviour; no platform-exclusive features.
   River grows slowly and deliberately rather than fast and uneven. **Linux: later or
   never** (not on the core roadmap; as an open-source project, a community port is welcome).
8. **Open source.** River's source is public (license TBD, see doc 06).

## Keep (core)
* Fountain for screen and stage; Markdown-style prose for novels
* Live paginated preview + PDF export: (MORE)/(CONT'D), dual dialogue, scene numbers,
  breaks at sentence boundaries, title page
* Templates: **Screenplay** (with a multi-cam option), **Stage Play**, **Novel/Manuscript**
* Navigator: scenes, sections, synopses, notes
* Notes `[[ ]]`, boneyard `/* */`, synopses `=`
* Final Draft (FDX) import and export
* Includes (`{{INCLUDE: …}}`) for chapter-per-file projects
* Characters list (names, counts, rename)
* Two themes only: **Light** and **Dark** (modelled on VS Code's Light/Dark Modern), following the system appearance (more can come later)
* **Scratchpad, todos, bin**, each just a plain-text dump (see doc 02)
* Git-based versioning with standard git vocabulary (see doc 04)
* **UI exactly like VS Code** (see doc 03)
* Automatic local backups

## Cut entirely
Cloud sync, Fling/web review and comments, accounts and sign-in, AI features,
subscriptions/paywalls/student licensing, analytics and crash reporting, surveys,
callouts and "What's New", non-creative templates, gender and unique-word reports,
grammar highlights, Shelf/Snippets/custom pages/cast and location pages, the
header/footer builder (one sensible header per template plus a directive override),
tables/checklists/code blocks/footnotes in story formats.

## Rethought from Highland Pro
| Highland Pro | River |
|---|---|
| Fling web review | Reader-copy PDF export (optional line numbers and margin notes) |
| Cloud versions / revisions | Git: commits, branches, diff, merge, tags (doc 04) |
| Revision mode | Revision marks = the diff since a tag, rendered as colored pages and asterisks |
| Sprints/goals/milestones | Optional daily word-goal ring, off by default |
| Scratchpad + Bin + Todos | Plain-text files in `_river/` |
| Backups | Automatic local backups to a chosen folder |

## Where it wins
* **Against Final Draft / Fade In:** plain text, calmer, cheaper, beautiful typography.
* **Against Highland Pro:** focused, private, one-time purchase, faster, no sign-in.
* **Against Scrivener / Ulysses:** truly correct script formatting, simpler model.
