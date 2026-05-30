#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

PLAN_PATH = Path(os.environ.get("RUSTOK_REMEDIATION_PLAN_PATH", "docs/research/control-plane-module-lifecycle-remediation-plan.md"))


def main() -> int:
    if not PLAN_PATH.exists():
        print(f"ERROR: remediation plan not found: {PLAN_PATH}")
        return 1

    text = PLAN_PATH.read_text(encoding="utf-8")
    lines = text.splitlines()
    pending: list[tuple[int, str]] = []
    in_progress: list[tuple[int, str]] = []

    for idx, line in enumerate(lines, start=1):
        if re.search(r"- \[ \]", line):
            pending.append((idx, line.strip()))
        elif re.search(r"- \[~\]", line):
            in_progress.append((idx, line.strip()))

    completed = len(re.findall(r"- \[x\]", text))

    open_items = in_progress + pending
    payload = {
        "source": str(PLAN_PATH),
        "completed": completed,
        "in_progress": len(in_progress),
        "pending": len(pending),
        "open": len(open_items),
        "is_complete": len(open_items) == 0,
        "top_in_progress": [
            {"line": line_no, "item": item} for line_no, item in in_progress[:10]
        ],
        "top_pending": [
            {"line": line_no, "item": item} for line_no, item in pending[:10]
        ],
        "top_open": [
            {"line": line_no, "item": item} for line_no, item in open_items[:10]
        ],
    }

    fail_on_pending = "--fail-on-pending" in sys.argv[1:]
    fail_on_open = "--fail-on-open" in sys.argv[1:]

    if "--json" in sys.argv[1:]:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        if fail_on_open and payload["open"] > 0:
            return 3
        if fail_on_pending and payload["pending"] > 0:
            return 2
        return 0

    print("Control-plane remediation plan progress")
    print(f"source: {PLAN_PATH}")
    print(f"completed: {completed}")
    print(f"in_progress: {len(in_progress)}")
    print(f"pending: {len(pending)}")
    print(f"open: {len(open_items)}")
    print(f"is_complete: {str(payload['is_complete']).lower()}")

    if in_progress:
        print("\nTop in-progress items:")
        for line_no, item in in_progress[:10]:
            print(f"  L{line_no}: {item}")

    if pending:
        print("\nTop pending items:")
        for line_no, item in pending[:10]:
            print(f"  L{line_no}: {item}")

    if fail_on_open and payload["open"] > 0:
        print("\nFAIL: open remediation items detected (--fail-on-open).")
        return 3

    if fail_on_pending and payload["pending"] > 0:
        print("\nFAIL: pending remediation items detected (--fail-on-pending).")
        return 2

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
