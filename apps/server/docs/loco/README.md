# Archived Loco.rs Documentation Index

Status: archived. This document is a navigation index for historical Loco
documentation retained for audit context only.

## ⚠️ For AI agents: read first

If you are modifying `apps/server/**`, first check:

1. This file (`apps/server/docs/loco/README.md`);
2. `apps/server/docs/loco/changes.md`;
3. `apps/server/docs/library-stack.md` (core server libraries and their roles);
4. `apps/server/docs/loco/upstream/VERSION` (snapshot timeliness);
5. Current patterns in `apps/server/src/**` and `crates/rustok-migrations/**`.

Short rule: **real code in `apps/server` is more important than abstract advice from the internet**.

## What this is

1. [Upstream Loco.rs snapshot (`./upstream/`)](./upstream/)
   - This is a pinned copy of the official Loco.rs documentation.
   - The source version is recorded in [`./upstream/VERSION`](./upstream/VERSION).

> **Rule for AI agents and contributors:** do not use `upstream/` or the local
> notes below to guide implementation. The active server architecture is Axum.

## Repo-specific notes (RusToK differences from default Loco only)

- The historical server implementation lived in `apps/server`; the active host
  is documented in `apps/server/docs/README.md`.
- When designing changes, priority is given to real code and current modules (`app.rs`, `controllers/`, `models/`, `migration/`).
- Brief changes to local practices are tracked in [`changes.md`](./changes.md).

## Updating upstream snapshot

```bash
scripts/docs/sync_loco_docs.sh
```

## What matters for AI agents

- Loco.rs is no longer used as the backend framework.
- For auth, permissions, migrations, and controllers, rely on current project patterns, not abstract "universal" recipes.
- If there are discrepancies between general guidance and the actual implementation — priority goes to real code in `apps/server`.

## How to keep this context fresh

- When changing server architecture, update this file in the same PR.
- Do not add new implementation notes to this archived directory.

## Upstream snapshot freshness

`apps/server/docs/loco/upstream/VERSION` stores snapshot metadata for upstream Loco references.

- `make docs-check-loco` validates that metadata exists and enforces freshness policy:
  - `>30` days old: CI warning;
  - `>60` days old: CI failure.
- `make docs-sync-loco` refreshes snapshot metadata date before opening a PR.

## How to remove Loco documentation and automation (if the temporary measure is no longer needed)

Remove everything in one PR to avoid leaving broken CI checks:

1. Delete the documentation folder:
   - `apps/server/docs/loco/` (including `upstream/VERSION`).
2. Delete the automation script:
   - `scripts/loco_upstream_snapshot.py`.
3. Delete the make targets:
   - `docs-sync-loco` and `docs-check-loco` from `Makefile`.
4. Delete the CI job:
   - `loco-docs-snapshot` from `.github/workflows/ci.yml`;
   - remove it from `ci-success.needs` and from the final check condition.
5. Delete the item from the PR template:
   - checkbox about `apps/server/docs/loco/upstream` freshness.

Minimum check after removal:

```bash
cargo check --workspace --all-targets --all-features
```

and verify that the CI workflow passes without `loco-docs-snapshot`.
