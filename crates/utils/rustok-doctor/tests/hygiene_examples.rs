use std::path::Path;

use serde_json::{Value, json};

#[test]
fn preserves_established_hygiene_examples() {
    let examples: Vec<Value> =
        serde_json::from_str(include_str!("hygiene_examples.json")).expect("valid examples");
    let mut differences = Vec::new();
    for (index, example) in examples.iter().enumerate() {
        let path = Path::new(example["path"].as_str().expect("example path"));
        let source = example["source"].as_str().expect("example source");
        let findings = rustok_doctor::scan_source(path, source);
        let actual: Vec<Value> = findings
            .iter()
            .map(|finding| {
                json!({
                    "pattern_id": finding.pattern_id,
                    "rule_name": finding.rule_name,
                    "severity": finding.severity,
                    "confidence": finding.confidence,
                    "line": finding.line,
                    "content": finding.content,
                    "message": finding.message,
                    "suggested_fix": finding.suggested_fix,
                })
            })
            .collect();
        if json!(actual) != example["findings"] {
            differences.push(format!(
                "example {index} {}\nexpected: {}\nactual: {}",
                path.display(),
                example["findings"],
                json!(actual)
            ));
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n\n"));
}

#[test]
fn vendored_paths_only_suppress_unfinished_markers() {
    let findings = rustok_doctor::scan_source(
        Path::new("crates/libs/vendored-code/src/lib.rs"),
        "// TODO upstream\nfn run() { let _ = operation(); }\n",
    );
    assert!(
        findings
            .iter()
            .all(|finding| finding.pattern_id != "P-UNFINISHED-CODE")
    );
    // The vendored exemption affects only incomplete markers.
    let other = rustok_doctor::scan_source(
        Path::new("crates/libs/vendored-code/src/lib.rs"),
        "include!(\"local.rs\");\n",
    );
    assert!(
        other
            .iter()
            .any(|finding| finding.pattern_id == "P-ZERO-INCLUDE")
    );
}

#[test]
fn incomplete_markers_in_inline_tests_are_excluded() {
    let source = "#[cfg(test)]\nmod tests {\n  // TODO test fixture\n  #[test] fn t() { todo!(\"fixture\"); }\n}\n// TODO production\n";
    let findings = rustok_doctor::scan_source(Path::new("src/lib.rs"), source);
    let lines: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-UNFINISHED-CODE")
        .map(|finding| finding.line)
        .collect();
    assert_eq!(lines, [6]);
}

#[test]
fn detects_queries_after_an_inner_loop() {
    let source = "async fn load() {\n for first in items {\n  for second in first.children {\n   inspect(second);\n  }\n  Entity::find().all(db).await;\n }\n}\n";
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    assert!(
        findings
            .iter()
            .any(|finding| finding.pattern_id == "P-BATCH-QUERY" && finding.line == 6)
    );
}

#[test]
fn detects_multiline_include_without_flagging_generated_sources() {
    let source =
        "include!(\n  \"local.rs\"\n);\ninclude!(concat!(env!(\"OUT_DIR\"), \"generated.rs\"));\n";
    let findings = rustok_doctor::scan_source(Path::new("src/lib.rs"), source);
    let includes: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-ZERO-INCLUDE")
        .collect();
    assert_eq!(includes.len(), 1);
    assert_eq!(includes[0].line, 1);
}
