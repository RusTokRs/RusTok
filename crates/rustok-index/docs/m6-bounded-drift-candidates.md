# M6 bounded stale-entity and orphan-link candidate contract

Status: `source_complete_downstream_confirmation_and_persistence_complete`.

## Purpose

`IndexDriftCandidateReader` defines the database-neutral boundary for discovering materialized Index
state that may no longer be justified by an owner source:

- an Index entity may be stale because the exact source entity is now absent;
- an Index link may be orphaned because its typed target is missing or deleted.

This contract discovers bounded candidates only. It does not prove source absence, record a finding,
resolve or ignore a finding, mutate Index storage, or repair owner state.

The PostgreSQL implementation exists separately as `PostgresIndexDriftCandidateReader`. See
[M6 PostgreSQL drift candidate reader](./m6-postgres-drift-candidate-reader.md).

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

The concrete reader chooses the fence on the first page and preserves the same fence for every
continuation page.

The PostgreSQL implementation binds the exact scope to a bounded `txid_current_snapshot()` token and
filters materialized row versions through `txid_visible_in_snapshot`. This prevents post-fence insert
or update versions from appearing later in the same pass. PostgreSQL does not provide stateless
historical row resurrection, so a later update/delete may conservatively remove an old candidate from
the current pass; a subsequent bounded pass evaluates the new state. Candidate discovery never
promotes that omission into a finding.

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

The PostgreSQL reader owns the opaque keyset encoding and phase transition. It executes `limit + 1`
queries, can transition from stale to orphan candidates within one bounded page, and never collects
arbitrary IDs in memory.

## Failure boundary

`IndexDriftCandidateFailure` exposes only:

- retryable or permanent classification;
- one bounded lowercase machine code.

The contract exposes no SQL, connection error, table name, row payload, owner error, or secret.

## Downstream boundary

The database-neutral contract remains unchanged and capability-minimal. Separate internal slices now
provide:

- `PostgresIndexDriftCandidateReader` for bounded PostgreSQL discovery;
- `IndexDriftCandidateConfirmer` for exact owner/materialized confirmation;
- `PostgresIndexDriftConfirmedCandidateWriter` for serializable write-time revalidation and
  idempotent finding persistence.

These downstream capabilities remain unmounted. The candidate contract itself still does not add:

- GraphQL, HTTP, CLI, MCP, or native-admin transport;
- source loads, absence providers, PostgreSQL access, or finding writes;
- resolve/ignore lifecycle commands or actor audit;
- cursor persistence, background scanning, scheduling, or cross-page accumulation;
- targeted, full, shadow, or automatic repair;
- retained execution evidence.

## Next implementation step

Add internal fail-closed resolve and ignore lifecycle commands with authorized actor identity,
bounded reason, exact state preconditions, and immutable audit evidence. Keep public transport,
scheduling, and repair separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidates -- --nocapture
cargo test -p rustok-index drift_candidate_reader -- --nocapture
node scripts/verify/verify-index-drift-candidate-contract.mjs
node scripts/verify/verify-index-postgres-drift-candidate-reader.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed by
the implementation agent.
