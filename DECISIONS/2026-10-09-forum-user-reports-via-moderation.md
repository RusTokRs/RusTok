# Forum user reports are filed with the Moderation owner

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`moderation_report.rs` port contract, `services/moderation_report.rs` service, `graphql/mutation.rs`, `graphql/runtime_data.rs`), `rustok-moderation-api` (subject and reason vocabulary, unchanged), `rustok-moderation` (report owner, unchanged), `apps/server` (`services/forum_moderation_report.rs` host adapter, `services/mod.rs` composition, `graphql/forum_principal_security.rs` policy)
- Extends: `DECISIONS/2026-10-09-forum-vote-subscription-write-audience.md`, `DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`
- Supersedes: None
- Superseded by: None

## Context

Forum had no way for a user to report a topic or a reply. The Moderation module already owns reports, cases,
decisions, applications, appeals, and audit, and Forum already registers its subject adapter through
`rustok-moderation-api`. The Forum crate must not depend on the `rustok-moderation` owner crate; the boundary script
`verify-forum-moderation-subject-adapter` enforces this. The owner's report command
(`ModerationCommandPort::submit_report`, `submit_report_replay_safe`) is not in the neutral API crate.

The product decision is that user reports and flags on forum content belong to `rustok-moderation`, not to Forum.

## Decision

1. **Moderation owns intake.** A user report is stored, queued, decided, and audited by `rustok-moderation`. Forum
   does not keep a report table, a queue, or a moderator view.
2. **Forum-owned port contract.** `rustok-forum` exposes `ForumModerationReportPort` in `moderation_report.rs`. The
   port takes a `ForumModerationReportCommand` made of `rustok-moderation-api` types and a `PortContext`, and returns the
   report id. Forum does not import the owner's command or record types.
3. **Host composition.** `apps/server` implements the port with `ModerationService` and publishes it through the same
   extension registry as the Forum audience facts port. The adapter sets reporter kind `User`, reporter id from the
   authenticated user, description reference `None`, and metadata `{"source": "forum"}`. The GraphQL layer reads the
   port through `ForumGraphqlRuntimeData`.
4. **Forum validates before it submits.** `ForumModerationReportService` checks, in order:
   - the caller has `forum_topics:read` (topics) or `forum_replies:read` (replies) and is an authenticated user;
   - the subject exists in the tenant and the caller can read it through the owner audience gate
     (`topic_write_audience_allows`); a missing or hidden subject returns `TopicNotFound` or `ReplyNotFound`;
   - replies must be `Approved`, the status other readers can see;
   - the caller is not the author. Self-reports return `Validation`.
5. **Subject revision.** The report carries the current subject revision read from the Forum moderation revision table
   (`forum_topic_moderation_subject_revisions`, `forum_reply_moderation_subject_revisions`), the same source the decision
   adapter compares. A later decision on a changed subject is rejected by the existing revision check.
6. **Replay-safe.** The idempotency key is `forum-report:{reporter}:{subject kind}:{subject id}:{revision}:{reason}`.
   A retried request replays the same report. The same reporter may file one report per reason per revision.
7. **Scope.** Reports use `ModerationScopeRef::platform()`. This matches the existing Forum producer path
   (`forum-moderation-lost-response`). Category-scoped routing is a follow-up that needs a Moderation scope decision.
8. **Transport.** GraphQL mutations `reportForumTopic(tenantId, topicId, reasonCode, locale)` and
   `reportForumReply(tenantId, replyId, reasonCode, locale)` return the report id. `reasonCode` is parsed with
   `ModerationReasonCode::parse`; an unknown code is rejected. The mutations are human-only: the host principal policy
   rejects service credentials for names that contain `Forum` and `Report`.
9. **Fail closed.** With no port composed (`mod-moderation` disabled or no adapter), the service returns
   `CapabilityFailure` with capability `forum.moderation_report` and code `forum.moderation_report.unavailable`. Reports
   are never dropped silently.

## Invariants

- Forum never writes a moderation row, opens a case, or applies a decision directly.
- The reporter identity comes only from the authenticated security context. Request input cannot select it.
- The reporter is not the author, and the reporter can read the subject before any submission.
- The subject revision used for the report is read in the same tenant and kind as the decision adapter reads it.

## Non-goals

- **Automatic flags (`auto_flag_threshold`).** The setting stays declared and unread. Escalation at N reports is a
  Moderation policy decision (queue priority or auto-opened case). It needs the owner's policy model and a count
  read, and is not part of this decision. The setting default is 0 in the soft-default decision.
- **Free-text description.** The owner stores a description reference, not text. This decision sends `None`. A
  description path needs a storage decision in Moderation.
- **Storefront UI.** The GraphQL mutations are the contract. The "report" action in the Next.js and Leptos frontends
  is a follow-up.
- **Moderator queue for forum reports.** Moderators use the existing Moderation queue. No Forum admin view is added.
- **REST transport.** GraphQL only in this slice.
- **Rate limiting of reports.** Not implemented. The idempotency key limits duplicates per revision and reason.

## Data, transaction, and concurrency boundary

- No schema change in Forum. The revision read runs in its own short transaction.
- The owner submission is a separate transaction in Moderation. A crash between the Forum read and the submission
  loses no data: the caller retries with the same key and gets the same report.
- A subject edited between the revision read and the submission produces a report pinned to the older revision. The
  decision on that report is then rejected by the revision check, and the moderator reviews the new content.

## Context dimensions

- Tenant: the subject lookup, revision read, and PortContext tenant are the same tenant.
- Actor: human users only. Service principals are rejected by the host policy and by the port actor.
- Locale: carried in PortContext for the audience gate.
- Channel: not part of the gate, as in the vote and subscription write gate (see the write-audience ADR).
- Policy and auth: `forum_topics:read` or `forum_replies:read`, then the owner audience gate.

## Events and projections

No Forum events are added. Moderation emits its own events through its owner path.

## Failure semantics

- Hidden or missing subject: `TopicNotFound` or `ReplyNotFound`, HTTP 404 through the existing mapping.
- Self-report: `Validation`, HTTP 400.
- Missing port: `CapabilityFailure`, HTTP 500 through the existing mapping, retryable false.
- Owner failure: the owner's `PortError` is mapped to `CapabilityFailure` with the owner code and retryable flag.

## Sources of truth and ownership

- The report, case, and decision are owned by `rustok-moderation`.
- The subject content and the revision are owned by `rustok-forum`.
- The vocabulary of subjects, scopes, and reasons is owned by `rustok-moderation-api`.

## Migration and cutover

- No data migration and no Forum schema change. Existing tenants gain the two mutations when the host runs the
  new build; no stored setting changes.
- The host adapter is composed only when both `mod-forum` and `mod-moderation` are enabled. A deployment without
  Moderation keeps the mutations and returns the fail-closed capability failure.
- Reports filed before this change do not exist, so there is nothing to backfill.

## Alternatives considered

- **Add the report command to `rustok-moderation-api` and let Forum depend on the neutral API only.** Rejected for
  this slice. The command and record types would have to move out of the owner crate, which is a wider contract change
  than this decision needs. The Forum-owned port keeps the change inside the Forum and host boundaries.
- **Forum depends on the `rustok-moderation` owner crate.** Rejected. The boundary script
  `verify-forum-moderation-subject-adapter` forbids it, and the adapter pattern exists for this reason.
- **Forum keeps its own report table and queue.** Rejected by the product decision. Reports belong to Moderation.
- **Count reports in Forum and open a case at a threshold.** Rejected for this slice. Escalation is a Moderation
  policy and needs the owner's count and policy model.
- **Client-supplied idempotency key.** Rejected. A deterministic key from reporter, subject, revision and reason
  gives replay safety without a new client contract.

## Consequences

- Moderators see user reports in the existing Moderation queue and can decide them through the existing subject
  adapter, including the revision check.
- Forum gains a write path to Moderation that is host-composed. Tests for the host adapter and the GraphQL layer are
  still needed.
- Automatic flags, free-text descriptions, report rate limits, the storefront report action and REST transport stay
  open. Each needs its own decision before it is built.

## Verification

- `crates/modules/rustok-forum/tests/moderation_report_sqlite.rs` covers: topic report with the revision and reporter
  handed to the intake, reply report with the `forum_post` subject, author self-report rejected with no intake call,
  unknown topic with no intake call, and missing intake failing closed.
- Not run in this session: no Rust toolchain and no crates.io access. The tests and the build must run in CI or in the
  integration pass before the work is treated as verified.
