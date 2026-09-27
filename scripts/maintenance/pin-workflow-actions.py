#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path

PINS = {
    "actions/checkout@v7": "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
    "actions/checkout@v7.0.1": "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
    "actions/setup-node@v6": "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
    "actions/setup-node@v7": "actions/setup-node@820762786026740c76f36085b0efc47a31fe5020",
    "actions/setup-node@v7.0.0": "actions/setup-node@820762786026740c76f36085b0efc47a31fe5020",
    "actions/upload-artifact@v7": "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
    "actions/download-artifact@v7": "actions/download-artifact@37930b1c2abaa49bbe596cd826c3c89aef350131",
    "actions/download-artifact@v8": "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
    "actions/download-artifact@v8.0.1": "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c",
    "dtolnay/rust-toolchain@stable": "dtolnay/rust-toolchain@6bed0761d98439e5a578e2877258200ad565ba87",
    "dtolnay/rust-toolchain@master": "dtolnay/rust-toolchain@02cb101ec7c40f2c49e1d9714d64511d8e1b74de",
    "Swatinem/rust-cache@v2": "Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6",
    "taiki-e/install-action@cargo-audit": "taiki-e/install-action@1ceecae37ba251f3f09906445d948ebfd5c4e06c",
    "taiki-e/install-action@cargo-udeps": "taiki-e/install-action@38912a20a471c0a6a727cfdeb44492def5985f0c",
    "taiki-e/install-action@cargo-llvm-cov": "taiki-e/install-action@6db3a280da9eee4283c045c7431ab5116f1a0089",
    "taiki-e/install-action@nextest": "taiki-e/install-action@23d41aa71228a2692ee31bb433f070fd4bdc6661",
    "EmbarkStudios/cargo-deny-action@v2": "EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25",
    "crate-ci/typos@master": "crate-ci/typos@b41eb62e9a8096c9ba6636e2365d3f56018a203c",
    "anchore/sbom-action@v0": "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610",
}

USES_RE = re.compile(r'^(\s*-?\s*uses:\s*)([^\s#]+)(.*)$')
SHA_RE = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[0-9a-f]{40}$")


def main() -> None:
    changes: dict[str, int] = {}
    unknown: list[str] = []
    replacements: list[str] = []

    for path in sorted(Path(".github/workflows").rglob("*.y*ml")):
        source = path.read_text()
        lines = source.splitlines(keepends=True)
        changed = False

        for index, line in enumerate(lines, start=1):
            match = USES_RE.match(line.rstrip("\n"))
            if not match:
                continue

            reference = match.group(2)
            if reference.startswith("./") or reference.startswith("docker://"):
                continue
            if SHA_RE.fullmatch(reference):
                continue

            replacement = PINS.get(reference)
            if replacement is None:
                unknown.append(f"{path}:{index}: {reference}")
                continue

            prefix, suffix = match.group(1), match.group(3)
            newline = "\n" if line.endswith("\n") else ""
            lines[index - 1] = f"{prefix}{replacement}{suffix}{newline}"
            changed = True
            replacements.append(f"{path}:{index}: {reference} -> {replacement}")

        if changed:
            path.write_text("".join(lines))
            changes[str(path)] = sum(1 for line in lines if USES_RE.match(line.rstrip("\n")))

    if unknown:
        print("UNKNOWN ACTION REFERENCES (migration refused):")
        print("\n".join(unknown))
        raise SystemExit(2)

    print(f"Pinned workflow files: {len(changes)}")
    print("\n".join(replacements) if replacements else "No mutable action references found.")


if __name__ == "__main__":
    main()
