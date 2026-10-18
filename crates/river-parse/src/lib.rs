//! Fountain state machine, a CommonMark whitelist, inline styles, and safe includes.
use river_model::*;
use std::{
    ops::Range,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Fountain,
    Markdown,
}
impl Format {
    pub fn for_path(p: &Path) -> Self {
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            Self::Markdown
        } else {
            Self::Fountain
        }
    }
}
fn sr(r: Range<usize>) -> SourceRange {
    SourceRange { file: 0, bytes: r }
}
fn diag(script: &mut Script, r: Range<usize>, message: impl Into<String>) {
    script.diagnostics.push(Diagnostic {
        severity: Severity::Error,
        message: message.into(),
        source: sr(r),
    })
}
fn escaped(text: &str, index: usize) -> bool {
    text[..index]
        .bytes()
        .rev()
        .take_while(|b| *b == b'\\')
        .count()
        % 2
        == 1
}
fn trim_range(s: &str, r: Range<usize>) -> Range<usize> {
    let t = &s[r.clone()];
    let start = r.start + t.len() - t.trim_start().len();
    start..start + t.trim().len()
}
fn push(e: &mut Element, text: &str, source: Range<usize>, style: Style) {
    let start = e.text.len();
    e.text.push_str(text);
    let end = e.text.len();
    if start == end {
        return;
    }
    if let Some(last) = e
        .spans
        .last_mut()
        .filter(|last| last.style == style && last.range.end == start)
    {
        last.range.end = end
    } else {
        e.spans.push(Span {
            range: start..end,
            style,
        })
    }
    e.mapping.push(Mapping {
        display: start..end,
        source: sr(source),
    });
}
// Consume combined closing runs (*** can close nested ** and *) without
// treating the opening run of a nested span as the outer span's closing marker.
fn closing_delimiter(raw: &str, token: &str) -> Option<usize> {
    let marker = token.as_bytes()[0];
    let mut stack = vec![token.len()];
    let mut i = token.len();
    while i < raw.len() {
        let tail = &raw[i..];
        if tail.starts_with('\\') {
            i += 1;
            if let Some(c) = raw[i..].chars().next() {
                i += c.len_utf8()
            }
            continue;
        }
        if tail.starts_with("[[") || tail.starts_with("/*") {
            let close = if tail.starts_with("[[") { "]]" } else { "*/" };
            if let Some(end) = tail[2..].find(close) {
                i += end + 4;
                continue;
            }
        }
        if raw.as_bytes()[i] != marker {
            i += tail.chars().next().unwrap().len_utf8();
            continue;
        }
        let count = tail.bytes().take_while(|b| *b == marker).count();
        let mut remaining = count;
        let mut used = 0;
        let can_close = raw[..i]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace());
        if can_close {
            while let Some(&needed) = stack.last() {
                if remaining < needed {
                    break;
                }
                remaining -= needed;
                stack.pop();
                if stack.is_empty() {
                    return Some(i + used);
                }
                used += needed;
            }
        }
        if remaining > 0
            && raw[i + count..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace())
        {
            if stack.len() >= 64 {
                return None;
            }
            stack.push(remaining);
        }
        i += count;
    }
    None
}
fn inline(raw: &str, base: usize, e: &mut Element, style: Style) {
    let mut i = 0;
    while i < raw.len() {
        let tail = &raw[i..];
        if tail.starts_with('\\')
            && let Some(ch) = tail[1..].chars().next()
            && ch.is_ascii_punctuation()
        {
            let n = 1 + ch.len_utf8();
            push(e, &ch.to_string(), base + i..base + i + n, style);
            i += n;
            continue;
        }
        if tail.starts_with("[[") || tail.starts_with("/*") {
            let close = if tail.starts_with("[[") { "]]" } else { "*/" };
            if let Some(end) = tail[2..].find(close) {
                i += end + 4;
                continue;
            }
        }
        let mut matched = false;
        for token in ["***", "**", "~~", "*", "_", "+"] {
            if !tail.starts_with(token) {
                continue;
            }
            let n = token.len();
            let close = closing_delimiter(tail, token);
            if let Some(end) = close.filter(|end| *end > n) {
                let extra = match token {
                    "***" => Style {
                        bold: true,
                        italic: true,
                        ..Style::default()
                    },
                    "**" => Style {
                        bold: true,
                        ..Style::default()
                    },
                    "*" => Style {
                        italic: true,
                        ..Style::default()
                    },
                    "_" => Style {
                        underline: true,
                        ..Style::default()
                    },
                    "~~" => Style {
                        strike: true,
                        ..Style::default()
                    },
                    _ => Style {
                        highlight: true,
                        ..Style::default()
                    },
                };
                inline(&tail[n..end], base + i + n, e, style.union(extra));
                i += end + n;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }
        let ch = tail.chars().next().unwrap();
        push(
            e,
            &ch.to_string(),
            base + i..base + i + ch.len_utf8(),
            style,
        );
        i += ch.len_utf8();
    }
}
fn element(source: &str, kind: Kind, range: Range<usize>, group: usize) -> Element {
    let mut e = Element::plain(kind, "");
    e.source = sr(range.clone());
    e.group = group;
    inline(
        &source[range.clone()],
        range.start,
        &mut e,
        Style::default(),
    );
    e
}
/// Mask hidden comments without moving any byte offsets, including UTF-8 and CRLF.
fn mask(source: &str, notes: bool, script: &mut Script) -> String {
    let mut bytes = source.as_bytes().to_vec();
    let mut i = 0;
    while i < source.len() {
        let tail = &source[i..];
        let close = if tail.starts_with("/*") && !escaped(source, i) {
            Some("*/")
        } else if notes && tail.starts_with("[[") && !escaped(source, i) {
            Some("]]")
        } else {
            None
        };
        if let Some(close) = close {
            let end = if let Some(at) = tail[2..].find(close) {
                i + at + 4
            } else {
                diag(script, i..source.len(), format!("Unclosed {}", &tail[..2]));
                source.len()
            };
            for b in &mut bytes[i..end] {
                if !matches!(*b, b'\r' | b'\n') {
                    *b = b' '
                }
            }
            i = end;
        } else {
            i += tail.chars().next().unwrap().len_utf8()
        }
    }
    String::from_utf8(bytes).expect("masked UTF-8 remains valid")
}
fn source_lines(s: &str) -> Vec<Range<usize>> {
    let mut offset = 0;
    s.split_inclusive('\n')
        .map(|line| {
            let r = offset..offset + line.trim_end_matches(['\r', '\n']).len();
            offset += line.len();
            r
        })
        .collect()
}
fn title_page(source: &str, lines: &[Range<usize>], script: &mut Script) -> usize {
    let mut i = 0;
    let known = [
        "title",
        "credit",
        "credits",
        "author",
        "authors",
        "source",
        "sources",
        "contact",
        "contact info",
        "contactinfo",
        "draft date",
        "date",
        "revision",
        "revisions",
    ];
    while i < lines.len() {
        let r = lines[i].clone();
        let text = &source[r.clone()];
        if text.trim().is_empty() {
            return if i == 0 { 0 } else { i + 1 };
        }
        if (text.starts_with(' ') || text.starts_with('\t')) && !script.title_page.is_empty() {
            let last = script.title_page.last_mut().unwrap();
            last.value.push('\n');
            last.value.push_str(text.trim());
            last.source.bytes.end = r.end;
            i += 1;
            continue;
        }
        let Some((key, value)) = text.split_once(':') else {
            break;
        };
        if !known.contains(&key.trim().to_lowercase().as_str()) {
            break;
        }
        script.title_page.push(TitleField {
            key: key.trim().to_lowercase(),
            value: value.trim().into(),
            source: sr(r),
        });
        i += 1;
    }
    i
}
fn scene(s: &str) -> bool {
    let s = s.to_uppercase();
    [
        "INT.",
        "EXT.",
        "EST.",
        "I./E.",
        "E./I.",
        "INT/EXT.",
        "INT./EXT.",
    ]
    .iter()
    .any(|p| s.starts_with(p))
}
fn transition(s: &str) -> bool {
    s.starts_with('>') || s.ends_with("TO:") || s.ends_with("FADE OUT.")
}
fn classify(s: &str) -> Kind {
    if s.starts_with('~') && !s.starts_with("~~") {
        Kind::Lyric
    } else if s.starts_with('.') && !s.starts_with("..") && !s.starts_with(". ") {
        Kind::Scene
    } else if s.starts_with('!') {
        Kind::Action
    } else if s.starts_with('>') && s.ends_with('<') {
        Kind::Centered
    } else if s.ends_with('>') && !escaped(s, s.len() - 1) {
        Kind::Transition
    } else if s.starts_with('<') {
        Kind::ForcedLeft
    } else if transition(s) {
        Kind::Transition
    } else if s.starts_with('#') {
        Kind::Section
    } else if s.len() >= 3 && s.chars().all(|c| c == '=') {
        Kind::PageBreak
    } else if s.starts_with('=') {
        Kind::Synopsis
    } else if scene(s) {
        Kind::Scene
    } else if s.starts_with("{{") && s.ends_with("}}") {
        Kind::Directive
    } else if s.starts_with("[[") && s.ends_with("]]") {
        Kind::Note
    } else {
        Kind::Action
    }
}
fn is_cue(s: &str) -> bool {
    if s.starts_with('@') {
        return true;
    }
    if matches!(
        classify(s),
        Kind::Scene
            | Kind::Transition
            | Kind::Centered
            | Kind::ForcedLeft
            | Kind::Section
            | Kind::Synopsis
            | Kind::Lyric
            | Kind::PageBreak
            | Kind::Directive
            | Kind::Note
    ) || s.starts_with(['!', '.'])
        || s.contains("::")
    {
        return false;
    }
    let name = s.split(['(', '^']).next().unwrap_or(s);
    name.chars().any(char::is_alphabetic) && !name.chars().any(char::is_lowercase)
}
fn fountain(source: &str, script: &mut Script) {
    let masked = mask(source, false, script);
    let lines = source_lines(&masked);
    let mut i = title_page(source, &lines, script);
    let mut group = 0;
    while i < lines.len() {
        if masked[lines[i].clone()].trim().is_empty() {
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && !masked[lines[i].clone()].trim().is_empty() {
            i += 1
        }
        let end = i;
        group += 1;
        let first = masked[lines[start].clone()].trim();
        if end - start > 1 && is_cue(first) {
            for (n, line_range) in lines.iter().enumerate().take(end).skip(start) {
                let mut r = trim_range(source, line_range.clone());
                let raw = &source[r.clone()];
                let kind = if n == start {
                    Kind::Cue
                } else if raw.starts_with('!') {
                    Kind::Action
                } else if raw.starts_with('~') && !raw.starts_with("~~") {
                    Kind::Lyric
                } else if raw.starts_with('(') && raw.ends_with(')') {
                    Kind::Parenthetical
                } else {
                    Kind::Dialogue
                };
                let dual = n == start && raw.ends_with('^');
                if n == start && raw.starts_with('@')
                    || n > start && matches!(kind, Kind::Action | Kind::Lyric)
                {
                    r.start += 1
                }
                if dual {
                    r.end -= 1
                }
                r = trim_range(source, r);
                let mut e = element(source, kind, r, group);
                e.dual = dual;
                script.elements.push(e);
            }
        } else {
            let mut r = trim_range(source, lines[start].start..lines[end - 1].end);
            let kind = classify(first);
            let mut level = 0;
            let mut number = None;
            match kind {
                Kind::Section => {
                    level = first.chars().take_while(|c| *c == '#').count().min(255) as u8;
                    r.start += usize::from(level)
                }
                Kind::Synopsis | Kind::Lyric | Kind::ForcedLeft => r.start += 1,
                Kind::Scene => {
                    if first.starts_with('.') && !first.starts_with("..") {
                        r.start += 1
                    }
                    if source[r.clone()].trim_end().ends_with('#') {
                        let t = source[r.clone()].trim_end();
                        if let Some(at) = t[..t.len() - 1].rfind('#') {
                            let n = &t[at + 1..t.len() - 1];
                            if !n.is_empty() && !n.chars().any(char::is_whitespace) {
                                number = Some(n.into());
                                r.end = r.start + at
                            }
                        }
                    }
                }
                Kind::Action => {
                    if first.starts_with('!') {
                        r.start += 1
                    }
                }
                Kind::Centered => {
                    r.start += 1;
                    r.end -= 1
                }
                Kind::Transition => {
                    if source[r.clone()].starts_with('>') {
                        r.start += 1
                    }
                    if source[r.clone()].ends_with('>') {
                        r.end -= 1
                    }
                }
                Kind::Directive => {
                    r.start += 2;
                    r.end -= 2
                }
                _ => {}
            }
            r = trim_range(source, r);
            let mut e = element(source, kind, r, group);
            e.level = level;
            e.scene_number = number;
            if kind == Kind::Note {
                e.text = source[e.source.bytes.clone()].into()
            }
            script.elements.push(e);
        }
    }
    // Inline notes can span paragraphs. Validate them independently of printability.
    let _ = mask(source, true, script);
}
fn markdown(source: &str, script: &mut Script) {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut masked = mask(source, true, script);
    let lines = source_lines(&masked);
    let start = title_page(source, &lines, script);
    if start > 0 {
        let end = if start < lines.len() {
            lines[start].start
        } else {
            source.len()
        };
        let mut b = masked.into_bytes();
        for ch in &mut b[..end] {
            if !matches!(*ch, b'\r' | b'\n') {
                *ch = b' '
            }
        }
        masked = String::from_utf8(b).unwrap()
    }
    let mut current: Option<Element> = None;
    let mut styles = vec![Style::default()];
    let mut quote = 0;
    let mut group = 0;
    let mut skip_image = 0;
    for (event, r) in Parser::new(&masked).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote -= 1,
            Event::Start(Tag::Item) => {
                if current.is_none() {
                    group += 1;
                    let mut e = Element::plain(Kind::ListItem, "");
                    e.source = sr(r.clone());
                    e.group = group;
                    push(
                        &mut e,
                        "- ",
                        r.start..(r.start + 2).min(r.end),
                        Style::default(),
                    );
                    current = Some(e)
                }
            }
            Event::End(TagEnd::Item) => {
                if let Some(e) = current.take() {
                    script.elements.push(e)
                }
            }
            Event::Start(Tag::Paragraph | Tag::Heading { .. }) => {
                if current.is_none() {
                    group += 1;
                    let mut e = Element::plain(
                        if quote > 0 {
                            Kind::Blockquote
                        } else {
                            Kind::Paragraph
                        },
                        "",
                    );
                    e.source = sr(r.clone());
                    e.group = group;
                    current = Some(e)
                }
                if let Event::Start(Tag::Heading { level, .. }) = event {
                    let e = current.as_mut().unwrap();
                    e.kind = Kind::Section;
                    e.level = level as u8;
                }
            }
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                if let Some(mut e) = current.take() {
                    let t = e.text.trim().to_string();
                    if t.starts_with("{{") && t.ends_with("}}") {
                        e.kind = Kind::Directive;
                        e = e.slice(2..e.text.len() - 2)
                    } else if t.len() >= 3 && t.chars().all(|c| c == '=') {
                        e.kind = Kind::PageBreak
                    } else if t.starts_with('=') {
                        e.kind = Kind::Synopsis
                    }
                    if !t.is_empty() {
                        script.elements.push(e)
                    }
                }
            }
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough) => {
                let mut st = *styles.last().unwrap();
                match event {
                    Event::Start(Tag::Strong) => st.bold = true,
                    Event::Start(Tag::Emphasis) => st.italic = true,
                    _ => st.strike = true,
                }
                styles.push(st)
            }
            Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => {
                styles.pop();
            }
            Event::Start(Tag::Image { .. }) => {
                skip_image += 1;
                script.diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Images are not part of River's manuscript subset".into(),
                    source: sr(r),
                })
            }
            Event::End(TagEnd::Image) => skip_image -= 1,
            Event::Text(text) if skip_image == 0 => {
                if let Some(e) = current.as_mut() {
                    let style = *styles.last().unwrap();
                    if masked[r.clone()] == *text {
                        for (at, ch) in text.char_indices() {
                            push(
                                e,
                                &ch.to_string(),
                                r.start + at..r.start + at + ch.len_utf8(),
                                style,
                            )
                        }
                    } else {
                        push(e, &text, r, style)
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(e) = current.as_mut() {
                    push(
                        e,
                        if event == Event::HardBreak { "\n" } else { " " },
                        r,
                        *styles.last().unwrap(),
                    )
                }
            }
            Event::Rule => {
                group += 1;
                script.elements.push(element(source, Kind::Rule, r, group))
            }
            Event::Start(Tag::CodeBlock(_))
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::Code(_) => {
                script.diagnostics.push(Diagnostic{severity:Severity::Warning,message:"Code and HTML are outside River's manuscript subset; literal text retained where possible".into(),source:sr(r.clone())});
                if let Some(e) = current.as_mut() {
                    push(e, &source[r.clone()], r, *styles.last().unwrap())
                } else {
                    group += 1;
                    script
                        .elements
                        .push(element(source, Kind::Paragraph, r, group))
                }
            }
            _ => {}
        }
    }
}
pub fn parse(source: &str, format: Format) -> Script {
    let mut script = Script {
        files: vec![SourceFile {
            path: None,
            text: source.into(),
        }],
        ..Script::default()
    };
    match format {
        Format::Fountain => fountain(source, &mut script),
        Format::Markdown => markdown(source, &mut script),
    }
    script
}
/// Includes are relative to their containing file, constrained to the initial
/// file's directory after canonicalization. No network or arbitrary external reads.
pub fn parse_file(path: &Path) -> Result<Script, String> {
    parse_file_with(path, &std::collections::HashMap::new())
}
/// Parse a saved root with normalized, unsaved editor buffers taking precedence.
/// Keys must be canonical paths; containment and cycle checks still apply.
pub fn parse_file_with(
    path: &Path,
    buffers: &std::collections::HashMap<PathBuf, String>,
) -> Result<Script, String> {
    let root = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let base = root
        .parent()
        .ok_or("Input has no containing directory")?
        .to_path_buf();
    fn load(
        path: &Path,
        base: &Path,
        stack: &mut Vec<PathBuf>,
        buffers: &std::collections::HashMap<PathBuf, String>,
    ) -> Result<Script, String> {
        let path = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if !path.starts_with(base) {
            return Err(format!(
                "Include escapes project directory: {}",
                path.display()
            ));
        }
        if stack.contains(&path) {
            return Err(format!("Include cycle: {}", path.display()));
        }
        if stack.len() >= 32 {
            return Err("Include depth exceeds 32".into());
        }
        stack.push(path.clone());
        let text = match buffers.get(&path) {
            Some(text) => text.clone(),
            None => {
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?
            }
        };
        let mut s = parse(&text, Format::for_path(&path));
        s.files[0].path = Some(path.clone());
        let mut elements = vec![];
        for e in std::mem::take(&mut s.elements) {
            if e.kind == Kind::Directive
                && e.text
                    .split_once(':')
                    .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("include"))
            {
                let (_, target) = e.text.split_once(':').unwrap();
                let target = target.trim().trim_matches('"');
                if target.is_empty() {
                    return Err("Empty include path".into());
                }
                let mut child = load(&path.parent().unwrap().join(target), base, stack, buffers)?;
                let offset = s.files.len();
                for e in &mut child.elements {
                    e.source.file += offset;
                    for m in &mut e.mapping {
                        m.source.file += offset
                    }
                }
                for d in &mut child.diagnostics {
                    d.source.file += offset
                }
                elements.extend(child.elements);
                s.files.extend(child.files);
                s.diagnostics.extend(child.diagnostics);
            } else {
                elements.push(e)
            }
        }
        // A block identity must never collide across include boundaries.
        let mut previous = None;
        let mut group = 0;
        for e in &mut elements {
            let key = (e.source.file, e.group);
            if previous.as_ref() != Some(&key) {
                group += 1;
                previous = Some(key)
            }
            e.group = group
        }
        s.elements = elements;
        stack.pop();
        Ok(s)
    }
    load(&root, &base, &mut vec![], buffers)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fountain_order_and_dialogue() {
        let s = parse(
            ".a room #12#\n\n@McKay ^\n(quietly)\nHello.\n!A bell.\n~A song\n\n> Not all caps TO:\n\n> centered <\n\n===\n",
            Format::Fountain,
        );
        assert_eq!(
            s.elements.iter().map(|e| e.kind).collect::<Vec<_>>(),
            vec![
                Kind::Scene,
                Kind::Cue,
                Kind::Parenthetical,
                Kind::Dialogue,
                Kind::Action,
                Kind::Lyric,
                Kind::Transition,
                Kind::Centered,
                Kind::PageBreak
            ]
        );
        assert_eq!(s.elements[0].scene_number.as_deref(), Some("12"));
        assert!(s.elements[1].dual)
    }
    #[test]
    fn utf8_crlf_inline_and_source() {
        let text = "!Café **bold *inside*** and \\*literal\\* [[secret]].\r\n";
        let s = parse(text, Format::Fountain);
        let e = &s.elements[0];
        assert!(!e.text.contains("secret"));
        assert!(e.text.contains("*literal*"));
        for m in &e.mapping {
            assert!(text.get(m.source.bytes.clone()).is_some());
            assert!(e.text.get(m.display.clone()).is_some())
        }
        assert!(e.spans.iter().any(|s| s.style.bold));
    }
    #[test]
    fn title_and_boneyard() {
        let s = parse(
            "Title: The River\nAuthor: N. Writer\n\n/* hidden\nEXT. SECRET\n*/\n\nEXT. BANK\n\nA boat.\n",
            Format::Fountain,
        );
        assert_eq!(s.title_page.len(), 2);
        assert_eq!(s.elements.len(), 2);
        assert_eq!(s.elements[0].text, "EXT. BANK")
    }
    #[test]
    fn unclosed_note_fails() {
        assert!(parse("Hello [[unfinished", Format::Fountain).has_errors())
    }
    #[test]
    fn lowercase_not_cue_and_uppercase_alone_action() {
        let s = parse("McKay\nHello\n\nALONE\n", Format::Fountain);
        assert!(s.elements.iter().all(|e| e.kind == Kind::Action))
    }
    #[test]
    fn markdown_subset() {
        let s = parse(
            "# Chapter\n\nA **bold** and *quiet* river. [[secret]]\n\n> A quote.\n\n- An item.\n\n---\n",
            Format::Markdown,
        );
        assert!(!s.has_errors());
        assert_eq!(s.elements[0].kind, Kind::Section);
        assert!(!s.elements.iter().any(|e| e.text.contains("secret")));
        assert!(s.elements.iter().any(|e| e.kind == Kind::Blockquote));
        assert!(s.elements.iter().any(|e| e.kind == Kind::ListItem));
        assert!(s.elements.iter().any(|e| e.kind == Kind::Rule))
    }
}

#[cfg(test)]
mod inline_regressions {
    use super::*;
    #[test]
    fn nested_emphasis_and_combined_closing_runs() {
        for source in [
            "!A **bold *inside*** word.",
            "!A *quiet **inside*** word.",
            "!A ***both*** word.",
        ] {
            let script = parse(source, Format::Fountain);
            let e = &script.elements[0];
            assert!(!e.text.contains('*'), "{}", e.text);
            assert!(e.spans.iter().any(|s| s.style.bold && s.style.italic));
        }
    }
}
