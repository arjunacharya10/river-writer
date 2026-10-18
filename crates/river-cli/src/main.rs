//! `river export` is the headless entry point for the production formatting crates.
use river_model::{Script, Severity};
use river_parse::parse_file;
use river_templates::TemplateId;
use river_text::TextEngine;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};
const HELP: &str = "River Writer — offline formatting engine (macOS)\n\n  river export FILE [-o OUTPUT.pdf] [--template NAME] [--preview-dir DIR]\n                    [--layout-json FILE] [--force]\n  river check FILE\n  river inspect FILE [--template NAME]\n\nTemplates: screenplay, screenplay-multicam, stageplay, manuscript\nDefaults: .md -> manuscript; .fountain -> screenplay.\nPreview: 144 dpi PNGs, from the same page display list as the PDF.\nIncludes stay within the input file's containing directory.\nExisting output files are preserved unless --force is supplied.\n";
#[derive(Default)]
struct Args {
    command: String,
    input: PathBuf,
    output: Option<PathBuf>,
    template: Option<TemplateId>,
    preview: Option<PathBuf>,
    json: Option<PathBuf>,
    force: bool,
}
fn args() -> Result<Option<Args>, String> {
    let mut argv = env::args().skip(1);
    let Some(command) = argv.next() else {
        return Ok(None);
    };
    if command == "--help" || command == "-h" {
        return Ok(None);
    }
    if command == "--version" {
        println!("river {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }
    if !["export", "check", "inspect"].contains(&command.as_str()) {
        return Err(format!("Unknown command: {command}"));
    }
    let mut a = Args {
        command,
        ..Args::default()
    };
    let mut input = None;
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "-o" | "--output" => a.output = Some(argv.next().ok_or("Missing output path")?.into()),
            "--template" => a.template = Some(argv.next().ok_or("Missing template name")?.parse()?),
            "--preview-dir" => {
                a.preview = Some(argv.next().ok_or("Missing preview directory")?.into())
            }
            "--layout-json" => a.json = Some(argv.next().ok_or("Missing JSON output path")?.into()),
            "--force" => a.force = true,
            s if s.starts_with('-') => return Err(format!("Unknown option: {s}")),
            _ => {
                if input.replace(PathBuf::from(arg)).is_some() {
                    return Err("Only one input file is accepted".into());
                }
            }
        }
    }
    a.input = input.ok_or("Missing input file")?;
    if a.command != "export"
        && (a.output.is_some() || a.preview.is_some() || a.json.is_some() || a.force)
    {
        return Err("Output options apply only to export".into());
    }
    match a
        .input
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("md" | "fountain") => {}
        _ => return Err("Input must be a .md or .fountain plain-text file".into()),
    }
    Ok(Some(a))
}
fn diagnostics(script: &Script) {
    for d in &script.diagnostics {
        let file = &script.files[d.source.file];
        let before = &file.text[..d.source.bytes.start.min(file.text.len())];
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        eprintln!(
            "{}:{line}: {}: {}",
            file.path
                .as_ref()
                .map_or_else(|| "<input>".into(), |p| p.display().to_string()),
            if d.severity == Severity::Error {
                "error"
            } else {
                "warning"
            },
            d.message
        )
    }
}
fn output(path: &Path, bytes: &[u8], force: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = parent.join(format!(".river-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if force {
            fs::rename(&temp, path)
        } else {
            fs::hard_link(&temp, path)
        }
        .map_err(|e| format!("{}: {e}", path.display()))
    })();
    let _ = fs::remove_file(&temp);
    result
}
fn same_file(a: &Path, b: &Path) -> bool {
    if a.canonicalize()
        .ok()
        .is_some_and(|p| b.canonicalize().ok().as_ref() == Some(&p))
    {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (fs::metadata(a), fs::metadata(b)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    false
}
fn run(a: Args) -> Result<(), String> {
    let start = Instant::now();
    let script = parse_file(&a.input)?;
    diagnostics(&script);
    if script.has_errors() {
        return Err("Input has parser errors; no output written".into());
    }
    if a.command == "check" {
        println!(
            "{}: {} elements, {} source files, {} warnings",
            a.input.display(),
            script.elements.len(),
            script.files.len(),
            script.diagnostics.len()
        );
        return Ok(());
    }
    let id = a
        .template
        .unwrap_or_else(|| TemplateId::default_for(&a.input));
    let mut engine = TextEngine::new();
    let publication = river_paginate::paginate(&mut engine, &script, id)?;
    if a.command == "inspect" {
        let json = serde_json::json!({"template":id,"script":script,"publication":publication});
        io::stdout()
            .write_all(
                serde_json::to_string_pretty(&json)
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let pdf_path = a.output.unwrap_or_else(|| a.input.with_extension("pdf"));
    // Never overwrite a source file, even with --force or a different path alias.
    for file in &script.files {
        if file
            .path
            .as_ref()
            .is_some_and(|source| same_file(&pdf_path, source))
        {
            return Err("Output would overwrite a source file".into());
        }
    }
    let pdf = river_pdf::encode(&publication)?;
    let mut outputs = vec![(pdf_path.clone(), pdf)];
    if let Some(json) = a.json {
        outputs.push((
            json,
            serde_json::to_vec_pretty(&publication).map_err(|e| e.to_string())?,
        ))
    }
    if let Some(dir) = a.preview {
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (i, page) in publication.pages.iter().enumerate() {
            let png = river_render::page(page, 144.0)?
                .encode_png()
                .map_err(|e| e.to_string())?;
            outputs.push((dir.join(format!("page-{:03}.png", i + 1)), png))
        }
    }
    // Check all destinations before writing any. Distinct output arguments must
    // not alias one another or any included source file.
    let absolute = |p: &Path| -> Result<PathBuf, String> {
        let parent = p
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        Ok(parent
            .canonicalize()
            .map_err(|e| format!("{}: {e}", parent.display()))?
            .join(p.file_name().ok_or("Invalid output filename")?))
    };
    let mut seen = std::collections::HashSet::new();
    for (path, _) in &outputs {
        let dest = if path.exists() {
            path.canonicalize().map_err(|e| e.to_string())?
        } else {
            absolute(path)?
        };
        if !seen.insert(dest) {
            return Err("Output destinations overlap".into());
        }
        if script.files.iter().any(|f| {
            f.path
                .as_ref()
                .is_some_and(|source| same_file(path, source))
        }) {
            return Err("Output would overwrite a source file".into());
        }
        if path.exists() && !a.force {
            return Err(format!("Output exists: {} (use --force)", path.display()));
        }
    }
    for (path, bytes) in outputs {
        output(&path, &bytes, a.force)?
    }
    eprintln!(
        "{}: {} pages · {} · {:.0} ms",
        pdf_path.display(),
        publication.pages.len(),
        id.name(),
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
fn main() -> ExitCode {
    match args() {
        Ok(None) => {
            if !env::args().any(|a| a == "--version") {
                print!("{HELP}")
            }
            ExitCode::SUCCESS
        }
        Ok(Some(a)) => match run(a) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("river: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("river: {e}\n\n{HELP}");
            ExitCode::FAILURE
        }
    }
}
