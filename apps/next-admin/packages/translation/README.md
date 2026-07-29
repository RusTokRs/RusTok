# @rustok/translation-admin

## Purpose

This package owns the Translation control-plane workbench rendered by the Next
admin host.

## Responsibilities

- Render policy, versioned glossary, Translation Memory, target, inventory,
  progress, and reviewed workflow operations.
- Use the host-provided GraphQL executor, tenant identity, authentication, and
  `next-intl` locale.
- Keep the selected workbench tab, glossary, and memory entry in the typed
  `tab`, `glossary_id`, and `memory_entry_id` URL query keys, without implicit
  first-item selection.
- Preserve caller-generated idempotency keys after failed commands.
- Keep Translation business and transport policy in the owning module rather
  than in host routes.

## Interactions

The package executes the same 32 module-owned operations as
`rustok-translation-admin`, including exact/contextual memory lookup and
revision-guarded retention, tombstone, and purge. It never reads owner tables
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
