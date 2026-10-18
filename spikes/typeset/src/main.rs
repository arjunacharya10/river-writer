//! Phase-0 typesetting spike: elements → Parley line layout → one display list →
//! PDF (krilla) and PNG (skrifa outlines + tiny-skia). Throwaway code (doc 05 §16).

use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStyle, FontWeight, IndentOptions, Layout,
    LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty,
};

const FONT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts/courier-prime");
const FONT_FILES: [&str; 4] = [
    "CourierPrime-Regular.ttf",
    "CourierPrime-Bold.ttf",
    "CourierPrime-Italic.ttf",
    "CourierPrime-BoldItalic.ttf",
];
const PAGE_W: f32 = 612.0;
const PAGE_H: f32 = 792.0;
const INCH: f32 = 72.0;
const SIZE: f32 = 12.0;

// ---------------------------------------------------------------- display list

/// Font face index into `FONT_FILES`.
type Face = usize;

#[derive(Clone, Debug)]
struct GlyphRun {
    face: Face,
    size: f32,
    /// Baseline origin in page points (top-left origin, y down).
    origin: (f32, f32),
    /// (glyph id, x relative to origin, advance, text range in `text`).
    glyphs: Vec<(u32, f32, f32, Range<usize>)>,
    text: String,
}

#[derive(Clone, Debug)]
enum DrawOp {
    Glyphs(GlyphRun),
    /// Underline etc.: from, to, width.
    Rule((f32, f32), (f32, f32), f32),
}

#[derive(Clone, Debug, Default)]
struct Page {
    ops: Vec<DrawOp>,
}

// ---------------------------------------------------------------- templates

#[derive(Clone, Copy, Debug, PartialEq)]
enum El {
    SceneHeading,
    Action,
    Cue,
    Parenthetical,
    Dialogue,
    Transition,
    Chapter,
    Prose,
}

#[derive(Clone, Copy)]
struct Geo {
    /// Left edge from the page edge, in inches.
    left: f32,
    /// Maximum width, in inches.
    width: f32,
    align: Alignment,
    bold: bool,
    upper: bool,
    first_indent: f32,
    /// Blank lines before this element (in line pitches).
    space_before: f32,
}

struct Template {
    pitch: f32,
    header: bool,
    geo: fn(El) -> Geo,
}

fn geo(left: f32, width: f32) -> Geo {
    Geo { left, width, align: Alignment::Start, bold: false, upper: false, first_indent: 0.0, space_before: 1.0 }
}

/// Screenplay geometry from HIGHLAND_SPEC §5 (Letter, 1″ margins, offsets from 1″).
fn screenplay_geo(el: El) -> Geo {
    match el {
        El::SceneHeading => Geo { upper: true, ..geo(1.5, 6.0) },
        El::Action => geo(1.5, 6.0),
        El::Cue => Geo { upper: true, ..geo(1.0 + 2.458, 7.5 - 3.458) },
        El::Parenthetical => Geo { space_before: 0.0, ..geo(1.0 + 1.875, 2.931) },
        El::Dialogue => Geo { space_before: 0.0, ..geo(1.0 + 1.486, 3.458) },
        El::Transition => Geo { align: Alignment::End, upper: true, ..geo(1.5, 6.0) },
        El::Chapter | El::Prose => geo(1.5, 6.0),
    }
}

/// Manuscript: 1″ margins, 6.5″ column, double-spaced, 0.5″ first-line indent.
fn manuscript_geo(el: El) -> Geo {
    match el {
        El::Chapter => Geo { align: Alignment::Center, upper: true, space_before: 0.0, ..geo(1.0, 6.5) },
        _ => Geo { first_indent: 36.0, space_before: 0.0, ..geo(1.0, 6.5) },
    }
}

const SCREENPLAY: Template = Template { pitch: 12.0, header: true, geo: screenplay_geo };
const MANUSCRIPT: Template = Template { pitch: 24.0, header: true, geo: manuscript_geo };

// ---------------------------------------------------------------- layout

struct Engine {
    font_cx: FontContext,
    layout_cx: LayoutContext<()>,
}

impl Engine {
    fn new(fonts: &[Vec<u8>]) -> Self {
        let mut collection = Collection::new(CollectionOptions { shared: false, system_fonts: false });
        for data in fonts {
            collection.register_fonts(Blob::new(Arc::new(data.clone())), None);
        }
        Self {
            font_cx: FontContext { collection, source_cache: SourceCache::default() },
            layout_cx: LayoutContext::new(),
        }
    }

    fn layout(&mut self, text: &str, g: &Geo, pitch: f32) -> Layout<()> {
        let mut b = self.layout_cx.ranged_builder(&mut self.font_cx, text, 1.0, true);
        b.push_default(StyleProperty::FontFamily("Courier Prime".into()));
        b.push_default(StyleProperty::FontSize(SIZE));
        b.push_default(StyleProperty::LineHeight(LineHeight::Absolute(pitch)));
        if g.bold {
            b.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
        }
        let mut layout = b.build(text);
        if g.first_indent > 0.0 {
            layout.set_text_indent(g.first_indent, IndentOptions::default());
        }
        layout.break_all_lines(Some(g.width * INCH));
        layout.align(g.align, AlignmentOptions::default());
        layout
    }
}

fn face_of(weight: FontWeight, style: FontStyle) -> Face {
    let bold = weight.value() >= 600.0;
    let italic = !matches!(style, FontStyle::Normal);
    match (bold, italic) {
        (false, false) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (true, true) => 3,
    }
}

/// One laid-out line, positioned relative to its element's left edge.
struct LineOut {
    run: GlyphRun,
}

fn element_lines(engine: &mut Engine, el: El, text: &str, t: &Template) -> (Geo, Vec<LineOut>) {
    let g = (t.geo)(el);
    let text = if g.upper { text.to_uppercase() } else { text.to_string() };
    let layout = engine.layout(&text, &g, t.pitch);
    let mut out = Vec::new();
    for (i, line) in layout.lines().enumerate() {
        // Parley's baseline is measured from the paragraph top; make it per-line.
        let baseline = line.metrics().baseline - i as f32 * t.pitch;
        let mut run_out: Option<GlyphRun> = None;
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(gr) = item else { continue };
            let run = gr.run();
            let attrs = run.font_attrs();
            let face = face_of(attrs.weight, attrs.style);
            let r = run_out.get_or_insert_with(|| GlyphRun {
                face,
                size: run.font_size(),
                origin: (0.0, baseline),
                glyphs: Vec::new(),
                text: text.clone(),
            });
            let mut x = gr.offset();
            for cluster in run.visual_clusters() {
                for gl in cluster.glyphs() {
                    r.glyphs.push((gl.id, x + gl.x, gl.advance, cluster.text_range()));
                    x += gl.advance;
                }
            }
        }
        if let Some(r) = run_out {
            out.push(LineOut { run: r });
        }
    }
    (g, out)
}

/// Very small paginator: fills pages line by line, keeps a cue with the next line,
/// adds a "%p." header from page 2. (MORE)/(CONT'D) and sentence splits are phase 1.
fn paginate(engine: &mut Engine, t: &Template, script: &[(El, String)]) -> Vec<Page> {
    let top = INCH;
    let bottom = PAGE_H - INCH;
    let mut pages = vec![Page::default()];
    let mut y = top;
    let mut first_on_page = true;
    for (el, text) in script {
        let (g, lines) = element_lines(engine, *el, text, t);
        let space = if first_on_page { 0.0 } else { g.space_before * t.pitch };
        // Keep scene headings and cues with at least one following line.
        let need = space + t.pitch * (lines.len().min(if matches!(el, El::Cue | El::SceneHeading) { 3 } else { 1 })) as f32;
        if y + need > bottom {
            pages.push(Page::default());
            y = top;
            first_on_page = true;
        } else {
            y += space;
        }
        for line in lines {
            if y + t.pitch > bottom {
                pages.push(Page::default());
                y = top;
            }
            let mut run = line.run;
            run.origin = (g.left * INCH, y + run.origin.1);
            pages.last_mut().unwrap().ops.push(DrawOp::Glyphs(run));
            y += t.pitch;
            first_on_page = false;
        }
    }
    if t.header {
        for (i, page) in pages.iter_mut().enumerate().skip(1) {
            let label = format!("{}.", i + 1);
            let (_, mut lines) = element_lines(engine, El::Action, &label, &SCREENPLAY);
            let mut run = lines.remove(0).run;
            let w: f32 = run.glyphs.iter().map(|g| g.2).sum();
            run.origin = (PAGE_W - INCH - w, 0.5 * INCH + run.origin.1);
            page.ops.push(DrawOp::Glyphs(run));
        }
    }
    pages
}

// ---------------------------------------------------------------- PDF (krilla)

fn write_pdf(pages: &[Page], fonts: &[Vec<u8>]) -> Vec<u8> {
    use krilla::geom::{PathBuilder, Point};
    use krilla::page::PageSettings;
    use krilla::text::{Font, GlyphId, KrillaGlyph};
    use krilla::Document;

    let kfonts: Vec<Font> = fonts.iter().map(|d| Font::new(d.clone().into(), 0).unwrap()).collect();
    let mut doc = Document::new();
    for page in pages {
        let mut p = doc.start_page_with(PageSettings::from_wh(PAGE_W, PAGE_H).unwrap());
        let mut s = p.surface();
        for op in &page.ops {
            match op {
                DrawOp::Glyphs(r) => {
                    let Some(first_x) = r.glyphs.first().map(|g| g.1) else { continue };
                    // krilla advances glyphs itself; encode our absolute x as offsets.
                    let mut pen = first_x;
                    let glyphs: Vec<KrillaGlyph> = r
                        .glyphs
                        .iter()
                        .map(|(id, x, adv, range)| {
                            let g = KrillaGlyph {
                                glyph_id: GlyphId::new(*id),
                                text_range: range.clone(),
                                x_advance: adv / r.size,
                                x_offset: (x - pen) / r.size,
                                y_offset: 0.0,
                                y_advance: 0.0,
                                location: None,
                            };
                            pen += adv;
                            g
                        })
                        .collect();
                    s.draw_glyphs(
                        Point::from_xy(r.origin.0 + first_x, r.origin.1),
                        &glyphs,
                        kfonts[r.face].clone(),
                        &r.text,
                        r.size,
                        false,
                    );
                }
                DrawOp::Rule(a, b, w) => {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(krilla::geom::Rect::from_xywh(a.0, a.1 - w / 2.0, b.0 - a.0, *w).unwrap());
                    s.draw_path(&pb.finish().unwrap());
                }
            }
        }
        s.finish();
        p.finish();
    }
    doc.finish().unwrap()
}

// ---------------------------------------------------------------- raster (tiny-skia)

struct Pen<'a> {
    pb: &'a mut tiny_skia::PathBuilder,
    ox: f32,
    oy: f32,
}

impl skrifa::outline::OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.ox + x, self.oy - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.ox + x, self.oy - y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.pb.quad_to(self.ox + cx, self.oy - cy, self.ox + x, self.oy - y);
    }
    fn curve_to(&mut self, a: f32, b: f32, c: f32, d: f32, x: f32, y: f32) {
        self.pb.cubic_to(self.ox + a, self.oy - b, self.ox + c, self.oy - d, self.ox + x, self.oy - y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

fn render_png(page: &Page, fonts: &[Vec<u8>], dpi: f32) -> tiny_skia::Pixmap {
    use skrifa::instance::{LocationRef, Size};
    use skrifa::outline::DrawSettings;
    use skrifa::{FontRef, MetadataProvider};

    let scale = dpi / 72.0;
    let mut pm = tiny_skia::Pixmap::new((PAGE_W * scale).round() as u32, (PAGE_H * scale).round() as u32).unwrap();
    pm.fill(tiny_skia::Color::WHITE);
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(tiny_skia::Color::BLACK);
    paint.anti_alias = true;
    let refs: Vec<FontRef> = fonts.iter().map(|d| FontRef::new(d).unwrap()).collect();
    let mut pb = tiny_skia::PathBuilder::new();
    for op in &page.ops {
        match op {
            DrawOp::Glyphs(r) => {
                let outlines = refs[r.face].outline_glyphs();
                let px = r.size * scale;
                for (id, x, _, _) in &r.glyphs {
                    let Some(g) = outlines.get(skrifa::GlyphId::new(*id)) else { continue };
                    let mut pen = Pen { pb: &mut pb, ox: (r.origin.0 + x) * scale, oy: r.origin.1 * scale };
                    let _ = g.draw(DrawSettings::unhinted(Size::new(px), LocationRef::default()), &mut pen);
                }
            }
            DrawOp::Rule(a, b, w) => {
                if let Some(rect) = tiny_skia::Rect::from_xywh(a.0 * scale, (a.1 - w / 2.0) * scale, (b.0 - a.0) * scale, w * scale) {
                    pb.push_rect(rect);
                }
            }
        }
    }
    if let Some(path) = std::mem::take(&mut pb).finish() {
        pm.fill_path(&path, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    }
    pm
}

// ---------------------------------------------------------------- sample text (original)

fn screenplay_sample() -> Vec<(El, String)> {
    use El::*;
    let s = |e, t: &str| (e, t.to_string());
    vec![
        s(SceneHeading, "int. ferry terminal - night"),
        s(Action, "Rain hammers the glass roof. A single vending machine hums in the corner, its light flickering over rows of empty plastic seats. MARA (40s, soaked, a duffel bag over one shoulder) pushes through the doors and checks the departures board. Every line reads CANCELLED."),
        s(Cue, "Mara"),
        s(Dialogue, "You have got to be kidding me."),
        s(Action, "Behind the ticket window, OTIS (70s, cardigan, reading glasses on a chain) doesn't look up from his crossword."),
        s(Cue, "Otis"),
        s(Parenthetical, "(without looking up)"),
        s(Dialogue, "Last boat left at nine. Next one's at six, if the river behaves. It usually doesn't, this time of year, but people keep asking anyway, so I keep the window open."),
        s(Cue, "Mara"),
        s(Dialogue, "I need to be across tonight."),
        s(Action, "Otis finally looks at her. Takes off his glasses. Folds the newspaper."),
        s(Transition, "cut to:"),
    ]
}

fn manuscript_sample() -> Vec<(El, String)> {
    use El::*;
    let s = |e, t: &str| (e, t.to_string());
    vec![
        s(Chapter, "Chapter One"),
        s(Prose, "The river had been rising for three days before anyone in the town thought to measure it. By then the water had already taken the lower road and the two oldest willows, and the ferryman had stopped pretending that the morning crossing would run on time."),
        s(Prose, "Mara arrived on the last bus of the evening with a single bag and a letter she had read so many times the folds had gone soft as cloth. She stood under the awning of the terminal and watched the dark water slide past, quick and silent, carrying branches and a child's red boot and once, impossibly, a whole wooden chair."),
        s(Prose, "\"You'll not get across tonight,\" said the old man behind the ticket window, before she had even asked. He said it kindly, the way a doctor might deliver news that was not, strictly speaking, his fault."),
    ]
}

// ---------------------------------------------------------------- main

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "out".into()));
    std::fs::create_dir_all(&out).unwrap();
    let fonts: Vec<Vec<u8>> = FONT_FILES.iter().map(|f| std::fs::read(format!("{FONT_DIR}/{f}")).unwrap()).collect();
    let mut engine = Engine::new(&fonts);

    for (name, t, sample) in [
        ("screenplay", &SCREENPLAY, screenplay_sample()),
        ("manuscript", &MANUSCRIPT, manuscript_sample()),
    ] {
        let pages = paginate(&mut engine, t, &sample);
        std::fs::write(out.join(format!("{name}.pdf")), write_pdf(&pages, &fonts)).unwrap();
        render_png(&pages[0], &fonts, 144.0).save_png(out.join(format!("{name}.png"))).unwrap();
        println!("{name}: {} page(s)", pages.len());
    }

    // Timing: lay out and paginate a ~120-page screenplay from scratch.
    let one = screenplay_sample();
    let big: Vec<_> = std::iter::repeat_n(one, 215).flatten().collect();
    let mut cold = Engine::new(&fonts);
    let t0 = Instant::now();
    let pages = paginate(&mut cold, &SCREENPLAY, &big);
    let layout_ms = t0.elapsed().as_secs_f64() * 1e3;
    let t1 = Instant::now();
    let pdf = write_pdf(&pages, &fonts);
    let pdf_ms = t1.elapsed().as_secs_f64() * 1e3;
    let t2 = Instant::now();
    let _ = render_png(&pages[pages.len() / 2], &fonts, 144.0);
    let raster_ms = t2.elapsed().as_secs_f64() * 1e3;
    std::fs::write(out.join("big.pdf"), pdf).unwrap();
    println!(
        "big: {} pages; full layout+paginate {layout_ms:.1} ms; pdf {pdf_ms:.1} ms; one page raster @144dpi {raster_ms:.1} ms",
        pages.len()
    );
}
