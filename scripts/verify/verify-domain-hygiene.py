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
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Iterator

# Ensure UTF-8 output and immediate line buffering on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace", line_buffering=True)
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace", line_buffering=True)

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
    r'\blet\s+_\s*=\s*(?:\w+\.)?(?:transaction|txn|tx|lock)\s*\.\s*(?:rollback|commit)\s*\('
)
RE_BLACK_BOX_SUPPRESS = re.compile(r'\b(?:std::hint::)?black_box\s*\(')
RE_EXPECT_UNWRAP = re.compile(r'\.(?:expect|unwrap)\s*\(')
RE_UNWRAP_ERR = re.compile(r'\.unwrap_err\s*\(')
RE_EXPLICIT_PANIC = re.compile(r'\bpanic!\s*\(')
RE_STATIC_CONST_DECL = re.compile(r'^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:static|const)\b')
RE_SHORT_ID_12 = re.compile(
    r'(?:VARCHAR\s*\(\s*12\s*\)|\[\s*0?\s*\.\.\s*12\s*\]|\"000000000000\"|left\s*\([^,]+,\s*12\s*\)|SHORT_ID_LEN(?:GTH)?\s*(?::\s*usize)?\s*=\s*12\b)'
)
RE_RBAC_LEAK = re.compile(r'PermissionScope::None\s*=>\s*(?:\{?\s*(?:return\s+)?)?(?:Ok\b|true\b)')
RE_ERR_VAR_CONTAINS = re.compile(
    r'\b(?:err|error|db_err|sql_err|query_err)\s*(?:\.\s*to_string\s*\(\s*\))?\s*\.\s*contains\s*\(\s*"([^"]+)"'
)
RE_GENERIC_CONTAINS = re.compile(
    r'\b(?:message|msg|e)\s*(?:\.\s*to_string\s*\(\s*\))?\s*\.\s*contains\s*\(\s*"([^"]+)"'
)
RE_SQL_ERROR_HINTS = re.compile(
    r'(?:duplicate|unique|violates|constraint|foreign\s*key|not-null|23505|23503|23502|23514|42P01|P0001|deadlock|syntax\s*error|relation\s*"\w+"\s*does\s*not\s*exist)',
    re.IGNORECASE,
)
RE_QUERY_IN_TXN = re.compile(
    r'\.(?:one|all|one_by_id|query_one|query_all)\s*\(\s*&?(?:(?:\w+\.)?\w*(?:txn|tx|transaction|db|conn|pool)\w*)\s*\)\.await'
)
RE_FOR_OR_WHILE_LOOP = re.compile(r'\b(?:for\s+[^;{]+?\s+in\b|while\b)')
RE_ALLOW_DEAD_CODE = re.compile(r'^\s*#!?\[allow\((?:dead_code|unused|unused_variables)\)\]')
RE_RAW_SQL_INTERP = re.compile(r'\.execute_unprepared\s*\(\s*&?format!\s*\(')
RE_LEPTOS_DEAD_SIGNAL = re.compile(r'let\s+_\s*=\s*(?:\w+_)?(?:nonce|signal|state)\.get\(\);')
RE_TENANT_FALLBACK = re.compile(
    r'\btenant(?:_id)?(?:\.[a-zA-Z_]\w*\(\))*\.unwrap_or(?:_default\(\)|_else\s*\(\s*(?:\|\|)?\s*(?:Uuid::nil\(\)?|Uuid::default\(\)?|Default::default\(\)?)\s*\)|\(\s*(?:Uuid::nil\(\)|Uuid::default\(\)|Default::default\(\))\s*\))'
)
RE_TODO_MARKER = re.compile(r'\b(?:to' + r'do!\(|FIXME\b|TODO\b)')
RE_OUTBOX_DETACHED = re.compile(r'(?:event_bus|event_publisher)\.publish\s*\(')
RE_ASYNC_FN = re.compile(r'^\s*(?:pub(?:\s*\([^)]*\))?\s+)?async\s+fn\s+([a-zA-Z_]\w*)')
RE_PUB_ASYNC_FN = re.compile(r'^\s*pub(?:\s*\([^)]*\))?\s+async\s+fn\s+([a-zA-Z_]\w*)')
RE_SYNC_LOCK_ACQUIRE = re.compile(
    r'\blet\s+(?:mut\s+)?([a-zA-Z_]\w*)\s*=\s*([^;\n]+?\.(?:lock|read|write)\s*\([^;\n]*\))'
)
RE_AWAIT = re.compile(r'\.await\b')
RE_EARLY_CLONE = re.compile(
    r'\b(?:let\s+(?:mut\s+)?\w+\s*=|if\s+let\s+Some\(\w+\)\s*=)\s*input\.\w+\.(?:clone\(\)|to_string\(\))'
)
RE_MUTATION_FN_NAME = re.compile(r'^(?:create|update|delete|set|reconcile|merge|publish|execute|handle|dispatch)')
RE_SYNC_SLEEP = re.compile(r'\bstd::thread::sleep\s*\(')


def is_test_or_bench(file_path: Path) -> bool:
    parts = [p.lower() for p in file_path.parts]
    file_name = file_path.name.lower()
    test_dirs = {"tests", "benches", "test_helpers", "test_support", "testing", "fixtures", "mocks"}
    return (
        bool(test_dirs.intersection(parts))
        or file_name.endswith("_test.rs")
        or file_name.endswith("_tests.rs")
        or file_name in ("test.rs", "tests.rs")
        or file_name.endswith("mock.rs")
        or file_name.endswith("stub.rs")
        or file_name.endswith("_evidence.rs")
    )


def is_build_or_example(file_path: Path) -> bool:
    parts = [p.lower() for p in file_path.parts]
    return file_path.name == "build.rs" or "examples" in parts


def is_migration(file_path: Path) -> bool:
    parts = [p.lower() for p in file_path.parts]
    return "migrations" in parts or "migration" in parts


def extract_module_name(file_path: Path) -> str:
    resolved_parts = file_path.resolve().parts
    for i, part in enumerate(resolved_parts):
        if part in ("modules", "libs", "workers", "apps", "ui", "tools") and i + 1 < len(resolved_parts):
            return resolved_parts[i + 1]
    return "root"


def strip_string_literals(line: str) -> str:
    """Strip character and string literals using strict backreference matching."""
    # 1. Byte raw strings br#"..."#
    s = re.sub(r'br(#*)".*?"\1', 'b""', line)
    # 2. Raw strings r#"..."#
    s = re.sub(r'r(#*)".*?"\1', '""', s)
    # 3. Byte strings b"..."
    s = re.sub(r'b"(?:\\.|[^"\\])*"', 'b""', s)
    # 4. Double quoted strings "..."
    s = re.sub(r'"(?:\\.|[^"\\])*"', '""', s)
    # 5. Byte char literals b'...'
    s = re.sub(r"b'(?:\\.|[^'\\])'", "b''", s)
    # 6. Char literals '...'
    s = re.sub(r"'(?:\\.|[^'\\])'", "''", s)
    return s


def split_code_and_comment(line: str) -> tuple[str, str]:
    """Split line into code and comment parts, ignoring // inside string/char literals."""
    in_quote: str | None = None  # '"' or "'"
    in_raw_hashes: int | None = None
    i = 0
    n = len(line)
    while i < n:
        c = line[i]
        # Check raw string start: r#*" or br#*"
        if in_quote is None and in_raw_hashes is None:
            idx = i
            if c == 'b' and i + 1 < n and line[i + 1] in ('r', '"', "'"):
                idx = i + 1
                c = line[idx]

            if c == 'r' and idx + 1 < n and (line[idx + 1] == '"' or line[idx + 1] == '#'):
                j = idx + 1
                while j < n and line[j] == '#':
                    j += 1
                if j < n and line[j] == '"':
                    in_raw_hashes = j - (idx + 1)
                    i = j + 1
                    continue
            elif c == '"':
                in_quote = '"'
                i = idx + 1
                continue
            elif c == "'":
                is_char_lit = False
                if idx + 2 < n and line[idx + 2] == "'":
                    is_char_lit = True
                elif idx + 3 < n and line[idx + 1] == '\\' and line[idx + 3] == "'":
                    is_char_lit = True

                if is_char_lit:
                    in_quote = "'"
                    i = idx + 1
                    continue
                else:
                    i = idx + 1
                    continue
            elif c == '/' and i + 1 < n and line[i + 1] == '/':
                return line[:i].strip(), line[i + 2:].strip()
            i += 1
            continue

        if in_raw_hashes is not None:
            if c == '"':
                hashes = 0
                j = i + 1
                while j < n and line[j] == '#' and hashes < in_raw_hashes:
                    hashes += 1
                    j += 1
                if hashes == in_raw_hashes:
                    in_raw_hashes = None
                    i = j
                    continue
            i += 1
            continue

        if in_quote == '"':
            if c == '\\':
                i += 2
                continue
            if c == '"':
                in_quote = None
            i += 1
            continue

        if in_quote == "'":
            if c == '\\':
                i += 2
                continue
            if c == "'":
                in_quote = None
            i += 1
            continue

    return line.strip(), ""


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
    in_test_fn_brace_depth: int | None = None
    pending_test_fn = False
    in_loop_brace_depth: int | None = None
    pending_loop = False
    in_async_fn_brace_depth: int | None = None
    pending_async_fn_name: str | None = None
    in_static_brace_depth: int | None = None
    pending_static = False
    active_sync_guards: dict[str, tuple[int, str, int]] = {}  # guard_name -> (line_num, line_str, brace_depth)
    in_block_comment = False
    current_brace_depth = 0
    file_has_txn_scope = "txn" in content or "DatabaseTransaction" in content

    is_test_file = is_test_or_bench(file_path)
    is_build_file = is_build_or_example(file_path)
    is_migr_file = is_migration(file_path)
    path_parts = [p.lower() for p in file_path.parts]
    is_services_file = (
        "services" in path_parts
        or "service" in path_parts
        or file_path.name in ("service.rs", "services.rs")
        or file_path.name.endswith("_service.rs")
    )
    in_raw_hashes: int | None = None
    in_multiline_str = False

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

        # Handle multi-line raw string literals r#" ... "#
        if in_raw_hashes is not None:
            close_pat = '"' + '#' * in_raw_hashes
            if close_pat in stripped:
                in_raw_hashes = None
                stripped = stripped.split(close_pat, 1)[1].strip()
            else:
                continue

        # Handle multi-line standard strings " ... "
        if in_multiline_str:
            m_close = re.search(r'^(?:\\.|[^"\\])*"', stripped)
            if m_close:
                in_multiline_str = False
                stripped = stripped[m_close.end():].strip()
            else:
                continue

        if not stripped:
            continue

        # Separate code portion from line-comment portion using quote-aware parser
        code_part, comment_part = split_code_and_comment(stripped)

        # 12. P-UNFINISHED-CODE: lingering todo!(), FIXME, or TODO markers (check both code and comments)
        if not is_test_file and not is_build_file:
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

        # Skip further inspection if the line is purely a comment
        if not code_part:
            continue

        clean_for_braces = strip_string_literals(code_part)

        # Check if line opened an unclosed raw string
        m_raw_open = re.search(r'\br(#*)"', clean_for_braces)
        if m_raw_open:
            in_raw_hashes = len(m_raw_open.group(1))
        elif clean_for_braces.count('"') % 2 == 1:
            in_multiline_str = True

        open_braces = clean_for_braces.count("{")
        close_braces = clean_for_braces.count("}")

        # Test attribute tracking (supporting multiline signatures)
        if "#[test]" in stripped or "#[tokio::test]" in stripped:
            pending_test_fn = True

        if pending_test_fn and "{" in clean_for_braces:
            in_test_fn_brace_depth = current_brace_depth
            pending_test_fn = False

        if "#[cfg(test)]" in stripped or re.search(r'\bmod\s+tests?\b', code_part):
            in_test_module_brace_depth = current_brace_depth

        # Loop tracking (supporting multiline headers)
        if RE_FOR_OR_WHILE_LOOP.search(code_part):
            if "{" in clean_for_braces:
                in_loop_brace_depth = current_brace_depth
            else:
                pending_loop = True
        elif pending_loop and "{" in clean_for_braces:
            in_loop_brace_depth = current_brace_depth
            pending_loop = False

        # Static / LazyLock block tracking
        if RE_STATIC_CONST_DECL.search(code_part) or "Lazy::new" in code_part or "LazyLock::new" in code_part or "lazy_static!" in code_part:
            if "{" in clean_for_braces:
                in_static_brace_depth = current_brace_depth
            else:
                pending_static = True
        elif pending_static and "{" in clean_for_braces:
            in_static_brace_depth = current_brace_depth
            pending_static = False

        is_test_context = (
            is_test_file
            or (in_test_module_brace_depth is not None and current_brace_depth >= in_test_module_brace_depth)
            or (in_test_fn_brace_depth is not None and current_brace_depth >= in_test_fn_brace_depth)
        )

        # Track async function entry (supporting multiline signatures)
        m_async_fn = RE_ASYNC_FN.match(code_part)
        if m_async_fn:
            fn_name = m_async_fn.group(1)
            if "{" in clean_for_braces:
                in_async_fn_brace_depth = current_brace_depth
            else:
                pending_async_fn_name = fn_name

            # 17. P-STRUCTURED-TRACE: uninstrumented public domain service mutation entrypoint
            is_pub_fn = RE_PUB_ASYNC_FN.match(code_part) is not None
            if is_services_file and not is_test_context and is_pub_fn and RE_MUTATION_FN_NAME.match(fn_name):
                has_instrument = any(
                    "instrument" in lines[j]
                    for j in range(max(0, idx - 16), idx - 1)
                )
                if not has_instrument:
                    findings.append(Finding(
                        pattern_id="P-STRUCTURED-TRACE",
                        rule_name="Contextual Trace Spans",
                        severity=Severity.P2_MEDIUM,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=code_part,
                        message=f"Public mutation service '{fn_name}' lacks #[tracing::instrument]. Missing contextual trace span.",
                        suggested_fix=f"#[tracing::instrument(skip(self, security), fields(tenant_id = %tenant_id))]"
                    ))
        elif pending_async_fn_name is not None and "{" in clean_for_braces:
            in_async_fn_brace_depth = current_brace_depth
            pending_async_fn_name = None

        # Track synchronous lock acquisition and sleep in async functions
        if in_async_fn_brace_depth is not None and not is_test_context:
            if RE_SYNC_SLEEP.search(code_part):
                findings.append(Finding(
                    pattern_id="P-SYNC-SLEEP-AWAIT",
                    rule_name="Sync Sleep in Async Code",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Calling std::thread::sleep in async code blocks executor threads. Use tokio::time::sleep(...).await.",
                    suggested_fix="tokio::time::sleep(duration).await;"
                ))

            m_lock = RE_SYNC_LOCK_ACQUIRE.search(code_part)
            if m_lock and ".await" not in code_part and "tokio::sync" not in code_part:
                guard_name = m_lock.group(1)
                active_sync_guards[guard_name] = (idx, code_part, current_brace_depth)

            # Drop explicitly
            m_drop = re.search(r'\bdrop\s*\(\s*([a-zA-Z_]\w*)\s*\)', code_part)
            if m_drop:
                dropped_var = m_drop.group(1)
                active_sync_guards.pop(dropped_var, None)

            # 15. P-SYNC-LOCK-AWAIT: synchronous lock guard held across .await point
            if RE_AWAIT.search(code_part) and active_sync_guards:
                in_scope_guards = [
                    (g_name, g_info)
                    for g_name, g_info in list(active_sync_guards.items())
                    if current_brace_depth >= g_info[2]
                ]
                for g_name, g_info in in_scope_guards:
                    findings.append(Finding(
                        pattern_id="P-SYNC-LOCK-AWAIT",
                        rule_name="Sync Lock Held Across Await",
                        severity=Severity.P0_CRITICAL,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=code_part,
                        message=f"Holding synchronous lock guard '{g_name}' (acquired at line {g_info[0]}) across an .await point. This blocks executor threads and can cause deadlocks.",
                        suggested_fix=f"Scope '{g_name}' inside a synchronous block or migrate lock to tokio::sync::Mutex."
                    ))
                    active_sync_guards.pop(g_name, None)

        # 1. P-ZERO-INCLUDE: file-stitching macro
        m_inc = RE_INCLUDE.search(code_part)
        if m_inc and "OUT_DIR" not in code_part:
            included_file = m_inc.group(1)
            mod_name = Path(included_file).stem
            findings.append(Finding(
                pattern_id="P-ZERO-INCLUDE",
                rule_name="Zero Include Slop",
                severity=Severity.P1_HIGH,
                file_path=rel_path,
                line_number=idx,
                line_content=code_part,
                message=f"Forbidden file-stitching macro '{m_inc.group(0)}'. Use canonical 'mod ...;' and public exports.",
                suggested_fix=f"mod {mod_name}; pub use {mod_name}::*;"
            ))

        # 2 & 3. P-ANTI-SWALLOW & P-TXN-ROLLBACK-SWALLOW: swallowed errors (supporting multiline expressions)
        if not is_test_context:
            if RE_LET_UNDERSCORE.match(code_part):
                # Look ahead for multiline expressions if semicolon is missing
                full_stmt = code_part
                if ";" not in code_part:
                    for k in range(idx, min(idx + 5, len(lines))):
                        sub, _ = split_code_and_comment(lines[k])
                        if sub.startswith("pub ") or sub.startswith("fn ") or sub.startswith("async "):
                            break
                        full_stmt += " " + sub
                        if ";" in sub:
                            break

                if RE_TXN_ROLLBACK_COMMIT.search(full_stmt):
                    findings.append(Finding(
                        pattern_id="P-TXN-ROLLBACK-SWALLOW",
                        rule_name="Transaction Rollback Swallowing",
                        severity=Severity.P0_CRITICAL,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=code_part,
                        message="Swallowing transaction rollback or commit via 'let _ = ... .rollback().await'. Swallowed failures keep locks (pg_advisory_xact_lock) or uncommitted state held until drop.",
                        suggested_fix="if let Err(rollback_err) = txn.rollback().await { tracing::warn!(%rollback_err, \"transaction rollback failed\"); }"
                    ))
                elif RE_LEPTOS_DEAD_SIGNAL.search(full_stmt):
                    # Check if inside create_effect or view! block
                    is_reactive_block = any(
                        "create_effect" in lines[j] or "create_memo" in lines[j] or "view!" in lines[j]
                        for j in range(max(0, idx - 8), idx)
                    )
                    if not is_reactive_block:
                        findings.append(Finding(
                            pattern_id="P-LEPTOS-DEAD-SIGNAL",
                            rule_name="Dead Reactive Signal Read",
                            severity=Severity.P1_HIGH,
                            file_path=rel_path,
                            line_number=idx,
                            line_content=code_part,
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
                        line_content=code_part,
                        message="Swallowed error or variable via 'let _ ='. Propagate typed Result or log with warn!(%err).",
                        suggested_fix="if let Err(err) = ... { tracing::warn!(%err, \"operation failed\"); }"
                    ))

            if RE_BLACK_BOX_SUPPRESS.search(code_part):
                findings.append(Finding(
                    pattern_id="P-ANTI-SWALLOW",
                    rule_name="Dummy Compiler Suppression",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Abuse of 'black_box' in production service code. Do not fake variable usage to silence linter.",
                    suggested_fix="Remove black_box and handle or return the underlying value."
                ))

        # 4. P-ZERO-PANIC-PROD: expect() or unwrap() in production paths (excluding tests, build.rs, and statics)
        if not is_test_context and not is_build_file:
            is_static_item = (
                (in_static_brace_depth is not None and current_brace_depth >= in_static_brace_depth)
                or RE_STATIC_CONST_DECL.match(code_part) is not None
                or "Lazy::new" in code_part
                or "LazyLock::new" in code_part
                or "lazy_static!" in code_part
            )
            m_panic = RE_EXPECT_UNWRAP.search(code_part)
            if m_panic and not is_static_item:
                if not ("Regex::new" in code_part and ("static" in code_part or "const" in code_part)):
                    findings.append(Finding(
                        pattern_id="P-ZERO-PANIC-PROD",
                        rule_name="Zero Production Panics",
                        severity=Severity.P1_HIGH,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=code_part,
                        message="Calling .expect() or .unwrap() in production path. Return typed DomainError / PortError.",
                        suggested_fix="Use '?' operator with .map_err(DomainError::from) or return Result."
                    ))

            if RE_UNWRAP_ERR.search(code_part):
                findings.append(Finding(
                    pattern_id="P-ZERO-PANIC-PROD",
                    rule_name="Zero Production Panics",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Calling .unwrap_err() in production path. Match on Result explicitly.",
                    suggested_fix="match result { Err(e) => ..., Ok(_) => return Err(...) }"
                ))

            if RE_EXPLICIT_PANIC.search(code_part):
                findings.append(Finding(
                    pattern_id="P-ZERO-PANIC-PROD",
                    rule_name="Zero Production Panics",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Calling panic!(...) in production path. Return typed DomainError / PortError.",
                    suggested_fix="return Err(DomainError::from(...));"
                ))

        # 5. P-ID-ENTROPY: obsolete 48-bit short_id
        if not is_test_context and RE_SHORT_ID_12.search(code_part):
            findings.append(Finding(
                pattern_id="P-ID-ENTROPY",
                rule_name="Bounded Identifier Entropy",
                severity=Severity.P2_MEDIUM,
                file_path=rel_path,
                line_number=idx,
                line_content=code_part,
                message="Deprecated 12-char identifier (48-bit entropy). Standardize on 16-hex characters (VARCHAR(16)).",
                suggested_fix="VARCHAR(16), [..16], and 64-bit entropy with deterministic collision fallback."
            ))

        # 6. P-FAIL-CLOSED: RBAC PermissionScope::None => Ok leakage
        if not is_test_context:
            full_rbac = code_part
            if "PermissionScope::None" in code_part and "=>" in code_part and ";" not in code_part and "}" not in code_part:
                for k in range(idx, min(idx + 3, len(lines))):
                    sub_code, _ = split_code_and_comment(lines[k])
                    full_rbac += " " + sub_code
                    if ";" in sub_code or "}" in sub_code or "," in sub_code:
                        break
            if RE_RBAC_LEAK.search(full_rbac):
                findings.append(Finding(
                    pattern_id="P-FAIL-CLOSED",
                    rule_name="Fail-Closed Security Floor",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="PermissionScope::None must resolve to Err(Forbidden), never Ok(()) or true.",
                    suggested_fix="PermissionScope::None => Err(DomainError::Forbidden)"
                ))

        # 7. P-SQLSTATE-DISCRIM: brittle string matching in error.rs or services
        if not is_test_context and ("error" in file_path.name or is_services_file):
            m_err_var = RE_ERR_VAR_CONTAINS.search(code_part)
            m_gen_var = RE_GENERIC_CONTAINS.search(code_part)
            is_sql_discrim = False
            if m_err_var:
                is_sql_discrim = True
            elif m_gen_var:
                matched_str = m_gen_var.group(1)
                if RE_SQL_ERROR_HINTS.search(matched_str):
                    is_sql_discrim = True

            if is_sql_discrim:
                findings.append(Finding(
                    pattern_id="P-SQLSTATE-DISCRIM",
                    rule_name="SQLSTATE Error Mapping",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Brittle string matching on error message. Discriminate using SQLSTATE (23505, P0001).",
                    suggested_fix="Match on db_err.sql_state() == Some(\"23505\") or \"P0001\"."
                ))

        # 8. P-BATCH-QUERY: N+1 queries in loops (.one(txn) / .all(txn) inside loops)
        if in_loop_brace_depth is not None and not is_test_context:
            if RE_QUERY_IN_TXN.search(code_part):
                findings.append(Finding(
                    pattern_id="P-BATCH-QUERY",
                    rule_name="Transactional Batching",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Suspected N+1 query: database query inside loop body. Batch with Column::Id.is_in(ids).all().",
                    suggested_fix="Collect IDs and perform one Entity::find().filter(Column::Id.is_in(ids)).all(txn) before loop."
                ))

        # 9. P-DEAD-TWIN: module-level #[allow(dead_code)] slop
        if idx <= 15 and RE_ALLOW_DEAD_CODE.match(code_part) and not is_test_context and not is_migr_file:
            findings.append(Finding(
                pattern_id="P-DEAD-TWIN",
                rule_name="File-Level Slop Suppression",
                severity=Severity.P2_MEDIUM,
                file_path=rel_path,
                line_number=idx,
                line_content=code_part,
                message="Module-level #[allow(dead_code|unused)]. AGENTS.md §14 forbids broad suppression of dead code.",
                suggested_fix="Delete unused code completely or implement canonical consumer."
            ))

        # 10. P-RAW-SQL-INTERP: execute_unprepared with format! string interpolation
        if not is_test_context and not is_migr_file:
            full_sql = code_part
            if "execute_unprepared" in code_part and ";" not in code_part:
                for k in range(idx, min(idx + 4, len(lines))):
                    sub_code, _ = split_code_and_comment(lines[k])
                    full_sql += " " + sub_code
                    if ";" in sub_code or ")" in sub_code:
                        break
            if RE_RAW_SQL_INTERP.search(full_sql):
                findings.append(Finding(
                    pattern_id="P-RAW-SQL-INTERP",
                    rule_name="Unprepared SQL Interpolation",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Calling execute_unprepared(&format!(...)) in service code. Use Statement::from_sql_and_values.",
                    suggested_fix="Statement::from_sql_and_values(db.get_database_backend(), sql, values)"
                ))

        # 11. P-TENANT-FALLBACK-LEAK: unwrap_or_default on tenant_id
        if not is_test_context:
            if RE_TENANT_FALLBACK.search(code_part):
                findings.append(Finding(
                    pattern_id="P-TENANT-FALLBACK-LEAK",
                    rule_name="Tenant Fallback Leakage",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Calling unwrap_or_default() on tenant context. Tenant isolation must fail closed.",
                    suggested_fix="tenant_id.ok_or(DomainError::TenantNotFound)?"
                ))

        # 13. P-OUTBOX-DETACHED: event_bus.publish() in transactional contexts
        if not is_test_context and file_has_txn_scope:
            if RE_OUTBOX_DETACHED.search(code_part):
                findings.append(Finding(
                    pattern_id="P-OUTBOX-DETACHED",
                    rule_name="Transactional Outbox Decoupling",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=code_part,
                    message="Calling event_bus.publish() in a transaction-aware file. Use event_bus.publish_in_tx(&txn, ...).",
                    suggested_fix="event_bus.publish_in_tx(&txn, event).await? to guarantee atomic delivery."
                ))

        # 16. P-HOTPATH-ALLOC: premature clone of input before validation gates
        if is_services_file and not is_test_context and RE_EARLY_CLONE.search(code_part):
            for next_idx in range(idx, min(idx + 5, len(lines))):
                next_code, _ = split_code_and_comment(lines[next_idx])
                if "validate_" in next_code or "check_" in next_code:
                    findings.append(Finding(
                        pattern_id="P-HOTPATH-ALLOC",
                        rule_name="Premature Input Allocation",
                        severity=Severity.P2_MEDIUM,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=code_part,
                        message="Cloning input field immediately before a validation check. Validate borrowed reference first.",
                        suggested_fix="Validate with &input.field or .as_deref() first, clone only after validation passes."
                    ))
                    break

        current_brace_depth += (open_braces - close_braces)
        if current_brace_depth < 0:
            current_brace_depth = 0

        # Lexical scope exit: drop any lock guards declared in inner blocks that have closed
        guards_to_drop = [
            g_name for g_name, g_info in active_sync_guards.items()
            if g_info[2] > current_brace_depth
        ]
        for g_name in guards_to_drop:
            active_sync_guards.pop(g_name, None)

        # Clean up scopes
        if in_test_module_brace_depth is not None and current_brace_depth <= in_test_module_brace_depth:
            in_test_module_brace_depth = None

        if in_test_fn_brace_depth is not None and current_brace_depth <= in_test_fn_brace_depth:
            in_test_fn_brace_depth = None

        if in_loop_brace_depth is not None and current_brace_depth <= in_loop_brace_depth:
            in_loop_brace_depth = None

        if in_async_fn_brace_depth is not None and current_brace_depth <= in_async_fn_brace_depth:
            in_async_fn_brace_depth = None
            active_sync_guards.clear()

        if in_static_brace_depth is not None and current_brace_depth <= in_static_brace_depth:
            in_static_brace_depth = None

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

    # Return deduplicated paths while preserving order
    return list(dict.fromkeys(resolved))


def discover_git_files(mode: str, base_ref: str | None = None) -> list[Path]:
    """Discover modified Rust files from git index, working tree, or branch diff."""
    files: set[Path] = set()
    git_base = ["git", "-c", "core.quotepath=false"]
    try:
        if mode == "staged":
            cmd = git_base + ["diff", "--cached", "--name-only", "--diff-filter=ACMR"]
            out = subprocess.check_output(cmd, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out.splitlines():
                clean_line = line.strip().strip('"')
                if clean_line.endswith(".rs"):
                    p = ROOT / clean_line
                    if p.is_file():
                        files.add(p)
        elif mode == "changed":
            cmd = git_base + ["diff", "--name-only", "HEAD", "--diff-filter=ACMR"]
            out = subprocess.check_output(cmd, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out.splitlines():
                clean_line = line.strip().strip('"')
                if clean_line.endswith(".rs"):
                    p = ROOT / clean_line
                    if p.is_file():
                        files.add(p)
            cmd_status = git_base + ["status", "--porcelain", "-uall"]
            out_status = subprocess.check_output(cmd_status, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out_status.splitlines():
                if line.startswith("?? "):
                    clean_line = line[3:].strip().strip('"')
                    p = ROOT / clean_line
                    if p.is_file() and p.name.endswith(".rs"):
                        files.add(p)
                    elif p.is_dir():
                        files.update(discover_rust_files(p))
        elif mode == "diff" and base_ref:
            cmd = git_base + ["diff", "--name-only", base_ref, "--diff-filter=ACMR"]
            out = subprocess.check_output(cmd, cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
            for line in out.splitlines():
                clean_line = line.strip().strip('"')
                if clean_line.endswith(".rs"):
                    p = ROOT / clean_line
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
            snippet = item.line_content.replace("|", "\\|").replace("`", "'").strip()
            fix = item.suggested_fix or item.message
            fix_escaped = fix.replace("|", "\\|").replace("`", "'")
            file_link = f"`{item.file_path.as_posix()}:{item.line_number}`"
            lines.append(f"| {file_link} | {item.line_number} | `{snippet[:60]}` | {fix_escaped} |")
        if len(f_list) > 15:
            lines.append(f"| *... and {len(f_list) - 15} more occurrences* | | | |")
        lines.append("")

    lines.append("---")
    lines.append("Canonical Standards Reference: [`docs/standards/logic-hygiene-patterns.md`](docs/standards/logic-hygiene-patterns.md)")
    return "\n".join(lines)


def generate_csv_report(findings: list[Finding]) -> str:
    """Generate CSV report for spreadsheet analysis and metric tracking."""
    import csv
    import io
    output = io.StringIO()
    writer = csv.writer(output)
    writer.writerow(["pattern_id", "rule_name", "severity", "file", "line", "snippet", "message", "suggested_fix"])
    for f in findings:
        writer.writerow([
            f.pattern_id,
            f.rule_name,
            f.severity.value,
            str(f.file_path).replace("\\", "/"),
            f.line_number,
            f.line_content.strip(),
            f.message,
            f.suggested_fix or "",
        ])
    return output.getvalue()


def generate_unified_patch(findings: list[Finding]) -> str:
    """Generate a unified diff (.patch) containing comments and proposed fixes."""
    patches: list[str] = []
    by_file: dict[Path, list[Finding]] = defaultdict(list)
    for f in findings:
        if f.suggested_fix:
            by_file[f.file_path].append(f)

    for file_path, f_list in sorted(by_file.items(), key=lambda x: str(x[0])):
        rel_str = str(file_path).replace("\\", "/")
        patches.append(f"--- a/{rel_str}")
        patches.append(f"+++ b/{rel_str}")
        for item in sorted(f_list, key=lambda x: x.line_number):
            patches.append(f"@@ -{item.line_number},1 +{item.line_number},1 @@")
            patches.append(f"-{item.line_content}")
            patches.append(f"+// REMEDIATION ({item.pattern_id}): {item.suggested_fix}")
    return "\n".join(patches) + "\n" if patches else ""


def print_github_annotations(findings: list[Finding]) -> None:
    """Emit GitHub Actions workflow commands for inline pull request annotations."""
    for f in findings:
        level = "error" if f.severity == Severity.P0_CRITICAL else ("warning" if f.severity == Severity.P1_HIGH else "notice")
        file_str = str(f.file_path).replace("\\", "/")
        msg = f.message.replace("\n", " ").replace("%", "%25").replace("\r", "")
        print(f"::{level} file={file_str},line={f.line_number},title={f.pattern_id}::{msg}")


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
        "--all",
        action="store_true",
        help="Scan the entire RusToK workspace (all crates, libs, workers, tools, and apps)",
    )
    parser.add_argument(
        "--summary-only",
        action="store_true",
        help="Display only the module scorecard table without detailed occurrence lists",
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
    parser.add_argument(
        "--csv",
        type=str,
        metavar="FILE",
        help="Export findings as CSV for spreadsheet analysis",
    )
    parser.add_argument(
        "--patch",
        type=str,
        metavar="FILE",
        help="Generate a unified diff (.patch) containing proposed remediations",
    )
    parser.add_argument(
        "--github-annotations",
        action="store_true",
        help="Emit GitHub Actions workflow commands (::error / ::warning) for inline PR annotations",
    )
    parser.add_argument(
        "--fail-on",
        choices=["P0", "P1", "P2"],
        help="Exit with code 1 if findings meet or exceed this severity (P0 = critical only, P1 = high or critical, P2 = any)",
    )
    parser.add_argument(
        "--ignore-rule",
        type=str,
        help="Comma-separated pattern IDs to ignore (e.g. P-STRUCTURED-TRACE,P-DEAD-TWIN)",
    )
    parser.add_argument(
        "--threads",
        type=int,
        default=os.cpu_count() or 4,
        help="Number of parallel worker threads for scanning (default: CPU cores)",
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
    elif args.all:
        for folder_name in ("crates", "apps"):
            target_dir = ROOT / folder_name
            if target_dir.exists():
                files_to_scan.extend(discover_rust_files(target_dir))
        target_description = "workspace (all crates & apps)"
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

    ignored_rules = set()
    if args.ignore_rule:
        ignored_rules = {r.strip() for r in args.ignore_rule.split(",") if r.strip()}

    if args.threads > 1 and len(files_to_scan) > 20:
        with ThreadPoolExecutor(max_workers=args.threads) as executor:
            for file_findings in executor.map(scan_file, files_to_scan):
                all_findings.extend(file_findings)
    else:
        for rs_file in files_to_scan:
            findings = scan_file(rs_file)
            all_findings.extend(findings)

    if ignored_rules:
        all_findings = [f for f in all_findings if f.pattern_id not in ignored_rules]

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

    # Display high-level scorecard if multiple files/modules were scanned or requested
    if args.summary_only or scanned_file_count > 30 or args.all_modules or args.all:
        print_scorecard(all_findings)

    if not args.summary_only:
        # Group findings by pattern ID
        by_pattern: dict[str, list[Finding]] = {}
        for f in all_findings:
            by_pattern.setdefault(f.pattern_id, []).append(f)

        for pid, findings in sorted(by_pattern.items()):
            sev = findings[0].severity.value
            print(f"=== [{sev}] {pid}: {findings[0].rule_name} ({len(findings)} occurrence(s)) ===")
            for item in findings[:10]:
                posix_file = str(item.file_path).replace("\\", "/")
                print(f"  {posix_file}:{item.line_number}")
                print(f"    Line:    {item.line_content}")
                print(f"    Fix:     {item.message}")
                if args.suggest_fixes and item.suggested_fix:
                    print(f"    Suggest: {item.suggested_fix}")
                print()
            if len(findings) > 10:
                print(f"  ... and {len(findings) - 10} more occurrences omitted.\n")

    # Optional CSV report export
    if args.csv:
        csv_path = ROOT / args.csv
        csv_content = generate_csv_report(all_findings)
        csv_path.write_text(csv_content, encoding="utf-8")
        try:
            rel_display = csv_path.relative_to(ROOT)
        except ValueError:
            rel_display = csv_path
        print(f"📊 CSV verification report saved to: {rel_display}")

    # Optional Markdown report export
    if args.markdown_report:
        report_path = ROOT / args.markdown_report
        report_content = generate_markdown_report(all_findings, scanned_file_count, target_description)
        report_path.write_text(report_content, encoding="utf-8")
        try:
            rel_display = report_path.relative_to(ROOT)
        except ValueError:
            rel_display = report_path
        print(f"📄 Markdown verification report saved to: {rel_display}")

    # Optional Patch export
    if args.patch:
        patch_path = ROOT / args.patch
        patch_content = generate_unified_patch(all_findings)
        patch_path.write_text(patch_content, encoding="utf-8")
        try:
            rel_display = patch_path.relative_to(ROOT)
        except ValueError:
            rel_display = patch_path
        print(f"🩹 Remediation patch saved to: {rel_display}")

    # GitHub Actions workflow annotations
    if args.github_annotations or os.environ.get("GITHUB_ACTIONS") == "true":
        print_github_annotations(all_findings)

    print("----------------------------------------------------------------------")
    print("📖 For remediation guides and canonical patterns, see:")
    print("   docs/standards/logic-hygiene-patterns.md")
    print("----------------------------------------------------------------------")

    should_fail = False
    if args.strict:
        should_fail = bool(all_findings)
    elif args.fail_on == "P0":
        should_fail = any(f.severity == Severity.P0_CRITICAL for f in all_findings)
    elif args.fail_on == "P1":
        should_fail = any(f.severity in (Severity.P0_CRITICAL, Severity.P1_HIGH) for f in all_findings)
    elif args.fail_on == "P2":
        should_fail = bool(all_findings)

    if should_fail:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
