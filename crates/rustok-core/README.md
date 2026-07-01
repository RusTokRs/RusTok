# rustok-core

## Purpose

`rustok-core` owns the minimal platform-wide runtime contracts used across RusToK.
Transport-neutral module port contracts are owned by `rustok-api`; `rustok-core`
does not define or re-export `Port*` types.

## Responsibilities

- Define the base module traits and registry-facing contracts.
- Define runtime identity, RBAC, security-context, ID, and error primitives; permission DTOs are consumed from `rustok-api`.
- Provide flex/custom-fields schema contracts and content-format helpers used by multiple domains.
- Keep compatibility re-exports for foundational runtime contracts that are being split into dedicated crates.
- Expose event foundation contracts, including EventBus stats, in-memory transport reliability, and backpressure observability controls, dispatcher retry semantics, and dispatch latency hooks.
- Stay free from host-specific transport, ORM, and UI concerns.
- Remain free from domain-specific orchestration logic (auth lifecycle, user CRUD, commerce flows).

## Entry points

- `RusToKModule`
- `ModuleRegistry`
- `Rbac` / `SecurityContext`
- `generate_id`
- `EventBus` / `MemoryTransport`
- `BackpressureController`
- `CustomFieldsSchema`
- foundational runtime types re-exported from `src/lib.rs`

## Interactions

- Used by all `rustok-*` domain and support crates as the common contract layer.
- Used by `apps/server` as the composition root for module registration and shared primitives.
- Works alongside `rustok-events`, which now owns canonical event schemas and validation rules.
- Works alongside `rustok-auth`, which owns canonical auth primitives, credential hashing, and JWT lifecycle.
- Used by `rustok-mcp`, `rustok-ai`, and other capability crates without pulling host-specific dependencies into the core layer.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
