# rustok-reactions-api

## Purpose

`rustok-reactions-api` defines neutral contracts for reusable reactions without
owning persistence, transports, UI, or any producer module's subject data.

## Responsibilities

- Validate bounded source slugs, subject kinds, reaction keys and catalogs.
- Bind every subject to tenant, source, kind, UUID and positive owner revision.
- Bind writes to a caller-provided command UUID and authenticated actor UUID.
- Support explicit single- or bounded multi-selection policy.
- Publish bounded actor state, aggregate snapshots and read/write ports.
- Publish a unique runtime registry for source-owned subject authorization.
- Preserve typed unavailable, invalid, conflict and retryable provider failures.

## Ownership boundary

A subject provider remains in the module that owns the subject. It validates the
current subject revision, visibility and reaction policy through its own owner
services. The Reactions owner never reads Forum, Blog, Comments, Profiles,
Groups, Media or Commerce tables.

The neutral crate contains no SeaORM entities, migrations, GraphQL, HTTP,
Leptos, React, event worker, catalog persistence or aggregate persistence.

Forum votes are not reactions. Any future migration from a module-specific vote
to a shared reaction requires an explicit semantic mapping and data migration.

## Entry points

- `ReactionSubjectRef`
- `ReactionCatalog`
- `ReactionSelectionPolicy`
- `ReactionCommandIdentity`
- `ApplyReactionCommand`
- `ReactionReadPort`
- `ReactionWritePort`
- `ReactionSubjectProvider`
- `ReactionSubjectProviderFactory`
- `ReactionSubjectRegistry`

See [module contract](docs/README.md) and
[implementation plan](docs/implementation-plan.md).
