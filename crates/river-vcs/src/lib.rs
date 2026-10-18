pub mod project;
// Private, local-only Git history. No transport features or network operations.
use git2::{ErrorCode, ObjectType, Oid, Repository, Signature};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Serialize, Debug)]
pub struct Version {
    pub id: String,
    pub timestamp: i64,
    pub bytes: usize,
}

pub struct History {
    repo: Repository,
    reference: String,
    key: String,
}
fn err(e: impl std::fmt::Display) -> String {
    format!("Local history: {e}")
}
fn hash(bytes: &[u8]) -> Result<String, String> {
    Oid::hash_object(ObjectType::Blob, bytes)
        .map(|id| id.to_string())
        .map_err(err)
}
impl History {
    /// Project history belongs to this exact project's repository, never a
    /// repository discovered in a parent directory. Standalone files use app data.
    pub fn open(project: Option<&Path>, file: &Path, data: &Path) -> Result<Self, String> {
        let (root, branch, alternate, key) =
            if let Some(project) = project.filter(|p| file.starts_with(p)) {
                let main = if project.join(".git").exists() {
                    Repository::open(project).map_err(err)?
                } else {
                    {
                        let mut options = git2::RepositoryInitOptions::new();
                        options.initial_head("main");
                        Repository::init_opts(project, &options).map_err(err)?
                    }
                };
                if main.is_bare() {
                    return Err(err("Choose a working project folder"));
                }
                // Worktrees have separate administrative directories and a shared ODB.
                let branch = if main.head_detached().map_err(err)? {
                    format!(
                        "detached-{}",
                        main.head()
                            .map_err(err)?
                            .target()
                            .ok_or("Missing Git HEAD")?
                    )
                } else {
                    main.find_reference("HEAD")
                        .map_err(err)?
                        .symbolic_target()
                        .ok_or("Invalid Git HEAD")?
                        .to_owned()
                };
                (
                    main.path().join("river-autosave"),
                    branch,
                    Some(main.commondir().join("objects")),
                    hash(
                        file.strip_prefix(project)
                            .map_err(err)?
                            .as_os_str()
                            .as_encoded_bytes(),
                    )?,
                )
            } else {
                let key = hash(file.as_os_str().as_encoded_bytes())?;
                (
                    data.join("history").join(format!("{key}.git")),
                    "standalone".into(),
                    None,
                    key,
                )
            };
        fs::create_dir_all(&root).map_err(err)?;
        let repo = if root.join("HEAD").exists() {
            Repository::open_bare(&root).map_err(err)?
        } else {
            Repository::init_bare(&root).map_err(err)?
        };
        // Alternates point FROM private history TO the main objects. History
        // objects are written only here, so a main-repository mirror cannot leak them.
        if let Some(alternate) = alternate {
            let alternate = alternate.canonicalize().map_err(err)?;
            let bytes = alternate.as_os_str().as_encoded_bytes();
            if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
                return Err(err("Git object path contains a newline"));
            }
            let path = root.join("objects/info/alternates");
            let mut value = bytes.to_vec();
            value.push(b'\n');
            if fs::read(&path).ok().as_deref() != Some(&value) {
                fs::write(&path, value).map_err(err)?;
            }
            // Reopen so libgit2 loads the newly installed alternate.
        }
        drop(repo);
        let repo = Repository::open_bare(&root).map_err(err)?;
        let mut config = repo.config().map_err(err)?;
        if config.get_bool("core.fsyncObjectFiles").ok() != Some(true) {
            config
                .set_bool("core.fsyncObjectFiles", true)
                .map_err(err)?;
        }
        let reference = format!("refs/heads/autosave-{}", hash(branch.as_bytes())?);
        Ok(Self {
            repo,
            reference,
            key,
        })
    }
    fn tip(&self) -> Result<Option<git2::Commit<'_>>, String> {
        match self.repo.find_reference(&self.reference) {
            Ok(reference) => reference.peel_to_commit().map(Some).map_err(err),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(err(e)),
        }
    }
    pub fn checkpoint(&self, bytes: &[u8]) -> Result<(), String> {
        let parent = self.tip()?;
        let old_tree = parent.as_ref().map(|p| p.tree()).transpose().map_err(err)?;
        let blob = self.repo.blob(bytes).map_err(err)?;
        if old_tree
            .as_ref()
            .and_then(|t| t.get_name(&self.key))
            .is_some_and(|e| e.id() == blob)
        {
            return Ok(());
        }
        let mut builder = self.repo.treebuilder(old_tree.as_ref()).map_err(err)?;
        builder.insert(&self.key, blob, 0o100644).map_err(err)?;
        let tree = self
            .repo
            .find_tree(builder.write().map_err(err)?)
            .map_err(err)?;
        let signature =
            Signature::now("River local history", "local@river.invalid").map_err(err)?;
        let parents: Vec<_> = parent.iter().collect();
        let commit = self
            .repo
            .commit(None, &signature, &signature, "autosave", &tree, &parents)
            .map_err(err)?;
        // Compare-and-swap prevents another River process from losing a snapshot.
        if let Some(parent) = parent {
            self.repo
                .reference_matching(&self.reference, commit, true, parent.id(), "autosave")
                .map_err(err)?;
        } else {
            self.repo
                .reference(&self.reference, commit, false, "autosave")
                .map_err(err)?;
        }
        Ok(())
    }
    pub fn versions(&self) -> Result<Vec<Version>, String> {
        self.older_versions(None)
    }
    pub fn older_versions(&self, before: Option<&str>) -> Result<Vec<Version>, String> {
        let mut versions = Vec::new();
        let mut next = self.tip()?;
        let before = before.map(Oid::from_str).transpose().map_err(err)?;
        let mut reached = before.is_none();
        while let Some(commit) = next {
            if !reached {
                reached = Some(commit.id()) == before;
                next = if commit.parent_count() > 0 {
                    Some(commit.parent(0).map_err(err)?)
                } else {
                    None
                };
                continue;
            }
            let tree = commit.tree().map_err(err)?;
            let blob = tree.get_name(&self.key).map(|e| e.id());
            let parent = if commit.parent_count() > 0 {
                Some(commit.parent(0).map_err(err)?)
            } else {
                None
            };
            let previous = parent
                .as_ref()
                .map(|p| p.tree())
                .transpose()
                .map_err(err)?
                .and_then(|t| t.get_name(&self.key).map(|e| e.id()));
            if let Some(id) = blob.filter(|id| Some(*id) != previous) {
                versions.push(Version {
                    id: commit.id().to_string(),
                    timestamp: commit.time().seconds(),
                    bytes: self.repo.find_blob(id).map_err(err)?.size(),
                });
                if versions.len() >= 200 {
                    break;
                }
            }
            next = parent;
        }
        Ok(versions)
    }
    pub fn read(&self, id: &str) -> Result<Vec<u8>, String> {
        let id = Oid::from_str(id).map_err(err)?;
        let tip = self.tip()?.ok_or("No local history for this branch")?;
        if id != tip.id() && !self.repo.graph_descendant_of(tip.id(), id).map_err(err)? {
            return Err(err("Version does not belong to this branch"));
        }
        let tree = self
            .repo
            .find_commit(id)
            .map_err(err)?
            .tree()
            .map_err(err)?;
        let entry = tree
            .get_name(&self.key)
            .ok_or("This version has no text for this file")?;
        Ok(self
            .repo
            .find_blob(entry.id())
            .map_err(err)?
            .content()
            .to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn folder() -> PathBuf {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "river-history-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn snapshots_deduplicate_survive_restart_and_are_per_file() {
        let folder = folder();
        let file = folder.join("Story.fountain");
        let history = History::open(Some(&folder), &file, &folder).unwrap();
        history.checkpoint(b"original").unwrap();
        history.checkpoint("new café 🎬".as_bytes()).unwrap();
        history.checkpoint("new café 🎬".as_bytes()).unwrap();
        let other = History::open(Some(&folder), &folder.join("Other.md"), &folder).unwrap();
        other.checkpoint(b"other story").unwrap();
        let history = History::open(Some(&folder), &file, &folder).unwrap();
        let versions = history.versions().unwrap();
        assert_eq!(versions.len(), 2);
        let older = history.older_versions(Some(&versions[0].id)).unwrap();
        assert_eq!(older.len(), 1);
        assert_eq!(older[0].id, versions[1].id);
        assert_eq!(history.read(&versions[1].id).unwrap(), b"original");
        assert_eq!(
            history.read(&versions[0].id).unwrap(),
            "new café 🎬".as_bytes()
        );
        assert_eq!(other.versions().unwrap().len(), 1);
        assert!(
            Repository::open(&folder)
                .unwrap()
                .references()
                .unwrap()
                .all(|r| !r.unwrap().name().unwrap().contains("autosave"))
        );
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn branches_and_standalone_files_are_isolated() {
        let folder = folder();
        let file = folder.join("Story.md");
        let first = History::open(Some(&folder), &file, &folder).unwrap();
        first.checkpoint(b"first branch secret").unwrap();
        let old = first.versions().unwrap()[0].id.clone();
        let repo = Repository::open(&folder).unwrap();
        repo.set_head("refs/heads/alternate").unwrap();
        let other = History::open(Some(&folder), &file, &folder).unwrap();
        assert!(other.versions().unwrap().is_empty());
        other.checkpoint(b"alternate").unwrap();
        assert!(other.read(&old).is_err());
        let lone = History::open(None, &file, &folder.join("app-data")).unwrap();
        lone.checkpoint(b"standalone").unwrap();
        assert_eq!(lone.versions().unwrap().len(), 1);
        assert!(lone.read(&old).is_err());
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn older_versions_remain_accessible_across_pages() {
        let folder = folder();
        let history = History::open(None, &folder.join("Story.md"), &folder).unwrap();
        for n in 0..205 {
            history.checkpoint(format!("draft {n}").as_bytes()).unwrap();
        }
        let first = history.versions().unwrap();
        assert_eq!(first.len(), 200);
        let older = history
            .older_versions(Some(&first.last().unwrap().id))
            .unwrap();
        assert_eq!(older.len(), 5);
        assert_eq!(history.read(&older.last().unwrap().id).unwrap(), b"draft 0");
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn linked_worktree_uses_its_own_private_history() {
        let folder = folder();
        let project = folder.join("main");
        let main = Repository::init(&project).unwrap();
        let tree = main
            .find_tree(main.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        let signature = Signature::now("Writer", "writer@example.invalid").unwrap();
        main.commit(Some("HEAD"), &signature, &signature, "public", &tree, &[])
            .unwrap();
        let checkout = folder.join("checkout");
        main.worktree("trial", &checkout, None).unwrap();
        assert!(checkout.join(".git").is_file());
        let history = History::open(Some(&checkout), &checkout.join("Story.md"), &folder).unwrap();
        history.checkpoint(b"worktree-only draft").unwrap();
        assert_eq!(history.versions().unwrap().len(), 1);
        assert!(
            History::open(Some(&project), &project.join("Story.md"), &folder)
                .unwrap()
                .versions()
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn mirror_push_cannot_include_private_autosaves() {
        let folder = folder();
        let project = folder.join("project");
        fs::create_dir(&project).unwrap();
        let main = Repository::init(&project).unwrap();
        let tree = main
            .find_tree(main.treebuilder(None).unwrap().write().unwrap())
            .unwrap();
        let signature = Signature::now("Writer", "writer@example.invalid").unwrap();
        main.commit(Some("HEAD"), &signature, &signature, "public", &tree, &[])
            .unwrap();
        let history = History::open(Some(&project), &project.join("Story.md"), &folder).unwrap();
        history.checkpoint(b"PRIVATE UNPUBLISHED DRAFT").unwrap();
        let private = history.versions().unwrap()[0].id.clone();
        fs::write(project.join("Story.md"), "Public draft").unwrap();
        project::identity(&project, "Writer", "writer@example.invalid").unwrap();
        let reviewed = project::review_changes(&project).unwrap();
        project::commit(
            &project,
            reviewed.head.as_deref(),
            &reviewed.tree,
            "Public commit from River",
        )
        .unwrap();

        let remote_path = folder.join("remote.git");
        Repository::init_bare(&remote_path).unwrap();
        // Independent regression proof of the exact CLI mirror case; the product
        // uses libgit2 and never invokes a shell or transport for local history.
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(["push", "--mirror"])
            .arg(&remote_path)
            .status()
            .unwrap();
        assert!(status.success());
        let remote = Repository::open_bare(&remote_path).unwrap();
        assert!(
            remote
                .find_commit(Oid::from_str(&private).unwrap())
                .is_err()
        );
        assert!(
            !remote
                .odb()
                .unwrap()
                .exists(Oid::hash_object(ObjectType::Blob, b"PRIVATE UNPUBLISHED DRAFT").unwrap())
        );
        fs::remove_dir_all(folder).unwrap();
    }
}
