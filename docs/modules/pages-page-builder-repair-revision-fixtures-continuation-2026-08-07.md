# Pages / Page Builder Repair Revision Fixture Continuation

Date: 2026-08-07  
Status: source-ready / repair-reviewed-publish-revision-fixtures-owner-aligned / execution-pending  
Scope: align repair PostgreSQL, failure and cache harness publish fences with the current reviewed-publish owner contract

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

## Corrected source packets

The following harnesses now pass the created body DTO's `updated_at` value directly:

```text
crates/rustok-pages/tests/explicit_artifact_repair_postgres.rs
crates/rustok-pages/tests/explicit_artifact_repair_failures_sqlite.rs
crates/rustok-pages/tests/explicit_artifact_repair_cache_postgres.rs
```

Their corresponding source guards now read:

```text
crates/rustok-pages/src/services/page/reviewed_publish.rs
```

and require the fixture to match `body_revision_snapshot`. The guards also forbid the stale SHA-256 revision construction and SHA-256 import in those repair harnesses.

Machine evidence remains unvalidated and records:

```text
reviewed_publish_revision_matches_owner_updated_at_snapshot = true
```

## Preserved boundaries

- No production Rust source is changed.
- No database migration or schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- No rebuild, activation or cache behavior is changed.
- No automatic audit-to-rebuild or rebuild-to-activation flow is introduced.
- No FFA/FBA promotion is made.

## Evidence state

The three repair packets remain source-only:

```text
pages_explicit_artifact_repair_postgres_source_unvalidated
pages_explicit_artifact_repair_failures_source_unvalidated
pages_explicit_artifact_repair_cache_source_unvalidated
```

Their `execution` arrays remain empty and every validation flag remains false. Tests, source guards, Cargo, PostgreSQL/SQLite scenarios, cache-handler execution, workflows and CI are intentionally not run in this cleanup.

## Next cursor

1. Retain the dedicated reviewed-publish provenance migration/publish packet for exact locale/source capture, aggregate-hash mismatch rollback, artifact-row loss and legacy no-backfill behavior.
2. Then execute the already source-ready audit and repair owner/transport/PostgreSQL/failure/cache packets and retain accepted evidence.
3. Retain bounded audit/repair transport execution with current-tenant and Pages Manage fencing.
4. Execute the broader artifact/HTTP/browser and tenant Wave packets before FFA/FBA promotion.
5. Keep automatic audit-to-rebuild and rebuild-to-activation chaining absent until accepted execution evidence supports a policy change.
