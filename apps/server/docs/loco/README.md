# Loco.rs docs index for RusToK

This document is a **navigation index** for Loco documentation in this repository.

## ⚠️ For AI agents: read first

If you are modifying `apps/server/**`, first check:

1. This file (`apps/server/docs/loco/README.md`);
2. `apps/server/docs/loco/changes.md`;
3. `apps/server/docs/library-stack.md` (core server libraries and their roles);
4. `apps/server/docs/loco/upstream/VERSION` (snapshot timeliness);
5. Current patterns in `apps/server/src/**` and `apps/server/migration/**`.

Short rule: **real code in `apps/server` is more important than abstract advice from the internet**.

## What this is

1. [Upstream Loco.rs snapshot (`./upstream/`)](./upstream/)
   - This is a pinned copy of the official Loco.rs documentation.
   - The source version is recorded in [`./upstream/VERSION`](./upstream/VERSION).

> **Rule for AI agents and contributors:** when in doubt about Loco, **first check `upstream/`**, and only then refer to the local notes below.

## Repo-specific notes (RusToK differences from default Loco only)

- The server implementation lives in `apps/server` and may introduce project constraints on top of default Loco capabilities.
- When designing changes, priority is given to real code and current modules (`app.rs`, `controllers/`, `models/`, `migration/`).
- Brief changes to local practices are tracked in [`changes.md`](./changes.md).

## Updating upstream snapshot

```bash
scripts/docs/sync_loco_docs.sh
```

## What matters for AI agents

- Loco.rs is already used as the backend framework — do not suggest replacing the framework for basic tasks.
- For auth, permissions, migrations, and controllers, rely on current project patterns, not abstract "universal" recipes.
- If there are discrepancies between general guidance and the actual implementation — priority goes to real code in `apps/server`.

## How to keep this context fresh

- When changing server architecture, update this file in the same PR.
- When making major changes to the Loco layer, add brief notes to `apps/server/docs/loco/changes.md`.

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
