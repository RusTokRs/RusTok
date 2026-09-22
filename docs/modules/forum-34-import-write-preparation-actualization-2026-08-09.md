# FORUM-34L bounded import write preparation actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / owner-write-adapter-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34K are merged before this slice. Fresh `main` for 34L is `1d5553f4b6bba52dd9ccee41ae6c3f5108081b6b`. The commits after 34K are Pages/Commerce work and do not overlap Forum import/export source.

Repository recheck still finds no neutral shared `ImportRunner` / `ImportJob` contract suitable for checkpoint, receipt, replay or recovery ownership. The canonical Forum implementation-plan ledger still carries the old `FORUM-34` planned cursor; this dated packet records the truthful 34L source cursor without replacing the large roadmap wholesale.

## Why 34L prepares before persistence

34K resolves source identities and dependencies, but its facts are intentionally not yet sufficient to write owner storage safely.

Existing interactive Forum create commands cannot simply be reused for migration because they generate new entity UUIDs and derive topic/reply authors from `SecurityContext`. A separate raw SeaORM writer would be worse: it would duplicate owner validation, category/topic/reply counters, state-machine behavior, outbox/domain-event semantics, taxonomy/flex hooks and future invariants.

A second recheck also found source-policy gaps that must be explicit before any write adapter exists:

- NodeBB category mapping has no owner-required category slug;
- category position may be absent or outside the owner `i32` range;
- category moderation/icon/color policy is not a source fact;
- NodeBB body text is still raw source text, not an admitted `RichTextDocument`;
- imported topic status is not currently a NodeBB source fact;
- non-deleted reply moderation status is not currently a NodeBB source fact;
- reply parent identity is not currently mapped from NodeBB;
- category timestamps can be absent from the current source mapping;
- historical import event/notification behavior must not be silently treated as interactive posting behavior.

34L therefore adds a side-effect-free **owner-write preparation** boundary rather than guessing those values inside a database writer.

## Public in-process contract

`ForumImportWritePreparer::prepare(...)` accepts `ForumImportWritePreparationRequest` containing:

- one already-resolved 34K `ForumResolvedImportApplicationBatch`;
- one explicit `ForumImportWriteEventMode`;
- exactly one category write decision per resolved category;
- exactly one topic write decision per resolved topic;
- exactly one reply write decision per resolved reply.

The request, decisions and prepared output are in-process only. No serde or transport contract is added.

The prepared output is `ForumPreparedImportWriteBatch` with explicit owner-ready category/topic/reply facts while preserving the 34K target IDs, source refs, authors and dependency identities.

## Bound and identity checks

34L keeps the existing source bound: at most `MAX_FORUM_IMPORT_WRITE_RECORDS_PER_BATCH = 512` prepared owner records across categories, topics and replies.

It rejects:

- nil tenant IDs;
- empty batches;
- invalid or non-normalized batch locale;
- nil category/topic/reply target UUIDs;
- duplicate target UUIDs within one owner kind;
- nil resolved author UUIDs;
- non-NodeBB or wrong-kind source references;
- missing, duplicate or unused preparation decisions.

The preparation boundary does not generate any entity UUID.

## Dependency recheck

34L rechecks target-side dependency closure instead of trusting a forgeable resolved DTO blindly:

- category parent IDs must point to categories in the same bounded batch;
- category target parent chains must remain acyclic;
- every topic category ID must point to a category in the same batch;
- every reply topic ID must point to a topic in the same batch;
- an explicit reply parent must point to another prepared reply in the same batch and same topic;
- self-parent replies are rejected.

Cross-batch dependency assembly remains a shared-runner concern. The current 34K/34L path intentionally accepts only self-contained application batches.

## Explicit category decisions

Each category decision must provide the facts the source mapping does not own:

- non-empty slug;
- final non-negative `i32` position;
- moderation flag;
- optional icon/color;
- explicit non-negative creation timestamp.

When NodeBB already supplied a category position, 34L does not silently rewrite it: the value must fit the owner `i32` range, remain non-negative and equal the preparation decision. When source position is absent, the caller must explicitly choose the owner position.

## Explicit topic decisions

Each topic decision provides:

- admitted `RichTextDocument` body transformation;
- explicit `TopicStatus`;
- explicit metadata;
- explicit tags;
- explicit channel slugs;
- explicit creation timestamp.

If 34K preserved a source timestamp, the decision must use exactly that timestamp. The preparer preserves the resolved topic ID, category ID, optional source author, title/slug, `body_source`, pin/lock facts and source order.

34L does not claim that a raw NodeBB body has been semantically converted merely because a caller supplied a RichText document; the source adapter/maintainer still owns that transformation policy.

## Explicit reply decisions

Each reply decision provides:

- admitted `RichTextDocument` content;
- explicit `ReplyStatus`;
- optional resolved parent reply UUID;
- explicit creation timestamp.

If a source reply is marked deleted, its prepared status must be `ReplyStatus::Deleted`. A live source reply cannot be prepared as deleted. Source timestamps, when present, cannot be rewritten by the decision.

## Event mode

`ForumImportWriteEventMode` is explicit and has no default:

- `SuppressInteractiveEvents` records the intent that historical import must not masquerade as interactive posting notifications/events;
- `EmitDomainEvents` records the intent that a future owner adapter should emit the normal domain-event path.

34L carries the decision only. It does not publish events, notifications or Search work itself.

The exact production behavior for `SuppressInteractiveEvents` still needs the owner write adapter to distinguish required consistency/rebuild signals from user-facing interactive side effects.

## Storage boundary

`import_write_preparation.rs` imports no SeaORM/database types, `SecurityContext`, owner service, event bus, runtime extension, transport or migration API. It performs no create/update/delete and no transaction.

This is deliberate. The next persistence slice must first factor or expose owner-owned in-transaction creation primitives so the import adapter can preserve admitted IDs/authors/timestamps/status while reusing Forum invariants rather than duplicating them.

## Current FORUM-34 import chain

The source-ready bounded import path is now:

`34A NodeBB mapping -> 34B/34C inspection -> 34K identity/application resolution -> 34L owner-write preparation`.

No persistence, durable checkpoint, receipt, replay or cross-batch source assembly is claimed yet.

## Next FORUM-34 cursor

The next safe slice should be **FORUM-34M**: factor the smallest Forum-owned in-transaction category/topic/reply import application primitives behind a write adapter consuming `ForumPreparedImportWriteBatch`.

That adapter must preserve category/topic/reply counters and owner validation, use caller-admitted entity IDs/authors/timestamps/status, define the explicit event-mode behavior, and apply one prepared batch atomically. It must not create a Forum-local runner/checkpoint/receipt system.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-import-write-preparation-source.mjs
```
