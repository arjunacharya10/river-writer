//! Selectable, font-embedded PDF from the shared page display list.
use krilla::{
    Document,
    color::rgb,
    geom::{PathBuilder, Point, Rect},
    page::PageSettings,
    paint::Fill,
    text::{Font, GlyphId, KrillaGlyph},
};
use river_model::{DrawOp, Publication};
use river_text::FONT_BYTES;
pub fn encode(publication: &Publication) -> Result<Vec<u8>, String> {
    let fonts: Vec<_> = FONT_BYTES
        .iter()
        .map(|b| Font::new(b.to_vec().into(), 0).ok_or("Invalid bundled PDF font"))
        .collect::<Result<_, _>>()?;
    let mut document = Document::new();
    for page in &publication.pages {
        let mut p = document.start_page_with(
            PageSettings::from_wh(page.width, page.height).ok_or("Invalid page dimensions")?,
        );
        let mut surface = p.surface();
        for op in &page.ops {
            match op {
                DrawOp::Glyphs(run) => {
                    surface.set_fill(Some(Fill {
                        paint: rgb::Color::new(0, 0, 0).into(),
                        ..Fill::default()
                    }));
                    let glyphs: Vec<_> = run
                        .glyphs
                        .iter()
                        .map(|g| KrillaGlyph {
                            glyph_id: GlyphId::new(g.id),
                            text_range: g.text_range.clone(),
                            x_advance: 0.0,
                            y_advance: 0.0,
                            x_offset: g.x / run.size,
                            y_offset: -g.y / run.size,
                            location: None,
                        })
                        .collect();
                    surface.draw_glyphs(
                        Point::from_xy(0.0, 0.0),
                        &glyphs,
                        fonts[run.face].clone(),
                        &run.text,
                        run.size,
                        false,
                    );
                }
                DrawOp::Rect {
                    x,
                    y,
                    width,
                    height,
                    color,
                } => {
                    surface.set_fill(Some(Fill {
                        paint: rgb::Color::new(color[0], color[1], color[2]).into(),
                        ..Fill::default()
                    }));
                    let mut path = PathBuilder::new();
                    path.push_rect(
                        Rect::from_xywh(*x, *y, *width, *height)
                            .ok_or("Invalid rectangle in display list")?,
                    );
                    surface.draw_path(&path.finish().ok_or("Invalid PDF path")?);
                }
            }
        }
        surface.finish();
        p.finish();
    }
    document
        .finish()
        .map_err(|e| format!("PDF export failed: {e:?}"))
}
