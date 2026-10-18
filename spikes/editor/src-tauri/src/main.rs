#[tauri::command]
fn report_results(report: serde_json::Value) -> Result<(), String> {
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../output");
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    std::fs::write(
        output.join("native-results.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())
}
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![report_results])
        .run(tauri::generate_context!())
        .expect("editor trial failed");
}
