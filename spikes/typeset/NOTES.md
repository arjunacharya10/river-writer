# Typeset spike — results and implementation notes

Goal (doc 05 §16 phase 0): Courier Prime screenplay page + manuscript page → PDF (krilla)
and PNG (tiny-skia) from one display list; outputs must match; 120 pages < 150 ms.

## API findings (parley 0.12, krilla 0.8.2, skrifa 0.44 to match parley)
* Parley fonts: `FontContext { collection: Collection::new(CollectionOptions { shared: false,
  system_fonts: false }), source_cache: SourceCache::default() }`, then
  `collection.register_fonts(Blob::new(Arc::new(bytes)), None)` for each Courier Prime TTF.
* Styles: `StyleProperty::FontFamily("Courier Prime".into())`, `FontSize(12.0)`,
  `LineHeight(LineHeight::Absolute(12.0))` (6 lines/inch; manuscript 24.0).
* Layout: `ranged_builder(...).build(text)`, `break_all_lines(Some(width_pt))`; glyphs via
  `line.items()` → `PositionedLayoutItem::GlyphRun(g)` → `g.positioned_glyphs()` (`id, x, y`),
  `g.run().font()` (`FontData { data, index }`), `g.run().font_size()`.
* krilla: `Font::new(data.into(), 0)`, `document.start_page_with(PageSettings::from_wh(612.0, 792.0))`,
  `surface.draw_glyphs(Point, &[KrillaGlyph], font, text, size, false)`; KrillaGlyph advances
  and offsets are in **em units** (krilla multiplies by size). Origin is top-left, y down.
  krilla uses skrifa 0.42 and parley uses 0.44: pass glyph ids as integers.
* Raster: skrifa 0.44 outlines → tiny-skia path. Independent check: `/opt/homebrew/bin/pdftoppm`.

## Geometry (Letter, margins 72 pt; spec §5)
Screenplay: action x=1.5″ w=6.0″ · cue x=3.458″ · paren x=2.875″ w=2.931″ ·
dialogue x=2.486″ w=3.458″ · transition right-aligned to 7.5″.
Manuscript: x=1″ w=6.5″, first-line indent 36 pt, 24 pt line pitch, ragged-right.

## Result (2026-10-17, macOS / Apple M5 / Rust 1.96)

Implemented in `src/main.rs`. Two original fixtures produce Letter-size PDF and
144 dpi PNG from the same positioned glyph list. All four font faces are exercised.
PDFs retain selectable text; no fonts are discovered from the operating system.

* Release full layout of 120 synthetic pages / 265,680 glyphs: median **56.67 ms**,
  worst **62.85 ms** across 10 warm samples after one warm-up. Each page has nine
  paragraphs, with a unique page number in its text. Font/layout contexts are reused;
  paragraphs are shaped and wrapped again, not cloned from a completed page.
* Export per page: PDF 0.99–1.92 ms; raster PNG 15.16–16.52 ms (includes encoding).
* Independent Poppler renders: zero dark pixels without a counterpart within a 1px
  radius, in either direction, without fitted alignment. Mean absolute image
  differences: screenplay 0.677, manuscript 0.916 on a 0–255 scale. Different
  anti-aliasing means these are **not byte-identical images**. A deliberately shifted
  3px image is rejected by the comparator.
* Unit tests: nominal 10 cpi across all four faces; Unicode cluster ranges through
  shaping/wrapping (accented letters, combining mark, punctuation); page bounds.
* `cargo clippy -- -D warnings` passes.

Courier Prime's actual advance is 1228/2048 em, or 7.1953125 pt at 12 pt. The
trial adds 0.0046875 pt letter spacing to reach a 7.2 pt writing grid without
changing the font outlines. Ten characters measure exactly 72 pt in Parley.

### Reproduce

From the repository root:

```sh
cargo test --manifest-path spikes/typeset/Cargo.toml
cargo run --release --manifest-path spikes/typeset/Cargo.toml
python3 spikes/typeset/verify.py
```

The comparison needs Poppler (`pdftoppm`, `pdftotext`, `pdfinfo`), Pillow and NumPy.
Generated files and `verification.json` live in ignored `spikes/typeset/output/`.
The Codex bundled Python includes the image dependencies on this machine.

### Limits / phase 1 follow-up

This proves the selected layout/PDF/raster combination on macOS, not the full
formatting engine. No parser, production pagination, scene grouping, title page,
(MORE)/(CONT'D), dual dialogue, incremental edit cache, or Windows validation yet.
The 150 ms measurement is full layout on synthetic page-sized blocks, not an end-to-end
edit-and-repaginate benchmark. Rasterization is excluded from that timing.

Screenplay uses a conservative 6-inch action measure ending at 7.5 inches; the
Highland spec's unresolved default-width/right-edge question remains open. The
first-line manuscript indent is simulated by separately wrapping the first line.
Production code should carry source ranges through that split and pagination.

## Shared geometry follow-up

Both trials now read `../template.json` (source geometry; no duplicate editor values).
The refactored trial was rechecked: 44.96 ms median / 45.67 ms max for the same
265,680-glyph workload. PDF/PNG comparison still passes, including its negative
control. Normal run-to-run timing varies with compilation and other applications.
