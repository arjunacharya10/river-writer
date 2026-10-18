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
    docs.enable_history(Path::new(&folder).join(".test-config"));
    let recovery =
        recovery::Recovery::new(Path::new(&folder).join(".test-config/recovery")).unwrap();
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
                "git_status" => serde_json::to_value(git::status(Path::new(
                    args["root"].as_str().ok_or("Missing root")?,
                ))?)
                .unwrap(),
                "git_init" => {
                    initialize_git(Path::new(args["root"].as_str().ok_or("Missing root")?))?;
                    Value::Null
                }
                "git_identity" => {
                    git::identity(
                        Path::new(args["root"].as_str().ok_or("Missing root")?),
                        args["name"].as_str().ok_or("Missing name")?,
                        args["email"].as_str().ok_or("Missing email")?,
                    )?;
                    Value::Null
                }
                "git_log" => serde_json::to_value(git::log(
                    Path::new(args["root"].as_str().ok_or("Missing root")?),
                    args["before"].as_str(),
                )?)
                .unwrap(),
                "git_diff" => serde_json::to_value(git::commit_diff(
                    Path::new(args["root"].as_str().ok_or("Missing root")?),
                    args["id"].as_str().ok_or("Missing commit")?,
                )?)
                .unwrap(),
                "git_commit" => serde_json::to_value(git::commit(
                    Path::new(args["root"].as_str().ok_or("Missing root")?),
                    args["head"].as_str(),
                    args["tree"].as_str().ok_or("Missing tree")?,
                    args["message"].as_str().ok_or("Missing message")?,
                )?)
                .unwrap(),
                "recovery_drafts" => serde_json::to_value(recovery.pending()?).unwrap(),
                "recover_draft" => serde_json::to_value(
                    recovery.restore(&mut docs, args["key"].as_str().ok_or("Missing key")?)?,
                )
                .unwrap(),
                "settle_scratchpad" => {
                    let root = Path::new(args["root"].as_str().ok_or("Missing root")?);
                    if extras::scratchpad(root)?.revision.as_deref() != args["expected"].as_str() {
                        return Err("Scratchpad changed again".into());
                    }
                    recovery.forget_note(&extras::scratch_path(root)?)?;
                    Value::Null
                }
                "load_scratchpad" => serde_json::to_value(extras::scratchpad(Path::new(
                    args["root"].as_str().ok_or("Missing root")?,
                ))?)
                .unwrap(),
                "protect_scratchpad" => {
                    let root = Path::new(args["root"].as_str().ok_or("Missing root")?);
                    let path = extras::scratch_path(root)?;
                    let text = args["text"].as_str().ok_or("Missing text")?;
                    if extras::scratchpad(root)?.text == text {
                        recovery.forget_note(&path)?;
                    } else {
                        recovery.protect_note(&path, text)?;
                    }
                    Value::Null
                }
                "save_scratchpad" => {
                    let root = Path::new(args["root"].as_str().ok_or("Missing root")?);
                    let text = args["text"].as_str().ok_or("Missing text")?;
                    recovery.protect_note(&extras::scratch_path(root)?, text)?;
                    let saved = extras::save_scratchpad(root, args["expected"].as_str(), text)?;
                    recovery.forget_note(&extras::scratch_path(root)?)?;
                    serde_json::to_value(saved).unwrap()
                }
                "copy_scratchpad" => {
                    let target = Path::new(args["root"].as_str().ok_or("Missing root")?)
                        .join("Scratchpad copy.md");
                    extras::save_copy(&target, args["text"].as_str().ok_or("Missing text")?)?;
                    json!(true)
                }
                "load_todos" => serde_json::to_value(todos::load(Path::new(
                    args["root"].as_str().ok_or("Missing root")?,
                ))?)
                .unwrap(),
                "update_todos" => serde_json::to_value(todos::update(
                    Path::new(args["root"].as_str().ok_or("Missing root")?),
                    args["expected"].as_str(),
                    serde_json::from_value(args["change"].clone()).map_err(|e| e.to_string())?,
                )?)
                .unwrap(),
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
                    recovery.protect(&d)?;
                    serde_json::to_value(analyze(&d)).unwrap()
                }
                "autosave_document" => {
                    let doc =
                        docs.autosave(id, args["revision"].as_u64().ok_or("Missing revision")?)?;
                    if let Some(doc) = &doc {
                        recovery.protect(doc)?;
                        docs.persist_saved_template(id)?;
                    }
                    serde_json::to_value(doc).unwrap()
                }
                "local_history" => {
                    serde_json::to_value(docs.local_history_before(id, args["before"].as_str())?)
                        .unwrap()
                }
                "history_text" => json!(
                    docs.history_text(id, args["version"].as_str().ok_or("Missing version")?)?
                ),
                "prepare_restore" => {
                    docs.before_restore(id, args["revision"].as_u64().ok_or("Missing revision")?)?;
                    json!(
                        docs.history_text(id, args["version"].as_str().ok_or("Missing version")?)?
                    )
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
                    let doc = docs.save(id, save_as.then(|| Path::new(&target)))?;
                    recovery.protect(&doc)?;
                    serde_json::to_value(doc).unwrap()
                }
                "external_change" => json!(docs.external_change(id)?),
                "reload_document" => {
                    let doc = docs.reload(id)?;
                    recovery.protect(&doc)?;
                    serde_json::to_value(doc).unwrap()
                }
                "close_document" => {
                    recovery.forget(id)?;
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
