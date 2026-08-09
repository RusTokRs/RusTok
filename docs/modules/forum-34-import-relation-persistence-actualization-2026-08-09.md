# FORUM-34N owner import relation persistence actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / owner-content-adapter-open / shared-runner-blocked`

## Cursor and fresh recheck

FORUM-34A through FORUM-34M are merged before this slice. Fresh `main` for 34N is `4841cc53656a29ef66fa5c8163fa8ec27c26ea69`. The only commit after the 34M merge is Pages-only PR #3414 and does not overlap Forum import, relation, category, topic or reply source.

The canonical Forum implementation-plan ledger still carries the stale FORUM-34 planned cursor. This dated packet records the truthful 34N source cursor without replacing the large roadmap wholesale.

## Why 34N is the relation persistence bridge first

34M makes historical relation policy explicit, but the existing owner persistence service still accepts only relation facts prepared from a live `SecurityContext` plus current Profile reads. Calling that path directly during import would re-resolve historical mention identities and could attribute relation events to the migration operator instead of the admitted source author.

At the same time, duplicating `forum_relation_revisions`, mention rows, replay fingerprints and source locks in a new import writer would create a second owner implementation.

34N therefore adds a narrow owner-internal bridge into the existing `MentionRelationService` before category/topic/reply import insertion is introduced.

## Import-admitted resolved mentions

`mentions_import.rs` adds only a crate-private constructor:

`ForumResolvedMentions::from_import_admission(...)`.

It accepts already-admitted `(user_id, normalized_handle)` facts plus admitted audiences and independently rechecks:

- non-nil user IDs;
- the canonical `ProfileService::normalize_handle` result;
- that handles are already normalized rather than silently rewritten;
- unique normalized handles;
- unique user IDs across handles;
- unique audiences;
- the existing `FORUM_MAX_MENTION_TARGETS_PER_REVISION` bound.

It performs no Profile read and does not infer current visibility/status or historical permissions. 34M remains the admission boundary that decided the explicit historical identity mapping.

## Owner-internal transaction bridge

`MentionRelationService::persist_import_admitted_in_tx(...)` accepts:

- an existing `DatabaseTransaction` owned by a future Forum import adapter;
- the admitted tenant ID;
- one 34M `ForumPreparedImportContentRelations` fact;
- the exact prepared RichText document that will be persisted for the topic/reply;
- the explicit historical actor ID, if present;
- the 34M relation event mode.

The method is `pub(crate)` and is not exported as a GraphQL/REST/serde/background-job surface.

Before writing it rechecks:

- non-nil tenant/target IDs;
- non-nil explicit actor when present;
- NodeBB source namespace;
- source kind to owner target kind (`Topic -> Topic`, `Post -> Reply`);
- normalized relation locale;
- the current quote boundary: every non-empty quote set still fails closed;
- mention/audience extraction from the exact RichText using the existing Forum parser;
- exact admitted mention handles and audiences against that extraction.

## Single-owned persistence path

For `MaterializeRelations`, 34N builds the existing private `PreparedMentionRelations` and then calls the already-established `MentionRelationService::persist_in_tx`.

That means 34N does **not** reimplement:

- source row locking;
- persisted topic/reply body fingerprint verification;
- relation replay/fingerprint detection;
- relation revision allocation;
- user/audience/quote row persistence;
- previous/current projection diff calculation;
- established mention event publication.

The existing owner method remains the single implementation of those invariants.

For `SuppressRelations`, the bridge writes no relation revision and returns `None`. The suppressed decision must still contain no relation facts. If added-target events were requested while the RichText contains mention targets, the call fails closed rather than claiming interactive semantics without relation materialization.

## Added-target event mode

`SuppressAddedTargetEvents` reuses the exact same `persist_in_tx` code through an ephemeral `MentionRelationService` view with the same Profile handle but `event_bus: None`. No alternate relation storage implementation is created.

`EmitAddedTargetEvents` uses the original owner service. When the RichText contains mention targets, an owner event bus must be available or the bridge fails closed.

The actor passed into established mention event/domain-event rows is the explicit historical actor supplied by the future import content adapter. 34N never reads `SecurityContext` and never substitutes the migration operator.

Consistency projection invalidation remains separate from these interactive added-target events and is still required from the future content adapter.

## Storage boundary

34N is the first FORUM-34 import slice that can cause persistence, but only **inside a caller-owned existing transaction and only through the established relation owner method**.

It does not:

- open or commit a database transaction;
- create a DatabaseConnection;
- insert category/topic/reply rows;
- allocate imported entity UUIDs;
- write counters or UserStats;
- publish category/topic search projection invalidation itself;
- create a durable import runner, checkpoint, receipt, replay journal or migration job table.

Because `persist_in_tx` verifies the source body already exists in the same transaction, the future content adapter must insert the admitted topic/reply body before invoking the relation bridge, then commit all content/relation/counter/projection work atomically.

## Current FORUM-34 import chain

The bounded import path is now:

`34A NodeBB mapping -> 34B/34C inspection -> 34K identity/application resolution -> 34L owner-write preparation -> 34M exact relation admission -> 34N owner relation persistence bridge`.

No complete category/topic/reply import batch has been persisted yet.

## Next FORUM-34 cursor

The next safe slice should be **FORUM-34O**: factor the smallest owner-internal category/topic/reply insert primitives and compose one bounded atomic content adapter over `ForumPreparedImportRelationBatch`.

34O must preserve or reuse, at minimum:

- category tree lock, route-key admission, sibling placement and required projection invalidation;
- topic title/RichText/flex/taxonomy/channel validation and counters/UserStats;
- reply monotonic position allocation, status/public counters and UserStats;
- caller-admitted entity IDs, authors, timestamps and statuses;
- 34N relation persistence after each persisted body inside the same transaction;
- consistency projection publication even when interactive events are suppressed;
- insert-only/fail-on-existing semantics until a shared durable runner owns retry receipts.

It must not add a Forum-local checkpoint/receipt/replay system.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-import-relation-persistence-source.mjs
```
