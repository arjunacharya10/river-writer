//! Desktop orchestration without a Tauri dependency. Editor text uses LF/UTF-8.
pub use river_vcs::project as git;
pub mod commands;
pub mod extras;
pub mod projects;
pub mod recovery;
pub mod todos;
use river_model::{Kind, Publication, Script};
use river_templates::{Template, TemplateId};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Serialize)]
pub struct Document {
    pub id: u64,
    pub path: Option<PathBuf>,
    pub text: String,
    pub revision: u64,
    pub template: TemplateId,
    pub dirty: bool,
    #[serde(skip)]
    disk: Option<Vec<u8>>,
    #[serde(skip)]
    ending: String,
    #[serde(skip)]
    bom: bool,
    #[serde(skip)]
    template_override: bool,
    #[serde(skip)]
    state_root: Option<PathBuf>,
}
#[derive(Default)]
pub struct Documents {
    next: u64,
    docs: HashMap<u64, Document>,
    project: Option<PathBuf>,
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn decode(bytes: &[u8]) -> Result<(String, String, bool), String> {
    let raw = std::str::from_utf8(bytes)
        .map_err(|_| "River opens UTF-8 text files. Convert this file to UTF-8 first.")?;
    let bom = raw.starts_with('\u{feff}');
    let ending = if raw.contains("\r\n") {
        "\r\n"
    } else if raw.contains('\r') {
        "\r"
    } else {
        "\n"
    };
    Ok((
        raw.trim_start_matches('\u{feff}')
            .replace("\r\n", "\n")
            .replace('\r', "\n"),
        ending.into(),
        bom,
    ))
}
fn story(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("fountain"))
}
fn template_for(path: &Path, root: &Path) -> TemplateId {
    let state = root.join("_river/river.json");
    fs::read(state)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| {
            v.get("templates")?
                .get(path.strip_prefix(root).ok()?.to_str()?)
                .and_then(|v| serde_json::from_value(v.clone()).ok())
        })
        .unwrap_or_else(|| TemplateId::default_for(path))
}
impl Documents {
    pub fn autosave(&mut self, id: u64, revision: u64) -> Result<Option<Document>, String> {
        let doc = self.get(id)?;
        if doc.revision != revision || !doc.dirty || doc.path.is_none() {
            return Ok(None);
        }
        self.save(id, None).map(Some)
    }
    pub fn project_dir(&self) -> Option<&Path> {
        self.project.as_deref()
    }

    pub fn set_project(&mut self, path: &Path) -> Result<(), String> {
        let root = path.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("Choose a project folder".into());
        }
        for doc in self.docs.values_mut() {
            if doc.path.as_ref().is_some_and(|p| p.starts_with(&root)) {
                doc.state_root = Some(root.clone());
            }
        }
        self.project = Some(root);
        Ok(())
    }
    pub fn new_document(&mut self) -> Document {
        self.next += 1;
        let doc = Document {
            id: self.next,
            path: None,
            text: String::new(),
            revision: 0,
            template: TemplateId::Manuscript,
            dirty: false,
            disk: None,
            ending: "\n".into(),
            bom: false,
            template_override: false,
            state_root: None,
        };
        self.docs.insert(doc.id, doc.clone());
        doc
    }
    pub fn open(&mut self, path: &Path) -> Result<Document, String> {
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        if !story(&path) {
            return Err("Choose a .fountain or .md story file".into());
        }
        if let Some(doc) = self.docs.values().find(|d| d.path.as_ref() == Some(&path)) {
            return Ok(doc.clone());
        }
        let bytes = read(&path)?;
        let (text, ending, bom) = decode(&bytes)?;
        self.next += 1;
        let root = self
            .project
            .as_ref()
            .filter(|root| path.starts_with(root))
            .cloned()
            .unwrap_or_else(|| path.parent().unwrap().to_path_buf());
        let template = template_for(&path, &root);
        let doc = Document {
            id: self.next,
            template,
            template_override: template != TemplateId::default_for(&path),
            state_root: Some(root),
            path: Some(path),
            text,
            revision: 0,
            dirty: false,
            disk: Some(bytes),
            ending,
            bom,
        };
        self.docs.insert(doc.id, doc.clone());
        Ok(doc)
    }
    pub fn get(&self, id: u64) -> Result<&Document, String> {
        self.docs
            .get(&id)
            .ok_or_else(|| "Document is closed".into())
    }
    /// Strictly increasing revisions prevent a delayed IPC request from reverting text.
    pub fn update(
        &mut self,
        id: u64,
        revision: u64,
        text: String,
        template: TemplateId,
    ) -> Result<Document, String> {
        let doc = self.docs.get_mut(&id).ok_or("Document is closed")?;
        if revision <= doc.revision {
            return Err("Stale document revision".into());
        }
        doc.text = text;
        if doc.template != template {
            doc.template_override = true;
        }
        doc.template = template;
        doc.revision = revision;
        doc.dirty = doc
            .disk
            .as_ref()
            .map(|b| decode(b).map(|v| v.0 != doc.text).unwrap_or(true))
            .unwrap_or(!doc.text.is_empty());
        Ok(doc.clone())
    }
    pub fn buffers(&self) -> HashMap<PathBuf, String> {
        self.docs
            .values()
            .filter_map(|d| d.path.clone().map(|p| (p, d.text.clone())))
            .collect()
    }
    pub fn external_change(&self, id: u64) -> Result<bool, String> {
        let d = self.get(id)?;
        Ok(d.path
            .as_ref()
            .is_some_and(|p| fs::read(p).ok().as_ref() != d.disk.as_ref()))
    }
    pub fn reload(&mut self, id: u64) -> Result<Document, String> {
        let d = self.docs.get_mut(&id).ok_or("Document is closed")?;
        let bytes = read(d.path.as_ref().ok_or("Save this file first")?)?;
        let (text, ending, bom) = decode(&bytes)?;
        d.text = text;
        d.ending = ending;
        d.bom = bom;
        d.disk = Some(bytes);
        d.dirty = false;
        d.revision += 1;
        Ok(d.clone())
    }
    pub fn close(&mut self, id: u64) {
        self.docs.remove(&id);
    }
    pub fn save(&mut self, id: u64, target: Option<&Path>) -> Result<Document, String> {
        let d = self.get(id)?.clone();
        let requested = target
            .or(d.path.as_deref())
            .ok_or("Choose where to save this file")?;
        if !story(requested) {
            return Err("Use a .md or .fountain extension".into());
        }
        let parent = requested
            .parent()
            .ok_or("Choose a containing folder")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let path = if requested.exists() {
            requested.canonicalize().map_err(|e| e.to_string())?
        } else {
            parent.join(requested.file_name().ok_or("Missing file name")?)
        };
        if self
            .docs
            .values()
            .any(|other| other.id != id && other.path.as_ref() == Some(&path))
        {
            return Err("This file is already open in another tab".into());
        }
        let expected = if d.path.as_ref() == Some(&path) {
            if fs::read(&path).ok().as_ref() != d.disk.as_ref() {
                return Err(
                    "File changed outside River. Reload it or Save As a different file.".into(),
                );
            }
            d.disk.as_deref()
        } else {
            // Save As never silently replaces another story, even after a native dialog.
            if path.exists() {
                return Err("That file already exists. Choose a new name.".into());
            }
            None
        };
        let mut bytes = if d.bom {
            vec![0xef, 0xbb, 0xbf]
        } else {
            vec![]
        };
        bytes.extend_from_slice(d.text.replace('\n', &d.ending).as_bytes());
        atomic_write(&path, &bytes, expected)?;
        let doc = self.docs.get_mut(&id).unwrap();
        if !doc.template_override {
            doc.template = TemplateId::default_for(&path);
        }
        if doc
            .state_root
            .as_ref()
            .is_none_or(|root| !path.starts_with(root))
        {
            doc.state_root = Some(
                self.project
                    .as_ref()
                    .filter(|root| path.starts_with(root))
                    .cloned()
                    .unwrap_or_else(|| path.parent().unwrap().to_path_buf()),
            );
        }
        doc.path = Some(path);
        doc.disk = Some(bytes);
        doc.dirty = false;
        Ok(doc.clone())
    }
    pub fn persist_saved_template(&self, id: u64) -> Result<(), String> {
        if self.get(id)?.template_override {
            self.persist_template(id)?;
        }
        Ok(())
    }
    pub fn persist_template(&self, id: u64) -> Result<(), String> {
        let d = self.get(id)?;
        let Some(path) = &d.path else { return Ok(()) };
        let root = d
            .state_root
            .as_deref()
            .unwrap_or_else(|| path.parent().unwrap());
        let dir = root.join("_river");
        let state = dir.join("river.json");
        let old = match fs::read(&state) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let mut value = match &old {
            Some(b) => serde_json::from_slice::<serde_json::Value>(b)
                .map_err(|e| format!("Cannot read river.json: {e}"))?,
            None => serde_json::json!({}),
        };
        if !value.is_object() {
            return Err("river.json must contain an object".into());
        }
        if value.get("templates").is_none() {
            value["templates"] = serde_json::json!({});
        }
        let templates = value["templates"]
            .as_object_mut()
            .ok_or("river.json templates must be an object")?;
        templates.insert(
            path.strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned(),
            serde_json::to_value(d.template).unwrap(),
        );
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        atomic_write(
            &state,
            &serde_json::to_vec_pretty(&value).unwrap(),
            old.as_deref(),
        )
    }
}
static TEMP: AtomicU64 = AtomicU64::new(0);
/// Same-directory replacement preserves Unix permissions and rejects changed contents.
pub fn atomic_write(path: &Path, bytes: &[u8], expected: Option<&[u8]>) -> Result<(), String> {
    let temp = path.with_file_name(format!(
        ".river-{}-{}.tmp",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        if let Ok(meta) = fs::metadata(path) {
            f.set_permissions(meta.permissions())
                .map_err(|e| e.to_string())?
        }
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        if let Some(expected) = expected {
            if read(path)? != expected {
                return Err("File changed before saving. Save As a different file.".into());
            }
            fs::rename(&temp, path).map_err(|e| e.to_string())?;
        } else {
            fs::hard_link(&temp, path)
                .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}
#[derive(Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
}
pub fn list_files(folder: &Path) -> Result<Vec<FileEntry>, String> {
    fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<FileEntry>) -> Result<(), String> {
        if depth > 16 || out.len() > 5000 {
            return Err("This folder is too large. Open a smaller story folder.".into());
        }
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            let path = entry.path();
            if kind.is_dir()
                && !entry.file_name().to_string_lossy().starts_with('.')
                && entry.file_name() != "_river"
                && entry.file_name() != "node_modules"
            {
                walk(root, &path, depth + 1, out)?
            } else if kind.is_file() && story(&path) {
                out.push(FileEntry {
                    name: path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    path,
                })
            }
        }
        Ok(())
    }
    let mut out = vec![];
    walk(folder, folder, 0, &mut out)?;
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
/// Create an empty story file at the top of the project folder, as the Explorer's
/// New File does. A name without `.md`/`.fountain` gets `.md`. Never replaces a file.
pub fn create_story_file(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Type a name for the new file.".into());
    }
    if name.contains(['/', '\\', ':', '\0']) {
        return Err("Use a file name without slashes or colons.".into());
    }
    if name.starts_with('.') {
        return Err("File names can't start with a dot.".into());
    }
    let mut file = folder.join(name);
    if !story(&file) {
        file = folder.join(format!("{name}.md"));
    }
    if file.file_name().is_some_and(|n| n.len() > 255) {
        return Err("That name is too long.".into());
    }
    let shown = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
    {
        Ok(_) => Ok(file),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(format!("A file named {shown} already exists."))
        }
        Err(e) => Err(format!("Could not create {shown}: {e}")),
    }
}
/// Editor positions are UTF-16 code units in the document text, as CodeMirror uses.
#[derive(Serialize)]
pub struct OutlineItem {
    pub text: String,
    pub kind: Kind,
    pub at: u32,
}
#[derive(Serialize)]
pub struct Problem {
    pub severity: river_model::Severity,
    pub message: String,
    pub at: u32,
}
/// A styled stretch of source text (bold, italic, ...), in UTF-16 positions.
#[derive(Serialize)]
pub struct Mark {
    pub from: u32,
    pub to: u32,
    pub style: river_model::Style,
}
/// What the editor draws for one element. Only cues and scene headings carry
/// text, for completions; everything else is positions and styles.
#[derive(Serialize)]
pub struct EditorElement {
    pub kind: Kind,
    pub from: u32,
    pub to: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
}
#[derive(Serialize)]
pub struct Analysis {
    pub dirty: bool,
    pub revision: u64,
    pub elements: Vec<EditorElement>,
    pub outline: Vec<OutlineItem>,
    pub diagnostics: Vec<Problem>,
    pub words: usize,
    pub scenes: usize,
    pub geometry: Template,
}
/// UTF-8 byte offset to UTF-16 position, for every byte boundary in `text`.
fn utf16_positions(text: &str) -> Vec<u32> {
    let mut table = vec![0u32; text.len() + 1];
    let mut utf16 = 0u32;
    for (byte, ch) in text.char_indices() {
        table[byte..byte + ch.len_utf8()].fill(utf16);
        utf16 += ch.len_utf16() as u32;
    }
    table[text.len()] = utf16;
    table
}
pub fn analyze(doc: &Document) -> Analysis {
    let script = river_parse::parse(
        &doc.text,
        river_parse::Format::for_path(doc.path.as_deref().unwrap_or(Path::new("Untitled.md"))),
    );
    let table = utf16_positions(&doc.text);
    let at = |byte: usize| table[byte.min(doc.text.len())];
    let mut elements = Vec::with_capacity(script.elements.len());
    let (mut words, mut scenes, mut outline) = (0, 0, Vec::new());
    for e in &script.elements {
        if e.kind == Kind::Scene {
            scenes += 1;
        }
        if matches!(e.kind, Kind::Scene | Kind::Section | Kind::Synopsis) {
            outline.push(OutlineItem {
                text: e.text.clone(),
                kind: e.kind,
                at: at(e.source.bytes.start),
            });
        }
        if !matches!(
            e.kind,
            Kind::Note | Kind::Boneyard | Kind::Directive | Kind::Synopsis | Kind::PageBreak
        ) {
            words += e.text.split_whitespace().count();
        }
        if e.source.file != 0 {
            continue;
        }
        // A styled display span covers the source from its first to its last
        // mapped character, so hidden markup between them stays inside the mark.
        let marks = e
            .spans
            .iter()
            .filter(|span| span.style != river_model::Style::default())
            .filter_map(|span| {
                let mut hits = e.mapping.iter().filter(|m| {
                    m.source.file == 0
                        && m.display.start < span.range.end
                        && m.display.end > span.range.start
                });
                let first = hits.next()?;
                let last = hits.next_back().unwrap_or(first);
                let (from, to) = (at(first.source.bytes.start), at(last.source.bytes.end));
                (from < to).then_some(Mark {
                    from,
                    to,
                    style: span.style,
                })
            })
            .collect();
        elements.push(EditorElement {
            kind: e.kind,
            from: at(e.source.bytes.start),
            to: at(e.source.bytes.end),
            text: matches!(e.kind, Kind::Cue | Kind::Scene).then(|| e.text.clone()),
            marks,
        });
    }
    let diagnostics = script
        .diagnostics
        .into_iter()
        .filter(|d| d.source.file == 0)
        .map(|d| Problem {
            severity: d.severity,
            message: d.message,
            at: at(d.source.bytes.start),
        })
        .collect();
    Analysis {
        dirty: doc.dirty,
        revision: doc.revision,
        elements,
        outline,
        diagnostics,
        words,
        scenes,
        geometry: Template::get(doc.template).clone(),
    }
}
pub fn script(doc: &Document, buffers: &HashMap<PathBuf, String>) -> Result<Script, String> {
    let s = if let Some(path) = &doc.path {
        river_parse::parse_file_with(path, buffers)?
    } else {
        river_parse::parse(&doc.text, river_parse::Format::Markdown)
    };
    if s.has_errors() {
        return Err(s
            .diagnostics
            .iter()
            .filter(|d| d.severity == river_model::Severity::Error)
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join("\n"));
    }
    Ok(s)
}
pub fn publication(
    engine: &mut river_text::TextEngine,
    doc: &Document,
    buffers: &HashMap<PathBuf, String>,
) -> Result<Publication, String> {
    river_paginate::paginate(engine, &script(doc, buffers)?, doc.template)
}

/// The Initialize button: same as opening a project (#62). Only the chosen
/// folder is used; a repository in a parent folder is never adopted.
pub fn initialize_git(root: &Path) -> Result<(), String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    git::ensure_repository(&root).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn autosave_preserves_encoding_and_rejects_external_edits() {
        let dir = dir();
        let path = dir.join("story.md");
        fs::write(&path, b"\xef\xbb\xbffirst\r\n").unwrap();
        let mut docs = Documents::default();
        docs.set_project(&dir).unwrap();
        let doc = docs.open(&path).unwrap();
        docs.update(doc.id, 1, "second\n".into(), doc.template)
            .unwrap();
        assert!(docs.autosave(doc.id, 0).unwrap().is_none());
        let saved = docs.autosave(doc.id, 1).unwrap().unwrap();
        assert!(!saved.dirty);
        assert_eq!(fs::read(&path).unwrap(), b"\xef\xbb\xbfsecond\r\n");
        docs.update(doc.id, 2, "my latest unsaved text".into(), doc.template)
            .unwrap();
        fs::write(&path, "external text").unwrap();
        assert!(docs.autosave(doc.id, 2).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external text");
        assert_eq!(docs.get(doc.id).unwrap().text, "my latest unsaved text");
        let untitled = docs.new_document();
        docs.update(untitled.id, 1, "untitled".into(), untitled.template)
            .unwrap();
        assert!(docs.autosave(untitled.id, 1).unwrap().is_none());
        fs::remove_dir_all(dir).unwrap();
    }
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "river-service-{}-{}",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn new_files_get_a_story_extension_and_never_replace_existing() {
        let dir = dir();
        let made = create_story_file(&dir, "  Act One ").unwrap();
        assert_eq!(made, dir.join("Act One.md"));
        assert_eq!(fs::read(&made).unwrap(), b"");
        assert_eq!(
            create_story_file(&dir, "Pilot.FOUNTAIN").unwrap(),
            dir.join("Pilot.FOUNTAIN")
        );
        assert_eq!(
            create_story_file(&dir, "notes.txt").unwrap(),
            dir.join("notes.txt.md")
        );
        fs::write(dir.join("Keep.md"), "words").unwrap();
        assert!(
            create_story_file(&dir, "Keep")
                .unwrap_err()
                .contains("already exists")
        );
        assert_eq!(fs::read_to_string(dir.join("Keep.md")).unwrap(), "words");
        for bad in ["", "   ", "acts/One", "../Escape", ".hidden", "a:b"] {
            assert!(create_story_file(&dir, bad).is_err(), "{bad}");
        }
        assert!(!dir.parent().unwrap().join("Escape.md").exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn save_preserves_encoding_and_rejects_external_changes() {
        let dir = dir();
        let p = dir.join("story.fountain");
        let original = b"\xef\xbb\xbfINT. CAFE - DAY\r\n\r\nCaf\xc3\xa9.\r\n";
        fs::write(&p, original).unwrap();
        let mut docs = Documents::default();
        let d = docs.open(&p).unwrap();
        assert!(!d.text.contains('\r'));
        assert!(!d.text.starts_with('\u{feff}'));
        docs.update(d.id, 1, format!("{}More.\n", d.text), d.template)
            .unwrap();
        docs.save(d.id, None).unwrap();
        let saved = read(&p).unwrap();
        assert!(saved.starts_with(original));
        assert!(saved.ends_with(b"More.\r\n"));
        fs::write(&p, b"Another app").unwrap();
        assert!(docs.external_change(d.id).unwrap());
        assert!(docs.save(d.id, None).is_err());
        assert_eq!(read(&p).unwrap(), b"Another app");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn revisions_and_save_as_protect_open_files() {
        let dir = dir();
        let p = dir.join("a.md");
        fs::write(&p, "old").unwrap();
        let mut docs = Documents::default();
        let a = docs.open(&p).unwrap();
        let b = docs.new_document();
        docs.update(b.id, 2, "new".into(), b.template).unwrap();
        assert!(docs.update(b.id, 1, "stale".into(), b.template).is_err());
        assert!(docs.save(b.id, Some(&p)).is_err());
        assert_eq!(docs.get(a.id).unwrap().text, "old");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unsaved_includes_and_template_state() {
        let dir = dir();
        let root = dir.join("root.fountain");
        let child = dir.join("child.fountain");
        fs::write(&root, "{{INCLUDE: child.fountain}}\n").unwrap();
        fs::write(&child, "!Old text.\n").unwrap();
        let mut docs = Documents::default();
        let a = docs.open(&root).unwrap();
        let b = docs.open(&child).unwrap();
        docs.update(b.id, 1, "!Unsaved café.\n".into(), b.template)
            .unwrap();
        let s = script(&a, &docs.buffers()).unwrap();
        assert!(s.elements.iter().any(|e| e.text.contains("Unsaved café")));
        docs.update(a.id, 1, a.text.clone(), TemplateId::Stageplay)
            .unwrap();
        docs.persist_template(a.id).unwrap();
        docs.close(a.id);
        assert_eq!(docs.open(&root).unwrap().template, TemplateId::Stageplay);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn new_file_defaults_and_project_relative_template_state() {
        let dir = dir();
        let sub = dir.join("chapters");
        fs::create_dir(&sub).unwrap();
        let mut docs = Documents::default();
        docs.set_project(&dir).unwrap();
        let a = docs.new_document();
        docs.update(a.id, 1, "INT. ROOM - DAY\n".into(), a.template)
            .unwrap();
        let path = sub.join("new.fountain");
        let saved = docs.save(a.id, Some(&path)).unwrap();
        assert_eq!(saved.template, TemplateId::Screenplay);
        docs.persist_saved_template(a.id).unwrap();
        assert!(!dir.join("_river").exists());
        docs.update(a.id, 2, saved.text.clone(), TemplateId::Stageplay)
            .unwrap();
        docs.persist_template(a.id).unwrap();
        let state: serde_json::Value =
            serde_json::from_slice(&read(&dir.join("_river/river.json")).unwrap()).unwrap();
        assert_eq!(state["templates"]["chapters/new.fountain"], "stageplay");
        docs.close(a.id);
        let reopened = docs.open(&path).unwrap();
        assert_eq!(reopened.template, TemplateId::Stageplay);
        docs.update(reopened.id, 1, "Changed".into(), reopened.template)
            .unwrap();
        let restored = docs
            .update(reopened.id, 2, reopened.text, reopened.template)
            .unwrap();
        assert!(!restored.dirty);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn explorer_excludes_extras_and_symlinks() {
        let dir = dir();
        fs::write(dir.join("a.md"), "story").unwrap();
        fs::create_dir(dir.join("_river")).unwrap();
        fs::write(dir.join("_river/scratchpad.md"), "extra").unwrap();
        std::os::unix::fs::symlink(dir.join("a.md"), dir.join("alias.md")).unwrap();
        assert_eq!(list_files(&dir).unwrap().len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn analysis_uses_editor_positions_and_stays_small() {
        let mut docs = Documents::default();
        let doc = docs.new_document();
        let text = "INT. CAFÉ 🎬 - NIGHT\n\nShe **runs**.\n\nMAYA\nHi.\n";
        let doc = docs
            .update(doc.id, 1, text.into(), TemplateId::Screenplay)
            .unwrap();
        // Untitled documents parse as Markdown; marks still map through hidden `**`.
        let a = analyze(&doc);
        let utf16: Vec<u16> = text.encode_utf16().collect();
        for e in &a.elements {
            assert!(e.from <= e.to && (e.to as usize) <= utf16.len());
            for m in &e.marks {
                assert!(e.from <= m.from && m.to <= e.to);
            }
        }
        let bold = a
            .elements
            .iter()
            .flat_map(|e| &e.marks)
            .find(|m| m.style.bold)
            .unwrap();
        let slice = String::from_utf16(&utf16[bold.from as usize..bold.to as usize]).unwrap();
        assert_eq!(slice, "runs");
        let json = serde_json::to_string(&a).unwrap();
        assert!(!json.contains("mapping"));
        assert_eq!(utf16_positions("é🎬x"), vec![0, 0, 1, 1, 1, 1, 3, 4]);
    }
}
