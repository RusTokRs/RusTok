#!/usr/bin/env python3
"""Unit tests for scan_codebase.py."""
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
SCANNER_SCRIPT = ROOT / "scripts" / "maintenance" / "scan_codebase.py"

spec = importlib.util.spec_from_file_location("scanner", SCANNER_SCRIPT)
if spec is None or spec.loader is None:
    print("Failed to load scanner module", file=sys.stderr)
    sys.exit(1)

scanner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = scanner
spec.loader.exec_module(scanner)


def test_find_test_line_ranges():
    lines = [
        "pub fn normal_function() {}",
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn it_works() {",
        "        let x = Some(1).unwrap();",
        "    }",
        "}",
        "pub fn another_normal_fn() {}",
    ]
    test_lines = scanner.find_test_line_ranges(lines)
    # Line 1 is normal
    assert 1 not in test_lines
    # Lines 2 to 8 are in test scope
    assert 2 in test_lines
    assert 6 in test_lines
    assert 8 in test_lines
    # Line 9 is normal
    assert 9 not in test_lines


def test_unwrap_with_invariant_comment():
    rule = {
        "id": "REL-UNWRAP-01",
        "tier": 0,
        "severity": "error",
        "pattern": r"(\.unwrap\(\))",
        "remediation": "test remediation",
        "reference": "test reference",
    }
    with tempfile.NamedTemporaryFile("w", suffix=".rs", delete=False) as f:
        f.write(
            """
pub fn safe_logic() {
    // INVARIANT: physically guaranteed by earlier check
    let x = val.unwrap();
    let y = unsafe_val.unwrap();
}
"""
        )
        temp_path = Path(f.name)

    try:
        findings = scanner.check_regex_pattern(temp_path, rule, "temp.rs")
        # x has invariant comment -> skipped.
        # y has no invariant comment -> flagged.
        assert len(findings) == 1
        assert findings[0].line == 5
        assert "unsafe_val.unwrap()" in findings[0].snippet
    finally:
        temp_path.unlink()


if __name__ == "__main__":
    test_find_test_line_ranges()
    test_unwrap_with_invariant_comment()
    print("[OK] All scan_codebase unit tests passed successfully!")
