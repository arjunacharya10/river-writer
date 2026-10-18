//! Element-to-line layout, with source-preserving uppercase and paragraph splits.
use river_model::*;
use river_templates::{Align, Geometry, Template, TemplateId};
use river_text::{Line, Measure, TextEngine};
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Debug)]
pub struct Block {
    pub element: Element,
    pub geometry: Geometry,
    pub lines: Vec<Line>,
    pub pitch: f32,
    pub before: f32,
    pub indent: f32,
}
impl Block {
    pub fn height(&self) -> f32 {
        self.lines.iter().map(|l| l.height).sum()
    }
    pub fn first_height(&self) -> f32 {
        self.lines.first().map_or(0.0, |l| l.height)
    }
}
fn uppercase(e: &Element) -> Element {
    let mut result = e.clone();
    result.text.clear();
    result.spans.clear();
    result.mapping.clear();
    for (at, ch) in e.text.char_indices() {
        let range = at..at + ch.len_utf8();
        let start = result.text.len();
        result.text.extend(ch.to_uppercase());
        let end = result.text.len();
        let style = e
            .spans
            .iter()
            .filter(|s| s.range.start < range.end && s.range.end > range.start)
            .fold(Style::default(), |st, s| st.union(s.style));
        if let Some(last) = result
            .spans
            .last_mut()
            .filter(|last| last.style == style && last.range.end == start)
        {
            last.range.end = end
        } else {
            result.spans.push(Span {
                range: start..end,
                style,
            })
        }
        if !e.mapping.is_empty() {
            result.mapping.push(Mapping {
                display: start..end,
                source: e.source_for(range),
            })
        }
    }
    result
}
pub fn element(
    engine: &mut TextEngine,
    e: &Element,
    t: &Template,
    override_geometry: Option<Geometry>,
    continued: bool,
) -> Result<Block, String> {
    let mut e = if t.uppercase(e.kind) {
        uppercase(e)
    } else {
        e.clone()
    };
    if e.kind == Kind::Rule {
        e.text = "* * *".into();
        e.spans.clear();
        e.mapping.clear()
    }
    let mut geometry = override_geometry.unwrap_or_else(|| t.geometry(e.kind));
    if e.kind == Kind::Rule {
        geometry.align = Align::Center
    }
    let pitch = t.pitch_for(e.kind);
    let indent =
        if !continued && matches!(e.kind, Kind::Paragraph) && t.id == TemplateId::Manuscript {
            t.first_indent
        } else {
            0.0
        };
    let before = if t.id == TemplateId::Manuscript {
        if matches!(e.kind, Kind::Section | Kind::Rule) {
            pitch
        } else {
            0.0
        }
    } else if matches!(e.kind, Kind::Dialogue | Kind::Parenthetical | Kind::Lyric) {
        0.0
    } else {
        t.pitch
    };
    let mut lines = if matches!(e.kind, Kind::PageBreak | Kind::Directive) {
        vec![]
    } else {
        engine.shape(
            &e,
            Measure {
                width: geometry.width,
                pitch,
                indent,
                align: match geometry.align {
                    Align::Left => 0,
                    Align::Center => 1,
                    Align::Right => 2,
                },
                base: t.style(e.kind),
            },
        )?
    };
    if e.kind == Kind::Scene
        && !continued
        && let Some(number) = &e.scene_number
    {
        let labels = engine.shape(
            &Element::plain(Kind::Scene, number),
            Measure {
                width: 43.2,
                pitch,
                indent: 0.0,
                align: 0,
                base: Style::default(),
            },
        )?;
        if labels.len() != 1 {
            return Err("Scene number is too long to fit the margin".into());
        }
        if let Some(first) = lines.first_mut() {
            for x in [54.0, 546.0] {
                for mut op in labels[0].ops.clone() {
                    op.translate(x - geometry.x, 0.0);
                    first.ops.push(op)
                }
            }
        }
    }
    Ok(Block {
        element: e,
        geometry,
        lines,
        pitch,
        before,
        indent,
    })
}
/// Split at the last complete sentence fitting the available line count; if no
/// sentence fits, split at a whole line. A single oversized sentence must progress.
/// Both halves are reshaped; the continuation loses the first-line paragraph indent.
pub fn split(
    engine: &mut TextEngine,
    block: &Block,
    t: &Template,
    fit: usize,
    min_tail: usize,
) -> Result<Option<(Block, Block)>, String> {
    if fit == 0 || fit >= block.lines.len() {
        return Ok(None);
    }
    let fit = fit.min(block.lines.len().saturating_sub(min_tail));
    if fit == 0 {
        return Ok(None);
    }
    let end = block.lines[fit - 1].range.end;
    let mut at = end;
    if t.sentence_breaks
        && let Some(boundary) = block
            .element
            .text
            .split_sentence_bound_indices()
            .map(|(i, s)| i + s.len())
            .filter(|i| *i > 0 && *i <= end)
            .last()
    {
        at = boundary
    }
    fn trimmed_start(text: &str, at: usize) -> usize {
        at + text[at..].len() - text[at..].trim_start().len()
    }
    let mut tail_start = trimmed_start(&block.element.text, at);
    if tail_start >= block.element.text.len() {
        return Ok(None);
    }
    let mut left = element(
        engine,
        &block.element.slice(0..at),
        t,
        Some(block.geometry),
        block.indent == 0.0,
    )?;
    let mut right = element(
        engine,
        &block.element.slice(tail_start..block.element.text.len()),
        t,
        Some(block.geometry),
        true,
    )?;
    // Respect dialogue/widow minimums even when sentence preference found an
    // earlier boundary. Hard line splitting is the necessary fallback.
    if left.lines.len() < 2 && fit >= 2 || right.lines.len() < min_tail {
        at = end;
        tail_start = trimmed_start(&block.element.text, at);
        left = element(
            engine,
            &block.element.slice(0..at),
            t,
            Some(block.geometry),
            block.indent == 0.0,
        )?;
        right = element(
            engine,
            &block.element.slice(tail_start..block.element.text.len()),
            t,
            Some(block.geometry),
            true,
        )?;
    }
    left.before = block.before;
    right.before = 0.0;
    Ok(Some((left, right)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uppercase_expansion_keeps_source() {
        let mut e = Element::plain(Kind::Cue, "Straße");
        e.mapping = e
            .text
            .char_indices()
            .map(|(i, c)| Mapping {
                display: i..i + c.len_utf8(),
                source: SourceRange {
                    file: 0,
                    bytes: i..i + c.len_utf8(),
                },
            })
            .collect();
        let u = uppercase(&e);
        assert_eq!(u.text, "STRASSE");
        assert_eq!(u.source_for(4..6).bytes, 4..6)
    }
    #[test]
    fn manuscript_continuation_has_no_indent() {
        let mut engine = TextEngine::new();
        let t = Template::get(TemplateId::Manuscript);
        let e = Element::plain(
            Kind::Paragraph,
            "A river runs slowly past the old house. ".repeat(30),
        );
        let block = element(&mut engine, &e, t, None, false).unwrap();
        let (_, right) = split(&mut engine, &block, t, 4, 2).unwrap().unwrap();
        assert_eq!(right.indent, 0.0);
        assert!(!right.lines.is_empty())
    }
}
