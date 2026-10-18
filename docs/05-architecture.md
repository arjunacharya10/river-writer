# 05 — Architecture notes (Tauri)

Ideation-level only. Nothing here is built yet.

## Shape
* **Tauri** desktop app: small, native-feeling, fully offline. All network
  permissions removed except what git remotes need.
* **Rust core**, UI-independent and unit-testable:
  * Fountain/Markdown **parser** (classification rules and dialogue state machine from
    `~/rea/HIGHLAND_SPEC.md` §4)
  * **Grouper** (keep-together groups, printability filter, blank-line compression) — spec §6
  * **Paginator** (fit by remaining height, sentence-boundary splits, (MORE)/(CONT'D),
    dual dialogue, continuation without a first-line indent) — spec §7
  * **Templates** as data: values from spec §5 / §10.2
  * **Layout/typesetting engine** shared by preview and PDF
  * **FDX** import/export
  * **git** via an embedded Rust git library (no system git needed)
* **Webview UI**: a replica of the VS Code workbench (doc 03) built on **Monaco** (editor +
  diff editor, MIT) and **Codicons**; Fountain/Markdown language modes; page preview as
  an editor tab. Not a Code-OSS fork.

## The hard part: typesetting
Highland relies on Apple's TextKit (`NSTextContainer`/`NSLayoutManager`) to measure and
break text. River must own this:
* One layout engine (text shaping + line breaking) produces the page geometry, and both
  the on-screen preview (canvas) and the PDF writer draw from it, so preview = print.
* **Screenplays are the easy case:** Courier is monospaced, so line breaking and page
  fitting are almost arithmetic (12 pt font, 72 pt margins, element indents from spec §5).
* **Prose is the harder half:** proportional fonts, justification (and the trick of
  keeping a page's last line justified), first-line indents, sentence-boundary breaks.
  Design for it from day one.
* **Bundle fonts** (e.g. Courier Prime, which has an open license, plus one prose serif)
  so pages render identically everywhere.

## Geometry model (from Highland)
Element x = page left margin + leftMargin × page width; width = width × page width.
Screenplay on Letter: action 1.5″, dialogue ~2.5″ (3.46″ wide), parenthetical ~2.9″,
character cue ~3.5″, all from the paper edge. Templates are data, so other paper sizes
scale.

## Platforms
Mac first or all three platforms is an open decision (doc 06). Tauri makes
cross-platform cheap; the cost is font rendering and print testing per platform.
