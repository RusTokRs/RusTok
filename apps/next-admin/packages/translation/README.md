# @rustok/translation-admin

## Purpose

This package owns the Translation control-plane workbench rendered by the Next
admin host.

## Responsibilities

- Render policy, versioned glossary, Translation Memory, bounded interchange,
  target, inventory, progress, reviewed workflow, and reviewer workload
  operations.
- Use the host-provided GraphQL executor, tenant identity, authentication, and
  `next-intl` locale.
- Keep the selected workbench tab, glossary, and memory entry in the typed
  `tab`, `glossary_id`, and `memory_entry_id` URL query keys, without implicit
  first-item selection.
- Preserve caller-generated idempotency keys after failed commands.
- Keep Translation business and transport policy in the owning module rather
  than in host routes.

## Interactions

The package executes the same 41 module-owned operations as
`rustok-translation-admin`, including exact/contextual memory lookup and
revision-guarded retention, tombstone, purge, immutable job export, and atomic
item import through canonical QA, plus machine-translation estimate, generation,
status, cancellation, and recovery controls, revision-guarded
assignment/unassignment, bounded reviewer queue and workload reads, blocked-item
retry, job cancellation, and owner-apply recovery. It never reads owner tables
and never performs cross-protocol mutation fallback. `apps/next-admin`
contributes only the route, GraphQL executor injection, and shell composition.

## Entry points

- `TranslationAdminPage`
- `executeTranslationOperation`
- `translationNavItems`
- Translation operation, response, and page-prop types

See the
[Translation implementation plan](../../../../crates/rustok-translation/docs/implementation-plan.md)
and the
[Next admin host contract](../../docs/README.md).
