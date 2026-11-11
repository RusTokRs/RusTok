use std::path::Path;

use serde_json::{Value, json};

#[test]
fn conventional_test_support_files_do_not_report_test_only_panics() {
    let source = "fn fixture() { connection().expect(\"fixture\"); }\n";
    for path in ["src/test_support.rs", "src/structured_test_support.rs"] {
        assert!(
            rustok_doctor::scan_source(Path::new(path), source)
                .iter()
                .all(|finding| finding.pattern_id != "P-ZERO-PANIC-PROD")
        );
    }
    assert!(
        rustok_doctor::scan_source(Path::new("src/runtime_support.rs"), source)
            .iter()
            .any(|finding| finding.pattern_id == "P-ZERO-PANIC-PROD")
    );
}

#[test]
fn completed_constants_do_not_hide_panics_in_later_functions() {
    let source = r#"const LIMIT: usize = 2;
const SIZES: [u8;
    2] = [1, 2];
fn request(value: Option<u8>) -> u8 {
    value.unwrap()
}
static SAFE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("constant").expect("valid constant")
});
fn next(value: Option<u8>) -> u8 {
    value.expect("runtime input")
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/runtime.rs"), source);
    let lines: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-ZERO-PANIC-PROD")
        .map(|finding| finding.line)
        .collect();
    assert_eq!(lines, [5, 11]);
}

#[test]
fn propagated_errors_are_not_reported_as_discarded() {
    let source = r#"async fn execute(tx: Transaction) -> Result<(), Error> {
    let _ = self.get_brand(tenant_id, brand_id, None).await?;
    let _ = operation()
        .await?;
    let _ = tx.commit().await?;
    let _ = operation().await;
    let _ = tx.rollback().await;
    let _ = { guard()?; operation() };
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let discarded: Vec<_> = findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.pattern_id,
                "P-ANTI-SWALLOW" | "P-TXN-ROLLBACK-SWALLOW"
            )
        })
        .map(|finding| (finding.line, finding.pattern_id))
        .collect();
    assert_eq!(
        discarded,
        [
            (6, "P-ANTI-SWALLOW"),
            (7, "P-TXN-ROLLBACK-SWALLOW"),
            (8, "P-ANTI-SWALLOW")
        ]
    );
}

#[test]
fn reviews_column_filtered_triggers_in_migration_literals() {
    let source = r##"
// CREATE TRIGGER ignored AFTER UPDATE OF revision ON products;
async fn up() {
    execute(r#"
        CREATE TRIGGER product_revision
        AFTER UPDATE OF revision, tenant_id ON products
        FOR EACH ROW EXECUTE FUNCTION refresh();
        CREATE TRIGGER product_safe
        AFTER UPDATE ON products
        FOR EACH ROW EXECUTE FUNCTION refresh();
    "#).await;
}
#[cfg(test)]
mod tests {
    const TEST_SQL: &str = "CREATE TRIGGER t AFTER UPDATE OF v ON t;";
}
#[cfg(test)] mod test { const SQL: &str = "CREATE TRIGGER t AFTER UPDATE OF v ON t;"; }
"##;
    let findings =
        rustok_doctor::scan_source(Path::new("src/migrations/m20260101_trigger.rs"), source);
    let triggers: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-TRIGGER-ROW-CHANGE")
        .collect();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].line, 6);
    assert_eq!(triggers[0].confidence, rustok_doctor::Confidence::Heuristic);
    assert!(
        rustok_doctor::scan_source(Path::new("src/service.rs"), source)
            .iter()
            .all(|finding| finding.pattern_id != "P-TRIGGER-ROW-CHANGE")
    );
}

#[test]
fn reports_historical_and_rollback_trigger_definitions_as_candidates() {
    let source = include_str!(
        "../../../modules/rustok-product/src/migrations/m20261109_000040_fix_index_graph_update_trigger.rs"
    );
    let findings =
        rustok_doctor::scan_source(Path::new("src/migrations/m20261109_000040.rs"), source);
    let triggers: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-TRIGGER-ROW-CHANGE")
        .collect();
    assert_eq!(triggers.len(), 1);
    assert_eq!(triggers[0].line, 48);
}

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
