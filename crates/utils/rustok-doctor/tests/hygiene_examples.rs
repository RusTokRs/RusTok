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
    assert!(
        bypasses
            .iter()
            .all(|f| f.severity == rustok_doctor::Severity::Critical)
    );
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
    let channel_file = include_str!("fixtures/channel_admin_rest_adapter.rs");
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

#[test]
fn identifies_blocking_io_in_async_code() {
    let source = r#"
async fn handle_request() -> Result<()> {
    let data = std::fs::read_to_string("config.toml")?;
    std::fs::write("log.txt", b"done")?;
    Ok(())
}

fn sync_handler() -> Result<()> {
    // Synchronous fn should NOT trigger P-BLOCKING-IO-IN-ASYNC
    let data = std::fs::read_to_string("config.toml")?;
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/handler.rs"), source);
    let blocking: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-BLOCKING-IO-IN-ASYNC")
        .collect();

    assert_eq!(blocking.len(), 2);
    assert_eq!(blocking[0].line, 3);
    assert_eq!(blocking[1].line, 4);
}

#[test]
fn identifies_overly_restrictive_pointer_arguments() {
    let source = r#"
pub fn bad_args(
    name: &String,
    items: &Vec<u32>,
    path: &PathBuf,
    opt: Option<&String>,
    opt_vec: Option<&Vec<String>>,
    slice_of_refs: &[&String],
) {
}

// In-place buffer mutation should NOT trigger
pub fn good_args(
    name: &str,
    items: &[u32],
    path: &Path,
    buf: &mut String,
    vec_buf: &mut Vec<u8>,
) {
}

// Trait conversion from &String should NOT trigger
pub fn from(value: &String) -> Self {
    Self
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/args.rs"), source);
    let ptr_args: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-PTR-ARG")
        .collect();

    assert_eq!(ptr_args.len(), 6);
    assert_eq!(ptr_args[0].line, 3);
    assert_eq!(ptr_args[1].line, 4);
    assert_eq!(ptr_args[2].line, 5);
    assert_eq!(ptr_args[3].line, 6);
    assert_eq!(ptr_args[4].line, 7);
    assert_eq!(ptr_args[5].line, 8);
}

#[test]
fn identifies_bare_allow_dead_code() {
    let source = r#"
#[allow(dead_code)]
fn unused_helper() {
}

#[expect(dead_code, reason = "temporary API stub")]
fn expected_stub() {
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/helper.rs"), source);
    let allow_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-STALE-ALLOW-LINT")
        .collect();

    assert_eq!(allow_findings.len(), 1);
    assert_eq!(allow_findings[0].line, 2);
}

#[test]
fn detects_pointer_argument_in_fulfillment_fixture() {
    let fulfillment_source = include_str!("fixtures/fulfillment_admin_core.rs");
    let findings = rustok_doctor::scan_source(
        Path::new("crates/modules/rustok-fulfillment/admin/src/core.rs"),
        fulfillment_source,
    );
    let ptr_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-PTR-ARG")
        .collect();
    assert_eq!(ptr_findings.len(), 1);
    assert_eq!(ptr_findings[0].line, 237);
}

#[test]
fn identifies_bare_allow_unused_imports_and_mut() {
    let source = r#"
#[allow(unused_imports)]
use std::time::Duration;

#[allow(unused_mut)]
fn test_mut() {
    let mut x = 1;
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let allow_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-STALE-ALLOW-LINT")
        .collect();

    assert_eq!(allow_findings.len(), 2);
    assert_eq!(allow_findings[0].line, 2);
    assert_eq!(allow_findings[1].line, 5);
}

#[test]
fn identifies_crate_and_bare_allow_deprecated() {
    let prod_source = concat!(
        "\n",
        "#",
        "![allow(deprecated)]\n\n",
        "#",
        "[allow(deprecated)]\n",
        "fn legacy_caller() {}\n\n",
        "#",
        "[expect(deprecated, reason = \"transitional compatibility bridge\")]\n",
        "fn tracked_caller() {}\n",
    );
    let prod_findings: Vec<_> =
        rustok_doctor::scan_source(Path::new("src/service.rs"), prod_source)
            .into_iter()
            .filter(|f| f.pattern_id == "P-STALE-ALLOW-LINT")
            .collect();
    assert_eq!(prod_findings.len(), 2);
    assert_eq!(prod_findings[0].line, 2);
    assert_eq!(prod_findings[1].line, 4);

    let test_source = concat!(
        "\n",
        "#",
        "![allow(deprecated)]\n\n",
        "#",
        "[allow(deprecated)]\n",
        "#[test]\n",
        "fn exercises_transitional_shim() {}\n",
    );
    let test_findings: Vec<_> =
        rustok_doctor::scan_source(Path::new("tests/brand.rs"), test_source)
            .into_iter()
            .filter(|f| f.pattern_id == "P-STALE-ALLOW-LINT")
            .collect();
    assert_eq!(test_findings.len(), 1);
    assert_eq!(test_findings[0].line, 2);
}

#[test]
fn respects_invariant_comments_for_expect_calls() {
    let source = r#"
fn run() {
    // INVARIANT: statically guaranteed valid constant
    let a = parse("fixed").expect("valid");

    // Without invariant comment, this must be flagged
    let b = parse("dynamic").expect("might fail");
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let panic_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-ZERO-PANIC-PROD")
        .collect();

    assert_eq!(panic_findings.len(), 1);
    assert_eq!(panic_findings[0].line, 7);
}

#[test]
fn identifies_post_hoc_in_memory_tenant_checks() {
    let source = r#"
async fn validate_access(app: &App, tenant_id: Uuid) -> Result<()> {
    if app.tenant_id != tenant_id {
        return Err(Error::Forbidden);
    }
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let tenant_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-TENANT-FALLBACK-LEAK")
        .collect();

    assert_eq!(tenant_findings.len(), 1);
    assert_eq!(tenant_findings[0].line, 3);
}

#[test]
fn identifies_direct_print_in_production_code() {
    let source = r#"
pub fn calculate() -> u32 {
    println!("debugging calculation");
    eprintln!("error calculation");
    dbg!(42);
    42
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let print_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-LOG-RAW-PRINT")
        .collect();

    assert_eq!(print_findings.len(), 3);
    assert_eq!(print_findings[0].line, 3);
    assert_eq!(print_findings[1].line, 4);
    assert_eq!(print_findings[2].line, 5);
}

#[test]
fn allows_direct_print_in_cli_and_tests() {
    let source = r#"
fn main() {
    println!("CLI output");
    eprintln!("CLI error");
}
"#;
    let cli_findings = rustok_doctor::scan_source(Path::new("src/bin/tool.rs"), source);
    assert!(
        cli_findings
            .iter()
            .all(|f| f.pattern_id != "P-LOG-RAW-PRINT")
    );

    let test_findings = rustok_doctor::scan_source(Path::new("tests/integration_test.rs"), source);
    assert!(
        test_findings
            .iter()
            .all(|f| f.pattern_id != "P-LOG-RAW-PRINT")
    );
}

#[test]
fn identifies_bare_allow_in_tuple_struct_field() {
    let source = r#"
struct ActiveJobHandle(#[allow(dead_code)] Uuid);
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let allow_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-STALE-ALLOW-LINT")
        .collect();

    assert_eq!(allow_findings.len(), 1);
    assert_eq!(allow_findings[0].line, 2);
}

#[test]
fn identifies_unreachable_in_production_code() {
    let source = r#"
pub fn map_status(val: &str) -> &'static str {
    match val {
        "active" => "active",
        _ => unreachable!(),
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let panic_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-ZERO-PANIC-PROD")
        .collect();

    assert_eq!(panic_findings.len(), 1);
    assert_eq!(panic_findings[0].line, 5);
}

#[test]
fn allows_unreachable_with_invariant_comment() {
    let source = r#"
pub fn map_status(val: &str) -> &'static str {
    match val {
        "active" => "active",
        // INVARIANT: validated by caller schema
        _ => unreachable!(),
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    assert!(findings.iter().all(|f| f.pattern_id != "P-ZERO-PANIC-PROD"));
}

#[test]
fn identifies_bare_string_error_in_result() {
    let source = r#"
pub fn parse_input(raw: &str) -> Result<Parsed, String> {
    Ok(Parsed)
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let error_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-TYPED-DOMAIN-ERROR")
        .collect();

    assert_eq!(error_findings.len(), 1);
    assert_eq!(error_findings[0].line, 2);
    assert!(error_findings[0].message.contains("String"));
}

#[test]
fn identifies_bare_str_slice_error_in_result() {
    let source = r#"
pub fn check_status(raw: &str) -> Result<(), &'static str> {
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let error_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-TYPED-DOMAIN-ERROR")
        .collect();

    assert_eq!(error_findings.len(), 1);
    assert_eq!(error_findings[0].line, 2);
}

#[test]
fn identifies_anyhow_in_domain_service() {
    let source = r#"
pub fn run_operation() -> Result<(), DomainError> {
    let _ = anyhow::bail!("failed");
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let error_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-TYPED-DOMAIN-ERROR")
        .collect();

    assert_eq!(error_findings.len(), 1);
    assert_eq!(error_findings[0].line, 3);
}

#[test]
fn allows_strongly_typed_domain_errors_in_result() {
    let source = r#"
pub fn parse_settings() -> Result<std::collections::BTreeMap<String, String>, DomainError> {
    Ok(Default::default())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    assert!(
        findings
            .iter()
            .all(|f| f.pattern_id != "P-TYPED-DOMAIN-ERROR")
    );
}

#[test]
fn identifies_check_then_unwrap_pattern() {
    let source = r#"
pub fn extract_value(opt: Option<String>) -> String {
    if opt.is_some() {
        opt.unwrap()
    } else {
        "default".to_string()
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let check_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-CHECK-THEN-UNWRAP")
        .collect();

    assert_eq!(check_findings.len(), 1);
    assert_eq!(check_findings[0].line, 3);
    assert!(check_findings[0].message.contains("opt"));
}

#[test]
fn identifies_check_then_expect_pattern() {
    let source = r#"
pub fn extract_result(res: Result<String, DomainError>) -> String {
    if res.is_ok() {
        res.expect("validated")
    } else {
        "fallback".to_string()
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let check_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-CHECK-THEN-UNWRAP")
        .collect();

    assert_eq!(check_findings.len(), 1);
    assert_eq!(check_findings[0].line, 3);
}

#[test]
fn allows_idiomatic_pattern_matching() {
    let source = r#"
pub fn extract_value(opt: Option<String>) -> String {
    if let Some(val) = opt {
        val
    } else {
        "default".to_string()
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    assert!(
        findings
            .iter()
            .all(|f| f.pattern_id != "P-CHECK-THEN-UNWRAP")
    );
}

#[test]
fn identifies_sync_process_command_in_async_fn() {
    let source = r#"
pub async fn execute_task() -> Result<(), DomainError> {
    let status = std::process::Command::new("ls").status()?;
    Ok(())
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/service.rs"), source);
    let blocking_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-BLOCKING-IO-IN-ASYNC")
        .collect();

    assert_eq!(blocking_findings.len(), 1);
    assert_eq!(blocking_findings[0].line, 3);
}

#[test]
fn parses_dev_only_crates_from_cargo_manifest() {
    let manifest = r#"
[package]
name = "example-crate"

[dependencies]
tokio = { workspace = true }
sqlx-core = { package = "sqlx-custom", version = "0.8" }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
serde_json = "1"
wiremock = "0.6"
my-helper = { package = "rustok-test-kit", path = "../kit" }
"#;
    let dev_only = rustok_doctor::parse_dev_only_crates(manifest);
    assert!(!dev_only.contains("tokio"));
    assert!(!dev_only.contains("sqlx_custom"));
    assert!(dev_only.contains("serde_json"));
    assert!(dev_only.contains("wiremock"));
    assert!(dev_only.contains("rustok_test_kit"));
}

#[test]
fn identifies_dev_dependency_leak_and_allows_cfg_test_usage() {
    let dev_only = rustok_doctor::parse_dev_only_crates(
        r#"
[dependencies]
tokio = "1"

[dev-dependencies]
tokio = "1"
wiremock = "0.6"
"#,
    );
    let source = r#"
use tokio::runtime::Runtime;
use wiremock::MockServer;

#[cfg(test)]
use wiremock::Mock;

#[cfg(test)]
mod tests {
    use wiremock::ResponseTemplate;
}
"#;
    let findings = rustok_doctor::scan_crate_source(Path::new("src/service.rs"), source, &dev_only);
    let leak_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.pattern_id == "P-DEV-DEP-PROD-LEAK")
        .collect();

    assert_eq!(leak_findings.len(), 1);
    assert_eq!(leak_findings[0].line, 3);
}

#[test]
fn identifies_grpc_deadline_exceeded_without_cancelled() {
    let bad = r#"
pub fn map_status(code: tonic::Code) -> PortErrorKind {
    match code {
        tonic::Code::DeadlineExceeded => PortErrorKind::Timeout,
        _ => PortErrorKind::Unavailable,
    }
}
"#;
    let good = r#"
pub fn map_status(code: tonic::Code) -> PortErrorKind {
    match code {
        tonic::Code::Cancelled | tonic::Code::DeadlineExceeded => PortErrorKind::Timeout,
        _ => PortErrorKind::Unavailable,
    }
}
"#;
    let bad_findings: Vec<_> = rustok_doctor::scan_source(Path::new("src/grpc/client.rs"), bad)
        .into_iter()
        .filter(|f| f.pattern_id == "P-GRPC-DEADLINE-STATUS")
        .collect();
    let good_findings: Vec<_> = rustok_doctor::scan_source(Path::new("src/grpc/client.rs"), good)
        .into_iter()
        .filter(|f| f.pattern_id == "P-GRPC-DEADLINE-STATUS")
        .collect();

    assert_eq!(bad_findings.len(), 1);
    assert_eq!(bad_findings[0].line, 4);
    assert!(good_findings.is_empty());
}

#[test]
fn identifies_raw_owner_fields_alongside_bounded_facts() {
    let bad = r#"
pub fn log_boundary(token_present: bool, error: &str) {
    tracing::warn!(
        token_present,
        ?error,
        "boundary rejected request"
    );
}
"#;
    let good = r#"
pub fn log_boundary(token_present: bool, error_kind: &str) {
    tracing::warn!(token_present, %error_kind, "boundary rejected request");
}
"#;
    let bad_findings: Vec<_> = rustok_doctor::scan_source(Path::new("src/adapter.rs"), bad)
        .into_iter()
        .filter(|f| f.pattern_id == "P-BOUNDED-OWNER-FACTS")
        .collect();
    let good_findings: Vec<_> = rustok_doctor::scan_source(Path::new("src/adapter.rs"), good)
        .into_iter()
        .filter(|f| f.pattern_id == "P-BOUNDED-OWNER-FACTS")
        .collect();

    assert_eq!(bad_findings.len(), 1);
    assert_eq!(bad_findings[0].line, 3);
    assert!(good_findings.is_empty());
}

#[test]
fn identifies_debug_assertions_outside_canonical_settings_owner() {
    let source = r#"
pub fn strict_headers_enabled() -> bool {
    !cfg!(debug_assertions)
}
"#;
    let non_owner: Vec<_> = rustok_doctor::scan_source(Path::new("src/security.rs"), source)
        .into_iter()
        .filter(|f| f.pattern_id == "P-RELEASE-SECURITY-MODE-DRIFT")
        .collect();
    let owner: Vec<_> =
        rustok_doctor::scan_source(Path::new("apps/server/src/common/settings.rs"), source)
            .into_iter()
            .filter(|f| f.pattern_id == "P-RELEASE-SECURITY-MODE-DRIFT")
            .collect();

    assert_eq!(non_owner.len(), 1);
    assert_eq!(non_owner[0].line, 3);
    assert!(owner.is_empty());
}

#[test]
fn identifies_slop_comments_while_allowing_rationale_and_docs() {
    let source = r#"
/// Create a new account record.
pub fn create_account() {
    // Create a new buffer for downstream parsing.
    let _buf = Vec::<u8>::new();
    // INVARIANT: empty vector triggers fallback parser in legacy mode.
    let _flag = true;
}
"#;
    let slop_findings: Vec<_> = rustok_doctor::scan_source(Path::new("src/service.rs"), source)
        .into_iter()
        .filter(|f| f.pattern_id == "P-SLOP-COMMENT")
        .collect();

    assert_eq!(slop_findings.len(), 1);
    assert_eq!(slop_findings[0].line, 4);
    assert!(slop_findings[0].content.contains("Create a new buffer"));
}

#[test]
fn ast_select_rule_only_checks_cancelled_branch_futures() {
    let source = r#"
async fn run(writer: &mut Writer, rx: &mut Receiver<()>, enabled: bool) {
    tokio::select! {
        _ = writer.write_all(b"payload"), if enabled => {},
        _ = rx.recv() => {
            writer.write_all(b"handler").await;
        },
    }
}

async fn safe_io(
    reader: &mut Reader,
    writer: &mut Writer,
    read_buffer: &mut [u8],
    write_buffer: &[u8],
) {
    tokio::select! {
        _ = reader.read(read_buffer) => {},
        _ = writer.write(write_buffer) => {},
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/worker.rs"), source);
    let cancellation: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-RUST-SELECT-CANCEL-UNSAFE")
        .collect();

    assert_eq!(cancellation.len(), 1);
    assert_eq!(cancellation[0].line, 3);
    assert_eq!(cancellation[0].evidence, rustok_doctor::Evidence::Ast);
    assert!(cancellation[0].message.contains("write_all"));
}

#[test]
fn ast_unsafe_send_sync_rule_skips_test_only_impls() {
    let source = r#"
unsafe impl Send for RawHandle {}
unsafe impl Sync for RawHandle {}

#[cfg(test)]
mod tests {
    unsafe impl Send for TestHandle {}
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/handle.rs"), source);
    let unsafe_impls: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-RUST-UNSAFE-SEND-SYNC-CONTRACT")
        .collect();

    assert_eq!(unsafe_impls.len(), 2);
    assert_eq!(unsafe_impls[0].line, 2);
    assert_eq!(unsafe_impls[1].line, 3);
    assert!(
        unsafe_impls
            .iter()
            .all(|finding| finding.confidence == rustok_doctor::Confidence::Heuristic)
    );
}

#[test]
fn ast_ffi_rule_only_flags_panic_risks_at_non_unwinding_boundaries() {
    let source = r#"
extern "C" fn callback() {
    maybe().unwrap();
}
extern "C-unwind" fn unwind_callback() {
    panic!("unwinding is part of this ABI");
}
extern "Rust" fn rust_callback() {
    panic!("the Rust ABI permits unwinding");
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/ffi.rs"), source);
    let ffi: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-RUST-FFI-UNWIND-BOUNDARY")
        .collect();

    assert_eq!(ffi.len(), 1);
    assert_eq!(ffi[0].line, 2);
    assert!(ffi[0].message.contains(".unwrap()"));
}

#[test]
fn ast_lock_order_rule_requires_opposite_live_guard_order() {
    let source = r#"
struct Store {
    first: Mutex<()>,
    second: Mutex<()>,
}

impl Store {
    async fn first_then_second(&self) {
        let _first = self.first.lock().await;
        let _second = self.second.lock().await;
    }

    async fn second_then_first(&self) {
        let second = self.second.lock().await;
        let _first = self.first.lock().await;
        drop(second);
    }

    async fn release_between(&self) {
        let first = self.first.lock().await;
        drop(first);
        let _second = self.second.lock().await;
    }
}
"#;
    let findings = rustok_doctor::scan_source(Path::new("src/store.rs"), source);
    let lock_orders: Vec<_> = findings
        .iter()
        .filter(|finding| finding.pattern_id == "P-RUST-LOCK-ORDER-CYCLE")
        .collect();

    assert_eq!(lock_orders.len(), 1);
    assert!(lock_orders[0].message.contains("first_then_second"));
    assert!(lock_orders[0].message.contains("second_then_first"));
}
