//! Bundled Courier Prime and Parley shaping. No operating-system font discovery.
use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStyle, FontWeight, IndentOptions, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use river_model::*;
use std::{collections::HashMap, ops::Range, sync::Arc};
pub const FONT_SIZE: f32 = 12.0;
pub const FONT_BYTES: [&[u8]; 4] = [
    include_bytes!("../../../fonts/courier-prime/CourierPrime-Regular.ttf"),
    include_bytes!("../../../fonts/courier-prime/CourierPrime-Bold.ttf"),
    include_bytes!("../../../fonts/courier-prime/CourierPrime-Italic.ttf"),
    include_bytes!("../../../fonts/courier-prime/CourierPrime-BoldItalic.ttf"),
];
#[derive(Clone, Debug)]
pub struct Line {
    pub range: Range<usize>,
    pub text: String,
    pub height: f32,
    pub baseline: f32,
    pub width: f32,
    pub ops: Vec<DrawOp>,
}
#[derive(Clone, Copy, Debug)]
pub struct Measure {
    pub width: f32,
    pub pitch: f32,
    pub indent: f32,
    pub align: u8,
    pub base: Style,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    text: String,
    spans: Vec<Span>,
    width: u32,
    pitch: u32,
    indent: u32,
    align: u8,
    base: Style,
}
/// Cache is bounded and stores source-independent shaping. Source ranges are
/// rebound on every result, so identical paragraphs in different files are safe.
pub struct TextEngine {
    fonts: FontContext,
    layout: LayoutContext<Style>,
    cache: HashMap<Key, Vec<Line>>,
    pub hits: usize,
    pub misses: usize,
}
impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}
impl TextEngine {
    pub fn new() -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        for bytes in FONT_BYTES {
            collection.register_fonts(Blob::new(Arc::new(bytes.to_vec())), None);
        }
        Self {
            fonts: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            layout: LayoutContext::new(),
            cache: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }
    pub fn advance() -> f32 {
        use skrifa::{
            FontRef, MetadataProvider,
            instance::{LocationRef, Size},
        };
        let font = FontRef::new(FONT_BYTES[0]).expect("bundled font");
        font.glyph_metrics(Size::new(FONT_SIZE), LocationRef::default())
            .advance_width(font.charmap().map('M').unwrap())
            .unwrap()
    }
    pub fn shape(&mut self, e: &Element, m: Measure) -> Result<Vec<Line>, String> {
        if !m.width.is_finite() || m.width <= 0.0 || !m.pitch.is_finite() || m.pitch < 12.0 {
            return Err("Invalid text measure".into());
        }
        let key = Key {
            text: e.text.clone(),
            spans: e.spans.clone(),
            width: m.width.to_bits(),
            pitch: m.pitch.to_bits(),
            indent: m.indent.to_bits(),
            align: m.align,
            base: m.base,
        };
        let mut result = if let Some(lines) = self.cache.get(&key) {
            self.hits += 1;
            lines.clone()
        } else {
            self.misses += 1;
            let lines = self.shape_uncached(e, m)?;
            if self.cache.len() >= 1024 {
                self.cache.clear()
            }
            self.cache.insert(key, lines.clone());
            lines
        };
        for line in &mut result {
            for op in &mut line.ops {
                if let DrawOp::Glyphs(run) = op {
                    for g in &mut run.glyphs {
                        g.source =
                            (!e.mapping.is_empty()).then(|| e.source_for(g.text_range.clone()));
                    }
                }
            }
        }
        Ok(result)
    }
    fn shape_uncached(&mut self, e: &Element, m: Measure) -> Result<Vec<Line>, String> {
        let mut b = self
            .layout
            .ranged_builder(&mut self.fonts, &e.text, 1.0, false);
        b.push_default(StyleProperty::FontFamily("Courier Prime".into()));
        b.push_default(StyleProperty::FontSize(FONT_SIZE));
        b.push_default(StyleProperty::OverflowWrap(parley::OverflowWrap::Anywhere));
        b.push_default(StyleProperty::LineHeight(LineHeight::Absolute(m.pitch)));
        b.push_default(StyleProperty::FontWeight(if m.base.bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        }));
        b.push_default(StyleProperty::FontStyle(if m.base.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        b.push_default(StyleProperty::Brush(m.base));
        for span in &e.spans {
            let st = m.base.union(span.style);
            b.push(
                StyleProperty::FontWeight(if st.bold {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                }),
                span.range.clone(),
            );
            b.push(
                StyleProperty::FontStyle(if st.italic {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                }),
                span.range.clone(),
            );
            b.push(StyleProperty::Brush(st), span.range.clone());
        }
        let mut layout = b.build(&e.text);
        if m.indent > 0.0 {
            layout.set_text_indent(m.indent, IndentOptions::default())
        }
        layout.break_all_lines(Some(m.width));
        layout.align(
            match m.align {
                1 => Alignment::Center,
                2 => Alignment::End,
                _ => Alignment::Start,
            },
            AlignmentOptions::default(),
        );
        let text: Arc<str> = e.text.as_str().into();
        let mut out = vec![];
        let mut top = 0.0;
        for line in layout.lines() {
            let mut ops = vec![];
            let mut cluster_cursors = HashMap::<(usize, u16), usize>::new();
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let run = gr.run();
                let attrs = run.font_attrs();
                let face = usize::from(attrs.weight.value() >= 600.0)
                    + 2 * usize::from(attrs.style != FontStyle::Normal);
                if run.font().data.data() != FONT_BYTES[face] {
                    return Err("Font substitution is not allowed in page output".into());
                }
                let ranges: Vec<_> = run
                    .visual_clusters()
                    .filter(|c| {
                        let r = c.text_range();
                        r.start >= line.text_range().start
                            && r.end <= line.text_range().end
                            && c.style_index() == gr.style_index()
                    })
                    .flat_map(|c| {
                        let r = c.text_range();
                        c.glyphs().map(move |_| r.clone())
                    })
                    .collect();
                let positions: Vec<_> = gr.positioned_glyphs().collect();
                let cursor = cluster_cursors
                    .entry((run.index(), gr.style_index()))
                    .or_default();
                let Some(ranges) = ranges.get(*cursor..*cursor + positions.len()) else {
                    return Err("Shaping cluster/source mapping mismatch".into());
                };
                *cursor += positions.len();
                let mut glyphs = vec![];
                for (g, range) in positions.iter().zip(ranges) {
                    if g.id == 0 {
                        return Err(format!(
                            "Courier Prime cannot render {:?}; unsupported characters cannot be exported silently",
                            &e.text[range.clone()]
                        ));
                    }
                    glyphs.push(Glyph {
                        id: g.id,
                        x: g.x,
                        y: g.y - top,
                        advance: g.advance,
                        text_range: range.clone(),
                        source: None,
                    });
                }
                if let (Some(first), Some(last)) = (glyphs.first(), glyphs.last()) {
                    let st = gr.style().brush;
                    let width = last.x + last.advance - first.x;
                    if st.highlight {
                        ops.push(DrawOp::Rect {
                            x: first.x,
                            y: 0.0,
                            width,
                            height: m.pitch,
                            color: [255, 249, 115],
                        })
                    }
                    if st.underline {
                        ops.push(DrawOp::Rect {
                            x: first.x,
                            y: first.y + 1.8,
                            width,
                            height: 0.6,
                            color: [0, 0, 0],
                        })
                    }
                    if st.strike {
                        ops.push(DrawOp::Rect {
                            x: first.x,
                            y: first.y - 3.5,
                            width,
                            height: 0.6,
                            color: [0, 0, 0],
                        })
                    }
                }
                ops.push(DrawOp::Glyphs(GlyphRun {
                    face,
                    size: FONT_SIZE,
                    text: text.clone(),
                    glyphs,
                }));
            }
            out.push(Line {
                range: line.text_range(),
                text: e.text[line.text_range()]
                    .trim_end_matches(['\r', '\n'])
                    .into(),
                height: m.pitch,
                baseline: line.metrics().baseline - top,
                width: line.metrics().advance,
                ops,
            });
            top += m.pitch;
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_courier_metrics() {
        assert!((TextEngine::advance() - 7.1953125).abs() < 0.00001)
    }
    #[test]
    fn style_runs_and_cache_rebind_source() {
        let mut engine = TextEngine::new();
        let mut e = Element::plain(Kind::Action, "one bold café");
        e.spans.push(Span {
            range: 4..8,
            style: Style {
                bold: true,
                ..Style::default()
            },
        });
        e.mapping = vec![Mapping {
            display: 0..e.text.len(),
            source: SourceRange {
                file: 0,
                bytes: 10..24,
            },
        }];
        let m = Measure {
            width: 432.0,
            pitch: 12.0,
            indent: 0.0,
            align: 0,
            base: Style::default(),
        };
        let first = engine.shape(&e, m).unwrap();
        assert!(
            first[0]
                .ops
                .iter()
                .any(|op| matches!(op,DrawOp::Glyphs(r) if r.face==1))
        );
        e.mapping[0].source.file = 1;
        let second = engine.shape(&e, m).unwrap();
        assert_eq!(engine.hits, 1);
        for op in &second[0].ops {
            if let DrawOp::Glyphs(run) = op {
                assert!(
                    run.glyphs
                        .iter()
                        .all(|g| g.source.as_ref().unwrap().file == 1)
                )
            }
        }
    }
    #[test]
    fn unsupported_glyph_is_error() {
        let mut engine = TextEngine::new();
        assert!(
            engine
                .shape(
                    &Element::plain(Kind::Action, "🛶"),
                    Measure {
                        width: 432.0,
                        pitch: 12.0,
                        indent: 0.0,
                        align: 0,
                        base: Style::default()
                    }
                )
                .is_err()
        )
    }
}
