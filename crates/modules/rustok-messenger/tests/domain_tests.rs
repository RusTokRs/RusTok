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

