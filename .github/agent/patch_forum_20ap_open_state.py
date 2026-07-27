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
    "use crate::state_machine::ReplyStatus;",
    "use crate::state_machine::{ReplyStatus, TopicStatus};",
)
replace_once(
    "crates/rustok-forum/src/notification_source.rs",
    '''        if self.recipient_context_port.is_some() {
            self.load_active_topic(tenant_id, topic_id).await
        } else {
            self.load_public_topic(tenant_id, topic_id).await
        }
''',
    '''        if self.recipient_context_port.is_some() {
            let topic = self.load_active_topic(tenant_id, topic_id).await?;
            Ok(topic.filter(|topic| topic.status == TopicStatus::Open))
        } else {
            self.load_public_topic(tenant_id, topic_id).await
        }
''',
)

replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''    ForumCategoryAudiencePolicyService, ForumModule, ForumNotificationRecipientContextPort,
    ForumNotificationRecipientContextRequest, SetForumCategoryAudiencePolicyInput,
''',
    '''    ForumCategoryAudiencePolicyService, ForumModule, ForumNotificationRecipientContextPort,
    ForumNotificationRecipientContextRequest, ModerationService,
    SetForumCategoryAudiencePolicyInput,
''',
)
replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''    let topic = TopicService::new(db.clone(), event_bus)
        .create(
            tenant_id,
            admin,
''',
    '''    let topic = TopicService::new(db.clone(), event_bus.clone())
        .create(
            tenant_id,
            admin.clone(),
''',
)
replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref,
            descriptor,
''',
    '''        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref.clone(),
            descriptor: descriptor.clone(),
''',
)
replace_once(
    "crates/rustok-forum/tests/notification_topic_descriptor_materialization_sqlite.rs",
    '''    assert_eq!(page.recipients().len(), 1);
    assert_eq!(page.recipients()[0].recipient_id, allowed_recipient);
}
''',
    '''    assert_eq!(page.recipients().len(), 1);
    assert_eq!(page.recipients()[0].recipient_id, allowed_recipient);

    ModerationService::new(db.clone(), event_bus)
        .close_topic(tenant_id, topic.id, admin)
        .await
        .expect("topic should close");
    assert!(
        recipient_provider
            .describe_event(DescribeNotificationRequest {
                event: event_ref.clone(),
            })
            .await
            .expect("closed descriptor recheck should complete")
            .is_none(),
        "closed initially non-public topic must not materialize a descriptor"
    );
    let closed_page = recipient_provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event_ref,
            descriptor,
            cursor: None,
            limit: 10,
        })
        .await
        .expect("closed stale descriptor should be rechecked");
    assert!(closed_page.recipients().is_empty());
    assert!(closed_page.is_complete());
}
''',
)

replace_once(
    "scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs",
    '''  "self.load_active_topic(tenant_id, topic_id).await",
  "self.load_public_topic(tenant_id, topic_id).await",
''',
    '''  "self.load_active_topic(tenant_id, topic_id).await?",
  "topic.status == TopicStatus::Open",
  "self.load_public_topic(tenant_id, topic_id).await",
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs",
    '''  "page.recipients()[0].recipient_id, allowed_recipient",
]) {
''',
    '''  "page.recipients()[0].recipient_id, allowed_recipient",
  "closed initially non-public topic must not materialize a descriptor",
  "closed stale descriptor should be rechecked",
  "closed_page.recipients().is_empty()",
]) {
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''  "async fn load_topic_for_description(",
  "ForumTopicAudienceViewer::public()",
''',
    '''  "async fn load_topic_for_description(",
  "topic.status == TopicStatus::Open",
  "ForumTopicAudienceViewer::public()",
''',
)
replace_once(
    "scripts/verify/verify-forum-notification-visibility-composition.mjs",
    '''  "page.recipients()[0].recipient_id, allowed_recipient",
]) {
''',
    '''  "page.recipients()[0].recipient_id, allowed_recipient",
  "closed initially non-public topic must not materialize a descriptor",
  "closed stale descriptor should be rechecked",
]) {
''',
)
