# Typeset spike — resume notes

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
