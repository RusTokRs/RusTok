# Pages Immutable Artifact Rebuild Provenance

Date: 2026-08-06  
Status: `source-ready / maintainer-validation-pending`

## Purpose

The Pages immutable artifact integrity audit can identify damaged or incomplete retained Page Builder artifacts, but a safe rebuild must not use the mutable current draft as historical authority.

This slice records an immutable, locale-specific rebuild source for every **new** reviewed publish operation. The source is created in the same owner transaction as the publish receipt and immutable artifact manifest.

It does not repair, replace, delete or rebind any artifact.

## Storage owner

Pages owns the forward-only table:

```text
page_publish_rebuild_sources
```

Each row is unique by:

```text
publish operation + locale
```

The row binds:

- tenant and page;
- publish operation;
- exact page body UUID, format and revision observed by the reviewed publish transaction;
- immutable artifact UUID;
- the exact canonical sanitized Page Builder project snapshot;
- per-locale sanitized hash;
- artifact source hash;
- reviewed runtime hash;
- artifact and materialization hashes;
- exact materialization identity;
- exact runtime snapshots;
- one deterministic provenance hash.

The provenance row deliberately does not foreign-key its artifact UUID. It remains available if an artifact row is missing or damaged, while the publish-operation foreign key keeps tenant/page lifecycle ownership explicit.

## Transactional capture

`page_publish_operation::ActiveModelBehavior::after_save` already persists the immutable publish manifest before the owner transaction commits.

The same hook now:

1. loads the exact locale-ordered published bindings;
2. reloads each exact GrapesJS body selected by the binding;
3. parses and sanitizes the project through the canonical Page Builder sanitizer;
4. verifies the sanitization envelope;
5. requires complete reviewed materialization evidence;
6. recomputes the locale-ordered `sanitized_set_hash`;
7. recomputes the locale-ordered `artifact_set_hash`;
8. rejects the publish receipt when either aggregate differs;
9. writes the artifact manifest and rebuild provenance rows.

Because an error from the hook aborts the surrounding owner transaction, a publish receipt cannot commit with a partial manifest or partial provenance set.

## Historical boundary

Existing publish operations and legacy artifacts are not backfilled.

A historical artifact without a provenance row remains auditable and rollback-selectable under the existing contracts, but it is not eligible for a future rebuild command until an explicit, separately reviewed import policy exists.

The current mutable draft is never treated as historical rebuild authority. The complete reviewed runtime context is not duplicated into this table; a future rebuild command must obtain an explicitly reviewed runtime context and prove that its review/context hashes match the retained provenance and materialization identity.

## Repair boundary

A future repair command must:

- require tenant-wide `pages:manage`;
- select one exact publish operation and locale provenance row;
- obtain an explicitly reviewed runtime context and require its review/context hashes to match the retained evidence;
- verify provenance, sanitization, runtime and artifact identities before compiling;
- append a new immutable artifact;
- never update the damaged artifact in place;
- keep binding replacement explicit, idempotent and separately authorized;
- emit lifecycle/cache effects only after an explicit binding switch.

This slice intentionally adds none of those command or transport behaviors. Automatic repair or rebuild is not added.

## Preserved behavior

No existing artifact payload, binding, rollback manifest, publish DTO, GraphQL/HTTP route, public storefront read, cache key, event schema, inline authoring flow, workflow or rollout behavior changes.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance.mjs
cargo test -p rustok-pages publish_rebuild_provenance -- --nocapture
cargo test -p rustok-pages artifact_rollback -- --nocapture
cargo check -p rustok-pages --all-targets
```

SQLite/PostgreSQL migration, reviewed publish, rollback compatibility and future rebuild replay evidence remain pending.
