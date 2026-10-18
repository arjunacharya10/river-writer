//! Private crash-recovery journals. Recovery never writes to a story's source path.
use crate::{Document, Documents, atomic_write};
use river_templates::TemplateId;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize, Deserialize)]
pub struct Draft {
    pub key: String,
    pub path: Option<PathBuf>,
    pub text: String,
    pub template: TemplateId,
}

pub struct Recovery {
    root: PathBuf,
    session: String,
}
impl Recovery {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        Ok(Self {
            root,
            session: format!(
                "{}-{stamp}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ),
        })
    }
    fn key(&self, id: u64) -> String {
        format!("{}-{id}", self.session)
    }
    fn path(&self, key: &str) -> Result<PathBuf, String> {
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            return Err("Invalid recovery identifier".into());
        }
        Ok(self.root.join(format!("{key}.json")))
    }
    pub fn protect(&self, doc: &Document) -> Result<(), String> {
        if !doc.dirty {
            return self.forget(doc.id);
        }
        let key = self.key(doc.id);
        let path = self.path(&key)?;
        let previous = match fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let bytes = serde_json::to_vec(&Draft {
            key,
            path: doc.path.clone(),
            text: doc.text.clone(),
            template: doc.template,
        })
        .map_err(|e| e.to_string())?;
        atomic_write(&path, &bytes, previous.as_deref())?;
        // Persist the directory entry too, so a completed journal survives a restart.
        #[cfg(unix)]
        fs::File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn forget(&self, id: u64) -> Result<(), String> {
        self.remove(&self.key(id))
    }
    pub fn remove(&self, key: &str) -> Result<(), String> {
        match fs::remove_file(self.path(key)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn pending(&self) -> Result<Vec<Draft>, String> {
        let mut drafts = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "json")
                || !entry.file_type().map_err(|e| e.to_string())?.is_file()
            {
                continue;
            }
            let key = path
                .file_stem()
                .and_then(|p| p.to_str())
                .ok_or("Invalid recovery filename")?;
            if key.starts_with(&format!("{}-", self.session)) {
                continue;
            }
            let draft: Draft = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Could not read recovered draft {}: {e}", path.display()))?;
            if self.path(&draft.key)? != path {
                return Err("Recovery identifier does not match its file".into());
            }
            drafts.push(draft);
        }
        drafts.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(drafts)
    }
    pub fn restore(&self, docs: &mut Documents, key: &str) -> Result<Document, String> {
        let draft: Draft =
            serde_json::from_slice(&fs::read(self.path(key)?).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let doc = docs.new_document();
        docs.update(doc.id, 1, draft.text, draft.template)?;
        // An intentionally emptied named file is still a recoverable edit.
        docs.docs.get_mut(&doc.id).unwrap().dirty = true;
        let restored = docs.get(doc.id)?.clone();
        // Transfer protection before removing the old journal. A crash here can
        // produce a duplicate recovery copy, but cannot lose the only draft.
        self.protect(&restored)?;
        self.remove(key)?;
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "river-recovery-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ))
    }
    #[test]
    fn restart_restores_untitled_without_touching_source() {
        let root = root();
        let source = root.with_extension("md");
        fs::write(&source, "original").unwrap();
        let recovery = Recovery::new(root.clone()).unwrap();
        let mut docs = Documents::default();
        let doc = docs.open(&source).unwrap();
        let doc = docs
            .update(doc.id, 1, "unsaved café 🎬".into(), TemplateId::Screenplay)
            .unwrap();
        recovery.protect(&doc).unwrap();
        let blank = docs.new_document();
        recovery
            .protect(
                &docs
                    .update(blank.id, 1, "untitled draft".into(), TemplateId::Manuscript)
                    .unwrap(),
            )
            .unwrap();
        assert!(recovery.pending().unwrap().is_empty());
        drop(recovery);
        fs::write(&source, "external edit").unwrap();
        let recovery = Recovery::new(root.clone()).unwrap();
        let pending = recovery.pending().unwrap();
        assert_eq!(pending.len(), 2);
        let mut restarted = Documents::default();
        for draft in pending {
            let restored = recovery.restore(&mut restarted, &draft.key).unwrap();
            assert!(restored.path.is_none());
            assert!(restored.dirty);
            assert_eq!(restored.text, draft.text);
            assert_eq!(restored.template, draft.template);
        }
        assert_eq!(fs::read_to_string(&source).unwrap(), "external edit");
        assert!(recovery.pending().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
    }
    #[test]
    fn an_emptied_named_story_is_recovered_as_a_dirty_copy() {
        let root = root();
        let source = root.with_extension("md");
        fs::write(&source, "remove this paragraph").unwrap();
        let recovery = Recovery::new(root.clone()).unwrap();
        let mut docs = Documents::default();
        let doc = docs.open(&source).unwrap();
        let doc = docs
            .update(doc.id, 1, String::new(), TemplateId::Manuscript)
            .unwrap();
        recovery.protect(&doc).unwrap();
        let restarted = Recovery::new(root.clone()).unwrap();
        let key = restarted.pending().unwrap()[0].key.clone();
        let doc = restarted.restore(&mut Documents::default(), &key).unwrap();
        assert!(doc.dirty && doc.text.is_empty() && doc.path.is_none());
        assert_eq!(
            fs::read_to_string(&source).unwrap(),
            "remove this paragraph"
        );
        assert_eq!(
            Recovery::new(root.clone())
                .unwrap()
                .pending()
                .unwrap()
                .len(),
            1
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(source).unwrap();
    }
    #[test]
    fn saved_and_explicitly_discarded_drafts_do_not_return() {
        let root = root();
        let recovery = Recovery::new(root.clone()).unwrap();
        let mut docs = Documents::default();
        let doc = docs.new_document();
        let doc = docs
            .update(doc.id, 1, "draft".into(), TemplateId::Manuscript)
            .unwrap();
        recovery.protect(&doc).unwrap();
        let clean = docs
            .update(doc.id, 2, "".into(), TemplateId::Manuscript)
            .unwrap();
        recovery.protect(&clean).unwrap();
        assert!(
            Recovery::new(root.clone())
                .unwrap()
                .pending()
                .unwrap()
                .is_empty()
        );
        recovery.protect(&doc).unwrap();
        recovery.forget(doc.id).unwrap();
        assert!(
            Recovery::new(root.clone())
                .unwrap()
                .pending()
                .unwrap()
                .is_empty()
        );
        assert!(recovery.remove("../../story").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
