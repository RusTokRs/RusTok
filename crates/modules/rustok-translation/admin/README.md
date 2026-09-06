# rustok-translation-admin

> **For contributors and AI agents — read before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

## Purpose

`rustok-translation-admin` owns the module's Leptos admin workbench and the
typed transport contract shared by its native and GraphQL runtime profiles.

## Responsibilities

- Keep framework-neutral admin requests, responses, view models, and command
  construction outside the Leptos render adapter.
- Select native server functions for SSR/hydrate and GraphQL for CSR/headless.
- Preserve tenant, permission, locale, deadline, and idempotency semantics
  across both paths.
- Expose the versioned glossary operator workflow with URL-owned
  `glossary_id` selection and immutable glossary revision binding during job
  creation.
- Expose Translation Memory list/read/lookup and revision-guarded
  retention/tombstone/purge operations with URL-owned `memory_entry_id`
  selection. No entry is selected implicitly.
- Expose bounded immutable job export and atomic per-item import through the
  module-owned interchange service and canonical QA path.
- Expose private object-storage interchange artifact create/list/read/store/
  process operations. Artifact bytes never enter the admin contract except as
  the bounded document selected for an explicit read or store; expiry,
  checksum, exclusive import-processing leases, and aggregate conflict
  reporting remain module-owned.
- Expose bounded reviewer queue and workload reads derived from current job-item
  and proposal evidence without introducing an admin-owned workflow store.
- Expose bounded private, append-only job/item workflow notes through the
  Translation owner service. Note bodies are never sent to Translation Memory,
  machine translation, owner application, or workflow-event payloads.
- Keep Translation business rules in `rustok-translation`.
- Consume owner content only through the Translation module's neutral provider
  registry.

## Interactions

The native adapter receives `HostRuntimeContext` from the host and calls
Translation services directly. It reads an optional host `StorageRuntime` only
for private interchange artifacts and fails that lifecycle closed when it is
absent. The GraphQL adapter uses `rustok-graphql`.
The Leptos root consumes only the transport facade. Neither adapter reads owner
tables.

## Entry points

- `TranslationAdmin`

The package-internal typed boundary contains 49 operations. Native and GraphQL
adapters cover the same set, and the workbench exposes the machine-translation
estimate, generation, status, cancellation, and recovery flow alongside
revision-guarded assignment/unassignment, bounded reviewer queue and workload
reads, private workflow-note list/create/resolve, object-storage artifact
create/list/read/store/process, blocked-item retry, job
cancellation, and owner-apply recovery. GraphQL
documents are validated against the
module-owned schema so the host cannot bypass or redefine the rendered module
surface.

The module manifest publishes this package together with the matching
[`@rustok/translation-admin`](../../../apps/next-admin/packages/translation/README.md)
Next admin surface.

See the
[module UI implementation guide](../../../docs/UI/module-package-implementation.md)
and the
[Translation implementation plan](../docs/implementation-plan.md).
