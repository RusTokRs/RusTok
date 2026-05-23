#!/usr/bin/env python3
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
checks = [
    (
        ROOT / "apps/server/docs/README.md",
        "[Runbook retry/compensation lifecycle hook failures](./module-lifecycle-retry-compensation-runbook.md)",
    ),
    (
        ROOT / "docs/index.md",
        "[Server runbook: retry/compensation lifecycle hook failures](../apps/server/docs/module-lifecycle-retry-compensation-runbook.md)",
    ),
]

missing = []
for path, marker in checks:
    content = path.read_text(encoding="utf-8")
    if marker not in content:
        missing.append((path, marker))

if missing:
    for path, marker in missing:
        print(f"missing marker in {path}: {marker}")
    sys.exit(1)

print("lifecycle runbook doc links check passed")
