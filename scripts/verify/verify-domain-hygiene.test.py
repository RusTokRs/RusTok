#!/usr/bin/env python3
"""Automated regression and invariant unit tests for RusToK Domain Hygiene Scanner.

Verifies pattern detection, lexical block scoping, and false-positive elimination.
Runs with standard Python 3:
    python scripts/verify/verify-domain-hygiene.test.py
"""
import csv
import importlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# Import the scanner module under test
sys.path.insert(0, str(Path(__file__).resolve().parent))

scanner = importlib.import_module("verify-domain-hygiene")
scan_file = scanner.scan_file
resolve_crate_paths = scanner.resolve_crate_paths
generate_markdown_report = scanner.generate_markdown_report
generate_csv_report = scanner.generate_csv_report
extract_module_name = scanner.extract_module_name


class TestDomainHygieneScanner(unittest.TestCase):

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.test_root = Path(self.temp_dir.name)

    def tearDown(self):
        self.temp_dir.cleanup()

    def _create_file(self, rel_path: str, content: str) -> Path:
        p = self.test_root / rel_path
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(content, encoding="utf-8")
        return p

    def test_zero_include_detected(self):
        code = '''
        pub mod foo;
        include!("bad_twin.rs");
        pub fn run() {}
        '''
        f = self._create_file("src/service.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-ZERO-INCLUDE", pids)

    def test_out_dir_include_permitted(self):
        code = '''
        include!(concat!(env!("OUT_DIR"), "/generated.rs"));
        '''
        f = self._create_file("src/page_builder.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-ZERO-INCLUDE", pids)

    def test_anti_swallow_let_underscore(self):
        code = '''
        pub fn process_order() {
            let _ = dangerous_operation();
        }
        '''
        f = self._create_file("src/service.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-ANTI-SWALLOW", pids)

    def test_txn_rollback_swallow_detected(self):
        code = '''
        pub async fn handle() -> Result<(), Error> {
            let _ = transaction.rollback().await;
            Err(Error::Failed)
        }
        '''
        f = self._create_file("src/workflow.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-TXN-ROLLBACK-SWALLOW", pids)

    def test_zero_panic_in_prod_detected(self):
        code = '''
        pub fn parse(s: &str) -> Uuid {
            Uuid::parse_str(s).expect("must parse")
        }
        '''
        f = self._create_file("src/parser.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-ZERO-PANIC-PROD", pids)

    def test_test_context_ignores_unwrap(self):
        code = '''
        #[cfg(test)]
        mod tests {
            #[test]
            fn test_foo() {
                let x: Result<i32, ()> = Ok(42);
                assert_eq!(x.unwrap(), 42);
            }
        }
        '''
        f = self._create_file("src/lib.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-ZERO-PANIC-PROD", pids)

    def test_tokio_test_function_ignores_unwrap_and_contains(self):
        code = '''
        #[tokio::test]
        async fn test_async_workflow() {
            let val = some_async_call().await.unwrap();
            assert!(val.to_string().contains("expected message"));
        }
        '''
        f = self._create_file("src/integration_check.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-ZERO-PANIC-PROD", pids)
        self.assertNotIn("P-SQLSTATE-DISCRIM", pids)

    def test_id_entropy_12_chars_detected(self):
        code = '''
        pub const SHORT_ID_LEN: usize = 12;
        pub fn make_route(uuid: &Uuid) -> &str {
            &uuid.simple().to_string()[..12]
        }
        '''
        f = self._create_file("src/route.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-ID-ENTROPY", pids)

    def test_rbac_fail_closed_leak(self):
        code = '''
        pub fn check(scope: PermissionScope) -> Result<(), Error> {
            match scope {
                PermissionScope::All => Ok(()),
                PermissionScope::Own => Ok(()),
                PermissionScope::None => Ok(()),
            }
        }
        '''
        f = self._create_file("src/services/rbac.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-FAIL-CLOSED", pids)

    def test_raw_sql_interpolation_detected(self):
        code = '''
        pub async fn update(txn: &DatabaseTransaction, id: i32) -> Result<(), Error> {
            txn.execute_unprepared(&format!("UPDATE table SET x = 1 WHERE id = {}", id)).await?;
            Ok(())
        }
        '''
        f = self._create_file("src/repository.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-RAW-SQL-INTERP", pids)

    def test_tenant_fallback_leak_detected(self):
        code = '''
        pub fn create_context(tenant: Option<Uuid>) {
            let tenant_id = tenant.unwrap_or_default();
        }
        '''
        f = self._create_file("src/context.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-TENANT-FALLBACK-LEAK", pids)

    def test_batch_query_in_loop_detected(self):
        code = '''
        pub async fn load_all(ids: Vec<Uuid>, txn: &DatabaseTransaction) -> Result<(), Error> {
            for id in ids {
                let item = Entity::find_by_id(id).one(txn).await?;
            }
            Ok(())
        }
        '''
        f = self._create_file("src/services/loader.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-BATCH-QUERY", pids)

    def test_sync_lock_held_across_await_detected(self):
        code = '''
        pub async fn execute(&self) -> Result<(), Error> {
            let guard = self.sync_mutex.lock().unwrap();
            remote_call().await?;
            Ok(())
        }
        '''
        f = self._create_file("src/service.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-SYNC-LOCK-AWAIT", pids)

    def test_sync_lock_scoped_in_block_dropped_before_await(self):
        code = '''
        pub async fn execute(&self) -> Result<(), Error> {
            let data = {
                let mut guard = self.sync_mutex.lock().unwrap();
                std::mem::take(&mut *guard)
            };
            remote_call().await?;
            Ok(())
        }
        '''
        f = self._create_file("src/service.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-SYNC-LOCK-AWAIT", pids)

    def test_hotpath_alloc_clone_before_validate(self):
        code = '''
        pub fn update_category(input: CategoryInput) -> Result<(), Error> {
            let name = input.name.clone();
            validate_category_name(&name)?;
            Ok(())
        }
        '''
        f = self._create_file("src/services/category.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-HOTPATH-ALLOC", pids)

    def test_smart_crate_resolution_finds_forum_and_commerce(self):
        paths = resolve_crate_paths("forum,commerce")
        names = [p.name for p in paths]
        self.assertIn("rustok-forum", names)
        self.assertIn("rustok-commerce", names)

    def test_markdown_report_generation(self):
        findings = [
            scanner.Finding(
                pattern_id="P-FAIL-CLOSED",
                rule_name="Fail-Closed Security Floor",
                severity=scanner.Severity.P0_CRITICAL,
                file_path=Path("crates/modules/rustok-forum/src/services/rbac.rs"),
                line_number=32,
                line_content="PermissionScope::None => Ok(())",
                message="Must resolve to Err(Forbidden)",
                suggested_fix="Err(Forbidden)"
            )
        ]
        report = generate_markdown_report(findings, 10, "test target")
        self.assertIn("RusToK Domain Hygiene & Invariant Verification Report", report)
        self.assertIn("P-FAIL-CLOSED", report)
        self.assertIn("| heuristic |", report)
        self.assertIn("`rustok-forum`", report)

    def test_sync_sleep_in_async_code(self):
        code = '''
        pub async fn run_job() {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        '''
        f = self._create_file("src/job.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-SYNC-SLEEP-AWAIT", pids)

    def test_csv_report_generation(self):
        findings = [
            scanner.Finding(
                pattern_id="P-FAIL-CLOSED",
                rule_name="Fail-Closed Security Floor",
                severity=scanner.Severity.P0_CRITICAL,
                file_path=Path("crates/modules/rustok-forum/src/services/rbac.rs"),
                line_number=32,
                line_content="PermissionScope::None => Ok(())",
                message="Must resolve to Err(Forbidden)",
                suggested_fix="Err(Forbidden)"
            )
        ]
        csv_out = generate_csv_report(findings)
        [row] = csv.DictReader(io.StringIO(csv_out))
        self.assertEqual(row["pattern_id"], "P-FAIL-CLOSED")
        self.assertEqual(row["severity"], "P0-CRITICAL")
        self.assertEqual(row["confidence"], "heuristic")
        self.assertEqual(row["file"], "crates/modules/rustok-forum/src/services/rbac.rs")

    def test_url_containing_double_slashes_not_split_as_comment(self):
        code = '''
        pub fn connect() {
            let url = "https://example.com/api";
            let _ = dangerous_operation();
        }
        '''
        f = self._create_file("src/service.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-ANTI-SWALLOW", pids)

    def test_literals_preserve_async_scope_and_hide_rule_markers(self):
        literals = {
            "raw": 'r#"<div class="test">} panic!(example)</div>"#',
            "raw multiline": 'br##"\nlet _ = not_code();\n}"# /*\n"##',
            "normal": '"/* } panic!(example)"',
            "normal multiline": '"line one\nlet _ = not_code();\n}"',
            "escaped backslash": '"C:\\\\folder\\\\\nsubfolder\\\\}"',
            "char": "'}'",
        }
        for name, literal in literals.items():
            with self.subTest(literal=name):
                code = (
                    "async fn run() {\n"
                    f"    let template = {literal};\n"
                    "    std::thread::sleep(duration);\n"
                    "}\n"
                    "fn run_sync() {\n"
                    "    std::thread::sleep(duration);\n"
                    "}\n"
                )
                findings = scan_file(self._create_file("src/render.rs", code))
                self.assertEqual(
                    [(finding.pattern_id, finding.line_number) for finding in findings],
                    [("P-SYNC-SLEEP-AWAIT", 3 + literal.count("\n"))],
                )

    def test_nested_block_comments_hide_code_but_allow_real_code_after_them(self):
        code = (
            "fn run() {\n"
            '    /* /* inner */ panic!("not code");\n'
            '    */ panic!("real code");\n'
            "}\n"
        )
        findings = scan_file(self._create_file("src/service.rs", code))
        self.assertEqual(
            [(finding.pattern_id, finding.line_number) for finding in findings],
            [("P-ZERO-PANIC-PROD", 3)],
        )

    def test_services_relative_path_correctly_classified(self):
        code = '''
        pub async fn update_user(input: UserInput) -> Result<(), Error> {
            let name = input.name.clone();
            validate_user(&name)?;
            Ok(())
        }
        '''
        f = self._create_file("services/user.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-HOTPATH-ALLOC", pids)

    def test_tests_relative_path_correctly_classified(self):
        self.assertTrue(scanner.is_test_or_bench(Path("tests/integration.rs")))
        self.assertTrue(scanner.is_test_or_bench(Path("benches/benchmark.rs")))
        self.assertTrue(scanner.is_test_or_bench(Path("fixtures/mock_data.rs")))

    def test_static_pub_regex_expect_not_flagged_as_prod_panic(self):
        code = '''
        pub static RE_TEST: Lazy<Regex> = Lazy::new(|| {
            Regex::new("^[a-z]+$").expect("valid regex")
        });
        '''
        f = self._create_file("src/regex_helper.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-ZERO-PANIC-PROD", pids)

    def test_user_message_contains_not_flagged_as_sqlstate_discrim(self):
        code = '''
        pub fn validate_post(message: &str) -> Result<(), Error> {
            if message.contains("bad_word") {
                return Err(Error::BadPost);
            }
            Ok(())
        }
        '''
        f = self._create_file("src/services/post.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-SQLSTATE-DISCRIM", pids)

    def test_sql_error_contains_flagged_as_sqlstate_discrim(self):
        code = '''
        pub fn map_error(db_err: &DbErr) -> Error {
            if db_err.to_string().contains("duplicate key") {
                return Error::Conflict;
            }
            Error::Internal
        }
        '''
        f = self._create_file("src/services/db_error.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-SQLSTATE-DISCRIM", pids)

    def test_leptos_signal_read_is_exempt_only_inside_reactive_delimiters(self):
        scopes = (
            ("create_effect(\n        move |_| {", "});"),
            ("create_memo(move |_| {", "});"),
            ("Effect::new(move |_| {", "});"),
            ("Memo::new(move |_| {", "});"),
            ("view! {", "};"),
            ("view! [", "];"),
        )
        for opening, closing in scopes:
            with self.subTest(opening=opening):
                code = (
                    "fn setup() {\n"
                    "    // create_effect(move |_| {\n"
                    '    let note = "view! {";\n'
                    "    let _ = signal.get();\n"
                    f"    {opening}\n"
                    + '        work("] })");\n' * 10
                    + "        let _ = signal.get();\n"
                    f"    {closing}\n"
                    "    let _ = signal.get();\n"
                    "}\n"
                )
                findings = scan_file(self._create_file("src/component.rs", code))
                self.assertEqual(
                    [(finding.pattern_id, finding.line_number) for finding in findings],
                    [
                        ("P-LEPTOS-DEAD-SIGNAL", 4),
                        ("P-LEPTOS-DEAD-SIGNAL", 18 + opening.count("\n")),
                    ],
                )

    def test_tenant_nil_uuid_fallback_detected(self):
        code = '''
        pub fn resolve(tenant: Option<Uuid>) -> Uuid {
            tenant.unwrap_or(Uuid::nil())
        }
        '''
        f = self._create_file("src/auth.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-TENANT-FALLBACK-LEAK", pids)

    def test_tenant_chained_as_ref_unwrap_or_default_detected(self):
        code = '''
        pub fn resolve(tenant: Option<Uuid>) -> Uuid {
            tenant.as_ref().copied().unwrap_or_default()
        }
        '''
        f = self._create_file("src/auth.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-TENANT-FALLBACK-LEAK", pids)

    def test_tenant_unwrap_or_else_nil_detected(self):
        code = '''
        pub fn resolve(tenant: Option<Uuid>) -> Uuid {
            tenant.unwrap_or_else(|| Uuid::nil())
        }
        '''
        f = self._create_file("src/auth.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-TENANT-FALLBACK-LEAK", pids)

    def test_batch_query_in_loop_with_self_db_detected(self):
        code = '''
        pub async fn fetch_all(&self, ids: Vec<Uuid>) -> Result<(), Error> {
            for id in ids {
                let row = Entity::find_by_id(id).one(&self.db).await?;
            }
            Ok(())
        }
        '''
        f = self._create_file("src/services/user.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-BATCH-QUERY", pids)

    def test_raw_sql_multiline_format_detected(self):
        code = '''
        pub async fn raw_update(db: &DatabaseConnection, id: i32) -> Result<(), Error> {
            db.execute_unprepared(
                &format!("UPDATE table SET x = {}", id)
            ).await?;
            Ok(())
        }
        '''
        f = self._create_file("src/services/store.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-RAW-SQL-INTERP", pids)

    def test_rbac_multiline_none_arm_detected(self):
        code = '''
        pub fn check(scope: PermissionScope) -> Result<(), Error> {
            match scope {
                PermissionScope::None =>
                    Ok(()),
                _ => Ok(()),
            }
        }
        '''
        f = self._create_file("src/services/rbac.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-FAIL-CLOSED", pids)

    def test_hotpath_alloc_skips_comment_lines(self):
        code = '''
        pub fn process(input: Input) -> Result<(), Error> {
            let name = input.name.clone();
            // check_permission was verified earlier
            println!("Proceeding with name: {}", name);
            Ok(())
        }
        '''
        f = self._create_file("src/services/processor.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-HOTPATH-ALLOC", pids)

    def test_structured_trace_skips_private_async_fn(self):
        code = '''
        async fn delete_internal_relations<C: ConnectionTrait>(id: Uuid) -> Result<(), Error> {
            Ok(())
        }
        '''
        f = self._create_file("src/services/relation.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-STRUCTURED-TRACE", pids)

    def test_structured_trace_flags_public_async_mutation_fn(self):
        code = '''
        pub async fn update_status(&self, id: Uuid) -> Result<(), Error> {
            Ok(())
        }
        '''
        f = self._create_file("src/services/relation.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-STRUCTURED-TRACE", pids)

    def test_explicit_panic_in_production_service_detected(self):
        code = '''
        pub fn check_state(status: &str) -> Result<(), Error> {
            if status == "corrupted" {
                panic!("state corrupted");
            }
            Ok(())
        }
        '''
        f = self._create_file("src/services/validator.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertIn("P-ZERO-PANIC-PROD", pids)

    def test_dead_code_in_migration_not_flagged(self):
        code = '''
        #[allow(dead_code)]
        enum Users { Id, Name }
        '''
        f = self._create_file("src/migrations/m20260101_init.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-DEAD-TWIN", pids)

    def assert_rule_lines(self, pattern_id, code, expected):
        findings = scanner.scan_source(Path("src/service.rs"), code)
        self.assertEqual([f.line_number for f in findings if f.pattern_id == pattern_id], expected)

    def test_rbac_flags_explicit_success_and_accepts_denial_values(self):
        for value, expected in (("Ok(true)", [2]), ("Ok(())", [2]), ("true", [2]),
                                ("Ok(false)", []), ("Ok(None)", []), ("false", []),
                                ("true && has_other_permission", [])):
            with self.subTest(value=value):
                code = f"match scope {{\n    PermissionScope::None => {value},\n}}\n"
                self.assert_rule_lines("P-FAIL-CLOSED", code, expected)

    def test_short_id_rule_requires_id_evidence(self):
        code = (
            "const TOPIC_ROUTE_SHORT_ID_LEN: usize = 12;\n"
            "fn head(data: &[u8]) -> &[u8] { &data[..12] }\n"
            'const SCHEMA: &str = "CREATE TABLE items (short_id VARCHAR(12))";\n'
            'const OTHER_SCHEMA: &str = "CREATE TABLE items (name VARCHAR(12))";\n'
        )
        self.assert_rule_lines("P-ID-ENTROPY", code, [1, 3])

    def test_tenant_binding_fallback_is_distinct_from_optional_owner_or_locale(self):
        code = (
            "fn resolve(owner: Option<Uuid>, tenant: Tenant) -> Uuid {\n"
            "    let owner_id = owner.unwrap_or_default();\n"
            "    let locale = tenant.locale().unwrap_or_default();\n"
            "    let tenant_id: Uuid = owner.unwrap_or_else(Uuid::nil);\n"
            "    tenant_id\n"
            "}\n"
        )
        self.assert_rule_lines("P-TENANT-FALLBACK-LEAK", code, [4])

    def test_discarded_call_is_distinct_from_plain_value(self):
        code = "fn run(size: usize) {\n    let _ = size;\n    let _ = operation();\n}\n"
        self.assert_rule_lines("P-ANTI-SWALLOW", code, [3])

    def test_commit_diagnostic_has_commit_specific_advice(self):
        code = "async fn run(transaction: DatabaseTransaction) {\n    let _ = transaction.commit().await;\n}\n"
        [finding] = scanner.scan_source(Path("src/service.rs"), code)
        self.assertEqual(finding.pattern_id, "P-TXN-ROLLBACK-SWALLOW")
        self.assertIn("commit", finding.suggested_fix)
        self.assertNotIn("rollback", finding.suggested_fix)

    def test_sql_format_tracks_direct_and_local_values_within_scope(self):
        code = (
            "async fn write(db: &DatabaseConnection, input: &str) {\n"
            '    db.execute_unprepared(&format!("SELECT * FROM {table}", table = "items")).await;\n'
            '    db.execute_unprepared(&format!("SELECT * FROM {}", "items")).await;\n'
            '    let mut sql = format!("DELETE FROM items WHERE name = {input}");\n'
            "    {\n"
            '        let sql = "SELECT 1";\n'
            "        db.execute_unprepared(sql).await;\n"
            "    }\n"
            "    db.execute_unprepared(&sql).await;\n"
            '    db.execute_unprepared(&format!("SELECT * FROM {}", input)).await;\n'
            '    db.execute_unprepared(&format!("SELECT \'{{input}}\'")).await;\n'
            '    sql = "SELECT 1".to_string();\n'
            "    db.execute_unprepared(&sql).await;\n"
            "}\n"
            "async fn read(db: &DatabaseConnection, sql: &str) {\n"
            "    db.execute_unprepared(sql).await;\n"
            "}\n"
        )
        self.assert_rule_lines("P-RAW-SQL-INTERP", code, [9, 10])

    def test_outbox_checks_transaction_evidence_in_same_function(self):
        code = (
            "use sea_orm::DatabaseTransaction;\n"
            "async fn write(txn: &DatabaseTransaction, event_bus: EventBus) {\n"
            "    event_bus.publish_in_tx(txn, event()).await;\n"
            "    event_bus.publish(event()).await;\n"
            "}\n"
            "async fn announce(event_bus: EventBus) {\n"
            "    event_bus.publish(startup_event()).await;\n"
            "}\n"
            "async fn write_with_begin(db: &DatabaseConnection, event_bus: EventBus) {\n"
            "    event_bus.publish(event()).await;\n"
            "    let txn = db.begin().await;\n"
            "}\n"
        )
        self.assert_rule_lines("P-OUTBOX-DETACHED", code, [4, 10])

    def test_awaited_lock_and_conditional_guard_drop(self):
        code = (
            "async fn async_lock(mutex: &tokio::sync::Mutex<u64>) {\n"
            "    let guard = mutex.lock()\n"
            "        .await;\n"
            "    remote_call().await;\n"
            "}\n"
            "async fn conditional(mutex: &std::sync::Mutex<u64>, early: bool) -> Result<(), Error> {\n"
            "    let guard = mutex.lock()?;\n"
            "    if early {\n"
            "        drop(guard);\n"
            "    }\n"
            "    remote_call().await;\n"
            "    Ok(())\n"
            "}\n"
            "async fn always_drops(mutex: &std::sync::Mutex<u64>) -> Result<(), Error> {\n"
            "    let guard = mutex.lock()?;\n"
            "    drop(guard);\n"
            "    remote_call().await;\n"
            "    Ok(())\n"
            "}\n"
        )
        self.assert_rule_lines("P-SYNC-LOCK-AWAIT", code, [11])

    def test_include_detection_requires_source_policy_match(self):
        code = (
            'try_include!("fixture.rs");\n'
            'include!(concat!(env!("OUT_DIR"), "/generated.rs"));\n'
            "#[cfg(test)]\n"
            "mod fixture_support {\n"
            '    include!("fixture.rs");\n'
            "}\n"
            'include!("OUT_DIR/handwritten.rs");\n'
        )
        [finding] = scanner.scan_source(Path("src/lib.rs"), code)
        self.assertEqual(
            (finding.pattern_id, finding.line_number, finding.confidence),
            ("P-ZERO-INCLUDE", 7, scanner.Confidence.HIGH),
        )

    def test_dead_twin_marker_requires_module_wide_suppression(self):
        code = "#![allow(unused_imports, dead_code)]\n#[allow(dead_code)]\nstruct Fixture;\n"
        self.assert_rule_lines("P-DEAD-TWIN", code, [1])

    def test_incomplete_code_distinguishes_macros_and_comments_from_strings(self):
        code = (
            'const MARKER: &str = "TODO: todo!()";\n'
            'fn stub() { unimplemented!("no implementation"); }\n'
            '// FIXME: verify the owner contract\n'
        )
        self.assert_rule_lines("P-UNFINISHED-CODE", code, [2, 3])

    def test_extract_module_name_resolved(self):
        mod_name = extract_module_name(Path("crates/modules/rustok-forum/src/lib.rs"))
        self.assertEqual(mod_name, "rustok-forum")


class TestDomainHygieneCLI(unittest.TestCase):
    def setUp(self):
        temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(temp_dir.cleanup)
        self.root = Path(temp_dir.name)
        self.script = self.root / "scripts" / "verify" / Path(scanner.__file__).name
        self.script.parent.mkdir(parents=True)
        shutil.copyfile(scanner.__file__, self.script)
        self.env = {
            **{name: value for name, value in os.environ.items() if not name.startswith("GIT_")},
            "GITHUB_ACTIONS": "false",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": os.devnull,
        }

    def run_command(self, *args):
        return subprocess.run(
            args, cwd=self.root, env=self.env, capture_output=True, encoding="utf-8", timeout=30
        )

    def git(self, *args):
        result = self.run_command("git", *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result

    def cli(self, *args):
        return self.run_command(sys.executable, "-B", str(self.script), *args)

    def test_staged_scan_uses_index_even_if_worktree_file_is_replaced_or_missing(self):
        self.git("init", "-q", "--template=")
        staged_bad = self.root / "staged bad.rs"
        staged_clean = self.root / "staged clean.rs"
        removed = self.root / "missing.rs"
        bad_code = b'mod other;\ninclude!("handwritten.rs");\n'
        clean_code = b"fn run() {}\n"
        staged_bad.write_bytes(bad_code)
        staged_clean.write_bytes(clean_code)
        removed.write_bytes(bad_code)
        self.git("add", "--", staged_bad.name, staged_clean.name, removed.name)
        staged_bad.write_bytes(clean_code)
        staged_clean.write_bytes(bad_code)
        removed.unlink()

        result = self.cli("--staged", "--strict", "--json")
        self.assertEqual(result.returncode, 1, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["scanned_files"], 3)
        self.assertEqual(
            [(finding["file"], finding["pattern_id"], finding["line"]) for finding in report["findings"]],
            [
                ("missing.rs", "P-ZERO-INCLUDE", 2),
                ("staged bad.rs", "P-ZERO-INCLUDE", 2),
            ],
        )
        staged_patch = self.cli("--staged", "--patch", "staged.patch")
        self.assertEqual(staged_patch.returncode, 0, staged_patch.stderr)
        self.git("apply", "--cached", "--check", "staged.patch")

    def test_diff_with_unknown_ref_returns_operational_error(self):
        self.git("init", "-q", "--template=")
        result = self.cli("--diff", "missing-test-ref", "--strict", "--json")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("missing-test-ref", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_fail_on_exit_codes_match_in_text_and_json(self):
        path = self.root / "service.rs"
        path.write_bytes(
            b'include!("handwritten.rs");\n'
            b"fn allowed(scope: PermissionScope) -> bool {\n"
            b"    match scope {\n"
            b"        PermissionScope::None => true,\n"
            b"        _ => false,\n"
            b"    }\n"
            b"}\n"
        )
        for threshold, expected in (("P0", 0), ("P1", 1), ("P2", 1)):
            for output_flags in ((), ("--json",)):
                with self.subTest(threshold=threshold, output=output_flags):
                    result = self.cli("--path", path.name, "--fail-on", threshold, *output_flags)
                    self.assertEqual(result.returncode, expected, result.stderr)
                    if output_flags:
                        report = json.loads(result.stdout)
                        self.assertEqual(
                            [(f["pattern_id"], f["severity"], f["confidence"]) for f in report["findings"]],
                            [
                                ("P-ZERO-INCLUDE", "P1-HIGH", "high"),
                                ("P-FAIL-CLOSED", "P0-CRITICAL", "heuristic"),
                            ],
                        )
                    else:
                        self.assertIn("[heuristic]", result.stdout)
                        self.assertIn("[high]", result.stdout)

        result = self.cli("--path", path.name, "--strict", "--ignore-rule", "P-ZERO-INCLUDE", "--github-annotations")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("::warning file=service.rs,line=4,title=P-FAIL-CLOSED::[heuristic]", result.stdout)
        self.assertNotIn("::error", result.stdout)

    def test_patch_applies_and_keeps_original_code_for_multiple_findings(self):
        self.git("init", "-q", "--template=")
        path = self.root / "source file.rs"
        original = b'fn run() {\r\n    let _ = dangerous_operation().unwrap();\r\n}'
        path.write_bytes(original)

        result = self.cli("--path", path.name, "--patch", "out.patch")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.git("apply", "--check", "out.patch")
        self.git("apply", "out.patch")
        patched = path.read_bytes()
        self.assertEqual(patched[:len(original)], original)
        self.assertIn(b"// REMEDIATION (P-ANTI-SWALLOW, line 2):", patched)
        self.assertIn(b"// REMEDIATION (P-ZERO-PANIC-PROD, line 2):", patched)


if __name__ == "__main__":
    unittest.main()
