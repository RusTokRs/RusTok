# AGENTS

This file defines rules and ownership for all contributors and AI agents working in the RusToK repository.

## How to engage

- Always start by reading [`docs/index.md`](docs/index.md) — the canonical documentation map.
- Review domain module documentation before making changes.
- Use module owners (or the platform team) for approvals when cross-cutting concerns are involved.
- For architecture changes, capture decisions in `DECISIONS/` using an ADR.

## Ownership map

- **Platform foundation**: `crates/rustok-core`, `apps/server`, shared infra.
- **Domain modules**: `crates/rustok-*` (content, commerce, pages, blog, forum, index, etc.).
- **Frontends**: `apps/admin`, `apps/storefront`, `apps/next-admin`, `apps/next-frontend`.
- **MCP server**: `crates/rustok-mcp`.
- **Operational tooling**: `scripts/`, `docker-compose*.yml`, `grafana/`, `prometheus/`.

Detailed module ownership and responsibilities are captured in [`docs/modules/registry.md`](docs/modules/registry.md).

## Documentation policy

### Language

- Central platform documentation (`docs/`) is written in **Russian** (the team's primary language).
- Public-facing contracts, `AGENTS.md`, `CONTRIBUTING.md`, and crate `README.md` files are in **English**.
- Mixed language within a single document is not allowed — choose one language per file.

### Placement

- Platform-wide documentation lives in `docs/`.
- Per-module/per-app documentation lives inside the component: `apps/<name>/docs/` or `crates/<name>/docs/`.
- Every app and crate must have a root `README.md` with: purpose, responsibilities, interactions, entry points, and a link to `docs/`.
- `docs/modules/_index.md` links to all per-module documentation folders.

### Keeping docs up to date

When changing **architecture, API, events, modules, tenancy, routing, UI contracts, or observability**:

1. Update the relevant local docs in the changed component (`apps/*` or `crates/*`).
2. Update the related central docs in `docs/`.
3. Update [`docs/index.md`](docs/index.md) so the map remains accurate.
4. If a module or application was added or renamed, update [`docs/modules/registry.md`](docs/modules/registry.md).
5. Mark outdated documents as `deprecated` or `archived` and point to the replacement.

Do not create a new document if a suitable one already exists — extend the existing one.

## AI Agent rules

Rules mandatory for all automated agents operating in this repository:

1. Always start by reading [`docs/index.md`](docs/index.md).
2. Do not create a new document when an existing one is suitable — extend it instead.
3. Documentation must reflect the actual state of the code.
4. Never bypass or disable pre-commit/pre-push hooks. Fix the root cause of failures.
5. Do not edit CI/CD workflow files unless explicitly requested.
6. Do not modify other branches — only work on the assigned task branch.
7. For Leptos apps and module-owned Leptos UI packages, use native `#[server]` functions as the default internal data layer and keep GraphQL in parallel. Do not remove or replace GraphQL when adding server functions.
