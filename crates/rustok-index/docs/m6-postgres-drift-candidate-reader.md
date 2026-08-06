# M6 PostgreSQL drift candidate reader

Status: `source_complete_downstream_confirmation_and_persistence_complete`.

## Purpose

`PostgresIndexDriftCandidateReader` implements the database-neutral
`IndexDriftCandidateReader` contract over the existing `index_entities` and `index_links` tables.
It discovers bounded identities that require later authoritative confirmation:

- live materialized entities are stale-entity candidates until the owner source proves that the
  exact entity still exists or supplies an admitted absence watermark;
- links whose typed target row is absent or deleted are orphan-link candidates until target-owner
  visibility and lifecycle policy are confirmed.

The reader does not record a finding and does not claim that any returned candidate is already a
confirmed inconsistency.

## Transaction boundary

Every page runs inside one PostgreSQL `REPEATABLE READ READ ONLY` transaction. The transaction:

1. captures or validates the reader fence;
2. decodes the scope-bound private cursor;
3. executes bounded keyset SQL for the active phase;
4. constructs the typed candidate page;
5. commits without writing application data.

Unsupported database backends fail permanently before transaction creation. Begin/query/commit
failures map to the retryable machine code `index_drift_candidate_storage_unavailable`; database
causes and SQL text are never returned through the reader contract.

## Immutable visibility fence

The first page captures `txid_current_snapshot()::text`. The encoded fence contains only:

- wire version 1;
- a domain-separated SHA-256 digest of tenant UUID plus exact `SchemaRef`;
- the bounded PostgreSQL snapshot token.

The full tenant/schema values remain in the typed request and private cursor. Using a fixed 64-byte
hex digest keeps the fence inside its 512-byte contract even when module/entity identifiers and the
snapshot token approach their maximum bounds.

Continuation requests must carry the same fence. The reader recomputes the exact scope digest,
compares it before page SQL, and validates the canonical `xmin:xmax:active-xids` token shape.

Stale entity rows, link rows, their exact source-entity rows, and deleted target rows must have an
insertion version visible in the same captured transaction snapshot. Post-fence inserted or updated
versions therefore cannot be admitted as those candidate components later in the pass, including a
late commit that began before the first page.

A PostgreSQL snapshot value cannot resurrect a row version that a later transaction has physically
made invisible through update or delete. A post-fence materialized mutation can therefore
conservatively remove a candidate from the remainder of the current pass. A current target row whose
version is not visible in the fence is skipped rather than treated as a deleted target. A physically
absent target remains only a candidate and must be rechecked by the confirmation boundary. These
rules cannot confirm an inconsistency; a later bounded pass evaluates the new materialized state.
The reader is a candidate discovery boundary, not retained time-travel storage.

## Private cursor

The cursor is URL-safe unpadded base64 over bounded JSON. It binds:

- wire version 1;
- tenant UUID;
- exact schema;
- the active phase;
- only the last ordering tuple.

The two phases are:

1. `stale`, positioned by `(entity_id, locale_key)`;
2. `orphan`, positioned by source entity/locale, link name, ordinal, and typed target identity.

A page may finish the stale phase and use remaining capacity for orphan candidates. When stale
candidates exactly fill a page, one orphan lookahead determines whether the next cursor should begin
the orphan phase. Neither fence nor cursor contains fields, payloads, SQL, database errors, or owner
records. They remain internal and must be authenticated and encrypted before any future transport.

## Bounded stale-entity query

The stale phase reads only:

- one exact tenant/module/entity/schema-version scope;
- `is_deleted = FALSE` rows;
- positive source versions;
- row versions visible in the captured fence;
- strict keyset positions after the cursor;
- `limit + 1`, where the contract limit is at most 32.

It selects only entity ID, locale key, and source version. Payload, schema JSON, fields, links, and
fingerprints are not loaded.

Every returned row becomes `IndexDriftStaleEntityCandidate`. The name denotes a candidate for exact
source verification; the reader itself does not call the source registry.

## Bounded orphan-link query

The orphan phase:

- scopes `index_links` to the same source tenant and exact source schema;
- joins the exact current source entity/version and requires it to be live;
- requires both link and source-entity row versions to be visible in the captured fence;
- left-joins the typed target key;
- accepts a deleted target only when that tombstone version is visible in the same fence;
- skips a present target whose current version is post-fence;
- retains a physically absent target only as a candidate for later exact confirmation;
- applies strict tuple keyset ordering;
- reads at most `limit + 1` rows.

It selects only source identity/version, link name, ordinal, and target identity. It does not load
source payload, target payload, graph aggregates, owner visibility policy, or repair instructions.

## Failure and validation boundary

Malformed, oversized, cross-scope, unsupported-version, or structurally invalid fence/cursor values
fail with bounded permanent machine codes. Invalid stored identifiers, locales, UUIDs, ordinals,
schema versions, or source versions fail as
`index_drift_candidate_materialized_invalid` without exposing row contents.

The materializer performs no SQL and starts no task. The reader is exported by `rustok-index` but is
not inserted into `ModuleRuntimeExtensions`, server services, GraphQL, HTTP, CLI, MCP, or native
admin.

## Downstream boundary

`IndexDriftCandidateConfirmer` now performs exact owner/materialized confirmation, and
`PostgresIndexDriftConfirmedCandidateWriter` performs serializable write-time revalidation plus
idempotent finding persistence. Both remain internal and unmounted.

The reader itself still does not add:

- owner-source calls or target-owner confirmation;
- finding persistence, resolve/ignore lifecycle, or repair;
- a sealed candidate continuation codec or public transport;
- cursor persistence, background iteration, scheduling, or cross-page accumulation;
- retained PostgreSQL, concurrency, or restart evidence.

## Next implementation step

Add internal fail-closed resolve and ignore lifecycle commands with authorized actor identity,
bounded reason, exact state preconditions, and immutable audit evidence. Keep public transport,
scheduling, and repair separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidate_reader -- --nocapture
cargo test -p rustok-index drift_candidates -- --nocapture
node scripts/verify/verify-index-postgres-drift-candidate-reader.mjs
node scripts/verify/verify-index-drift-candidate-contract.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed by
the implementation agent.
