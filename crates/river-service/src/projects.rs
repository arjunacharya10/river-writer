//! Plain-folder projects, recent locations and UI-only session state.
use crate::{FileEntry, atomic_write, list_files};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Session {
    pub open_files: Vec<String>,
    pub active_file: Option<String>,
    pub cursors: HashMap<String, usize>,
    pub scrolls: HashMap<String, f64>,
    pub sidebar: bool,
    pub side_view: String,
    pub preview: bool,
    pub sidebar_sizes: [f64; 2],
    pub editor_sizes: [f64; 2],
    pub theme: Option<String>,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            open_files: vec![],
            active_file: None,
            cursors: HashMap::new(),
            scrolls: HashMap::new(),
            sidebar: true,
            side_view: "explorer".into(),
            preview: false,
            sidebar_sizes: [0.19, 0.81],
            editor_sizes: [0.5, 0.5],
            theme: None,
        }
    }
}
#[derive(Serialize)]
pub struct Project {
    pub path: PathBuf,
    pub name: String,
    pub files: Vec<FileEntry>,
    pub session: Session,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub path: PathBuf,
    pub name: String,
    pub last_opened: u64,
    #[serde(default)]
    pub available: bool,
}
fn canonical_folder(path: &Path) -> Result<PathBuf, String> {
    let p = path
        .canonicalize()
        .map_err(|_| format!("Project folder is unavailable: {}", path.display()))?;
    if !p.is_dir() {
        return Err("Choose a project folder".into());
    }
    Ok(p)
}
fn relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
/// River saves into the project (stories, `_river/`, Git), so a folder it cannot
/// write to is refused up front rather than failing on every later save.
fn writable(path: &Path) -> Result<(), String> {
    let probe = path.join(format!(".river-write-check-{}", std::process::id()));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(_) => Err(format!(
            "River can't save in “{}”. Choose a folder you can write to, such as one in Documents.",
            path.file_name().unwrap_or_default().to_string_lossy()
        )),
    }
}
pub fn open(path: &Path) -> Result<Project, String> {
    let path = canonical_folder(path)?;
    writable(&path)?;
    let files = list_files(&path)?;
    let state = path.join("_river/river.json");
    let mut session = match fs::read(&state) {
        Ok(bytes) => {
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
            serde_json::from_value::<Session>(
                value
                    .get("session")
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
            )
            .unwrap_or_default()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Session::default(),
        Err(e) => return Err(e.to_string()),
    };
    session
        .open_files
        .retain(|name| relative(name) && files.iter().any(|f| f.path == path.join(name)));
    session.open_files.dedup();
    session.open_files.truncate(100);
    if session
        .active_file
        .as_ref()
        .is_none_or(|name| !session.open_files.contains(name))
    {
        session.active_file = session.open_files.first().cloned()
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Project {
        path,
        name,
        files,
        session,
    })
}
pub fn create(path: &Path) -> Result<Project, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.trim().is_empty() && !n.starts_with('.'))
        .ok_or("Choose a project name")?;
    let parent = canonical_folder(path.parent().ok_or("Choose a project location")?)?;
    let path = parent.join(name);
    fs::create_dir(&path)
        .map_err(|e| format!("Cannot create project: {e}. Choose a new folder name."))?;
    let story = path.join(format!("{name}.md"));
    if let Err(error) = atomic_write(&story, b"", None) {
        let _ = fs::remove_dir(&path);
        return Err(error);
    }
    open(&path)
}
pub fn save_session(path: &Path, mut session: Session) -> Result<(), String> {
    let root = canonical_folder(path)?;
    for sizes in [&mut session.sidebar_sizes, &mut session.editor_sizes] {
        if sizes.iter().any(|size| !size.is_finite() || *size <= 0.0) {
            *sizes = [0.5, 0.5];
        }
        let total = sizes[0] + sizes[1];
        sizes[0] /= total;
        sizes[1] /= total;
    }
    if !matches!(session.theme.as_deref(), None | Some("light" | "dark")) {
        session.theme = None;
    }

    session.open_files.retain(|name| relative(name));
    session.open_files.truncate(100);
    session.cursors.retain(|name, _| relative(name));
    session
        .scrolls
        .retain(|name, value| relative(name) && value.is_finite() && *value >= 0.0);
    if session
        .active_file
        .as_ref()
        .is_some_and(|name| !session.open_files.contains(name))
    {
        session.active_file = None
    }
    let dir = root.join("_river");
    let state = dir.join("river.json");
    let old = match fs::read(&state) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    let mut value = match &old {
        Some(b) => serde_json::from_slice::<serde_json::Value>(b).map_err(|e| e.to_string())?,
        None => serde_json::json!({"version":1}),
    };
    if !value.is_object() {
        return Err("Project UI state must contain an object".into());
    }
    value["session"] = serde_json::to_value(session).map_err(|e| e.to_string())?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    atomic_write(
        &state,
        &serde_json::to_vec_pretty(&value).unwrap(),
        old.as_deref(),
    )
}
pub fn recents(config: &Path) -> Result<Vec<RecentProject>, String> {
    let bytes = match fs::read(config.join("projects.json")) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    let mut items: Vec<RecentProject> =
        serde_json::from_slice(&bytes).map_err(|e| format!("Cannot read recent projects: {e}"))?;
    items.sort_by_key(|p| std::cmp::Reverse(p.last_opened));
    items.truncate(12);
    for item in &mut items {
        item.available = item.path.is_dir()
    }
    Ok(items)
}
/// Remove one location from Recent Projects. The folder itself is untouched.
pub fn forget(config: &Path, path: &Path) -> Result<(), String> {
    let state = config.join("projects.json");
    let Ok(old) = fs::read(&state) else {
        return Ok(());
    };
    let mut items = recents(config)?;
    items.retain(|p| p.path != path);
    atomic_write(
        &state,
        &serde_json::to_vec_pretty(&items).unwrap(),
        Some(&old),
    )
}
pub fn remember(config: &Path, project: &Project) -> Result<(), String> {
    let state = config.join("projects.json");
    let old = fs::read(&state).ok();
    let mut items = recents(config)?;
    items.retain(|p| p.path != project.path);
    items.insert(
        0,
        RecentProject {
            path: project.path.clone(),
            name: project.name.clone(),
            last_opened: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
            available: true,
        },
    );
    items.truncate(12);
    fs::create_dir_all(config).map_err(|e| e.to_string())?;
    atomic_write(
        &state,
        &serde_json::to_vec_pretty(&items).unwrap(),
        old.as_deref(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "river-project-{}-{}",
            std::process::id(),
            crate::TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn creation_is_plain_and_never_replaces_existing() {
        let dir = dir();
        let path = dir.join("The River");
        let p = create(&path).unwrap();
        assert_eq!(p.files.len(), 1);
        assert_eq!(fs::read_to_string(path.join("The River.md")).unwrap(), "");
        assert!(!path.join("_river").exists());
        assert!(create(&path).is_err());
        assert!(create(&dir.join("missing/Story")).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn restore_filters_missing_and_escaping_paths_preserves_templates() {
        let dir = dir();
        let p = create(&dir.join("Story")).unwrap();
        fs::create_dir(p.path.join("_river")).unwrap();
        fs::write(
            p.path.join("_river/river.json"),
            r#"{"templates":{"Story.md":"stageplay"},"unknown":true}"#,
        )
        .unwrap();
        let session = Session {
            open_files: vec![
                "Story.md".into(),
                "../outside.md".into(),
                "missing.md".into(),
            ],
            active_file: Some("Story.md".into()),
            preview: true,
            ..Default::default()
        };
        save_session(&p.path, session).unwrap();
        let restored = open(&p.path).unwrap();
        assert_eq!(restored.session.open_files, vec!["Story.md"]);
        assert!(restored.session.preview);
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(p.path.join("_river/river.json")).unwrap()).unwrap();
        assert_eq!(value["templates"]["Story.md"], "stageplay");
        assert_eq!(value["unknown"], true);
        assert_eq!(fs::read(p.path.join("Story.md")).unwrap(), b"");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn recents_deduplicate_and_retain_missing_projects() {
        let dir = dir();
        let p = create(&dir.join("Story")).unwrap();
        let config = dir.join("config");
        remember(&config, &p).unwrap();
        remember(&config, &p).unwrap();
        assert_eq!(recents(&config).unwrap().len(), 1);
        fs::remove_dir(&p.path).unwrap_err();
        fs::remove_file(p.path.join("Story.md")).unwrap();
        fs::remove_dir(&p.path).unwrap();
        let r = recents(&config).unwrap();
        assert_eq!(r.len(), 1);
        assert!(!r[0].available);
        assert!(open(&p.path).is_err());
        forget(&config, &p.path).unwrap();
        assert!(recents(&config).unwrap().is_empty());
        forget(&dir.join("no-config"), &p.path).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn folders_river_cannot_write_to_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = dir();
        let locked = dir.join("Locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("Story.md"), "words").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let refused = open(&locked);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refused.err().unwrap().contains("can't save"));
        let p = open(&locked).unwrap();
        assert_eq!(p.files.len(), 1);
        assert!(
            fs::read_dir(&locked).unwrap().count() == 1,
            "probe file left behind"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
