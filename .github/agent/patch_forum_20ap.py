from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    source = target.read_text()
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}")
    target.write_text(source.replace(old, new, 1))


replace_once(
    "crates/rustok-forum/src/notification_source.rs",
    '''    async fn load_public_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        self.load_topic_for_viewer(tenant_id, topic_id, &ForumTopicAudienceViewer::public())
            .await
    }

    async fn load_topic_for_subscription_audience(
''',
    '''    async fn load_public_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        self.load_topic_for_viewer(tenant_id, topic_id, &ForumTopicAudienceViewer::public())
            .await
    }

    async fn load_topic_for_description(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
    ) -> NotificationProviderResult<Option<forum_topic::Model>> {
        if self.recipient_context_port.is_some() {
            self.load_active_topic(tenant_id, topic_id).await
        } else {
            self.load_public_topic(tenant_id, topic_id).await
        }
    }

    async fn load_topic_for_subscription_audience(
''',
)
replace_once(
    "crates/rustok-forum/src/notification_source.rs",
    '''                let Some(topic) = self
                    .load_public_topic(event.tenant_id, event.aggregate_id)
                    .await?
''',
    '''                let Some(topic) = self
                    .load_topic_for_description(event.tenant_id, event.aggregate_id)
                    .await?
''',
)

replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''        Ok(PortContext::new(
            request.tenant_id.to_string(),
            PortActor::user(request.recipient_id.to_string()),
            context.locale,
            context.correlation_id,
        )
        .with_role(*role)
        .with_claim(Permission::FORUM_TOPICS_READ.to_string())
        .with_deadline_ms(context.deadline_ms))
''',
    '''        let mut recipient = PortContext::new(
            request.tenant_id.to_string(),
            PortActor::user(request.recipient_id.to_string()),
            context.locale.clone(),
            context.correlation_id.clone(),
        )
        .with_role(*role)
        .with_claim(Permission::FORUM_TOPICS_READ.to_string());
        recipient.causation_id = context.causation_id;
        recipient.traceparent = context.traceparent;
        recipient.deadline_ms = context.deadline_ms;
        Ok(recipient)
''',
)
replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''    assert_eq!(descriptor.target.id, topic.id);
    assert_eq!(descriptor.template_data.len(), 2);
''',
    '''    let topic_id = topic.id.to_string();
    let category_id = category.id.to_string();
    assert_eq!(descriptor.target.id, topic.id);
    assert_eq!(descriptor.template_data.len(), 2);
''',
)
replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''        Some(topic.id.to_string().as_str())
''',
    '''        Some(topic_id.as_str())
''',
)
replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''        Some(category.id.to_string().as_str())
''',
    '''        Some(category_id.as_str())
''',
)

replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    '''| `FORUM-20` | `in_progress` | FORUM-20A-AO provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh. Write audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |
''',
    '''| `FORUM-20` | `in_progress` | FORUM-20A-AP provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh; FORUM-20AP materializes initially non-public topic-created descriptors behind exact recipient capability. Write audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |
''',
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    '''- clear prior mutation feedback when the auth scope changes while preserving explicit
  post-command refresh, compile-profile transport selection, and no-fallback behavior.

### Compatibility and degraded mode
''',
    '''- clear prior mutation feedback when the auth scope changes while preserving explicit
  post-command refresh, compile-profile transport selection, and no-fallback behavior.

### Delivered in `FORUM-20AP`

- materialize `forum.topic.created` descriptors for active topics that are already non-public
  when the host publishes the exact notification recipient-context capability;
- keep public-only descriptor creation when that capability is absent, preserving the existing
  fail-closed optional-module profile;
- expose only topic/category identifiers in the descriptor and defer all recipient authority to
  the bounded subscription audience recheck;
- preserve author exclusion, raw subscription cursor progress, current category/topic audience
  evaluation, non-oracular inactive targets, and later target-open authorization.

### Compatibility and degraded mode
''',
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    '''- materialize descriptors for topics that are already non-public at creation time without
  weakening recipient-specific reauthorization;
''',
    '''''',
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    '''node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
''',
    '''node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
''',
)

replace_once(
    "crates/rustok-notifications/docs/implementation-plan.md",
    '''action exposes the localized Notifications route and exact unread badge. Tenant-wide
scheduling/redaction, delivery transports, and PostgreSQL cross-consumer evidence remain open.
''',
    '''action exposes the localized Notifications route and exact unread badge. Forum topic-created
sources also materialize minimal initially non-public descriptors when exact recipient context is
composed, while audience pages still reauthorize every subscriber. Tenant-wide scheduling/redaction,
delivery transports, and PostgreSQL cross-consumer evidence remain open.
''',
)
replace_once(
    "crates/rustok-notifications/docs/implementation-plan.md",
    '''### `FORUM-20AO`

- grouped bootstrap source combines the manual refresh nonce and the reactive transport context;
- auth token, tenant, sign-in, sign-out, and refresh-session changes trigger automatic reload;
- one context snapshot is reused for exact unread count and the first bounded summary page;
- auth-scope changes clear prior mutation feedback without polling or shadow client state.

## Remaining `NOTIFY-01`
''',
    '''### `FORUM-20AO`

- grouped bootstrap source combines the manual refresh nonce and the reactive transport context;
- auth token, tenant, sign-in, sign-out, and refresh-session changes trigger automatic reload;
- one context snapshot is reused for exact unread count and the first bounded summary page;
- auth-scope changes clear prior mutation feedback without polling or shadow client state.

### `FORUM-20AP`

- active initially non-public topic-created descriptors materialize only when the host publishes
  exact Forum notification recipient context;
- descriptors contain topic/category identifiers only and do not carry title, body, route, or
  recipient identity;
- bounded category subscription fanout still reauthorizes current Forum visibility for every
  exact candidate and retains public-only fallback when recipient context is absent.

## Remaining `NOTIFY-01`
''',
)
replace_once(
    "crates/rustok-notifications/docs/implementation-plan.md",
    '''node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
''',
    '''node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
''',
)
replace_once(
    "crates/rustok-notifications/docs/implementation-plan.md",
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO` source and documentation slices. `Cargo.lock` was
''',
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO/20AP` source and documentation slices. `Cargo.lock` was
''',
)

replace_once(
    "crates/rustok-notifications/README.md",
    '''Source contracts are guarded by the `FORUM-20AG` through `FORUM-20AO` machine contracts and
''',
    '''Source contracts are guarded by the `FORUM-20AG` through `FORUM-20AP` machine contracts and
''',
)
replace_once(
    "crates/rustok-notifications/README.md",
    '''and semantic identities derived from committed envelopes. Mention processing
still verifies the exact immutable relation and current topic/reply visibility.
Moderator audience expansion remains deferred until a bounded owner directory
''',
    '''and semantic identities derived from committed envelopes. With exact recipient context,
active initially non-public topic-created events materialize identifier-only descriptors and
still reauthorize every bounded subscription candidate. Mention processing verifies the exact
immutable relation and current topic/reply visibility. Moderator audience expansion remains
deferred until a bounded owner directory
''',
)

replace_once(
    "crates/rustok-notifications/docs/README.md",
    '''committed envelopes. Mention handling verifies immutable relation and current target
visibility. Pending replies are retryable; closed, hidden, deleted, self-mentioned,
or restricted sources fail closed. Moderator audience expansion remains deferred.
''',
    '''committed envelopes. With exact recipient context, active initially non-public topic-created
events materialize identifier-only descriptors and still reauthorize every bounded subscription
candidate. Mention handling verifies immutable relation and current target visibility. Pending
replies are retryable; closed, hidden, deleted, self-mentioned, or restricted sources fail closed.
Moderator audience expansion remains deferred.
''',
)
replace_once(
    "crates/rustok-notifications/docs/README.md",
    '''node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
''',
    '''node scripts/verify/verify-forum-notification-inbox-auth-reactive-bootstrap.mjs
node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs
node scripts/verify/verify-forum-notification-plan-sync.mjs
''',
)
replace_once(
    "crates/rustok-notifications/docs/README.md",
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO`.
''',
    '''`FORUM-20R/20S/20T/20U/20V/20W/20X/20Y/20Z/20AA/20AB/20AC/20AD/20AE/20AF/20AG/20AH/20AI/20AJ/20AK/20AL/20AM/20AN/20AO/20AP`.
''',
)

replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''const subscriptionTestSource = read(contract.recipient_topic_subscription_test_file ?? "");
''',
    '''const subscriptionTestSource = read(contract.recipient_topic_subscription_test_file ?? "");
const descriptorContract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-topic-descriptor-materialization.json") ||
    "{}",
);
const descriptorTestSource = read(descriptorContract.runtime_test_file ?? "");
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''  "async fn load_public_topic(",
  "ForumTopicAudienceViewer::public()",
''',
    '''  "async fn load_public_topic(",
  "async fn load_topic_for_description(",
  "ForumTopicAudienceViewer::public()",
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''  [describeBlock, "load_public_topic(event.tenant_id, event.aggregate_id)", "topic-created public description"],
''',
    '''  [describeBlock, "load_topic_for_description(event.tenant_id, event.aggregate_id)", "topic-created capability-gated description"],
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''for (const marker of [
  "topic_subscription_audience_filters_exact_recipients_before_cursor_progress",
''',
    '''for (const marker of [
  "initially_non_public_topic_descriptor_requires_recipient_capability_and_reauthorizes",
  "without recipient capability an initially non-public topic must remain absent",
  "active initially non-public topic should materialize a descriptor",
  "page.recipients()[0].recipient_id, allowed_recipient",
]) {
  requireText(descriptorTestSource, marker, `topic descriptor SQLite scenario is missing ${marker}`);
}
for (const marker of [
  "topic_subscription_audience_filters_exact_recipients_before_cursor_progress",
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''if (
  subscriptionContract.schema_version !== 1 ||
''',
    '''if (
  descriptorContract.schema_version !== 1 ||
  descriptorContract.task !== "FORUM-20AP" ||
  descriptorContract.upstream_task !== "FORUM-20AO" ||
  descriptorContract.composition?.topic_created_descriptor_materialization !== true ||
  descriptorContract.composition?.exact_recipient_subscription_reauthorization !== true
) {
  failures.push("FORUM-20K visibility composition must recognize the downstream FORUM-20AP descriptor closure");
}
if (
  subscriptionContract.schema_version !== 1 ||
''',
)

replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '''  "FORUM-20A-AO provide",
''',
    '''  "FORUM-20A-AP provide",
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '''  "### Delivered in `FORUM-20AO`",
  "PostgreSQL concurrency",
''',
    '''  "### Delivered in `FORUM-20AO`",
  "### Delivered in `FORUM-20AP`",
  "PostgreSQL concurrency",
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '''  "### `FORUM-20AO`",
]) {
''',
    '''  "### `FORUM-20AO`",
  "### `FORUM-20AP`",
]) {
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '''  "automatically reloads its bootstrap",
]) {
  requireText(owner, marker, `Notifications owner README is missing ${marker}`);
''',
    '''  "automatically reloads its bootstrap",
  "initially non-public topic-created events materialize identifier-only descriptors",
]) {
  requireText(owner, marker, `Notifications owner README is missing ${marker}`);
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '''  "automatically reloads its bootstrap",
]) {
  requireText(live, marker, `Notifications live contract is missing ${marker}`);
''',
    '''  "automatically reloads its bootstrap",
  "initially non-public topic-created",
]) {
  requireText(live, marker, `Notifications live contract is missing ${marker}`);
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '''console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AO.");
''',
    '''console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AP.");
''',
)
