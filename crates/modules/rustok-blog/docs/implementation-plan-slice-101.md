# rustok-blog implementation plan — slice 101 continuation

Status: `canonical_plan_current_cursor_source_ready_no_runtime_promotion`.

This slice continues `crates/rustok-blog/docs/implementation-plan-slice-100.md`.

## Goal

Repair the planning boundary after the historical root implementation plan fell behind the continuation series.

The repository contains a long `implementation-plan.md` whose inline current-state and completed-slice sections stop before the later slice files. By slice 100, several old phrases in that historical file had become actively misleading: they still described the remote Comments transport and cached snapshot as future implementation work, still treated the nonexistent storefront comment form as a planned fallback surface, and still described Blog category Translation PostgreSQL evidence as source work not yet retained.

Slice 101 does not rewrite the 1,000+ line historical log. Instead it creates one stable canonical current-cursor document and makes the Blog docs index point maintainers and agents to it before the historical baseline.

## Canonical current cursor

New document:

`crates/rustok-blog/docs/implementation-plan-current.md`

It records the following current source interpretations:

```text
remote_comments_transport = source_implemented_maintainer_execution_pending
category_translation_postgres = source_ready_maintainer_execution_pending
cached_public_comments_snapshot = source_ready_maintainer_execution_pending
comment_form_fallback = not_applicable_no_storefront_write_surface
```

It also records the execution gate from slice 97:

```text
source_row_and_recovery_audit_retention = blocked_until_slices_95_97_execution
```

No runtime or production result is promoted.

## Why the historical root plan is not rewritten wholesale

`implementation-plan.md` is both a current-state document and a chronological record. Its embedded completed-slice list ends before the standalone continuation series that now reaches slice 100. Replacing the whole file through the contents API solely to refresh several live-cursor paragraphs would create a large, review-hostile rewrite with high accidental-history risk.

The safer contract is:

1. preserve the historical file unchanged;
2. publish one small canonical current cursor;
3. link it from `docs/README.md` before the historical plan;
4. retain machine evidence for the exact interpretation;
5. add a fail-closed verifier and focused self-test so stale phrases cannot silently re-enter the canonical cursor.

Future maintainers may deliberately consolidate the historical log in a separately reviewed documentation migration, but current source work must use the canonical cursor first.

## Re-audited source facts

### Remote Comments

The continuation source chain after the old root cursor includes the typed remote adapter/TCP transport, server adapter/listener, host channel/provider selection, delegated user-write authorization, key/keyring lifecycle, scheduled replacement persistence/audit, canonical event contract and writer, bounded source handoff/retry/dead-letter/recovery, restart/ambiguous-commit evidence sources, and canonical Outbox relay evidence.

Slice 97 is explicitly source-ready for canonical relay PostgreSQL execution. Therefore the old root instruction `then implement the remote network transport` is superseded; another transport or relay would violate the established ownership boundary.

### Category Translation

Slice 98 retains PostgreSQL source targets for real migration up/down/up, same-revision concurrent CAS, and ordinary change-cursor recovery. The correct next result is maintainer execution and readiness recording, not another source harness for those same cases.

### Storefront fallback

Slice 99 retains the shared cached public Comments snapshot source across GraphQL/native SSR and the stale-data UI signal. Slice 100 proves the active storefront has no public Comments write surface. The old combined phrase `cached snapshot and comment-form fallback remain planned` is therefore doubly stale: one half is source-ready and the other is not applicable.

## Machine evidence

Current-cursor evidence:

`crates/rustok-blog/contracts/evidence/blog-canonical-plan-current-source.json`

Fail-closed verifier:

`scripts/verify/verify-blog-canonical-plan-current.mjs`

Focused self-test:

`scripts/verify/verify-blog-canonical-plan-current.test.mjs`

The verifier source-locks the current cursor against slices 97–100 and rejects the stale historical phrases if they appear as live instructions in the current-cursor document.

## Preserved boundaries

Slice 101 changes no production behavior. It does not change:

- Comments transport, listener, authorization, delegation, key lifecycle, replay, schedule state or persistence;
- Blog source audit, recovery, retry/dead-letter, handoff, or source-row retention;
- canonical event schema, digests, writer, `sys_events`, Outbox relay, retry or DLQ;
- Blog category schema, Translation target CAS, receipts, journal or Search reindex behavior;
- storefront DTOs, cache policy, UI rendering, routes or write surfaces;
- Blog FBA/FFA status;
- package scripts or aggregate verification-chain order.

## Explicit non-claims

This slice does not claim:

- any Rust/JavaScript verifier or test execution;
- compile/check/format/Clippy results;
- PostgreSQL, Redis, TCP, browser, HTTP, workflow, CI or production execution;
- execution of slices 95–100;
- source-row/recovery-audit retention completion;
- Translation production enablement;
- cached-snapshot runtime evidence;
- a newly discovered production source gap.

## Next cursor

The canonical current cursor intentionally names **no independent production source gap** inside the re-audited Comments/Translation/storefront tracks.

The next autonomous source slice must be justified by a fresh repository audit outside those execution-gated tracks. If maintainers execute the retained targets first, continue only from the explicit follow-up unlocked by those results.
