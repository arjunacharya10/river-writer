//! The one table of commands the workbench can call. The desktop app and the
//! browser-test host both dispatch through [`Service::call`]; only the [`Shell`]
//! (native dialogs and app folders) differs between them.
use crate::{Documents, extras, git, projects, recovery::Recovery, todos};
use river_model::Publication;
use river_templates::TemplateId;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
};

/// Everything that needs the operating system's UI. `None`/`false` means the
/// writer cancelled.
pub trait Shell: Send + Sync {
    fn pick_story(&self) -> Option<PathBuf>;
    fn pick_project(&self) -> Option<PathBuf>;
    fn new_project_location(&self) -> Option<PathBuf>;
    /// Save As for a story; `name` is the suggested file name.
    fn save_story(&self, name: &str, folder: Option<&Path>) -> Option<PathBuf>;
    fn save_pdf(&self, name: &str, folder: Option<&Path>) -> Option<PathBuf>;
    fn save_scratchpad_copy(&self, folder: &Path) -> Option<PathBuf>;
    fn confirm_discard(&self, name: &str) -> bool;
    /// Where recent projects are remembered, if available.
    fn config_dir(&self) -> Option<PathBuf>;
}

#[derive(Debug)]
pub enum Reply {
    Json(Value),
    /// Binary payloads (page images) skip JSON encoding.
    Bytes(Vec<u8>),
}

pub struct Service {
    documents: Mutex<Documents>,
    previews: Mutex<HashMap<u64, (u64, Arc<Publication>)>>,
    serial: AtomicU64,
    recovery: Recovery,
    engine: Mutex<river_text::TextEngine>,
    shell: Box<dyn Shell>,
}

fn lock<T>(m: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    m.lock()
        .map_err(|_| "The document service stopped. Restart River.".into())
}
fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("Invalid {key}: {e}"))
}
fn reply<T: Serialize>(value: T) -> Result<Reply, String> {
    serde_json::to_value(value)
        .map(Reply::Json)
        .map_err(|e| e.to_string())
}
const DONE: Result<Reply, String> = Ok(Reply::Json(Value::Null));

impl Service {
    /// `data` holds private history and crash-recovery journals.
    pub fn new(data: PathBuf, shell: Box<dyn Shell>) -> Result<Self, String> {
        let mut documents = Documents::default();
        documents.enable_history(data.clone());
        Ok(Self {
            documents: Mutex::new(documents),
            previews: Mutex::default(),
            serial: AtomicU64::new(0),
            recovery: Recovery::new(data.join("recovery"))?,
            engine: Mutex::new(river_text::TextEngine::new()),
            shell,
        })
    }

    pub fn call(&self, name: &str, args: &Value) -> Result<Reply, String> {
        let root = || arg::<PathBuf>(args, "root");
        let id = || arg::<u64>(args, "id");
        let docs = || lock(&self.documents);
        match name {
            // Git. Commands that change files hold the document lock so they
            // never interleave with saves.
            "git_status" => reply(git::status(&root()?)?),
            "git_review" => reply(git::review_changes(&root()?)?),
            "git_log" => reply(git::log(
                &root()?,
                arg::<Option<String>>(args, "before")?.as_deref(),
            )?),
            "git_diff" => reply(git::commit_diff(&root()?, &arg::<String>(args, "id")?)?),
            "git_branches" => reply(git::branches(&root()?)?),
            "git_identity" => {
                git::identity(
                    &root()?,
                    &arg::<String>(args, "name")?,
                    &arg::<String>(args, "email")?,
                )?;
                DONE
            }
            "git_init" => {
                let _guard = docs()?;
                crate::initialize_git(&root()?)?;
                DONE
            }
            "git_commit" => {
                let _guard = docs()?;
                reply(git::commit(
                    &root()?,
                    arg::<Option<String>>(args, "head")?.as_deref(),
                    &arg::<String>(args, "tree")?,
                    &arg::<String>(args, "message")?,
                )?)
            }
            "git_create_branch" => {
                let _guard = docs()?;
                git::create_branch(&root()?, &arg::<String>(args, "name")?)?;
                DONE
            }
            "git_switch_branch" => {
                reply(docs()?.git_switch_branch(&root()?, &arg::<String>(args, "name")?)?)
            }
            "git_restore" => reply(docs()?.git_restore(
                &root()?,
                &arg::<String>(args, "commit")?,
                arg::<Option<String>>(args, "path")?.as_deref(),
            )?),
            "git_undo_restore" => {
                docs()?.git_undo_restore(&root()?, &arg(args, "restored")?)?;
                DONE
            }

            // Projects.
            "recent_projects" => match self.shell.config_dir() {
                Some(config) => reply(projects::recents(&config)?),
                None => reply(Vec::<projects::RecentProject>::new()),
            },
            "open_project" => {
                let path = match arg::<Option<PathBuf>>(args, "path")? {
                    Some(path) => path,
                    None => match self.shell.pick_project() {
                        Some(path) => path,
                        None => return reply(Value::Null),
                    },
                };
                self.enter_project(projects::open(&path)?)
            }
            "create_project" => match self.shell.new_project_location() {
                Some(path) => self.enter_project(projects::create(&path)?),
                None => reply(Value::Null),
            },
            "save_project_session" => {
                let _guard = docs()?;
                projects::save_session(&arg::<PathBuf>(args, "path")?, arg(args, "session")?)?;
                DONE
            }
            "refresh_folder" => reply(crate::list_files(&arg::<PathBuf>(args, "path")?)?),

            // Todos and Scratchpad.
            "load_todos" => reply(todos::load(&root()?)?),
            "update_todos" => {
                let _guard = docs()?;
                reply(todos::update(
                    &root()?,
                    arg::<Option<String>>(args, "expected")?.as_deref(),
                    arg(args, "change")?,
                )?)
            }
            "load_scratchpad" => reply(extras::scratchpad(&root()?)?),
            "protect_scratchpad" => {
                let _guard = docs()?;
                let (root, text) = (root()?, arg::<String>(args, "text")?);
                let path = extras::scratch_path(&root)?;
                if extras::scratchpad(&root)?.text == text {
                    self.recovery.forget_note(&path)?;
                } else {
                    self.recovery.protect_note(&path, &text)?;
                }
                DONE
            }
            "save_scratchpad" => {
                let _guard = docs()?;
                let (root, text) = (root()?, arg::<String>(args, "text")?);
                let path = extras::scratch_path(&root)?;
                self.recovery.protect_note(&path, &text)?;
                let saved = extras::save_scratchpad(
                    &root,
                    arg::<Option<String>>(args, "expected")?.as_deref(),
                    &text,
                )?;
                self.recovery.forget_note(&path)?;
                reply(saved)
            }
            "settle_scratchpad" => {
                let _guard = docs()?;
                let root = root()?;
                if extras::scratchpad(&root)?.revision != arg::<Option<String>>(args, "expected")? {
                    return Err("Scratchpad changed again. Reload to see the latest text.".into());
                }
                self.recovery.forget_note(&extras::scratch_path(&root)?)?;
                DONE
            }
            "copy_scratchpad" => {
                let text = arg::<String>(args, "text")?;
                match self.shell.save_scratchpad_copy(&root()?) {
                    Some(path) => {
                        extras::save_copy(&path, &text)?;
                        reply(true)
                    }
                    None => reply(false),
                }
            }

            // Documents.
            "new_document" => reply(docs()?.new_document()),
            "open_document" => {
                let path = match arg::<Option<PathBuf>>(args, "path")? {
                    Some(path) => path,
                    None => match self.shell.pick_story() {
                        Some(path) => path,
                        None => return reply(Value::Null),
                    },
                };
                reply(docs()?.open(&path)?)
            }
            "sync_document" => {
                let (id, revision) = (id()?, arg::<u64>(args, "revision")?);
                let (text, template) = (
                    arg::<String>(args, "text")?,
                    arg::<TemplateId>(args, "template")?,
                );
                let mut docs = docs()?;
                // A failed journal write can be retried without replaying an edit.
                let current = docs.get(id)?;
                let doc = if current.revision == revision
                    && current.text == text
                    && current.template == template
                {
                    current.clone()
                } else {
                    docs.update(id, revision, text, template)?
                };
                self.recovery.protect(&doc)?;
                reply(crate::analyze(&doc))
            }
            "analyze_document" => {
                let doc = docs()?.get(id()?)?.clone();
                reply(crate::analyze(&doc))
            }
            "save_document" => {
                self.save_document(id()?, arg::<Option<bool>>(args, "saveAs")?.unwrap_or(false))
            }
            "autosave_document" => {
                let id = id()?;
                let mut docs = docs()?;
                let doc = docs.autosave(id, arg(args, "revision")?)?;
                if let Some(doc) = &doc {
                    self.recovery.protect(doc)?;
                    docs.persist_saved_template(id)?;
                }
                reply(doc)
            }
            "persist_template" => {
                docs()?.persist_template(id()?)?;
                DONE
            }
            "persist_saved_template" => {
                docs()?.persist_saved_template(id()?)?;
                DONE
            }
            "external_change" => reply(docs()?.external_change(id()?)?),
            "reload_document" => {
                let doc = docs()?.reload(id()?)?;
                self.recovery.protect(&doc)?;
                reply(doc)
            }
            "close_document" => {
                let id = id()?;
                let mut docs = docs()?;
                self.recovery.forget(id)?;
                docs.close(id);
                lock(&self.previews)?.remove(&id);
                DONE
            }
            "confirm_discard" => reply(self.shell.confirm_discard(&arg::<String>(args, "name")?)),

            // Crash recovery and private history.
            "recovery_drafts" => reply(self.recovery.pending()?),
            "recover_draft" => {
                let key = arg::<String>(args, "key")?;
                reply(self.recovery.restore(&mut *docs()?, &key)?)
            }
            "local_history" => {
                reply(docs()?.local_history_before(
                    id()?,
                    arg::<Option<String>>(args, "before")?.as_deref(),
                )?)
            }
            "history_text" => reply(docs()?.history_text(id()?, &arg::<String>(args, "version")?)?),
            "prepare_restore" => {
                let (id, version) = (id()?, arg::<String>(args, "version")?);
                let docs = docs()?;
                docs.before_restore(id, arg(args, "revision")?)?;
                reply(docs.history_text(id, &version)?)
            }

            // Pages.
            "paginate" => self.paginate(id()?),
            "render_page" => self.render_page(id()?, arg(args, "token")?, arg(args, "page")?),
            "export_pdf" => self.export_pdf(id()?),
            other => Err(format!("Unknown command {other}")),
        }
    }

    fn enter_project(&self, project: projects::Project) -> Result<Reply, String> {
        // Remembering a recent location must never turn a successful open into an error.
        if let Some(config) = self.shell.config_dir() {
            let _ = projects::remember(&config, &project);
        }
        lock(&self.documents)?.set_project(&project.path)?;
        reply(project)
    }

    fn save_document(&self, id: u64, save_as: bool) -> Result<Reply, String> {
        let (doc, project) = {
            let docs = lock(&self.documents)?;
            (
                docs.get(id)?.clone(),
                docs.project_dir().map(Path::to_path_buf),
            )
        };
        // The dialog runs without the lock so typing and autosave continue meanwhile.
        let target = if save_as || doc.path.is_none() {
            let name = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled.md");
            let folder = doc
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .or(project.as_deref());
            match self.shell.save_story(name, folder) {
                Some(path) => Some(path),
                None => return reply(Value::Null),
            }
        } else {
            None
        };
        let doc = lock(&self.documents)?.save(id, target.as_deref())?;
        self.recovery.protect(&doc)?;
        reply(doc)
    }

    fn paginate(&self, id: u64) -> Result<Reply, String> {
        let (doc, buffers) = {
            let docs = lock(&self.documents)?;
            (docs.get(id)?.clone(), docs.buffers())
        };
        let token = self.serial.fetch_add(1, Ordering::Relaxed) + 1;
        let publication = crate::publication(&mut *lock(&self.engine)?, &doc, &buffers)?;
        let pages = publication.pages.len();
        let docs = lock(&self.documents)?;
        // A result for a closed or edited document must not replace its current preview.
        if docs.get(id)?.revision != doc.revision {
            return Err("Preview superseded by a newer edit".into());
        }
        let mut previews = lock(&self.previews)?;
        if previews
            .get(&id)
            .is_some_and(|(current, _)| *current > token)
        {
            return Err("Preview superseded by a newer request".into());
        }
        previews.insert(id, (token, Arc::new(publication)));
        reply(json!({ "token": token, "revision": doc.revision, "pages": pages }))
    }

    fn render_page(&self, id: u64, token: u64, page: usize) -> Result<Reply, String> {
        let publication = {
            let previews = lock(&self.previews)?;
            let (current, publication) = previews.get(&id).ok_or("Preview expired")?;
            if *current != token {
                return Err("Preview expired".into());
            }
            publication.clone()
        };
        let page = publication.pages.get(page).ok_or("Page does not exist")?;
        let png = river_render::page(page, 144.0)?
            .encode_png()
            .map_err(|e| e.to_string())?;
        Ok(Reply::Bytes(png))
    }

    fn export_pdf(&self, id: u64) -> Result<Reply, String> {
        let (doc, buffers) = {
            let docs = lock(&self.documents)?;
            (docs.get(id)?.clone(), docs.buffers())
        };
        let stem = doc
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        let Some(path) = self.shell.save_pdf(
            &format!("{stem}.pdf"),
            doc.path.as_ref().and_then(|p| p.parent()),
        ) else {
            return reply(Value::Null);
        };
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            return Err("Choose a .pdf file name".into());
        }
        let parsed = crate::script(&doc, &buffers)?;
        // No source file, including a nested include, may be used as a PDF target.
        if let Ok(target) = path.canonicalize()
            && buffers
                .keys()
                .chain(parsed.files.iter().filter_map(|f| f.path.as_ref()))
                .any(|source| *source == target)
        {
            return Err("Choose a different PDF path".into());
        }
        let expected = std::fs::read(&path).ok();
        let publication = crate::publication(&mut river_text::TextEngine::new(), &doc, &buffers)?;
        crate::atomic_write(
            &path,
            &river_pdf::encode(&publication)?,
            expected.as_deref(),
        )?;
        reply(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every dialog cancelled.
    struct Cancel;
    impl Shell for Cancel {
        fn pick_story(&self) -> Option<PathBuf> {
            None
        }
        fn pick_project(&self) -> Option<PathBuf> {
            None
        }
        fn new_project_location(&self) -> Option<PathBuf> {
            None
        }
        fn save_story(&self, _: &str, _: Option<&Path>) -> Option<PathBuf> {
            None
        }
        fn save_pdf(&self, _: &str, _: Option<&Path>) -> Option<PathBuf> {
            None
        }
        fn save_scratchpad_copy(&self, _: &Path) -> Option<PathBuf> {
            None
        }
        fn confirm_discard(&self, _: &str) -> bool {
            false
        }
        fn config_dir(&self) -> Option<PathBuf> {
            None
        }
    }
    fn json(reply: Result<Reply, String>) -> Value {
        match reply.unwrap() {
            Reply::Json(value) => value,
            Reply::Bytes(_) => panic!("unexpected bytes"),
        }
    }
    #[test]
    fn cancelled_dialogs_change_nothing_and_unknown_commands_fail() {
        let data = std::env::temp_dir().join(format!(
            "river-commands-{}-{}",
            std::process::id(),
            crate::TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let service = Service::new(data.clone(), Box::new(Cancel)).unwrap();
        for name in ["open_document", "open_project", "create_project"] {
            assert_eq!(json(service.call(name, &json!({}))), Value::Null, "{name}");
        }
        let doc = json(service.call("new_document", &json!({})));
        let id = doc["id"].clone();
        json(service.call(
            "sync_document",
            &json!({"id": id, "revision": 1, "text": "Draft", "template": "manuscript"}),
        ));
        // Retrying the same revision (after a failed journal write) is accepted.
        json(service.call(
            "sync_document",
            &json!({"id": id, "revision": 1, "text": "Draft", "template": "manuscript"}),
        ));
        assert_eq!(
            json(service.call("save_document", &json!({"id": id}))),
            Value::Null
        );
        assert_eq!(
            json(service.call("export_pdf", &json!({"id": id}))),
            Value::Null
        );
        assert_eq!(json(service.call("recent_projects", &json!({}))), json!([]));
        assert!(service.call("no_such_command", &json!({})).is_err());
        assert!(
            service
                .call("close_document", &json!({}))
                .unwrap_err()
                .contains("id")
        );
        std::fs::remove_dir_all(data).ok();
    }
}
