//! UI-independent document model and the one page display list used by all outputs.
use serde::{Deserialize, Serialize};
use std::{ops::Range, path::PathBuf, sync::Arc};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub file: usize,
    pub bytes: Range<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: Option<PathBuf>,
    pub text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Action,
    Scene,
    Transition,
    Centered,
    ForcedLeft,
    Section,
    Synopsis,
    PageBreak,
    Lyric,
    Cue,
    Dialogue,
    Parenthetical,
    Paragraph,
    Blockquote,
    ListItem,
    Rule,
    Note,
    Boneyard,
    Directive,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub highlight: bool,
}
impl Style {
    pub fn union(self, other: Self) -> Self {
        Self {
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            underline: self.underline || other.underline,
            strike: self.strike || other.strike,
            highlight: self.highlight || other.highlight,
        }
    }
    pub fn face(self) -> usize {
        usize::from(self.bold) + 2 * usize::from(self.italic)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
}
/// Each mapping covers one displayed Unicode scalar (which may expand on uppercase).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mapping {
    pub display: Range<usize>,
    pub source: SourceRange,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Element {
    pub kind: Kind,
    pub source: SourceRange,
    pub text: String,
    pub spans: Vec<Span>,
    pub mapping: Vec<Mapping>,
    pub level: u8,
    pub scene_number: Option<String>,
    pub dual: bool,
    pub group: usize,
}
impl Element {
    pub fn plain(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            source: SourceRange::default(),
            text: text.into(),
            spans: vec![],
            mapping: vec![],
            level: 0,
            scene_number: None,
            dual: false,
            group: 0,
        }
    }
    pub fn source_for(&self, r: Range<usize>) -> SourceRange {
        let mut hits = self
            .mapping
            .iter()
            .filter(|m| m.display.start < r.end && m.display.end > r.start);
        if let Some(first) = hits.next() {
            let mut result = first.source.clone();
            for hit in hits {
                result.bytes.end = result.bytes.end.max(hit.source.bytes.end);
            }
            result
        } else {
            self.source.clone()
        }
    }
    pub fn slice(&self, range: Range<usize>) -> Self {
        let mut e = self.clone();
        e.text = self.text[range.clone()].into();
        e.source = self.source_for(range.clone());
        e.spans = self
            .spans
            .iter()
            .filter_map(|s| {
                let a = s.range.start.max(range.start);
                let b = s.range.end.min(range.end);
                (a < b).then(|| Span {
                    range: a - range.start..b - range.start,
                    style: s.style,
                })
            })
            .collect();
        e.mapping = self
            .mapping
            .iter()
            .filter_map(|m| {
                let a = m.display.start.max(range.start);
                let b = m.display.end.min(range.end);
                (a < b).then(|| Mapping {
                    display: a - range.start..b - range.start,
                    source: m.source.clone(),
                })
            })
            .collect();
        e
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TitleField {
    pub key: String,
    pub value: String,
    pub source: SourceRange,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Warning,
    Error,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub source: SourceRange,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Script {
    pub files: Vec<SourceFile>,
    pub title_page: Vec<TitleField>,
    pub elements: Vec<Element>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Script {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub text_range: Range<usize>,
    pub source: Option<SourceRange>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlyphRun {
    pub face: usize,
    pub size: f32,
    pub text: Arc<str>,
    pub glyphs: Vec<Glyph>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum DrawOp {
    Glyphs(GlyphRun),
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [u8; 3],
    },
}
impl DrawOp {
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Self::Glyphs(run) => {
                for g in &mut run.glyphs {
                    g.x += dx;
                    g.y += dy
                }
            }
            Self::Rect { x, y, .. } => {
                *x += dx;
                *y += dy
            }
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub source: Option<SourceRange>,
    pub kind: Kind,
    pub generated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub number: Option<usize>,
    pub ops: Vec<DrawOp>,
    pub lines: Vec<PlacedLine>,
}
impl Page {
    pub fn letter(number: Option<usize>) -> Self {
        Self {
            width: 612.0,
            height: 792.0,
            number,
            ops: vec![],
            lines: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Publication {
    pub pages: Vec<Page>,
    pub diagnostics: Vec<Diagnostic>,
}
