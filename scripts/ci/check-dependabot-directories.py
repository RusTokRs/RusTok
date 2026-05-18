#!/usr/bin/env python3
"""Validate that every Dependabot update directory exists in the repository."""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".github" / "dependabot.yml"
DIRECTORY_RE = re.compile(r'^\s*directory:\s*["\']?([^"\'\s#]+)')


def main() -> int:
    missing: list[str] = []
    for line in CONFIG.read_text(encoding="utf-8").splitlines():
        match = DIRECTORY_RE.match(line)
        if not match:
            continue
        directory = match.group(1)
        path = ROOT / directory.lstrip("/")
        if not path.is_dir():
            missing.append(directory)

    if missing:
        print("Dependabot directories do not exist:", file=sys.stderr)
        for directory in missing:
            print(f"  - {directory}", file=sys.stderr)
        return 1

    print("All Dependabot update directories exist.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
