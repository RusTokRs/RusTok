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


def test_clean_diff():
    diff = "+ let total = calculate_sum(items);\n+ return Ok(total);"
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 0, f"Expected 0 violations, got {violations}"


def test_forbidden_allow():
    diff = "+ #[allow(unused)]\n+ fn legacy_helper() {}"
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "AGENTS.md §14" in violations[0]


def test_forbidden_ts_ignore():
    diff = "+ // @ts-ignore: bypass type error\n+ const val = obj.unknownField;"
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "AGENTS.md §14" in violations[0]


def test_forbidden_todo():
    diff = '+ todo!("finish this later");'
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "AGENTS.md §14" in violations[0]


def test_forbidden_test_deletion():
    diff = "- #[test]\n- fn test_important_invariants() {\n-     assert!(true);\n- }"
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "forbids deleting tests" in violations[0]


def test_forbidden_include_macro():
    diff = '+ include!("category_helper.rs");'
    violations = gate.check_diff_invariants(diff)
    assert len(violations) == 1
    assert "file stitching is prohibited" in violations[0]


if __name__ == "__main__":
    test_clean_diff()
    test_forbidden_allow()
    test_forbidden_ts_ignore()
    test_forbidden_todo()
    test_forbidden_test_deletion()
    test_forbidden_include_macro()
    print("[OK] All verify-remediation-gate invariant tests passed successfully!")

