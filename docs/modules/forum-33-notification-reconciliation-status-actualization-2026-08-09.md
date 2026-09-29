# FORUM-33G Notifications reconciliation status actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / notifications-repair-owner-unchanged`

## Rechecked FORUM-33 cursor

The source sequence is now:

- FORUM-33A: bounded topic/category counter reconciliation;
- FORUM-33B: independent counter keyset continuation;
- FORUM-33C: accepted-solution and solution-stat reconciliation;
- FORUM-33D: persisted subscription reconciliation;
- FORUM-33E: persisted mention reconciliation;
- FORUM-33F: read-only Forum -> Search projection convergence status.

Attachments remain blocked on FORUM-14 because current Forum still has no persisted Forum-owned attachment relation/source-revision authority to reconcile. This slice does not infer attachment truth from Media-private state.

## Existing Notifications owner boundary

Notifications already owns FORUM-20V `NotificationInboxReconcileService`. It scans one exact-recipient non-archived page, reuses current recipient privacy and source target authorization, and archives only rows whose current open decision is `Unavailable` through the exact Notifications state owner.

That existing mutation path remains the only owner reconciliation/repair path. FORUM-33 must not copy notification rows into Forum, read Notifications private tables from Forum, or implement a second archive/retry worker.

## FORUM-33G dry-run owner inspection

`NotificationInboxReconcileService::inspect_page` now reuses the exact same bounded raw page and open-time authorization pipeline without calling the state owner.

The inspection response contains only:

```text
scanned
unavailable
next_cursor
has_more
```

It exposes no notification id, route, target owner/kind/id, template payload or delivery-attempt state.

The raw page remains exact tenant + recipient + non-archived owner state, ordered by `created_at DESC, id DESC`, with the existing default 20 / hard 64 limit and the existing bounded `i1` nanosecond + UUID cursor. Foreign privacy/source calls remain outside any Notifications transaction.

A retryable privacy/source failure fails the inspection rather than returning a partial clean result.

## Forum operator composition

The GraphQL field remains:

```text
forumNotificationReconciliationStatus(
  recipientId: UUID!,
  cursor: String,
  limit: Int
)
```

Its canonical GraphQL implementation belongs to the manifest-owned
`rustok-forum::graphql::ForumReconciliationQuery`. Forum owns the operator admission policy;
the executable server only composes the declared module query root.

Admission requires:

- runtime `forum` enabled;
- runtime `notifications` enabled;
- authenticated tenant equal to `TenantContext`;
- effective `forum_categories:manage`;
- effective `forum_topics:manage`;
- non-nil recipient UUID.

Forum resolves the optional
`NotificationInboxReconciliationInspectPort` from its manifest-attached GraphQL runtime data.
That port is published by Notifications through `ModuleRuntimeExtensions` and materialized from
the immutable host runtime context. Forum passes tenant, actor, locale, correlation and deadline
context through the neutral port and receives only the bounded inspection page.

Notifications remains Forum-agnostic: it owns the durable reconciliation implementation and the
neutral read contract, but contains no Forum-specific GraphQL field or Forum RBAC rule. Forum does
not read Notifications private persistence or instantiate `NotificationInboxReconcileService`
directly.

## Report semantics

The GraphQL response contains recipient identity plus counts and continuation metadata only. `clean` is page-local: it means `unavailable == 0` for the returned page. Whole-recipient clean status requires exhausting every cursor page with every page clean.

This is current-policy diagnostic evidence, not a snapshot across Notifications, Profiles, Social Graph and Forum source owners. Privacy/source state can legitimately change between rows or pages. The diagnostic therefore must not be used as a serializable repair fence.

## Repair and delivery boundaries

FORUM-33G performs no:

- inbox archive;
- seen/read/unread mutation;
- delivery-attempt mutation;
- retry scheduling;
- source inbox/fanout/candidate mutation;
- Forum mutation;
- direct Notifications-table read from Forum/server composition.

The existing Notifications `reconcile_page` remains the durable archive owner. Delivery-time target authorization and tenant-wide scheduled reconciliation remain Notifications product work and are not claimed by this slice.

Platform module entrypoint/span/error telemetry is emitted by the Forum GraphQL owner; the Notifications neutral port does not add a Forum-specific metric family.

## Canonical-plan drift

The canonical Forum implementation plan still lags the merged FORUM-33D/E/F execution cursor and its FORUM-20 summary also predates later Notifications owner slices. This dated packet records the actual source cursor without rewriting the large canonical document through a whole-file replacement operation.

## Next FORUM-33 cursor

Attachments remain blocked on FORUM-14.

After this Notifications dry-run status, continue with another permitted shared-owner diagnostic or a non-duplicative operational metric from the canonical list: moderation, notification/search lag, unread/activity, locale fallback or spam outcomes. Do not add a metric that duplicates an owner metric already exposed by the responsible module.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source check:

```bash
node scripts/verify/verify-forum-notification-reconciliation-status-source.mjs
```
