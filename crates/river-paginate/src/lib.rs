//! Grouping and pagination. Page output is independent of Tauri and the editor.
use river_layout::{Block, element, split};
use river_model::*;
use river_templates::{Align, Geometry, Template, TemplateId};
use river_text::TextEngine;
use std::collections::VecDeque;
const TOP: f32 = 72.0;
const BOTTOM: f32 = 720.0;
#[derive(Clone)]
struct Column {
    cue: Block,
    body: VecDeque<Block>,
}
impl Column {
    fn height(&self) -> f32 {
        self.cue.height() + self.body.iter().map(Block::height).sum::<f32>()
    }
    fn body_minimum(&self) -> f32 {
        let count = self.body.iter().flat_map(|b| &b.lines).count();
        self.body
            .iter()
            .flat_map(|b| &b.lines)
            .take(if count < 4 { count } else { 2 })
            .map(|l| l.height)
            .sum()
    }
    fn minimum(&self) -> f32 {
        self.cue.height() + self.body_minimum()
    }
}
enum Unit {
    Text(Block),
    Dialogue(Vec<Column>),
    Break,
    Directive(Element),
}
impl Unit {
    fn minimum(&self) -> f32 {
        match self {
            Self::Text(b) => b.lines.iter().take(2).map(|l| l.height).sum(),
            Self::Dialogue(cols) => cols.iter().map(Column::minimum).fold(0.0, f32::max),
            _ => 0.0,
        }
    }
}
fn dual_geometry(t: &Template, kind: Kind, side: usize) -> Geometry {
    let left = 108.0 + 234.0 * side as f32;
    let width = 198.0;
    match kind {
        Kind::Cue => Geometry {
            x: left + 54.0,
            width: width - 54.0,
            align: Align::Left,
        },
        Kind::Parenthetical => Geometry {
            x: left + 24.0,
            width: width - 24.0,
            align: Align::Left,
        },
        _ => Geometry {
            x: left,
            width,
            align: t.geometry(kind).align,
        },
    }
}
fn prepare(engine: &mut TextEngine, script: &Script, t: &Template) -> Result<Vec<Unit>, String> {
    let mut units = vec![];
    let mut i = 0;
    while i < script.elements.len() {
        let e = &script.elements[i];
        if e.kind == Kind::PageBreak {
            units.push(Unit::Break);
            i += 1;
            continue;
        }
        if e.kind == Kind::Directive {
            units.push(Unit::Directive(e.clone()));
            i += 1;
            continue;
        }
        if !t.printable(e.kind) {
            i += 1;
            continue;
        }
        if e.kind == Kind::Cue {
            let cue = element(engine, e, t, None, false)?;
            let mut body = VecDeque::new();
            i += 1;
            while i < script.elements.len()
                && script.elements[i].group == e.group
                && script.elements[i].kind != Kind::Cue
            {
                let item = &script.elements[i];
                if t.id == TemplateId::ScreenplayMulticam
                    && item.kind == Kind::Parenthetical
                    && let Some(next) = script
                        .elements
                        .get(i + 1)
                        .filter(|next| next.group == e.group && next.kind == Kind::Dialogue)
                {
                    let mut merged = item.clone();
                    merged.kind = Kind::Dialogue;
                    merged.text.push(' ');
                    let offset = merged.text.len();
                    merged.text.push_str(&next.text);
                    merged.spans.extend(next.spans.iter().map(|s| Span {
                        range: s.range.start + offset..s.range.end + offset,
                        style: s.style,
                    }));
                    merged.mapping.extend(next.mapping.iter().map(|m| Mapping {
                        display: m.display.start + offset..m.display.end + offset,
                        source: m.source.clone(),
                    }));
                    merged.source.bytes.end = next.source.bytes.end;
                    body.push_back(element(engine, &merged, t, None, false)?);
                    i += 2;
                    continue;
                }
                if t.printable(item.kind) && item.kind != Kind::PageBreak {
                    body.push_back(element(engine, item, t, None, false)?)
                }
                i += 1;
            }
            let col = Column { cue, body };
            if e.dual && matches!(units.last(),Some(Unit::Dialogue(cols)) if cols.len()==1) {
                let Unit::Dialogue(cols) = units.last_mut().unwrap() else {
                    unreachable!()
                };
                cols.push(col);
                for (side, c) in cols.iter_mut().enumerate() {
                    c.cue = element(
                        engine,
                        &c.cue.element,
                        t,
                        Some(dual_geometry(t, Kind::Cue, side)),
                        false,
                    )?;
                    for b in &mut c.body {
                        *b = element(
                            engine,
                            &b.element,
                            t,
                            Some(dual_geometry(t, b.element.kind, side)),
                            false,
                        )?
                    }
                }
            } else {
                units.push(Unit::Dialogue(vec![col]))
            }
        } else {
            units.push(Unit::Text(element(engine, e, t, None, false)?));
            i += 1
        }
    }
    Ok(units)
}
#[derive(Clone)]
struct Header {
    header: String,
    footer: String,
}
struct Paginator<'a> {
    engine: &'a mut TextEngine,
    t: &'a Template,
    pages: Vec<Page>,
    headers: Vec<Header>,
    header: Header,
    y: f32,
    next_number: usize,
}
impl<'a> Paginator<'a> {
    fn new(engine: &'a mut TextEngine, t: &'a Template) -> Self {
        let header = Header {
            header: if t.id == TemplateId::Manuscript {
                "%p".into()
            } else {
                "%p.".into()
            },
            footer: "%none".into(),
        };
        Self {
            engine,
            t,
            pages: vec![Page::letter(Some(1))],
            headers: vec![header.clone()],
            header,
            y: TOP,
            next_number: 2,
        }
    }
    fn cheat(&self) -> f32 {
        if self.header.footer == "%none" || self.header.footer.is_empty() {
            self.t.cheat
        } else {
            0.0
        }
    }
    fn empty(&self) -> bool {
        self.pages.last().unwrap().lines.is_empty()
    }
    fn new_page(&mut self) {
        if self.empty() {
            self.y = TOP;
            return;
        }
        self.pages.push(Page::letter(Some(self.next_number)));
        self.next_number += 1;
        self.headers.push(self.header.clone());
        self.y = TOP;
    }
    fn put(&mut self, b: &Block, y: f32, generated: bool) -> f32 {
        let page = self.pages.last_mut().unwrap();
        let mut at = y;
        for line in &b.lines {
            for mut op in line.ops.clone() {
                op.translate(b.geometry.x, at);
                page.ops.push(op)
            }
            page.lines.push(PlacedLine {
                text: line.text.clone(),
                x: b.geometry.x,
                y: at,
                height: line.height,
                source: (!generated).then(|| b.element.source_for(line.range.clone())),
                kind: b.element.kind,
                generated,
            });
            at += line.height;
        }
        at
    }
    fn label(&mut self, text: &str, kind: Kind, geometry: Geometry) -> Result<Block, String> {
        element(
            self.engine,
            &Element::plain(kind, text),
            self.t,
            Some(geometry),
            true,
        )
    }
    fn text(&mut self, mut block: Block, keep: f32) -> Result<(), String> {
        if block.lines.is_empty() {
            return Ok(());
        }
        let space = if self.empty() { 0.0 } else { block.before };
        if keep > 0.0
            && self.y + space + block.height() + keep > BOTTOM + self.cheat()
            && !self.empty()
        {
            self.new_page()
        }
        if !self.empty() {
            self.y += block.before
        }
        loop {
            if self.y + block.height() <= BOTTOM + self.cheat() {
                self.y = self.put(&block, self.y, false);
                break;
            }
            let mut fit = 0;
            let mut height = 0.0;
            for line in &block.lines {
                if self.y + height + line.height > BOTTOM {
                    break;
                }
                height += line.height;
                fit += 1
            }
            if fit < 2 && !self.empty() {
                self.new_page();
                continue;
            }
            let min_tail = if block.lines.len() > 2 { 2 } else { 1 };
            if let Some((left, right)) = split(self.engine, &block, self.t, fit, min_tail)? {
                self.y = self.put(&left, self.y, false);
                block = right;
                self.new_page();
            } else if !self.empty() {
                self.new_page()
            } else {
                return Err(format!(
                    "Element cannot fit on a page: {:?}",
                    block.element.text.chars().take(60).collect::<String>()
                ));
            }
        }
        Ok(())
    }
    fn dialogue(&mut self, mut cols: Vec<Column>) -> Result<(), String> {
        let full = cols.iter().map(Column::height).fold(0.0, f32::max);
        let space = if self.empty() { 0.0 } else { self.t.pitch };
        if !self.empty()
            && self.y + space + full > BOTTOM + self.cheat()
            && full <= BOTTOM - TOP + self.cheat()
        {
            self.new_page()
        } else if !self.empty() {
            self.y += space
        }
        let mut continuation = false;
        loop {
            let mut cues = vec![];
            for c in &cols {
                if continuation && !c.body.is_empty() {
                    let name = &c.cue.element.text;
                    let name = if name.ends_with("(CONT'D)") {
                        name.clone()
                    } else {
                        format!("{name} (CONT'D)")
                    };
                    cues.push(self.label(&name, Kind::Cue, c.cue.geometry)?);
                } else {
                    cues.push(c.cue.clone())
                }
            }
            let remaining = cols
                .iter()
                .zip(&cues)
                .filter(|(c, _)| !c.body.is_empty())
                .map(|(c, cue)| cue.height() + c.body.iter().map(Block::height).sum::<f32>())
                .fold(0.0, f32::max);
            if self.y + remaining <= BOTTOM + self.cheat() {
                let mut bottom = self.y;
                for (c, cue) in cols.iter().zip(&cues) {
                    if c.body.is_empty() {
                        continue;
                    }
                    let mut y = self.put(cue, self.y, continuation);
                    for b in &c.body {
                        y = self.put(b, y, false)
                    }
                    bottom = bottom.max(y)
                }
                self.y = bottom;
                return Ok(());
            }
            let minimum = cols
                .iter()
                .zip(&cues)
                .filter(|(c, _)| !c.body.is_empty())
                .map(|(c, cue)| cue.height() + c.body_minimum() + self.t.pitch)
                .fold(0.0, f32::max);
            if self.y + minimum > BOTTOM {
                if self.empty() {
                    return Err("Dialogue cue/parenthetical is taller than a printable page".into());
                }
                self.new_page();
                continue;
            }
            let start = self.y;
            let mut bottom = start;
            let mut progress = false;
            let dual = cols.len() == 2;
            for (c, cue) in cols.iter_mut().zip(&cues) {
                if c.body.is_empty() {
                    continue;
                }
                let budget = BOTTOM - start - cue.height() - self.t.pitch;
                let taken = take_body(self.engine, &mut c.body, self.t, budget)?;
                if taken.is_empty() {
                    return Err(
                        "Dialogue cannot be split without an orphan cue/parenthetical".into(),
                    );
                }
                progress = true;
                let mut y = self.put(cue, start, continuation);
                for b in &taken {
                    y = self.put(b, y, false)
                }
                if !c.body.is_empty() {
                    let geometry = if dual {
                        Geometry {
                            x: c.cue.geometry.x - 30.0,
                            width: 174.0,
                            align: Align::Left,
                        }
                    } else {
                        self.t.geometry(Kind::Parenthetical)
                    };
                    let more = self.label("(MORE)", Kind::Parenthetical, geometry)?;
                    y = self.put(&more, y, true);
                }
                bottom = bottom.max(y);
            }
            if !progress {
                return Err("Dialogue pagination made no progress".into());
            }
            self.y = bottom;
            self.new_page();
            continuation = true;
        }
    }
    fn directive(&mut self, e: &Element) -> Result<(), String> {
        let Some((key, value)) = e.text.split_once(':') else {
            return Err(format!("Unsupported directive: {}", e.text));
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "header" => self.header.header = value.trim().into(),
            "footer" => self.header.footer = value.trim().into(),
            "pagenumber" => {
                let n = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "PAGENUMBER needs a positive integer")?;
                if n == 0 || n > 1_000_000 {
                    return Err("PAGENUMBER must be between 1 and 1000000".into());
                }
                if !self.empty() {
                    self.new_page()
                }
                self.pages.last_mut().unwrap().number = Some(n);
                self.next_number = n + 1
            }
            _ => {
                return Err(format!(
                    "Unsupported directive: {} (not silently discarded)",
                    e.text
                ));
            }
        }
        // The directive is its own group and applies from this page onward.
        *self.headers.last_mut().unwrap() = self.header.clone();
        Ok(())
    }
}
fn take_body(
    engine: &mut TextEngine,
    body: &mut VecDeque<Block>,
    t: &Template,
    budget: f32,
) -> Result<Vec<Block>, String> {
    let mut result = vec![];
    let mut used = 0.0;
    while let Some(mut block) = body.pop_front() {
        if block.element.kind == Kind::Parenthetical {
            let next = body
                .front()
                .map_or(0.0, |b| b.lines.iter().take(2).map(|l| l.height).sum());
            if used + block.height() + next > budget {
                body.push_front(block);
                break;
            }
        }
        if used + block.height() <= budget {
            used += block.height();
            result.push(block);
            continue;
        }
        let fit = ((budget - used) / block.pitch).floor().max(0.0) as usize;
        if fit >= 2
            && let Some((left, right)) = split(engine, &block, t, fit, 2)?
        {
            result.push(left);
            block = right
        }
        body.push_front(block);
        break;
    }
    // In dual dialogue, a shorter side may otherwise leave a one-line
    // continuation even though the longer side needs several more pages.
    if body.iter().flat_map(|b| &b.lines).count() == 1
        && let Some(last) = result.pop()
    {
        if last.lines.len() > 1
            && let Some((left, right)) = split(engine, &last, t, last.lines.len() - 1, 1)?
        {
            result.push(left);
            body.push_front(right);
        } else {
            body.push_front(last)
        }
    }
    // Do not strand a cue with only one dialogue line when no other body exists.
    if result.iter().flat_map(|b| &b.lines).count() < 2 && !body.is_empty() {
        for b in result.drain(..).rev() {
            body.push_front(b)
        }
    }
    Ok(result)
}
fn title_page(
    engine: &mut TextEngine,
    script: &Script,
    t: &Template,
) -> Result<Option<Page>, String> {
    if script.title_page.is_empty() {
        return Ok(None);
    }
    let mut page = Page::letter(None);
    let mut y = 240.0;
    let mut bottom_fields = vec![];
    for field in &script.title_page {
        if field.key.starts_with("contact")
            || field.key.contains("date")
            || field.key.starts_with("revision")
        {
            bottom_fields.push(field);
            continue;
        }
        let mut e = Element::plain(Kind::Centered, &field.value);
        e.source = field.source.clone();
        e.mapping = vec![Mapping {
            display: 0..e.text.len(),
            source: field.source.clone(),
        }];
        if field.key == "title" {
            e.spans.push(Span {
                range: 0..e.text.len(),
                style: Style {
                    bold: true,
                    ..Style::default()
                },
            })
        }
        let b = element(
            engine,
            &e,
            t,
            Some(Geometry {
                x: 108.0,
                width: 396.0,
                align: Align::Center,
            }),
            true,
        )?;
        for line in &b.lines {
            for mut op in line.ops.clone() {
                op.translate(108.0, y);
                page.ops.push(op)
            }
            page.lines.push(PlacedLine {
                text: line.text.clone(),
                x: 108.0,
                y,
                height: line.height,
                source: Some(field.source.clone()),
                kind: Kind::Centered,
                generated: false,
            });
            y += line.height
        }
        y += 24.0;
    }
    let mut bottoms = vec![];
    let mut height = 0.0;
    for field in bottom_fields {
        let mut e = Element::plain(Kind::Action, &field.value);
        e.source = field.source.clone();
        e.mapping = vec![Mapping {
            display: 0..e.text.len(),
            source: field.source.clone(),
        }];
        let mut title_t = t.clone();
        title_t.pitch = 12.0;
        let b = element(
            engine,
            &e,
            &title_t,
            Some(Geometry {
                x: 72.0,
                width: 468.0,
                align: Align::Left,
            }),
            true,
        )?;
        height += b.height() + 12.0;
        bottoms.push(b);
    }
    let mut bottom_y = 720.0 - height;
    if y > bottom_y || y > 720.0 {
        return Err("Title-page fields exceed one page; shorten them before export".into());
    }
    for b in bottoms {
        for line in &b.lines {
            for mut op in line.ops.clone() {
                op.translate(72.0, bottom_y);
                page.ops.push(op)
            }
            page.lines.push(PlacedLine {
                text: line.text.clone(),
                x: 72.0,
                y: bottom_y,
                height: line.height,
                source: Some(b.element.source.clone()),
                kind: Kind::Action,
                generated: false,
            });
            bottom_y += line.height
        }
        bottom_y += 12.0
    }
    Ok(Some(page))
}
fn page_labels(
    engine: &mut TextEngine,
    pages: &mut [Page],
    headers: &[Header],
    script: &Script,
    t: &Template,
) -> Result<(), String> {
    let title = script
        .title_page
        .iter()
        .find(|f| f.key == "title")
        .map_or("", |f| f.value.as_str());
    let count = pages.len();
    for (i, page) in pages.iter_mut().enumerate() {
        let number = page.number.unwrap_or(i + 1);
        if i == 0 && !t.number_first && headers[i].header == "%p." && headers[i].footer == "%none" {
            continue;
        }
        for (pattern, y, default_align) in [
            (&headers[i].header, 36.0, Align::Right),
            (&headers[i].footer, 744.0, Align::Center),
        ] {
            if pattern == "%none" || pattern.is_empty() {
                continue;
            }
            let label = pattern
                .replace("%pp", &count.to_string())
                .replace("%p", &number.to_string())
                .replace("%title", title);
            let pieces: Vec<_> = label.split('|').collect();
            if pieces.len() != 1 && pieces.len() != 3 {
                return Err(
                    "Header/footer requires one field or three fields separated by |".into(),
                );
            }
            for (part, text) in pieces.iter().enumerate() {
                if text.trim().is_empty() {
                    continue;
                }
                let geometry = if pieces.len() == 3 {
                    Geometry {
                        x: 72.0 + 156.0 * part as f32,
                        width: 156.0,
                        align: [Align::Left, Align::Center, Align::Right][part],
                    }
                } else {
                    Geometry {
                        x: 72.0,
                        width: 468.0,
                        align: default_align,
                    }
                };
                let mut header_t = t.clone();
                header_t.pitch = 12.0;
                let b = element(
                    engine,
                    &Element::plain(Kind::Action, text.trim()),
                    &header_t,
                    Some(geometry),
                    true,
                )?;
                if b.lines.len() != 1 {
                    return Err("Header/footer must fit on one line".into());
                }
                for mut op in b.lines[0].ops.clone() {
                    op.translate(geometry.x, y);
                    page.ops.push(op)
                }
                page.lines.push(PlacedLine {
                    text: text.trim().into(),
                    x: geometry.x,
                    y,
                    height: 12.0,
                    source: None,
                    kind: Kind::Action,
                    generated: true,
                });
            }
        }
    }
    Ok(())
}
pub fn paginate(
    engine: &mut TextEngine,
    script: &Script,
    id: TemplateId,
) -> Result<Publication, String> {
    if script.has_errors() {
        return Err("Resolve parser errors before exporting".into());
    }
    let t = Template::get(id);
    let units = prepare(engine, script, t)?;
    let title = title_page(engine, script, t)?;
    let mut p = Paginator::new(engine, t);
    for (i, unit) in units.iter().enumerate() {
        match unit {
            Unit::Text(b) => p.text(
                b.clone(),
                if matches!(b.element.kind, Kind::Scene | Kind::Section) {
                    units.get(i + 1).map_or(0.0, Unit::minimum)
                } else {
                    0.0
                },
            )?,
            Unit::Dialogue(cols) => p.dialogue(cols.clone())?,
            Unit::Break => p.new_page(),
            Unit::Directive(e) => p.directive(e)?,
        }
    }
    if p.pages.len() > 1 && p.empty() {
        p.pages.pop();
        p.headers.pop();
    }
    if !units.is_empty() || title.is_none() {
        page_labels(p.engine, &mut p.pages, &p.headers, script, t)?
    } else {
        p.pages.clear()
    }
    if let Some(title) = title {
        p.pages.insert(0, title)
    }
    Ok(Publication {
        pages: p.pages,
        diagnostics: script.diagnostics.clone(),
    })
}
