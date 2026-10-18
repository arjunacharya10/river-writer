//! Rust preview rasterizer. Receives the exact page operations used by PDF export.
use river_model::{DrawOp, Page};
use river_text::FONT_BYTES;
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
};
use std::collections::HashMap;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};
struct Pen(PathBuilder);
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, -y)
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, -y)
    }
    fn quad_to(&mut self, a: f32, b: f32, x: f32, y: f32) {
        self.0.quad_to(a, -b, x, -y)
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.0.cubic_to(a, -b, c, -d, x, -y)
    }
    fn close(&mut self) {
        self.0.close()
    }
}
pub fn page(page: &Page, dpi: f32) -> Result<Pixmap, String> {
    if !dpi.is_finite() || !(36.0..=600.0).contains(&dpi) {
        return Err("Preview DPI must be between 36 and 600".into());
    }
    let scale = dpi / 72.0;
    let width = (page.width * scale).round() as u32;
    let height = (page.height * scale).round() as u32;
    if u64::from(width) * u64::from(height) > 50_000_000 {
        return Err("Preview exceeds 50 megapixels".into());
    }
    let mut pixmap = Pixmap::new(width, height).ok_or("Cannot allocate preview image")?;
    pixmap.fill(Color::WHITE);
    let fonts: Vec<_> = FONT_BYTES
        .iter()
        .map(|b| FontRef::new(b).expect("validated bundled font"))
        .collect();
    let mut outlines = HashMap::new();
    for op in &page.ops {
        match op {
            DrawOp::Glyphs(run) => {
                let mut paint = Paint::default();
                paint.set_color(Color::BLACK);
                for g in &run.glyphs {
                    let key = (run.face, g.id, run.size.to_bits());
                    if let std::collections::hash_map::Entry::Vacant(entry) = outlines.entry(key) {
                        let outline = fonts[run.face]
                            .outline_glyphs()
                            .get(GlyphId::new(g.id))
                            .ok_or("Missing glyph outline")?;
                        let mut pen = Pen(PathBuilder::new());
                        outline
                            .draw(
                                DrawSettings::unhinted(Size::new(run.size), LocationRef::default()),
                                &mut pen,
                            )
                            .map_err(|e| format!("Glyph rendering failed: {e:?}"))?;
                        entry.insert(pen.0.finish());
                    }
                    if let Some(path) = outlines.get(&key).unwrap() {
                        pixmap.fill_path(
                            path,
                            &paint,
                            FillRule::Winding,
                            Transform::from_row(scale, 0.0, 0.0, scale, g.x * scale, g.y * scale),
                            None,
                        );
                    }
                }
            }
            DrawOp::Rect {
                x,
                y,
                width,
                height,
                color,
            } => {
                let rect = tiny_skia::Rect::from_xywh(
                    *x * scale,
                    *y * scale,
                    *width * scale,
                    *height * scale,
                )
                .ok_or("Invalid rectangle in display list")?;
                let mut paint = Paint::default();
                paint.set_color_rgba8(color[0], color[1], color[2], 255);
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
        }
    }
    Ok(pixmap)
}
