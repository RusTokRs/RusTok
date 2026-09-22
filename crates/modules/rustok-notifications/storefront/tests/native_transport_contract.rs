use rustok_notifications_storefront::{
    NotificationInboxAvailability, NotificationStorefrontGroupItemsRequest,
    NotificationStorefrontGroupStateAction, NotificationStorefrontGroupStateCommand,
    NotificationStorefrontGroupSummaryRequest, NotificationStorefrontItemState,
    NotificationStorefrontOpenRequest, NotificationStorefrontState,
};

#[test]
fn native_storefront_requests_do_not_expose_owner_identity_fields() {
    let requests = [
        serde_json::to_value(NotificationStorefrontGroupSummaryRequest {
            cursor: Some("cursor-a".to_string()),
            limit: 20,
        })
        .expect("group summary request should serialize"),
        serde_json::to_value(NotificationStorefrontGroupItemsRequest {
            group_key: "g1:forum:00000000-0000-0000-0000-000000000001".to_string(),
            state: Some(NotificationStorefrontItemState::Unread),
            cursor: None,
            limit: 20,
        })
        .expect("group item request should serialize"),
        serde_json::to_value(NotificationStorefrontOpenRequest {
            notification_id: "00000000-0000-0000-0000-000000000002".to_string(),
        })
        .expect("open request should serialize"),
        serde_json::to_value(NotificationStorefrontGroupStateCommand {
            group_key: "g1:forum:00000000-0000-0000-0000-000000000003".to_string(),
            action: NotificationStorefrontGroupStateAction::MarkRead,
            cursor: None,
            limit: 20,
            idempotency_key: "notification-group-state-a".to_string(),
        })
        .expect("group state command should serialize"),
    ];

    for request in requests {
        let object = request
            .as_object()
            .expect("storefront transport request should be an object");
        assert!(!object.contains_key("tenant_id"));
        assert!(!object.contains_key("recipient_id"));
        assert!(!object.contains_key("user_id"));
    }
}

#[test]
fn group_state_command_retains_write_admission_input() {
    let encoded = serde_json::to_value(NotificationStorefrontGroupStateCommand {
        group_key: "g1:forum:00000000-0000-0000-0000-000000000004".to_string(),
        action: NotificationStorefrontGroupStateAction::MarkUnread,
        cursor: Some("cursor-b".to_string()),
        limit: 64,
        idempotency_key: "notification-group-state-b".to_string(),
    })
    .expect("group state command should serialize");

    assert_eq!(encoded["action"], "mark_unread");
    assert_eq!(encoded["idempotency_key"], "notification-group-state-b");
    assert_eq!(encoded["limit"], 64);
}

#[test]
fn grouped_ui_remains_explicitly_unavailable_until_composed() {
    assert_eq!(
        NotificationStorefrontState::foundation(),
        NotificationStorefrontState {
            availability: NotificationInboxAvailability::Unavailable,
            unread_count: None,
        }
    );
}
