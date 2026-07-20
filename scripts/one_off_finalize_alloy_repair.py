from pathlib import Path


def replace_exact(path: str, old: str, new: str, expected: int = 1) -> None:
    target = Path(path)
    source = target.read_text()
    found = source.count(old)
    if found != expected:
        raise SystemExit(f"{path}: expected {expected} occurrence(s), found {found}: {old!r}")
    target.write_text(source.replace(old, new))


replace_exact(
    "crates/alloy/src/runner/test.rs",
    "crate::MAX_TEST_ERROR_LENGTH",
    "crate::model::MAX_TEST_ERROR_LENGTH",
)
replace_exact(
    "crates/alloy/src/runner/test.rs",
    "use crate::TEST_RUN_LEASE_SECONDS;",
    "use crate::model::TEST_RUN_LEASE_SECONDS;",
)
replace_exact(
    "crates/alloy/src/storage/memory.rs",
    "crate::test_run_lease_expires_at",
    "crate::model::test_run_lease_expires_at",
    2,
)
replace_exact(
    "crates/alloy/src/storage/sea_orm.rs",
    "crate::test_run_lease_expires_at",
    "crate::model::test_run_lease_expires_at",
    2,
)
replace_exact(
    "crates/alloy/src/runner/release.rs",
    "G: AlloyReleaseGovernance,",
    "G: AlloyReleaseGovernance + ?Sized,",
    2,
)
replace_exact(
    "crates/alloy/src/storage/sea_orm.rs",
    ".col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt))",
    ".col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt).into())",
    2,
)
replace_exact(
    "crates/alloy/src/api/dto.rs",
    "#[cfg(test)]\nmod tests {\n    use super::*;\n    use chrono::{TimeZone, Utc};",
    "#[cfg(test)]\nmod execution_log_tests {\n    use super::*;\n    use chrono::{TimeZone, Utc};",
)
replace_exact(
    "crates/alloy/src/lib.rs",
    '                EntityProxy::empty("invoice"),',
    '                EntityProxy::new("invoice-1", "invoice", std::collections::HashMap::new()),',
)
replace_exact(
    "crates/rustok-modules/src/migrations/m20260718_000031_artifact_data_exports.rs",
    '        manager\n            .get_connection()\n            .execute_unprepared("DROP TABLE module_artifact_data_exports")\n            .await\n',
    '        manager\n            .get_connection()\n            .execute_unprepared("DROP TABLE module_artifact_data_exports")\n            .await?;\n        Ok(())\n',
)
