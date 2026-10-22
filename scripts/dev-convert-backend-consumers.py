#!/usr/bin/env python3
"""One-shot H-1c converter: rewrite data-helper consumers to SupportedBackend.

Applies the same rules as the data/ conversion to every crate file that
imports placeholder/uuid_value/now_expression/uuid_from_row from
crate::data (the first three helpers are deleted in favour of
SupportedBackend methods). Backend bindings validate through the file's
canonical error mapper. Strictly syntactic; leftovers (remaining
DbBackend mentions, wildcard match arms) are reported for hand fixing.
"""
import re
import sys

SRC = "crates/modules/rustok-modules/src/"

FN_MAPPER = {
    "artifact_node_reconciliation.rs": "store_error",
    "build.rs": "persistence_error",
    "data_copier.rs": "storage_error",
    "data_object_migration.rs": "storage_error",
    "data_post_purge_recovery.rs": "storage_error",
    "data_snapshot.rs": "snapshot_storage_error",
    "data_snapshot_intents.rs": "storage_error",
    "delivery_descriptor.rs": "storage_error",
    "distribution.rs": "store_error",
    "distribution_bootstrap.rs": "store_error",
    "distribution_release.rs": "store_error",
    "distribution_rollout.rs": "store_error",
    "event_delivery.rs": "storage_error",
    "execution_audit.rs": "storage_error",
    "policy_revision_consumer.rs": "storage_error",
    "promotion.rs": "store_error",
    "queue_drain.rs": "storage_error",
    "schedule_delivery.rs": "storage_error",
    "schedule_materializer.rs": "storage_error",
    "secrets.rs": "storage_error",
    "security_state.rs": "store_error",
    "transition_service.rs": "database_error",
}
CLOSURE_MAPPER = {
    "composition.rs": "|error| ModuleCompositionError::Store(error.to_string())",
    "operations_tool.rs": "|e| OperationsToolError::Storage(e.to_string())",
    "source_object.rs": "|e| SourceObjectError::Storage(e.to_string())",
    "data_snapshot_readiness.rs": "|e| SnapshotReadinessError::Storage(e.to_string())",
}

B = r"(tx_backend|backend)"


def convert(name: str, mapper: str) -> None:
    path = SRC + name
    s = open(path).read()
    orig = s

    # bindings
    s = re.sub(
        r"let (tx_)?backend = ([\w.]+)\.get_database_backend\(\);",
        lambda m: f"let {m.group(1) or ''}backend = SupportedBackend::new({m.group(2)}.get_database_backend()).map_err({mapper})?;",
        s,
    )
    # helper calls -> methods
    s = re.sub(r"placeholder\(" + B + r",\s*([^()]+)\)", r"\1.placeholder(\2)", s)
    s = re.sub(r"uuid_value\(([^,()]+),\s*" + B + r"\)", r"\2.uuid_value(\1)", s)
    s = re.sub(r"now_expression\(" + B + r"\)", r"\1.now_sql()", s)
    # Statement first args
    s = re.sub(
        r"Statement::from_sql_and_values\(" + B + ",",
        r"Statement::from_sql_and_values(\1.db_backend(),",
        s,
    )
    s = re.sub(
        r"Statement::from_string\(" + B + ",", r"Statement::from_string(\1.db_backend(),", s
    )
    s = re.sub(
        r"(from_sql_and_values|from_string)\(\n(\s*)" + B + ",",
        r"\1(\n\2\3.db_backend(),",
        s,
    )
    # params & arms
    s = s.replace("backend: DbBackend,", "backend: SupportedBackend,")
    s = s.replace("backend: DbBackend)", "backend: SupportedBackend)")
    s = s.replace("DbBackend::Postgres =>", "SupportedBackend::Postgres =>")
    s = s.replace("DbBackend::Sqlite =>", "SupportedBackend::Sqlite =>")
    # drop deleted helpers from data::{...} imports (keep uuid_from_row)
    def clean_data_import(m: re.Match) -> str:
        items = m.group(1)
        for dead in ("placeholder", "uuid_value", "now_expression"):
            items = re.sub(rf"\b{dead},\s*", "", items)
            items = re.sub(rf",\s*\b{dead}\b", "", items)
        return "data::{" + items + "}"

    s = re.sub(r"data::\{([^}]*)\}", clean_data_import, s, flags=re.S)
    # single-item forms
    s = re.sub(r"use crate::data::(placeholder|uuid_value|now_expression);\n", "", s)
    # add the SupportedBackend import if the file now mentions the type
    if "SupportedBackend" in s and "backend::SupportedBackend" not in s:
        s = s.replace(
            "use crate::", "use crate::backend::SupportedBackend;\nuse crate::", 1
        )
    # drop now-unused DbBackend import token
    if "DbBackend" in s:
        body = re.sub(r"use [^;]*DbBackend[^;]*;", "", s, count=1)
        if "DbBackend" not in body:
            s = s.replace("DbBackend, ", "")
            s = s.replace(", DbBackend}", "}")

    if s != orig:
        open(path, "w").write(s)
        print("converted", name)
    leftovers = [
        (i + 1, line.strip())
        for i, line in enumerate(s.splitlines())
        if "DbBackend" in line or "get_database_backend()" in line and "SupportedBackend::new" not in line
    ]
    for ln, text in leftovers:
        print(f"  LEFTOVER {name}:{ln}: {text[:110]}")


def main() -> None:
    for name, mapper in FN_MAPPER.items():
        convert(name, mapper)
    for name, mapper in CLOSURE_MAPPER.items():
        convert(name, mapper)


if __name__ == "__main__":
    sys.exit(main())
