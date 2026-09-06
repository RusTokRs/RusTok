# RusToK RBAC CLI

Operational command adapters for the RBAC module.

## Responsibilities

- Parse RBAC maintenance commands.
- Invoke RBAC owner services with explicit tenant and actor context.
- Preserve structured errors and command evidence.

## Interactions

This crate depends on `rustok-rbac`, `rustok-cli-core`, and the shared runtime. RBAC policy and persistence remain owner-controlled.

## Entry points

- `src/lib.rs` — command registration and dispatch.

See the [RBAC documentation](../docs/README.md) and the [module documentation map](../../../../docs/modules/_index.md).
