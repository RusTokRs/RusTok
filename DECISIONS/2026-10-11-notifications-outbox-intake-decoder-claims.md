# Notifications outbox intake selects by decoder claims, not by a module event list

- Date: 2026-10-11
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-notifications` (outbox intake worker); `apps/server` (outbox envelope decoder wiring); consumers: every module that publishes a notification source event, currently `rustok-forum`
- Extends: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`, `DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`
- Supersedes: None
- Superseded by: None

## Context

`NotificationOutboxIntakeWorker::pending_outbox_event_ids` selects the next bounded
page of committed `sys_events` rows and permanently rejects every row the decoder
cannot decode. To avoid rejecting other producers' events, the query pre-filtered
`event_type` against a list of two literals owned by another module:

```text
"forum.topic.created"
"forum.mention.user_added"
```

Two defects followed from that list living in the Notifications owner crate:

1. The consumer hard-coded a producer's vocabulary. `rustok-forum` already declares
   its notification event types through `ForumNotificationSourceProvider::supported_types`
   and `is_supported_event_type`, so the crate held a second, silently divergent
   source of truth. Every added forum notification type required an edit in the
   Notifications module, and the edit was easy to miss: the intake simply never
   selected the row, and no component reported it.
2. The gap is live. `rustok-forum` publishes `forum.topic.replied` from reply
   creation, moderation approval, restore, and import, while its notification
   provider supports only the two types above. A topic subscriber therefore gets a
   notification for a new topic and nothing for a reply, and the replied event could
   not reach intake even after the provider learned the type, because the SQL
   pre-filter would have skipped it.

## Decision

The intake keeps its bounded selection, but the claimed set moves to the decoder:

- `NotificationOutboxEnvelopeDecoder` gains a provided method
  `accepted_event_types(&self) -> Vec<String>`. An empty list claims every committed
  outbox row; a non-empty list restricts selection to those event types.
- `NotificationOutboxIntakeWorker::pending_outbox_event_ids` builds its `event_type`
  filter from `self.decoder.accepted_event_types()` and applies no type filter when
  the list is empty. The Notifications owner crate no longer names a module event type.
- The host decoder (`apps/server`) returns exactly the event types its `decode`
  implementation claims, keeping selection and decoding in one file and one owner.
- A module that adds a notification event type changes its own provider
  (`supported_types`, `is_supported_event_type`, describe and audience resolution) and
  the host decode/claim pair. The Notifications module is not part of that change.

Selection stays a pre-filter, not a routing decision: the provider remains the
authority on whether an event type is supported, exactly as
`ForumNotificationFanoutWorker` already checks `provider.supported_types()` instead
of duplicating the list.

## Sources of truth and ownership

- `rustok-forum`: the notification event vocabulary and every audience, privacy, and
  target-open decision about who may be notified (`NotificationSourceProvider`
  implementation, `forum_domain_event` rows, subscription levels).
- `rustok-notifications`: intake receipts, rejections, and the source inbox; no
  knowledge of any module event type.
- `apps/server`: composes the decoder and declares the claim set the decoder honors.
- Derived state: `notification_source_inbox` receipts are append-only per outbox
  event id and remain rebuildable by clearing receipts and re-running intake.

## Invariants

- The Notifications owner crate never contains a module-specific event-type literal.
- A row that no decoder claims is left pending: it gets neither a receipt nor a
  rejection, so registering a new claim later still allows processing.
- The claim set is a superset of the event types the same decoder can decode. A
  narrower claim set would strand events; a wider one is rejected by the provider's
  own `supported_types` check, not silently accepted.
- Batch bounds and stable ordering (`created_at`, `id`) are unchanged by the filter.

### Allowed states

- Decoder returns an empty claim list and filters while decoding.
- Decoder returns its claimed types and intake selects only those rows.

### Forbidden states

- A module event type literal inside `crates/modules/rustok-notifications`.
- Recording a permanent rejection for an outbox row that belongs to an unclaimed or
  future source.
- Moving audience or recipient decisions out of the source provider into intake.

## Non-goals

- This decision does not add a reply-notification event type, recipient set, template,
  or delivery semantics. `forum.topic.replied` delivery requires the reply recipient
  and audience rules in `rustok-forum` plus the localized template work tracked by
  `NOTIFY-06`; those remain planned in
  `crates/modules/rustok-forum/docs/implementation-plan.md`.
- It does not change the fan-out worker, which already consults the provider registry.

## Data, transaction, and concurrency boundary

No schema or migration change. Selection stays a bounded read over committed
`sys_events` rows (`created_at`, `id`) that excludes ids already present in the intake
receipt and rejection subqueries; a claim filter adds one `event_type IN (...)` predicate
to that same statement and changes no lock or ordering behavior. Intake receipts and
rejections keep their existing write transaction, so a row is claimed at most once per
processing attempt.

## Context dimensions

- Tenant: not a selection dimension. Intake scans the committed outbox globally and the
  owner provider applies tenant scope when it loads the event and resolves recipients.
- Locale: not used by intake; the host decoder carries the event payload through to the
  provider, which resolves content locale.
- Channel: not used by intake.
- Principal and auth: intake runs as the worker identity; recipient authorization stays
  in the provider's audience resolution.
- Policy: unchanged; the claim filter is a performance and safety pre-filter, not an
  authorization rule.
- Trace and deadline: unchanged worker loop, bounded batch, shared shutdown state.

## Events and projections

No new event types and no producer changes. The consumer side of
`forum.topic.created` and `forum.mention.user_added` keeps the same envelopes.
`notification_source_inbox` receipts and the derived fan-out candidates stay rebuildable:
clearing the receipts for a batch of outbox ids returns those rows to the pending set
whenever the decoder still claims them.

## Failure semantics

- A decode failure stays a permanent rejection; a retryable failure stays an error
  without a receipt. Neither path changed.
- If the host claims a type it cannot decode, intake records a rejection for those
  rows and reports it in the batch result; the claim list and the decode arms live in
  the same file so the mismatch is reviewable in one place.

## Migration and cutover

- No data migration and no backfill. Rows that were previously skipped stay skipped;
  rows that were previously rejected by a foreign event type are now left pending
  instead, which is the correction.
- Rollout order is free: a host that does not implement `accepted_event_types` gets the
  provided empty claim and behaves exactly as before, so the notification crate and the
  server binary can be deployed in either order.
- `outbox_intake_sqlite` keeps its fake decoder on the default claim, so existing test
  fixtures select the same rows.

## Alternatives considered

- Keep the event-type list in `rustok-notifications` and document it. Rejected: the list
  is a producer's vocabulary, and a second source of truth is what let a published event
  type strand silently.
- Drive the pre-filter from `NotificationSourceProvider::supported_types()`. Rejected:
  intake would then depend on module providers being registered and enabled, and an
  unregistered provider would make rows look processable while every decode still fails;
  the decoder is the component that can actually turn a row into an event.
- Remove the pre-filter and let the decoder reject foreign rows. Rejected: a non-retryable
  decode error records a permanent rejection, so a foreign row would be poisoned before
  its own consumer existed.
- Implement reply notification fan-out in the same change. Rejected: recipient and audience
  rules for replies are open plan work, and guessing them risks disclosing reply content
  to the wrong subscribers.

## Verification

- `cargo test -p rustok-notifications --test outbox_intake_sqlite` — the fake decoder
  keeps the provided default (empty claims, no type filter) and its fixtures publish
  only `forum.topic.created`, so selected and rejected sets are unchanged. Not executed
  in the authoring session: the sandbox had no Rust toolchain.
- `cargo clippy -p rustok-notifications -p server --all-targets -- -D warnings` —
  required to confirm the removed `Condition` import and the new trait default.
- `node scripts/verify/verify-notifications-fanout-worker.mjs` still passes (worker
  start order and the neutral-module rule).
- `cargo test -p rustok-forum --test notification_source_sqlite` now walks the reply
  audience across both subscription stages (author exclusion, topic-level opt-out
  suppression, deep-link route, malformed cursor) and
  `cargo test -p rustok-server --lib notification_outbox_intake_worker` covers the replied
  decode arm plus the claimed-type list. Neither command ran in the authoring session.
- `python scripts/verify/verify-remediation-gate.py --files <paths>` passes its diff
  invariants; its dependency-guard stage needs `cargo metadata` and was not run here.

## Consequences

- Adding a forum notification type is a two-file change (owner provider plus host
  decoder) instead of a cross-module edit, which removes the silent-stranding failure
  mode found while auditing reply notifications.
- The reply-notification gap that motivated this decision is closed through exactly the
  path the decision reserved: `rustok-forum` describes `forum.topic.replied` (topic-level
  watchers, then the owning category's watchers, per-recipient visibility recheck, reply
  deep link) and the host decoder claims the type. The Notifications module and its
  migrations were not touched.
- Localized template rendering stays outside this decision and is tracked as `NOTIFY-06`;
  until it lands the storefront inbox renders the stored template key, which is the
  pre-existing behaviour for every Forum notification type.
- The implementation status remains `In progress` until CI runs the Forum provider and
  `rustok-server` decoder suites; the authoring sandbox had no Rust toolchain.
