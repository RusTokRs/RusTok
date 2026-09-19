#!/usr/bin/env python3
"""RusToK Remediation Gatekeeper.

Enforces AGENTS.md §14 (No stubs, fake completion, or suppression)
and validates git diffs before auto-remediation commits are accepted.
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

# Ensure UTF-8 output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent.parent

# Disallowed patterns that must NEVER be added by an auto-remediation agent
DISALLOWED_ADDITIONS = [
    (r"^\+.*#\[allow\(dead_code\)\]", "Added #[allow(dead_code)] is prohibited by AGENTS.md §14"),
    (r"^\+.*#\[allow\(unused\)\]", "Added #[allow(unused)] is prohibited by AGENTS.md §14"),
    (r"^\+.*#\[allow\(unused_variables\)\]", "Added #[allow(unused_variables)] is prohibited by AGENTS.md §14"),
    (r"^\+.*@ts-ignore", "Added @ts-ignore is prohibited by AGENTS.md §14"),
    (r"^\+.*@ts-expect-error", "Added @ts-expect-error is prohibited by AGENTS.md §14"),
    (r"^\+.*todo!\(", "Added todo!() is prohibited by AGENTS.md §14"),
    (r"^\+.*unimplemented!\(", "Added unimplemented!() is prohibited by AGENTS.md §14"),
    (r"^\+.*include!\(\"[^\"]+\.rs\"\)", "Added include!(\"*.rs\") file stitching is prohibited (use canonical Rust modules)"),
]


def get_git_diff(staged_only: bool = False) -> str:
    cmd = ["git", "diff"]
    if staged_only:
        cmd.append("--cached")
    try:
        res = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)
        return res.stdout or ""
    except Exception as e:
        print(f"Error running git diff: {e}", file=sys.stderr)
        return ""


def get_changed_files(staged_only: bool = False) -> list[str]:
    cmd = ["git", "diff", "--name-only"]
    if staged_only:
        cmd.append("--cached")
    try:
        res = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace", check=True)
        return [line.strip() for line in (res.stdout or "").splitlines() if line.strip()]
    except Exception:
        return []


def check_diff_invariants(diff_text: str) -> list[str]:
    violations: list[str] = []
    lines = diff_text.splitlines()

    deleted_tests = 0
    added_tests = 0
    for line in lines:
        for pattern, reason in DISALLOWED_ADDITIONS:
            if re.search(pattern, line):
                violations.append(f"Forbidden diff line: '{line.strip()}' -> {reason}")

        # Guard against deleted tests
        if line.startswith("-") and not line.startswith("---"):
            if "#[test]" in line or "#[tokio::test]" in line:
                deleted_tests += 1
        if line.startswith("+") and not line.startswith("+++"):
            if "#[test]" in line or "#[tokio::test]" in line:
                added_tests += 1

    if deleted_tests > added_tests:
        violations.append(
            f"Forbidden test deletion: {deleted_tests} test attribute(s) removed vs {added_tests} added -> AGENTS.md §14 forbids deleting tests"
        )

    return violations


def run_architecture_guard() -> tuple[bool, str]:
    script = ROOT / "scripts" / "architecture_dependency_guard.py"
    if not script.exists():
        return True, "Architecture guard script not found, skipping."
    
    res = subprocess.run([sys.executable, str(script)], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if res.returncode != 0:
        return False, f"Architecture dependency guard failed:\n{res.stdout}\n{res.stderr}"
    return True, "Architecture dependency guard passed."


def run_crate_checks(changed_files: list[str], target_crate: str | None = None) -> tuple[bool, str]:
    packages_to_check: set[str] = set()
    if target_crate:
        packages_to_check.add(target_crate)
    else:
        for f in changed_files:
            p = ROOT / f
            for parent in p.parents:
                cargo_toml = parent / "Cargo.toml"
                if cargo_toml.exists() and parent != ROOT:
                    try:
                        content = cargo_toml.read_text(encoding="utf-8")
                        m = re.search(r'^\s*name\s*=\s*"([^"]+)"', content, re.MULTILINE)
                        if m:
                            packages_to_check.add(m.group(1))
                        else:
                            packages_to_check.add(parent.name)
                    except Exception:
                        packages_to_check.add(parent.name)
                    break

    if not packages_to_check:
        return True, "No specific Rust packages affected by diff."

    for pkg in sorted(packages_to_check):
        print(f"[Gatekeeper] Running cargo clippy on package: {pkg}", flush=True)
        clippy_res = subprocess.run(
            ["cargo", "clippy", "--package", pkg, "--all-targets", "--", "-D", "warnings"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if clippy_res.returncode != 0:
            return False, f"cargo clippy failed for {pkg}:\n{clippy_res.stderr or clippy_res.stdout}"

    return True, f"Cargo checks passed for packages: {', '.join(packages_to_check)}"


def main() -> int:
    parser = argparse.ArgumentParser(description="RusToK Auto-Remediation Gatekeeper")
    parser.add_argument("--staged", action="store_true", help="Inspect only staged git changes")
    parser.add_argument("--files", nargs="*", help="Specific files to inspect instead of whole git diff")
    parser.add_argument("--crate", type=str, help="Specific cargo package to run clippy on")
    parser.add_argument("--skip-cargo", action="store_true", help="Skip cargo clippy/test checks")
    args = parser.parse_args()

    print("[Gatekeeper] Inspecting changes for AGENTS.md compliance...", flush=True)
    
    if args.files:
        changed_files = args.files
        # Get diff for specific files
        cmd = ["git", "diff", "--"] + args.files
        res = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
        diff_text = res.stdout or ""
    else:
        diff_text = get_git_diff(staged_only=args.staged)
        changed_files = get_changed_files(staged_only=args.staged)

    if not diff_text.strip():
        print("[Gatekeeper] Diff is clean. Nothing to gate.", flush=True)
        return 0

    violations = check_diff_invariants(diff_text)
    if violations:
        print("\n[Gatekeeper] [FAIL] GATE REJECTED: Disallowed changes detected in diff:", file=sys.stderr, flush=True)
        for v in violations:
            print(f"  - {v}", file=sys.stderr, flush=True)
        return 1

    print("[Gatekeeper] [OK] Diff invariants passed (no illegal suppressions or deleted tests).", flush=True)

    # Architecture dependency guard
    arch_ok, arch_msg = run_architecture_guard()
    if not arch_ok:
        print(f"\n[Gatekeeper] [FAIL] GATE REJECTED: {arch_msg}", file=sys.stderr, flush=True)
        return 1
    print(f"[Gatekeeper] [OK] {arch_msg}", flush=True)

    # Crate clippy checks
    if not args.skip_cargo:
        cargo_ok, cargo_msg = run_crate_checks(changed_files, target_crate=args.crate)
        if not cargo_ok:
            print(f"\n[Gatekeeper] [FAIL] GATE REJECTED: {cargo_msg}", file=sys.stderr, flush=True)
            return 1
        print(f"[Gatekeeper] [OK] {cargo_msg}", flush=True)

    print("\n[Gatekeeper] [OK] ALL GATES PASSED. Change is approved.", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
