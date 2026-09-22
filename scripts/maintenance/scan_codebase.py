#!/usr/bin/env python3
"""RusToK Continuous Code Review Scanner.

Scans the codebase according to scripts/maintenance/review_rules.toml
and outputs a prioritized findings queue (.review_backlog.json).
"""
from __future__ import annotations

import argparse
import fnmatch
import json
import os
import re
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

ROOT = Path(__file__).resolve().parent.parent.parent
DEFAULT_RULES_PATH = ROOT / "scripts" / "maintenance" / "review_rules.toml"
DEFAULT_OUTPUT_PATH = ROOT / ".review_backlog.json"


@dataclass
class Finding:
    id: str
    rule_id: str
    tier: int
    severity: str
    file: str
    line: int
    snippet: str
    remediation: str
    reference: str


def load_rules(rules_path: Path) -> list[dict]:
    with rules_path.open("rb") as f:
        data = tomllib.load(f)
    return data.get("rules", [])


def matches_globs(relative_path: str, globs: list[str]) -> bool:
    norm_path = relative_path.replace("\\", "/")
    for glob in globs:
        norm_glob = glob.replace("\\", "/")
        if fnmatch.fnmatch(norm_path, norm_glob) or fnmatch.fnmatch(norm_path, f"*/{norm_glob}"):
            return True
        # Handle **/ prefix
        if "**" in norm_glob:
            parts = norm_glob.split("**/")
            if len(parts) == 2 and (norm_path.startswith(parts[0]) or not parts[0]) and fnmatch.fnmatch(norm_path, f"*{parts[1]}"):
                return True
    return False


def is_file_matching(relative_path: str, target_globs: list[str], exclude_globs: list[str]) -> bool:
    norm_path = relative_path.replace("\\", "/")
    
    # Check exclude first
    for ex in exclude_globs:
        norm_ex = ex.replace("\\", "/")
        if fnmatch.fnmatch(norm_path, norm_ex) or norm_ex.strip("*") in norm_path:
            return False

    # Check target
    for tg in target_globs:
        norm_tg = tg.replace("\\", "/")
        if fnmatch.fnmatch(norm_path, norm_tg):
            return True
        if "**" in norm_tg:
            prefix, suffix = norm_tg.split("**", 1)
            prefix = prefix.rstrip("/")
            suffix = suffix.lstrip("/")
            if norm_path.startswith(prefix) and (not suffix or norm_path.endswith(suffix) or fnmatch.fnmatch(norm_path, f"*{suffix}")):
                return True
    return False


def find_test_line_ranges(lines: list[str]) -> set[int]:
    test_lines: set[int] = set()
    in_mod_test = False
    has_seen_mod_open = False
    mod_test_depth = 0

    in_fn_test = False
    has_seen_fn_open = False
    fn_test_depth = 0

    for idx, line in enumerate(lines, start=1):
        stripped = line.strip()

        # Check for module-level test configuration (e.g. #[cfg(test)] or #[cfg(all(test, ...))])
        if re.search(r"#\[cfg\(.*?\btest\b.*?\)\]", stripped):
            in_mod_test = True
            has_seen_mod_open = False
            mod_test_depth = 0
            test_lines.add(idx)
            continue

        if in_mod_test:
            test_lines.add(idx)
            if "{" in stripped:
                has_seen_mod_open = True
            mod_test_depth += stripped.count("{") - stripped.count("}")
            if has_seen_mod_open and mod_test_depth <= 0:
                in_mod_test = False
            continue

        # Check for standalone #[test] or #[tokio::test] outside mod tests
        if "#[test]" in stripped or "#[tokio::test]" in stripped:
            in_fn_test = True
            has_seen_fn_open = False
            fn_test_depth = 0
            test_lines.add(idx)
            continue

        if in_fn_test:
            test_lines.add(idx)
            if "{" in stripped:
                has_seen_fn_open = True
            fn_test_depth += stripped.count("{") - stripped.count("}")
            if has_seen_fn_open and fn_test_depth <= 0:
                in_fn_test = False
            continue

    return test_lines


def check_regex_pattern(file_path: Path, rule: dict, rel_path: str) -> list[Finding]:
    findings: list[Finding] = []
    pattern_str = rule.get("pattern")
    if not pattern_str:
        return findings

    try:
        rx = re.compile(pattern_str)
    except re.error as err:
        print(f"Warning: Invalid regex in rule {rule['id']}: {err}", file=sys.stderr)
        return findings

    lines = file_path.read_text(encoding="utf-8", errors="replace").splitlines()
    test_lines = find_test_line_ranges(lines) if file_path.suffix == ".rs" else set()

    for idx, line in enumerate(lines, start=1):
        stripped = line.strip()
        if rx.search(line):
            # Skip comments for runtime code rules
            if rule["id"] != "SLOP-COMMENT-01" and (
                stripped.startswith("//") or stripped.startswith("/*") or stripped.startswith("*")
            ):
                continue

            # If the rule is runtime-only, skip lines inside test blocks
            if rule["id"] in (
                "REL-UNWRAP-01",
                "ERR-ANYHOW-01",
                "LOG-RAW-PRINT-01",
                "EVENT-OUTBOX-01",
                "ASYNC-BLOCKING-IO-01",
            ):
                if idx in test_lines:
                    continue

            # Special check for REL-UNWRAP-01: inspect immediately preceding comments for "// INVARIANT:"
            if rule["id"] == "REL-UNWRAP-01":
                has_invariant = False
                in_method_chain = line.strip().startswith(".")
                for prev_idx in range(idx - 2, max(-1, idx - 8), -1):
                    prev_line = lines[prev_idx].strip()
                    if not prev_line:
                        continue
                    if "INVARIANT:" in prev_line or "physically guaranteed:" in prev_line:
                        has_invariant = True
                        break
                    if in_method_chain:
                        # Inside a multi-line chained call, traverse up to the comment preceding the expression
                        if prev_line.endswith(";") or prev_line.endswith("}"):
                            break
                        continue
                    if not prev_line.startswith("//"):
                        break
                if has_invariant:
                    continue

            # Strip leading/trailing whitespace for snippet
            snippet = line.strip()
            findings.append(
                Finding(
                    id=f"{rule['id']}-{rel_path.replace('/', '_')}-{idx}",
                    rule_id=rule["id"],
                    tier=rule["tier"],
                    severity=rule["severity"],
                    file=rel_path,
                    line=idx,
                    snippet=snippet[:120],
                    remediation=rule["remediation"],
                    reference=rule["reference"],
                )
            )
    return findings


def check_heuristics(file_path: Path, rule: dict, rel_path: str) -> list[Finding]:
    findings: list[Finding] = []
    heuristic = rule.get("heuristic")
    if not heuristic:
        return findings

    content = file_path.read_text(encoding="utf-8", errors="replace")
    lines = content.splitlines()
    test_lines = find_test_line_ranges(lines) if file_path.suffix == ".rs" else set()

    if heuristic == "file_length_exceeded":
        max_lines = rule.get("max_lines", 1000)
        if len(lines) > max_lines:
            findings.append(
                Finding(
                    id=f"{rule['id']}-{rel_path.replace('/', '_')}-1",
                    rule_id=rule["id"],
                    tier=rule["tier"],
                    severity=rule["severity"],
                    file=rel_path,
                    line=1,
                    snippet=f"File has {len(lines)} lines (limit: {max_lines})",
                    remediation=rule["remediation"],
                    reference=rule["reference"],
                )
            )

    elif heuristic == "function_length_exceeded":
        max_lines = rule.get("max_lines", 40)
        in_fn = False
        fn_name = ""
        fn_start = 0
        brace_depth = 0
        has_seen_open = False
        fn_rx = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)")

        for idx, line in enumerate(lines, start=1):
            if idx in test_lines:
                continue
            stripped = line.strip()
            if not in_fn:
                m = fn_rx.match(stripped)
                if m:
                    in_fn = True
                    fn_name = m.group(1)
                    fn_start = idx
                    has_seen_open = "{" in stripped
                    brace_depth = stripped.count("{") - stripped.count("}")
            else:
                if "{" in stripped:
                    has_seen_open = True
                brace_depth += stripped.count("{") - stripped.count("}")
                if has_seen_open and brace_depth <= 0:
                    fn_len = idx - fn_start + 1
                    if fn_len > max_lines:
                        findings.append(
                            Finding(
                                id=f"{rule['id']}-{rel_path.replace('/', '_')}-{fn_start}",
                                rule_id=rule["id"],
                                tier=rule["tier"],
                                severity=rule["severity"],
                                file=rel_path,
                                line=fn_start,
                                snippet=f"fn {fn_name}() has {fn_len} lines (limit: {max_lines})",
                                remediation=rule["remediation"],
                                reference=rule["reference"],
                            )
                        )
                    in_fn = False
                    has_seen_open = False
                    brace_depth = 0

    elif heuristic == "service_missing_instrument":
        # Check pub async fn in service.rs
        for idx, line in enumerate(lines, start=1):
            if idx in test_lines:
                continue
            if re.match(r"^\s*pub\s+async\s+fn\s+([a-zA-Z0-9_]+)", line):
                # Inspect previous 3 lines for #[instrument]
                has_instrument = False
                for prev_idx in range(max(0, idx - 4), idx - 1):
                    if "#[instrument" in lines[prev_idx] or "#[tracing::instrument" in lines[prev_idx]:
                        has_instrument = True
                        break
                if not has_instrument:
                    findings.append(
                        Finding(
                            id=f"{rule['id']}-{rel_path.replace('/', '_')}-{idx}",
                            rule_id=rule["id"],
                            tier=rule["tier"],
                            severity=rule["severity"],
                            file=rel_path,
                            line=idx,
                            snippet=line.strip()[:120],
                            remediation=rule["remediation"],
                            reference=rule["reference"],
                        )
                    )

    elif heuristic == "tenant_filter_missing":
        # Look for SeaORM calls like Entity::find().all(&db) or find().one(&db) without TenantId filter
        find_call_rx = re.compile(r"(::find\(\)|::find_by_id\()", re.MULTILINE)
        for match in find_call_rx.finditer(content):
            start_pos = match.start()
            line_idx = content[:start_pos].count("\n") + 1
            if line_idx in test_lines:
                continue
            # Check the statement block up to the semicolon
            semicolon_pos = content.find(";", start_pos)
            if semicolon_pos == -1:
                semicolon_pos = start_pos + 300
            stmt = content[start_pos:semicolon_pos]
            if "tenant" not in stmt.lower():
                findings.append(
                    Finding(
                        id=f"{rule['id']}-{rel_path.replace('/', '_')}-{line_idx}",
                        rule_id=rule["id"],
                        tier=rule["tier"],
                        severity=rule["severity"],
                        file=rel_path,
                        line=line_idx,
                        snippet=stmt.strip().replace("\n", " ")[:120],
                        remediation=rule["remediation"],
                        reference=rule["reference"],
                    )
                )

    return findings


def scan(
    root: Path,
    rules: list[dict],
    target_path: Path | None = None,
    rule_id_filter: str | None = None,
    tier_filter: int | None = None,
    max_findings: int = 100,
) -> list[Finding]:
    all_findings: list[Finding] = []

    if target_path:
        target_path = target_path if target_path.is_absolute() else (root / target_path).resolve()

    search_root = target_path if target_path and target_path.is_dir() else root
    candidate_files: list[Path] = []

    IGNORED_DIRS = {"target", ".git", "node_modules", "dist", ".next", "build", "coverage", ".gemini"}

    if target_path and target_path.is_file():
        candidate_files = [target_path]
    else:
        for root_dir, dirs, files in os.walk(search_root):
            # Prune ignored directories in-place to avoid descending into target/ or node_modules/
            dirs[:] = [d for d in dirs if d not in IGNORED_DIRS]
            for file_name in files:
                ext = os.path.splitext(file_name)[1]
                if ext in (".rs", ".ts", ".tsx"):
                    candidate_files.append(Path(root_dir) / file_name)

    for rule in rules:
        if rule_id_filter and rule["id"] != rule_id_filter:
            continue
        if tier_filter is not None and rule["tier"] != tier_filter:
            continue

        target_globs = rule.get("target_globs", [])
        exclude_globs = rule.get("exclude_globs", [])

        for file_path in candidate_files:
            rel_str = str(file_path.relative_to(root)).replace("\\", "/")
            if not is_file_matching(rel_str, target_globs, exclude_globs):
                continue

            # Run pattern check
            if "pattern" in rule:
                findings = check_regex_pattern(file_path, rule, rel_str)
                all_findings.extend(findings)

            # Run heuristic check
            if "heuristic" in rule:
                findings = check_heuristics(file_path, rule, rel_str)
                all_findings.extend(findings)

            if len(all_findings) >= max_findings:
                return all_findings[:max_findings]

    return all_findings[:max_findings]


def main() -> int:
    parser = argparse.ArgumentParser(description="RusToK Continuous Review Scanner")
    parser.add_argument("--rules", type=Path, default=DEFAULT_RULES_PATH, help="Path to review_rules.toml")
    parser.add_argument("--target", type=Path, default=None, help="Target directory or file to scan")
    parser.add_argument("--rule", type=str, default=None, help="Scan only for specific rule ID (e.g. REL-UNWRAP-01)")
    parser.add_argument("--tier", type=int, default=None, help="Scan only rules of specific tier (0, 1, 2)")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT_PATH, help="Output JSON path")
    parser.add_argument("--max-findings", type=int, default=100, help="Maximum findings to collect")
    parser.add_argument("--summary", action="store_true", help="Print summary table to stdout")
    args = parser.parse_args()

    if not args.rules.exists():
        print(f"Error: Rules file not found at {args.rules}", file=sys.stderr)
        return 1

    rules = load_rules(args.rules)
    findings = scan(
        root=ROOT,
        rules=rules,
        target_path=args.target,
        rule_id_filter=args.rule,
        tier_filter=args.tier,
        max_findings=args.max_findings,
    )

    by_tier: dict[int, int] = {}
    by_rule: dict[str, int] = {}
    for f in findings:
        by_tier[f.tier] = by_tier.get(f.tier, 0) + 1
        by_rule[f.rule_id] = by_rule.get(f.rule_id, 0) + 1

    payload = {
        "scanned_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "total_findings": len(findings),
        "by_tier": by_tier,
        "by_rule": by_rule,
        "findings": [asdict(f) for f in findings],
    }

    with args.output.open("w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    print(f"[ACRE Scanner] Scanned codebase. Found {len(findings)} actionable findings. Saved to {args.output.name}")
    if args.summary or len(findings) > 0:
        print("\nFindings Breakdown by Tier:")
        for tier in sorted(by_tier.keys()):
            tier_name = {0: "Tier 0 (Critical/Panic)", 1: "Tier 1 (Architecture/Events)", 2: "Tier 2 (Cleanliness/Metrics)"}.get(tier, f"Tier {tier}")
            print(f"  - {tier_name}: {by_tier[tier]}")
        print("\nFindings Breakdown by Rule:")
        for r_id, count in sorted(by_rule.items()):
            print(f"  - {r_id}: {count}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
