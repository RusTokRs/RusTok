# M6 bounded stale-entity and orphan-link candidate contract

Status: `source_complete_downstream_repair_composition_complete`.

## Purpose

`IndexDriftCandidateReader` is the database-neutral boundary for discovering materialized Index state
that may no longer be justified by an owner source:

- a live Index entity may be stale because the exact owner entity is absent;
- a link may be orphaned because its typed target is absent or deleted.

Candidates are not findings and grant no confirmation, lifecycle, or repair authority.

## Exact scope and continuation

Every request carries one non-nil tenant UUID, one exact positive-version `SchemaRef`, and a
page limit in `1..=32`.

The first request has neither fence nor cursor. Continuation requires both:

- opaque fence bounded to 512 bytes;
- opaque cursor bounded to 4 KiB.

`IndexDriftCandidatePage::new` rejects changed fences, non-advancing cursors, empty continuation
pages, oversized pages, scope escape, duplicate identities, and descending order. Cursor and fence
`Debug` output expose encoded length only.

## Typed candidates

A stale-entity candidate carries only exact `EntityKey` and positive indexed source version.

An orphan-link candidate carries only exact source `EntityKey`, positive source version, typed
`LinkName`, `u32` ordinal, and typed `LinkedEntityKey` target.

No candidate carries payload, fields, source records, SQL rows, findings, actors, lifecycle state, or
repair input.

## Stable ordering

Ordering is strict and deterministic:

1. stale entities by exact `EntityKey`;
2. orphan links by source key, link name, ordinal, and target identity.

The PostgreSQL reader owns the private keyset representation and the stale-to-orphan phase
transition. It uses bounded `limit + 1` reads and does not collect arbitrary identifiers in memory.

## Failure boundary

`IndexDriftCandidateFailure` exposes only retryable/permanent classification and one bounded
lowercase machine code. SQL, database causes, owner errors, records, and secrets do not cross this
boundary.

## Downstream boundaries

Separate internal slices now provide:

- `PostgresIndexDriftCandidateReader` for bounded PostgreSQL discovery;
- `IndexDriftCandidateConfirmer` for exact owner/materialized confirmation;
- `PostgresIndexDriftConfirmedCandidateWriter` for serializable finding persistence;
- fail-closed resolve/ignore lifecycle commands with immutable audit;
- generic targeted-repair reservations and receipts;
- one concrete missing-entity evidence reader and inbox-idempotent delete owner.

The candidate contract itself remains unchanged and still contains no PostgreSQL, source calls,
finding writes, lifecycle transitions, mutation owner, transport, scheduler, page loop, or automatic
repair.

## Next implementation step

Add the separate fail-closed recovery policy for ambiguous durable `prepared` repair commands.
Candidate discovery remains outside that policy and receives no recovery or mutation capability.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidates -- --nocapture
node scripts/verify/verify-index-drift-candidate-contract.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were run by
the implementation agent.
