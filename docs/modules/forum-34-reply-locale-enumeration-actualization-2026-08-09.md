# FORUM-34E bounded reply locale enumeration actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34D are merged on `main`:

- FORUM-34A maps bounded NodeBB category/topic/post source batches into runner-neutral external references without inventing RusTok target identities;
- FORUM-34B inspects unresolved in-batch and external-owner dependencies without treating an observation as persistence admission;
- FORUM-34C rejects source-local category cycles while leaving missing cross-batch parents unresolved;
- FORUM-34D maps already-authorized full Forum owner responses into tenant-scoped export fragments using canonical rich-text documents and effective locales.

The merged 34A-34D source was re-read before this slice. No ownership inversion or duplicate runner path was found. In particular, source IDs remain external references, cross-batch dependencies remain unresolved, only actual cycle members receive `cyclic_batch_relation`, and the export mapper remains side-effect free and cannot deserialize arbitrary external input as an authorized owner-view batch.

The canonical Forum ledger still says `FORUM-34` is `planned`, which is stale relative to merged 34A-34D. This dated packet records the truthful execution cursor without replacing the large concurrent canonical plan through a whole-file contents write.

Fresh `main` for this slice is `777002dac974df22d1b3374c7313812f86149a55`. The commits after FORUM-34D are Commerce and Blog only and do not modify `crates/rustok-forum/*`.

## Gap selected

FORUM-34D deliberately did not claim multilingual export completeness because `ReplyResponse` exposes one resolved locale view and does not expose the complete set of stored reply locales. Repeated fallback reads cannot prove completeness: multiple requested locales can resolve to the same stored effective locale.

The next Forum-owned slice therefore exposes exact stored locale enumeration at the existing `ReplyService` owner boundary. It does not add an import/export runner, transport or second reply read model.

## Public owner contract

`ReplyService::available_locales_for_replies` accepts:

- one explicit tenant id;
- the existing trusted `SecurityContext`;
- a caller-ordered slice of reply UUIDs.

The method:

- requires `forum_replies:list` through the existing Forum RBAC boundary;
- rejects a nil tenant id;
- accepts at most `ReplyService::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS = 512` reply IDs;
- rejects nil reply IDs and duplicate reply IDs;
- verifies every requested reply exists in the same tenant before returning locale metadata;
- loads all stored reply bodies for the bounded ID set through the existing batched `load_bodies_map` path;
- derives locales directly from stored `forum_reply_body.locale` rows through `rustok_content::available_locales_from`;
- returns one `(reply_id, locales)` pair for every requested reply in caller order;
- fails closed when an existing reply has no stored body locale.

The contract exposes stored locale identities only. It does not return reply content, rendered HTML, author identity, status, votes, solution state or counters.

## No fallback and no N+1 semantics

Locale enumeration is intentionally not a localized read. It does not call `resolve_by_locale_with_fallback`, does not accept a requested locale and does not synthesize fallback locales.

The bounded method performs one tenant-scoped reply existence query and reuses the existing batched body loader for the complete bounded ID set. It does not call `ReplyService::get` or `find_reply` once per ID and therefore does not introduce an N+1 enumeration path.

## Ownership and limits

This slice adds no:

- shared or Forum-only import/export runner;
- checkpoint, receipt, replay or audit persistence;
- import-side identity resolution;
- candidate-to-owner write adapter;
- attachment/media mapping;
- votes/reputation transfer;
- revision/history transfer policy;
- Search rebuild orchestration;
- GraphQL, REST, CLI or admin transport;
- migration or schema change.

`ReplyService` remains the Forum owner boundary. A future operator-authorized export composer can first enumerate exact locales for a bounded reply set and then perform the corresponding full owner reads under the same trusted tenant/RBAC context before constructing `ForumExportOwnerViewBatch`.

This still does not make a complete tenant export by itself. Topic/category enumeration, cursor/checkpoint semantics and the neutral shared runner remain separate open work.

## Remaining FORUM-34 scope

After FORUM-34E, still open:

- neutral shared import/export runner contract and host composition;
- durable cursor/checkpoint/receipt/replay/audit semantics;
- operator-authorized bounded export reader/composer over category/topic/reply owner services;
- cross-batch import dependency resolution;
- external-user resolution through auth/Profile owner contracts;
- candidate-to-existing Forum owner command adapter;
- revisions/history export/import policy;
- votes/reputation export/import through their owners;
- attachment/media mapping after FORUM-14 establishes Forum-owned relations;
- URL alias transfer where route-owner contracts permit it;
- runner-level dry-run/reconciliation/Search rebuild orchestration;
- CLI/admin transport only after RBAC/idempotency/audit admission;
- retained SQLite/PostgreSQL/restart/lost-response evidence.

## Maintainer validation

Per maintainer instruction, no test, Cargo command, Node verifier, formatter, migration, database scenario, workflow, CI, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-reply-locale-enumeration-source.mjs
```
