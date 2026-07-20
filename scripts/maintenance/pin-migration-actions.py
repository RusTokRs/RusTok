#!/usr/bin/env python3
from pathlib import Path


CHECKOUT = "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0"
SETUP_NODE = "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38"
RUST_TOOLCHAIN = "dtolnay/rust-toolchain@2c7215f132e9ebf062739d9130488b56d53c060c"
UPLOAD_ARTIFACT = "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
DOWNLOAD_ARTIFACT = "actions/download-artifact@634f93cb2916e3fdff6788551b99b062d0335ce0"


def replace_exact(path: Path, old: str, new: str, expected: int = 1) -> None:
    source = path.read_text()
    actual = source.count(old)
    if actual != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s) of {old!r}, found {actual}"
        )
    path.write_text(source.replace(old, new))


def patch_workflow() -> None:
    workflow = Path(".github/workflows/migration-compatibility.yml")
    replace_exact(workflow, "actions/checkout@v7", CHECKOUT, 7)
    replace_exact(workflow, "actions/setup-node@v6", SETUP_NODE, 2)
    replace_exact(workflow, "actions/upload-artifact@v7", UPLOAD_ARTIFACT)
    replace_exact(workflow, "actions/download-artifact@v5", DOWNLOAD_ARTIFACT)
    replace_exact(
        workflow,
        "      - uses: dtolnay/rust-toolchain@stable\n",
        (
            f"      - uses: {RUST_TOOLCHAIN}\n"
            "        with:\n"
            "          toolchain: 1.96.0\n"
        ),
        3,
    )


def pin_contract_block() -> str:
    return f'''const PINNED_MIGRATION_ACTIONS = Object.freeze({{
  checkout: "{CHECKOUT}",
  setupNode: "{SETUP_NODE}",
  rustToolchain: "{RUST_TOOLCHAIN}",
  uploadArtifact: "{UPLOAD_ARTIFACT}",
  downloadArtifact: "{DOWNLOAD_ARTIFACT}",
}});

function migrationActionReferences(relativePath) {{
  if (!requireFile(relativePath)) return [];
  return [...read(relativePath).matchAll(/^\\s*uses:\\s*([^\\s#]+)(?:\\s+#.*)?\\s*$/gm)].map(
    (match) => match[1],
  );
}}

function requirePinnedMigrationActions(relativePath, expectedCounts) {{
  const references = migrationActionReferences(relativePath);
  const approved = new Set(Object.values(PINNED_MIGRATION_ACTIONS));
  for (const reference of references) {{
    if (!/^[A-Za-z0-9_.-]+\\/[A-Za-z0-9_.-]+@[0-9a-f]{{40}}$/.test(reference)) {{
      failures.push(`${{relativePath}}: action must be pinned to a full lowercase commit SHA: ${{reference}}`);
    }}
    if (!approved.has(reference)) {{
      failures.push(`${{relativePath}}: unapproved action reference ${{reference}}`);
    }}
  }}
  for (const [reference, expected] of expectedCounts) {{
    const actual = references.filter((candidate) => candidate === reference).length;
    if (actual !== expected) {{
      failures.push(`${{relativePath}}: expected ${{expected}} use(s) of ${{reference}}, found ${{actual}}`);
    }}
  }}
}}

requirePinnedMigrationActions(
  workflow,
  new Map([
    [PINNED_MIGRATION_ACTIONS.checkout, 7],
    [PINNED_MIGRATION_ACTIONS.setupNode, 2],
    [PINNED_MIGRATION_ACTIONS.rustToolchain, 3],
    [PINNED_MIGRATION_ACTIONS.uploadArtifact, 1],
    [PINNED_MIGRATION_ACTIONS.downloadArtifact, 1],
  ]),
);
requirePinnedMigrationActions(
  approvalWorkflow,
  new Map([
    [PINNED_MIGRATION_ACTIONS.checkout, 2],
    [PINNED_MIGRATION_ACTIONS.setupNode, 1],
  ]),
);
forbidFile(".github/workflows/one-off-pin-migration-actions.yml");
forbidFile(".github/workflows/one-off-migration-pin-status.yml");
forbidFile("scripts/maintenance/pin-migration-actions.py");

'''


def patch_verifier() -> None:
    verifier = Path("scripts/verify/verify-migration-compatibility-contract.mjs")
    replace_exact(
        verifier,
        '  "actions/upload-artifact@v7",\n',
        f'  "{UPLOAD_ARTIFACT}",\n',
    )
    replace_exact(
        verifier,
        '  "actions/download-artifact@v5",\n',
        f'  "{DOWNLOAD_ARTIFACT}",\n',
    )
    marker = 'forbidFile(".github/workflows/one-off-wire-migration-backfill-workflow.yml");\n'
    replace_exact(verifier, marker, pin_contract_block() + marker)


def remove_temporary_files() -> None:
    for relative_path in [
        ".github/workflows/one-off-pin-migration-actions.yml",
        ".github/workflows/one-off-migration-pin-status.yml",
        "scripts/maintenance/pin-migration-actions.py",
    ]:
        path = Path(relative_path)
        if path.exists():
            path.unlink()


def main() -> None:
    patch_workflow()
    patch_verifier()
    remove_temporary_files()


if __name__ == "__main__":
    main()
