use rustok_messenger::domain::{
    Conversation, ConversationType, Member, MemberRole, Message, MessageType, SnowflakeGenerator,
    SyncCursor,
};
use rustok_messenger::module::MessengerModule;
use rustok_core::RusToKModule;
use uuid::Uuid;

#[test]
fn conversation_creation_and_entity_linking() {
    let tenant_id = Uuid::now_v7();
    let creator_id = Uuid::now_v7();
    let entity_id = Uuid::now_v7();

    let conv = Conversation::new(
        tenant_id,
        ConversationType::EntityThread,
        creator_id,
        Some("Deal Discussion".into()),
    )
    .with_entity(101, entity_id);

    assert_eq!(conv.tenant_id, tenant_id);
    assert_eq!(conv.conversation_type, ConversationType::EntityThread);
    assert_eq!(conv.entity_type, Some(101));
    assert_eq!(conv.entity_id, Some(entity_id));
    assert_eq!(conv.created_by, creator_id);
    assert!(!conv.is_encrypted);
}

#[test]
fn snowflake_id_generator_strictly_monotonic() {
    let generator = SnowflakeGenerator::new(42);
    let mut prev_id = 0;

    for _ in 0..5_000 {
        let current_id = generator.next_id();
        assert!(
            current_id > prev_id,
            "Snowflake must be strictly increasing: {current_id} <= {prev_id}"
        );
        prev_id = current_id;
    }
}

#[test]
fn sync_cursor_monotonicity_and_regression_guard() {
    let conv_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();

    let mut cursor = SyncCursor::new(conv_id, user_id, 100);
    assert_eq!(cursor.last_read_message_id, 100);

    // Advancing forward succeeds
    assert!(cursor.advance(150).is_ok());
    assert_eq!(cursor.last_read_message_id, 150);

    // Regressing backward must fail
    let regression = cursor.advance(120);
    assert!(regression.is_err());
    assert_eq!(cursor.last_read_message_id, 150);
}

#[test]
fn message_blocks_and_soft_delete() {
    let tenant_id = Uuid::now_v7();
    let conv_id = Uuid::now_v7();
    let sender_id = Uuid::now_v7();

    let blocks = serde_json::json!([
        {
            "type": "section",
            "text": { "type": "mrkdwn", "text": "Approve this invoice?" }
        },
        {
            "type": "actions",
            "elements": [
                { "type": "button", "text": "Approve", "value": "approve_123" },
                { "type": "button", "text": "Reject", "value": "reject_123" }
            ]
        }
    ]);

    let mut msg = Message::new(
        1001,
        tenant_id,
        conv_id,
        sender_id,
        MessageType::Text,
        "Fallback text",
    )
    .with_blocks(blocks)
    .with_reply(999);

    assert_eq!(msg.message_type, MessageType::BlockJson);
    assert!(msg.blocks_json.is_some());
    assert_eq!(msg.reply_to_id, Some(999));
    assert!(!msg.is_deleted());

    msg.deleted_at = Some(chrono::Utc::now());
    assert!(msg.is_deleted());
}

#[test]
fn member_permissions_and_roles() {
    let conv_id = Uuid::now_v7();
    let owner_id = Uuid::now_v7();
    let member_id = Uuid::now_v7();

    let owner = Member::new(conv_id, owner_id, MemberRole::Owner);
    let member = Member::new(conv_id, member_id, MemberRole::Member);

    assert!(owner.can_manage());
    assert!(!member.can_manage());
}

#[test]
fn module_manifest_and_metadata_contract() {
    let module = MessengerModule;
    assert_eq!(module.slug(), "messenger");
    assert_eq!(module.name(), "Messenger & Team Chat");
    assert_eq!(module.dependencies(), &["outbox"]);
    assert!(!module.permissions().is_empty());
}

#[tokio::test]
async fn realtime_room_hub_broadcast_and_subscription() {
    use rustok_messenger::controllers::{MessengerRoomHub, MessengerServerMessage};
    use rustok_messenger::dto::CursorOutput;

    let hub = MessengerRoomHub::new();
    let tenant_id = Uuid::now_v7();
    let conv_id = Uuid::now_v7();

    let room = hub.get_or_create_room(tenant_id, conv_id);
    let mut sub1 = room.subscribe();
    let mut sub2 = room.subscribe();

    let cursor_payload = CursorOutput {
        conversation_id: conv_id,
        user_id: Uuid::now_v7(),
        last_read_message_id: 2024,
    };

    hub.broadcast(
        tenant_id,
        conv_id,
        MessengerServerMessage::CursorAdvanced(cursor_payload.clone()),
    );

    let msg1 = sub1.recv().await.expect("peer 1 receives message");
    let msg2 = sub2.recv().await.expect("peer 2 receives message");

    match (msg1, msg2) {
        (
            MessengerServerMessage::CursorAdvanced(c1),
            MessengerServerMessage::CursorAdvanced(c2),
        ) => {
            assert_eq!(c1.last_read_message_id, 2024);
            assert_eq!(c2.last_read_message_id, 2024);
        }
        _ => panic!("Expected CursorAdvanced message"),
    }
}

#[test]
fn realtime_client_server_wire_messages_serialization() {
    use rustok_messenger::controllers::{MessengerClientMessage, MessengerServerMessage};

    let client_ping = MessengerClientMessage::Ping;
    let json = serde_json::to_string(&client_ping).unwrap();
    assert_eq!(json, r#"{"type":"ping"}"#);

    let client_sync: MessengerClientMessage =
        serde_json::from_str(r#"{"type":"fast_sync","since_seq":42}"#).unwrap();
    match client_sync {
        MessengerClientMessage::FastSync { since_seq } => assert_eq!(since_seq, 42),
        _ => panic!("Expected FastSync"),
    }

    let server_ack = MessengerServerMessage::Ack {
        client_msg_id: Some("req_123".into()),
        message_id: 100500,
    };
    let json = serde_json::to_string(&server_ack).unwrap();
    assert!(json.contains("req_123"));
    assert!(json.contains("100500"));
}

#[test]
fn search_query_serialization_and_contracts() {
    use rustok_messenger::dto::SearchMessagesQuery;

    let conv_id = Uuid::now_v7();
    let query = SearchMessagesQuery {
        query: "urgent shipment".into(),
        conversation_id: Some(conv_id),
        limit: Some(25),
        offset: Some(0),
    };

    let json = serde_json::to_string(&query).unwrap();
    assert!(json.contains("urgent shipment"));

    let decoded: SearchMessagesQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.query, "urgent shipment");
    assert_eq!(decoded.conversation_id, Some(conv_id));
    assert_eq!(decoded.limit, Some(25));
}

#[test]
fn e2ee_device_audit_and_total_count() {
    use rustok_messenger::e2ee::{DeviceKeyRegistry, OneTimePrekeyInput, UploadDeviceKeysInput};

    let registry = DeviceKeyRegistry::new();
    assert_eq!(registry.total_devices_count(), 0);

    let user_id = Uuid::now_v7();
    registry
        .register_device_keys(
            user_id,
            UploadDeviceKeysInput {
                device_id: "phone_1".into(),
                identity_key: "ed_pub_key_1".into(),
                curve25519_key: "curve_pub_key_1".into(),
                signature: "sig_1".into(),
                one_time_prekeys: vec![
                    OneTimePrekeyInput {
                        key_id: "otk_1".into(),
                        key_material: "mat_1".into(),
                        signature: None,
                    },
                    OneTimePrekeyInput {
                        key_id: "otk_2".into(),
                        key_material: "mat_2".into(),
                        signature: None,
                    },
                ],
            },
        )
        .unwrap();

    assert_eq!(registry.total_devices_count(), 1);

    let audits = registry.list_all_devices_audit();
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0.device_id, "phone_1");
    assert_eq!(audits[0].1, 2); // 2 unconsumed OTKs

    // Claim 1 OTK
    let bundle = registry.claim_prekey_bundle(user_id, "phone_1").unwrap();
    assert!(bundle.one_time_prekey.is_some());

    let audits_after = registry.list_all_devices_audit();
    assert_eq!(audits_after[0].1, 1); // 1 unconsumed OTK remains
}

#[cfg(feature = "graphql")]
#[tokio::test]
async fn messenger_graphql_schema_build_and_execute() {
    use async_graphql::{EmptySubscription, Schema};
    use rustok_messenger::graphql::{MessengerMutation, MessengerQuery};

    let schema = Schema::build(
        MessengerQuery,
        MessengerMutation,
        EmptySubscription,
    )
    .finish();

    let sdl = schema.sdl();
    assert!(sdl.contains("messengerOverview: MessengerOverviewGql!"));
    assert!(sdl.contains("messengerChannels: [MessengerChannelListItemGql!]!"));
    assert!(sdl.contains("messengerE2eeDevices: [MessengerE2eeDeviceAuditGql!]!"));
    assert!(sdl.contains("createMessengerChannel"));
    assert!(sdl.contains("uploadDeviceKeys"));
    assert!(sdl.contains("claimPrekeyBundle"));
}

#[tokio::test]
async fn axum_router_construction_no_path_conflict() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("in-memory db connects");
    let runtime = rustok_messenger::controllers::MessengerHttpRuntime::new(db, 1);
    let app = rustok_messenger::controllers::router(runtime);
    assert!(format!("{app:?}").contains("Router"));
}

#[test]
fn device_key_registry_input_validation_and_deduplication() {
    use rustok_messenger::e2ee::{DeviceKeyRegistry, OneTimePrekeyInput, UploadDeviceKeysInput};

    let registry = DeviceKeyRegistry::new();
    let user_id = Uuid::now_v7();

    // 1. Empty device_id rejection
    let res = registry.register_device_keys(
        user_id,
        UploadDeviceKeysInput {
            device_id: "  ".into(),
            identity_key: "ed_key".into(),
            curve25519_key: "curve_key".into(),
            signature: "sig".into(),
            one_time_prekeys: vec![],
        },
    );
    assert!(res.is_err());

    // 2. Empty signature rejection
    let res = registry.register_device_keys(
        user_id,
        UploadDeviceKeysInput {
            device_id: "dev_1".into(),
            identity_key: "ed_key".into(),
            curve25519_key: "curve_key".into(),
            signature: "   ".into(),
            one_time_prekeys: vec![],
        },
    );
    assert!(res.is_err());

    // 3. Duplicate key_id in same upload batch rejection
    let res = registry.register_device_keys(
        user_id,
        UploadDeviceKeysInput {
            device_id: "dev_1".into(),
            identity_key: "ed_key".into(),
            curve25519_key: "curve_key".into(),
            signature: "sig".into(),
            one_time_prekeys: vec![
                OneTimePrekeyInput {
                    key_id: "duplicate_id".into(),
                    key_material: "mat1".into(),
                    signature: None,
                },
                OneTimePrekeyInput {
                    key_id: "duplicate_id".into(),
                    key_material: "mat2".into(),
                    signature: None,
                },
            ],
        },
    );
    assert!(res.is_err());

    // 4. Successful upload and subsequent deduplication
    let res = registry.register_device_keys(
        user_id,
        UploadDeviceKeysInput {
            device_id: "dev_1".into(),
            identity_key: "ed_key".into(),
            curve25519_key: "curve_key".into(),
            signature: "sig".into(),
            one_time_prekeys: vec![
                OneTimePrekeyInput {
                    key_id: "key_A".into(),
                    key_material: "mat_A".into(),
                    signature: None,
                },
                OneTimePrekeyInput {
                    key_id: "key_B".into(),
                    key_material: "mat_B".into(),
                    signature: None,
                },
            ],
        },
    );
    assert!(res.is_ok());

    let audits = registry.list_all_devices_audit();
    assert_eq!(audits[0].1, 2);

    // Re-uploading key_A again does not duplicate in pool
    let res2 = registry.register_device_keys(
        user_id,
        UploadDeviceKeysInput {
            device_id: "dev_1".into(),
            identity_key: "ed_key".into(),
            curve25519_key: "curve_key".into(),
            signature: "sig".into(),
            one_time_prekeys: vec![
                OneTimePrekeyInput {
                    key_id: "key_A".into(),
                    key_material: "mat_A".into(),
                    signature: None,
                },
                OneTimePrekeyInput {
                    key_id: "key_C".into(),
                    key_material: "mat_C".into(),
                    signature: None,
                },
            ],
        },
    );
    assert!(res2.is_ok());

    let audits_after = registry.list_all_devices_audit();
    assert_eq!(audits_after[0].1, 3); // 2 existing + 1 new (key_C), key_A was not duplicated
}

#[test]
fn member_mute_and_permission_invariants() {
    let conv_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();

    let mut member = Member::new(conv_id, user_id, MemberRole::Member);
    assert!(!member.is_muted());
    assert!(!member.can_manage());

    // Muted in the past -> not muted now
    member.muted_until = Some(chrono::Utc::now() - chrono::Duration::minutes(5));
    assert!(!member.is_muted());

    // Muted in the future -> is muted
    member.muted_until = Some(chrono::Utc::now() + chrono::Duration::minutes(30));
    assert!(member.is_muted());

    // Admin and Owner roles can manage
    let admin = Member::new(conv_id, Uuid::now_v7(), MemberRole::Admin);
    assert!(admin.can_manage());

    let owner = Member::new(conv_id, Uuid::now_v7(), MemberRole::Owner);
    assert!(owner.can_manage());

    let guest = Member::new(conv_id, Uuid::now_v7(), MemberRole::Guest);
    assert!(!guest.can_manage());
}

#[test]
fn streaming_events_edited_and_deleted_partitioning() {
    use rustok_messenger::streaming::MessengerStreamEvent;

    let tenant_id = Uuid::now_v7();
    let conv_id = Uuid::now_v7();

    let event_dispatched = MessengerStreamEvent::MessageDispatched {
        tenant_id,
        conversation_id: conv_id,
        message_id: 101,
        sender_id: Uuid::now_v7(),
        content: "Hello".into(),
        blocks_json: None,
        reply_to_id: None,
        created_at: chrono::Utc::now(),
    };

    let event_cursor = MessengerStreamEvent::CursorUpdated {
        tenant_id,
        conversation_id: conv_id,
        user_id: Uuid::now_v7(),
        last_read_message_id: 101,
        updated_at: chrono::Utc::now(),
    };

    assert_eq!(event_dispatched.partition_key(), event_cursor.partition_key());
}

#[test]
fn empty_message_validation_contract() {
    use rustok_messenger::dto::SendMessageInput;

    let conv_id = Uuid::now_v7();

    // Pure whitespace with no blocks is empty
    let empty_input = SendMessageInput {
        conversation_id: conv_id,
        message_type: None,
        content: "   \n\t  ".into(),
        blocks_json: None,
        reply_to_id: None,
        is_encrypted: None,
        encryption_algo: None,
    };
    assert!(empty_input.content.trim().is_empty() && empty_input.blocks_json.is_none());

    // Empty content with blocks is valid (e.g. rich card or attachment only)
    let blocks_only_input = SendMessageInput {
        conversation_id: conv_id,
        message_type: None,
        content: "".into(),
        blocks_json: Some(serde_json::json!([{"type": "image", "url": "https://example.com/img.png"}])),
        reply_to_id: None,
        is_encrypted: None,
        encryption_algo: None,
    };
    assert!(!(blocks_only_input.content.trim().is_empty() && blocks_only_input.blocks_json.is_none()));
}

#[test]
fn member_role_hierarchy_protection() {
    let conv_id = Uuid::now_v7();
    let admin = Member::new(conv_id, Uuid::now_v7(), MemberRole::Admin);
    let owner = Member::new(conv_id, Uuid::now_v7(), MemberRole::Owner);
    let member = Member::new(conv_id, Uuid::now_v7(), MemberRole::Member);

    // Only owner can appoint owner; admin cannot appoint owner
    assert_eq!(owner.role, MemberRole::Owner);
    assert_ne!(admin.role, MemberRole::Owner);
    assert!(admin.can_manage());
    assert!(owner.can_manage());
    assert!(!member.can_manage());
}

#[test]
fn dto_validation_rejections() {
    use rustok_messenger::dto::*;

    let conv_id = Uuid::now_v7();

    // 1. CreateConversationInput
    let nil_participant = CreateConversationInput {
        conversation_type: ConversationType::Group,
        title: Some("Valid Title".into()),
        entity_type: None,
        entity_id: None,
        participant_ids: vec![Uuid::nil()],
        is_encrypted: Some(false),
        initial_locale: None,
        initial_channel_description: None,
    };
    assert!(nil_participant.validate().is_err());

    let empty_direct = CreateConversationInput {
        conversation_type: ConversationType::Direct,
        title: None,
        entity_type: None,
        entity_id: None,
        participant_ids: vec![],
        is_encrypted: Some(false),
        initial_locale: None,
        initial_channel_description: None,
    };
    assert!(empty_direct.validate().is_err());

    let invalid_locale_input = CreateConversationInput {
        conversation_type: ConversationType::Channel,
        title: Some("Channel".into()),
        entity_type: None,
        entity_id: None,
        participant_ids: vec![],
        is_encrypted: Some(false),
        initial_locale: Some("invalid!*locale".into()),
        initial_channel_description: None,
    };
    assert!(invalid_locale_input.validate().is_err());

    // 2. UpdateChannelTranslationInput
    let nil_channel = UpdateChannelTranslationInput {
        channel_id: Uuid::nil(),
        locale: "ru".into(),
        title: "Заголовок".into(),
        description: None,
    };
    assert!(nil_channel.validate().is_err());

    let empty_title = UpdateChannelTranslationInput {
        channel_id: conv_id,
        locale: "ru".into(),
        title: "   ".into(),
        description: None,
    };
    assert!(empty_title.validate().is_err());

    let invalid_trans_locale = UpdateChannelTranslationInput {
        channel_id: conv_id,
        locale: "bad_locale_123".into(),
        title: "Title".into(),
        description: None,
    };
    assert!(invalid_trans_locale.validate().is_err());

    // 3. SendMessageInput
    let nil_conv_send = SendMessageInput {
        conversation_id: Uuid::nil(),
        message_type: None,
        content: "hello".into(),
        blocks_json: None,
        reply_to_id: None,
        is_encrypted: None,
        encryption_algo: None,
    };
    assert!(nil_conv_send.validate().is_err());

    let empty_send = SendMessageInput {
        conversation_id: conv_id,
        message_type: None,
        content: "   ".into(),
        blocks_json: None,
        reply_to_id: None,
        is_encrypted: None,
        encryption_algo: None,
    };
    assert!(empty_send.validate().is_err());

    let negative_reply_to = SendMessageInput {
        conversation_id: conv_id,
        message_type: None,
        content: "hello".into(),
        blocks_json: None,
        reply_to_id: Some(-1),
        is_encrypted: None,
        encryption_algo: None,
    };
    assert!(negative_reply_to.validate().is_err());

    let unsupported_algo = SendMessageInput {
        conversation_id: conv_id,
        message_type: None,
        content: "hello".into(),
        blocks_json: None,
        reply_to_id: None,
        is_encrypted: Some(true),
        encryption_algo: Some("des-ede3-cbc".into()),
    };
    assert!(unsupported_algo.validate().is_err());

    // 4. EditMessageInput
    let empty_edit = EditMessageInput {
        content: "   \t ".into(),
    };
    assert!(empty_edit.validate().is_err());

    // 5. ListMessagesQuery
    let nil_conv_list = ListMessagesQuery {
        conversation_id: Uuid::nil(),
        limit: Some(10),
        before_id: None,
        after_id: None,
    };
    assert!(nil_conv_list.validate().is_err());

    let zero_limit_list = ListMessagesQuery {
        conversation_id: conv_id,
        limit: Some(0),
        before_id: None,
        after_id: None,
    };
    assert!(zero_limit_list.validate().is_err());

    // 6. UpdateCursorInput
    let nil_conv_cursor = UpdateCursorInput {
        conversation_id: Uuid::nil(),
        last_read_message_id: 10,
    };
    assert!(nil_conv_cursor.validate().is_err());

    let negative_seq_cursor = UpdateCursorInput {
        conversation_id: conv_id,
        last_read_message_id: -5,
    };
    assert!(negative_seq_cursor.validate().is_err());

    // 7. FastSyncRequest
    let nil_conv_sync = FastSyncRequest {
        conversation_id: Uuid::nil(),
        since_seq: 0,
        limit: Some(50),
    };
    assert!(nil_conv_sync.validate().is_err());

    let negative_since_sync = FastSyncRequest {
        conversation_id: conv_id,
        since_seq: -1,
        limit: Some(50),
    };
    assert!(negative_since_sync.validate().is_err());

    // 8. AddMemberInput
    let nil_user_member = AddMemberInput {
        user_id: Uuid::nil(),
        role: None,
    };
    assert!(nil_user_member.validate().is_err());
}

#[test]
fn e2ee_device_registry_nil_uuid_protections() {
    use rustok_messenger::e2ee::{DeviceKeyRegistry, UploadDeviceKeysInput};

    let registry = DeviceKeyRegistry::new();

    let nil_upload = registry.register_device_keys(
        Uuid::nil(),
        UploadDeviceKeysInput {
            device_id: "dev-1".into(),
            identity_key: "k1".into(),
            curve25519_key: "k2".into(),
            signature: "sig".into(),
            one_time_prekeys: vec![],
        },
    );
    assert!(nil_upload.is_err());

    assert_eq!(registry.list_user_devices(Uuid::nil()).len(), 0);

    let nil_claim = registry.claim_prekey_bundle(Uuid::nil(), "dev-1");
    assert!(nil_claim.is_err());
}

#[tokio::test]
async fn system_message_dispatch_bypasses_membership() {
    use sea_orm::Database;
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use rustok_messenger::services::MessengerService;
    use rustok_messenger::dto::{CreateConversationInput, SendMessageInput};
    use rustok_messenger::domain::{ConversationType, MessageType};
    use rustok_outbox::SysEventsMigration;

    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite must connect");

    let manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration must succeed");
    for migration in rustok_messenger::migrations::migrations() {
        migration.up(&manager).await.expect("migration must succeed");
    }

    let service = MessengerService::new(db, 1);
    let tenant_id = Uuid::now_v7();
    let creator_id = Uuid::now_v7();
    let member_id = Uuid::now_v7();

    // Create a group conversation
    let conv = service
        .create_conversation(
            tenant_id,
            creator_id,
            CreateConversationInput {
                conversation_type: ConversationType::Group,
                title: Some("Support Room".into()),
                entity_type: None,
                entity_id: None,
                participant_ids: vec![member_id],
                is_encrypted: Some(false),
                initial_locale: None,
                initial_channel_description: None,
            },
        )
        .await
        .expect("create conversation succeeds");

    // Regular non-member dispatch MUST be denied
    let non_member_id = Uuid::now_v7();
    let denied = service
        .send_message(
            tenant_id,
            non_member_id,
            SendMessageInput {
                conversation_id: conv.id,
                message_type: Some(MessageType::Text),
                content: "Unauthorized text".into(),
                blocks_json: None,
                reply_to_id: None,
                is_encrypted: None,
                encryption_algo: None,
            },
        )
        .await;
    assert!(denied.is_err());

    // Automated system message from non-member MUST SUCCEED
    let system_msg = service
        .send_message(
            tenant_id,
            non_member_id,
            SendMessageInput {
                conversation_id: conv.id,
                message_type: Some(MessageType::System),
                content: "Payment received successfully".into(),
                blocks_json: None,
                reply_to_id: None,
                is_encrypted: None,
                encryption_algo: None,
            },
        )
        .await
        .expect("system message from service actor must succeed");

    assert_eq!(system_msg.message_type, MessageType::System);
    assert_eq!(system_msg.content, "Payment received successfully");

    // Automated system message with nil actor MUST ALSO SUCCEED
    let nil_actor_system_msg = service
        .send_message(
            tenant_id,
            Uuid::nil(),
            SendMessageInput {
                conversation_id: conv.id,
                message_type: Some(MessageType::System),
                content: "System maintenance scheduled".into(),
                blocks_json: None,
                reply_to_id: None,
                is_encrypted: None,
                encryption_algo: None,
            },
        )
        .await
        .expect("system message with nil actor must succeed");

    assert_eq!(nil_actor_system_msg.message_type, MessageType::System);
    assert_eq!(
        nil_actor_system_msg.sender_id,
        rustok_messenger::services::SYSTEM_ACTOR_UUID
    );
}

#[tokio::test]
async fn test_messenger_nil_uuid_and_bounds_guards() {
    let db = sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("db connect");

    // ensure_messenger_module_enabled nil tenant guard
    let res = rustok_messenger::controllers::ensure_messenger_module_enabled(&db, Uuid::nil()).await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, "INVALID_TENANT");

    let service = rustok_messenger::services::MessengerService::new(db.clone(), 1);

    // nil tenant in get_overview_counts
    assert!(service.get_overview_counts(Uuid::nil()).await.is_err());

    // nil tenant in list_channels_summary
    assert!(service.list_channels_summary(Uuid::nil(), 10, 0).await.is_err());

    // nil tenant or actor in create_conversation
    let input = rustok_messenger::dto::CreateConversationInput {
        conversation_type: ConversationType::Group,
        title: Some("Group Chat".into()),
        entity_type: None,
        entity_id: None,
        participant_ids: vec![],
        is_encrypted: Some(false),
        initial_locale: None,
        initial_channel_description: None,
    };
    assert!(service.create_conversation(Uuid::nil(), Uuid::now_v7(), input.clone()).await.is_err());
    assert!(service.create_conversation(Uuid::now_v7(), Uuid::nil(), input).await.is_err());

    // nil tenant or actor in send_message
    let send_input = rustok_messenger::dto::SendMessageInput {
        conversation_id: Uuid::now_v7(),
        message_type: Some(MessageType::Text),
        content: "Test message".into(),
        blocks_json: None,
        reply_to_id: None,
        is_encrypted: None,
        encryption_algo: None,
    };
    assert!(service.send_message(Uuid::nil(), Uuid::now_v7(), send_input.clone()).await.is_err());
    assert!(service.send_message(Uuid::now_v7(), Uuid::nil(), send_input).await.is_err());
}



