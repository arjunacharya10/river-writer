//! Scratchpad files, outside the story-tab model.
use crate::{atomic_write, decode};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn sidecar(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("Choose a project folder.".into());
    }
    let path = root.join("_river");
    if path.is_symlink() {
        return Err("Keep _river inside the project, without a symbolic link.".into());
    }
    Ok(path)
}
fn ordinary(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_symlink() {
        return Err("River extras cannot use symbolic links.".into());
    }
    Ok(path)
}
fn raw(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub fn scratch_path(root: &Path) -> Result<PathBuf, String> {
    ordinary(sidecar(root)?.join("scratchpad.md"))
}
#[derive(Serialize)]
pub struct Scratchpad {
    pub text: String,
    pub revision: Option<String>,
}
pub fn scratchpad(root: &Path) -> Result<Scratchpad, String> {
    let revision = raw(&scratch_path(root)?)?;
    let text = decode(revision.as_deref().unwrap_or_default().as_bytes())?.0;
    Ok(Scratchpad { text, revision })
}
pub fn save_scratchpad(
    root: &Path,
    expected: Option<&str>,
    text: &str,
) -> Result<Scratchpad, String> {
    let path = scratch_path(root)?;
    let previous = raw(&path)?;
    if previous.as_deref() != expected {
        return Err(
            "Scratchpad changed outside River. Save a copy of your text, then reload.".into(),
        );
    }
    let (_, ending, bom) = decode(expected.unwrap_or_default().as_bytes())?;
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut bytes = if bom { vec![0xef, 0xbb, 0xbf] } else { vec![] };
    bytes.extend_from_slice(text.replace('\n', &ending).as_bytes());
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    atomic_write(&path, &bytes, expected.map(str::as_bytes))?;
    Ok(Scratchpad {
        text,
        revision: Some(String::from_utf8(bytes).map_err(|e| e.to_string())?),
    })
}
/// An explicit, user-chosen copy never replaces an existing file.
pub fn save_copy(path: &Path, text: &str) -> Result<(), String> {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("md"))
    {
        return Err("Use a .md filename for your scratchpad copy.".into());
    }
    let parent = path
        .parent()
        .ok_or("Choose a folder for the copy.")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let path = parent.join(path.file_name().ok_or("Choose a filename.")?);
    atomic_write(&path, text.as_bytes(), None)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "river-extras-{}-{}",
            std::process::id(),
            crate::TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn scratchpad_preserves_encoding_and_rejects_conflicts() {
        let root = root();
        assert!(scratchpad(&root).unwrap().revision.is_none());
        assert!(!root.join("_river").exists());
        let saved = save_scratchpad(&root, None, "Café 🎬\nnotes").unwrap();
        assert_eq!(scratchpad(&root).unwrap().text, saved.text);
        fs::write(scratch_path(&root).unwrap(), "\u{feff}outside\r\n").unwrap();
        assert!(save_scratchpad(&root, saved.revision.as_deref(), "mine").is_err());
        let loaded = scratchpad(&root).unwrap();
        let saved = save_scratchpad(&root, loaded.revision.as_deref(), "one\ntwo\n").unwrap();
        assert_eq!(saved.revision.as_deref(), Some("\u{feff}one\r\ntwo\r\n"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn explicit_copy_never_replaces_existing_text() {
        let root = root();
        let target = root.join("My scratchpad copy.md");
        save_copy(&target, "My unsaved café notes 🎬").unwrap();
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "My unsaved café notes 🎬"
        );
        assert!(save_copy(&target, "replacement").is_err());
        assert!(save_copy(&root.join("notes.fountain"), "wrong type").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn extras_reject_symlinks() {
        let root = root();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("_river")).unwrap();
        assert!(scratchpad(&root).is_err());
        assert!(save_scratchpad(&root, None, "words").is_err());
        assert!(!outside.join("scratchpad.md").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
