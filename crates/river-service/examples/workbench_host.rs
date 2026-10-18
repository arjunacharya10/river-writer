//! Local test adapter: real service/engine, deterministic replacements for OS dialogs.
use river_service::*;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{self, BufRead},
    path::Path,
};
fn main() {
    let folder = std::env::args().nth(1).expect("test folder");
    let mut docs = Documents::default();
    let mut engine = river_text::TextEngine::new();
    let mut previews = HashMap::<u64, (u64, river_model::Publication)>::new();
    let mut serial = 0;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let req: Value = serde_json::from_str(&line).unwrap();
        let args = &req["args"];
        let id = args["id"].as_u64().unwrap_or(0);
        let result = (|| -> Result<Value, String> {
            Ok(match req["name"].as_str().unwrap() {
                "new_document" => serde_json::to_value(docs.new_document()).unwrap(),
                "open_document" => serde_json::to_value(
                    docs.open(Path::new(
                        args["path"]
                            .as_str()
                            .unwrap_or(&format!("{folder}/Script.fountain")),
                    ))?,
                )
                .unwrap(),
                "recent_projects" => serde_json::to_value(projects::recents(
                    &Path::new(&folder).join(".test-config"),
                )?)
                .unwrap(),
                "open_project" => {
                    let path = args["path"].as_str().unwrap_or(&folder);
                    let project = projects::open(Path::new(path))?;
                    projects::remember(&Path::new(&folder).join(".test-config"), &project)?;
                    docs.set_project(&project.path)?;
                    serde_json::to_value(project).unwrap()
                }
                "create_project" => {
                    let project = projects::create(&Path::new(&folder).join("New Story"))?;
                    projects::remember(&Path::new(&folder).join(".test-config"), &project)?;
                    docs.set_project(&project.path)?;
                    serde_json::to_value(project).unwrap()
                }
                "save_project_session" => {
                    projects::save_session(
                        Path::new(args["path"].as_str().unwrap()),
                        serde_json::from_value(args["session"].clone()).unwrap(),
                    )?;
                    Value::Null
                }
                "open_folder" => {
                    docs.set_project(Path::new(&folder))?;
                    json!([folder, list_files(Path::new(&folder))?])
                }
                "refresh_folder" => serde_json::to_value(list_files(Path::new(
                    args["path"].as_str().unwrap_or(&folder),
                ))?)
                .unwrap(),
                "sync_document" => {
                    let d = docs.update(
                        id,
                        args["revision"].as_u64().unwrap(),
                        args["text"].as_str().unwrap().into(),
                        serde_json::from_value(args["template"].clone()).unwrap(),
                    )?;
                    serde_json::to_value(analyze(&d)).unwrap()
                }
                "analyze_document" => serde_json::to_value(analyze(docs.get(id)?)).unwrap(),
                "persist_saved_template" => {
                    docs.persist_saved_template(id)?;
                    Value::Null
                }
                "persist_template" => {
                    docs.persist_template(id)?;
                    Value::Null
                }
                "save_document" => {
                    let target = format!("{folder}/Saved.md");
                    let save_as =
                        args["saveAs"].as_bool().unwrap_or(false) || docs.get(id)?.path.is_none();
                    serde_json::to_value(docs.save(id, save_as.then(|| Path::new(&target)))?)
                        .unwrap()
                }
                "external_change" => json!(docs.external_change(id)?),
                "reload_document" => serde_json::to_value(docs.reload(id)?).unwrap(),
                "close_document" => {
                    docs.close(id);
                    previews.remove(&id);
                    Value::Null
                }
                "confirm_discard" => json!(false),
                "paginate" => {
                    let d = docs.get(id)?.clone();
                    let p = publication(&mut engine, &d, &docs.buffers())?;
                    serial += 1;
                    let result =
                        json!({"token":serial,"revision":d.revision,"pages":p.pages.len()});
                    previews.insert(id, (serial, p));
                    result
                }
                "render_page" => {
                    let (token, p) = previews.get(&id).ok_or("No preview")?;
                    if Some(*token) != args["token"].as_u64() {
                        return Err("Preview expired".into());
                    }
                    let bytes = river_render::page(
                        &p.pages[args["page"].as_u64().unwrap() as usize],
                        144.0,
                    )?
                    .encode_png()
                    .map_err(|e| e.to_string())?;
                    json!(bytes)
                }
                "export_pdf" => {
                    let path = std::path::PathBuf::from(format!("{folder}/Export.pdf"));
                    let p = publication(&mut engine, docs.get(id)?, &docs.buffers())?;
                    atomic_write(&path, &river_pdf::encode(&p)?, None)?;
                    json!(path)
                }
                command => return Err(format!("Unknown test command {command}")),
            })
        })();
        println!(
            "{}",
            match result {
                Ok(value) => json!({"request":req["request"],"value":value}),
                Err(e) => json!({"request":req["request"],"error":e}),
            }
        );
    }
}
