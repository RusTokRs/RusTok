# Documentation `rustok-content`

`rustok-content` is the shared content/orchestration module of the platform. It no longer
owns product CRUD for blog/forum/pages, but holds the common rich-text, locale and
conversion contracts that domain modules rely on.

For the target richtext boundary, neutral wire types live in
`rustok-api::richtext`; this module owns executable profiles, validation,
normalization, safe HTML rendering, and plain-text extraction. Domain modules
retain their locale rows and body storage. The legacy executable implementation
still lives in `rustok-core::rt_json` and is tracked for atomic removal by the
[central plan](../../../docs/modules/rich-text-implementation-plan.md).

## Purpose

- publish a shared content/orchestration runtime contract;
- keep locale normalization, rich-text validation and conversion semantics inside the module;
- provide domain modules with a stable orchestration layer without reverting to shared product storage.

## Scope

- `ContentOrchestrationService`, orchestration audit/idempotency and canonical URL state;
- shared richtext policy and locale fallback helpers without shared domain-body
  persistence;
- conversion flows `topic <-> post`, split/merge topic and canonical URL policy, including prohibition of cross-target canonical collisions and alias shadowing;
- owner-owned GraphQL query `resolveCanonicalRoute` for canonical URL read contract;
- content-owned GraphQL dataloaders for `nodes`, `node_translations` and `bodies`;
- owner-owned dashboard helper `load_post_stats_snapshot` and DTO `ContentCountSnapshot` for post-statistics without SQL on `nodes` inside `apps/server`;
- orchestration tables, audit trail and domain events;
- absence of product-owned CRUD/runtime adapters for blog/forum/pages.

## Integration

- used by `rustok-blog`, `rustok-forum`, `rustok-pages` and `rustok-comments` as a shared helper/orchestration contract;
- `rustok-content-orchestration` holds the integration bridge and GraphQL mutations conversion path;
- `apps/server` only assembles GraphQL roots, registers owner-owned dataloaders from owner/support crates and composes the content-owned dashboard post analytics helper;
- `rustok-index` depends on canonical URL and reindex semantics, but does not become the owner of orchestration logic;
- RBAC, idempotency and unsafe-input validation must remain part of the module-level contract.

## Verification

- `npm run verify:content:orchestration` — compile-free guardrail for orchestration RBAC/idempotency/audit/outbox/canonical URL invariants, targeted collision rollback/no-outbox evidence markers and docs/registry synchronization.
- `cargo xtask module validate content`
- `cargo xtask module test content`
- targeted tests for orchestration commands, canonical URL collision/alias shadowing rollback, locale fallback and rich-text validation

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Richtext implementation plan](../../../docs/modules/rich-text-implementation-plan.md)
- [Legacy RT JSON implementation snapshot](../../../docs/standards/rt-json-v1.md)
