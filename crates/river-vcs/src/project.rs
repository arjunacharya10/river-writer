//! Standard local Git operations. The private autosave repository is never traversed.
use git2::{DiffFormat, Index, IndexAddOption, Oid, Repository, RepositoryState, Signature};
use serde::{Deserialize, Serialize};
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
    pub identity: Option<Identity>,
    pub blocked: Option<String>,
    /// Changed paths from Git's stat-cached status; nothing is hashed or written.
    pub files: Vec<Change>,
}
/// A full snapshot of the working tree, built on demand for review and commit.
#[derive(Serialize)]
pub struct Reviewed {
    pub head: Option<String>,
    /// Commit only proceeds if the project still produces exactly this tree.
    pub tree: String,
    pub review: Review,
}
#[derive(Serialize)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}
/// The Mac account's full name and short user name (`id -F`, `id -un`), used
/// when Git has no identity so writers are never asked for one up front.
fn account_identity() -> Option<(String, String)> {
    let id = |flag: &str| {
        std::process::Command::new("/usr/bin/id")
            .arg(flag)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let name = id("-F")?;
    let user = id("-un")?;
    Some((name, format!("{user}@localhost")))
}
/// Git's configured identity, else the given account identity.
fn author_with(
    repo: &Repository,
    account: impl FnOnce() -> Option<(String, String)>,
) -> Option<Signature<'static>> {
    repo.signature()
        .ok()
        .or_else(|| account().and_then(|(name, email)| Signature::now(&name, &email).ok()))
}
fn author(repo: &Repository) -> Option<Signature<'static>> {
    author_with(repo, account_identity)
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
/// Add every non-ignored file, skipping folders that are Git repositories of
/// their own (another project inside this one): libgit2 cannot add those.
fn add_project_files(repo: &Repository, index: &mut Index) -> Result<(), String> {
    let root = repo
        .workdir()
        .ok_or("Choose a working project folder")?
        .to_path_buf();
    let mut skip_nested = |path: &Path, _: &[u8]| -> i32 {
        let nested = path.to_string_lossy().ends_with('/') || root.join(path).join(".git").exists();
        i32::from(nested)
    };
    index
        .add_all(["*"], IndexAddOption::DEFAULT, Some(&mut skip_nested))
        .map_err(err)
}
fn candidate(repo: &Repository, index: &mut Index) -> Result<Oid, String> {
    repo.set_index(index).map_err(err)?;
    if let Some(head) = head(repo)? {
        index.read_tree(&head.tree().map_err(err)?).map_err(err)?;
    }
    index.update_all(["*"], None).map_err(err)?;
    add_project_files(repo, index)?;
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
        if truncated || patch.len() + line.content().len() + 1 > 1_000_000 {
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
/// Cheap enough to poll: uses the index's stat cache and writes nothing.
pub fn status(root: &Path) -> Result<Status, String> {
    if !root.join(".git").exists() {
        return Ok(Status {
            initialized: false,
            branch: String::new(),
            head: None,
            identity: None,
            blocked: None,
            files: vec![],
        });
    }
    let repo = open(root)?;
    let blocked = if repo.state() != RepositoryState::Clean
        || repo.index().map_err(err)?.has_conflicts()
    {
        Some("Resolve the current Git operation or conflicts before committing in River.".into())
    } else if repo.head_detached().map_err(err)? {
        Some("Switch to a branch below before committing.".into())
    } else {
        None
    };
    let mut options = git2::StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .exclude_submodules(true)
        .renames_head_to_index(false)
        .renames_index_to_workdir(false);
    let files = repo
        .statuses(Some(&mut options))
        .map_err(err)?
        .iter()
        .filter_map(|entry| {
            let path = entry.path()?.to_owned();
            // UI state, and other projects nested inside this one, are not changes.
            if path == "_river/river.json" || path.ends_with('/') {
                return None;
            }
            let flags = entry.status();
            let kind = if flags.intersects(git2::Status::WT_NEW | git2::Status::INDEX_NEW) {
                "Added"
            } else if flags.intersects(git2::Status::WT_DELETED | git2::Status::INDEX_DELETED) {
                "Deleted"
            } else {
                "Modified"
            };
            Some(Change {
                path,
                kind: kind.into(),
            })
        })
        .collect();
    let identity = author(&repo).map(|s| Identity {
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
        head: head(&repo)?.map(|h| h.id().to_string()),
        identity,
        blocked,
        files,
    })
}
/// Snapshot every non-ignored file and diff it against HEAD. Objects go to an
/// in-memory store dropped with the repository handle, so review writes nothing.
pub fn review_changes(root: &Path) -> Result<Reviewed, String> {
    let repo = open(root)?;
    let odb = repo.odb().map_err(err)?;
    let _memory = odb.add_new_mempack_backend(1000).map_err(err)?;
    let parent = head(&repo)?;
    let old = parent.as_ref().map(|h| h.tree()).transpose().map_err(err)?;
    let tree = candidate(&repo, &mut Index::new().map_err(err)?)?;
    let review = review(&repo, old.as_ref(), &repo.find_tree(tree).map_err(err)?)?;
    Ok(Reviewed {
        head: parent.as_ref().map(|h| h.id().to_string()),
        tree: tree.to_string(),
        review,
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
/// Keep River's per-machine UI state out of Git without touching the writer's
/// `.gitignore`: the rule lives in the repository's local `info/exclude`.
fn exclude_ui_state(repo: &Repository) -> Result<(), String> {
    let info = repo.path().join("info");
    let exclude = info.join("exclude");
    let old = match fs::read_to_string(&exclude) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(err(e)),
    };
    if old
        .lines()
        .map(str::trim)
        .any(|line| line == "/_river/river.json" || line == "_river/river.json")
    {
        return Ok(());
    }
    let mut next = old;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("# River UI state (this machine only)\n/_river/river.json\n");
    fs::create_dir_all(&info).map_err(err)?;
    fs::write(&exclude, next).map_err(err)
}
/// Every River project has Git. A new repository starts with a snapshot of the
/// files already there ("Start project history"), so Changes lists only what the
/// writer edits afterwards. An existing repository with history is adopted as
/// it is. Returns whether River made the starting snapshot.
pub fn ensure_repository(root: &Path) -> Result<bool, String> {
    let created = !root.join(".git").exists();
    if created {
        init(root)?;
    }
    let repo = open(root)?;
    exclude_ui_state(&repo)?;
    // An existing repository is adopted as it is, unless it has no commits and
    // nothing staged (e.g. one an older River created): that gets the snapshot too.
    if !created && (head(&repo)?.is_some() || !repo.index().map_err(err)?.is_empty()) {
        return Ok(false);
    }
    // The on-disk index must match the snapshot, or status would show it staged.
    let mut index = repo.index().map_err(err)?;
    add_project_files(&repo, &mut index)?;
    if index.get_path(Path::new("_river/river.json"), 0).is_some() {
        index
            .remove_path(Path::new("_river/river.json"))
            .map_err(err)?;
    }
    index.write().map_err(err)?;
    let tree = repo
        .find_tree(index.write_tree().map_err(err)?)
        .map_err(err)?;
    // The writer's identity (Git's, or their Mac account's); River's as a last resort.
    let signature = match author(&repo) {
        Some(signature) => signature,
        None => Signature::now("River Writer", "river@localhost").map_err(err)?,
    };
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Start project history",
        &tree,
        &[],
    )
    .map_err(err)?;
    Ok(true)
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
        let signature = author(&repo)
            .ok_or_else(|| "Set your Git name and email for this project first.".to_owned())?;
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
    let c = reachable(&repo, id)?;
    let old = if c.parent_count() > 0 {
        Some(c.parent(0).map_err(err)?.tree().map_err(err)?)
    } else {
        None
    };
    review(&repo, old.as_ref(), &c.tree().map_err(err)?)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Restored {
    /// Pre-restore project tree; the undo source.
    pub before: String,
    /// Project tree just after the restore; undo checks restored files still match it.
    pub after: String,
    /// Project-relative paths that were written or removed.
    pub paths: Vec<String>,
}
fn reachable<'r>(repo: &'r Repository, id: &str) -> Result<git2::Commit<'r>, String> {
    let id = Oid::from_str(id).map_err(err)?;
    let tip = head(repo)?.ok_or("No commits yet")?;
    if id != tip.id() && !repo.graph_descendant_of(tip.id(), id).map_err(err)? {
        return Err("This commit is not in the current branch history.".into());
    }
    repo.find_commit(id).map_err(err)
}
fn current_tree(repo: &Repository) -> Result<Oid, String> {
    if repo.state() != RepositoryState::Clean || repo.index().map_err(err)?.has_conflicts() {
        return Err("Finish the current Git operation or conflicts before restoring.".into());
    }
    candidate(repo, &mut Index::new().map_err(err)?)
}
fn blob_at(tree: &git2::Tree<'_>, path: &str) -> Result<Option<(Oid, i32)>, String> {
    match tree.get_path(Path::new(path)) {
        Ok(entry) => match entry.kind() {
            Some(git2::ObjectType::Blob) if matches!(entry.filemode(), 0o100644 | 0o100755) => {
                Ok(Some((entry.id(), entry.filemode())))
            }
            _ => Err(format!(
                "{path} is a link, folder or submodule in Git. Restore it with another Git tool."
            )),
        },
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}
/// A tree path becomes a disk path only through plain components and real
/// folders under the project root, never through a symbolic link.
fn disk_path(root: &Path, path: &str) -> Result<std::path::PathBuf, String> {
    let relative = Path::new(path);
    if path == "_river/river.json"
        || relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
        || relative.starts_with(".git")
    {
        return Err(format!("River will not restore {path}."));
    }
    let mut at = root.to_path_buf();
    for part in relative.components() {
        at.push(part);
        if at.is_symlink() {
            return Err(format!(
                "{path} goes through a symbolic link; River will not replace it."
            ));
        }
    }
    if at.is_dir() {
        return Err(format!(
            "{path} is a folder on disk; River will not replace it."
        ));
    }
    Ok(at)
}
/// Write or remove each path so it matches `source`. Every path is validated
/// before any disk change; a later I/O failure rolls back what was applied.
fn apply(
    repo: &Repository,
    root: &Path,
    source: &git2::Tree<'_>,
    paths: &[String],
    before: &git2::Tree<'_>,
) -> Result<(), String> {
    let mut plan = Vec::new();
    for path in paths {
        plan.push((disk_path(root, path)?, blob_at(source, path)?));
    }
    let mut done: Vec<usize> = Vec::new();
    let write = |disk: &Path, target: &Option<(Oid, i32)>| -> Result<(), String> {
        match target {
            Some((id, mode)) => {
                let blob = repo.find_blob(*id).map_err(err)?;
                let parent = disk.parent().ok_or("Invalid restore path")?;
                fs::create_dir_all(parent).map_err(err)?;
                let temp = parent.join(format!(".river-restore-{}-{}", std::process::id(), id));
                let result = (|| {
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&temp)?;
                    file.write_all(blob.content())?;
                    file.sync_all()?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = if *mode == 0o100755 { 0o755 } else { 0o644 };
                        fs::set_permissions(&temp, fs::Permissions::from_mode(mode))?;
                    }
                    fs::rename(&temp, disk)
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&temp);
                }
                result.map_err(err)
            }
            None => match fs::remove_file(disk) {
                Ok(()) => {
                    // Tidy folders emptied by the restore, never the root itself.
                    let mut dir = disk.parent();
                    while let Some(d) = dir.filter(|d| *d != root && d.starts_with(root)) {
                        if fs::remove_dir(d).is_err() {
                            break;
                        }
                        dir = d.parent();
                    }
                    Ok(())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(err(e)),
            },
        }
    };
    for (n, (disk, target)) in plan.iter().enumerate() {
        if let Err(e) = write(disk, target) {
            for &m in done.iter().rev() {
                let original = blob_at(before, &paths[m]).ok().flatten();
                let _ = write(&plan[m].0, &original);
            }
            return Err(format!("Restore stopped and was rolled back: {e}"));
        }
        done.push(n);
    }
    Ok(())
}
/// Restore one file (`path`) or the whole project from a commit in this branch.
/// A file deleted by that commit is restored from the commit's parent.
pub fn restore(root: &Path, commit: &str, path: Option<&str>) -> Result<Restored, String> {
    let repo = open(root)?;
    let c = reachable(&repo, commit)?;
    let before_id = current_tree(&repo)?;
    let before = repo.find_tree(before_id).map_err(err)?;
    let (source, paths) = match path {
        Some(path) => {
            let tree = c.tree().map_err(err)?;
            let source = if blob_at(&tree, path)?.is_some() {
                tree
            } else {
                (c.parent_count() > 0)
                    .then(|| c.parent(0).and_then(|p| p.tree()))
                    .transpose()
                    .map_err(err)?
                    .filter(|t| matches!(blob_at(t, path), Ok(Some(_))))
                    .ok_or_else(|| format!("{path} is not part of this commit."))?
            };
            let paths = if blob_at(&before, path)? == blob_at(&source, path)? {
                vec![]
            } else {
                vec![path.to_owned()]
            };
            (source, paths)
        }
        None => {
            let source = c.tree().map_err(err)?;
            let diff = repo
                .diff_tree_to_tree(Some(&before), Some(&source), None)
                .map_err(err)?;
            let paths = diff
                .deltas()
                .filter_map(|d| d.new_file().path().or(d.old_file().path()))
                .map(|p| p.to_string_lossy().into_owned())
                .filter(|p| p != "_river/river.json")
                .collect();
            (source, paths)
        }
    };
    if paths.is_empty() {
        return Err("Nothing to restore: it already matches this commit.".into());
    }
    apply(&repo, root, &source, &paths, &before)?;
    Ok(Restored {
        before: before_id.to_string(),
        after: current_tree(&repo)?.to_string(),
        paths,
    })
}
/// Put restored paths back as they were before the restore, unless one of them
/// was edited after it (then the newer writing wins and undo is refused).
pub fn undo_restore(root: &Path, restored: &Restored) -> Result<(), String> {
    let repo = open(root)?;
    let before = repo
        .find_tree(Oid::from_str(&restored.before).map_err(err)?)
        .map_err(|_| "The pre-restore state is no longer available.".to_owned())?;
    let after = repo
        .find_tree(Oid::from_str(&restored.after).map_err(err)?)
        .map_err(|_| "The restore can no longer be undone.".to_owned())?;
    let now = repo.find_tree(current_tree(&repo)?).map_err(err)?;
    for path in &restored.paths {
        if blob_at(&now, path)? != blob_at(&after, path)? {
            return Err(format!(
                "{path} changed after the restore, so it was not undone. Your newer writing was kept."
            ));
        }
    }
    apply(&repo, root, &before, &restored.paths, &now)
}

#[derive(Serialize)]
pub struct Branch {
    pub name: String,
    pub current: bool,
}
pub fn branches(root: &Path) -> Result<Vec<Branch>, String> {
    let repo = open(root)?;
    let current = repo
        .find_reference("HEAD")
        .ok()
        .and_then(|h| h.symbolic_target().map(str::to_owned));
    let mut list = Vec::new();
    for branch in repo.branches(Some(git2::BranchType::Local)).map_err(err)? {
        let (branch, _) = branch.map_err(err)?;
        let Some(name) = branch.name().map_err(err)? else {
            continue;
        };
        let reference = format!("refs/heads/{name}");
        // An unborn current branch has no ref yet, so it is listed below.
        list.push(Branch {
            current: current.as_deref() == Some(reference.as_str()),
            name: name.to_owned(),
        });
    }
    if let Some(unborn) = current
        .as_deref()
        .and_then(|r| r.strip_prefix("refs/heads/"))
        .filter(|n| !list.iter().any(|b| b.name == *n))
    {
        list.push(Branch {
            name: unborn.to_owned(),
            current: true,
        });
    }
    list.sort_by_key(|a| a.name.to_lowercase());
    Ok(list)
}
/// Create a branch at the current commit. It does not switch to it.
pub fn create_branch(root: &Path, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || !git2::Branch::name_is_valid(name).map_err(err)? {
        return Err("Use a branch name without spaces or special characters, like draft-2.".into());
    }
    let repo = open(root)?;
    let tip = head(&repo)?.ok_or("Commit once before creating branches.")?;
    if repo.find_branch(name, git2::BranchType::Local).is_ok() {
        return Err(format!("A branch named {name} already exists."));
    }
    repo.branch(name, &tip, false).map_err(err)?;
    Ok(())
}
/// Switch to a local branch. Uncommitted changes must be committed first, so a
/// switch never loses or carries writing between drafts. Returns rewritten paths.
pub fn switch_branch(root: &Path, name: &str) -> Result<Vec<String>, String> {
    let repo = open(root)?;
    if repo.state() != RepositoryState::Clean || repo.index().map_err(err)?.has_conflicts() {
        return Err("Finish the current Git operation or conflicts before switching.".into());
    }
    let reference = format!("refs/heads/{name}");
    let target = repo
        .find_branch(name, git2::BranchType::Local)
        .map_err(|_| format!("There is no branch named {name}."))?
        .get()
        .peel_to_commit()
        .map_err(err)?;
    let attached = repo
        .find_reference("HEAD")
        .ok()
        .and_then(|h| h.symbolic_target().map(str::to_owned));
    if attached.as_deref() == Some(reference.as_str()) {
        return Ok(vec![]);
    }
    let tip = head(&repo)?.ok_or("Commit once before switching branches.")?;
    if tip.id() == target.id() {
        // Same commit (e.g. a branch just created here): keep uncommitted work, like `git switch -c`.
        repo.set_head(&reference).map_err(err)?;
        return Ok(vec![]);
    }
    // A separate handle: `candidate` installs an in-memory index on its repository.
    let working = current_tree(&open(root)?)?;
    if working != tip.tree_id() {
        return Err("Commit your changes before switching branches.".into());
    }
    let old = tip.tree().map_err(err)?;
    let new = target.tree().map_err(err)?;
    let diff = repo
        .diff_tree_to_tree(Some(&old), Some(&new), None)
        .map_err(err)?;
    let paths: Vec<String> = diff
        .deltas()
        .filter_map(|d| d.new_file().path().or(d.old_file().path()))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.safe();
    repo.checkout_tree(new.as_object(), Some(&mut checkout))
        .map_err(err)?;
    repo.set_head(&reference).map_err(err)?;
    Ok(paths)
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
        let s = review_changes(root).unwrap();
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
        assert_eq!(s.files.len(), 3);
        let r = review_changes(&root).unwrap();
        assert_eq!(r.review.files.len(), 3);
        assert!(r.review.patch.contains("+INT. CAFÉ"));
        let first = save(&root, "First draft");
        assert!(status(&root).unwrap().files.is_empty());
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
        let s = review_changes(&root).unwrap();
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
        assert!(status(&root).unwrap().files.is_empty());
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
        let s = review_changes(&root).unwrap();
        assert!(commit(&root, s.head.as_deref(), &s.tree, "Fail").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restores_files_and_projects_and_undoes_them() {
        let root = folder();
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        fs::create_dir_all(root.join("acts")).unwrap();
        fs::write(root.join("acts/One.fountain"), "INT. CAFÉ - DAY\n").unwrap();
        fs::write(root.join("Deleted.md"), "a scene worth keeping").unwrap();
        let first = save(&root, "First");
        fs::remove_file(root.join("Deleted.md")).unwrap();
        fs::write(root.join("acts/One.fountain"), "INT. CAFÉ - NIGHT 🎬\n").unwrap();
        let second = save(&root, "Second");
        fs::write(root.join("New.md"), "uncommitted new chapter").unwrap();
        // Bring back a file deleted by a commit, from that commit's parent.
        let r = restore(&root, &second, Some("Deleted.md")).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("Deleted.md")).unwrap(),
            "a scene worth keeping"
        );
        undo_restore(&root, &r).unwrap();
        assert!(!root.join("Deleted.md").exists());
        // Whole project: overwrites, recreates and removes newer files.
        let r = restore(&root, &first, None).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("acts/One.fountain")).unwrap(),
            "INT. CAFÉ - DAY\n"
        );
        assert!(root.join("Deleted.md").exists() && !root.join("New.md").exists());
        undo_restore(&root, &r).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("acts/One.fountain")).unwrap(),
            "INT. CAFÉ - NIGHT 🎬\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("New.md")).unwrap(),
            "uncommitted new chapter"
        );
        assert!(!root.join("Deleted.md").exists());
        assert!(
            restore(&root, &second, Some("acts/One.fountain"))
                .unwrap_err()
                .contains("already matches")
        );
        // Undo never overwrites writing done after the restore.
        let r = restore(&root, &first, Some("acts/One.fountain")).unwrap();
        fs::write(root.join("acts/One.fountain"), "newer writing").unwrap();
        assert!(
            undo_restore(&root, &r)
                .unwrap_err()
                .contains("changed after")
        );
        assert_eq!(
            fs::read_to_string(root.join("acts/One.fountain")).unwrap(),
            "newer writing"
        );
        // Restoring never commits or moves the branch.
        assert_eq!(log(&root, None).unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restore_refuses_foreign_commits_and_links() {
        let root = folder();
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        fs::write(root.join("Story.md"), "one").unwrap();
        save(&root, "First");
        fs::write(root.join("Story.md"), "two").unwrap();
        assert!(restore(&root, &"0".repeat(40), None).is_err());
        #[cfg(unix)]
        {
            let outside = folder();
            fs::create_dir(root.join("real")).unwrap();
            fs::write(root.join("real/Linked.md"), "inside").unwrap();
            let linked = save(&root, "Folder");
            fs::remove_dir_all(root.join("real")).unwrap();
            std::os::unix::fs::symlink(&outside, root.join("real")).unwrap();
            assert!(
                restore(&root, &linked, Some("real/Linked.md"))
                    .unwrap_err()
                    .contains("symbolic link")
            );
            assert!(!outside.join("Linked.md").exists());
            fs::remove_dir_all(outside).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn branches_create_switch_and_protect_uncommitted_writing() {
        let root = folder();
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        assert_eq!(branches(&root).unwrap()[0].name, "main");
        assert!(create_branch(&root, "draft-2").is_err());
        fs::write(root.join("Story.md"), "main draft").unwrap();
        fs::write(root.join("Shared.md"), "same everywhere").unwrap();
        save(&root, "Main");
        assert!(create_branch(&root, "bad name").is_err());
        create_branch(&root, "draft-2").unwrap();
        assert!(
            create_branch(&root, "draft-2")
                .unwrap_err()
                .contains("already exists")
        );
        let names: Vec<_> = branches(&root)
            .unwrap()
            .into_iter()
            .map(|b| (b.name, b.current))
            .collect();
        assert_eq!(
            names,
            vec![("draft-2".into(), false), ("main".into(), true)]
        );
        fs::write(root.join("Story.md"), "alternate ending").unwrap();
        // Same commit: uncommitted writing comes along to the new branch.
        assert!(switch_branch(&root, "draft-2").unwrap().is_empty());
        assert_eq!(status(&root).unwrap().branch, "draft-2");
        fs::write(root.join("Extra.md"), "only on draft-2").unwrap();
        save(&root, "Alternate");
        // Once branches diverge, uncommitted writing blocks a switch and stays on disk.
        fs::write(root.join("Unsaved.md"), "not committed").unwrap();
        assert!(
            switch_branch(&root, "main")
                .unwrap_err()
                .contains("Commit your changes")
        );
        assert_eq!(
            fs::read_to_string(root.join("Unsaved.md")).unwrap(),
            "not committed"
        );
        fs::remove_file(root.join("Unsaved.md")).unwrap();
        let mut paths = switch_branch(&root, "main").unwrap();
        paths.sort();
        assert_eq!(paths, vec!["Extra.md", "Story.md"]);
        assert_eq!(
            fs::read_to_string(root.join("Story.md")).unwrap(),
            "main draft"
        );
        assert!(!root.join("Extra.md").exists());
        assert!(status(&root).unwrap().files.is_empty());
        assert_eq!(status(&root).unwrap().branch, "main");
        switch_branch(&root, "draft-2").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("Extra.md")).unwrap(),
            "only on draft-2"
        );
        assert_eq!(log(&root, None).unwrap()[0].message, "Alternate");
        assert!(switch_branch(&root, "missing").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn polling_and_review_write_no_git_objects() {
        fn objects(root: &Path) -> usize {
            fn walk(dir: &Path) -> usize {
                fs::read_dir(dir).map_or(0, |entries| {
                    entries
                        .flatten()
                        .map(|e| {
                            if e.path().is_dir() {
                                walk(&e.path())
                            } else {
                                1
                            }
                        })
                        .sum()
                })
            }
            walk(&root.join(".git/objects"))
        }
        let root = folder();
        init(&root).unwrap();
        identity(&root, "Writer", "writer@example.invalid").unwrap();
        fs::write(root.join("Story.md"), "first").unwrap();
        save(&root, "First");
        let before = objects(&root);
        for n in 0..5 {
            fs::write(root.join("Story.md"), format!("draft {n} café")).unwrap();
            fs::write(root.join("New.md"), format!("new {n}")).unwrap();
            let s = status(&root).unwrap();
            assert_eq!(s.files.len(), 2);
            let r = review_changes(&root).unwrap();
            assert!(r.review.patch.contains(&format!("+draft {n} café")));
        }
        assert_eq!(objects(&root), before);
        // The in-memory review tree still commits exactly what was reviewed.
        let r = review_changes(&root).unwrap();
        commit(&root, r.head.as_deref(), &r.tree, "Second").unwrap();
        assert!(status(&root).unwrap().files.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn new_repositories_start_clean_with_a_snapshot_and_local_ignore() {
        let root = folder();
        fs::create_dir(root.join("_river")).unwrap();
        fs::write(root.join("Story.md"), "existing chapter").unwrap();
        fs::write(root.join("_river/todos.md"), "- [ ] Task").unwrap();
        fs::write(root.join("_river/river.json"), "{}").unwrap();
        assert!(ensure_repository(&root).unwrap());
        // Nothing River created is a pending change, and .gitignore is untouched.
        assert!(status(&root).unwrap().files.is_empty());
        assert!(!root.join(".gitignore").exists());
        let history = log(&root, None).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "Start project history");
        let snapshot = commit_diff(&root, &history[0].id).unwrap();
        assert!(snapshot.files.iter().any(|f| f.path == "Story.md"));
        assert!(snapshot.files.iter().any(|f| f.path == "_river/todos.md"));
        assert!(!snapshot.files.iter().any(|f| f.path == "_river/river.json"));
        // Only the writer's edits appear afterwards; branches are available at once.
        fs::write(root.join("Story.md"), "edited chapter").unwrap();
        let changed: Vec<_> = status(&root)
            .unwrap()
            .files
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert_eq!(changed, vec!["Story.md"]);
        create_branch(&root, "draft-2").unwrap();
        // Adopting an existing repository changes nothing but the local ignore.
        assert!(!ensure_repository(&root).unwrap());
        assert_eq!(log(&root, None).unwrap().len(), 1);
        let exclude = fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        assert_eq!(exclude.matches("/_river/river.json").count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn empty_repositories_left_by_older_river_get_the_snapshot_too() {
        let root = folder();
        init(&root).unwrap();
        fs::write(root.join(".gitignore"), "/_river/river.json\n").unwrap();
        fs::write(root.join("Test 2.md"), "").unwrap();
        assert_eq!(status(&root).unwrap().files.len(), 2);
        assert!(ensure_repository(&root).unwrap());
        assert!(status(&root).unwrap().files.is_empty());
        // Something staged by the writer is never committed for them.
        let other = folder();
        init(&other).unwrap();
        fs::write(other.join("Draft.md"), "staged on purpose").unwrap();
        let repo = open(&other).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("Draft.md")).unwrap();
        index.write().unwrap();
        assert!(!ensure_repository(&other).unwrap());
        assert!(log(&other, None).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other).unwrap();
    }
    #[test]
    fn a_project_inside_another_is_skipped_not_fatal() {
        let root = folder();
        fs::write(root.join("Outer.md"), "outer").unwrap();
        fs::create_dir(root.join("Inner Story")).unwrap();
        fs::write(root.join("Inner Story/Inner.md"), "inner").unwrap();
        assert!(ensure_repository(&root.join("Inner Story")).unwrap());
        assert!(ensure_repository(&root).unwrap());
        let snapshot = commit_diff(&root, &log(&root, None).unwrap()[0].id).unwrap();
        let paths: Vec<_> = snapshot.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["Outer.md"]);
        review_changes(&root).unwrap();
        assert!(status(&root).unwrap().files.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_git_identity_falls_back_to_the_mac_account() {
        let root = folder();
        init(&root).unwrap();
        let repo = open(&root).unwrap();
        // An empty configured name is no identity, as for a new writer.
        repo.config().unwrap().set_str("user.name", "").unwrap();
        let account = || Some(("Nora Writer".to_owned(), "nora@localhost".to_owned()));
        let signature = author_with(&repo, account).unwrap();
        assert_eq!(signature.name(), Some("Nora Writer"));
        assert_eq!(signature.email(), Some("nora@localhost"));
        assert!(author_with(&repo, || None).is_none());
        // A configured Git identity always wins.
        identity(&root, "Git Name", "git@example.invalid").unwrap();
        let signature = author_with(&open(&root).unwrap(), account).unwrap();
        assert_eq!(signature.name(), Some("Git Name"));
        if let Some((name, email)) = account_identity() {
            assert!(!name.is_empty() && email.ends_with("@localhost"));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
