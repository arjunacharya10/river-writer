# 09 — Engine CLI: implemented baseline

The Rust workspace now turns real `.fountain` and `.md` files into paginated PDF and
PNG previews. It is the first production engine baseline, not a finished desktop app.
Current target is macOS (decision #36). None of the engine crates depends on Tauri.

## Use

From the repository root:

```sh
cargo run -p river-cli -- export fixtures/screenplay.fountain -o output.pdf
cargo run -p river-cli -- export fixtures/manuscript.md -o manuscript.pdf \
  --preview-dir preview --layout-json pages.json
cargo run -p river-cli -- inspect fixtures/screenplay.fountain
cargo run -p river-cli -- check fixtures/manuscript.md
```

`--template` accepts `screenplay`, `screenplay-multicam`, `stageplay`, `manuscript`.
The extension selects the parser; the template selects output formatting. Defaults:
`.fountain` → Screenplay, `.md` → Manuscript. There is no font option.

PDF/JSON destination parent directories must exist; the preview directory is created
as needed. Existing files are preserved unless `--force` is given. Source files,
including included files and aliases/hard links to them, cannot be used as outputs.
Each file is written through a temporary file before being installed at its destination.
A multi-file export is not a single filesystem transaction.

## Crates and responsibilities

| Crate | Responsibility |
|---|---|
| `river-model` | Elements, styles, per-file UTF-8 source ranges and shared display list |
| `river-parse` | Fountain state machine, CommonMark subset, inline formatting, includes |
| `river-templates` | Typed bundled data for the four templates |
| `river-text` | Parley shaping, bundled Courier Prime faces and bounded shape cache |
| `river-layout` | Element geometry, uppercase mapping, styled lines and sentence-aware splits |
| `river-paginate` | Keep-with-next, dialogue/dual dialogue, continuation labels, title page, headers |
| `river-pdf` | Selectable, subset-embedded-font PDF from the display list |
| `river-render` | White-paper PNG preview from the same display list |
| `river-cli` | File input, diagnostics, export/check/inspect commands |

## Covered behaviour

* Fountain classification, forced elements, sections, synopses, hidden notes/boneyard,
  lyrics, explicit page breaks, scene numbers, cues/parentheticals/dialogue and `^`.
* Fountain inline bold, italic, combined bold/italic, underline, highlight, strike,
  escapes and hidden notes, with source ranges preserved through stripping/uppercase.
* Markdown headings, paragraphs, emphasis, blockquotes, lists and scene-break rules,
  parsed through pulldown-cmark. Unsupported code/HTML/images produce diagnostics;
  this is deliberately a small creative-writing subset, not a general Markdown renderer.
* Title/credit/author/source/contact/date/revision fields and indented continuation values.
* Relative `{{INCLUDE: path}}` expansion. Canonical paths stay within the initial
  file's directory; cycles and depth beyond 32 are rejected. Included elements retain
  their source-file identity. No content is fetched from the network.
* Heading keep-with-next, paragraph widows/orphans, sentence-preferred page splits,
  and removal of first-line indentation on manuscript continuations. If a sentence
  exceeds a page, whole-line fallback guarantees progress.
* Long dialogue splits with `(MORE)` and repeated `CUE (CONT'D)`; at least two body
  lines are kept on each side. Parentheticals stay with subsequent dialogue.
* Side-by-side dual dialogue, including continuations and unequal-length sides.
* Multi-cam double-spaced dialogue and parentheticals merged into following dialogue.
* Conventional title page, page numbers, `HEADER`, `FOOTER`, `PAGENUMBER` directives.
  Header/footer fields support `%p`, `%pp`, `%title`, `%none`, either one field or
  left/center/right fields separated by `|`. Other directives fail explicitly.
* All four real Courier Prime faces at 12 pt. Use actual font advances (7.1953125 pt),
  not rounded 7.2 pt column arithmetic. Unsupported glyphs fail clearly instead of
  being silently substituted. Inline decorations are identical display-list operations
  in PDF and preview.

## Validation

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p river-cli --example benchmark
python3 scripts/verify_engine.py
```

The image check needs Poppler (`pdftoppm`, `pdftotext`, `pdffonts`), Pillow and NumPy,
and a built `target/debug/river` (or `RIVER_BIN`). It exports the original fixtures,
checks selectable text, hidden-text exclusion, embedded fonts and the 108 pt scene
heading position, and compares independent PDF renders to previews at 144 dpi.
It also rejects a deliberately shifted 3px negative-control image.
Outputs and the JSON report are under ignored `output/engine-verification/`.

There are 36 unit/integration tests, including golden page/line snapshots for all four
templates, speech conservation over multiple pages, dual dialogue, source mappings,
Unicode/CRLF, includes, output collisions, source hard links and malformed input.
Goldens in `fixtures/golden/` are reviewed regression baselines for River's original
fixtures, not independent proof of exact Highland pagination. To deliberately replace
them after reviewing a formatting change:

```sh
RIVER_UPDATE_GOLDENS=1 cargo test -p river-cli --test engine golden_all_templates
```

Measured 2026-10-17, Apple M5 / Rust 1.96, release build; one cold run and one warm edit:

| Operation | Time |
|---|---:|
| Parse 225,610 bytes | 20.6 ms |
| Shape/layout/paginate 120 actual pages | 51.0 ms |
| Full reparse and repaginate after a middle-file edit, reusing shape cache | 45.3 ms |
| Encode the 120-page PDF | 16.0 ms |
| Rasterize first page at 144 dpi, excluding PNG encoding | 8.6 ms |

The benchmark's sections are separated by explicit page breaks and asserted to fit
120 real pages. It exercises grouping and pagination; its workload is still synthetic.
The warm edit reparses the entire document: it is **not yet incremental parsing or
incremental page invalidation**. Both PDF/raster times are excluded from layout timing.

## Boundaries and follow-up

* Letter paper only for this baseline; A4 and Page Setup integration remain future work.
* The title page uses a conventional arrangement, not Highland's full 3×3 box model.
* Stage Play uses verified H2 geometry; exact Pro Stage Play formulas remain an open
  research item. Dual-dialogue columns use a conventional layout rather than a claim
  of pixel-for-pixel Highland parity. These need broader writer/reference fixtures.
* `%#`, TOC, revision directives, automatic scene numbering, PDF outlines, FDX,
  includes outside the input folder and printing hidden notes are not implemented.
* No system-font fallback yet; unsupported characters are reported as export errors.
* Scene/character indexes, semantic editor tokens, incremental parse/re-pagination,
  project-level configuration and application services come with later integration.
* No git, production workbench, autosave, file watcher or native print UI in these crates.
  The phase-0 trials were removed after the production app replaced them. The shell uses
  Kobalte/Corvu with River wrappers and native Mac menus/dialogs (decision #37).
