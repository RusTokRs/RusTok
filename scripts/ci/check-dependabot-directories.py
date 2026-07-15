#!/usr/bin/env python3
"""Validate that every Dependabot update directory exists in the repository."""
from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".github" / "dependabot.yml"
DIRECTORY_RE = re.compile(r"^\s*directory:\s*(.+?)\s*$")
PACKAGE_ECOSYSTEM_RE = re.compile(r"^\s*-\s*package-ecosystem:\s*(.+?)\s*$")


def parse_directory_value(raw_value: str) -> str:
    value = raw_value.strip()
    if not value:
        return ""
    # Strip YAML inline comments for unquoted scalars.
    if value[0] not in {'"', "'"}:
        value = value.split("#", 1)[0].strip()
        return value

    quote = value[0]
    if len(value) < 2:
        return ""
    closing_index = -1
    escaped = False
    for index in range(1, len(value)):
        ch = value[index]
        if quote == '"' and ch == "\\" and not escaped:
            escaped = True
            continue
        if ch == quote and not escaped:
            closing_index = index
            break
        escaped = False
    if closing_index == -1:
        return ""
    trailing = value[closing_index + 1 :].strip()
    if trailing and not trailing.startswith("#"):
        return ""
    parsed = value[1:closing_index]
    if quote == '"':
        parsed = parsed.replace(r"\\", "\\").replace(r"\"", '"')
    return parsed


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate that every Dependabot update directory exists."
    )
    parser.add_argument(
        "--root",
        type=pathlib.Path,
        default=ROOT,
        help="Repository root directory (default: auto-detected from script location).",
    )
    parser.add_argument(
        "--config",
        type=pathlib.Path,
        default=CONFIG,
        help="Path to dependabot.yml (default: <root>/.github/dependabot.yml).",
    )
    return parser.parse_args()


def normalize_directory(directory: str) -> str:
    trimmed = directory.strip()
    if not trimmed:
        return trimmed
    if not trimmed.startswith("/"):
        trimmed = f"/{trimmed}"
    if trimmed != "/":
        trimmed = trimmed.rstrip("/")
    return trimmed


def is_safe_repo_relative_path(directory: str) -> bool:
    parts = pathlib.PurePosixPath(directory).parts
    return ".." not in parts


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    config = args.config if args.config.is_absolute() else (root / args.config)
    if not config.is_file():
        print(f"Dependabot config file not found: {config}", file=sys.stderr)
        return 1

    missing: list[str] = []
    seen: set[tuple[str, str]] = set()
    duplicates: set[tuple[str, str]] = set()
    invalid: set[str] = set()
    found_directory_entries = 0
    ecosystem = ""
    for line in config.read_text(encoding="utf-8").splitlines():
        ecosystem_match = PACKAGE_ECOSYSTEM_RE.match(line)
        if ecosystem_match:
            ecosystem = parse_directory_value(ecosystem_match.group(1))
            continue
        match = DIRECTORY_RE.match(line)
        if not match:
            continue
        found_directory_entries += 1
        raw_directory = parse_directory_value(match.group(1))
        directory = normalize_directory(raw_directory)
        if not directory or not is_safe_repo_relative_path(directory):
            invalid.add(raw_directory)
            continue
        key = (ecosystem, directory)
        if key in seen:
            duplicates.add(key)
        else:
            seen.add(key)
        path = root / directory.lstrip("/")
        if not path.is_dir():
            missing.append(directory)

    if invalid:
        print("Dependabot directories contain invalid paths:", file=sys.stderr)
        for directory in sorted(invalid):
            print(f"  - {directory}", file=sys.stderr)
        return 1

    if found_directory_entries == 0:
        print(
            "Dependabot config does not contain any directory entries.",
            file=sys.stderr,
        )
        return 1

    if duplicates:
        print("Dependabot directories contain duplicates:", file=sys.stderr)
        for ecosystem, directory in sorted(duplicates):
            print(f"  - {ecosystem}: {directory}", file=sys.stderr)
        return 1

    if missing:
        print("Dependabot directories do not exist:", file=sys.stderr)
        for directory in sorted(set(missing)):
            print(f"  - {directory}", file=sys.stderr)
        return 1

    print("All Dependabot update directories exist.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
