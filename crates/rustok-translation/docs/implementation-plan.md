---
id: doc://crates/rustok-translation/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-27
---

# Translation implementation plan

## Scope

Build the optional, admin-only translation control plane described by the
central plan without taking ownership of domain-localized data or runtime locale
selection.

## Current State

- The platform module, manifest, RBAC resources, migration source, and
  distribution feature are present.
- The module persists tenant-scoped inventory identities and provider cursor
  checkpoints without storing source or translated text.
- `TranslationInventoryService` consumes only the neutral provider registry,
  checks Translation workflow permission, delegates owner authorization to the
  provider, and advances checkpoints with optimistic revision protection.
- Media is the first owner provider with durable change-cursor repair.
- Jobs, proposals, review, memory, glossaries, interchange, transports, UI, and
  AI integration are not implemented yet.

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `core_no_ui`
- Evidence:
  - module-owned core and neutral provider dependency are separated;
  - no transport or UI has been published;
  - admin UI is planned for Leptos and Next with native/GraphQL parity.
- Last verified at (UTC): 2026-07-27
- Owner: Translation module maintainers

## Milestones

1. Verify inventory replay, tenant isolation, stale-checkpoint conflict, provider
   outage, and full-rescan recovery with Media and reference providers.
2. Add manual jobs, immutable source snapshots, proposals, assignments, review,
   approval, quality issues, and transactional application receipts.
3. Add rebuildable progress projections and operator recovery.
4. Add translation memory and versioned glossaries with separate permissions.
5. Add bounded owner-aware import/export.
6. Publish module-owned GraphQL and native server-function adapters.
7. Add Leptos and Next admin surfaces in parity.
8. Integrate `rustok-ai-translation` only after manual apply and structured AI
   execution foundations pass their gates.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`
- `cargo check -p rustok-server --lib --no-default-features --features mod-translation`
- `node scripts/verify/verify-translation-surface-registry.mjs`

## Update Rules

- Never query or mutate owner tables from this module.
- Never count runtime fallback as an exact translation.
- Never let AI output apply owner data without deterministic validation and
  review.
- Keep the local FFA/FBA block and central readiness board synchronized when
  transport or UI appears.
- Keep `rustok-translation-targets` dependency-neutral even if its directory is
  physically colocated with this module later.
