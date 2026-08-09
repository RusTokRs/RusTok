# FORUM-34M bounded import relation admission actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / owner-write-adapter-open / shared-runner-blocked`

## Cursor and fresh recheck

FORUM-34A through FORUM-34L are merged before this slice. Fresh `main` for 34M is `dcd2e57e060530ad62ac9c940d97f1b67834f5a4`; there are no newer commits after that 34L merge at slice start.

Repository search still finds the canonical Forum implementation plan with the stale FORUM-34 planned cursor. The dated packet remains the truthful working cursor; the large canonical roadmap is not replaced wholesale by this slice.

## Why 34M changed after the write-path recheck

34L originally pointed at immediate in-transaction category/topic/reply import primitives. Re-reading the current owner create paths exposed an additional invariant that must be made explicit first:

- the public Topic owner create path materializes mention relations from canonical RichText before commit;
- the public Reply owner create path materializes mention/quote relations before commit;
- relation preparation currently derives moderator-audience admission and relation event actor from `SecurityContext`;
- an import operator `SecurityContext` is not the historical source author and must not be substituted for that identity;
- category owner creation also publishes full Forum projection invalidation, which is a consistency signal rather than an optional interactive notification.

A direct import INSERT after 34L would therefore still bypass established relation/projection semantics. 34M closes the relation-admission gap before any persistence adapter is introduced.

## Public in-process contract

`ForumImportRelationPreparer::prepare(...)` accepts `ForumImportRelationPreparationRequest` containing:

- one already-prepared 34L `ForumPreparedImportWriteBatch`;
- exactly one relation decision for every prepared topic;
- exactly one relation decision for every prepared reply.

The output is `ForumPreparedImportRelationBatch`, which wraps the unchanged 34L write batch and adds exact topic/reply relation facts plus the derived relation-event publication mode.

The types are in-process only. No serde, GraphQL, REST, CLI or background-job contract is added.

## Bounds and structural recheck

34M keeps the existing import bound: at most 512 topic/reply relation targets in one batch.

It independently rejects:

- nil tenant IDs;
- invalid or non-normalized batch locale;
- per-record locale drift from the batch locale;
- nil topic/reply owner target IDs;
- non-NodeBB or wrong-kind source references;
- missing, duplicate or unused relation decisions.

Topic relation targets use the admitted owner Topic UUID. Reply relation targets use the admitted owner Reply UUID. Topic `body_source` remains the NodeBB post external reference from 34K/34L and is rechecked as a NodeBB post source.

## Explicit relation mode

Every topic/reply decision selects one non-default mode:

- `SuppressRelations` — do not materialize mention/quote relation state for this imported document;
- `MaterializeRelations` — materialize only the exact admitted relation facts that match the prepared RichText.

`SuppressRelations` must contain no mention, audience or quote facts. This is an explicit historical-import policy choice, not an implicit fallback.

If the 34L batch selected `EmitDomainEvents` and the document actually contains mention targets, `SuppressRelations` is rejected. Normal event semantics cannot claim full interactive publication while silently dropping mention relation state.

## Mention admission without source-author impersonation

34M does not call `ProfilesReader` and does not use `SecurityContext`.

For `MaterializeRelations`, the caller supplies explicit `ForumImportMentionBinding { handle, user_id }` facts. The preparer:

- normalizes each handle with the same `ProfileService::normalize_handle` used by Forum mention handling;
- requires non-nil admitted user IDs;
- rejects duplicate normalized handles;
- rejects multiple handles collapsing onto one admitted user ID;
- deduplicates no caller mistakes silently;
- extracts mention candidates from the exact prepared RichText using the existing Forum mention parser;
- allows moderator-audience parsing only because the import decision is explicit, not because the operator owns moderation scope;
- requires the normalized handle set and audience set to equal the RichText projection exactly;
- retains the existing 32-target per-revision mention bound.

This validates document-to-owner relation identity without fabricating a source author or relying on current operator permissions.

## Moderator audience boundary

`ForumMentionAudience::Moderators` can be materialized only when it is actually present in the prepared RichText and explicitly admitted in the relation decision.

34M deliberately does not infer historical moderator permission. It records an explicit migration decision for later owner persistence. The import policy/runner remains responsible for deciding whether such historical audience references are acceptable for the migration being executed.

## Quote boundary remains fail-closed

The current NodeBB mapping/inspection/resolution path does not carry the owner `ForumRevisionIdentity` needed by `ForumQuoteReference` persistence.

34M therefore keeps the quote field visible in the decision contract but rejects every non-empty quote set with `QuoteRelationsUnsupported`. It also preserves the existing 32-quote bound before rejecting unsupported quote materialization.

No synthetic revision IDs, target revisions or quote aliases are generated.

A later source-mapping/identity-ledger slice must provide real quote revision identity before import quote persistence can be enabled.

## Relation event mode

34M derives one relation-event mode from the already-explicit 34L write event mode:

- `SuppressInteractiveEvents` -> `SuppressAddedTargetEvents`;
- `EmitDomainEvents` -> `EmitAddedTargetEvents`.

This matters because the current `MentionRelationService::persist_in_tx` publishes added-user/audience mention events as part of relation persistence. A future import persistence entrypoint must be able to persist relation revisions while suppressing those interactive added-target events when the import event policy requires suppression.

34M only carries that decision; it publishes nothing.

## Projection invalidation is not an interactive event

Fresh owner-path review also confirms that category owner creation publishes Forum projection invalidation before commit. Topic/reply owner mutations have their own projection/event consistency paths.

`SuppressInteractiveEvents` must **not** be interpreted as permission to suppress required consistency projections. A future 34N owner adapter must always preserve the owner projection/invalidation requirements independently of relation/user-notification event mode.

## Storage boundary

`import_relation_preparation.rs` imports no SeaORM/database connection, transaction, owner service, event bus, runtime extension or `SecurityContext` and performs no write.

It does not call `MentionRelationService::prepare`, because that path resolves current profiles and derives moderator policy/actor from `SecurityContext`. Instead it emits explicit admitted relation facts that a dedicated owner-internal import persistence entrypoint can consume later.

## Current FORUM-34 import chain

The bounded source-ready import path is now:

`34A NodeBB mapping -> 34B/34C inspection -> 34K identity/application resolution -> 34L owner-write preparation -> 34M exact relation admission`.

No persistence, durable checkpoint, receipt, replay or cross-batch assembly is claimed.

## Next FORUM-34 cursor

The next safe slice should be **FORUM-34N**: add the smallest owner-internal relation persistence bridge plus in-transaction category/topic/reply import primitives, then compose them behind one atomic bounded adapter only if all owner invariants can be reused in the same transaction.

34N must preserve, at minimum:

- category route-key/tree/position and projection invalidation rules;
- topic RichText/flex/taxonomy/channel/counter/user-stat rules;
- reply position/status/public-counter/user-stat rules;
- exact admitted IDs/authors/timestamps/status from 34L;
- 34M relation state and its added-target event mode;
- consistency projection publication even when interactive events are suppressed.

It must not introduce a Forum-local durable runner, checkpoint, receipt or replay store.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-import-relation-admission-source.mjs
```
