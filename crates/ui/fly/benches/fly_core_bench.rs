//! Dependency-free micro-benchmarks for the Fly core hot paths.
//!
//! Run with `cargo bench -p fly --bench fly_core_bench`. The harness is intentionally manual
//! (no criterion dependency): it prints one line per scenario so CI logs stay greppable, and it
//! exists to make performance regressions visible rather than to produce statistical reports.
//! Scenarios mirror the audit baseline sizes: 100 / 1 000 / 10 000 component nodes.

use fly::{
    ComponentPatch, EditorCommand, FlyEditor, GrapesJsCodec, PageSelection, RegistrySet,
    RenderPolicy, render_page,
};
use serde_json::{Map, Value, json};
use std::time::Instant;

fn document_with_nodes(nodes: usize) -> fly::ProjectDocument {
    let children: Vec<Value> = (0..nodes)
        .map(|index| {
            json!({
                "id": format!("node-{index}"),
                "type": "text",
                "content": format!("Node {index}"),
                "style": { "margin": "4px" }
            })
        })
        .collect();
    GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "bench",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": children
            }
        }]
    }))
    .expect("bench document must decode")
}

fn bench(label: &str, iterations: u32, mut run: impl FnMut()) {
    // One warm-up pass so first-run allocation costs do not dominate the measurement.
    run();
    let started = Instant::now();
    for _ in 0..iterations {
        run();
    }
    let elapsed = started.elapsed();
    println!(
        "{label}: {} iterations in {:?} ({:?}/iteration)",
        iterations,
        elapsed,
        elapsed / iterations
    );
}

fn main() {
    for nodes in [100_usize, 1_000, 10_000] {
        let document = document_with_nodes(nodes);
        let iterations = if nodes >= 10_000 { 20 } else { 200 };

        bench(
            &format!("codec_roundtrip[{nodes} nodes]"),
            iterations,
            || {
                let encoded = GrapesJsCodec::encode_vec(&document).expect("encode");
                let decoded = GrapesJsCodec::decode_slice(&encoded).expect("decode");
                std::hint::black_box(decoded);
            },
        );

        bench(&format!("validate[{nodes} nodes]"), iterations, || {
            let editor = FlyEditor::new(document.clone(), RegistrySet::with_builtins());
            std::hint::black_box(editor.validate());
        });

        bench(&format!("render[{nodes} nodes]"), iterations, || {
            let rendered = render_page(&document, &PageSelection::First, &RenderPolicy::default())
                .expect("render");
            std::hint::black_box(rendered.document_html().len());
        });

        // The authoring hot path: one property patch against the last node, including the full
        // clone + validation + revision hash `FlyEditor::apply` performs per command.
        let mut editor = FlyEditor::new(document.clone(), RegistrySet::with_builtins());
        let target = format!("node-{}", nodes - 1);
        bench(&format!("apply_patch[{nodes} nodes]"), iterations, || {
            let mut style = Map::new();
            style.insert("margin".to_string(), Value::String("8px".to_string()));
            editor
                .apply(EditorCommand::Patch {
                    component_id: target.clone(),
                    patch: ComponentPatch {
                        style: Some(Value::Object(style)),
                        ..ComponentPatch::default()
                    },
                })
                .expect("patch must apply");
        });
    }
}
