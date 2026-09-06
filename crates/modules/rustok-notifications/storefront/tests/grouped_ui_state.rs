use std::collections::BTreeMap;

use rustok_notifications_storefront::{
    NotificationStorefrontGroupItemsPage, NotificationStorefrontGroupItemsSnapshot,
    NotificationStorefrontGroupStateAction, NotificationStorefrontGroupSummary,
    NotificationStorefrontGroupSummaryPage, NotificationStorefrontInboxSnapshot,
    NotificationStorefrontItem, NotificationStorefrontItemState, NotificationStorefrontPriority,
};

#[test]
fn summary_pages_append_without_duplicate_group_state() {
    let mut snapshot = NotificationStorefrontInboxSnapshot::new(
        3,
        NotificationStorefrontGroupSummaryPage {
            groups: vec![group("group-a", 10, 2)],
            next_cursor: Some("cursor-a".to_string()),
            has_more: true,
        },
    );

    let appended = snapshot.append_page(NotificationStorefrontGroupSummaryPage {
        groups: vec![group("group-a", 10, 2), group("group-b", 20, 1)],
        next_cursor: Some("cursor-b".to_string()),
        has_more: false,
    });

    assert_eq!(appended, 1);
    assert_eq!(snapshot.unread_count, 3);
    assert_eq!(
        snapshot
            .groups
            .iter()
            .map(|group| group.group_key.as_str())
            .collect::<Vec<_>>(),
        vec!["group-a", "group-b"]
    );
    assert_eq!(snapshot.next_cursor.as_deref(), Some("cursor-b"));
    assert!(!snapshot.has_more);
}

#[test]
fn item_pages_append_without_duplicate_notification_identity() {
    let mut snapshot = NotificationStorefrontGroupItemsSnapshot::from_page(
        "group-a".to_string(),
        NotificationStorefrontGroupItemsPage {
            items: vec![item(1, "type-a")],
            next_cursor: Some("cursor-a".to_string()),
            has_more: true,
        },
    );

    let appended = snapshot.append_page(NotificationStorefrontGroupItemsPage {
        items: vec![item(1, "type-a"), item(2, "type-b")],
        next_cursor: None,
        has_more: false,
    });

    assert_eq!(appended, 1);
    assert_eq!(
        snapshot
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "00000000-0000-0000-0000-000000000001",
            "00000000-0000-0000-0000-000000000002"
        ]
    );
    assert_eq!(snapshot.next_cursor, None);
    assert!(!snapshot.has_more);
}

#[test]
fn presentation_uses_bounded_template_fields_then_semantic_fallbacks() {
    let mut rich = item(1, "forum.mention");
    rich.template_data = BTreeMap::from([
        ("title".to_string(), "A topic mentioned you".to_string()),
        (
            "body".to_string(),
            "Open the topic to review the mention.".to_string(),
        ),
    ]);
    assert_eq!(rich.display_title(), "A topic mentioned you");
    assert_eq!(rich.display_body(), "Open the topic to review the mention.");

    let fallback = item(2, "forum.topic.created");
    assert_eq!(fallback.display_title(), "forum.topic.created");
    assert_eq!(fallback.display_body(), "forum.topic.created.v1");
}

#[test]
fn group_action_labels_match_transport_contract() {
    assert_eq!(
        NotificationStorefrontGroupStateAction::MarkRead.as_str(),
        "mark_read"
    );
    assert_eq!(
        NotificationStorefrontGroupStateAction::MarkUnread.as_str(),
        "mark_unread"
    );
    assert_eq!(
        NotificationStorefrontGroupStateAction::Archive.as_str(),
        "archive"
    );
}

fn group(group_key: &str, id: u128, unread_count: u64) -> NotificationStorefrontGroupSummary {
    NotificationStorefrontGroupSummary {
        group_key: group_key.to_string(),
        item_count: 2,
        unread_count,
        latest_item: item(id, "forum.topic.created"),
    }
}

fn item(id: u128, notification_type: &str) -> NotificationStorefrontItem {
    NotificationStorefrontItem {
        id: format!("{id:032x}").chars().enumerate().fold(
            String::new(),
            |mut output, (index, character)| {
                if matches!(index, 8 | 12 | 16 | 20) {
                    output.push('-');
                }
                output.push(character);
                output
            },
        ),
        source: "forum".to_string(),
        notification_type: notification_type.to_string(),
        template_key: format!("{notification_type}.v1"),
        actor_id: None,
        priority: NotificationStorefrontPriority::Normal,
        state: NotificationStorefrontItemState::Unread,
        template_data: BTreeMap::new(),
        seen_at: None,
        read_at: None,
        archived_at: None,
        created_at: "2026-07-27T00:00:00+00:00".to_string(),
    }
}
