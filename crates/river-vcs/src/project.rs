//! Standard local Git operations. The private autosave repository is never traversed.
use git2::{DiffFormat, Index, IndexAddOption, Oid, Repository, RepositoryState, Signature};
use serde::Serialize;
use std::{fs, io::Write, path::Path};
fn err(e: impl std::fmt::Display) -> String {
    format!("Git: {e}")
}
fn open(root: &Path) -> Result<Repository, String> {
    if !root.join(".git").exists() {
        return Err("Initialize Git for this project first.".into());
    }
    let repo = Repository::open(root).map_err(err)?;
    if repo.is_bare()
        || repo.workdir().and_then(|p| p.canonicalize().ok())
            != Some(root.canonicalize().map_err(err)?)
    {
        return Err("Choose the root of a working Git project.".into());
    }
    Ok(repo)
}
#[derive(Serialize)]
pub struct Identity {
    pub name: String,
    pub email: String,
}
#[derive(Serialize)]
pub struct Change {
    pub path: String,
    pub kind: String,
}
#[derive(Serialize)]
pub struct Review {
    pub files: Vec<Change>,
    pub patch: String,
    pub truncated: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub initialized: bool,
    pub branch: String,
    pub head: Option<String>,
    pub tree: String,
    pub identity: Option<Identity>,
    pub blocked: Option<String>,
    pub review: Review,
}
#[derive(Serialize)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}
fn head(repo: &Repository) -> Result<Option<git2::Commit<'_>>, String> {
    match repo.head() {
        Ok(h) => h.peel_to_commit().map(Some).map_err(err),
        Err(e)
            if e.code() == git2::ErrorCode::UnbornBranch
                || e.code() == git2::ErrorCode::NotFound =>
        {
            Ok(None)
        }
        Err(e) => Err(err(e)),
    }
}
fn candidate(repo: &Repository, index: &mut Index) -> Result<Oid, String> {
    repo.set_index(index).map_err(err)?;
    if let Some(head) = head(repo)? {
        index.read_tree(&head.tree().map_err(err)?).map_err(err)?;
    }
    index.update_all(["*"], None).map_err(err)?;
    index
        .add_all(["*"], IndexAddOption::DEFAULT, None)
        .map_err(err)?;
    // UI state is never published, including repositories that previously tracked it.
    if index.get_path(Path::new("_river/river.json"), 0).is_some() {
        index
            .remove_path(Path::new("_river/river.json"))
            .map_err(err)?;
    }
    index.write_tree_to(repo).map_err(err)
}
fn review(
    repo: &Repository,
    old: Option<&git2::Tree<'_>>,
    new: &git2::Tree<'_>,
) -> Result<Review, String> {
    let diff = repo.diff_tree_to_tree(old, Some(new), None).map_err(err)?;
    let files = diff
        .deltas()
        .map(|d| Change {
            path: d
                .new_file()
                .path()
                .or(d.old_file().path())
                .unwrap_or(Path::new("?"))
                .to_string_lossy()
                .into_owned(),
            kind: format!("{:?}", d.status()),
        })
        .collect();
    let mut patch = Vec::new();
    let mut truncated = false;
    diff.print(DiffFormat::Patch, |_, _, line| {
        if patch.len() + line.content().len() + 1 > 1_000_000 {
            truncated = true;
            return true;
        }
        if matches!(line.origin(), '+' | '-' | ' ') {
            patch.push(line.origin() as u8);
        }
        patch.extend_from_slice(line.content());
        true
    })
    .map_err(err)?;
    Ok(Review {
        files,
        patch: String::from_utf8_lossy(&patch).into_owned(),
        truncated,
    })
}
pub fn status(root: &Path) -> Result<Status, String> {
    if !root.join(".git").exists() {
        return Ok(Status {
            initialized: false,
            branch: String::new(),
            head: None,
            tree: String::new(),
            identity: None,
            blocked: None,
            review: Review {
                files: vec![],
                patch: String::new(),
                truncated: false,
            },
        });
    }
    let repo = open(root)?;
    let blocked = if repo.state() != RepositoryState::Clean
        || repo.index().map_err(err)?.has_conflicts()
    {
        Some("Resolve the current Git operation or conflicts before committing in River.".into())
    } else if repo.head_detached().map_err(err)? {
        Some("Check out a branch in Git before committing in River.".into())
    } else {
        None
    };
    let parent = head(&repo)?;
    let old = parent.as_ref().map(|h| h.tree()).transpose().map_err(err)?;
    let mut index = Index::new().map_err(err)?;
    let tree = candidate(&repo, &mut index)?;
    let identity = repo.signature().ok().map(|s| Identity {
        name: s.name().unwrap_or_default().into(),
        email: s.email().unwrap_or_default().into(),
    });
    let branch = repo
        .find_reference("HEAD")
        .ok()
        .and_then(|h| {
            h.symbolic_target()
                .map(|s| s.trim_start_matches("refs/heads/").to_owned())
        })
        .unwrap_or_else(|| "Detached HEAD".into());
    Ok(Status {
        initialized: true,
        branch,
        head: parent.as_ref().map(|h| h.id().to_string()),
        tree: tree.to_string(),
        identity,
        blocked,
        review: review(&repo, old.as_ref(), &repo.find_tree(tree).map_err(err)?)?,
    })
}
pub fn init(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err("Open a project folder first.".into());
    }
    if root.join(".git").exists() {
        open(root)?;
        return Ok(());
    }
    let mut options = git2::RepositoryInitOptions::new();
    options.initial_head("main");
    Repository::init_opts(root, &options).map_err(err)?;
    Ok(())
}
pub fn identity(root: &Path, name: &str, email: &str) -> Result<(), String> {
    let (name, email) = (name.trim(), email.trim());
    if name.is_empty()
        || email.is_empty()
        || name.contains(['\n', '\r', '<', '>'])
        || email.contains(['\n', '\r', '<', '>'])
    {
        return Err("Enter a Git name and email.".into());
    }
    Signature::now(name, email).map_err(err)?;
    let repo = open(root)?;
    let mut config = repo
        .config()
        .map_err(err)?
        .open_level(git2::ConfigLevel::Local)
        .map_err(err)?;
    config.set_str("user.name", name).map_err(err)?;
    config.set_str("user.email", email).map_err(err)
}
/// Reserve the ordinary index lock. No visible staging changes on rejected commits.
pub fn commit(
    root: &Path,
    expected_head: Option<&str>,
    expected_tree: &str,
    message: &str,
) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("Describe what you changed before committing.".into());
    }
    let repo = open(root)?;
    let index_path = repo.path().join("index");
    let lock_path = repo.path().join("index.lock");
    let mut lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|e| err(format!("Another Git operation may be running: {e}")))?;
    let scratch = repo
        .path()
        .join(format!("river-commit-index-{}", std::process::id()));
    let mut owns_scratch = false;
    let mut owns_lock = true;
    let result = (|| {
        if repo.state() != RepositoryState::Clean
            || repo.index().map_err(err)?.has_conflicts()
            || repo.head_detached().map_err(err)?
        {
            return Err("Resolve Git conflicts or check out a branch before committing.".into());
        }
        let parent = head(&repo)?;
        if parent.as_ref().map(|h| h.id().to_string()).as_deref() != expected_head {
            return Err("Git history changed. Refresh and review before committing.".into());
        }
        // Separate temporary index: preserve an existing staged index on any failure.
        if scratch.exists() {
            return Err(
                "A temporary Git index already exists. Restart River before retrying.".into(),
            );
        }
        owns_scratch = true;
        let mut index = Index::open(&scratch).map_err(err)?;
        let tree = candidate(&repo, &mut index)?;
        if tree.to_string() != expected_tree {
            return Err("Project files changed. Refresh and review before committing.".into());
        }
        if parent.as_ref().is_some_and(|h| h.tree_id() == tree) {
            return Err("No changes to commit.".into());
        }
        let signature = repo
            .signature()
            .map_err(|_| "Set your Git name and email for this project first.".to_owned())?;
        index.write().map_err(err)?;
        lock.write_all(&fs::read(&scratch).map_err(err)?)
            .map_err(err)?;
        lock.sync_all().map_err(err)?;
        let tree = repo.find_tree(tree).map_err(err)?;
        let parents: Vec<_> = parent.iter().collect();
        let id = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .map_err(err)?;
        fs::rename(&lock_path, &index_path).map_err(|e| {
            err(format!(
                "Commit {id} was created, but the Git index could not be refreshed: {e}"
            ))
        })?;
        owns_lock = false;
        Ok(id.to_string())
    })();
    drop(lock);
    if owns_lock {
        let _ = fs::remove_file(&lock_path);
    }
    if owns_scratch {
        let _ = fs::remove_file(scratch);
    }
    result
}
pub fn log(root: &Path, before: Option<&str>) -> Result<Vec<Commit>, String> {
    let repo = open(root)?;
    if head(&repo)?.is_none() {
        return Ok(vec![]);
    }
    let mut walk = repo.revwalk().map_err(err)?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)
        .map_err(err)?;
    walk.push_head().map_err(err)?;
    let mut reached = before.is_none();
    let mut commits = Vec::new();
    for id in walk {
        let id = id.map_err(err)?;
        if !reached {
            reached = Some(id.to_string()).as_deref() == before;
            continue;
        }
        let c = repo.find_commit(id).map_err(err)?;
        commits.push(Commit {
            id: id.to_string(),
            message: c.message().unwrap_or("Untitled commit").into(),
            author: c.author().name().unwrap_or("Unknown").into(),
            timestamp: c.time().seconds(),
        });
        if commits.len() == 100 {
            break;
        }
    }
    Ok(commits)
}
pub fn commit_diff(root: &Path, id: &str) -> Result<Review, String> {
    let repo = open(root)?;
    let id = Oid::from_str(id).map_err(err)?;
    let tip = head(&repo)?.ok_or("No commits yet")?;
    if id != tip.id() && !repo.graph_descendant_of(tip.id(), id).map_err(err)? {
        return Err("This commit is not in the current branch history.".into());
    }
    let c = repo.find_commit(id).map_err(err)?;
    let old = if c.parent_count() > 0 {
        Some(c.parent(0).map_err(err)?.tree().map_err(err)?)
    } else {
        None
    };
    review(&repo, old.as_ref(), &c.tree().map_err(err)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder() -> std::path::PathBuf {
        // Parallel tests can read the same clock value; a counter keeps folders unique.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "river-git-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    fn save(root: &Path, message: &str) -> String {
        let s = status(root).unwrap();
        commit(root, s.head.as_deref(), &s.tree, message).unwrap()
    }
    #[test]
    fn normal_commits_include_add_edit_delete_extras_and_ignore_rules() {
        let root = folder();
        assert!(!status(&root).unwrap().initialized);
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        fs::create_dir(root.join("_river")).unwrap();
        fs::write(root.join(".gitignore"), "secret.md\n/_river/river.json\n").unwrap();
        fs::write(root.join("secret.md"), "private").unwrap();
        fs::write(root.join("Story.fountain"), "INT. CAFÉ - NIGHT\n").unwrap();
        fs::write(root.join("_river/scratchpad.md"), "notes").unwrap();
        fs::write(root.join("_river/river.json"), "{}").unwrap();
        let s = status(&root).unwrap();
        assert_eq!(s.branch, "main");
        assert_eq!(s.review.files.len(), 3);
        assert!(s.review.patch.contains("+INT. CAFÉ"));
        let first = save(&root, "First draft");
        assert!(status(&root).unwrap().review.files.is_empty());
        fs::remove_file(root.join("Story.fountain")).unwrap();
        fs::write(root.join("_river/scratchpad.md"), "revised notes 🎬").unwrap();
        fs::write(root.join("New.md"), "new chapter").unwrap();
        save(&root, "New direction");
        assert_eq!(log(&root, None).unwrap().len(), 2);
        assert_eq!(
            log(&root, Some(&log(&root, None).unwrap()[0].id))
                .unwrap()
                .len(),
            1
        );
        assert!(
            commit_diff(&root, &first)
                .unwrap()
                .patch
                .contains("+INT. CAFÉ")
        );
        let repo = open(&root).unwrap();
        assert!(
            repo.index()
                .unwrap()
                .get_path(Path::new("secret.md"), 0)
                .is_none()
        );
        assert!(
            repo.index()
                .unwrap()
                .get_path(Path::new("_river/river.json"), 0)
                .is_none()
        );
        assert!(
            repo.statuses(None)
                .unwrap()
                .iter()
                .all(|s| s.status() == git2::Status::IGNORED)
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stale_review_and_index_lock_preserve_history_and_staging() {
        let root = folder();
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        fs::write(root.join("Story.md"), "first").unwrap();
        save(&root, "First");
        fs::write(root.join("Story.md"), "staged").unwrap();
        let repo = open(&root).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("Story.md")).unwrap();
        index.write().unwrap();
        let old_index = fs::read(repo.path().join("index")).unwrap();
        let s = status(&root).unwrap();
        assert_eq!(old_index, fs::read(repo.path().join("index")).unwrap());
        fs::write(root.join("Story.md"), "new unseen edits").unwrap();
        assert!(
            commit(&root, s.head.as_deref(), &s.tree, "Fail")
                .unwrap_err()
                .contains("changed")
        );
        assert_eq!(old_index, fs::read(repo.path().join("index")).unwrap());
        assert_eq!(log(&root, None).unwrap().len(), 1);
        fs::write(repo.path().join("index.lock"), "external lock").unwrap();
        assert!(commit(&root, s.head.as_deref(), &s.tree, "Fail").is_err());
        assert_eq!(
            fs::read(repo.path().join("index.lock")).unwrap(),
            b"external lock"
        );
        fs::remove_file(repo.path().join("index.lock")).unwrap();
        save(&root, "All working changes");
        assert!(status(&root).unwrap().review.files.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn detached_and_conflicted_operations_are_blocked_and_parent_repo_is_not_adopted() {
        let root = folder();
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        fs::write(root.join("Story.md"), "first").unwrap();
        let id = save(&root, "First");
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        assert!(!status(&sub).unwrap().initialized);
        let repo = open(&root).unwrap();
        repo.set_head_detached(Oid::from_str(&id).unwrap()).unwrap();
        assert!(status(&root).unwrap().blocked.is_some());
        fs::write(repo.path().join("MERGE_HEAD"), &id).unwrap();
        assert!(status(&root).unwrap().blocked.is_some());
        let s = status(&root).unwrap();
        assert!(commit(&root, s.head.as_deref(), &s.tree, "Fail").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
