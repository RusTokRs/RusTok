#!/usr/bin/env python3
"""Cross-checks the admin module registry that `apps/admin/build.rs` generates.

Purpose: the build script turns every module manifest with `[provides.admin_ui].leptos_crate`
into generated Rust that names `{leptos_crate}::{PascalSlug}Admin` and registers it. Three of the
invariants behind that generated code are decidable without a compiler, but only two are checked by
the build script itself:

  A. `admin/Cargo.toml` exists if and only if `leptos_crate` is declared (build.rs checks this).
  B. every generated `{leptos_crate}` is a dependency of `apps/admin` (E0433 otherwise).
  C. every generated component name appears in that crate's `src/lib.rs` (E0425/E0433 otherwise).

B and C are the gaps this checker fills: a module that gains an admin UI without being added to
`apps/admin/Cargo.toml`, or whose root component is renamed, breaks `cargo check -p rustok-admin`
with an error that names the generated glue rather than the manifest that is actually wrong.

Usage: python3 scripts/audit/admin_module_registry_check.py [repo-root]
Exit code: 0 when every invariant holds, 1 otherwise.
"""

import re
import sys
import tomllib
from pathlib import Path


def pascal_case(value: str) -> str:
    return "".join(part[:1].upper() + part[1:] for part in re.split(r"[-_]", value) if part)


def load(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def main(argv: list[str]) -> int:
    root = Path(argv[1] if len(argv) > 1 else ".").resolve()
    modules_manifest = root / "modules.toml"
    admin_manifest = root / "apps/admin/Cargo.toml"
    missing = [path for path in (modules_manifest, admin_manifest) if not path.exists()]
    if missing:
        # A wrong root argument should say so, not raise a traceback.
        print("not a repository root (missing manifest): " + ", ".join(str(p) for p in missing))
        return 1
    specs = load(modules_manifest)["modules"]
    admin_deps = set(load(admin_manifest).get("dependencies", {}))

    problems: list[str] = []
    registered = 0
    for spec in sorted(specs.values(), key=lambda value: str(value)):
        if not isinstance(spec, dict) or "path" not in spec:
            continue
        module_root = root / spec["path"]
        manifest_path = module_root / "rustok-module.toml"
        if not manifest_path.exists():
            continue

        manifest = load(manifest_path)
        admin_ui = (manifest.get("provides") or {}).get("admin_ui") or {}
        leptos_crate = (admin_ui.get("leptos_crate") or "").strip() or None
        ui_manifest = module_root / "admin" / "Cargo.toml"

        if ui_manifest.exists() and leptos_crate is None:
            problems.append(
                f"[A] {spec['path']}: admin/Cargo.toml exists but [provides.admin_ui].leptos_crate "
                "is missing (build.rs aborts)"
            )
        if not ui_manifest.exists() and leptos_crate is not None:
            problems.append(
                f"[A] {spec['path']}: leptos_crate is declared but admin/Cargo.toml is missing "
                "(build.rs aborts)"
            )
        if leptos_crate is None:
            continue

        registered += 1
        slug = manifest["module"]["slug"]
        component = f"{pascal_case(slug)}Admin"
        crate_ident = leptos_crate.replace("-", "_")

        if leptos_crate not in admin_deps:
            problems.append(
                f"[B] {slug}: generated code names `{crate_ident}::{component}` but apps/admin "
                f"does not depend on `{leptos_crate}`"
            )

        lib = module_root / "admin" / "src" / "lib.rs"
        if not lib.exists():
            problems.append(f"[C] {slug}: {lib} is missing")
        elif not re.search(rf"\b{re.escape(component)}\b", lib.read_text(encoding="utf-8")):
            problems.append(f"[C] {slug}: `{component}` does not appear in {lib}")

    print(f"{registered} module(s) register an admin UI")
    if problems:
        print("registry problems:")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print("generated admin registry resolves: dependencies present, root components exported")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
