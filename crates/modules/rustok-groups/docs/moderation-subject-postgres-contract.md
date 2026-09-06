# Groups moderation subject PostgreSQL contract

This packet closes one bounded GROUPS-07 evidence gap for the already source-complete neutral
`groups / GroupMembership` moderation adapter. It does not add a new moderation effect or move any
Groups-owned state into Moderation.

## Runtime boundary

`crates/rustok-groups/tests/moderation_subject_postgres.rs` runs the production
`GroupsModerationSubjectAdapterFactory` against PostgreSQL. The isolated schema installs the real
Outbox idempotency migrations and the real Groups migrations, then seeds only Groups-owned group and
membership rows. No Moderation owner tables are installed or read.

The evidence exercises the same trusted application context emitted by Moderation dispatch:

- service actor `rustok-moderation`;
- exact tenant UUID;
- canonical `Group` scope claim;
- decision UUID as the producer idempotency key;
- exact `groups / GroupMembership` subject and reviewed membership revision;
- typed `SuspendSubject` decision effect.

## Apply and lost-response replay

The first decision must create one Groups-owned suspension with `source_kind = moderation_decision`,
preserve the immutable Moderation decision UUID/hash as provenance, advance membership revision from
1 to 2 and group version from 1 to 2, and keep lifecycle `member_count` unchanged.

The exact same decision is then submitted again after the membership revision has already advanced.
It must return the exact stored application. This is the executable receipt-before-subject-read
assertion: a replay that re-read and re-fenced the current membership first would incorrectly reject
the already committed decision as stale. Audit and semantic-event counts must remain one.

The same decision UUID with changed request content must conflict without a second Groups mutation,
proving that the producer receipt binds the full `{scope, command}` request identity rather than only
the idempotency key.

## Concurrency revision fence

Two different immutable Moderation decisions concurrently target the same membership at reviewed
revision 1. Both may admit independent producer receipts, but the Groups owner lock sequence
serializes the domain mutation. Exactly one decision may advance the membership to revision 2; the
other must fail non-retryably with `groups.moderation_subject_revision_conflict` after observing the
winner's revision.

Replaying the losing decision must replay that stored non-retryable conflict. No second suspension,
audit row, semantic event, group-version increment, or lifecycle-count change is allowed.

## Execution

The durable workflow is
`.github/workflows/groups-moderation-subject-postgres.yml`. It supplies PostgreSQL 16 and executes:

```bash
node scripts/verify/verify-groups-moderation-subject-postgres.mjs

RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://postgres:postgres@localhost:5432/rustok_groups_moderation_evidence' \
  cargo test --locked -p rustok-groups \
  --test moderation_subject_postgres -- --ignored --nocapture
```

The workflow also checks the focused test target with the locked workspace graph. Success is bounded
to moderation-adapter apply/replay/concurrency evidence; broader provider cutover, GraphQL parity,
SQLite parity, and remaining GROUPS-07/GROUPS-19 gates stay open.
