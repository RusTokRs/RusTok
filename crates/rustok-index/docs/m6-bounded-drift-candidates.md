# M6 bounded stale-entity and orphan-link candidate contract

Status: `source_complete_postgres_reader_pending`.

## Purpose

`IndexDriftCandidateReader` defines the database-neutral boundary for discovering materialized Index
state that may no longer be justified by an owner source:

- an Index entity may be stale because the exact source entity is now absent;
- an Index link may be orphaned because its typed target is missing or deleted.

This contract discovers bounded candidates only. It does not prove source absence, record a finding,
resolve or ignore a finding, mutate Index storage, or repair owner state.

## Exact request scope

Every request carries one `IndexDriftCandidateScope`:

- one non-nil tenant UUID;
- one exact `SchemaRef`, including a positive schema version.

`IndexDriftCandidateRequest` accepts a page limit in `1..=32`.

The first request has no continuation. Every later request must carry both:

- one opaque `IndexDriftCandidateFence` bounded to 512 bytes;
- one opaque `IndexDriftCandidateCursor` bounded to 4 KiB.

Supplying only a fence or only a cursor fails before reader access.

## Fence and continuation semantics

The concrete reader chooses the fence on the first page. A PostgreSQL implementation may use a
captured high-watermark, exported snapshot identity, or another immutable repeatable boundary, but
it must preserve the same fence for every continuation page.

`IndexDriftCandidatePage::new` rejects:

- a changed fence;
- a cursor that does not advance;
- an empty page with a continuation;
- more candidates than the requested limit;
- candidates outside the exact tenant/schema scope;
- duplicate or descending candidate identities.

Cursor and fence `Debug` output reveals only encoded length. A future transport must seal them in an
authenticated confidential envelope rather than expose either value directly.

## Typed candidates

`IndexDriftCandidate` has two separate variants.

### Stale entity

`IndexDriftStaleEntityCandidate` carries only:

- the exact indexed `EntityKey`;
- the positive indexed source version.

It does not carry indexed fields, links, schema fingerprint, owner payload, absence proof, finding
state, or repair intent.

### Orphan link

`IndexDriftOrphanLinkCandidate` carries only:

- the exact source `EntityKey`;
- the positive source entity version stored by Index;
- one typed `LinkName`;
- one bounded `u32` ordinal;
- one typed `LinkedEntityKey` target identity.

It does not carry the source record, target record, link payload, SQL row, database cause, or a
caller-selected target list.

## Stable ordering

Candidate ordering is strict and deterministic:

1. stale entity identities ordered by exact `EntityKey`;
2. orphan link identities ordered by source key, link name, ordinal, and target identity.

The concrete reader owns the opaque keyset encoding and phase transition. A page cannot return a
duplicate or descending identity. This permits bounded continuation without collecting arbitrary IDs
in memory.

## Failure boundary

`IndexDriftCandidateFailure` exposes only:

- retryable or permanent classification;
- one bounded lowercase machine code.

The contract exposes no SQL, connection error, table name, row payload, owner error, or secret.

## Deliberate limits

This slice does not add:

- a PostgreSQL candidate reader;
- a GraphQL, HTTP, CLI, MCP, or native-admin transport;
- source absence verification for a stale candidate;
- target visibility or owner validation for an orphan link;
- drift finding persistence or lifecycle commands;
- cursor persistence, background scanning, scheduling, or cross-page accumulation;
- targeted, full, shadow, or automatic repair;
- retained execution evidence.

## Next implementation step

Add one PostgreSQL `IndexDriftCandidateReader` that:

- uses one read-only repeatable boundary or immutable high-watermark fence;
- performs bounded keyset reads only;
- never starts with an unbounded in-memory ID collection;
- returns live, non-deleted `index_entities` as stale candidates for later exact source proof;
- returns `index_links` whose typed target is absent or deleted as orphan candidates;
- preserves the contract ordering and exact scope;
- maps database failures to bounded candidate failure codes;
- performs no finding write, lifecycle transition, or repair.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidates -- --nocapture
node scripts/verify/verify-index-drift-candidate-contract.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed by
the implementation agent.
