# rustok-core

## Purpose

`rustok-core` owns the minimal platform-wide runtime contracts used across RusToK.

## Responsibilities

- Define the base module traits and registry-facing contracts.
- Define shared permission, identity, ID, and error primitives.
- Provide flex/custom-fields schema contracts and content-format helpers used by multiple domains.
- Keep compatibility re-exports for foundational runtime contracts that are being split into dedicated crates.
- Stay free from host-specific transport, ORM, and UI concerns.
- Remain free from domain-specific orchestration logic (auth lifecycle, user CRUD, commerce flows).

## Entry points

- `RusToKModule`
- `ModuleRegistry`
- `Permission`
- `generate_id`
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
