# Forum widget previews apply the owner audience of every topic and reply they return

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (`ForumWidgetPreviewService`, topic widget list, controllers, admin native transport); `rustok-api` (`PortContext`, `ForumTopicReadOperation`)
- Extends: `DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`
- Supersedes: None
- Superseded by: None

## Context

Page Builder widget previews (`POST /api/forum/widgets/preview` and the native
`forum/page-builder-widget-preview` server function) read Forum data for an
authenticated caller. Before this decision they did not apply the owner audience
contract:

1. `forum.topic_detail` read the topic through `TopicService::get_with_locale_fallback`.
   That path applies only the base visibility floor. A topic restricted by a
   `minimum_trust_level`, role, or topic-local layer was returned to a caller who
   could not read it through the owner transports.
2. `forum.reply_stream` listed replies by `topic_id` with no check on the parent
   topic. Replies of a restricted topic were returned.
3. `forum.topic_list` filtered only by the hidden-category set. Restricted topics
   were returned and counted in `total`.
4. The Page Builder transports constructed the service without the host-published
   audience facts port, so a richer layer could not be evaluated even where it was
   configured.

The owner read decision (2026-10-09) already fixed this for GraphQL and REST topic
reads. Widget previews are another projection of the same data and must use the same
boundary.

## Decision

1. `ForumWidgetPreviewService::preview` requires a `PortContext` built by the transport
   from the authenticated principal (`topic_read_audience_port_context` with
   `ForumTopicReadOperation::WidgetPreview`). The service first checks it with
   `ForumTopicAudienceViewer::authenticated`, so a public security context or an actor
   that differs from the caller is rejected before any data read.
2. `forum.topic_detail` reads the topic with
   `ForumTopicAudienceReadService::get_authenticated_owner_visible_with_audience_context`.
   A denied or missing topic returns `ForumError::TopicNotFound`. The `locale` prop
   overrides the request locale in a copy of the context, so the facts and the content
   use the same locale.
3. `forum.reply_stream` and the reply list of `forum.topic_detail` use
   `ForumReplyAudienceReadService::list_response_authenticated_owner_visible_with_audience_context`.
   Authorization goes through the parent topic. A denied parent returns
   `TopicNotFound`, not an empty page.
4. `forum.topic_list` uses the new `TopicService::list_widget_preview_owner_visible`.
   It walks the ordered widget candidate query in batches of 100. Each candidate is
   checked with `ForumTopicAudienceVisibilityService::is_topic_owner_visible`. A
   candidate counts toward `total` and fills the requested page only if it is visible.
   Pages have no gaps, and hidden topics are not counted.
5. The scan is capped at `FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES` (1000) candidate
   topics. A larger candidate set fails with a validation error that names the
   `category_id` narrowing. The service never returns a partial total.
6. The transports pass the host audience facts port. REST uses
   `ForumHttpRuntime::widget_preview_service()`. The admin native transport uses
   `host.shared_get::<SharedForumAudienceFactsPort>()`. `ForumWidgetPreviewService::new`
   takes `Option<SharedForumAudienceFactsPort>` as an explicit argument with no default, the
   same shape as `ForumTopicAudienceVisibilityService::new`. A transport that has the host
   port must pass `Some(port)`. `None` is valid only when the host publishes no facts port;
   a richer layer that needs owner facts then returns a `capability_unavailable` error instead of
   allowing the topic.
7. The hidden-category pre-filter is removed from the widget path. The owner audience
   check covers the category floor for every candidate, so no separate path remains.
8. The widget contract (`forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`,
   their props, and the response shape) is unchanged. The `ReplyStatus` sets for
   `approved_only` and moderator previews are unchanged. The moderation scope check on
   non-approved reply streams is unchanged.

## Sources of truth and ownership

- Topic and reply audience: `rustok-forum` (`ForumTopicAudienceVisibilityService`).
- Widget configuration: `ForumWidgetContractService` (unchanged).
- Trust, Channel, and group facts: the host-published `SharedForumAudienceFactsPort`.
- Page Builder: consumes the JSON projection only and owns no visibility decision.

## Invariants

- A widget preview never returns a topic or reply that the caller cannot read through
  the owner audience path.
- `total` and pagination of the topic list count only topics the caller can read.
- A denied topic detail and a missing topic produce the same error.
- The audience context actor always equals the security context user.

### Forbidden states

- A widget path that reads topics or replies through the storefront, base-floor, or
  system-context services.
- A widget list whose `total` includes topics the caller cannot read.
- A preview service constructed without the facts port by a transport that has one.

## Non-goals

- Pushing the audience filter into SQL. Richer layers need trust, Channel, and group
  facts that the database does not hold, so they are evaluated per candidate in Rust.
- Route-channel scoping for previews. The preview does not take a channel slug, so
  channel-restricted topics are judged by their audience layers only.
- A cached total. The scan is recomputed on every preview.

## Data, transaction, and concurrency boundary

No schema or migration change. The preview is a read. Each candidate check reads the
topic and its policy separately, so a policy change between two checks can make the
total and the page reflect different policy states. This is accepted for a preview
and matches the existing owner list behaviour.

## Context dimensions

- Tenant: every call takes `tenant_id`; the candidate query is scoped to it.
- Locale: the request locale is in the `PortContext`. The topic-detail `locale` prop overrides it for that widget only.
- Channel: not used (see Non-goals).
- Principal and auth: REST uses `AuthContext`; the admin native transport uses the same extractor. Both require `forum_topics:read`.
- Policy: category and topic audience constraints via `ForumTopicAudienceVisibilityService`.
- Trace and deadline: `PortContext` carries a correlation id and a five-second deadline.

## Events and projections

No event changes. Widget previews are not cached or projected.

## Failure semantics

- A missing trust or Channel fact for a required layer fails closed, which denies the candidate or the topic.
- A candidate count above the cap returns a validation error. It does not return a partial result.
- Invalid widget props return the existing `valid: false` response with status 422 at REST. This is unchanged.

## Migration and cutover

- Previews of restricted topics change: the detail and reply widgets return an error
  (REST 404) where they previously returned the content; the list widget omits those
  topics and their `total` drops for restricted viewers. This tightens access and is
  not a compatibility regression.
- Previews of restricted topics by viewers who can read them are unchanged.
- The `total` for a list preview may differ from the previous database count for
  viewers who cannot read some topics. Page Builder shows whatever the owner returns.
- A list preview over more than 1000 candidates now fails until `category_id` narrows
  it. Operators who relied on unbounded preview totals must narrow the widget.

## Alternatives considered

- Keep the hidden-category pre-filter and check only the detail and reply paths.
  Rejected: the list would still leak restricted topics and count them.
- Filter in SQL with the hidden-category set plus a topic-level join. Rejected: richer
  layers depend on facts that are not in the database.
- Return an empty reply page for a denied parent topic, as the storefront does. Rejected:
  the owner boundary returns `TopicNotFound`, and the preview must match the owner
  transports rather than invent a third outcome.

## Verification

Scoped evidence for this decision:

- New runtime tests in `crates/modules/rustok-forum/tests/widget_preview_audience_sqlite.rs`:
  - list total and pagination count only owner-visible topics for a low-trust and a trusted viewer;
  - topic detail denies a restricted topic as `TopicNotFound` and allows the trusted viewer;
  - reply stream denies replies of a restricted topic as `TopicNotFound`;
  - a preview with an audience context for another actor is rejected as validation.
- Existing verify markers in `scripts/verify/verify-forum-page-builder-runtime-authorization-evidence.mjs`
  for `widget_preview.rs` (`reply_stream_preview_statuses`, `PermissionScope::None`, the moderator status set) are preserved.

The Rust compilation and these tests were not executed in the authoring environment
because the Rust toolchain is not installed and crates.io is not reachable. They must
be run by `cargo check -p rustok-forum` and `cargo test -p rustok-forum --test widget_preview_audience_sqlite`
before merge.

## Consequences

- Restricted topics and replies disappear from Page Builder previews for viewers who cannot read them.
- Topic-list previews on large categories need a `category_id` narrowing. The cap is a deliberate limit, not a silent truncation.
- Follow-up work: a single owner-filtered candidate query with the facts resolved once per batch, which would remove the per-candidate policy reads.
