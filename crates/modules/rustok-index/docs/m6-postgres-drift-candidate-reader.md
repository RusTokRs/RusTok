# M6 PostgreSQL drift candidate reader

Status: `source_complete_downstream_repair_composition_complete`.

## Purpose

`PostgresIndexDriftCandidateReader` implements the database-neutral candidate contract over
`index_entities` and `index_links`. It returns bounded identities that require later authoritative
confirmation; it never records a finding or claims an inconsistency.

## Transaction and fence

Every page runs in one PostgreSQL `REPEATABLE READ READ ONLY` transaction. The first page captures
`txid_current_snapshot()::text`; the fence contains wire version, a domain-separated SHA-256 digest
of exact tenant/schema scope, and the bounded snapshot token. This keeps the fence inside 512 bytes.

Entity, link, source-entity, and deleted-target insertion versions are filtered through
`txid_visible_in_snapshot`. Post-fence insert/update versions cannot enter continuation pages. A
later materialized delete may conservatively remove a candidate because the stateless snapshot token
cannot resurrect historical rows; a later pass reevaluates current state.

A current target row whose version is not visible in the fence is skipped, not classified as a
deleted target. Physically absent targets remain candidates only.

## Private cursor and bounded SQL

The URL-safe unpadded cursor binds wire version, tenant, exact schema, phase, and the last ordering
tuple. Phases are stale then orphan.

Both queries use strict keyset ordering and `limit + 1`:

- stale rows by `(entity_id, locale_key)`;
- orphan rows by source identity, link name, ordinal, and typed target identity.

The stale query selects only identity and source version. The orphan query selects only source
identity/version, link identity, ordinal, and target identity. Payload, fields, schema JSON,
fingerprints, graph aggregates, owner records, and repair instructions are not loaded.

## Failure and composition boundary

Malformed fence/cursor and invalid stored identity/version map to bounded machine codes. SQL and
database causes are not returned.

The reader materializer starts no task and the reader is not inserted into `ModuleRuntimeExtensions`
or any GraphQL, HTTP, CLI, MCP, or native-admin transport.

## Downstream boundaries

Separate internal layers now provide exact confirmation, serializable finding persistence,
authorization-gated lifecycle commands, generic durable targeted repair, and
one concrete missing-entity repair path through the canonical mutation inbox.

The reader itself still has no source call, finding write, lifecycle transition, mutation owner,
public continuation, scheduler, page loop, or automatic repair capability.

## Next implementation step

Add the separate authorized recovery policy for ambiguous durable `prepared` repair commands. The
candidate reader remains a read-only discovery boundary and is not expanded by that policy.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidate_reader -- --nocapture
node scripts/verify/verify-index-postgres-drift-candidate-reader.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were run by
the implementation agent.
