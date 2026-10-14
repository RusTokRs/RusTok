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
