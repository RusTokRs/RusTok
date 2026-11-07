#!/usr/bin/env python3
"""RusToK Domain Logic, Invariant Integrity & Anti-Slop Scanner.

Codified against docs/standards/logic-hygiene-patterns.md.
Scans Rust crates and modules for known anti-patterns, slop, and logic bugs:
  - P-ZERO-INCLUDE: file-stitching macros include!("*.rs")
  - P-ANTI-SWALLOW: swallowed errors let _ =, dummy black_box suppression
  - P-ZERO-PANIC-PROD: unwrap() and expect() in production service paths
  - P-ID-ENTROPY: obsolete 48-bit short_id (VARCHAR(12), [..12], left(..., 12))
  - P-FAIL-CLOSED: RBAC PermissionScope::None => Ok / true leakage
  - P-SQLSTATE-DISCRIM: brittle string matching on database error messages
  - P-BATCH-QUERY: N+1 queries in loops (.one(txn) / .all(txn) inside loops)
  - P-DEAD-TWIN: module-level #[allow(dead_code)] / #[allow(unused)] slop
"""
from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator

# Ensure UTF-8 output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent


@dataclass
class Finding:
    pattern_id: str
    rule_name: str
    file_path: Path
    line_number: int
    line_content: str
    message: str


# Compiled regex patterns for AST-lite inspection
RE_INCLUDE = re.compile(r'include!\s*\(\s*"([^"]+\.rs)"\s*\)')
# Matches single-line and multiline 'let _ ='
RE_LET_UNDERSCORE = re.compile(r'^\s*let\s+_\s*=')
RE_BLACK_BOX_SUPPRESS = re.compile(r'\b(?:std::hint::)?black_box\s*\(')
RE_EXPECT_UNWRAP = re.compile(r'\.(?:expect|unwrap)\s*\(')
RE_SHORT_ID_12 = re.compile(
    r'(?:VARCHAR\s*\(\s*12\s*\)|\[\s*0?\s*\.\.\s*12\s*\]|\"000000000000\"|left\s*\([^,]+,\s*12\s*\)|SHORT_ID_LEN(?:GTH)?\s*(?::\s*usize)?\s*=\s*12\b)'
)
RE_RBAC_LEAK = re.compile(r'PermissionScope::None\s*=>\s*(?:\{?\s*(?:return\s+)?)?(?:Ok\b|true\b)')
RE_ERR_CONTAINS = re.compile(r'\b(?:message|msg|err|error|db_err|e)\s*(?:\.\s*to_string\s*\(\s*\))?\s*\.\s*contains\s*\(\s*"')
RE_QUERY_IN_TXN = re.compile(r'\.(?:one|all|one_by_id|query_one|query_all)\s*\(\s*(?:&?\w*txn|&?\w*tx|&db)\s*\)\.await')
RE_FOR_OR_WHILE_LOOP = re.compile(r'\b(?:for\s+\w+\s+in\b|while\b)')
RE_ALLOW_DEAD_CODE = re.compile(r'^\s*#!?\[allow\((?:dead_code|unused|unused_variables)\)\]')


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
    )


def is_build_or_example(file_path: Path) -> bool:
    path_str = file_path.as_posix().lower()
    return file_path.name == "build.rs" or "/examples/" in path_str


def scan_file(file_path: Path) -> list[Finding]:
    findings: list[Finding] = []
    rel_path = file_path.relative_to(ROOT)

    try:
        content = file_path.read_text(encoding="utf-8", errors="replace")
    except Exception as e:
        print(f"[WARN] Failed to read {rel_path}: {e}", file=sys.stderr)
        return []

    lines = content.splitlines()
    in_test_module_brace_depth: int | None = None
    in_loop_brace_depth: int | None = None
    in_block_comment = False
    current_brace_depth = 0

    is_test_file = is_test_or_bench(file_path)
    is_build_file = is_build_or_example(file_path)

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
                # Comment opens and closes on the same line; remove it
                stripped = re.sub(r'/\*.*?\*/', '', stripped).strip()
            else:
                in_block_comment = True
                stripped = stripped.split("/*", 1)[0].strip()

        if not stripped or stripped.startswith("//"):
            continue

        # Track brace depth for scope tracking
        open_braces = stripped.count("{")
        close_braces = stripped.count("}")

        # Track test module entry (e.g. #[cfg(test)] mod tests { )
        if "#[cfg(test)]" in stripped or re.search(r'\bmod\s+tests?\b', stripped):
            in_test_module_brace_depth = current_brace_depth

        # Track loop entry
        if RE_FOR_OR_WHILE_LOOP.search(stripped) and "{" in stripped:
            in_loop_brace_depth = current_brace_depth

        is_test_context = (
            is_test_file
            or (in_test_module_brace_depth is not None and current_brace_depth >= in_test_module_brace_depth)
        )

        # 1. P-ZERO-INCLUDE: file-stitching macro
        m_inc = RE_INCLUDE.search(stripped)
        if m_inc and "OUT_DIR" not in stripped:
            findings.append(Finding(
                pattern_id="P-ZERO-INCLUDE",
                rule_name="Zero Include Slop",
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message=f"Forbidden file-stitching macro '{m_inc.group(0)}'. Use canonical 'mod ...;' and public exports."
            ))

        # 2. P-ANTI-SWALLOW: let _ = or dummy black_box suppression in prod
        if not is_test_context:
            m_let = RE_LET_UNDERSCORE.match(stripped)
            if m_let:
                findings.append(Finding(
                    pattern_id="P-ANTI-SWALLOW",
                    rule_name="Zero Error Swallowing",
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Swallowed error or variable via 'let _ ='. Propagate typed Result or log with warn!(%err)."
                ))

            if RE_BLACK_BOX_SUPPRESS.search(stripped):
                findings.append(Finding(
                    pattern_id="P-ANTI-SWALLOW",
                    rule_name="Dummy Compiler Suppression",
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Abuse of 'black_box' in production service code. Do not fake variable usage to silence linter."
                ))

        # 3. P-ZERO-PANIC-PROD: expect() or unwrap() in production paths (excluding tests and build.rs)
        if not is_test_context and not is_build_file:
            m_panic = RE_EXPECT_UNWRAP.search(stripped)
            if m_panic and "static" not in file_path.name:
                findings.append(Finding(
                    pattern_id="P-ZERO-PANIC-PROD",
                    rule_name="Zero Production Panics",
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Calling .expect() or .unwrap() in production path. Return typed DomainError / PortError."
                ))

        # 4. P-ID-ENTROPY: obsolete 48-bit short_id
        if RE_SHORT_ID_12.search(stripped):
            findings.append(Finding(
                pattern_id="P-ID-ENTROPY",
                rule_name="Bounded Identifier Entropy",
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="Deprecated 12-char identifier (48-bit entropy). Standardize on 16-hex characters (VARCHAR(16))."
            ))

        # 5. P-FAIL-CLOSED: RBAC PermissionScope::None => Ok leakage
        if RE_RBAC_LEAK.search(stripped):
            findings.append(Finding(
                pattern_id="P-FAIL-CLOSED",
                rule_name="Fail-Closed Security Floor",
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="PermissionScope::None must resolve to Err(Forbidden), never Ok(()) or true."
            ))

        # 6. P-SQLSTATE-DISCRIM: brittle string matching in error.rs or error handlers
        if ("error" in file_path.name or "service" in file_path.name) and RE_ERR_CONTAINS.search(stripped):
            findings.append(Finding(
                pattern_id="P-SQLSTATE-DISCRIM",
                rule_name="SQLSTATE Error Mapping",
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="Brittle string matching on error message. Discriminate using SQLSTATE (23505, P0001)."
            ))

        # 7. P-BATCH-QUERY: N+1 queries in loops (.one(txn) / .all(txn) inside loops)
        if in_loop_brace_depth is not None and not is_test_context:
            if RE_QUERY_IN_TXN.search(stripped):
                findings.append(Finding(
                    pattern_id="P-BATCH-QUERY",
                    rule_name="Transactional Batching",
                    file_path=rel_path,
                    line_number=idx,
                    line_content=stripped,
                    message="Suspected N+1 query: database query inside loop body. Batch with Column::Id.is_in(ids).all()."
                ))

        # 8. P-DEAD-TWIN: module-level #[allow(dead_code)] slop
        if idx <= 15 and RE_ALLOW_DEAD_CODE.match(stripped) and not is_test_context:
            findings.append(Finding(
                pattern_id="P-DEAD-TWIN",
                rule_name="File-Level Slop Suppression",
                file_path=rel_path,
                line_number=idx,
                line_content=stripped,
                message="Module-level #[allow(dead_code|unused)]. AGENTS.md §14 forbids broad suppression of dead code."
            ))

        # Update brace depths at end of line
        current_brace_depth += (open_braces - close_braces)
        if current_brace_depth < 0:
            current_brace_depth = 0

        # Reset test module if exited
        if in_test_module_brace_depth is not None and current_brace_depth <= in_test_module_brace_depth:
            in_test_module_brace_depth = None

        # Reset loop if exited
        if in_loop_brace_depth is not None and current_brace_depth <= in_loop_brace_depth:
            in_loop_brace_depth = None

    return findings


def discover_rust_files(target_dir: Path) -> Iterator[Path]:
    for root, dirs, files in os.walk(target_dir):
        # Exclude target/, node_modules/, and .git
        dirs[:] = [d for d in dirs if d not in ("target", "node_modules", ".git")]
        for f in files:
            if f.endswith(".rs"):
                yield Path(root) / f


def main() -> int:
    parser = argparse.ArgumentParser(
        description="RusToK Domain Logic, Invariant Integrity & Anti-Slop Scanner."
    )
    parser.add_argument(
        "--crate",
        type=str,
        help="Target crate name (e.g. rustok-commerce, rustok-forum, rustok-index)",
    )
    parser.add_argument(
        "--path",
        type=str,
        help="Target directory path relative to repository root (e.g. crates/modules/rustok-commerce)",
    )
    parser.add_argument(
        "--all-modules",
        action="store_true",
        help="Scan all modules in crates/modules/",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit with code 1 if any finding is discovered (useful for CI/pre-commit)",
    )
    args = parser.parse_args()

    target_paths: list[Path] = []

    if args.crate:
        # Locate crate under crates/modules/ or crates/libs/
        candidates = list((ROOT / "crates" / "modules").glob(args.crate)) + list(
            (ROOT / "crates" / "libs").glob(args.crate)
        )
        if not candidates:
            print(f"[ERROR] Could not find crate matching '{args.crate}'", file=sys.stderr)
            return 2
        target_paths.append(candidates[0])
    elif args.path:
        p = ROOT / args.path
        if not p.exists():
            print(f"[ERROR] Path does not exist: {p}", file=sys.stderr)
            return 2
        target_paths.append(p)
    elif args.all_modules:
        target_paths.append(ROOT / "crates" / "modules")
    else:
        # Default: scan current directory if inside repo, or crates/modules
        cwd = Path.cwd()
        if cwd.resolve() == ROOT.resolve():
            target_paths.append(ROOT / "crates" / "modules")
        else:
            target_paths.append(cwd)

    all_findings: list[Finding] = []
    scanned_file_count = 0

    print("======================================================================")
    print(" 🛡️  RusToK Domain Hygiene & Anti-Slop Scanner")
    print(" Canonical Reference: docs/standards/logic-hygiene-patterns.md")
    print("======================================================================")

    for base_path in target_paths:
        print(f"[SCAN] Scanning directory: {base_path.relative_to(ROOT)}")
        for rs_file in discover_rust_files(base_path):
            scanned_file_count += 1
            findings = scan_file(rs_file)
            all_findings.extend(findings)

    print("----------------------------------------------------------------------")
    print(f"Scanned {scanned_file_count} Rust file(s). Found {len(all_findings)} issue(s).\n")

    if not all_findings:
        print("✅ No domain logic defects, slop, or anti-patterns detected! Clean code.")
        return 0

    # Group findings by pattern ID
    by_pattern: dict[str, list[Finding]] = {}
    for f in all_findings:
        by_pattern.setdefault(f.pattern_id, []).append(f)

    for pid, findings in sorted(by_pattern.items()):
        print(f"=== {pid}: {findings[0].rule_name} ({len(findings)} occurrence(s)) ===")
        for item in findings[:10]:  # Cap output per pattern
            print(f"  {item.file_path}:{item.line_number}")
            print(f"    Line:    {item.line_content}")
            print(f"    Fix:     {item.message}")
            print()
        if len(findings) > 10:
            print(f"  ... and {len(findings) - 10} more occurrences omitted.\n")

    print("----------------------------------------------------------------------")
    print("📖 For remediation guides and canonical patterns, see:")
    print("   docs/standards/logic-hygiene-patterns.md")
    print("----------------------------------------------------------------------")

    if args.strict:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
