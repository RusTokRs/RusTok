# Forum moderation effect verification contract

Status: source-ready; Rust service/concurrency and full migration-chain execution remain unvalidated.

## Owner contract

`src/moderation_subject.rs` is the trusted Forum adapter. It retains the tenant-scoped
owner-operation receipt and reviewed-subject revision; it does not acquire author
self-deletion settings. Trusted removal uses `ReplyService::remove_in_tx`, sharing
the locking and removal-effects core with the public delete command.

- `RejectPublication` persists `ReplyStatus::Rejected`; hide persists Hidden.
  Both use shared publication effects after status persistence. Accepted solutions
  are removed and member reply/solution counts change in one guarded write.
- Removed accepted solution: persist `status=deleted` and its moderation snapshot,
  clear the solution, then update derived state. Archived topics retain their local
  approved-reply projection but contribute no category/member counts.
- Changed hide/reject effects invalidate category (on public withdrawal) and topic
  projections; public/solution removal invalidates both. Events, projections and
  receipt completion commit in the same transaction.
- Category → parent topic → solution scope precedes subject/revision locking.
  Recheck a reply's discovered parent. SQLite reserves its writer before parent
  discovery; PostgreSQL retains Serializable isolation and revision checks.
- Exact decision replay returns the stored immutable application; same-target
  hide/reject returns without domain/event/projection writes.
- Unpublished remains distinct: unsupported effects fail closed instead of being
  silently remapped. A nonretryable failure retains a failed owner-operation receipt;
  retryable storage failures keep the lease reclaimable as before.

## Regression sources

`tests/moderation_effect_contract_postgres.rs` retains rejected ordinary reply,
removed solution reply, replay/no-op and unsupported Unpublished coverage.

`tests/support/moderation_effects.rs`, mounted by
`tests/logic_review_remediation_sqlite.rs`, adds a shared matrix on both backends:
hide/reject/remove × ordinary/accepted × open/archived (12 scenarios per backend).
It checks source status, solution cleanup, topic/category/member counters, last reply,
status events, PostgreSQL projection ledger, receipt completion/replay and same-target
no-ops. An injected status-outbox failure after canonical mutations checks rollback
of source/body/revision/snapshot/solution/statistics/events before a fresh decision.
These are authored tests, not executed runtime evidence.

Existing PostgreSQL body/translation edit concurrency suites must be rerun with the
new admission order. Source ordering checks do not establish absence of deadlocks.

## Supplemental evidence and commands

The source verifier checks shared effects and lock ordering; 11 Node fixtures reject
missing solution cleanup, combined statistics, archive exemption, category/solution
fences, parent revalidation, SQLite reservation and topic invalidations. Three more
lost-response guard fixtures accept wrapped/inline replay assertions and reject their
removal; the verifier now tolerates Rust formatting without dropping the assertion.

Migration 46 is new on this branch. Its cleanup removes invalid nonpublic/deleted
solution targets before aggregate rebuild, preserving approved archived solutions.
The extracted SQL harness passes 4 SQLite + 4 PostgreSQL 17.6 cases, including
idempotency and immutable tombstone guards. This is not Rust MigrationTrait or
full-chain validation. Quiesce writers and rebuild search projections at cutover;
down never resurrects invalid relations.

```bash
npm run verify:forum:moderation-effects
npm run test:verify:forum:moderation-effects
python scripts/verify/verify-forum-topic-repair.py
# Set RUSTOK_FORUM_TEST_DATABASE_URL and install pg8000 in a test environment.
python scripts/verify/verify-forum-topic-repair.py --postgres
cargo test -p rustok-forum --test logic_review_remediation_sqlite trusted_moderation_effects
cargo test -p rustok-forum --test moderation_effect_contract_postgres
cargo test -p rustok-forum --test moderation_revision_concurrency_postgres
cargo test -p rustok-forum --test moderation_terminal_concurrency_postgres
```

Cargo/rustc are absent in the remediation environment. WASM rustfmt parsing is not
compilation; the source/SQL checks above do not establish service or application readiness.
