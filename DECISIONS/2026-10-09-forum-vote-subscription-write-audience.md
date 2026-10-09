# Forum votes and topic subscriptions require the owner audience of the parent topic

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`VoteService`, `SubscriptionService`, write audience gate, GraphQL runtime, REST controllers); `rustok-api` (`PortContext`, `ForumTopicReadTransport` and `ForumReplyReadTransport` operations)
- Extends: `DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`
- Supersedes: None
- Superseded by: None

## Context

The owner audience contract (`ForumTopicAudienceVisibilityService::is_topic_owner_visible`)
decides whether a viewer can read a topic. Read paths apply it after the 2026-10-09 read
decision. Write paths did not:

1. `VoteService::set_topic_vote` and `set_reply_vote` and `SubscriptionService::set_topic_subscription`
   and `update_topic_subscription` accepted any authenticated caller who passed the category
   visibility floor. A viewer blocked by a `minimum_trust_level`, role, or topic-local layer could
   vote on or subscribe to a topic whose content they could not read.
2. A user could vote on their own topic or reply. Nothing in the write path rejected
   `author_id == user_id`.
3. The REST `set_reply_vote` handler ran a separate owner read before the write. That read was
   the only audience check on the path and duplicated the decision the service must own.
4. Denied writes and absent topics were not guaranteed to produce the same outcome.

## Decision

1. Setting a topic vote, a reply vote, or a topic subscription requires that the caller can read
   the parent topic through the owner audience contract. The gate lives in
   `services/topic_write_audience.rs` (`topic_write_audience_allows`) and builds a
   `ForumTopicAudienceViewer::authenticated` from the caller's `SecurityContext` and a
   `PortContext`. The gate is evaluated by the service, not by the transport.
2. A denied or missing topic returns `ForumError::TopicNotFound(topic_id)`. For a reply vote, the
   gate runs on the reply's parent topic, and a denied or missing reply returns
   `ForumError::ReplyNotFound(reply_id)`. A denial and an absent target are not distinguishable.
3. Self-voting is forbidden. `set_topic_vote` and `set_reply_vote` return `ForumError::forbidden`
   when the author equals the caller. The check runs inside the write transaction after the topic
   row lock, so it reads the author value that the write observes.
4. Clearing a vote (`clear_topic_vote`, `clear_reply_vote`) and clearing a subscription
   (`clear_topic_subscription`) are not gated. They remove only the caller's own row. The clear
   path does not probe topic existence either, so clear returns the same result for a missing and
   a hidden topic. Removing the probe changes one behaviour: clearing a subscription on a missing
   topic with no row now succeeds as a no-op instead of returning `TopicNotFound`. No test relied
   on the old error.
5. Subscription writes go through a private `write_topic_subscription`. The public `update` path
   checks the audience first, then calls the private writer. The clear path calls the same
   writer directly.
6. Every transport reads the topic or reply after a vote or subscription write through the owner
   audience path (`get_authenticated_owner_visible_with_audience_context`), using a `PortContext`
   built by `topic_read_audience_port_context` or `reply_read_audience_port_context`. This applies
   to set and clear operations on REST and GraphQL. The post-write read cannot disclose a topic
   that the caller cannot read, because clear operations do not gate the write.
7. REST `set_reply_vote` and `clear_reply_vote` no longer run a preflight owner read. The service
   owns the decision, and the post-write owner read is the response.

## Sources of truth and ownership

- Topic and reply audience: `rustok-forum` (`ForumTopicAudienceVisibilityService`).
- Write authorization for votes and subscriptions: `rustok-forum` services. Transports pass a typed
  `PortContext` and do not decide access.
- Trust, channel, and group facts: the host-published `SharedForumAudienceFactsPort` (`rustok-api`).
  `VoteService::with_audience_facts` and `SubscriptionService::with_audience_facts` inject it.
  Without the port, a topic that needs richer facts is denied.

## Invariants

- A vote or subscription set for a topic the caller cannot read through the owner audience path is
  never persisted.
- A vote on a reply is authorised by the audience of the reply's parent topic.
- An author cannot vote on their own topic or reply.
- An audience denial and an absent parent produce the same error for a given target kind.

### Forbidden states

- A write path that accepts `PortContext` but does not use it for an audience decision.
- A transport that decides write access itself, by a preflight read or by a permission check that
  replaces the service gate.
- A post-write read that returns content through the non-owner path.

## Non-goals

- Channel-scoped write rules. The gate ignores the route channel: votes and subscriptions do not
  carry a channel slug, and channel membership is evaluated only when the topic carries a channel
  layer. This is a known limitation, listed in the crate README.
- Rate limits for votes and subscriptions. Only the global `/api/` limiter applies.
- Moderation and export paths.

## Data, transaction, and concurrency boundary

- The audience gate runs before the write transaction, on the owner-read connection. A policy or
  trust change that commits between the gate and the write can let one write through. The window
  is bounded to one request and is accepted for this decision. Re-evaluating the gate inside the
  transaction would require the policy and facts reads to share the write transaction. That
  change is out of scope and is recorded as a follow-up.
- The self-vote check runs inside the transaction, after `lock_active_topic_vote_write_in_tx` and
  `lock_topic_vote_scopes_in_tx`. It reads the locked topic row.
- Clear operations use the existing lock helpers and do not change their locking order.
- No schema or migration changes.

## Context dimensions

- Tenant: every call takes `tenant_id`, and the gate reads only that tenant's rows.
- Locale: post-write reads use the request locale through `PortContext` and the fallback chain.
- Channel: not used by the write gate (see Non-goals).
- Principal and auth: writes require a user actor. `ForumTopicAudienceViewer::authenticated` checks
  that the `PortContext` actor matches `SecurityContext::user_id`.
- Policy: category audience constraints from `ForumCategoryAudiencePolicy`.
- Trace and deadline: `PortContext` carries a correlation id and a read deadline.

## Events and projections

- Vote writes still publish `publish_forum_topic_projection_direct_in_tx`. Subscription writes do
  not change their existing events. No event is added or removed.

## Failure semantics

- A missing trust or channel fact for a required layer fails closed, which the gate reports as
  a denial.
- A denied vote or subscription returns not-found. It does not return forbidden, because forbidden
  would reveal that the target exists.
- Self-vote returns `forbidden`. The caller already has the topic, so the response does not leak
  existence.

## Migration and cutover

- Signatures change:
  - `VoteService::set_topic_vote(tenant_id, topic_id, security, context, value)`
  - `VoteService::set_reply_vote(tenant_id, reply_id, security, context, value)`
  - `SubscriptionService::set_topic_subscription(tenant_id, topic_id, security, context)`
  - `SubscriptionService::update_topic_subscription(tenant_id, topic_id, security, context, input)`
- Callers inside the repository are updated: GraphQL mutations, REST controllers, and the
  `rustok-forum` tests. No caller in `apps/` or other crates was found.
- REST responses for a topic that a viewer can no longer read change from 200 to 404 after a clear.
  This tightens access.
- Self-votes that succeeded before now return 403. External clients that relied on this need to
  handle it.

## Alternatives considered

- Gate only in the transport. Rejected: the GraphQL and REST paths would need the same
  code, and the Leptos server functions would be left out.
- Gate clears too. Rejected: a user who loses access must still be able to remove their own vote and
  subscription. Gating clears would also make missing and hidden topics distinguishable.
- Move the gate into the write transaction. Rejected for this change. It needs the facts and policy
  reads to run on the transaction connection, which the current facts port does not support.

## Verification

Scoped evidence for this decision:

- Runtime tests in `crates/modules/rustok-forum/tests/votes.rs`:
  `vote_and_subscription_writes_require_owner_audience_and_reject_self_votes`. It covers the
  denial of a low-trust viewer for a topic and a reply, the pass for a trusted viewer, the self-vote
  forbidden, clears without a gate, and a missing topic.
- The `write_context` helper is added to the existing vote and subscription test files. It builds a
  `PortActor::user` from `security.user_id`.

The Rust compilation and these tests were not executed in the authoring environment because the
Rust toolchain is not installed and crates.io is not reachable. They must be run by
`cargo check -p rustok-forum` and `cargo test -p rustok-forum` before merge.

## Consequences

- Votes and subscriptions on restricted topics fail for viewers outside the audience. Existing
  rows of such viewers are unaffected and can still be cleared.
- A user can no longer vote on their own content by default. This is a behaviour change visible in the UI
  and in API clients. A tenant can opt in with `allow_self_voting`; see
  `DECISIONS/2026-10-09-forum-vote-policy-settings.md`.
- Follow-up work: channel-scoped write rules, a transactional re-evaluation of the gate, rate limits
  for votes and subscriptions. Moderation solution write responses are decided in
  `DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`.
