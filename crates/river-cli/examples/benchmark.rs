//! Full-file parsing/layout, a warm edit, PDF encoding and first-page raster timing.
use river_parse::{Format, parse};
use river_templates::TemplateId;
use river_text::TextEngine;
use std::time::Instant;
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn main() {
    let action="Nora walks along the river, counting each lamp on the far bank. A boat waits beside the steps. Nobody has touched the brass key. ".repeat(4);
    let dialogue="Every light is still burning. We can wait until morning, but the water will not wait with us. ".repeat(3);
    let source=(0..120).map(|i|format!("EXT. FERRY TERMINAL {i} - NIGHT\n\n{action}\n\nMAYA\n{dialogue}\n\n{action}\n\n{action}\n\n===\n\n")).collect::<String>();
    let mut engine = TextEngine::new();
    let start = Instant::now();
    let script = parse(&source, Format::Fountain);
    let parse_ms = ms(start);
    let start = Instant::now();
    let publication =
        river_paginate::paginate(&mut engine, &script, TemplateId::Screenplay).unwrap();
    let layout_ms = ms(start);
    assert_eq!(
        publication.pages.len(),
        120,
        "benchmark sections must fit on real pages"
    );
    let at = source.len() / 2;
    let at = source[at..].find("boat").unwrap() + at;
    let mut edit = source.clone();
    edit.replace_range(at..at + 4, "ferry");
    let start = Instant::now();
    let edited = parse(&edit, Format::Fountain);
    let edited_publication =
        river_paginate::paginate(&mut engine, &edited, TemplateId::Screenplay).unwrap();
    let warm_ms = ms(start);
    let start = Instant::now();
    let pdf = river_pdf::encode(&edited_publication).unwrap();
    let pdf_ms = ms(start);
    let start = Instant::now();
    let image = river_render::page(&edited_publication.pages[0], 144.0).unwrap();
    let raster_ms = ms(start);
    std::hint::black_box((pdf, image));
    println!(
        "{}",
        serde_json::json!({"pages":publication.pages.len(),"source_bytes":source.len(),"parse_cold_ms":parse_ms,"layout_and_paginate_cold_ms":layout_ms,"warm_edit_full_parse_and_paginate_ms":warm_ms,"pdf_encode_ms":pdf_ms,"first_page_raster_ms":raster_ms,"shape_cache_hits":engine.hits,"shape_cache_misses":engine.misses})
    );
}
