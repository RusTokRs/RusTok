use std::path::Path;

use serde_json::{Value, json};

#[test]
fn identifies_filter_map_that_converts_errors_to_none() {
    let source = r#"fn run() {
    let a = inputs.iter().filter_map(|v| parse(v).ok()).collect();
    let b = inputs.into_iter().filter_map(
        Result::ok,
    ).collect();
    let c = inputs.iter().filter_map(|v| v.as_str()).collect();
    let d = "items.filter_map(Result::ok)";
    // inputs.filter_map(Result::ok);
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/reader.rs"), source);
    assert!(
        findings
            .iter()
            .filter(|f| f.pattern_id == "P-FILTER-DROPPED-ERROR")
            .all(|f| f.confidence == rustok_doctor::Confidence::Heuristic)
    );
    let lines: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-FILTER-DROPPED-ERROR")
        .map(|f| f.line)
        .collect();
    assert_eq!(lines, [2, 3]);
}

#[test]
fn detects_direct_json_decode_defaults_without_flagging_handled_errors() {
    let source = r#"fn run() {
    let a = serde_json::from_value(data).unwrap_or_default();
    let b = serde_json::from_str::<Vec<String>>(
        input,
    )
    .unwrap_or_default();
    let c = serde_json::from_value(data).map_err(json_err)?;
    let d = optional.unwrap_or_default();
    let e = "serde_json::from_value(data).unwrap_or_default()";
    // serde_json::from_value(data).unwrap_or_default();
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    assert!(
        findings
            .iter()
            .filter(|f| f.pattern_id == "P-JSON-DECODE-DEFAULT")
            .all(|f| f.confidence == rustok_doctor::Confidence::Heuristic)
    );
    let lines: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-JSON-DECODE-DEFAULT")
        .map(|f| f.line)
        .collect();
    assert_eq!(lines, [2, 3]);
}

#[test]
fn detects_real_ai_accounting_and_trace_candidates() {
    let mapping = include_str!("../../../modules/rustok-ai/src/service/mapping.rs");
    let findings = rustok_doctor::scan_source(Path::new("src/service/mapping.rs"), mapping);
    assert!(
        findings
            .iter()
            .filter(|f| f.pattern_id == "P-JSON-DECODE-DEFAULT")
            .count()
            >= 2
    );

    let accounting = include_str!("../../../modules/rustok-ai/src/accounting.rs");
    let findings = rustok_doctor::scan_source(Path::new("src/accounting.rs"), accounting);
    assert!(
        findings
            .iter()
            .filter(|f| f.pattern_id == "P-TXN-ROLLBACK-SWALLOW")
            .count()
            >= 2
    );

    let migration = include_str!(
        "../../../modules/rustok-ai/src/migrations/m20260712_000001_provider_targets.rs"
    );
    let findings = rustok_doctor::scan_source(
        Path::new("src/migrations/m20260712_000001_provider_targets.rs"),
        migration,
    );
    assert!(
        findings
            .iter()
            .any(|f| f.pattern_id == "P-FILTER-DROPPED-ERROR")
    );
}

#[test]
fn identifies_transaction_error_to_option_without_duplicate_let_findings() {
    let source = "async fn run() {\n transaction.rollback().await.ok();\n tx\n  .commit()\n  .await\n  .ok();\n let _ = txn.rollback().await.ok();\n let _ =\n  transaction.rollback().await.ok();\n transaction.rollback().await?;\n // transaction.rollback().await.ok();\n}\n";
    let findings = rustok_doctor::scan_source(Path::new("src/runtime.rs"), source);
    let lines: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-TXN-ROLLBACK-SWALLOW")
        .map(|f| f.line)
        .collect();
    assert_eq!(lines, [2, 3, 7, 8]);
}

#[test]
fn conventional_test_support_files_do_not_report_test_only_panics() {
    let source = "fn fixture() { connection().expect(\"fixture\"); }\n";
    for path in [
        "src/test_support.rs",
        "src/structured_test_support.rs",
        "crates/utils/rustok-test-utils/src/db.rs",
    ] {
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

#[test]
fn identifies_unsupervised_task_spawns_without_flagging_supervised_or_tail_returns() {
    let source = r#"fn run() {
    // 1: comment spawn
    let text = "tokio::spawn(async move {});";
    // Standalone discarded spawn (line 4)
    tokio::spawn(async move {
        worker().await;
    });
    // Discarded with let _ = (line 8)
    let _ = tokio::spawn(async move {
        worker().await;
    });
    // Supervised: bound to a task handle
    let handle = tokio::spawn(async move {
        worker().await;
    });
    // Supervised: stored in a collection
    tasks.push(tokio::spawn(async move {
        worker().await;
    }));
    // Immediately awaited
    tokio::spawn(async move {
        worker().await;
    }).await.unwrap();
}

pub fn start_worker() -> JoinHandle<()> {
    tokio::spawn(async move {
        supervised_loop().await;
    })
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/worker.rs"), source);
    let orphans: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-ORPHAN-SPAWN")
        .collect();

    assert_eq!(orphans.len(), 2);
    assert_eq!(orphans[0].line, 5);
    assert_eq!(orphans[1].line, 9);
    assert!(
        orphans
            .iter()
            .all(|f| f.severity == rustok_doctor::Severity::High)
    );

    // Ensure let _ = tokio::spawn did not trigger duplicate P-ANTI-SWALLOW
    assert!(
        findings
            .iter()
            .filter(|f| f.pattern_id == "P-ANTI-SWALLOW")
            .all(|f| f.line != 9)
    );
}

#[test]
fn identifies_unchecked_vec_set_len() {
    let source = r#"unsafe fn decompress() {
    let mut out = Vec::with_capacity(1024);
    out.set_len(512);
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/ffi.rs"), source);
    let set_len_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-UNSAFE-VEC-SETLEN")
        .collect();

    assert_eq!(set_len_findings.len(), 1);
    assert_eq!(set_len_findings[0].line, 3);
    assert_eq!(
        set_len_findings[0].severity,
        rustok_doctor::Severity::Critical
    );
}

#[test]
fn detects_real_cron_scheduler_orphan_spawn() {
    let cron_file = include_str!("../../../modules/rustok-workflow/src/services/cron_scheduler.rs");
    let findings = rustok_doctor::scan_source(
        Path::new("crates/modules/rustok-workflow/src/services/cron_scheduler.rs"),
        cron_file,
    );
    let orphans: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-ORPHAN-SPAWN")
        .collect();

    // Line 109 is orphaned, line 37 returns JoinHandle (not orphaned)
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].line, 109);
}

#[test]
fn identifies_client_per_request_allocation_in_async_or_loops() {
    let source = r#"
async fn send_probe() -> Result<()> {
    // 1: Client/socket per async call (lines 4, 5)
    let client = reqwest::Client::new();
    let udp = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    Ok(())
}

fn process_items(items: Vec<String>) {
    for _ in items {
        // In loop allocation (line 12)
        let c = reqwest::Client::new();
    }
}

// Struct constructor (not in async, not in loop) - should NOT trigger
pub struct Service {
    client: reqwest::Client,
}
impl Service {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/adapter.rs"), source);
    let per_request: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-CLIENT-PER-REQUEST")
        .collect();

    assert_eq!(per_request.len(), 3);
    assert_eq!(per_request[0].line, 4);
    assert_eq!(per_request[1].line, 5);
    assert_eq!(per_request[2].line, 12);
}

#[test]
fn identifies_tls_danger_bypass() {
    let source = r#"
fn verify_sig() -> Result<HandshakeSignatureValid, rustls::Error> {
    Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
}

fn verify_cert() -> Result<ServerCertVerified, rustls::Error> {
    Ok(ServerCertVerified::assertion())
}

fn build_insecure_client() {
    let _ = reqwest::Client::builder().danger_accept_invalid_certs(true);
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/tls.rs"), source);
    let bypasses: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-TLS-DANGER-BYPASS")
        .collect();

    assert_eq!(bypasses.len(), 3);
    assert!(bypasses.iter().all(|f| f.severity == rustok_doctor::Severity::Critical));
}

#[test]
fn identifies_unbounded_network_buffer_allocation_and_respects_guards() {
    let source = r#"
fn insecure_read(buf: [u8; 4]) {
    let len = u32::from_be_bytes(buf) as usize;
    let mut response_bytes = vec![0u8; len];
}

fn secured_read_ensure(buf: [u8; 4], max: usize) -> Result<(), ()> {
    let len = u32::from_be_bytes(buf) as usize;
    ensure_frame_size(len, max)?;
    let mut response_bytes = vec![0u8; len];
    Ok(())
}

fn secured_read_max(buf: [u8; 4]) -> Result<(), ()> {
    let len = u32::from_be_bytes(buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(());
    }
    let mut response_bytes = vec![0u8; len];
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/protocol.rs"), source);
    let oom_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-UNBOUNDED-ALLOC-OOM")
        .collect();

    assert_eq!(oom_findings.len(), 1);
    assert_eq!(oom_findings[0].line, 3);
    assert_eq!(oom_findings[0].severity, rustok_doctor::Severity::Critical);
}

#[test]
fn detects_real_channel_admin_client_per_request() {
    let channel_file =
        include_str!("../../../modules/rustok-channel/admin/src/transport/rest_adapter.rs");
    let findings = rustok_doctor::scan_source(
        Path::new("crates/modules/rustok-channel/admin/src/transport/rest_adapter.rs"),
        channel_file,
    );
    let per_request: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-CLIENT-PER-REQUEST")
        .collect();

    // In get_json, post_json, put_json, delete_json
    assert_eq!(per_request.len(), 4);
    assert_eq!(per_request[0].line, 38);
    assert_eq!(per_request[1].line, 71);
    assert_eq!(per_request[2].line, 107);
    assert_eq!(per_request[3].line, 141);
}

#[test]
fn detects_non_production_comments_as_unfinished_code() {
    let source = r#"
fn quic_probe() {
    // Note: In production you'd use QUIC datagrams so this isn't production code
    run_raw_probe();
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/probe.rs"), source);
    let unfinished: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-UNFINISHED-CODE")
        .collect();

    assert_eq!(unfinished.len(), 1);
    assert_eq!(unfinished[0].line, 3);
}
