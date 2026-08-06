# PostgreSQL drift candidate reader recheck — 2026-08-06

Status: `source_reviewed_unvalidated`.

## Reviewed boundary

The review covered the new PostgreSQL reader, its application contract, storage migration, public
exports, documentation, and static verifier.

The reader is source-reviewed to:

- implement only `IndexDriftCandidateReader`;
- reject non-PostgreSQL backends;
- run one `REPEATABLE READ READ ONLY` transaction per page;
- capture a scope-bound `txid_current_snapshot()` fence;
- validate version, tenant, exact schema, cursor phase, and keyset position;
- execute only exact-scope `limit + 1` keyset reads;
- enumerate live entity candidates before absent/deleted-target link candidates;
- return only typed identities and positive source versions;
- map failures to bounded machine codes;
- perform no source call, finding write, lifecycle transition, scheduling, or repair.

## Fence conclusion

A timestamp fence can admit a late commit whose transaction began earlier. The implementation uses
PostgreSQL transaction-snapshot visibility over row `xmin` instead. Post-fence inserted or updated
row versions cannot appear in continuation pages.

A later transaction cannot resurrect a row version removed after the fence, so the current pass may
conservatively omit that row. This cannot confirm a finding; a later bounded pass evaluates the new
state. No transaction or connection is held between page requests.

## SQL conclusion

The stale phase selects only entity ID, locale, and source version from exact-scope live
`index_entities`, ordered by `(entity_id, locale_key)`.

The orphan phase joins exact current source entity/version, left-joins the typed target key, retains
absent or deleted targets, and orders by source key, link, ordinal, and target identity.

No payload, `SELECT *`, offset pagination, write SQL, row lock, advisory lock, or unbounded ID
collection was added.

## Open boundaries

Exact owner confirmation, orphan policy confirmation, finding persistence, public transport,
background execution, lifecycle, repair, and retained PostgreSQL evidence remain open.

## Validation disclosure

No tests, verifiers, formatting, Cargo commands, PostgreSQL scenarios, workflows, or CI were run.
Compilation and runtime behavior are not claimed.
