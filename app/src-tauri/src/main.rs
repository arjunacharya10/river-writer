use river_service::commands::{Reply, Service, Shell};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

/// App-shell state that the command table does not own.
#[derive(Default)]
struct Desktop {
    exit: AtomicBool,
    /// Story files macOS asked River to open (Finder, Dock, Open With) not yet shown.
    opened: Mutex<Vec<PathBuf>>,
}

/// Native Mac dialogs and folders for the shared command table.
struct MacShell(tauri::AppHandle);
fn chosen(path: Option<tauri_plugin_dialog::FilePath>) -> Option<PathBuf> {
    path.and_then(|p| p.into_path().ok())
}
impl Shell for MacShell {
    fn pick_story(&self) -> Option<PathBuf> {
        chosen(
            self.0
                .dialog()
                .file()
                .add_filter("Stories", &["fountain", "md"])
                .blocking_pick_file(),
        )
    }
    fn pick_project(&self) -> Option<PathBuf> {
        chosen(
            self.0
                .dialog()
                .file()
                .set_title("Open Project")
                .blocking_pick_folder(),
        )
    }
    fn new_project_location(&self) -> Option<PathBuf> {
        chosen(
            self.0
                .dialog()
                .file()
                .set_title("New Project")
                .set_file_name("Untitled")
                .blocking_save_file(),
        )
    }
    fn save_story(&self, name: &str, folder: Option<&Path>) -> Option<PathBuf> {
        let dialog = self
            .0
            .dialog()
            .file()
            .add_filter("Stories", &["md", "fountain"])
            .set_file_name(name);
        let dialog = match folder {
            Some(folder) => dialog.set_directory(folder),
            None => dialog,
        };
        chosen(dialog.blocking_save_file())
    }
    fn save_pdf(&self, name: &str, folder: Option<&Path>) -> Option<PathBuf> {
        let dialog = self
            .0
            .dialog()
            .file()
            .add_filter("PDF", &["pdf"])
            .set_file_name(name);
        let dialog = match folder {
            Some(folder) => dialog.set_directory(folder),
            None => dialog,
        };
        chosen(dialog.blocking_save_file())
    }
    fn save_scratchpad_copy(&self, folder: &Path) -> Option<PathBuf> {
        chosen(
            self.0
                .dialog()
                .file()
                .set_title("Save Scratchpad Copy")
                .set_directory(folder)
                .set_file_name("Scratchpad copy.md")
                .add_filter("Markdown", &["md"])
                .blocking_save_file(),
        )
    }
    fn confirm_discard(&self, name: &str) -> bool {
        let dialog = self
            .0
            .dialog()
            .message(format!(
                "Discard unsaved changes to {name}? Your saved file will be kept."
            ))
            .title("Unsaved changes")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Discard Changes".into(),
                "Keep Editing".into(),
            ));
        let dialog = match self.0.get_webview_window("main") {
            Some(window) => dialog.parent(&window),
            None => dialog,
        };
        dialog.blocking_show()
    }
    fn config_dir(&self) -> Option<PathBuf> {
        self.0.path().app_config_dir().ok()
    }
}

/// Run a command off the main thread (dialogs block; the engine takes time).
async fn dispatch(app: tauri::AppHandle, name: String, args: Value) -> Result<Reply, String> {
    let service = app.state::<Arc<Service>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.call(&name, &args))
        .await
        .map_err(|e| e.to_string())?
}
/// Every workbench command, by name. See `river_service::commands`.
#[tauri::command]
async fn river(app: tauri::AppHandle, name: String, args: Value) -> Result<Value, String> {
    match dispatch(app, name, args).await? {
        Reply::Json(value) => Ok(value),
        Reply::Bytes(_) => Err("This command returns binary data; use river_bytes.".into()),
    }
}
/// Binary replies (page images) without JSON encoding.
#[tauri::command]
async fn river_bytes(
    app: tauri::AppHandle,
    name: String,
    args: Value,
) -> Result<tauri::ipc::Response, String> {
    match dispatch(app, name, args).await? {
        Reply::Bytes(bytes) => Ok(tauri::ipc::Response::new(bytes)),
        Reply::Json(_) => Err("This command returns JSON; use river.".into()),
    }
}
#[tauri::command]
fn take_opened_files(app: tauri::AppHandle) -> Result<Vec<PathBuf>, String> {
    let desktop = app.state::<Desktop>();
    let mut opened = desktop.opened.lock().map_err(|e| e.to_string())?;
    Ok(std::mem::take(&mut *opened))
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
            let service = Service::new(
                app.path().app_data_dir()?,
                Box::new(MacShell(app.handle().clone())),
            )?;
            app.manage(Arc::new(service));
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
                            version: Some(app.package_info().version.to_string()),
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
            river,
            river_bytes,
            take_opened_files,
            quit_confirmed
        ])
        .build(tauri::generate_context!())
        .expect("River could not start");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. }
            if !app.state::<Desktop>().exit.load(Ordering::Relaxed) =>
        {
            api.prevent_exit();
            let _ = app.emit("river:command", "quit");
        }
        // Finder/Dock opens can arrive before the UI listens: queue, then nudge.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            let files: Vec<PathBuf> = urls
                .iter()
                .filter_map(|url| url.to_file_path().ok())
                .collect();
            if let Ok(mut queue) = app.state::<Desktop>().opened.lock() {
                queue.extend(files);
            }
            let _ = app.emit("river:open-files", ());
        }
        _ => {}
    });
}
