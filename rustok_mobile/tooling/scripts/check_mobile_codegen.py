#!/usr/bin/env python3
"""Run mobile manifest codegen into temp files and compare tracked outputs."""

from __future__ import annotations

import argparse
import difflib
import pathlib
import subprocess
import sys
import tempfile


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Path to repository root containing crates/*/rustok-module.toml",
    )
    parser.add_argument(
        "--manifest",
        default=(
            "rustok_mobile/apps/rustok_admin_mobile/lib/registry/mobile_manifest.g.dart"
        ),
        help="Path to committed generated Dart manifest",
    )
    parser.add_argument(
        "--snapshot",
        default=("rustok_mobile/tooling/snapshots/mobile_manifest.snapshot.json"),
        help="Path to committed generated registry snapshot JSON",
    )
    parser.add_argument(
        "--surface",
        choices=("admin", "storefront"),
        default="admin",
        help="Manifest surface to verify. Defaults to admin.",
    )
    return parser.parse_args()


def _read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def _print_diff(path: pathlib.Path, current: str, generated: str) -> None:
    print(f"ERROR: generated output differs from committed file: {path}")
    print("Diff (committed -> generated):")
    for line in difflib.unified_diff(
        current.splitlines(),
        generated.splitlines(),
        fromfile=f"{path} (committed)",
        tofile=f"{path} (generated)",
        lineterm="",
    ):
        print(line)


def _run_generator(
    *,
    repo_root: pathlib.Path,
    generated_manifest: pathlib.Path,
    generated_snapshot: pathlib.Path,
    surface: str,
) -> None:
    generator = (
        pathlib.Path(__file__).resolve().with_name("generate_mobile_manifest.py")
    )
    subprocess.run(
        [
            sys.executable,
            str(generator),
            "--repo-root",
            str(repo_root),
            "--output",
            str(generated_manifest),
            "--snapshot-output",
            str(generated_snapshot),
            "--surface",
            surface,
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    manifest_path = pathlib.Path(args.manifest).resolve()
    snapshot_path = pathlib.Path(args.snapshot).resolve()

    if not manifest_path.exists():
        print(f"Manifest file is missing: {manifest_path}")
        return 1
    if not snapshot_path.exists():
        print(f"Snapshot file is missing: {snapshot_path}")
        return 1

    with tempfile.TemporaryDirectory() as tmp:
        tmp_root = pathlib.Path(tmp)
        generated_manifest = tmp_root / "mobile_manifest.g.dart"
        generated_snapshot = tmp_root / "mobile_manifest.snapshot.json"
        try:
            _run_generator(
                repo_root=repo_root,
                generated_manifest=generated_manifest,
                generated_snapshot=generated_snapshot,
                surface=args.surface,
            )
        except subprocess.CalledProcessError as exc:
            print("ERROR: mobile manifest generator failed")
            if exc.stdout:
                print(exc.stdout, end="")
            return exc.returncode or 1

        has_diff = False
        current_manifest = _read(manifest_path)
        generated_manifest_text = _read(generated_manifest)
        if current_manifest != generated_manifest_text:
            _print_diff(manifest_path, current_manifest, generated_manifest_text)
            has_diff = True

        current_snapshot = _read(snapshot_path)
        generated_snapshot_text = _read(generated_snapshot)
        if current_snapshot != generated_snapshot_text:
            _print_diff(snapshot_path, current_snapshot, generated_snapshot_text)
            has_diff = True

    if has_diff:
        print("Run:")
        print(
            "  python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py "
            f"--repo-root {repo_root}"
        )
        return 1

    print("OK: generated mobile manifest outputs match committed files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
