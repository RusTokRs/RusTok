# PostgreSQL drift candidate reader recheck — 2026-08-06

Status: `source_reviewed_unvalidated`.

## Reviewed boundary

The review covered the new PostgreSQL reader, its application contract, storage migration, public
exports, documentation, and static verifier.

The reader is source-reviewed to:

- implement only `IndexDriftCandidateReader`;
- reject non-PostgreSQL backends;
- run one `REPEATABLE READ READ ONLY` transaction per page;
- capture a compact scope-digest-bound `txid_current_snapshot()` fence;
- validate version, exact scope binding, cursor phase, and keyset position;
- execute only exact-scope `limit + 1` keyset reads;
- enumerate live entity candidates before absent/deleted-target link candidates;
- require stale entity, link, exact source entity, and deleted target versions to be visible in the
  same fence;
- return only typed identities and positive source versions;
- map failures to bounded machine codes;
- perform no source call, finding write, lifecycle transition, scheduling, or repair.

## Fence conclusion

A timestamp fence can admit a late commit whose transaction began earlier. The implementation uses
PostgreSQL transaction-snapshot visibility over row `xmin` instead. Post-fence entity, link, source,
and deleted-target versions cannot be admitted as candidate components in continuation pages.

A present target whose current row version is post-fence is skipped rather than classified as a
deleted target. A later transaction still cannot resurrect a physically removed row version, so a
current pass may conservatively omit or surface an absent-target candidate that requires exact
confirmation. Candidate discovery cannot confirm a finding; a later bounded pass evaluates the new
state. No transaction or connection is held between page requests.

The fence stores a domain-separated SHA-256 digest of the exact tenant/schema scope rather than the
full maximum-length identifiers. This keeps the encoded fence within its 512-byte application
contract while preserving deterministic scope mismatch detection.

## SQL conclusion

The stale phase selects only entity ID, locale, and source version from exact-scope live
`index_entities`, ordered by `(entity_id, locale_key)`.

The orphan phase joins the exact current source entity/version, left-joins the typed target key,
accepts a deleted target only when its tombstone version is fence-visible, retains a physically
absent target only as a candidate, and orders by source key, link, ordinal, and target identity.

No payload, `SELECT *`, offset pagination, write SQL, row lock, advisory lock, or unbounded ID
collection was added.

## Open boundaries

Exact owner confirmation, absent-target timing confirmation, orphan policy confirmation, finding
persistence, public transport, background execution, lifecycle, repair, and retained PostgreSQL
evidence remain open.

## Validation disclosure

No tests, verifiers, formatting, Cargo commands, PostgreSQL scenarios, workflows, or CI were run.
Compilation and runtime behavior are not claimed.
