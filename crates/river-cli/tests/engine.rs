use river_model::*;
use river_parse::{Format, parse, parse_file};
use river_templates::TemplateId;
use river_text::TextEngine;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
fn publication(source: &str, id: TemplateId) -> Publication {
    let format = if id == TemplateId::Manuscript {
        Format::Markdown
    } else {
        Format::Fountain
    };
    river_paginate::paginate(&mut TextEngine::new(), &parse(source, format), id).unwrap()
}
fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name),
    )
    .unwrap()
}
fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "river-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn golden_all_templates() {
    for id in TemplateId::ALL {
        let p = publication(
            &fixture(if id == TemplateId::Manuscript {
                "manuscript.md"
            } else {
                "screenplay.fountain"
            }),
            id,
        );
        let mut text = format!("template {}\n", id.name());
        for (i, page) in p.pages.iter().enumerate() {
            text += &format!("PAGE {} number {:?}\n", i + 1, page.number);
            for line in &page.lines {
                text += &format!(
                    "{:?} {:.3} {:.3} {:.3} {}{}\n",
                    line.kind,
                    line.x,
                    line.y,
                    line.height,
                    if line.generated { "[generated] " } else { "" },
                    line.text.trim_end()
                );
            }
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/golden")
            .join(format!("{}.txt", id.name()));
        if std::env::var_os("RIVER_UPDATE_GOLDENS").is_some() {
            fs::write(&path, &text).unwrap()
        }
        assert_eq!(
            text,
            fs::read_to_string(&path).expect("reviewed golden exists"),
            "{}",
            id.name()
        );
    }
}
#[test]
fn long_dialogue_repeats_cue_and_loses_no_words() {
    let dialogue = "Every light on the far bank is a promise. ".repeat(180);
    let p = publication(&format!("MAYA\n{dialogue}\n"), TemplateId::Screenplay);
    assert!(p.pages.len() >= 3);
    assert!(p.pages[0].lines.iter().any(|l| l.text == "(MORE)"));
    assert!(p.pages[1].lines.iter().any(|l| l.text == "MAYA (CONT'D)"));
    let actual = p
        .pages
        .iter()
        .flat_map(|p| &p.lines)
        .filter(|l| l.kind == Kind::Dialogue)
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(norm(&actual), norm(&dialogue));
    for page in &p.pages {
        assert!(
            page.lines
                .iter()
                .filter(|l| l.kind == Kind::Dialogue)
                .count()
                >= 2
        );
        assert!(page.lines.iter().all(|l| l.y + l.height <= 744.01));
    }
}
#[test]
fn dual_dialogue_has_shared_top_and_continues() {
    let a = "Left bank. A bell sounds. ".repeat(130);
    let b = "Right bank. A lamp shines. ".repeat(95);
    let p = publication(
        &format!("MAYA\n{a}\n\nELIAS ^\n{b}\n"),
        TemplateId::Screenplay,
    );
    let cues = p.pages[0]
        .lines
        .iter()
        .filter(|l| l.kind == Kind::Cue)
        .collect::<Vec<_>>();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].y, cues[1].y);
    assert!(cues[1].x > cues[0].x + 100.0);
    let lines = p
        .pages
        .iter()
        .flat_map(|p| &p.lines)
        .filter(|l| l.kind == Kind::Dialogue)
        .collect::<Vec<_>>();
    assert_eq!(
        norm(
            &lines
                .iter()
                .filter(|l| l.x < 300.0)
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        ),
        norm(&a)
    );
    assert_eq!(
        norm(
            &lines
                .iter()
                .filter(|l| l.x > 300.0)
                .map(|l| l.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        ),
        norm(&b)
    );
}
#[test]
fn explicit_breaks_compress_and_no_trailing_blank_page() {
    let p = publication(
        "===\n\nA boat.\n\n===\n\n===\n\nA bell.\n\n===\n",
        TemplateId::Screenplay,
    );
    assert_eq!(p.pages.len(), 2);
    assert!(p.pages[0].lines.iter().any(|l| l.text == "A boat."));
    assert!(p.pages[1].lines.iter().any(|l| l.text == "A bell."));
}
#[test]
fn scene_heading_never_orphaned() {
    let source = format!(
        "{}\n\nEXT. RIVER - NIGHT\n\nThe boat arrives.\n",
        (0..26)
            .map(|i| format!("!Beat {i}.\n"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let p = publication(&source, TemplateId::Screenplay);
    for page in &p.pages {
        let body = page
            .lines
            .iter()
            .filter(|l| !l.generated)
            .collect::<Vec<_>>();
        assert!(body.last().is_none_or(|l| l.kind != Kind::Scene));
    }
}
#[test]
fn manuscript_continuation_left_edge_and_conservation() {
    let text = "By morning the river had carried away the old steps. ".repeat(160);
    let p = publication(&text, TemplateId::Manuscript);
    assert!(p.pages.len() > 2);
    assert!(p.pages[0].lines[0].text.trim().starts_with("By morning"));
    for page in p.pages.iter().skip(1) {
        let first = page
            .ops
            .iter()
            .find_map(|op| {
                if let DrawOp::Glyphs(r) = op {
                    r.glyphs.first()
                } else {
                    None
                }
            })
            .unwrap();
        assert!((first.x - 72.0).abs() < 0.01)
    }
    let actual = p
        .pages
        .iter()
        .flat_map(|p| &p.lines)
        .filter(|l| !l.generated)
        .map(|l| l.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(norm(&actual), norm(&text));
}
#[test]
fn multi_cam_dialogue_is_double_spaced() {
    let p = publication(
        "MAYA\nOne two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen.\n",
        TemplateId::ScreenplayMulticam,
    );
    let lines = p.pages[0]
        .lines
        .iter()
        .filter(|l| l.kind == Kind::Dialogue)
        .collect::<Vec<_>>();
    assert!(lines.len() >= 2);
    assert_eq!(lines[1].y - lines[0].y, 24.0)
}
#[test]
fn title_page_is_separate_and_unnumbered() {
    let p = publication(
        "Title: The Other Bank\nAuthor: N. Writer\nContact: writer@example.test\n\nEXT. BANK - NIGHT\n\nA light.\n",
        TemplateId::Screenplay,
    );
    assert_eq!(p.pages.len(), 2);
    assert_eq!(p.pages[0].number, None);
    assert_eq!(p.pages[1].number, Some(1));
    assert!(p.pages[0].lines.iter().any(|l| l.text == "The Other Bank"));
}
#[test]
fn hidden_text_never_enters_display_list() {
    let p = publication(
        "EXT. BANK\n\nA [[secret]] light.\n\n/* hidden */\n\n= invisible synopsis\n",
        TemplateId::Screenplay,
    );
    let json = serde_json::to_string(&p).unwrap();
    for forbidden in ["secret", "hidden", "invisible"] {
        assert!(!json.contains(forbidden));
    }
}
#[test]
fn glyph_positions_and_source_ranges_are_valid() {
    for id in TemplateId::ALL {
        let source = fixture(if id == TemplateId::Manuscript {
            "manuscript.md"
        } else {
            "screenplay.fountain"
        });
        let p = publication(&source, id);
        for page in &p.pages {
            for op in &page.ops {
                if let DrawOp::Glyphs(run) = op {
                    for g in &run.glyphs {
                        assert!(g.x >= 35.0 && g.x < 590.0, "x {}", g.x);
                        assert!(g.y >= 30.0 && g.y < 790.0, "y {}", g.y);
                        assert!(run.text.get(g.text_range.clone()).is_some());
                        if let Some(src) = &g.source {
                            assert!(source.get(src.bytes.clone()).is_some())
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn includes_are_relative_and_cycles_and_escape_are_rejected() {
    let outer = Temp::new();
    let temp = Temp(outer.0.join("project"));
    fs::create_dir(&temp.0).unwrap();
    fs::create_dir(temp.0.join("chapters")).unwrap();
    fs::write(
        temp.0.join("root.md"),
        "# Book\n\n{{INCLUDE: chapters/one.md}}\n",
    )
    .unwrap();
    fs::write(temp.0.join("chapters/one.md"), "A café by the river.\n").unwrap();
    let s = parse_file(&temp.0.join("root.md")).unwrap();
    assert_eq!(s.files.len(), 2);
    assert_eq!(s.elements.last().unwrap().source.file, 1);
    fs::write(temp.0.join("chapters/one.md"), "{{INCLUDE: ../root.md}}\n").unwrap();
    assert!(
        parse_file(&temp.0.join("root.md"))
            .unwrap_err()
            .contains("cycle")
    );
    fs::write(temp.0.join("root.md"), "{{INCLUDE: ../outside.md}}\n").unwrap();
    fs::write(temp.0.parent().unwrap().join("outside.md"), "external").unwrap();
    assert!(
        parse_file(&temp.0.join("root.md"))
            .unwrap_err()
            .contains("escapes")
    );
    fs::remove_file(temp.0.parent().unwrap().join("outside.md")).unwrap();
}
#[test]
fn cli_exports_pdf_png_json_and_preserves_existing_files() {
    let temp = Temp::new();
    let input = temp.0.join("story.fountain");
    fs::write(&input, "EXT. BANK\n\nA **bright** light.\n").unwrap();
    let output = temp.0.join("story.pdf");
    let json = temp.0.join("pages.json");
    let images = temp.0.join("preview");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_river"))
            .arg("export")
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .arg("--preview-dir")
            .arg(&images)
            .arg("--layout-json")
            .arg(&json)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(fs::read(&output).unwrap().starts_with(b"%PDF-"));
    assert!(
        fs::read(images.join("page-001.png"))
            .unwrap()
            .starts_with(b"\x89PNG")
    );
    let old = fs::read(&output).unwrap();
    assert!(!run().status.success());
    assert_eq!(fs::read(&output).unwrap(), old);
    let err = Command::new(env!("CARGO_BIN_EXE_river"))
        .args(["export"])
        .arg(&input)
        .arg("-o")
        .arg(&input)
        .arg("--force")
        .output()
        .unwrap();
    assert!(!err.status.success());
    assert_eq!(
        fs::read_to_string(&input).unwrap(),
        "EXT. BANK\n\nA **bright** light.\n"
    );
}
#[test]
fn unsupported_input_fails_without_output() {
    let temp = Temp::new();
    let input = temp.0.join("broken.fountain");
    fs::write(&input, "A [[broken note").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_river"))
        .arg("export")
        .arg(&input)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!input.with_extension("pdf").exists());
}
#[test]
fn long_unbroken_word_wraps_inside_margin() {
    let p = publication(&format!("!{}\n", "A".repeat(500)), TemplateId::Screenplay);
    assert!(p.pages[0].lines.len() > 5);
    for op in &p.pages[0].ops {
        if let DrawOp::Glyphs(r) = op {
            assert!(r.glyphs.iter().all(|g| g.x < 540.0))
        }
    }
}

#[test]
fn split_dialogue_prefers_sentence_end() {
    let text = "Do you see the light? Every lamp is still burning. ".repeat(140);
    let p = publication(&format!("MAYA\n{text}"), TemplateId::Screenplay);
    for page in p.pages.iter().take(p.pages.len() - 1) {
        let last = page
            .lines
            .iter()
            .rfind(|l| l.kind == Kind::Dialogue)
            .unwrap();
        assert!(last.text.trim_end().ends_with(['.', '?']), "{}", last.text);
    }
}
#[test]
fn hard_line_breaks_survive_shaping() {
    let p = publication("!First line.\nSecond line.\n", TemplateId::Screenplay);
    assert_eq!(p.pages[0].lines.len(), 2);
    assert_eq!(p.pages[0].lines[1].text, "Second line.")
}
#[cfg(unix)]
#[test]
fn source_hardlink_cannot_be_overwritten() {
    let temp = Temp::new();
    let input = temp.0.join("source.md");
    let output = temp.0.join("alias.pdf");
    fs::write(&input, "A river.").unwrap();
    fs::hard_link(&input, &output).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_river"))
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--force")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(&input).unwrap(), "A river.");
}

#[test]
fn scene_numbers_are_in_both_margins() {
    let p = publication("EXT. RIVER #12A#\n\nA boat.", TemplateId::Screenplay);
    let numbers = p.pages[0]
        .ops
        .iter()
        .filter_map(|op| {
            if let DrawOp::Glyphs(r) = op {
                (r.text.as_ref() == "12A").then_some(r)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(numbers.len(), 2);
    assert!(numbers[0].glyphs[0].x < 108.0);
    assert!(numbers[1].glyphs[0].x > 540.0);
}
#[test]
fn unsupported_directive_and_invalid_page_number_fail_cleanly() {
    for text in ["{{REVISION: blue}}", "{{PAGENUMBER: 18446744073709551615}}"] {
        let script = parse(text, Format::Fountain);
        assert!(
            river_paginate::paginate(&mut TextEngine::new(), &script, TemplateId::Screenplay)
                .is_err()
        );
    }
}

#[test]
fn multicam_parenthetical_merges_and_preserves_source() {
    let source = "MAYA\n(quietly)\nThe boat has left.\n";
    let p = publication(source, TemplateId::ScreenplayMulticam);
    let dialogue = p.pages[0]
        .lines
        .iter()
        .find(|l| l.kind == Kind::Dialogue)
        .unwrap();
    assert!(dialogue.text.starts_with("(quietly) The boat"));
    assert!(
        !p.pages[0]
            .lines
            .iter()
            .any(|l| l.kind == Kind::Parenthetical)
    );
    let r = dialogue.source.as_ref().unwrap().bytes.clone();
    assert!(source[r].contains("(quietly)\nThe boat"));
}

#[test]
fn dual_shorter_side_never_strands_one_line() {
    let left = (0..53)
        .map(|i| format!("Left line {i}.\n"))
        .collect::<String>();
    let right = (0..120)
        .map(|i| format!("Right line {i}.\n"))
        .collect::<String>();
    let p = publication(
        &format!("MAYA\n{left}\nELIAS ^\n{right}"),
        TemplateId::Screenplay,
    );
    for page in &p.pages {
        for side in [false, true] {
            let count = page
                .lines
                .iter()
                .filter(|l| l.kind == Kind::Dialogue && (l.x > 300.0) == side)
                .count();
            assert_ne!(count, 1)
        }
    }
}

#[test]
fn custom_headers_footers_and_page_numbers() {
    let p = publication(
        "{{PAGENUMBER: 7}}\n\n{{HEADER: River | %p/%pp | Draft}}\n\n{{FOOTER: %p}}\n\nA boat.\n",
        TemplateId::Screenplay,
    );
    assert_eq!(p.pages[0].number, Some(7));
    for label in ["River", "7/1", "Draft", "7"] {
        assert!(
            p.pages[0]
                .lines
                .iter()
                .any(|l| l.generated && l.text == label)
        )
    }
    let invalid = parse("{{HEADER: left | right}}\n\nA boat.", Format::Fountain);
    assert!(
        river_paginate::paginate(&mut TextEngine::new(), &invalid, TemplateId::Screenplay).is_err()
    );
}
