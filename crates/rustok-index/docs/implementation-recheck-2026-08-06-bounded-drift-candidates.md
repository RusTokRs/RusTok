# `rustok-index` implementation recheck — bounded drift candidates

Audited baseline: `main@53aeddfbf05ceccea27f6c2f639af904c3ace6b2`.
Latest default branch checked through
`main@66f36254ce5607f38fa480968e69b355a0128fe6`.

The only main delta after the baseline is Pages storefront Navigation/SEO ETag composition. It does
not touch `crates/rustok-index`, Product Index composition, Index GraphQL transports, Index diagnosis
services, or Index verifier files changed by PR #3033.

## Rechecked scope

This slice adds only the database-neutral discovery contract:

- `IndexDriftCandidateScope`;
- `IndexDriftCandidateFence`;
- `IndexDriftCandidateCursor`;
- `IndexDriftCandidateRequest`;
- `IndexDriftStaleEntityCandidate`;
- `IndexDriftOrphanLinkCandidate`;
- `IndexDriftCandidatePage`;
- `IndexDriftCandidateReader` and bounded failure classification.

No PostgreSQL reader, server composition, GraphQL resolver, scheduler, finding writer, lifecycle
command, or repair adapter is added.

## Request and continuation checks

The request constructor requires:

- one non-nil tenant;
- one exact positive-version `SchemaRef`;
- one limit in `1..=32`;
- either no continuation or both an opaque fence and opaque cursor.

Cursor content is bounded to 4 KiB. Fence content is bounded to 512 bytes. Their `Debug`
implementations reveal only encoded length.

The contract intentionally keeps fence and cursor separate. The future PostgreSQL reader owns their
contents, and a later transport must seal both values before publication.

## Candidate checks

Stale entity construction requires one valid exact `EntityKey` and one positive indexed source
version.

Orphan link construction requires:

- one valid source `EntityKey`;
- one positive indexed source version;
- one typed `LinkName`;
- one `u32` ordinal;
- one non-nil positive-version `LinkedEntityKey` target.

The candidate values contain identity and version metadata only. No indexed fields, owner fields,
schema fingerprint, record body, link payload, SQL row, database cause, finding state, or repair
intent crosses this boundary.

## Page checks

`IndexDriftCandidatePage::new` verifies before page return:

1. candidate count does not exceed the request limit;
2. a continuation request keeps the same fence;
3. an empty page cannot advertise continuation;
4. the next cursor differs from the request cursor;
5. every source candidate remains inside the exact tenant/schema scope;
6. candidate identities are strictly increasing.

Ordering is phase-stable: stale entity keys first, then orphan links ordered by source key, link name,
ordinal, and target identity. The PostgreSQL reader must encode the phase and last ordering tuple in
its private cursor.

## Failure boundary

Reader failures expose only retryable/permanent classification and one bounded machine code. The
contract itself imports no SeaORM, database connection, GraphQL, environment, secret resolver,
scheduler, lifecycle, or repair capability.

## Source-review limitations

This recheck does not claim:

- successful compilation;
- test or verifier execution;
- correctness of a future SQL query or index selection;
- an admitted repeatable-read or high-watermark fence implementation;
- proof that an enumerated entity is actually absent from its owner source;
- proof that an orphan target violates owner visibility or lifecycle policy;
- finding persistence, lifecycle, or repair behavior.

The next slice must compose and separately review a PostgreSQL `IndexDriftCandidateReader` using
bounded keyset reads and an immutable read-only fence.

## Validation ownership

Per maintainer instruction, the implementation agent did not run tests, JavaScript verifiers,
formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI.
