//! Plain Markdown tasks. Line identities are valid only for the supplied snapshot.
use crate::atomic_write;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize)]
pub struct Task {
    pub id: usize,
    pub text: String,
    pub completed: bool,
}
#[derive(Serialize)]
pub struct Todos {
    pub revision: Option<String>,
    pub tasks: Vec<Task>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Change {
    Add { text: String },
    Rename { id: usize, text: String },
    Toggle { id: usize },
    Delete { id: usize },
    Move { id: usize, before: Option<usize> },
}
#[derive(Clone)]
struct Line {
    body: String,
    ending: String,
}
struct Text {
    lines: Vec<Line>,
    bom: bool,
    ending: String,
}
impl Text {
    fn parse(raw: &str) -> Self {
        let bom = raw.starts_with('\u{feff}');
        let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
        let mut lines = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < raw.len() {
            let byte = raw.as_bytes()[i];
            if byte == b'\r' || byte == b'\n' {
                let end = i + if byte == b'\r' && raw.as_bytes().get(i + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                lines.push(Line {
                    body: raw[start..i].into(),
                    ending: raw[i..end].into(),
                });
                i = end;
                start = end;
            } else {
                i += 1;
            }
        }
        if start < raw.len() {
            lines.push(Line {
                body: raw[start..].into(),
                ending: String::new(),
            });
        }
        let ending = lines
            .iter()
            .find(|l| !l.ending.is_empty())
            .map(|l| l.ending.clone())
            .unwrap_or("\n".into());
        Self { lines, bom, ending }
    }
    fn tasks(&self) -> Vec<Task> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(id, line)| {
                let text = line.body.trim_start();
                let bytes = text.as_bytes();
                if bytes.len() < 5
                    || !matches!(bytes[0], b'-' | b'*')
                    || &bytes[1..3] != b" ["
                    || !matches!(bytes[3], b' ' | b'x' | b'X')
                    || bytes[4] != b']'
                    || bytes.get(5).is_some_and(|b| !b.is_ascii_whitespace())
                {
                    return None;
                }
                Some(Task {
                    id,
                    text: text[5..].trim().into(),
                    completed: bytes[3] != b' ',
                })
            })
            .collect()
    }
    fn encode(&self) -> String {
        let mut raw = if self.bom {
            "\u{feff}".into()
        } else {
            String::new()
        };
        for line in &self.lines {
            raw.push_str(&line.body);
            raw.push_str(&line.ending);
        }
        raw
    }
    fn change(&mut self, change: Change) -> Result<(), String> {
        fn title(text: String) -> Result<String, String> {
            if text.contains(['\r', '\n']) || text.trim().is_empty() {
                return Err("Enter a task on one line.".into());
            }
            Ok(text.trim().to_owned())
        }
        let tasks = self.tasks();
        let task = |id| {
            tasks
                .iter()
                .find(|t| t.id == id)
                .ok_or("That task no longer exists. Reload the list.")
        };
        match change {
            Change::Add { text } => {
                let text = title(text)?;
                if let Some(last) = self.lines.last_mut().filter(|l| l.ending.is_empty()) {
                    last.ending = self.ending.clone();
                }
                self.lines.push(Line {
                    body: format!("- [ ] {text}"),
                    ending: self.ending.clone(),
                });
            }
            Change::Rename { id, text } => {
                task(id)?;
                let text = title(text)?;
                let line = &mut self.lines[id];
                let indent = line.body.len() - line.body.trim_start().len();
                line.body = format!("{} {text}", &line.body[..indent + 5]);
            }
            Change::Toggle { id } => {
                let completed = task(id)?.completed;
                let line = &mut self.lines[id];
                let offset = line.body.len() - line.body.trim_start().len() + 3;
                line.body
                    .replace_range(offset..offset + 1, if completed { " " } else { "x" });
            }
            Change::Delete { id } => {
                task(id)?;
                self.lines.remove(id);
            }
            Change::Move { id, before } => {
                task(id)?;
                if before == Some(id) {
                    return Ok(());
                }
                if let Some(before) = before {
                    task(before)?;
                }
                let mut order: Vec<_> = tasks.iter().map(|t| t.id).collect();
                order.retain(|other| *other != id);
                let position = before
                    .and_then(|before| order.iter().position(|i| *i == before))
                    .unwrap_or(order.len());
                order.insert(position, id);
                let bodies: Vec<_> = order
                    .iter()
                    .map(|id| self.lines[*id].body.clone())
                    .collect();
                // Keep non-task Markdown and each physical line's newline intact.
                for (task, body) in tasks.iter().zip(bodies) {
                    self.lines[task.id].body = body;
                }
            }
        }
        Ok(())
    }
}
fn location(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("Choose a project folder for tasks.".into());
    }
    let sidecar = root.join("_river");
    let path = sidecar.join("todos.md");
    if sidecar.is_symlink() || path.is_symlink() {
        return Err(
            "Todos cannot use a symbolic link. Keep _river/todos.md in the project folder.".into(),
        );
    }
    Ok(path)
}
pub fn load(root: &Path) -> Result<Todos, String> {
    let path = location(root)?;
    let revision = match fs::read_to_string(path) {
        Ok(raw) => Some(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("Could not read tasks: {e}")),
    };
    let tasks = Text::parse(revision.as_deref().unwrap_or_default()).tasks();
    Ok(Todos { revision, tasks })
}
pub fn update(root: &Path, expected: Option<&str>, change: Change) -> Result<Todos, String> {
    let snapshot = load(root)?;
    if snapshot.revision.as_deref() != expected {
        return Err("Tasks changed outside River. Reload the list before making changes.".into());
    }
    let mut text = Text::parse(expected.unwrap_or_default());
    text.change(change)?;
    let raw = text.encode();
    let path = location(root)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    atomic_write(&path, raw.as_bytes(), expected.map(str::as_bytes))?;
    Ok(Todos {
        tasks: text.tasks(),
        revision: Some(raw),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "river-todos-{}-{}",
            std::process::id(),
            crate::TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn adds_edits_completes_reorders_and_reopens_plain_text() {
        let root = root();
        let mut list = load(&root).unwrap();
        assert!(!root.join("_river").exists());
        for text in ["Fix café scene 🎬", "Rewrite ending"] {
            list = update(
                &root,
                list.revision.as_deref(),
                Change::Add { text: text.into() },
            )
            .unwrap();
        }
        list = update(
            &root,
            list.revision.as_deref(),
            Change::Rename {
                id: 0,
                text: "Fix opening".into(),
            },
        )
        .unwrap();
        list = update(
            &root,
            list.revision.as_deref(),
            Change::Move {
                id: 1,
                before: Some(0),
            },
        )
        .unwrap();
        assert_eq!(list.tasks[0].text, "Rewrite ending");
        list = update(&root, list.revision.as_deref(), Change::Toggle { id: 0 }).unwrap();
        assert!(list.tasks[0].completed);
        assert!(load(&root).unwrap().tasks[0].completed);
        list = update(&root, list.revision.as_deref(), Change::Toggle { id: 0 }).unwrap();
        list = update(&root, list.revision.as_deref(), Change::Delete { id: 1 }).unwrap();
        assert_eq!(list.revision.unwrap(), "- [ ] Rewrite ending\n");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn preserves_manual_markdown_bom_and_newlines_and_rejects_stale_writes() {
        let root = root();
        fs::create_dir(root.join("_river")).unwrap();
        let path = root.join("_river/todos.md");
        let raw = "\u{feff}# My tasks\r\n\r\n* [X] Café\r\nKeep this note.\r\n- [ ] Ending";
        fs::write(&path, raw).unwrap();
        let list = load(&root).unwrap();
        assert_eq!(list.tasks.len(), 2);
        let newer = update(&root, list.revision.as_deref(), Change::Toggle { id: 2 }).unwrap();
        assert_eq!(
            newer.revision.as_deref(),
            Some(raw.replace("[X]", "[ ]").as_str())
        );
        assert!(update(&root, list.revision.as_deref(), Change::Delete { id: 4 }).is_err());
        fs::write(&path, "- [ ] External change\n").unwrap();
        assert!(
            update(
                &root,
                newer.revision.as_deref(),
                Change::Add {
                    text: "Mine".into()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(path).unwrap(), "- [ ] External change\n");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_invalid_tasks_and_symlink_sidecars() {
        let root = root();
        assert!(update(&root, None, Change::Add { text: "\n".into() }).is_err());
        assert!(!root.join("_river").exists());
        #[cfg(unix)]
        {
            let outside = root.join("outside");
            fs::create_dir(&outside).unwrap();
            std::os::unix::fs::symlink(&outside, root.join("_river")).unwrap();
            assert!(load(&root).is_err());
            assert!(update(&root, None, Change::Add { text: "No".into() }).is_err());
            assert!(!outside.join("todos.md").exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
