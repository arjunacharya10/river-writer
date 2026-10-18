use river_service::recovery::{Draft, Recovery};
use river_service::{Document, Documents};
use river_templates::TemplateId;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
#[derive(Default)]
struct Desktop {
    documents: Mutex<Documents>,
    previews: Mutex<HashMap<u64, (u64, Arc<river_model::Publication>)>>,
    serial: AtomicU64,
    exit: AtomicBool,
}
fn lock<T>(m: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, String> {
    m.lock()
        .map_err(|_| "The document service stopped. Restart River.".into())
}
async fn work<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_status(root: PathBuf) -> Result<river_service::git::Status, String> {
    work(move || river_service::git::status(&root)).await
}
#[tauri::command]
async fn git_init(app: tauri::AppHandle, root: PathBuf) -> Result<(), String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _guard = lock(&state.documents)?;
        river_service::initialize_git(&root)
    })
    .await
}
#[tauri::command]
async fn git_identity(root: PathBuf, name: String, email: String) -> Result<(), String> {
    work(move || river_service::git::identity(&root, &name, &email)).await
}
#[tauri::command]
async fn git_log(
    root: PathBuf,
    before: Option<String>,
) -> Result<Vec<river_service::git::Commit>, String> {
    work(move || river_service::git::log(&root, before.as_deref())).await
}
#[tauri::command]
async fn git_diff(root: PathBuf, id: String) -> Result<river_service::git::Review, String> {
    work(move || river_service::git::commit_diff(&root, &id)).await
}
#[tauri::command]
async fn git_commit(
    app: tauri::AppHandle,
    root: PathBuf,
    head: Option<String>,
    tree: String,
    message: String,
) -> Result<String, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _guard = lock(&state.documents)?;
        river_service::git::commit(&root, head.as_deref(), &tree, &message)
    })
    .await
}
#[tauri::command]
async fn new_document(app: tauri::AppHandle) -> Result<Document, String> {
    work(move || Ok(lock(&app.state::<Desktop>().documents)?.new_document())).await
}
#[tauri::command]
async fn open_document(
    app: tauri::AppHandle,
    path: Option<PathBuf>,
) -> Result<Option<Document>, String> {
    work(move || {
        let path = match path {
            Some(p) => p,
            None => match app
                .dialog()
                .file()
                .add_filter("Stories", &["fountain", "md"])
                .blocking_pick_file()
            {
                Some(p) => p.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            },
        };
        Ok(Some(lock(&app.state::<Desktop>().documents)?.open(&path)?))
    })
    .await
}
#[tauri::command]
async fn open_folder(
    app: tauri::AppHandle,
) -> Result<Option<(PathBuf, Vec<river_service::FileEntry>)>, String> {
    work(move || {
        let Some(path) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let entries = river_service::list_files(&path)?;
        lock(&app.state::<Desktop>().documents)?.set_project(&path)?;
        Ok(Some((path, entries)))
    })
    .await
}
#[tauri::command]
async fn recent_projects(
    app: tauri::AppHandle,
) -> Result<Vec<river_service::projects::RecentProject>, String> {
    work(move || {
        river_service::projects::recents(&app.path().app_config_dir().map_err(|e| e.to_string())?)
    })
    .await
}
#[tauri::command]
async fn open_project(
    app: tauri::AppHandle,
    path: Option<PathBuf>,
) -> Result<Option<river_service::projects::Project>, String> {
    work(move || {
        let path = match path {
            Some(p) => p,
            None => match app
                .dialog()
                .file()
                .set_title("Open Project")
                .blocking_pick_folder()
            {
                Some(p) => p.into_path().map_err(|e| e.to_string())?,
                None => return Ok(None),
            },
        };
        let project = river_service::projects::open(&path)?;
        if let Ok(config) = app.path().app_config_dir() {
            let _ = river_service::projects::remember(&config, &project);
        }
        lock(&app.state::<Desktop>().documents)?.set_project(&project.path)?;
        Ok(Some(project))
    })
    .await
}
#[tauri::command]
async fn create_project(
    app: tauri::AppHandle,
) -> Result<Option<river_service::projects::Project>, String> {
    work(move || {
        let Some(path) = app
            .dialog()
            .file()
            .set_title("New Project")
            .set_file_name("Untitled")
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let project =
            river_service::projects::create(&path.into_path().map_err(|e| e.to_string())?)?;
        // Recent-location persistence is not allowed to turn successful creation into an error.
        if let Ok(config) = app.path().app_config_dir() {
            let _ = river_service::projects::remember(&config, &project);
        }
        lock(&app.state::<Desktop>().documents)?.set_project(&project.path)?;
        Ok(Some(project))
    })
    .await
}
#[tauri::command]
async fn save_project_session(
    app: tauri::AppHandle,
    path: PathBuf,
    session: river_service::projects::Session,
) -> Result<(), String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _documents = lock(&state.documents)?;
        river_service::projects::save_session(&path, session)
    })
    .await
}
#[tauri::command]
async fn refresh_folder(path: PathBuf) -> Result<Vec<river_service::FileEntry>, String> {
    work(move || river_service::list_files(&path)).await
}
#[tauri::command]
async fn load_todos(root: PathBuf) -> Result<river_service::todos::Todos, String> {
    work(move || river_service::todos::load(&root)).await
}
#[tauri::command]
async fn update_todos(
    app: tauri::AppHandle,
    root: PathBuf,
    expected: Option<String>,
    change: river_service::todos::Change,
) -> Result<river_service::todos::Todos, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _documents = lock(&state.documents)?;
        river_service::todos::update(&root, expected.as_deref(), change)
    })
    .await
}
#[tauri::command]
async fn load_scratchpad(root: PathBuf) -> Result<river_service::extras::Scratchpad, String> {
    work(move || river_service::extras::scratchpad(&root)).await
}
#[tauri::command]
async fn protect_scratchpad(
    app: tauri::AppHandle,
    root: PathBuf,
    text: String,
) -> Result<(), String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _guard = lock(&state.documents)?;
        let path = river_service::extras::scratch_path(&root)?;
        if river_service::extras::scratchpad(&root)?.text == text {
            app.state::<Recovery>().forget_note(&path)
        } else {
            app.state::<Recovery>().protect_note(&path, &text)
        }
    })
    .await
}
#[tauri::command]
async fn save_scratchpad(
    app: tauri::AppHandle,
    root: PathBuf,
    expected: Option<String>,
    text: String,
) -> Result<river_service::extras::Scratchpad, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _guard = lock(&state.documents)?;
        let path = river_service::extras::scratch_path(&root)?;
        app.state::<Recovery>().protect_note(&path, &text)?;
        let saved = river_service::extras::save_scratchpad(&root, expected.as_deref(), &text)?;
        app.state::<Recovery>().forget_note(&path)?;
        Ok(saved)
    })
    .await
}
#[tauri::command]
async fn settle_scratchpad(
    app: tauri::AppHandle,
    root: PathBuf,
    expected: Option<String>,
) -> Result<(), String> {
    work(move || {
        let state = app.state::<Desktop>();
        let _guard = lock(&state.documents)?;
        if river_service::extras::scratchpad(&root)?.revision != expected {
            return Err("Scratchpad changed again. Reload to see the latest text.".into());
        }
        app.state::<Recovery>()
            .forget_note(&river_service::extras::scratch_path(&root)?)
    })
    .await
}
#[tauri::command]
async fn copy_scratchpad(
    app: tauri::AppHandle,
    root: PathBuf,
    text: String,
) -> Result<bool, String> {
    work(move || {
        let Some(path) = app
            .dialog()
            .file()
            .set_title("Save Scratchpad Copy")
            .set_directory(root)
            .set_file_name("Scratchpad copy.md")
            .add_filter("Markdown", &["md"])
            .blocking_save_file()
        else {
            return Ok(false);
        };
        river_service::extras::save_copy(&path.into_path().map_err(|e| e.to_string())?, &text)?;
        Ok(true)
    })
    .await
}
#[tauri::command]
async fn sync_document(
    app: tauri::AppHandle,
    id: u64,
    revision: u64,
    text: String,
    template: TemplateId,
) -> Result<river_service::Analysis, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let mut docs = lock(&state.documents)?;
        // A failed journal write can be retried without replaying an edit.
        let current = docs.get(id)?;
        let doc =
            if current.revision == revision && current.text == text && current.template == template
            {
                current.clone()
            } else {
                docs.update(id, revision, text, template)?
            };
        app.state::<Recovery>().protect(&doc)?;
        Ok(river_service::analyze(&doc))
    })
    .await
}
#[tauri::command]
async fn analyze_document(
    app: tauri::AppHandle,
    id: u64,
) -> Result<river_service::Analysis, String> {
    work(move || {
        let doc = lock(&app.state::<Desktop>().documents)?.get(id)?.clone();
        Ok(river_service::analyze(&doc))
    })
    .await
}
#[tauri::command]
async fn save_document(
    app: tauri::AppHandle,
    id: u64,
    save_as: bool,
) -> Result<Option<Document>, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let doc = lock(&state.documents)?.get(id)?.clone();
        let target = if save_as || doc.path.is_none() {
            let name = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled.md");
            let dialog = app
                .dialog()
                .file()
                .add_filter("Stories", &["md", "fountain"])
                .set_file_name(name);
            let dialog = if let Some(parent) = doc.path.as_ref().and_then(|p| p.parent()) {
                dialog.set_directory(parent)
            } else {
                let project = lock(&state.documents)?
                    .project_dir()
                    .map(|p| p.to_path_buf());
                if let Some(path) = project {
                    dialog.set_directory(path)
                } else {
                    dialog
                }
            };
            let Some(p) = dialog.blocking_save_file() else {
                return Ok(None);
            };
            Some(p.into_path().map_err(|e| e.to_string())?)
        } else {
            None
        };
        let mut docs = lock(&state.documents)?;
        let doc = docs.save(id, target.as_deref())?;
        app.state::<Recovery>().protect(&doc)?;
        Ok(Some(doc))
    })
    .await
}
#[tauri::command]
async fn persist_template(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    work(move || lock(&app.state::<Desktop>().documents)?.persist_template(id)).await
}
#[tauri::command]
async fn persist_saved_template(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    work(move || lock(&app.state::<Desktop>().documents)?.persist_saved_template(id)).await
}
#[tauri::command]
async fn external_change(app: tauri::AppHandle, id: u64) -> Result<bool, String> {
    work(move || lock(&app.state::<Desktop>().documents)?.external_change(id)).await
}
#[tauri::command]
async fn reload_document(app: tauri::AppHandle, id: u64) -> Result<Document, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let mut docs = lock(&state.documents)?;
        let doc = docs.reload(id)?;
        app.state::<Recovery>().protect(&doc)?;
        Ok(doc)
    })
    .await
}
#[tauri::command]
async fn recovery_drafts(app: tauri::AppHandle) -> Result<Vec<Draft>, String> {
    work(move || app.state::<Recovery>().pending()).await
}
#[tauri::command]
async fn autosave_document(
    app: tauri::AppHandle,
    id: u64,
    revision: u64,
) -> Result<Option<Document>, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let mut docs = lock(&state.documents)?;
        let doc = docs.autosave(id, revision)?;
        if let Some(doc) = &doc {
            app.state::<Recovery>().protect(doc)?;
            docs.persist_saved_template(id)?;
        }
        Ok(doc)
    })
    .await
}
#[tauri::command]
async fn local_history(
    app: tauri::AppHandle,
    id: u64,
    before: Option<String>,
) -> Result<Vec<river_service::HistoryVersion>, String> {
    work(move || {
        lock(&app.state::<Desktop>().documents)?.local_history_before(id, before.as_deref())
    })
    .await
}
#[tauri::command]
async fn history_text(app: tauri::AppHandle, id: u64, version: String) -> Result<String, String> {
    work(move || lock(&app.state::<Desktop>().documents)?.history_text(id, &version)).await
}
#[tauri::command]
async fn prepare_restore(
    app: tauri::AppHandle,
    id: u64,
    revision: u64,
    version: String,
) -> Result<String, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let docs = lock(&state.documents)?;
        docs.before_restore(id, revision)?;
        docs.history_text(id, &version)
    })
    .await
}
#[tauri::command]
async fn recover_draft(app: tauri::AppHandle, key: String) -> Result<Document, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let mut docs = lock(&state.documents)?;
        app.state::<Recovery>().restore(&mut docs, &key)
    })
    .await
}
#[tauri::command]
fn close_document(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<Desktop>();
    let mut docs = lock(&state.documents)?;
    app.state::<Recovery>().forget(id)?;
    docs.close(id);
    lock(&state.previews)?.remove(&id);
    Ok(())
}
#[derive(Serialize)]
struct Preview {
    token: u64,
    revision: u64,
    pages: usize,
}
#[tauri::command]
async fn paginate(app: tauri::AppHandle, id: u64) -> Result<Preview, String> {
    work(move || {
        let state = app.state::<Desktop>();
        let (doc, buffers) = {
            let docs = lock(&state.documents)?;
            (docs.get(id)?.clone(), docs.buffers())
        };
        let token = state.serial.fetch_add(1, Ordering::Relaxed) + 1;
        thread_local! {
            static ENGINE: std::cell::RefCell<river_text::TextEngine> = std::cell::RefCell::new(river_text::TextEngine::new());
        }
        let publication = ENGINE.with(|engine| {
            river_service::publication(&mut engine.borrow_mut(), &doc, &buffers)
        })?;
        let pages = publication.pages.len();
        let documents = lock(&state.documents)?;
        if documents.get(id)?.revision != doc.revision {
            return Err("Preview superseded by a newer edit".into());
        }
        let mut previews = lock(&state.previews)?;
        if previews.get(&id).is_some_and(|(current, _)| *current > token) {
            return Err("Preview superseded by a newer request".into());
        }
        previews.insert(id, (token, Arc::new(publication)));
        Ok(Preview { token, revision: doc.revision, pages })
    }).await
}
#[tauri::command]
async fn render_page(
    app: tauri::AppHandle,
    id: u64,
    token: u64,
    page: usize,
) -> Result<tauri::ipc::Response, String> {
    work(move || {
        let publication = {
            let state = app.state::<Desktop>();
            let previews = lock(&state.previews)?;
            let (current, p) = previews.get(&id).ok_or("Preview expired")?;
            if *current != token {
                return Err("Preview expired".into());
            }
            p.clone()
        };
        let page = publication.pages.get(page).ok_or("Page does not exist")?;
        let png = river_render::page(page, 144.0)?
            .encode_png()
            .map_err(|e| e.to_string())?;
        Ok(tauri::ipc::Response::new(png))
    })
    .await
}
#[tauri::command]
async fn export_pdf(app: tauri::AppHandle, id: u64) -> Result<Option<PathBuf>, String> {
    work(move || {
        let (doc, buffers) = {
            let state = app.state::<Desktop>();
            let docs = lock(&state.documents)?;
            (docs.get(id)?.clone(), docs.buffers())
        };
        let stem = doc
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        let mut dialog = app
            .dialog()
            .file()
            .add_filter("PDF", &["pdf"])
            .set_file_name(format!("{stem}.pdf"));
        if let Some(parent) = doc.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(parent)
        }
        let Some(target) = dialog.blocking_save_file() else {
            return Ok(None);
        };
        let path = target.into_path().map_err(|e| e.to_string())?;
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            return Err("Choose a .pdf file name".into());
        }
        let parsed = river_service::script(&doc, &buffers)?;
        // No source file, including a nested include, may be used as a PDF target.
        if let Ok(target) = path.canonicalize() {
            for source in buffers
                .keys()
                .chain(parsed.files.iter().filter_map(|f| f.path.as_ref()))
            {
                if target == *source {
                    return Err("Choose a different PDF path".into());
                }
            }
        }
        let expected = std::fs::read(&path).ok();
        let publication =
            river_service::publication(&mut river_text::TextEngine::new(), &doc, &buffers)?;
        let bytes = river_pdf::encode(&publication)?;
        river_service::atomic_write(&path, &bytes, expected.as_deref())?;
        Ok(Some(path))
    })
    .await
}
#[tauri::command]
async fn confirm_discard(app: tauri::AppHandle, name: String) -> Result<bool, String> {
    work(move || {
        Ok(app
            .dialog()
            .message(format!(
                "Discard unsaved changes to {name}? Your saved file will be kept."
            ))
            .title("Unsaved changes")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Discard Changes".into(),
                "Keep Editing".into(),
            ))
            .blocking_show())
    })
    .await
}
#[tauri::command]
fn quit_confirmed(app: tauri::AppHandle) {
    app.state::<Desktop>().exit.store(true, Ordering::Relaxed);
    app.exit(0)
}
fn main() {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem as Native, Submenu};
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Desktop::default())
        .setup(|app| {
            app.manage(Recovery::new(app.path().app_data_dir()?.join("recovery"))?);
            lock(&app.state::<Desktop>().documents)?.enable_history(app.path().app_data_dir()?);
            let item =
                |id: &str, text: &str, key: &str| MenuItem::with_id(app, id, text, true, Some(key));
            let app_menu = Submenu::with_items(
                app,
                "River Writer",
                true,
                &[
                    &Native::about(
                        app,
                        None,
                        Some(tauri::menu::AboutMetadata {
                            name: Some("River Writer".into()),
                            version: Some("0.1.0 · Development".into()),
                            license: Some(
                                "River Writer: MIT. Courier Prime: SIL Open Font License 1.1."
                                    .into(),
                            ),
                            ..Default::default()
                        }),
                    )?,
                    &Native::separator(app)?,
                    &Native::hide(app, None)?,
                    &Native::hide_others(app, None)?,
                    &Native::separator(app)?,
                    &item("quit", "Quit River Writer", "CmdOrCtrl+Q")?,
                ],
            )?;
            let file = Submenu::with_items(
                app,
                "File",
                true,
                &[
                    &item("new-project", "New Project…", "CmdOrCtrl+Shift+N")?,
                    &item("folder", "Open Project…", "CmdOrCtrl+Shift+O")?,
                    &item("close-project", "Close Project", "CmdOrCtrl+Shift+W")?,
                    &Native::separator(app)?,
                    &item("new", "New File", "CmdOrCtrl+N")?,
                    &item("open", "Open…", "CmdOrCtrl+O")?,
                    &Native::separator(app)?,
                    &item("save", "Save", "CmdOrCtrl+S")?,
                    &item("save-as", "Save As…", "CmdOrCtrl+Shift+S")?,
                    &item("close", "Close Tab", "CmdOrCtrl+W")?,
                    &Native::separator(app)?,
                    &item("export", "Export PDF…", "CmdOrCtrl+Alt+E")?,
                ],
            )?;
            let edit = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &Native::undo(app, None)?,
                    &Native::redo(app, None)?,
                    &Native::separator(app)?,
                    &Native::cut(app, None)?,
                    &Native::copy(app, None)?,
                    &Native::paste(app, None)?,
                    &Native::select_all(app, None)?,
                    &item("find", "Find", "CmdOrCtrl+F")?,
                ],
            )?;
            let view = Submenu::with_items(
                app,
                "View",
                true,
                &[
                    &item("palette", "Command Palette…", "CmdOrCtrl+Shift+P")?,
                    &item("preview", "Toggle Preview", "CmdOrCtrl+Shift+V")?,
                    &item("sidebar", "Toggle Sidebar", "CmdOrCtrl+B")?,
                    &item("theme", "Toggle Light / Dark", "CmdOrCtrl+Alt+T")?,
                ],
            )?;
            let window = Submenu::with_items(
                app,
                "Window",
                true,
                &[&Native::minimize(app, None)?, &Native::maximize(app, None)?],
            )?;
            app.set_menu(Menu::with_items(
                app,
                &[&app_menu, &file, &edit, &view, &window],
            )?)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("river:command", event.id().as_ref());
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && !window.state::<Desktop>().exit.load(Ordering::Relaxed)
            {
                api.prevent_close();
                let _ = window.emit("river:command", "quit");
            }
        })
        .invoke_handler(tauri::generate_handler![
            git_status,
            git_init,
            git_identity,
            git_log,
            git_diff,
            git_commit,
            new_document,
            open_document,
            open_folder,
            refresh_folder,
            settle_scratchpad,
            load_scratchpad,
            protect_scratchpad,
            save_scratchpad,
            copy_scratchpad,
            load_todos,
            update_todos,
            recent_projects,
            create_project,
            open_project,
            save_project_session,
            sync_document,
            recovery_drafts,
            recover_draft,
            autosave_document,
            local_history,
            history_text,
            prepare_restore,
            analyze_document,
            save_document,
            persist_template,
            persist_saved_template,
            external_change,
            reload_document,
            close_document,
            paginate,
            render_page,
            export_pdf,
            confirm_discard,
            quit_confirmed
        ])
        .build(tauri::generate_context!())
        .expect("River could not start");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && !app.state::<Desktop>().exit.load(Ordering::Relaxed)
        {
            api.prevent_exit();
            let _ = app.emit("river:command", "quit");
        }
    });
}
