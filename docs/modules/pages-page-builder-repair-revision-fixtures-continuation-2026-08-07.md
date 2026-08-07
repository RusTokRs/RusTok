# Pages / Page Builder Repair Revision Fixture Continuation

Date: 2026-08-07  
Status: source-ready / repair-reviewed-publish-revision-fixtures-owner-aligned / provenance-postgres-harness-source-ready / execution-pending  
Scope: align repair publish fences with the current reviewed-publish owner and advance the provenance execution cursor

## Rechecked production contract

`PageService::publish_reviewed` compares the caller-provided body revision set against `reviewed_publish::body_revision_snapshot`.

The current owner snapshot is:

```text
(locale, body.updated_at.to_string())
```

The repair source packets had drifted from that implementation by constructing:

```text
updated_at:sha256(format\0content)
```

That composite value is not the current production owner contract and could reject the fixture before the intended rebuild/activation assertions are reached.

## Corrected repair source packets

The following harnesses now pass the created body DTO's `updated_at` value directly:

```text
crates/rustok-pages/tests/explicit_artifact_repair_postgres.rs
crates/rustok-pages/tests/explicit_artifact_repair_failures_sqlite.rs
crates/rustok-pages/tests/explicit_artifact_repair_cache_postgres.rs
```

Their corresponding source guards read:

```text
crates/rustok-pages/src/services/page/reviewed_publish.rs
```

and require the fixture to match `body_revision_snapshot`. The guards also forbid the stale SHA-256 revision construction and SHA-256 import in those repair harnesses.

Machine evidence remains unvalidated and records:

```text
reviewed_publish_revision_matches_owner_updated_at_snapshot = true
```

## Provenance cursor now source-ready

The dedicated reviewed-publish provenance PostgreSQL packet now exists:

```text
crates/rustok-pages/tests/publish_rebuild_provenance_postgres.rs
crates/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-postgres-source.json
crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance-postgres.mjs
docs/modules/pages-page-builder-publish-provenance-postgres-continuation-2026-08-07.md
```

It retains source scaffolding for:

- exact two-locale body/provenance capture;
- `artifact_set_hash` mismatch rollback;
- `sanitized_set_hash` mismatch rollback;
- no fake manifest/provenance rows after aggregate mismatch;
- rebuild provenance surviving loss of its referenced artifact row after other artifact references are removed;
- migration source boundaries proving no artifact foreign key and no legacy publish-receipt backfill path.

The provenance packet is source-ready only. It has not been executed.

## Preserved boundaries

- No production Rust source is changed.
- No database migration or schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- No rebuild, activation or cache behavior is changed.
- No automatic audit-to-rebuild or rebuild-to-activation flow is introduced.
- No FFA/FBA promotion is made.

## Evidence state

The repair packets remain source-only:

```text
pages_explicit_artifact_repair_postgres_source_unvalidated
pages_explicit_artifact_repair_failures_source_unvalidated
pages_explicit_artifact_repair_cache_source_unvalidated
```

The new provenance packet remains:

```text
pages_publish_rebuild_provenance_postgres_source_unvalidated
```

Their `execution` arrays remain empty and validation flags remain false. Tests, source guards, Cargo, PostgreSQL/SQLite scenarios, cache-handler execution, workflows and CI are intentionally not run in these source slices.

## Next cursor

1. Execute the provenance source guard/PostgreSQL harness and retain accepted exact locale capture, aggregate rollback, artifact-loss and migration-boundary evidence.
2. Execute the already source-ready SQLite/PostgreSQL audit packets.
3. Execute the repair owner/transport/PostgreSQL/failure/cache packets and retain accepted evidence.
4. Retain bounded audit/repair transport execution with current-tenant and Pages Manage fencing.
5. Execute the broader artifact/HTTP/browser and tenant Wave packets before FFA/FBA promotion.
6. Keep automatic audit-to-rebuild and rebuild-to-activation chaining absent until accepted execution evidence supports a policy change.
