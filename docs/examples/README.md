---
id: doc://docs/examples/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Executable Example Catalog

This section is the single point of discoverability for examples, smoke scenarios, and
reproduction commands used in the platform documentation.

## Purpose

- consolidate scattered "example commands" from random documents;
- provide a single entry point for operators, DevEx, and module owners;
- make examples suitable for gradual inclusion in DOC-07 quality gates.

## Example Record Format

Each example in child documents must contain:

1. **Context** — where it is used (module/app/guide).
2. **Command(s)** — minimum runnable set.
3. **Expected result** — what constitutes successful execution.
4. **Environment constraints** — what may block execution.
5. **Owner** — who is responsible for keeping the example up to date.

## Basic Smoke Scenarios (First Layer)

### 1) Full local stack (dev-start)

```bash
./scripts/dev-start.sh
```

Expected result:

- backend is available at `http://localhost:5150`;
- admin/storefront host surfaces are up in dev-profile.

Source: `docs/guides/quickstart.md`, `scripts/dev-start.sh`.

### 2) Installer preflight (no migrations)

Send an `InstallPlan` JSON document to `POST /api/install/preflight`. The
production server binary does not accept `install` subcommands; see the
[installer HTTP adapter](../guides/quickstart.md#installer-http-adapter).

Expected result:

- preflight returns a report;
- no migrations or side-effect bootstrap steps are triggered.

Source: `docs/guides/quickstart.md`.

### 3) Docs lint baseline

```bash
npx --yes markdownlint-cli <changed-files>
```

Expected result:

- correct `exit code` (`0` for pass, otherwise fail).

Source: `docs/research/fix docs.md`.

## Related Documents

- [Quickstart](../guides/quickstart.md)
- [Documentation Fix Plan](../research/fix%20docs.md)
- [Platform Summary Verification Plan](../verification/PLATFORM_VERIFICATION_PLAN.md)
