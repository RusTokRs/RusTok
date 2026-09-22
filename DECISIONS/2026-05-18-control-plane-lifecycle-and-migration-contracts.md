# 2026-05-18 — Control-plane lifecycle and migration ordering contracts

## Status

Accepted.

## Context

The control plane must no longer consider `modules.toml` as the runtime source of truth: the active composition is stored in
`platform_state`, while `modules.toml` remains a bootstrap/dev input. Re-checking the module lifecycle revealed two
residual risks: a non-atomic `platform_state -> builds` path and lifecycle hooks that ran after
changing tenant state with only a partial rollback of the enabled flag. Separately, the server migrator had dependency-aware
ordering, but the `product_tags -> taxonomy_tables` dependency was hardcoded in the server migrator.

## Decision

- Composition update and build enqueue are considered a single control-plane action: CAS-update of `platform_state` and insert
  into `builds` are performed in a single database transaction and use a common `platform_state:<revision>` manifest ref.
- Immutable manifest artifact hash — SHA-256 of the canonical JSON of the full manifest snapshot, not a short hash of a
  subset of module fields.
- Enable/disable lifecycle records the operation with a `running` status before state mutation. Existing `on_enable` /
  `on_disable` are treated as compat pre-hooks: on error, tenant state is not changed, the operation becomes `failed`.
  A successful hook allows atomically changing the tenant state and completing the operation as `done`.
- Direct model-level toggle of the tenant module flag is no longer a public lifecycle API; only an explicitly
  named internal migration escape hatch remains.
- Cross-module migration ordering is declared alongside the module-owned migration exporter via lightweight descriptor
  metadata. The server migrator performs a topological sort and fails on missing dependency/cycle.

## Consequences

- Admin/runtime surfaces must go through canonical lifecycle/build contracts or a thin adapter that preserves the
  transaction boundary and manifest hash semantics.
- Hooks that require a post-commit side effect must be moved to a separate idempotent/retryable post-phase
  before this can be considered a hard lifecycle dependency.
- New module-owned migrations must not add server-local hardcoded dependency `match`; dependency metadata
  is added next to the owning module's exporter.
