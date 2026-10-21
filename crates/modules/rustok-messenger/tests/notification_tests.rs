use chrono::{Duration as ChronoDuration, Utc};
use rustok_core::{ModuleRuntimeExtensions, RusToKModule};
use rustok_messenger::entities::{conversation, member, message};
use rustok_messenger::notification_source::{
    MESSENGER_CONVERSATION_TARGET, MESSENGER_SOURCE, MESSAGE_DISPATCHED_TYPE,
    MessengerNotificationSourceProvider,
    message_dispatched_type, messenger_conversation_target_kind, messenger_source_slug,
};
use rustok_messenger::module::MessengerModule;
use rustok_notifications_api::{
    AuthorizeNotificationTargetRequest, DescribeNotificationRequest,
    NotificationOpenAuthorization, NotificationPriority, NotificationSemanticDescriptor,
    NotificationSourceEventRef, NotificationSourceProvider, NotificationSourceSlug,
    NotificationTargetKind, NotificationTargetRef, NotificationTemplateData,
    NotificationTemplateKey, ResolveNotificationAudienceRequest,
    ensure_notification_source_factory_registry,
};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite must connect");

    let manager = SchemaManager::new(&db);
    for migration in rustok_messenger::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("messenger migration must succeed");
    }

    db
}

#[test]
fn test_notification_semantic_keys_and_registration() {
    assert_eq!(messenger_source_slug().as_str(), MESSENGER_SOURCE);
    assert_eq!(message_dispatched_type().as_str(), MESSAGE_DISPATCHED_TYPE);
    assert_eq!(
        messenger_conversation_target_kind().as_str(),
        MESSENGER_CONVERSATION_TARGET
    );

    let mut extensions = ModuleRuntimeExtensions::default();
    let module = MessengerModule;
    assert!(module.register_runtime_extensions(&mut extensions).is_ok());

    let registry = ensure_notification_source_factory_registry(&mut extensions);
    assert_eq!(registry.len(), 1);
}

#[tokio::test]
async fn test_provider_contract_metadata() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db);

    assert_eq!(provider.slug().as_str(), "messenger");
    assert_eq!(provider.display_name(), "Messenger");
    assert_eq!(
        provider.supported_types(),
        vec![message_dispatched_type()]
    );
}

#[tokio::test]
async fn test_describe_event_unencrypted_message() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let conv_id = Uuid::now_v7();
    let sender_id = Uuid::new_v4();
    let now = Utc::now();

    conversation::ActiveModel {
        id: Set(conv_id),
        tenant_id: Set(tenant_id),
        conversation_type: Set(2),
        title: Set(Some("Dev Team".to_string())),
        entity_type: Set(None),
        entity_id: Set(None),
        is_encrypted: Set(false),
        created_by: Set(sender_id),
        last_message_id: Set(Some(1001)),
        last_message_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("conversation must insert");

    message::ActiveModel {
        id: Set(1001),
        tenant_id: Set(tenant_id),
        conversation_id: Set(conv_id),
        sender_id: Set(sender_id),
        message_type: Set(1),
        content: Set("We deployed the latest patch to staging.".to_string()),
        blocks_json: Set(None),
        is_encrypted: Set(false),
        encryption_algo: Set(None),
        reply_to_id: Set(None),
        created_at: Set(now),
        deleted_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("message must insert");

    let event = NotificationSourceEventRef::new(
        tenant_id,
        conv_id,
        messenger_source_slug(),
        message_dispatched_type(),
        1001,
    )
    .expect("source event ref is valid");

    let descriptor = provider
        .describe_event(DescribeNotificationRequest { event })
        .await
        .expect("describe_event succeeds")
        .expect("descriptor is Some");

    assert_eq!(descriptor.notification_type, message_dispatched_type());
    assert_eq!(descriptor.target.id, conv_id);
    assert_eq!(descriptor.target.owner.as_str(), "messenger");
    assert_eq!(descriptor.target.kind.as_str(), "messenger.conversation");
    assert_eq!(descriptor.actor_id, Some(sender_id));

    let preview = descriptor.template_data.get("message_preview").unwrap();
    assert_eq!(preview, "We deployed the latest patch to staging.");
    let title = descriptor.template_data.get("conversation_title").unwrap();
    assert_eq!(title, "Dev Team");
}

#[tokio::test]
async fn test_describe_event_encrypted_message_ensures_zero_plaintext_leak() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let conv_id = Uuid::now_v7();
    let sender_id = Uuid::new_v4();
    let now = Utc::now();

    conversation::ActiveModel {
        id: Set(conv_id),
        tenant_id: Set(tenant_id),
        conversation_type: Set(1),
        title: Set(None),
        entity_type: Set(None),
        entity_id: Set(None),
        is_encrypted: Set(true),
        created_by: Set(sender_id),
        last_message_id: Set(Some(2001)),
        last_message_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("conversation must insert");

    let sensitive_content = "CONFIDENTIAL_PRIVATE_PLAINTEXT_SECRET";
    message::ActiveModel {
        id: Set(2001),
        tenant_id: Set(tenant_id),
        conversation_id: Set(conv_id),
        sender_id: Set(sender_id),
        message_type: Set(1),
        content: Set(sensitive_content.to_string()),
        blocks_json: Set(None),
        is_encrypted: Set(true),
        encryption_algo: Set(Some("m.megolm.v1.aes-sha2".to_string())),
        reply_to_id: Set(None),
        created_at: Set(now),
        deleted_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("message must insert");

    let event = NotificationSourceEventRef::new(
        tenant_id,
        conv_id,
        messenger_source_slug(),
        message_dispatched_type(),
        2001,
    )
    .expect("source event ref is valid");

    let descriptor = provider
        .describe_event(DescribeNotificationRequest { event })
        .await
        .expect("describe_event succeeds")
        .expect("descriptor is Some");

    let preview = descriptor.template_data.get("message_preview").unwrap();
    assert_eq!(preview, "New encrypted message");

    // Zero-leak invariant: plaintext must NEVER appear in template data
    for (k, v) in descriptor.template_data.into_inner() {
        assert!(
            !v.contains(sensitive_content),
            "Key {k} leaked plaintext: {v}"
        );
    }
}

#[tokio::test]
async fn test_describe_event_deleted_message_returns_none() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let conv_id = Uuid::now_v7();
    let sender_id = Uuid::new_v4();
    let now = Utc::now();

    conversation::ActiveModel {
        id: Set(conv_id),
        tenant_id: Set(tenant_id),
        conversation_type: Set(2),
        title: Set(Some("Public".to_string())),
        entity_type: Set(None),
        entity_id: Set(None),
        is_encrypted: Set(false),
        created_by: Set(sender_id),
        last_message_id: Set(Some(3001)),
        last_message_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("conversation must insert");

    message::ActiveModel {
        id: Set(3001),
        tenant_id: Set(tenant_id),
        conversation_id: Set(conv_id),
        sender_id: Set(sender_id),
        message_type: Set(1),
        content: Set("Soft deleted message".to_string()),
        blocks_json: Set(None),
        is_encrypted: Set(false),
        encryption_algo: Set(None),
        reply_to_id: Set(None),
        created_at: Set(now),
        deleted_at: Set(Some(now)), // Marked deleted
    }
    .insert(&db)
    .await
    .expect("message must insert");

    let event = NotificationSourceEventRef::new(
        tenant_id,
        conv_id,
        messenger_source_slug(),
        message_dispatched_type(),
        3001,
    )
    .expect("source event ref is valid");

    let descriptor = provider
        .describe_event(DescribeNotificationRequest { event })
        .await
        .expect("describe_event succeeds");

    assert!(descriptor.is_none());
}

#[tokio::test]
async fn test_resolve_audience_excludes_sender_and_muted_members() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let conv_id = Uuid::now_v7();
    let sender_id = Uuid::new_v4();
    let active_member_1 = Uuid::new_v4();
    let active_member_2 = Uuid::new_v4();
    let muted_member = Uuid::new_v4();
    let now = Utc::now();

    conversation::ActiveModel {
        id: Set(conv_id),
        tenant_id: Set(tenant_id),
        conversation_type: Set(2),
        title: Set(Some("Audience Test".to_string())),
        entity_type: Set(None),
        entity_id: Set(None),
        is_encrypted: Set(false),
        created_by: Set(sender_id),
        last_message_id: Set(None),
        last_message_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();

    // 1. Sender member
    member::ActiveModel {
        id: Set(Uuid::now_v7()),
        conversation_id: Set(conv_id),
        user_id: Set(sender_id),
        role: Set(1),
        joined_at: Set(now),
        muted_until: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    // 2. Active member 1
    member::ActiveModel {
        id: Set(Uuid::now_v7()),
        conversation_id: Set(conv_id),
        user_id: Set(active_member_1),
        role: Set(1),
        joined_at: Set(now),
        muted_until: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    // 3. Active member 2
    member::ActiveModel {
        id: Set(Uuid::now_v7()),
        conversation_id: Set(conv_id),
        user_id: Set(active_member_2),
        role: Set(1),
        joined_at: Set(now),
        muted_until: Set(Some(now - ChronoDuration::hours(1))), // Expired mute
    }
    .insert(&db)
    .await
    .unwrap();

    // 4. Muted member (currently muted for 2 hours)
    member::ActiveModel {
        id: Set(Uuid::now_v7()),
        conversation_id: Set(conv_id),
        user_id: Set(muted_member),
        role: Set(1),
        joined_at: Set(now),
        muted_until: Set(Some(now + ChronoDuration::hours(2))),
    }
    .insert(&db)
    .await
    .unwrap();

    let event = NotificationSourceEventRef::new(
        tenant_id,
        conv_id,
        messenger_source_slug(),
        message_dispatched_type(),
        1001,
    )
    .unwrap();

    let descriptor = NotificationSemanticDescriptor {
        notification_type: message_dispatched_type(),
        template_key: NotificationTemplateKey::new(MESSAGE_DISPATCHED_TYPE).unwrap(),
        target: NotificationTargetRef {
            owner: messenger_source_slug(),
            kind: messenger_conversation_target_kind(),
            id: conv_id,
        },
        actor_id: Some(sender_id),
        priority: NotificationPriority::Normal,
        template_data: NotificationTemplateData::default(),
    };

    let audience = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event,
            descriptor,
            cursor: None,
            limit: 50,
        })
        .await
        .expect("audience resolution succeeds");

    let recipient_ids: Vec<Uuid> = audience
        .recipients()
        .iter()
        .map(|c| c.recipient_id)
        .collect();

    assert_eq!(recipient_ids.len(), 2);
    assert!(recipient_ids.contains(&active_member_1));
    assert!(recipient_ids.contains(&active_member_2));
    assert!(!recipient_ids.contains(&sender_id), "Sender must be excluded from audience");
    assert!(!recipient_ids.contains(&muted_member), "Muted member must be excluded from audience");
    assert!(audience.is_complete());
}

#[tokio::test]
async fn test_resolve_audience_cursor_pagination() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let conv_id = Uuid::now_v7();
    let sender_id = Uuid::new_v4();
    let now = Utc::now();

    conversation::ActiveModel {
        id: Set(conv_id),
        tenant_id: Set(tenant_id),
        conversation_type: Set(2),
        title: Set(Some("Pagination Test".to_string())),
        entity_type: Set(None),
        entity_id: Set(None),
        is_encrypted: Set(false),
        created_by: Set(sender_id),
        last_message_id: Set(None),
        last_message_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();

    let mut members = Vec::new();
    for _ in 0..5 {
        let user_id = Uuid::new_v4();
        members.push(user_id);
        member::ActiveModel {
            id: Set(Uuid::now_v7()),
            conversation_id: Set(conv_id),
            user_id: Set(user_id),
            role: Set(1),
            joined_at: Set(now),
            muted_until: Set(None),
        }
        .insert(&db)
        .await
        .unwrap();
    }
    members.sort();

    let event = NotificationSourceEventRef::new(
        tenant_id,
        conv_id,
        messenger_source_slug(),
        message_dispatched_type(),
        1001,
    )
    .unwrap();

    let descriptor = NotificationSemanticDescriptor {
        notification_type: message_dispatched_type(),
        template_key: NotificationTemplateKey::new(MESSAGE_DISPATCHED_TYPE).unwrap(),
        target: NotificationTargetRef {
            owner: messenger_source_slug(),
            kind: messenger_conversation_target_kind(),
            id: conv_id,
        },
        actor_id: Some(sender_id),
        priority: NotificationPriority::Normal,
        template_data: NotificationTemplateData::default(),
    };

    // First page with limit 2
    let page1 = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event.clone(),
            descriptor: descriptor.clone(),
            cursor: None,
            limit: 2,
        })
        .await
        .expect("page 1 succeeds");

    assert_eq!(page1.recipients().len(), 2);
    assert_eq!(page1.recipients()[0].recipient_id, members[0]);
    assert_eq!(page1.recipients()[1].recipient_id, members[1]);
    assert!(!page1.is_complete());
    let cursor1 = page1.next_cursor().expect("cursor must exist").clone();

    // Second page with limit 2
    let page2 = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event: event.clone(),
            descriptor: descriptor.clone(),
            cursor: Some(cursor1),
            limit: 2,
        })
        .await
        .expect("page 2 succeeds");

    assert_eq!(page2.recipients().len(), 2);
    assert_eq!(page2.recipients()[0].recipient_id, members[2]);
    assert_eq!(page2.recipients()[1].recipient_id, members[3]);
    assert!(!page2.is_complete());
    let cursor2 = page2.next_cursor().expect("cursor must exist").clone();

    // Third page with limit 2 (final item)
    let page3 = provider
        .resolve_audience(ResolveNotificationAudienceRequest {
            event,
            descriptor,
            cursor: Some(cursor2),
            limit: 2,
        })
        .await
        .expect("page 3 succeeds");

    assert_eq!(page3.recipients().len(), 1);
    assert_eq!(page3.recipients()[0].recipient_id, members[4]);
    assert!(page3.is_complete());
}

#[tokio::test]
async fn test_authorize_target_open() {
    let db = setup_test_db().await;
    let provider = MessengerNotificationSourceProvider::new(db.clone());

    let tenant_id = Uuid::new_v4();
    let conv_id = Uuid::now_v7();
    let member_user_id = Uuid::new_v4();
    let non_member_user_id = Uuid::new_v4();
    let now = Utc::now();

    conversation::ActiveModel {
        id: Set(conv_id),
        tenant_id: Set(tenant_id),
        conversation_type: Set(2),
        title: Set(Some("Target Open Test".to_string())),
        entity_type: Set(None),
        entity_id: Set(None),
        is_encrypted: Set(false),
        created_by: Set(member_user_id),
        last_message_id: Set(None),
        last_message_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .unwrap();

    member::ActiveModel {
        id: Set(Uuid::now_v7()),
        conversation_id: Set(conv_id),
        user_id: Set(member_user_id),
        role: Set(1),
        joined_at: Set(now),
        muted_until: Set(None),
    }
    .insert(&db)
    .await
    .unwrap();

    let target = NotificationTargetRef {
        owner: messenger_source_slug(),
        kind: messenger_conversation_target_kind(),
        id: conv_id,
    };

    // 1. Authorized member
    let auth_result = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id: member_user_id,
            target: target.clone(),
        })
        .await
        .expect("authorization check succeeds");

    match auth_result {
        NotificationOpenAuthorization::Allowed { route } => {
            assert_eq!(route.as_str(), format!("/messages?c={conv_id}"));
        }
        _ => panic!("Expected Allowed authorization for member"),
    }

    // 2. Non-member access denied / unavailable
    let unauth_result = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id: non_member_user_id,
            target: target.clone(),
        })
        .await
        .expect("authorization check succeeds");

    assert_eq!(unauth_result, NotificationOpenAuthorization::Unavailable);

    // 3. Foreign target owner or kind
    let foreign_target = NotificationTargetRef {
        owner: NotificationSourceSlug::new("forum").unwrap(),
        kind: NotificationTargetKind::new("forum.topic").unwrap(),
        id: conv_id,
    };
    let foreign_result = provider
        .authorize_target_open(AuthorizeNotificationTargetRequest {
            tenant_id,
            recipient_id: member_user_id,
            target: foreign_target,
        })
        .await
        .expect("authorization check succeeds");

    assert_eq!(foreign_result, NotificationOpenAuthorization::Unavailable);
}
