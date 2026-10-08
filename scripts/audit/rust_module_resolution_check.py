#!/usr/bin/env python3
"""Module-declaration resolver for Rust sources, used when no toolchain is available.

Purpose: `mod foo;`, `#[path = "..."] mod foo;`, `include!`, `include_str!` and `include_bytes!`
all name files by string. A target that does not exist is E0583 / E0584 at `cargo check` time —
a hard build failure that needs no type information to find, which makes it the one whole class of
compile error that can be caught statically. This checker exists for the audit sandbox, where
`cargo`/`rustc` are absent; it is NOT a substitute for the crate's canonical checks.

What it does not do: it does not resolve `use` paths, macro-generated modules, or modules produced
by build scripts. `mod tests { ... }` inline blocks are skipped, and a declaration carrying a
`#[path]` attribute is resolved through that attribute only.

Usage: python3 scripts/audit/rust_module_resolution_check.py <crate-dir> [...]
Exit code: 0 when every declaration resolves, 1 when any target is missing.
"""

import re
import sys
from pathlib import Path

MOD_DECL = re.compile(
    r'^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*(;|\{)', re.M
)
PATH_ATTR = re.compile(
    r'#\[path\s*=\s*"([^"]+)"\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;'
)
INCLUDE = re.compile(r'include(?:_str|_bytes)?!\s*\(\s*"([^"]+)"\s*\)')


def check_crate(crate: Path) -> list[str]:
    src = crate / "src"
    if not src.is_dir():
        return [f"{crate}: no src/ directory"]

    problems: list[str] = []
    checked = 0
    for rs in sorted(src.rglob("*.rs")):
        text = rs.read_text(encoding="utf-8", errors="replace")
        parent = rs.parent
        # A `mod` inside `foo.rs` lives under `foo/`; inside `foo/mod.rs`, `lib.rs` or `main.rs`
        # it lives directly in the containing directory.
        base = parent if rs.name in ("lib.rs", "main.rs", "mod.rs") else parent / rs.stem

        attributed: set[str] = set()
        for match in PATH_ATTR.finditer(text):
            checked += 1
            attributed.add(match.group(2))
            if not (parent / match.group(1)).exists():
                problems.append(
                    f"{rs}: #[path] mod {match.group(2)} -> missing {parent / match.group(1)}"
                )

        for match in MOD_DECL.finditer(text):
            if match.group(2) == "{" or match.group(1) in attributed:
                continue
            checked += 1
            name = match.group(1)
            candidates = (base / f"{name}.rs", base / name / "mod.rs")
            if not any(candidate.exists() for candidate in candidates):
                problems.append(
                    f"{rs}: mod {name}; -> no file at "
                    + " or ".join(str(candidate) for candidate in candidates)
                )

        for match in INCLUDE.finditer(text):
            checked += 1
            if not (parent / match.group(1)).exists():
                problems.append(f"{rs}: include!(\"{match.group(1)}\") -> missing target")

    print(f"{crate}: {checked} module/include declaration(s) checked")
    return problems


def main(argv: list[str]) -> int:
    targets = [Path(arg) for arg in argv[1:]] or [Path(".")]
    problems: list[str] = []
    for target in targets:
        problems.extend(check_crate(target))
    if problems:
        print("\nunresolved declarations:")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print("all declarations resolve to existing files")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
