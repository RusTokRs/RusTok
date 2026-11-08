#!/usr/bin/env python3
"""Automated regression and invariant unit tests for RusToK Domain Hygiene Scanner.

Verifies pattern detection, lexical block scoping, and false-positive elimination.
Runs with standard Python 3:
    python scripts/verify/verify-domain-hygiene.test.py
"""
import sys
import tempfile
import unittest
from pathlib import Path

# Import the scanner module under test
sys.path.insert(0, str(Path(__file__).resolve().parent))
import importlib

scanner = importlib.import_module("verify-domain-hygiene")
scan_file = scanner.scan_file
resolve_crate_paths = scanner.resolve_crate_paths
generate_markdown_report = scanner.generate_markdown_report
generate_csv_report = scanner.generate_csv_report
generate_unified_patch = scanner.generate_unified_patch
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

    def test_unified_patch_generation(self):
        findings = [
            scanner.Finding(
                pattern_id="P-ANTI-SWALLOW",
                rule_name="Zero Error Swallowing",
                severity=scanner.Severity.P1_HIGH,
                file_path=Path("crates/modules/rustok-customer/src/lib.rs"),
                line_number=42,
                line_content="let _ = dangerous_call();",
                message="Swallowed error",
                suggested_fix="if let Err(e) = dangerous_call() { warn!(%e); }"
            )
        ]
        patch = generate_unified_patch(findings)
        self.assertIn("--- a/crates/modules/rustok-customer/src/lib.rs", patch)
        self.assertIn("+++ b/crates/modules/rustok-customer/src/lib.rs", patch)
        self.assertIn("@@ -42,1 +42,1 @@", patch)
        self.assertIn("-let _ = dangerous_call();", patch)
        self.assertIn("+// REMEDIATION (P-ANTI-SWALLOW):", patch)

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
        self.assertIn("pattern_id,rule_name,severity,file,line,snippet,message,suggested_fix", csv_out)
        self.assertIn("P-FAIL-CLOSED", csv_out)
        self.assertIn("crates/modules/rustok-forum/src/services/rbac.rs", csv_out)

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

    def test_raw_string_with_quotes_and_braces_does_not_corrupt_brace_depth(self):
        code = '''
        pub fn render_html() -> String {
            let tmpl = r#"<div class="test">{val}</div>"#;
            tmpl.to_string()
        }
        '''
        f = self._create_file("src/render.rs", code)
        findings = scan_file(f)
        self.assertEqual(len(findings), 0)

    def test_multiline_raw_string_ignores_internal_braces(self):
        code = '''
        pub fn get_sql() -> &'static str {
            let query = r#"
                SELECT * FROM users
                WHERE data = '{"role": "admin"}'
            "#;
            query
        }
        '''
        f = self._create_file("src/query.rs", code)
        findings = scan_file(f)
        self.assertEqual(len(findings), 0)

    def test_multiline_double_quoted_string_ignores_internal_braces(self):
        code = '''
        pub fn get_json() -> &'static str {
            let s = "line one {
                line two
            }";
            s
        }
        '''
        f = self._create_file("src/strings.rs", code)
        findings = scan_file(f)
        self.assertEqual(len(findings), 0)

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

    def test_leptos_signal_get_inside_create_effect_not_flagged(self):
        code = '''
        pub fn setup_counter() {
            create_effect(move |_| {
                let _ = count.get();
            });
        }
        '''
        f = self._create_file("src/component.rs", code)
        findings = scan_file(f)
        pids = [item.pattern_id for item in findings]
        self.assertNotIn("P-LEPTOS-DEAD-SIGNAL", pids)

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
            tenant.as_ref().unwrap_or_default()
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

    def test_multiline_string_closing_with_escaped_backslash(self):
        code = '''
        pub fn get_path() -> &'static str {
            let s = "C:\\\\folder\\\\
            subfolder\\\\";
            s
        }
        '''
        f = self._create_file("src/path_helper.rs", code)
        findings = scan_file(f)
        self.assertEqual(len(findings), 0)

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

    def test_extract_module_name_resolved(self):
        mod_name = extract_module_name(Path("crates/modules/rustok-forum/src/lib.rs"))
        self.assertEqual(mod_name, "rustok-forum")


if __name__ == "__main__":
    unittest.main()
