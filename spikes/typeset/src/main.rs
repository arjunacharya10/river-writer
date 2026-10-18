//! Disposable phase-0 experiment, not River's parser or production paginator.
use krilla::{
    Document,
    geom::Point,
    page::PageSettings,
    text::{Font, GlyphId as PdfGlyphId, KrillaGlyph},
};
use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{
    FontContext, FontStyle, FontWeight, Layout, LayoutContext, LineHeight, PositionedLayoutItem,
    StyleProperty,
};
use skrifa::{
    FontRef, GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
};
use std::{fs, ops::Range, path::Path, sync::Arc, time::Instant};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const SIZE: f32 = 12.0;
const WIDTH: f32 = 612.0;
const HEIGHT: f32 = 792.0;
const SCALE: f32 = 2.0; // 144 dpi, independent of display scale.

#[derive(Clone, Copy, Debug)]
enum Face {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}
impl Face {
    fn index(self) -> usize {
        self as usize
    }
    fn name(self) -> &'static str {
        match self {
            Self::Regular => "Regular",
            Self::Bold => "Bold",
            Self::Italic => "Italic",
            Self::BoldItalic => "BoldItalic",
        }
    }
}
const FACES: [Face; 4] = [Face::Regular, Face::Bold, Face::Italic, Face::BoldItalic];
#[derive(Clone, Debug)]
struct Glyph {
    id: u32,
    x: f32,
    y: f32,
    range: Range<usize>,
}
#[derive(Clone, Debug)]
struct DrawRun {
    face: Face,
    text: Arc<str>,
    glyphs: Vec<Glyph>,
}
#[derive(Clone, Debug, Default)]
struct Page {
    runs: Vec<DrawRun>,
}
struct Engine {
    fonts: FontContext,
    context: LayoutContext<()>,
    bytes: Vec<Vec<u8>>,
    tracking: Vec<f32>,
}
impl Engine {
    fn new() -> Self {
        let mut fonts = FontContext {
            collection: Collection::new(CollectionOptions {
                shared: false,
                system_fonts: false,
            }),
            source_cache: SourceCache::default(),
        };
        let bytes: Vec<_> = FACES
            .iter()
            .map(|face| {
                let data = fs::read(format!(
                    "{ROOT}/../../fonts/courier-prime/CourierPrime-{}.ttf",
                    face.name()
                ))
                .unwrap();
                assert!(
                    !fonts
                        .collection
                        .register_fonts(Blob::new(Arc::new(data.clone())), None)
                        .is_empty()
                );
                data
            })
            .collect();
        let tracking = bytes
            .iter()
            .map(|data| {
                let font = FontRef::new(data).unwrap();
                let advance = font
                    .glyph_metrics(Size::new(SIZE), LocationRef::default())
                    .advance_width(font.charmap().map('M').unwrap())
                    .unwrap();
                7.2 - advance
            })
            .collect();
        Self {
            fonts,
            context: LayoutContext::new(),
            bytes,
            tracking,
        }
    }
    fn shape(&mut self, text: &str, width: f32, face: Face, pitch: f32) -> Layout<()> {
        let mut b = self
            .context
            .ranged_builder(&mut self.fonts, text, 1.0, false);
        b.push_default(StyleProperty::FontFamily("Courier Prime".into()));
        b.push_default(StyleProperty::FontSize(SIZE));
        // Courier Prime's 1228/2048 em advance is just short of 0.6 em.
        // Preserve the font outlines at 12 pt and normalize the writing grid.
        b.push_default(StyleProperty::LetterSpacing(self.tracking[face.index()]));
        b.push_default(StyleProperty::LineHeight(LineHeight::Absolute(pitch)));
        b.push_default(StyleProperty::FontWeight(FontWeight::new(
            if matches!(face, Face::Bold | Face::BoldItalic) {
                700.0
            } else {
                400.0
            },
        )));
        b.push_default(StyleProperty::FontStyle(
            if matches!(face, Face::Italic | Face::BoldItalic) {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
        ));
        let mut layout = b.build(text);
        layout.break_all_lines(Some(width));
        layout
    }
    fn paragraph(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        face: Face,
        pitch: f32,
    ) -> (Vec<DrawRun>, f32) {
        let layout = self.shape(text, width, face, pitch);
        let shared: Arc<str> = text.into();
        let mut runs = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(g) = item {
                    // Catch accidental font fallback or synthesis before exporting.
                    assert_eq!(g.run().font().data.data(), self.bytes[face.index()]);
                    let ranges: Vec<_> = g
                        .run()
                        .visual_clusters()
                        .filter(|c| {
                            let r = c.text_range();
                            r.start >= line.text_range().start && r.end <= line.text_range().end
                        })
                        .flat_map(|c| {
                            let range = c.text_range();
                            c.glyphs().map(move |_| range.clone())
                        })
                        .collect();
                    let positions: Vec<_> = g.positioned_glyphs().collect();
                    assert_eq!(ranges.len(), positions.len(), "cluster mapping");
                    let glyphs = positions
                        .into_iter()
                        .zip(ranges)
                        .map(|(p, range)| {
                            assert_ne!(p.id, 0, "unsupported glyph in fixture");
                            Glyph {
                                id: p.id,
                                x: x + p.x,
                                y: y + p.y,
                                range,
                            }
                        })
                        .collect();
                    runs.push(DrawRun {
                        face,
                        text: shared.clone(),
                        glyphs,
                    });
                }
            }
        }
        (runs, layout.height())
    }
}

// Original fixtures, written for River; no Highland assets.
const ACTION: &str = "Rain taps the roof of a deserted ferry terminal. MAYA sets a brass key on the counter. Across the river, a single window flickers.";
const DIALOGUE: &str = "The last boat left an hour ago. If someone is still over there, they found another way across.";
const PROSE: &str = "By morning the river had carried away the steps. Nora stood where the garden ended and watched a branch turn slowly in the current. She had expected the house to feel smaller after everyone left. Instead, each empty room seemed to have acquired another door.";
fn screenplay(engine: &mut Engine) -> Page {
    let mut page = Page::default();
    let mut y = 72.0;
    for (text, x, width, face, gap) in [
        (
            "EXT. FERRY TERMINAL - NIGHT",
            108.0,
            400.0,
            Face::Bold,
            12.0,
        ),
        (ACTION, 108.0, 432.0, Face::Regular, 12.0),
        ("MAYA", 248.976, 291.024, Face::Regular, 0.0),
        ("(turning the key)", 207.0, 211.032, Face::Italic, 0.0),
        (DIALOGUE, 178.992, 248.976, Face::Regular, 12.0),
        (
            "A bell sounds once. Maya looks toward the water.",
            108.0,
            432.0,
            Face::Regular,
            12.0,
        ),
        ("ELIAS (O.S.)", 248.976, 291.024, Face::Regular, 0.0),
        (
            "Then we wait for the light.",
            178.992,
            248.976,
            Face::Regular,
            12.0,
        ),
        ("CUT TO:", 489.6, 50.4, Face::Regular, 12.0),
        ("INT. SIGNAL ROOM - NIGHT", 108.0, 400.0, Face::Bold, 12.0),
        (
            "An empty chair. A wet coat. The radio begins to hiss.",
            108.0,
            432.0,
            Face::Regular,
            12.0,
        ),
        (
            "Somebody has been here.",
            108.0,
            432.0,
            Face::BoldItalic,
            0.0,
        ),
    ] {
        let (runs, height) = engine.paragraph(text, x, y, width, face, 12.0);
        page.runs.extend(runs);
        y += height + gap;
    }
    assert!(y < 720.0);
    page
}
fn manuscript(engine: &mut Engine) -> Page {
    let mut page = Page::default();
    let (header, _) = engine.paragraph(
        "RIVER / THE OTHER BANK / 1",
        360.0,
        36.0,
        216.0,
        Face::Regular,
        12.0,
    );
    page.runs.extend(header);
    let (heading, _) = engine.paragraph("CHAPTER ONE", 266.4, 96.0, 100.0, Face::Bold, 24.0);
    page.runs.extend(heading);
    let mut y = 156.0;
    for text in [
        PROSE,
        "She put the key in her pocket. On the far bank, the ferryman raised one hand, as though they had arranged to meet. Nora did not wave back.",
        "The letter had said nothing about a flood. It had mentioned the apple trees, the broken clock, and the spare room at the end of the hall. It had said: Come before the first frost.",
    ] {
        // Parley does not expose first-line indent directly here. Wrap the first
        // line at the narrower measure, then re-shape the remaining paragraph.
        let first_layout = engine.shape(text, 432.0, Face::Regular, 24.0);
        let first_end = first_layout.lines().next().unwrap().text_range().end;
        let first = text[..first_end].trim_end();
        let rest = text[first_end..].trim_start();
        let (runs, h) = engine.paragraph(first, 108.0, y, 432.0, Face::Regular, 24.0);
        page.runs.extend(runs);
        y += h;
        if !rest.is_empty() {
            let (runs, h) = engine.paragraph(rest, 72.0, y, 468.0, Face::Regular, 24.0);
            page.runs.extend(runs);
            y += h;
        }
    }
    assert!(y < 720.0);
    page
}
fn pdf(page: &Page, bytes: &[Vec<u8>], path: &Path) {
    let fonts: Vec<_> = bytes
        .iter()
        .map(|b| Font::new(b.clone().into(), 0).unwrap())
        .collect();
    let mut doc = Document::new();
    let mut p = doc.start_page_with(PageSettings::from_wh(WIDTH, HEIGHT).unwrap());
    let mut surface = p.surface();
    for run in &page.runs {
        // Absolute positions in our list, converted to krilla's em units.
        // Zero advances prevent krilla from accumulating a second set of positions.
        let glyphs: Vec<_> = run
            .glyphs
            .iter()
            .map(|g| KrillaGlyph {
                glyph_id: PdfGlyphId::new(g.id),
                text_range: g.range.clone(),
                x_advance: 0.0,
                y_advance: 0.0,
                x_offset: g.x / SIZE,
                y_offset: -g.y / SIZE,
                location: None,
            })
            .collect();
        surface.draw_glyphs(
            Point::from_xy(0.0, 0.0),
            &glyphs,
            fonts[run.face.index()].clone(),
            &run.text,
            SIZE,
            false,
        );
    }
    surface.finish();
    p.finish();
    fs::write(path, doc.finish().unwrap()).unwrap();
}
struct Pen(tiny_skia::PathBuilder);
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, -y)
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, -y)
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.quad_to(cx, -cy, x, -y)
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.0.cubic_to(a, -b, c, -d, x, -y)
    }
    fn close(&mut self) {
        self.0.close()
    }
}
fn raster(page: &Page, bytes: &[Vec<u8>], path: &Path) {
    let mut pix = tiny_skia::Pixmap::new((WIDTH * SCALE) as u32, (HEIGHT * SCALE) as u32).unwrap();
    pix.fill(tiny_skia::Color::WHITE);
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    for run in &page.runs {
        let font = FontRef::new(&bytes[run.face.index()]).unwrap();
        for glyph in &run.glyphs {
            let outline = font.outline_glyphs().get(GlyphId::new(glyph.id)).unwrap();
            let mut pen = Pen(tiny_skia::PathBuilder::new());
            outline
                .draw(
                    DrawSettings::unhinted(Size::new(SIZE), LocationRef::default()),
                    &mut pen,
                )
                .unwrap();
            if let Some(path) = pen.0.finish() {
                pix.fill_path(
                    &path,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    tiny_skia::Transform::from_row(
                        SCALE,
                        0.0,
                        0.0,
                        SCALE,
                        glyph.x * SCALE,
                        glyph.y * SCALE,
                    ),
                    None,
                );
            }
        }
    }
    pix.save_png(path).unwrap();
}
fn verify_metrics(engine: &mut Engine) {
    // Courier's actual units can differ slightly from the nominal 7.2 pt.
    for face in FACES {
        let layout = engine.shape("MMMMMMMMMM", 1000.0, face, 12.0);
        println!(
            "{}: ten characters = {:.5} pt (nominal 72 pt)",
            face.name(),
            layout.width()
        );
        assert!((layout.width() - 72.0).abs() < 0.02);
        let wrapped = engine.shape(
            "ONE TWO THREE FOUR FIVE SIX SEVEN EIGHT NINE TEN",
            72.0,
            face,
            12.0,
        );
        assert!(wrapped.lines().count() > 1);
    }
}
fn benchmark(engine: &mut Engine) {
    // Unique paragraphs, not 120 clones of an already laid-out page. Full
    // shaping + wrapping + display-list construction, without render/export.
    let paragraphs: Vec<_> = (0..120)
        .map(|i| format!("{ACTION} Crossing number {i:03}. {DIALOGUE}"))
        .collect();
    let mut times = Vec::new();
    let mut glyph_count = 0;
    for sample in 0..11 {
        let start = Instant::now();
        let mut pages = Vec::new();
        for text in &paragraphs {
            let mut page = Page::default();
            for row in 0..9 {
                let (runs, _) = engine.paragraph(
                    text,
                    108.0,
                    72.0 + row as f32 * 72.0,
                    432.0,
                    Face::Regular,
                    12.0,
                );
                page.runs.extend(runs);
            }
            pages.push(page);
        }
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        glyph_count = pages
            .iter()
            .flat_map(|p| &p.runs)
            .map(|r| r.glyphs.len())
            .sum();
        std::hint::black_box(&pages);
        if sample > 0 {
            times.push(elapsed);
        }
    }
    times.sort_by(f64::total_cmp);
    println!(
        "120 synthetic pages ({glyph_count} glyphs): full layout median {:.2} ms, max {:.2} ms (10 warm samples)",
        times[5], times[9]
    );
    println!(
        "150 ms target: {} (full layout; production incremental pagination not implemented)",
        if times[9] < 150.0 { "PASS" } else { "NOT MET" }
    );
}
fn main() {
    let output = Path::new(ROOT).join("output");
    fs::create_dir_all(&output).unwrap();
    let mut engine = Engine::new();
    verify_metrics(&mut engine);
    for (name, page) in [
        ("screenplay", screenplay(&mut engine)),
        ("manuscript", manuscript(&mut engine)),
    ] {
        let start = Instant::now();
        pdf(&page, &engine.bytes, &output.join(format!("{name}.pdf")));
        let pdf_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        raster(&page, &engine.bytes, &output.join(format!("{name}.png")));
        println!(
            "{name}: PDF {pdf_ms:.2} ms, PNG {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
    benchmark(&mut engine);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn courier_metrics_and_cluster_ranges() {
        let mut e = Engine::new();
        verify_metrics(&mut e);
        for face in FACES {
            let text = "A café, ‘river’, fi fl — and e\u{301}.";
            let (runs, _) = e.paragraph(text, 72.0, 72.0, 150.0, face, 12.0);
            for run in runs {
                for g in run.glyphs {
                    assert!(text.get(g.range).is_some());
                    assert!(g.x >= 72.0);
                    assert!(g.y >= 72.0);
                }
            }
        }
    }
    #[test]
    fn pages_stay_within_paper() {
        let mut e = Engine::new();
        for page in [screenplay(&mut e), manuscript(&mut e)] {
            assert!(!page.runs.is_empty());
            for run in page.runs {
                for g in run.glyphs {
                    assert!((36.0..=576.0).contains(&g.x));
                    assert!((36.0..=720.0).contains(&g.y));
                }
            }
        }
    }
}
