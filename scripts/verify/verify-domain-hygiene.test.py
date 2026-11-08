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


if __name__ == "__main__":
    unittest.main()
