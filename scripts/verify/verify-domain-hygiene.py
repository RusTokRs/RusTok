#!/usr/bin/env python3
"""RusToK Domain Logic, Invariant Integrity & Anti-Slop Scanner.

Codified against docs/standards/logic-hygiene-patterns.md.
Scans Rust crates and modules for known anti-patterns, slop, and logic bugs:
  - P-ZERO-INCLUDE: file-stitching macros include!("*.rs")
  - P-ANTI-SWALLOW: swallowed errors let _ =, dummy black_box suppression
  - P-TXN-ROLLBACK-SWALLOW: swallowed transaction rollback/commit holding locks until drop
  - P-ZERO-PANIC-PROD: unwrap() and expect() in production service paths
  - P-ID-ENTROPY: obsolete 48-bit short_id (VARCHAR(12), [..12], left(..., 12))
  - P-FAIL-CLOSED: RBAC PermissionScope::None => Ok / true leakage
  - P-SQLSTATE-DISCRIM: brittle string matching on database error messages
  - P-BATCH-QUERY: N+1 queries in loops (.one(txn) / .all(txn) inside loops)
  - P-DEAD-TWIN: module-level #[allow(dead_code)] / #[allow(unused)] slop
  - P-RAW-SQL-INTERP: execute_unprepared(&format!(...)) SQL injection risk
  - P-LEPTOS-DEAD-SIGNAL: let _ = signal.get() dead reactive read in UI
  - P-TENANT-FALLBACK-LEAK: unwrap_or_default() on tenant_id / security context
  - P-UNFINISHED-CODE: lingering todo!(), FIXME, or TODO markers in production
  - P-OUTBOX-DETACHED: event_bus.publish() in transactional contexts (missing _in_tx)
  - P-SYNC-LOCK-AWAIT: std::sync::Mutex or parking_lot guard held across .await points
  - P-HOTPATH-ALLOC: premature clone of input before validation gates
  - P-STRUCTURED-TRACE: uninstrumented domain service mutation entrypoints
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Iterator

# Ensure UTF-8 output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent


class Severity(str, Enum):
    P0_CRITICAL = "P0-CRITICAL"
    P1_HIGH = "P1-HIGH"
    P2_MEDIUM = "P2-MEDIUM"


@dataclass
class Finding:
    pattern_id: str
    rule_name: str
    severity: Severity
    file_path: Path
    line_number: int
    line_content: str
    message: str
    suggested_fix: str | None = None


# Compiled regex patterns for AST-lite inspection
RE_INCLUDE = re.compile(r'include!\s*\(\s*"([^"]+\.rs)"\s*\)')
RE_LET_UNDERSCORE = re.compile(r'^\s*let\s+_\s*=')
RE_TXN_ROLLBACK_COMMIT = re.compile(
    r'^\s*let\s+_\s*=\s*(?:\w+\.)?(?:transaction|txn|tx|lock)\s*\.\s*(?:rollback|commit)\s*\('
)
RE_BLACK_BOX_SUPPRESS = re.compile(r'\b(?:std::hint::)?black_box\s*\(')
RE_EXPECT_UNWRAP = re.compile(r'\.(?:expect|unwrap)\s*\(')
RE_UNWRAP_ERR = re.compile(r'\.unwrap_err\s*\(')
RE_SHORT_ID_12 = re.compile(
    r'(?:VARCHAR\s*\(\s*12\s*\)|\[\s*0?\s*\.\.\s*12\s*\]|\"000000000000\"|left\s*\([^,]+,\s*12\s*\)|SHORT_ID_LEN(?:GTH)?\s*(?::\s*usize)?\s*=\s*12\b)'
)
RE_RBAC_LEAK = re.compile(r'PermissionScope::None\s*=>\s*(?:\{?\s*(?:return\s+)?)?(?:Ok\b|true\b)')
RE_ERR_CONTAINS = re.compile(r'\b(?:message|msg|err|error|db_err|e)\s*(?:\.\s*to_string\s*\(\s*\))?\s*\.\s*contains\s*\(\s*"')
RE_QUERY_IN_TXN = re.compile(r'\.(?:one|all|one_by_id|query_one|query_all)\s*\(\s*(?:&?\w*txn|&?\w*tx|&db)\s*\)\.await')
RE_FOR_OR_WHILE_LOOP = re.compile(r'\b(?:for\s+\w+\s+in\b|while\b)')
RE_ALLOW_DEAD_CODE = re.compile(r'^\s*#!?\[allow\((?:dead_code|unused|unused_variables)\)\]')
RE_RAW_SQL_INTERP = re.compile(r'\.execute_unprepared\s*\(\s*&format!\s*\(')
RE_LEPTOS_DEAD_SIGNAL = re.compile(r'let\s+_\s*=\s*(?:\w+_)?(?:nonce|signal|state)\.get\(\);')
RE_TENANT_FALLBACK = re.compile(r'(?:tenant_id|tenant)\.unwrap_or_default\(\)')
RE_TODO_MARKER = re.compile(r'\b(?:todo!\(|FIXME\b|TODO\b)')
RE_OUTBOX_DETACHED = re.compile(r'(?:event_bus|event_publisher)\.publish\s*\(')
RE_ASYNC_FN = re.compile(r'^\s*(?:pub\s+)?async\s+fn\s+([a-zA-Z_]\w*)')
RE_SYNC_LOCK_ACQUIRE = re.compile(
    r'\blet\s+(?:mut\s+)?([a-zA-Z_]\w*)\s*=\s*([^;\n]+?\.(?:lock|read|write)\s*\([^;\n]*\))'
)
RE_AWAIT = re.compile(r'\.await\b')
RE_EARLY_CLONE = re.compile(
    r'\b(?:let\s+(?:mut\s+)?\w+\s*=|if\s+let\s+Some\(\w+\)\s*=)\s*input\.\w+\.(?:clone\(\)|to_string\(\))'
)
RE_MUTATION_FN_NAME = re.compile(r'^(?:create|update|delete|set|reconcile|merge|publish|execute|handle|dispatch)')


def is_test_or_bench(file_path: Path) -> bool:
    path_str = file_path.as_posix().lower()
    return (
        "/tests/" in path_str
        or "/benches/" in path_str
        or path_str.endswith("_test.rs")
        or path_str.endswith("_tests.rs")
        or path_str.endswith("test.rs")
        or path_str.endswith("tests.rs")
        or "/test_helpers/" in path_str
        or "/test_support/" in path_str
        or "/testing/" in path_str
        or "/fixtures/" in path_str
        or "/mocks/" in path_str
        or path_str.endswith("mock.rs")
        or path_str.endswith("stub.rs")
    )


def is_build_or_example(file_path: Path) -> bool:
    path_str = file_path.as_posix().lower()
    return file_path.name == "build.rs" or "/examples/" in path_str


def is_migration(file_path: Path) -> bool:
    path_str = file_path.as_posix().lower()
    return "/migrations/" in path_str or "/migration/" in path_str


def extract_module_name(file_path: Path) -> str:
    parts = file_path.parts
    for i, part in enumerate(parts):
        if part in ("modules", "libs", "workers", "apps", "ui", "tools") and i + 1 < len(parts):
            return parts[i + 1]
    return "root"


def strip_string_literals(line: str) -> str:
    """Strip string literals to avoid counting braces inside strings."""
    # Strip escaped characters
    s = re.sub(r'\\.', '', line)
    # Strip double quoted strings
    s = re.sub(r'"[^"]*"', '""', s)
    return s


def scan_file(file_path: Path) -> list[Finding]:
    findings: list[Finding] = []
    try:
        rel_path = file_path.relative_to(ROOT)
    except ValueError:
        rel_path = file_path

    try:
        content = file_path.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        print(f"[WARN] Failed to read {rel_path}: {e}", file=sys.stderr)
        return []

    lines = content.splitlines()
    in_test_module_brace_depth: int | None = None
    in_loop_brace_depth: int | None = None
    in_async_fn_brace_depth: int | None = None
    active_sync_guards: dict[str, tuple[int, str, int]] = {}  # guard_name -> (line_num, line_str, brace_depth)
    in_block_comment = False
    current_brace_depth = 0
    file_has_txn_scope = "txn" in content or "DatabaseTransaction" in content

    is_test_file = is_test_or_bench(file_path)
    is_build_file = is_build_or_example(file_path)
    is_migr_file = is_migration(file_path)
    is_services_file = "/services/" in file_path.as_posix() or file_path.name.endswith("_service.rs")

    for idx, line in enumerate(lines, start=1):
        stripped = line.strip()

        # Handle multi-line block comments /* ... */
        if in_block_comment:
            if "*/" in stripped:
                in_block_comment = False
                stripped = stripped.split("*/", 1)[1].strip()
            else:
                continue

        if "/*" in stripped:
            if "*/" in stripped:
                stripped = re.sub(r'/\*.*?\*/', '', stripped).strip()
            else:
                in_block_comment = True
                stripped = stripped.split("/*", 1)[0].strip()

        if not stripped or stripped.startswith("//"):
            continue

        clean_for_braces = strip_string_literals(stripped)
        open_braces = clean_for_braces.count("{")
        close_braces = clean_for_braces.count("}")

        if "#[cfg(test)]" in stripped or re.search(r'\bmod\s+tests?\b', stripped):
            in_test_module_brace_depth = current_brace_depth

        if RE_FOR_OR_WHILE_LOOP.search(stripped) and "{" in stripped:
            in_loop_brace_depth = current_brace_depth

        is_test_context = (
            is_test_file
            or (in_test_module_brace_depth is not None and current_brace_depth >= in_test_module_brace_depth)
        )

        # Track async function entry
        m_async_fn = RE_ASYNC_FN.match(stripped)
        if m_async_fn and "{" in stripped:
            in_async_fn_brace_depth = current_brace_depth
            fn_name = m_async_fn.group(1)

            # 17. P-STRUCTURED-TRACE: uninstrumented domain service mutation entrypoint
            if is_services_file and not is_test_context:
                if RE_MUTATION_FN_NAME.match(fn_name):
                    # Check preceding 8 lines for tracing::instrument
                    has_instrument = any(
                        "instrument" in lines[j]
                        for j in range(max(0, idx - 9), idx - 1)
                    )
                    if not has_instrument:
                        findings.append(Finding(
                            pattern_id="P-STRUCTURED-TRACE",
                            rule_name="Contextual Trace Spans",
                            severity=Severity.P2_MEDIUM,
                            file_path=rel_path,
                            line_number=idx,
                            line_content=stripped,
                            message=f"Public mutation service '{fn_name}' lacks #[tracing::instrument]. Missing contextual trace span.",
                            suggested_fix=f"#[tracing::instrument(skip(self, security), fields(tenant_id = %tenant_id))]"
                        ))

        # Track synchronous lock acquisition in async functions
        if in_async_fn_brace_depth is not None and not is_test_context:
            m_lock = RE_SYNC_LOCK_ACQUIRE.search(stripped)
            if m_lock and ".await" not in stripped and "tokio::sync" not in stripped:
                guard_name = m_lock.group(1)
                active_sync_guards[guard_name] = (idx, stripped, current_brace_depth)

            # Drop explicitly
            m_drop = re.search(r'\bdrop\s*\(\s*([a-zA-Z_]\w*)\s*\)', stripped)
            if m_drop:
                dropped_var = m_drop.group(1)
                active_sync_guards.pop(dropped_var, None)

            # 15. P-SYNC-LOCK-AWAIT: synchronous lock guard held across .await point
            if RE_AWAIT.search(stripped) and active_sync_guards:
                # Check if any guard is still in scope at current brace depth
                in_scope_guards = [
                    (g_name, g_info)
                    for g_name, g_info in active_sync_guards.items()
                    if current_brace_depth >= g_info[2]
                ]
                for g_name, g_info in in_scope_guards:
                    findings.append(Finding(
                        pattern_id="P-SYNC-LOCK-AWAIT",
                        rule_name="Sync Lock Held Across Await",
                        severity=Severity.P0_CRITICAL,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=stripped,
                        message=f"Holding synchronous lock guard '{g_name}' (acquired at line {g_info[0]}) across an .await point. This blocks executor threads and can cause deadlocks.",
                        suggested_fix=f"Scope '{g_name}' inside a synchronous block or migrate lock to tokio::sync::Mutex."
                    ))
                active_sync_guards.clear()

        # 1. P-ZERO-INCLUDE: file-stitching macro
        m_inc = RE_INCLUDE.search(stripped)
        if m_inc and "OUT_DIR" not in stripped:
            included_file = m_inc.group(1)
            mod_name = Path(included_file).stem
            findings.append(Finding(
                pattern_id="P-ZERO-INCLUDE",
                rule_name="Zero Include Slop",
                severity=Severity.P1_HIGH,
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message=f"Forbidden file-stitching macro '{m_inc.group(0)}'. Use canonical 'mod ...;' and public exports.",
                suggested_fix=f"mod {mod_name}; pub use {mod_name}::*;"
            ))

        # 2 & 3. P-ANTI-SWALLOW & P-TXN-ROLLBACK-SWALLOW: swallowed errors
        if not is_test_context:
            if RE_TXN_ROLLBACK_COMMIT.search(stripped):
                findings.append(Finding(
                    pattern_id="P-TXN-ROLLBACK-SWALLOW",
                    rule_name="Transaction Rollback Swallowing",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Swallowing transaction rollback or commit via 'let _ = ... .rollback().await'. Swallowed failures keep locks (pg_advisory_xact_lock) or uncommitted state held until drop.",
                    suggested_fix="if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, \"transaction rollback failed\"); }"
                ))
            elif RE_LET_UNDERSCORE.match(stripped):
                if RE_LEPTOS_DEAD_SIGNAL.search(stripped):
                    findings.append(Finding(
                        pattern_id="P-LEPTOS-DEAD-SIGNAL",
                        rule_name="Dead Reactive Signal Read",
                        severity=Severity.P1_HIGH,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=stripped,
                        message="Calling signal.get() in let _ = outside of create_effect / view! drops reactivity. UI will not update.",
                        suggested_fix="create_effect(move |_| { signal.get(); /* trigger reactive update */ });"
                    ))
                else:
                    findings.append(Finding(
                        pattern_id="P-ANTI-SWALLOW",
                        rule_name="Zero Error Swallowing",
                        severity=Severity.P1_HIGH,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=stripped,
                        message="Swallowed error or variable via 'let _ ='. Propagate typed Result or log with warn!(%err).",
                        suggested_fix="if let Err(err) = ... { tracing::warn!(%err, \"operation failed\"); }"
                    ))

            if RE_BLACK_BOX_SUPPRESS.search(stripped):
                findings.append(Finding(
                    pattern_id="P-ANTI-SWALLOW",
                    rule_name="Dummy Compiler Suppression",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Abuse of 'black_box' in production service code. Do not fake variable usage to silence linter.",
                    suggested_fix="Remove black_box and handle or return the underlying value."
                ))

        # 4. P-ZERO-PANIC-PROD: expect() or unwrap() in production paths (excluding tests and build.rs)
        if not is_test_context and not is_build_file:
            m_panic = RE_EXPECT_UNWRAP.search(stripped)
            if m_panic and "static" not in file_path.name:
                findings.append(Finding(
                    pattern_id="P-ZERO-PANIC-PROD",
                    rule_name="Zero Production Panics",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Calling .expect() or .unwrap() in production path. Return typed DomainError / PortError.",
                    suggested_fix="Use '?' operator with .map_err(DomainError::from) or return Result."
                ))

            if RE_UNWRAP_ERR.search(stripped):
                findings.append(Finding(
                    pattern_id="P-ZERO-PANIC-PROD",
                    rule_name="Zero Production Panics",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Calling .unwrap_err() in production path. Match on Result explicitly.",
                    suggested_fix="match result { Err(e) => ..., Ok(_) => return Err(...) }"
                ))

        # 5. P-ID-ENTROPY: obsolete 48-bit short_id
        if RE_SHORT_ID_12.search(stripped):
            findings.append(Finding(
                pattern_id="P-ID-ENTROPY",
                rule_name="Bounded Identifier Entropy",
                severity=Severity.P2_MEDIUM,
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="Deprecated 12-char identifier (48-bit entropy). Standardize on 16-hex characters (VARCHAR(16)).",
                suggested_fix="VARCHAR(16), [..16], and 64-bit entropy with deterministic collision fallback."
            ))

        # 6. P-FAIL-CLOSED: RBAC PermissionScope::None => Ok leakage
        if RE_RBAC_LEAK.search(stripped):
            findings.append(Finding(
                pattern_id="P-FAIL-CLOSED",
                rule_name="Fail-Closed Security Floor",
                severity=Severity.P0_CRITICAL,
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="PermissionScope::None must resolve to Err(Forbidden), never Ok(()) or true.",
                suggested_fix="PermissionScope::None => Err(DomainError::Forbidden)"
            ))

        # 7. P-SQLSTATE-DISCRIM: brittle string matching in error.rs or services
        if not is_test_context and ("error" in file_path.name or "service" in file_path.name) and RE_ERR_CONTAINS.search(stripped):
            findings.append(Finding(
                pattern_id="P-SQLSTATE-DISCRIM",
                rule_name="SQLSTATE Error Mapping",
                severity=Severity.P1_HIGH,
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="Brittle string matching on error message. Discriminate using SQLSTATE (23505, P0001).",
                suggested_fix="Match on db_err.sql_state() == Some(\"23505\") or \"P0001\"."
            ))

        # 8. P-BATCH-QUERY: N+1 queries in loops (.one(txn) / .all(txn) inside loops)
        if in_loop_brace_depth is not None and not is_test_context:
            if RE_QUERY_IN_TXN.search(stripped):
                findings.append(Finding(
                    pattern_id="P-BATCH-QUERY",
                    rule_name="Transactional Batching",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Suspected N+1 query: database query inside loop body. Batch with Column::Id.is_in(ids).all().",
                    suggested_fix="Collect IDs and perform one Entity::find().filter(Column::Id.is_in(ids)).all(txn) before loop."
                ))

        # 9. P-DEAD-TWIN: module-level #[allow(dead_code)] slop
        if idx <= 15 and RE_ALLOW_DEAD_CODE.match(stripped) and not is_test_context:
            findings.append(Finding(
                pattern_id="P-DEAD-TWIN",
                rule_name="File-Level Slop Suppression",
                severity=Severity.P2_MEDIUM,
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="Module-level #[allow(dead_code|unused)]. AGENTS.md §14 forbids broad suppression of dead code.",
                suggested_fix="Delete unused code completely or implement canonical consumer."
            ))

        # 10. P-RAW-SQL-INTERP: execute_unprepared with format! string interpolation
        if not is_test_context and not is_migr_file:
            if RE_RAW_SQL_INTERP.search(stripped):
                findings.append(Finding(
                    pattern_id="P-RAW-SQL-INTERP",
                    rule_name="Unprepared SQL Interpolation",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Calling execute_unprepared(&format!(...)) in service code. Use Statement::from_sql_and_values.",
                    suggested_fix="Statement::from_sql_and_values(db.get_database_backend(), sql, values)"
                ))

        # 11. P-TENANT-FALLBACK-LEAK: unwrap_or_default on tenant_id
        if not is_test_context:
            if RE_TENANT_FALLBACK.search(stripped):
                findings.append(Finding(
                    pattern_id="P-TENANT-FALLBACK-LEAK",
                    rule_name="Tenant Fallback Leakage",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Calling unwrap_or_default() on tenant context. Tenant isolation must fail closed.",
                    suggested_fix="tenant_id.ok_or(DomainError::TenantNotFound)?"
                ))

        # 12. P-UNFINISHED-CODE: lingering todo!(), FIXME, or TODO markers
        if not is_test_context and not is_build_file:
            m_todo = RE_TODO_MARKER.search(stripped)
            if m_todo:
                findings.append(Finding(
                    pattern_id="P-UNFINISHED-CODE",
                    rule_name="Lingering Incomplete Marker",
                    severity=Severity.P2_MEDIUM,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message=f"Found unfinished work marker '{m_todo.group(0)}'. Resolve before shipping or record in ledger.",
                    suggested_fix="Implement missing logic or record in continuous-review-ledger.md."
                ))

        # 13. P-OUTBOX-DETACHED: event_bus.publish() in transactional contexts
        if not is_test_context and file_has_txn_scope:
            if RE_OUTBOX_DETACHED.search(stripped):
                findings.append(Finding(
                    pattern_id="P-OUTBOX-DETACHED",
                    rule_name="Transactional Outbox Decoupling",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Calling event_bus.publish() in a transaction-aware file. Use event_bus.publish_in_tx(&txn, ...).",
                    suggested_fix="event_bus.publish_in_tx(&txn, event).await? to guarantee atomic delivery."
                ))

        # 16. P-HOTPATH-ALLOC: premature clone of input before validation gates
        if is_services_file and not is_test_context and RE_EARLY_CLONE.search(stripped):
            # Check if next 5 lines validate the cloned variable
            for next_idx in range(idx, min(idx + 5, len(lines))):
                next_line = lines[next_idx]
                if "validate_" in next_line or "check_" in next_line:
                    findings.append(Finding(
                        pattern_id="P-HOTPATH-ALLOC",
                        rule_name="Premature Input Allocation",
                        severity=Severity.P2_MEDIUM,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=stripped,
                        message="Cloning input field immediately before a validation check. Validate borrowed reference first.",
                        suggested_fix="Validate with &input.field or .as_deref() first, clone only after validation passes."
                    ))
                    break

        current_brace_depth += (open_braces - close_braces)
        if current_brace_depth < 0:
            current_brace_depth = 0

        # Clean up scopes
        if in_test_module_brace_depth is not None and current_brace_depth <= in_test_module_brace_depth:
            in_test_module_brace_depth = None

        if in_loop_brace_depth is not None and current_brace_depth <= in_loop_brace_depth:
            in_loop_brace_depth = None

        if in_async_fn_brace_depth is not None and current_brace_depth <= in_async_fn_brace_depth:
            in_async_fn_brace_depth = None
            active_sync_guards.clear()

    return findings


def discover_rust_files(target_dir: Path) -> Iterator[Path]:
    for root, dirs, files in os.walk(target_dir):
        dirs[:] = [d for d in dirs if d not in ("target", "node_modules", ".git")]
        for f in files:
            if f.endswith(".rs"):
                yield Path(root) / f


def resolve_crate_paths(crate_arg: str) -> list[Path]:
    """Smart fuzzy resolution for crate names across all workspace folders."""
    search_dirs = [
        ROOT / "crates" / "modules",
        ROOT / "crates" / "libs",
        ROOT / "crates" / "ui",
        ROOT / "crates" / "workers",
        ROOT / "crates" / "tools",
        ROOT / "apps",
    ]
    resolved: list[Path] = []
    tokens = [t.strip() for t in crate_arg.split(",") if t.strip()]

    for token in tokens:
        matches: list[Path] = []
        for base in search_dirs:
            if not base.exists():
                continue
            cand = base / token
            if cand.is_dir():
                matches.append(cand)
            if not token.startswith("rustok-"):
                cand_pre = base / f"rustok-{token}"
                if cand_pre.is_dir():
                    matches.append(cand_pre)

        # Substring match if no exact match found
        if not matches:
            for base in search_dirs:
                if not base.exists():
                    continue
                for d in base.iterdir():
                    if d.is_dir() and token in d.name:
                        matches.append(d)

        if matches:
            resolved.append(matches[0])
        else:
            print(f"[WARN] Could not find directory matching crate '{token}'", file=sys.stderr)

    return resolved


def discover_git_files(mode: str, base_ref: str | None = None) -> list[Path]:
    """Discover modified Rust files from git index, working tree, or branch diff."""
    files: set[Path] = set()
    try:
        if mode == "staged":
            cmd = ["git", "diff", "--cached", "--name-only", "--diff-filter=ACMR"]
            out = subprocess.check_output(cmd, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out.splitlines():
                if line.endswith(".rs"):
                    p = ROOT / line
                    if p.is_file():
                        files.add(p)
        elif mode == "changed":
            # Staged + unstaged modifications
            cmd = ["git", "diff", "--name-only", "HEAD", "--diff-filter=ACMR"]
            out = subprocess.check_output(cmd, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out.splitlines():
                if line.endswith(".rs"):
                    p = ROOT / line
                    if p.is_file():
                        files.add(p)
            # Untracked files
            cmd_status = ["git", "status", "--porcelain"]
            out_status = subprocess.check_output(cmd_status, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out_status.splitlines():
                if line.startswith("?? ") and line.endswith(".rs"):
                    p = ROOT / line[3:].strip()
                    if p.is_file():
                        files.add(p)
        elif mode == "diff" and base_ref:
            cmd = ["git", "diff", "--name-only", base_ref, "--diff-filter=ACMR"]
            out = subprocess.check_output(cmd, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out.splitlines():
                if line.endswith(".rs"):
                    p = ROOT / line
                    if p.is_file():
                        files.add(p)
    except Exception as e:
        print(f"[WARN] Failed to query git status: {e}", file=sys.stderr)

    return sorted(files)


def print_scorecard(findings: list[Finding]) -> None:
    by_module: dict[str, dict[str, int]] = defaultdict(lambda: {"P0": 0, "P1": 0, "P2": 0, "total": 0})
    for f in findings:
        mod_name = extract_module_name(f.file_path)
        sev_key = f.severity.value[:2]  # "P0", "P1", "P2"
        by_module[mod_name][sev_key] += 1
        by_module[mod_name]["total"] += 1

    print("\n" + "=" * 80)
    print(" 📊 PLATFORM HYGIENE SCORECARD (BY MODULE)")
    print("=" * 80)
    header = f"{'Module / Component':<34} | {'P0 Crit':<8} | {'P1 High':<8} | {'P2 Med':<8} | {'Total':<6} | {'Status'}"
    print(header)
    print("-" * 80)

    for mod, stats in sorted(by_module.items(), key=lambda x: (x[1]["P0"], x[1]["P1"], x[1]["total"]), reverse=True):
        if stats["P0"] > 0:
            status = "🔴 Action Required"
        elif stats["P1"] > 0:
            status = "🟡 Logic Warnings"
        else:
            status = "🟢 Tech Debt Only"
        row = f"{mod:<34} | {stats['P0']:<8} | {stats['P1']:<8} | {stats['P2']:<8} | {stats['total']:<6} | {status}"
        print(row)
    print("=" * 80 + "\n")


def generate_markdown_report(findings: list[Finding], scanned_files: int, target_desc: str) -> str:
    """Generate a clean GitHub Flavored Markdown report suitable for PR descriptions or audit evidence."""
    by_sev: dict[str, int] = {"P0-CRITICAL": 0, "P1-HIGH": 0, "P2-MEDIUM": 0}
    by_pattern: dict[str, list[Finding]] = defaultdict(list)
    by_module: dict[str, dict[str, int]] = defaultdict(lambda: {"P0": 0, "P1": 0, "P2": 0, "total": 0})

    for f in findings:
        by_sev[f.severity.value] += 1
        by_pattern[f.pattern_id].append(f)
        mod_name = extract_module_name(f.file_path)
        by_module[mod_name][f.severity.value[:2]] += 1
        by_module[mod_name]["total"] += 1

    lines = [
        "# RusToK Domain Hygiene & Invariant Verification Report",
        "",
        f"- **Target:** `{target_desc}`",
        f"- **Files Scanned:** {scanned_files}",
        f"- **Total Issues:** {len(findings)} (🔴 Critical: {by_sev['P0-CRITICAL']}, 🟡 High: {by_sev['P1-HIGH']}, 🟢 Medium: {by_sev['P2-MEDIUM']})",
        "",
        "## Module Scorecard",
        "",
        "| Module | P0 Critical | P1 High | P2 Medium | Total | Status |",
        "|---|---|---|---|---|---|",
    ]

    for mod, stats in sorted(by_module.items(), key=lambda x: (x[1]["P0"], x[1]["P1"], x[1]["total"]), reverse=True):
        status = "🔴 Action Required" if stats["P0"] > 0 else ("🟡 Logic Warnings" if stats["P1"] > 0 else "🟢 Clean / Minor")
        lines.append(f"| `{mod}` | {stats['P0']} | {stats['P1']} | {stats['P2']} | {stats['total']} | {status} |")

    lines.append("")
    lines.append("## Discovered Findings by Pattern")
    lines.append("")

    for pid, f_list in sorted(by_pattern.items()):
        rule_name = f_list[0].rule_name
        sev = f_list[0].severity.value
        lines.append(f"### `[{sev}]` {pid}: {rule_name} ({len(f_list)} occurrence(s))")
        lines.append("")
        lines.append("| File | Line | Snippet | Recommendation |")
        lines.append("|---|---|---|---|")
        for item in f_list[:15]:
            snippet = item.line_content.replace("|", "\\|").strip()
            fix = item.suggested_fix or item.message
            fix_escaped = fix.replace("|", "\\|")
            file_link = f"`{item.file_path.as_posix()}:{item.line_number}`"
            lines.append(f"| {file_link} | {item.line_number} | `{snippet[:60]}` | {fix_escaped} |")
        if len(f_list) > 15:
            lines.append(f"| *... and {len(f_list) - 15} more occurrences* | | | |")
        lines.append("")

    lines.append("---")
    lines.append("Canonical Standards Reference: [`docs/standards/logic-hygiene-patterns.md`](docs/standards/logic-hygiene-patterns.md)")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="RusToK Domain Logic, Invariant Integrity & Anti-Slop Scanner."
    )
    parser.add_argument(
        "--crate",
        type=str,
        help="Target crate name or comma-separated names (e.g. commerce, forum, server, rustok-index)",
    )
    parser.add_argument(
        "--path",
        type=str,
        help="Target directory or file path relative to repository root",
    )
    parser.add_argument(
        "--all-modules",
        action="store_true",
        help="Scan all modules in crates/modules/",
    )
    parser.add_argument(
        "--staged",
        action="store_true",
        help="Scan only git staged .rs files (ideal for pre-commit hooks)",
    )
    parser.add_argument(
        "--changed",
        action="store_true",
        help="Scan all modified and untracked .rs files in working tree",
    )
    parser.add_argument(
        "--diff",
        type=str,
        metavar="REF",
        help="Scan all .rs files modified relative to a git ref (e.g. main)",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit with code 1 if any finding is discovered (useful for CI/pre-commit)",
    )
    parser.add_argument(
        "--severity",
        choices=["P0", "P1", "P2"],
        help="Filter findings by minimum severity (P0 = critical only, P1 = high+, P2 = all)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output findings as JSON for CI or bot integration",
    )
    parser.add_argument(
        "--suggest-fixes",
        action="store_true",
        help="Display copy-pasteable remediation suggestions for flagged occurrences",
    )
    parser.add_argument(
        "--markdown-report",
        type=str,
        metavar="FILE",
        help="Generate a GitHub-flavored markdown report artifact file",
    )
    args = parser.parse_args()

    files_to_scan: list[Path] = []
    target_description = ""

    if args.staged:
        files_to_scan = discover_git_files("staged")
        target_description = "git staged files"
    elif args.changed:
        files_to_scan = discover_git_files("changed")
        target_description = "working tree modified files"
    elif args.diff:
        files_to_scan = discover_git_files("diff", args.diff)
        target_description = f"git diff against {args.diff}"
    elif args.crate:
        paths = resolve_crate_paths(args.crate)
        if not paths:
            print(f"[ERROR] Could not resolve any paths for --crate '{args.crate}'", file=sys.stderr)
            return 2
        for p in paths:
            files_to_scan.extend(discover_rust_files(p))
        target_description = f"crates: {args.crate}"
    elif args.path:
        p = ROOT / args.path
        if not p.exists():
            print(f"[ERROR] Path does not exist: {p}", file=sys.stderr)
            return 2
        if p.is_file() and p.name.endswith(".rs"):
            files_to_scan.append(p)
        elif p.is_dir():
            files_to_scan.extend(discover_rust_files(p))
        target_description = f"path: {args.path}"
    elif args.all_modules:
        target_dir = ROOT / "crates" / "modules"
        files_to_scan.extend(discover_rust_files(target_dir))
        target_description = "crates/modules (all)"
    else:
        cwd = Path.cwd()
        if cwd.resolve() == ROOT.resolve():
            target_dir = ROOT / "crates" / "modules"
            files_to_scan.extend(discover_rust_files(target_dir))
            target_description = "crates/modules (default)"
        else:
            files_to_scan.extend(discover_rust_files(cwd))
            target_description = f"directory: {cwd.relative_to(ROOT)}"

    all_findings: list[Finding] = []
    scanned_file_count = len(files_to_scan)

    if not args.json:
        print("======================================================================")
        print(" 🛡️  RusToK Domain Hygiene & Anti-Slop Scanner")
        print(" Canonical Reference: docs/standards/logic-hygiene-patterns.md")
        print(f" Target: {target_description}")
        print("======================================================================")

    for rs_file in files_to_scan:
        findings = scan_file(rs_file)
        all_findings.extend(findings)

    # Filter by severity if requested
    if args.severity == "P0":
        all_findings = [f for f in all_findings if f.severity == Severity.P0_CRITICAL]
    elif args.severity == "P1":
        all_findings = [f for f in all_findings if f.severity in (Severity.P0_CRITICAL, Severity.P1_HIGH)]

    if args.json:
        out_data = {
            "target": target_description,
            "scanned_files": scanned_file_count,
            "total_findings": len(all_findings),
            "findings": [
                {
                    "pattern_id": f.pattern_id,
                    "rule_name": f.rule_name,
                    "severity": f.severity.value,
                    "file": str(f.file_path).replace("\\", "/"),
                    "line": f.line_number,
                    "content": f.line_content,
                    "message": f.message,
                    "suggested_fix": f.suggested_fix,
                }
                for f in all_findings
            ],
        }
        print(json.dumps(out_data, indent=2))
        return 1 if (args.strict and all_findings) else 0

    print(f"Scanned {scanned_file_count} Rust file(s). Found {len(all_findings)} issue(s).\n")

    if not all_findings:
        print("✅ No domain logic defects, slop, or anti-patterns detected! Clean code.")
        return 0

    # Display high-level scorecard if multiple files/modules were scanned
    if scanned_file_count > 30 or args.all_modules:
        print_scorecard(all_findings)

    # Group findings by pattern ID
    by_pattern: dict[str, list[Finding]] = {}
    for f in all_findings:
        by_pattern.setdefault(f.pattern_id, []).append(f)

    for pid, findings in sorted(by_pattern.items()):
        sev = findings[0].severity.value
        print(f"=== [{sev}] {pid}: {findings[0].rule_name} ({len(findings)} occurrence(s)) ===")
        for item in findings[:10]:
            print(f"  {item.file_path}:{item.line_number}")
            print(f"    Line:    {item.line_content}")
            print(f"    Fix:     {item.message}")
            if args.suggest_fixes and item.suggested_fix:
                print(f"    Suggest: {item.suggested_fix}")
            print()
        if len(findings) > 10:
            print(f"  ... and {len(findings) - 10} more occurrences omitted.\n")

    # Optional Markdown report export
    if args.markdown_report:
        report_path = ROOT / args.markdown_report
        report_content = generate_markdown_report(all_findings, scanned_file_count, target_description)
        report_path.write_text(report_content, encoding="utf-8")
        print(f"📄 Markdown verification report saved to: {report_path.relative_to(ROOT)}")

    print("----------------------------------------------------------------------")
    print("📖 For remediation guides and canonical patterns, see:")
    print("   docs/standards/logic-hygiene-patterns.md")
    print("----------------------------------------------------------------------")

    if args.strict:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
