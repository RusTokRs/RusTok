# FORUM-20AD notification group-key population

`m20260726_000015_populate_notification_group_keys` makes grouping durable at the Notifications persistence boundary.

## Contract

- a missing group key is derived as `g1:{target_owner}:{target_id}`;
- the format is versioned and remains within the existing 191-byte storage/read contract because target owners are capped at 64 bytes and target identities are UUIDs;
- notification type and target kind are intentionally excluded, so different notification variants for one source-owned target UUID share one group;
- PostgreSQL assigns the value in a `BEFORE INSERT` trigger;
- SQLite assigns the value in an `AFTER INSERT` trigger within the same transaction;
- the migration backfills every existing `NULL` group key;
- explicit non-`NULL` group keys are preserved;
- candidate finalization may continue to submit `group_key = NULL`; persistence owns the final durable value;
- no producer descriptor, shared notifications API, inbox state, inbox timestamp, or delivery-attempt contract changes.

The migration depends on `m20260723_000014_add_outbox_intake_rejections`. Its down path removes the triggers and clears values that exactly match the derived `g1` format.

## Deliberate residual

This slice makes the existing exact-group listing usable for persisted notifications, but it does not deliver group summaries, group unread totals, latest-item projections, external transport adapters, or admin/storefront UI.

## Evidence

SQLite source evidence is in `tests/group_key_population_sqlite.rs`. It covers pre-migration backfill, new-row population, same-target grouping across notification variants and target kinds, target-owner isolation, explicit-key preservation, unchanged inbox state, and zero delivery attempts.

The static source contract is `scripts/verify/verify-forum-notification-group-key-population.mjs`, and the machine-readable Forum contract is `crates/rustok-forum/contracts/forum-notification-group-key-population.json`.

Tests, formatting, Cargo commands, verifiers, workflows, and CI were not run by the implementation agent.
