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
async fn refresh_folder(path: PathBuf) -> Result<Vec<river_service::FileEntry>, String> {
    work(move || river_service::list_files(&path)).await
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
        let doc = lock(&state.documents)?.update(id, revision, text, template)?;
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
                dialog
            };
            let Some(p) = dialog.blocking_save_file() else {
                return Ok(None);
            };
            Some(p.into_path().map_err(|e| e.to_string())?)
        } else {
            None
        };
        let mut docs = lock(&state.documents)?;
        Ok(Some(docs.save(id, target.as_deref())?))
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
    work(move || lock(&app.state::<Desktop>().documents)?.reload(id)).await
}
#[tauri::command]
fn close_document(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let state = app.state::<Desktop>();
    lock(&state.documents)?.close(id);
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
                            license: Some("Courier Prime: SIL Open Font License 1.1".into()),
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
                    &item("new", "New File", "CmdOrCtrl+N")?,
                    &item("open", "Open…", "CmdOrCtrl+O")?,
                    &item("folder", "Open Folder…", "CmdOrCtrl+Shift+O")?,
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
            new_document,
            open_document,
            open_folder,
            refresh_folder,
            sync_document,
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
