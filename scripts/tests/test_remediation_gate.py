#!/usr/bin/env python3
"""Unit tests for verify-remediation-gate.py invariant checks."""
import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
GATE_SCRIPT = ROOT / "scripts" / "verify" / "verify-remediation-gate.py"

spec = importlib.util.spec_from_file_location("gate", GATE_SCRIPT)
if spec is None or spec.loader is None:
    print("Failed to load gate module", file=sys.stderr)
    sys.exit(1)

gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


def file_diff(path: str, lines: str) -> str:
    return f"diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n{lines}\n"


def test_clean_diff():
    diff = file_diff("src/lib.rs", "+ let total = calculate_sum(items);\n+ return Ok(total);")
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 0, f"Expected 0 violations, got {violations}"


def test_forbidden_allow():
    diff = file_diff("src/lib.rs", "+ #[allow(unused)]\n+ fn legacy_helper() {}")
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "AGENTS.md §14" in violations[0]


def test_forbidden_ts_ignore():
    diff = file_diff("src/service.ts", "+ // @ts-ignore: bypass type error\n+ const val = obj.unknownField;")
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "AGENTS.md §14" in violations[0]


def test_forbidden_todo():
    diff = file_diff("src/lib.rs", '+ todo!("finish this later");')
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "AGENTS.md §14" in violations[0]


def test_forbidden_test_deletion():
    diff = file_diff("src/lib.rs", "- #[test]\n- fn test_important_invariants() {\n-     assert!(true);\n- }")
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "forbids deleting tests" in violations[0]


def test_forbidden_include_macro():
    diff = file_diff("src/lib.rs", '+ include!("category_helper.rs");')
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "file stitching is prohibited" in violations[0]


def test_examples_are_not_source_changes_in_a_mixed_diff():
    diff = (
        file_diff("src/lib.rs", '+ include!("category_helper.rs");')
        + file_diff("scripts/fixture.test.py", '- "#[test]\\n"\n+ \'include!("fixture.rs");\'')
        + file_diff("docs/guide.md", '+ `todo!()` and `@ts-ignore` are prohibited.')
        + file_diff("src/service.ts", "+ // @ts-expect-error")
    )
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 2
    assert "file stitching is prohibited" in violations[0]
    assert "Added @ts-expect-error" in violations[1]


if __name__ == "__main__":
    test_clean_diff()
    test_forbidden_allow()
    test_forbidden_ts_ignore()
    test_forbidden_todo()
    test_forbidden_test_deletion()
    test_forbidden_include_macro()
    test_examples_are_not_source_changes_in_a_mixed_diff()
    print("[OK] All verify-remediation-gate invariant tests passed successfully!")

