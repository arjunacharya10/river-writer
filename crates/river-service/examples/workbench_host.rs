//! Local test adapter: the real command table over stdin/stdout JSON, with
//! deterministic stand-ins for native dialogs.
use river_service::commands::{Reply, Service, Shell};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead},
    path::{Path, PathBuf},
};

/// Fixed answers in place of Mac dialogs. Cancellation is simulated in the page.
struct TestShell {
    folder: PathBuf,
}
impl Shell for TestShell {
    fn pick_story(&self) -> Option<PathBuf> {
        Some(self.folder.join("Script.fountain"))
    }
    fn pick_project(&self) -> Option<PathBuf> {
        Some(self.folder.clone())
    }
    fn new_project_location(&self) -> Option<PathBuf> {
        Some(self.folder.join("New Story"))
    }
    fn save_story(&self, _name: &str, _folder: Option<&Path>) -> Option<PathBuf> {
        Some(self.folder.join("Saved.md"))
    }
    fn save_pdf(&self, _name: &str, _folder: Option<&Path>) -> Option<PathBuf> {
        Some(self.folder.join("Export.pdf"))
    }
    fn save_scratchpad_copy(&self, folder: &Path) -> Option<PathBuf> {
        Some(folder.join("Scratchpad copy.md"))
    }
    fn confirm_discard(&self, _name: &str) -> bool {
        false
    }
    fn config_dir(&self) -> Option<PathBuf> {
        Some(self.folder.join(".test-config"))
    }
}

fn main() {
    let folder = PathBuf::from(std::env::args().nth(1).expect("test folder"));
    let service = Service::new(
        folder.join(".test-config"),
        Box::new(TestShell {
            folder: folder.clone(),
        }),
    )
    .expect("service");
    for line in io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line.expect("stdin")).expect("JSON request");
        let name = request["name"].as_str().unwrap_or_default();
        let reply = match service.call(name, &request["args"]) {
            Ok(Reply::Json(value)) => json!({ "request": request["request"], "value": value }),
            // The page converts this byte array back into an ArrayBuffer.
            Ok(Reply::Bytes(bytes)) => json!({ "request": request["request"], "value": bytes }),
            Err(error) => json!({ "request": request["request"], "error": error }),
        };
        println!("{reply}");
    }
}
