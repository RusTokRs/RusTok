#!/usr/bin/env python3
"""RusToK Domain Logic, Invariant Integrity & Anti-Slop Scanner.

Codified against docs/standards/logic-hygiene-patterns.md.
Scans Rust crates and modules for known anti-patterns, slop, and logic bugs:
  - P-ZERO-INCLUDE: file-stitching macros include!("*.rs")
  - P-ANTI-SWALLOW: swallowed errors let _ =, dummy black_box suppression
  - P-TXN-ROLLBACK-SWALLOW: ignored rollback or commit result on a transaction-like value
  - P-ZERO-PANIC-PROD: unwrap() and expect() in production service paths
  - P-ID-ENTROPY: possible 12-hex route identifiers
  - P-FAIL-CLOSED: possible allow decision on PermissionScope::None
  - P-SQLSTATE-DISCRIM: brittle string matching on database error messages
  - P-BATCH-QUERY: N+1 queries in loops (.one(txn) / .all(txn) inside loops)
  - P-DEAD-TWIN: module-level #[allow(dead_code)] / #[allow(unused)] slop
  - P-RAW-SQL-INTERP: execute_unprepared(&format!(...)) SQL injection risk
  - P-LEPTOS-DEAD-SIGNAL: let _ = signal.get() dead reactive read in UI
  - P-TENANT-FALLBACK-LEAK: default UUID fallback on a tenant selector
  - P-UNFINISHED-CODE: lingering todo!(), FIXME, or TODO markers in production
  - P-OUTBOX-DETACHED: direct event_bus.publish() in a function with transaction evidence
  - P-SYNC-LOCK-AWAIT: std::sync::Mutex or parking_lot guard held across .await points
  - P-SYNC-SLEEP-AWAIT: std::thread::sleep() in async functions
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
from bisect import bisect_left, bisect_right
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from difflib import unified_diff
from enum import Enum
from io import StringIO
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


class Confidence(str, Enum):
    HIGH = "high"
    HEURISTIC = "heuristic"


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
    confidence: Confidence = Confidence.HEURISTIC


# Compiled regex patterns for AST-lite inspection
RE_INCLUDE = re.compile(r'\binclude\s*!\s*\(\s*"([^"\\]+\.rs)"\s*\)')
RE_LET_UNDERSCORE = re.compile(r'^\s*let\s+_\s*=')
RE_TXN_ROLLBACK_COMMIT = re.compile(
    r'\blet\s+_\s*=\s*(?:\w+\.)?(?:transaction|txn|tx)\s*\.\s*(rollback|commit)\s*\(\s*\)\s*\.\s*await\b'
)
RE_DISCARDED_CALL = re.compile(r'\b(?:\w+\s*\(|await\b)')
RE_BLACK_BOX_SUPPRESS = re.compile(r'\b(?:std::hint::)?black_box\s*\(')
RE_EXPECT_UNWRAP = re.compile(r'\.(?:expect|unwrap)\s*\(')
RE_UNWRAP_ERR = re.compile(r'\.unwrap_err\s*\(')
RE_EXPLICIT_PANIC = re.compile(r'\bpanic!\s*\(')
RE_STATIC_CONST_DECL = re.compile(r'^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:static|const)\b')
RE_SHORT_ID_12 = re.compile(
    r'\b(?:[A-Z0-9_]+_)?SHORT_ID_LEN(?:GTH)?\s*(?::\s*usize)?\s*=\s*12\b'
    r'|\.\s*simple\s*\(\s*\)\s*\.\s*to_string\s*\(\s*\)\s*\[\s*0?\s*\.\.\s*12\s*\]'
)
RE_SHORT_ID_SQL = re.compile(r'\b(?:route_)?short_(?:id|identity)"?\s+VARCHAR\s*\(\s*12\s*\)', re.IGNORECASE)
RE_RBAC_LEAK = re.compile(
    r'\bPermissionScope::None\s*=>\s*(?:\{\s*)?(?:return\s+)?'
    r'(?:Ok\s*\(\s*(?:true\b|\(\s*\))\s*\)|true\b)(?=\s*[,;}]|\s*$)'
)
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
RE_TEST_ATTRIBUTE = re.compile(r'#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|(?:tokio::)?test(?:\s*\([^]]*\))?)\s*\]')
RE_TEST_MODULE = re.compile(r'\bmod\s+tests?\b')
RE_ALLOW_DEAD_CODE = re.compile(r'^#!\s*\[\s*allow\s*\([^)]*\b(?:dead_code|unused|unused_variables)\b')
RE_EXECUTE_UNPREPARED = re.compile(r'\.\s*execute_unprepared\s*\(\s*&?\s*')
RE_FORMAT_CALL = re.compile(r'(?:std::)?format\s*!\s*[({\[]')
RE_LOCAL_BINDING = re.compile(r'^\s*let\s+(?:mut\s+)?([A-Za-z_]\w*)(?:\s*:\s*[^=;]+)?\s*=\s*')
RE_LOCAL_ASSIGNMENT = re.compile(r'^\s*([A-Za-z_]\w*)\s*=(?!=|>)\s*')
RE_LOCAL_SQL_ARG = re.compile(r'([A-Za-z_]\w*)\s*,?\s*\)')
RE_FORMAT_LITERAL = re.compile(
    r'r(?P<hashes>#*)".*"(?P=hashes)|"(?:\\.|[^"\\])*"|true|false|[+-]?\d[\d_]*(?:[iu](?:\d+|size))?',
    re.DOTALL,
)
RE_FORMAT_CAPTURE = re.compile(r'\{([A-Za-z_]\w*)(?=[:}])')
RE_LEPTOS_DEAD_SIGNAL = re.compile(r'let\s+_\s*=\s*(?:\w+_)?(?:nonce|signal|state)\.get\(\);')
TENANT_FALLBACK_METHOD = (
    r'(?:\s*\.\s*(?:as_ref|as_deref|copied|cloned)\s*\(\s*\))*\s*\.\s*unwrap_or'
    r'(?:_default\s*\(\s*\)|_else\s*\(\s*(?:\|\|\s*)?(?:Uuid::nil|Uuid::default|Default::default)'
    r'\s*(?:\(\s*\))?\s*\)|\s*\(\s*(?:Uuid::nil|Uuid::default|Default::default)\s*\(\s*\)\s*\))'
)
RE_TENANT_FALLBACK = re.compile(r'\btenant(?:_id)?' + TENANT_FALLBACK_METHOD)
RE_TENANT_BINDING_FALLBACK = re.compile(
    r'^\s*let\s+(?:mut\s+)?tenant_id(?:\s*:\s*[^=;]+)?\s*=\s*(?:[A-Za-z_]\w*\s*\.\s*)*[A-Za-z_]\w*'
    + TENANT_FALLBACK_METHOD
)
RE_INCOMPLETE_MACRO = re.compile(r'\b(?:todo|unimplemented)\s*!\s*[(\[{]')
RE_WORK_COMMENT = re.compile(r'\b(?:FIXME|TODO)\b')
RE_OUTBOX_DETACHED = re.compile(r'\b(?:event_bus|event_publisher)\s*\.\s*publish\s*\(')
RE_TRANSACTION_TOKEN = re.compile(
    r'(?P<function>\bfn\s+(?:r#)?[A-Za-z_]\w*)'
    r'|(?P<transaction>\bDatabaseTransaction\b|\.\s*begin(?:_with_config)?\s*\()'
    r'|(?P<publish>' + RE_OUTBOX_DETACHED.pattern + r')|[(){}\[\];]'
)
RE_ASYNC_FN = re.compile(r'^\s*(?:pub(?:\s*\([^)]*\))?\s+)?async\s+fn\s+([a-zA-Z_]\w*)')
RE_PUB_ASYNC_FN = re.compile(r'^\s*pub(?:\s*\([^)]*\))?\s+async\s+fn\s+([a-zA-Z_]\w*)')
RE_SYNC_LOCK_ACQUIRE = re.compile(
    r'\blet\s+(?:mut\s+)?([a-zA-Z_]\w*)\s*=\s*([^;\n]+?\.(?:lock|read|write)\s*\([^;\n]*\))'
)
RE_AWAIT = re.compile(r'\.await\b')
RE_EXPLICIT_DROP = re.compile(r'^(?:std::mem::)?drop\s*\(\s*([A-Za-z_]\w*)\s*\)\s*;')
RE_EARLY_CLONE = re.compile(
    r'\b(?:let\s+(?:mut\s+)?\w+\s*=|if\s+let\s+Some\(\w+\)\s*=)\s*input\.\w+\.(?:clone\(\)|to_string\(\))'
)
RE_MUTATION_FN_NAME = re.compile(r'^(?:create|update|delete|set|reconcile|merge|publish|execute|handle|dispatch)')
RE_SYNC_SLEEP = re.compile(r'\bstd::thread::sleep\s*\(')
RE_RUST_TOKEN = re.compile(
    r'''//|/\*|\b(?:br|cr|r)(?P<hashes>\#*)"|"|'''
    r"'(?:[^'\\\r\n]|\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|[^\r\n]))'"
)
RE_BLOCK_COMMENT_DELIMITER = re.compile(r'/\*|\*/')
RE_STRING_DELIMITER = re.compile(r'\\[\s\S]|"')
RE_REACTIVE_TOKEN = re.compile(
    r'(?P<reactive>\b(?:create_effect|create_memo|Effect::new|Memo::new)\s*\('
    r'|\bview!\s*[{(\[])|[(){}\[\]]'
)
DELIMITER_PAIRS = {"(": ")", "[": "]", "{": "}"}


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


def rust_source_lines(content: str) -> list[tuple[str, str, str]]:
    """Return aligned code, syntax without literals, and line comments for each line."""
    code_parts: list[str] = []
    syntax_parts: list[str] = []
    copied_until = 0
    position = 0
    while token := RE_RUST_TOKEN.search(content, position):
        position = token.end()
        delimiter = token.group()
        if delimiter == "//":
            line_end = content.find("\n", position)
            position = len(content) if line_end == -1 else line_end + 1
            continue

        if delimiter == "/*":
            depth = 1
            for boundary in RE_BLOCK_COMMENT_DELIMITER.finditer(content, position):
                depth += 1 if boundary.group() == "/*" else -1
                if depth == 0:
                    position = boundary.end()
                    break
            else:
                position = len(content)
        elif token["hashes"] is not None:
            closing = '"' + token["hashes"]
            literal_end = content.find(closing, position)
            position = len(content) if literal_end == -1 else literal_end + len(closing)
        elif delimiter == '"':
            for boundary in RE_STRING_DELIMITER.finditer(content, position):
                if boundary.group() == '"':
                    position = boundary.end()
                    break
            else:
                position = len(content)

        prefix = content[copied_until:token.start()]
        literal = content[token.start():position]
        masked = re.sub(r"[^\r\n]+", lambda match: " " * len(match.group()), literal)
        code_parts.extend((prefix, masked if delimiter == "/*" else literal))
        syntax_parts.extend((prefix, masked))
        copied_until = position

    tail = content[copied_until:]
    code_parts.append(tail)
    syntax_parts.append(tail)
    lines = []
    for code, syntax in zip("".join(code_parts).split("\n"), "".join(syntax_parts).split("\n")):
        syntax, _, comment = syntax.partition("//")
        lines.append((code[:len(syntax)], syntax, comment))
    return lines


def match_with_literals(pattern: re.Pattern[str], code: str, syntax: str) -> re.Match[str] | None:
    """Match a construct with literal arguments only if it starts in Rust code."""
    return next((match for match in pattern.finditer(code) if not syntax[match.start()].isspace()), None)


def reactive_scope_boundaries(syntax: str) -> list[int]:
    """Track entry and exit offsets of recognized Leptos calls and macros."""
    closings: list[str] = []
    reactive_depth = None
    boundaries: list[int] = []
    for token in RE_REACTIVE_TOKEN.finditer(syntax):
        delimiter = token.group()[-1]
        if delimiter in DELIMITER_PAIRS:
            closings.append(DELIMITER_PAIRS[delimiter])
            if token.lastgroup == "reactive" and reactive_depth is None:
                reactive_depth = len(closings)
                boundaries.append(token.end())
        elif closings and delimiter == closings[-1]:
            if len(closings) == reactive_depth:
                boundaries.append(token.start())
                reactive_depth = None
            closings.pop()
    return boundaries


@dataclass
class TransactionScope:
    depth: int
    has_transaction: bool = False
    publications: list[int] = field(default_factory=list)


def transactional_publications(syntax: str) -> list[int]:
    """Find publication offsets in functions with their own transaction evidence."""
    if not RE_OUTBOX_DETACHED.search(syntax):
        return []

    pending: TransactionScope | None = None
    functions: list[TransactionScope] = []
    closings: list[str] = []
    publications: list[int] = []
    for token in RE_TRANSACTION_TOKEN.finditer(syntax):
        delimiter = token.group()[-1]
        if token.lastgroup == "function":
            pending = TransactionScope(len(closings))
            continue
        if token.lastgroup == "transaction":
            scope = pending or (functions[-1] if functions else None)
            if scope is not None:
                scope.has_transaction = True
        if token.lastgroup == "publish" and functions and pending is None:
            functions[-1].publications.append(token.start())

        if delimiter in DELIMITER_PAIRS:
            if delimiter == "{" and pending is not None and pending.depth == len(closings):
                functions.append(pending)
                pending = None
            closings.append(DELIMITER_PAIRS[delimiter])
        elif closings and delimiter == closings[-1]:
            closings.pop()
            if functions and functions[-1].depth == len(closings):
                scope = functions.pop()
                if scope.has_transaction:
                    publications.extend(scope.publications)
        elif delimiter == ";" and pending is not None and pending.depth == len(closings):
            pending = None
    return sorted(publications)


def format_uses_dynamic_values(code: str, syntax: str, opening: int) -> bool:
    """Inspect arguments of a single format! call in a bounded source window."""
    closings = [DELIMITER_PAIRS[syntax[opening]]]
    arguments: list[str] = []
    start = opening + 1
    for token in re.finditer(r"[(){}\[\],]", syntax[start:]):
        delimiter = token.group()
        offset = opening + 1 + token.start()
        if delimiter in DELIMITER_PAIRS:
            closings.append(DELIMITER_PAIRS[delimiter])
        elif delimiter == closings[-1]:
            closings.pop()
            if not closings:
                arguments.append(code[start:offset].strip())
                break
        elif delimiter == "," and len(closings) == 1:
            arguments.append(code[start:offset].strip())
            start = offset + 1
    else:
        return False

    template, *values = arguments
    explicit_names = set()
    for value in values:
        if not value:
            continue  # Optional trailing comma
        name = RE_LOCAL_ASSIGNMENT.match(value)
        if name:
            explicit_names.add(name.group(1))
            value = value[name.end():]
        if not RE_FORMAT_LITERAL.fullmatch(value):
            return True

    template = template.replace("{{", "").replace("}}", "")
    return any(capture not in explicit_names for capture in RE_FORMAT_CAPTURE.findall(template))


def check_interpolated_sql(
    code: str, syntax: str, depth: int, bindings: dict[int, dict[str, bool]]
) -> bool:
    """Track direct format! bindings and uses within lexical blocks."""
    assignment = RE_LOCAL_BINDING.match(syntax) or RE_LOCAL_ASSIGNMENT.match(syntax)
    if assignment:
        name = assignment.group(1)
        format_call = RE_FORMAT_CALL.match(syntax, assignment.end())
        if format_call or any(name in scope for scope in bindings.values()):
            bindings.setdefault(depth, {})[name] = (
                format_call is not None and format_uses_dynamic_values(code, syntax, format_call.end() - 1)
            )

    first_line_end = syntax.find("\n")
    for call in RE_EXECUTE_UNPREPARED.finditer(syntax):
        if first_line_end != -1 and call.start() > first_line_end:
            break
        format_call = RE_FORMAT_CALL.match(syntax, call.end())
        if format_call and format_uses_dynamic_values(code, syntax, format_call.end() - 1):
            return True
        argument = RE_LOCAL_SQL_ARG.match(syntax, call.end())
        if argument:
            name = argument.group(1)
            for binding_depth in sorted(bindings, reverse=True):
                if name in bindings[binding_depth]:
                    if bindings[binding_depth][name]:
                        return True
                    break
    return False


def scan_file(file_path: Path) -> list[Finding]:
    return scan_source(file_path, file_path.read_bytes().decode("utf-8"))


def scan_source(file_path: Path, content: str) -> list[Finding]:
    findings: list[Finding] = []
    try:
        rel_path = file_path.relative_to(ROOT)
    except ValueError:
        rel_path = file_path

    source_lines = content.split("\n")
    lines = rust_source_lines(content)
    code_lines = [syntax.strip() for _, syntax, _ in lines]
    syntax = "\n".join(syntax for _, syntax, _ in lines)
    reactive_boundaries = reactive_scope_boundaries(syntax)
    outbox_publications = transactional_publications(syntax)
    line_offset = 0
    test_scopes: list[int] = []
    pending_test_item = False
    in_loop_brace_depth: int | None = None
    pending_loop = False
    in_async_fn_brace_depth: int | None = None
    pending_async_fn_name: str | None = None
    in_static_brace_depth: int | None = None
    pending_static = False
    active_sync_guards: dict[str, tuple[int, int]] = {}  # guard name -> (line, scope depth)
    current_brace_depth = 0
    sql_bindings: dict[int, dict[str, bool]] = {}
    has_unprepared_sql = "execute_unprepared" in syntax

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

    for idx, (literal_code, syntax_line, comment) in enumerate(lines, start=1):
        code_part = code_lines[idx - 1]
        snippet = literal_code.strip()
        line_start = line_offset
        statement_offset = line_start + len(syntax_line) - len(syntax_line.lstrip())
        line_offset += len(syntax_line) + 1

        # 12. P-UNFINISHED-CODE: incomplete macros in code and work markers in comments
        if not is_test_file and not is_build_file:
            m_todo = RE_INCOMPLETE_MACRO.search(code_part) or RE_WORK_COMMENT.search(comment)
            if m_todo:
                findings.append(Finding(
                    pattern_id="P-UNFINISHED-CODE",
                    rule_name="Lingering Incomplete Marker",
                    severity=Severity.P2_MEDIUM,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=source_lines[idx - 1].strip(),
                    message=f"Found unfinished work marker '{m_todo.group(0)}'. Check the reachable behavior.",
                    suggested_fix="Implement required behavior and track agreed follow-up in the owner plan."
                ))

        # Skip further inspection if the line is purely a comment
        if not code_part:
            continue

        open_braces = code_part.count("{")
        close_braces = code_part.count("}")

        # A test attribute can precede a multiline item of any name.
        if RE_TEST_ATTRIBUTE.search(code_part) or RE_TEST_MODULE.search(code_part):
            pending_test_item = True
        is_test_context = is_test_file or bool(test_scopes) or pending_test_item
        if pending_test_item:
            if open_braces:
                test_scopes.append(current_brace_depth)
                pending_test_item = False
            elif ";" in code_part:
                pending_test_item = False

        # Loop tracking (supporting multiline headers)
        if RE_FOR_OR_WHILE_LOOP.search(code_part):
            if "{" in code_part:
                in_loop_brace_depth = current_brace_depth
            else:
                pending_loop = True
        elif pending_loop and "{" in code_part:
            in_loop_brace_depth = current_brace_depth
            pending_loop = False

        # Static / LazyLock block tracking
        if RE_STATIC_CONST_DECL.search(code_part) or "Lazy::new" in code_part or "LazyLock::new" in code_part or "lazy_static!" in code_part:
            if "{" in code_part:
                in_static_brace_depth = current_brace_depth
            else:
                pending_static = True
        elif pending_static and "{" in code_part:
            in_static_brace_depth = current_brace_depth
            pending_static = False

        # Track async function entry (supporting multiline signatures)
        m_async_fn = RE_ASYNC_FN.match(code_part)
        if m_async_fn:
            fn_name = m_async_fn.group(1)
            if "{" in code_part:
                in_async_fn_brace_depth = current_brace_depth
            else:
                pending_async_fn_name = fn_name

            # 17. P-STRUCTURED-TRACE: uninstrumented public domain service mutation entrypoint
            is_pub_fn = RE_PUB_ASYNC_FN.match(code_part) is not None
            if is_services_file and not is_test_context and is_pub_fn and RE_MUTATION_FN_NAME.match(fn_name):
                has_instrument = any(
                    "instrument" in code_lines[j]
                    for j in range(max(0, idx - 16), idx - 1)
                )
                if not has_instrument:
                    findings.append(Finding(
                        pattern_id="P-STRUCTURED-TRACE",
                        rule_name="Contextual Trace Spans",
                        severity=Severity.P2_MEDIUM,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=snippet,
                        message=f"Candidate mutation '{fn_name}' has no nearby instrumentation marker.",
                        suggested_fix="Verify that the owner entry point has the required operation and tenant context."
                    ))
        elif pending_async_fn_name is not None and "{" in code_part:
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
                    line_content=snippet,
                    message="Blocking sleep appears in an async function. Verify which thread executes it.",
                    suggested_fix="Use tokio::time::sleep(duration).await on executor workers."
                ))

            m_lock = RE_SYNC_LOCK_ACQUIRE.search(code_part)
            if m_lock and "tokio::sync" not in code_part:
                acquisition = " ".join(code_lines[idx - 1:idx + 5]).partition(";")[0]
                if not RE_AWAIT.search(acquisition):
                    active_sync_guards[m_lock.group(1)] = (idx, current_brace_depth)

            # An inner branch can drop an outer guard on only some paths.
            m_drop = RE_EXPLICIT_DROP.match(code_part)
            if m_drop:
                guard_name = m_drop.group(1)
                guard = active_sync_guards.get(guard_name)
                if guard is not None and guard[1] == current_brace_depth:
                    active_sync_guards.pop(guard_name)

            # 15. P-SYNC-LOCK-AWAIT: synchronous lock guard held across .await point
            if RE_AWAIT.search(code_part) and active_sync_guards:
                in_scope_guards = [
                    (g_name, g_info)
                    for g_name, g_info in list(active_sync_guards.items())
                    if current_brace_depth >= g_info[1]
                ]
                for g_name, g_info in in_scope_guards:
                    findings.append(Finding(
                        pattern_id="P-SYNC-LOCK-AWAIT",
                        rule_name="Sync Lock Held Across Await",
                        severity=Severity.P0_CRITICAL,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=snippet,
                        message=f"Possible synchronous guard '{g_name}' from line {g_info[0]} across .await.",
                        suggested_fix=f"Verify the guard type and end '{g_name}' scope before awaiting when possible."
                    ))
                    active_sync_guards.pop(g_name, None)

        # 1. P-ZERO-INCLUDE: file-stitching macro
        m_inc = match_with_literals(RE_INCLUDE, literal_code, syntax_line)
        if m_inc and not is_test_context and not is_build_file:
            included_file = m_inc.group(1)
            mod_name = Path(included_file).stem
            findings.append(Finding(
                pattern_id="P-ZERO-INCLUDE",
                rule_name="Zero Include Slop",
                severity=Severity.P1_HIGH,
                file_path=rel_path,
                line_number=idx,
                line_content=snippet,
                message=f"Forbidden file-stitching macro '{m_inc.group(0)}'. Use canonical 'mod ...;' and public exports.",
                suggested_fix=f"Declare mod {mod_name}; and expose only the required items.",
                confidence=Confidence.HIGH,
            ))

        # 2 & 3. P-ANTI-SWALLOW & P-TXN-ROLLBACK-SWALLOW: swallowed errors (supporting multiline expressions)
        if not is_test_context:
            if RE_LET_UNDERSCORE.match(code_part):
                # Look ahead for multiline expressions if semicolon is missing
                full_stmt = code_part
                if ";" not in code_part:
                    for k in range(idx, min(idx + 5, len(lines))):
                        sub = code_lines[k]
                        if sub.startswith("pub ") or sub.startswith("fn ") or sub.startswith("async "):
                            break
                        full_stmt += " " + sub
                        if ";" in sub:
                            break

                transaction_end = RE_TXN_ROLLBACK_COMMIT.search(full_stmt)
                if transaction_end:
                    operation = transaction_end.group(1)
                    suggested_fix = (
                        "Propagate the commit failure and preserve the operation's retry contract."
                        if operation == "commit" else
                        "Handle the rollback failure while preserving the primary operation error."
                    )
                    findings.append(Finding(
                        pattern_id="P-TXN-ROLLBACK-SWALLOW",
                        rule_name="Ignored Transaction Finalization Result",
                        severity=Severity.P0_CRITICAL,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=snippet,
                        message=f"Ignored {operation} result on a transaction-like value.",
                        suggested_fix=suggested_fix,
                    ))
                elif RE_LEPTOS_DEAD_SIGNAL.search(full_stmt):
                    if bisect_right(reactive_boundaries, statement_offset) % 2 == 0:
                        findings.append(Finding(
                            pattern_id="P-LEPTOS-DEAD-SIGNAL",
                            rule_name="Dead Reactive Signal Read",
                            severity=Severity.P1_HIGH,
                            file_path=rel_path,
                            line_number=idx,
                            line_content=snippet,
                            message="Calling signal.get() in let _ = outside of create_effect / view! drops reactivity. UI will not update.",
                            suggested_fix="create_effect(move |_| { signal.get(); /* trigger reactive update */ });"
                        ))
                elif RE_DISCARDED_CALL.search(full_stmt):
                    findings.append(Finding(
                        pattern_id="P-ANTI-SWALLOW",
                        rule_name="Zero Error Swallowing",
                        severity=Severity.P1_HIGH,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=snippet,
                        message="Discarded call result via 'let _ ='. Verify whether it can fail.",
                        suggested_fix="Propagate a required failure or record an explicitly best-effort failure."
                    ))

            if RE_BLACK_BOX_SUPPRESS.search(code_part):
                findings.append(Finding(
                    pattern_id="P-ANTI-SWALLOW",
                    rule_name="Dummy Compiler Suppression",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=snippet,
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
                        line_content=snippet,
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
                    line_content=snippet,
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
                    line_content=snippet,
                    message="Calling panic!(...) in production path. Return typed DomainError / PortError.",
                    suggested_fix="return Err(DomainError::from(...));"
                ))

        # 5. P-ID-ENTROPY: possible 12-hex route identifiers
        if not is_test_context and (RE_SHORT_ID_12.search(code_part) or RE_SHORT_ID_SQL.search(literal_code)):
            findings.append(Finding(
                pattern_id="P-ID-ENTROPY",
                rule_name="Bounded Identifier Entropy",
                severity=Severity.P2_MEDIUM,
                file_path=rel_path,
                line_number=idx,
                line_content=snippet,
                message="Possible 12-hex identifier. Verify generator entropy and the uniqueness scope.",
                suggested_fix="Use an owner-defined collision policy and preserve existing route compatibility."
            ))

        # 6. P-FAIL-CLOSED: RBAC PermissionScope::None => Ok leakage
        if not is_test_context:
            full_rbac = code_part
            if "PermissionScope::None" in code_part and "=>" in code_part and ";" not in code_part and "}" not in code_part:
                for k in range(idx, min(idx + 3, len(lines))):
                    sub_code = code_lines[k]
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
                    line_content=snippet,
                    message="PermissionScope::None may grant access. Verify the caller's decision contract.",
                    suggested_fix="Return the owner-defined denial value at the authorization boundary."
                ))

        # 7. P-SQLSTATE-DISCRIM: brittle string matching in error.rs or services
        if not is_test_context and ("error" in file_path.name or is_services_file):
            m_err_var = match_with_literals(RE_ERR_VAR_CONTAINS, literal_code, syntax_line)
            m_gen_var = match_with_literals(RE_GENERIC_CONTAINS, literal_code, syntax_line)
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
                    line_content=snippet,
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
                    line_content=snippet,
                    message="Suspected N+1 query: database query inside loop body. Batch with Column::Id.is_in(ids).all().",
                    suggested_fix="Collect IDs and perform one Entity::find().filter(Column::Id.is_in(ids)).all(txn) before loop."
                ))

        # 9. P-DEAD-TWIN: module-level #[allow(dead_code)] slop
        if RE_ALLOW_DEAD_CODE.match(code_part) and not is_test_context and not is_migr_file:
            findings.append(Finding(
                pattern_id="P-DEAD-TWIN",
                rule_name="File-Level Slop Suppression",
                severity=Severity.P2_MEDIUM,
                file_path=rel_path,
                line_number=idx,
                line_content=snippet,
                message="Module-wide dead-code or unused lint suppression may hide unused implementations.",
                suggested_fix="Verify reachability and remove dormant implementations."
            ))

        # 10. P-RAW-SQL-INTERP: direct or locally bound format! used as unprepared SQL
        if has_unprepared_sql and not is_test_context and not is_migr_file:
            sql_line = (
                "execute_unprepared" in code_part
                or RE_LOCAL_BINDING.match(code_part)
                or RE_LOCAL_ASSIGNMENT.match(code_part)
            )
            window = lines[idx - 1:idx + 15] if sql_line else []
            if window and check_interpolated_sql(
                "\n".join(code for code, _, _ in window),
                "\n".join(syntax for _, syntax, _ in window),
                current_brace_depth,
                sql_bindings,
            ):
                findings.append(Finding(
                    pattern_id="P-RAW-SQL-INTERP",
                    rule_name="Unprepared SQL Interpolation",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=snippet,
                    message="Unprepared SQL contains formatted values. Verify their origin and parameterize query data.",
                    suggested_fix="Statement::from_sql_and_values(db.get_database_backend(), sql, values)"
                ))

        # 11. P-TENANT-FALLBACK-LEAK: unwrap_or_default on tenant_id
        if not is_test_context:
            if RE_TENANT_FALLBACK.search(code_part) or RE_TENANT_BINDING_FALLBACK.search(code_part):
                findings.append(Finding(
                    pattern_id="P-TENANT-FALLBACK-LEAK",
                    rule_name="Tenant Fallback Leakage",
                    severity=Severity.P0_CRITICAL,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=snippet,
                    message="Possible default tenant selector. Verify that missing tenant authority fails closed.",
                    suggested_fix="tenant_id.ok_or(DomainError::TenantNotFound)?"
                ))

        # 13. P-OUTBOX-DETACHED: event_bus.publish() in transactional contexts
        if not is_test_context and outbox_publications:
            publication = bisect_left(outbox_publications, line_start)
            if publication < len(outbox_publications) and outbox_publications[publication] < line_offset:
                findings.append(Finding(
                    pattern_id="P-OUTBOX-DETACHED",
                    rule_name="Transactional Outbox Decoupling",
                    severity=Severity.P1_HIGH,
                    file_path=rel_path,
                    line_number=idx,
                    line_content=snippet,
                    message="Direct publication in a function with transaction evidence. Verify which mutation the event represents.",
                    suggested_fix="Write events for the mutation through the owner outbox in the same transaction."
                ))

        # 16. P-HOTPATH-ALLOC: premature clone of input before validation gates
        if is_services_file and not is_test_context and RE_EARLY_CLONE.search(code_part):
            for next_idx in range(idx, min(idx + 5, len(lines))):
                next_code = code_lines[next_idx]
                if "validate_" in next_code or "check_" in next_code:
                    findings.append(Finding(
                        pattern_id="P-HOTPATH-ALLOC",
                        rule_name="Premature Input Allocation",
                        severity=Severity.P2_MEDIUM,
                        file_path=rel_path,
                        line_number=idx,
                        line_content=snippet,
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
            if g_info[1] > current_brace_depth
        ]
        for g_name in guards_to_drop:
            active_sync_guards.pop(g_name, None)

        for depth in list(sql_bindings):
            if depth > current_brace_depth:
                del sql_bindings[depth]

        while test_scopes and current_brace_depth <= test_scopes[-1]:
            test_scopes.pop()

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


def git_output(*args: str) -> bytes:
    try:
        return subprocess.run(
            ["git", *args], cwd=ROOT, capture_output=True, check=True
        ).stdout
    except subprocess.CalledProcessError as error:
        detail = error.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"git {' '.join(args)} failed: {detail or error.returncode}") from error


def staged_rust_blobs() -> dict[Path, str]:
    """Capture staged paths and blob IDs from the same index snapshot."""
    records = git_output(
        "diff", "--cached", "--raw", "--no-abbrev", "--no-renames",
        "--diff-filter=AMTU", "-z", "--"
    ).split(b"\0")
    blobs = {}
    for metadata, filename in zip(records[0:-1:2], records[1:-1:2]):
        if not filename.endswith(b".rs"):
            continue
        file_path = ROOT / os.fsdecode(filename)
        _, mode, _, oid, status = metadata.split()
        if status == b"U":
            raise RuntimeError(f"Unmerged Rust file in Git index: {file_path}")
        if mode in (b"100644", b"100755"):
            blobs[file_path] = oid.decode("ascii")
    return blobs


def discover_git_files(base_ref: str | None = None) -> list[Path]:
    """Select working-tree files changed relative to a commit."""
    if base_ref is None:
        base = "HEAD"
    else:
        base = git_output("rev-parse", "--verify", "--end-of-options", f"{base_ref}^{{commit}}").decode("ascii").strip()
    filenames = git_output("diff", "--name-only", "--diff-filter=ACMRT", "-z", base, "--").split(b"\0")
    if base_ref is None:
        filenames.extend(git_output("ls-files", "--others", "--exclude-standard", "-z", "--").split(b"\0"))
    files = {ROOT / os.fsdecode(name) for name in filenames if name.endswith(b".rs")}
    return sorted(path for path in files if path.is_file())


def module_scorecard(findings: list[Finding]) -> list[tuple[str, Counter[str]]]:
    counts: dict[str, Counter[str]] = defaultdict(Counter)
    for finding in findings:
        stats = counts[extract_module_name(finding.file_path)]
        stats[finding.severity.value[:2]] += 1
        stats[finding.confidence.value] += 1
    return sorted(
        counts.items(),
        key=lambda item: (-item[1]["high"], -item[1]["P0"], -item[1]["P1"], item[0]),
    )


def print_scorecard(findings: list[Finding]) -> None:
    print("\nPLATFORM HYGIENE SCORECARD (BY MODULE)")
    print(f"{'Module / Component':<34} | {'P0':>4} | {'P1':>4} | {'P2':>4} | {'High confidence':>15} | Heuristic")
    for mod, stats in module_scorecard(findings):
        print(
            f"{mod:<34} | {stats['P0']:>4} | {stats['P1']:>4} | {stats['P2']:>4}"
            f" | {stats['high']:>15} | {stats['heuristic']}"
        )


def generate_markdown_report(findings: list[Finding], scanned_files: int, target_desc: str) -> str:
    """Generate a clean GitHub Flavored Markdown report suitable for PR descriptions or audit evidence."""
    by_pattern: dict[str, list[Finding]] = defaultdict(list)
    confidence_counts = Counter(finding.confidence for finding in findings)
    for finding in findings:
        by_pattern[finding.pattern_id].append(finding)

    lines = [
        "# RusToK Domain Hygiene & Invariant Verification Report",
        "",
        f"- **Target:** `{target_desc}`",
        f"- **Files Scanned:** {scanned_files}",
        f"- **Findings:** {len(findings)} ({confidence_counts[Confidence.HIGH]} high confidence, "
        f"{confidence_counts[Confidence.HEURISTIC]} heuristic)",
        "",
        "## Module Scorecard",
        "",
        "| Module | P0 Critical | P1 High | P2 Medium | High confidence | Heuristic |",
        "|---|---|---|---|---|---|",
    ]

    for mod, stats in module_scorecard(findings):
        lines.append(
            f"| `{mod}` | {stats['P0']} | {stats['P1']} | {stats['P2']} | {stats['high']} | {stats['heuristic']} |"
        )

    lines.append("")
    lines.append("## Discovered Findings by Pattern")
    lines.append("")

    for pid, f_list in sorted(by_pattern.items()):
        rule_name = f_list[0].rule_name
        sev = f_list[0].severity.value
        lines.append(f"### `[{sev}]` {pid}: {rule_name} ({len(f_list)} occurrence(s))")
        lines.append("")
        lines.append("| File | Line | Confidence | Snippet | Recommendation |")
        lines.append("|---|---|---|---|---|")
        for item in f_list[:15]:
            snippet = item.line_content.replace("|", "\\|").replace("`", "'").strip()
            fix = item.suggested_fix or item.message
            fix_escaped = fix.replace("|", "\\|").replace("`", "'")
            file_link = f"`{item.file_path.as_posix()}:{item.line_number}`"
            lines.append(f"| {file_link} | {item.line_number} | {item.confidence.value} | `{snippet[:60]}` | {fix_escaped} |")
        if len(f_list) > 15:
            lines.append(f"| *... and {len(f_list) - 15} more occurrences* | | | | |")
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
    writer.writerow(["pattern_id", "rule_name", "severity", "file", "line", "snippet", "message", "suggested_fix", "confidence"])
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
            f.confidence.value,
        ])
    return output.getvalue()


def diff_path(prefix: str, path: Path) -> str:
    name = f"{prefix}/{path.as_posix()}"
    if not re.search(r'[\x00-\x1f"\\]', name):
        return name
    quoted = "".join(
        chr(byte) if 32 <= byte < 127 and byte not in (34, 92) else f"\\{byte:03o}"
        for byte in os.fsencode(name)
    )
    return f'"{quoted}"'


def generate_unified_patch(findings: list[Finding], sources: dict[Path, str]) -> str:
    """Append advisory comments without editing the flagged Rust code."""
    patches: list[str] = []
    by_file: dict[Path, list[Finding]] = defaultdict(list)
    for finding in findings:
        if finding.suggested_fix:
            by_file[finding.file_path].append(finding)

    for file_path, file_findings in sorted(by_file.items()):
        if file_path.is_absolute() or ".." in file_path.parts:
            raise ValueError(f"--patch requires files inside the repository: {file_path}")

        content = sources[file_path]
        newline = "\r\n" if "\r\n" in content else "\n"
        notes = []
        # A finding can be reported on a line that starts inside a string or block comment.
        # Place notes at EOF so insertion cannot change the content of those tokens.
        for finding in sorted(file_findings, key=lambda item: item.line_number):
            fix = " ".join(finding.suggested_fix.splitlines())
            notes.append(f"// REMEDIATION ({finding.pattern_id}, line {finding.line_number}): {fix}{newline}")
        updated = content + ("" if content.endswith("\n") else newline) + newline + "".join(notes)
        diff = unified_diff(
            StringIO(content, newline="\n").readlines(),
            StringIO(updated, newline="\n").readlines(),
            fromfile=diff_path("a", file_path),
            tofile=diff_path("b", file_path),
        )
        for line in diff:
            patches.append(line if line.endswith("\n") else line + "\n\\ No newline at end of file\n")
    return "".join(patches)


def print_github_annotations(findings: list[Finding]) -> None:
    """Emit GitHub Actions workflow commands for inline pull request annotations."""
    for f in findings:
        if f.confidence == Confidence.HIGH and f.severity == Severity.P0_CRITICAL:
            level = "error"
        elif f.severity in (Severity.P0_CRITICAL, Severity.P1_HIGH):
            level = "warning"
        else:
            level = "notice"
        file_str = str(f.file_path).replace("\\", "/")
        msg = f.message.replace("\n", " ").replace("%", "%25").replace("\r", "")
        print(f"::{level} file={file_str},line={f.line_number},title={f.pattern_id}::[{f.confidence.value}] {msg}")


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
        help="Scan .rs file contents from the Git index (ideal for pre-commit hooks)",
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
        help="Exit with code 1 for any high-confidence finding (equivalent to --fail-on P2)",
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
        help="Generate an applicable unified diff (.patch) with advisory comments",
    )
    parser.add_argument(
        "--github-annotations",
        action="store_true",
        help="Emit GitHub Actions workflow commands (::error / ::warning) for inline PR annotations",
    )
    parser.add_argument(
        "--fail-on",
        choices=["P0", "P1", "P2"],
        help="Exit with code 1 for high-confidence findings at or above this severity",
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

    staged_blobs: dict[Path, str] = {}
    try:
        if args.staged:
            staged_blobs = staged_rust_blobs()
            files_to_scan = sorted(staged_blobs)
            target_description = "git staged files"
        elif args.changed:
            files_to_scan = discover_git_files()
            target_description = "working tree modified files"
        elif args.diff is not None:
            files_to_scan = discover_git_files(args.diff)
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
    except (OSError, RuntimeError) as error:
        print(f"[ERROR] Could not select scan targets: {error}", file=sys.stderr)
        return 2

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

    def scan_target(path: Path) -> tuple[list[Finding], str]:
        if path in staged_blobs:
            data = git_output("cat-file", "blob", staged_blobs[path])
        else:
            data = path.read_bytes()
        content = data.decode("utf-8")
        return scan_source(path, content), content

    patch_sources: dict[Path, str] = {}

    def collect_results(results: Iterator[tuple[list[Finding], str]]) -> None:
        for findings, content in results:
            all_findings.extend(findings)
            if args.patch and findings:
                patch_sources[findings[0].file_path] = content

    try:
        if args.threads > 1 and len(files_to_scan) > 20:
            with ThreadPoolExecutor(max_workers=args.threads) as executor:
                collect_results(executor.map(scan_target, files_to_scan))
        else:
            collect_results(map(scan_target, files_to_scan))
    except (OSError, RuntimeError, UnicodeError) as error:
        print(f"[ERROR] Could not read scan targets: {error}", file=sys.stderr)
        return 2

    if ignored_rules:
        all_findings = [f for f in all_findings if f.pattern_id not in ignored_rules]

    # Filter by severity if requested
    if args.severity == "P0":
        all_findings = [f for f in all_findings if f.severity == Severity.P0_CRITICAL]
    elif args.severity == "P1":
        all_findings = [f for f in all_findings if f.severity in (Severity.P0_CRITICAL, Severity.P1_HIGH)]

    fail_on = "P2" if args.strict else args.fail_on
    exit_code = int(fail_on is not None and any(
        finding.confidence == Confidence.HIGH and finding.severity.value[:2] <= fail_on
        for finding in all_findings
    ))

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
                    "confidence": f.confidence.value,
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
        return exit_code

    print(f"Scanned {scanned_file_count} Rust file(s). Found {len(all_findings)} issue(s).\n")

    if not all_findings:
        print("No findings for the selected rules and source files.")
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
                print(f"  {posix_file}:{item.line_number} [{item.confidence.value}]")
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
        try:
            patch_content = generate_unified_patch(all_findings, patch_sources)
            patch_path.write_bytes(patch_content.encode("utf-8"))
        except (OSError, ValueError) as error:
            print(f"[ERROR] Could not write patch: {error}", file=sys.stderr)
            return 2
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

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
