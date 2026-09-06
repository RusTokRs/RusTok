use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationInboxAvailability {
    Unavailable,
    Available,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontState {
    pub availability: NotificationInboxAvailability,
    pub unread_count: Option<u32>,
}

impl NotificationStorefrontState {
    pub const fn foundation() -> Self {
        Self {
            availability: NotificationInboxAvailability::Unavailable,
            unread_count: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStorefrontItemState {
    Unread,
    Seen,
    Read,
    Archived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStorefrontPriority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStorefrontGroupStateAction {
    MarkRead,
    MarkUnread,
    Archive,
}

impl NotificationStorefrontGroupStateAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarkRead => "mark_read",
            Self::MarkUnread => "mark_unread",
            Self::Archive => "archive",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontUnreadCount {
    pub unread_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupSummaryRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupItemsRequest {
    pub group_key: String,
    #[serde(default)]
    pub state: Option<NotificationStorefrontItemState>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontOpenRequest {
    pub notification_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupStateCommand {
    pub group_key: String,
    pub action: NotificationStorefrontGroupStateAction,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: u16,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontItem {
    pub id: String,
    pub source: String,
    pub notification_type: String,
    pub template_key: String,
    pub actor_id: Option<String>,
    pub priority: NotificationStorefrontPriority,
    pub state: NotificationStorefrontItemState,
    pub template_data: BTreeMap<String, String>,
    pub seen_at: Option<String>,
    pub read_at: Option<String>,
    pub archived_at: Option<String>,
    pub created_at: String,
}

impl NotificationStorefrontItem {
    pub fn display_title(&self) -> String {
        self.template_data
            .get("title")
            .or_else(|| self.template_data.get("topic_title"))
            .or_else(|| self.template_data.get("subject"))
            .cloned()
            .unwrap_or_else(|| self.notification_type.clone())
    }

    pub fn display_body(&self) -> String {
        self.template_data
            .get("body")
            .or_else(|| self.template_data.get("message"))
            .or_else(|| self.template_data.get("summary"))
            .cloned()
            .unwrap_or_else(|| self.template_key.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupSummary {
    pub group_key: String,
    pub item_count: u64,
    pub unread_count: u64,
    pub latest_item: NotificationStorefrontItem,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupSummaryPage {
    pub groups: Vec<NotificationStorefrontGroupSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupItemsPage {
    pub items: Vec<NotificationStorefrontItem>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum NotificationStorefrontOpenDecision {
    Allowed { route: String },
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontGroupStatePage {
    pub scanned: u16,
    pub changed: u16,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontInboxSnapshot {
    pub unread_count: u64,
    pub groups: Vec<NotificationStorefrontGroupSummary>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl NotificationStorefrontInboxSnapshot {
    pub fn new(unread_count: u64, page: NotificationStorefrontGroupSummaryPage) -> Self {
        Self {
            unread_count,
            groups: page.groups,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        }
    }

    pub fn append_page(&mut self, page: NotificationStorefrontGroupSummaryPage) -> usize {
        let mut known = self
            .groups
            .iter()
            .map(|group| group.group_key.clone())
            .collect::<BTreeSet<_>>();
        let mut appended = 0;
        for group in page.groups {
            if known.insert(group.group_key.clone()) {
                self.groups.push(group);
                appended += 1;
            }
        }
        self.next_cursor = page.next_cursor;
        self.has_more = page.has_more;
        appended
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationStorefrontGroupItemsSnapshot {
    pub group_key: String,
    pub items: Vec<NotificationStorefrontItem>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

impl NotificationStorefrontGroupItemsSnapshot {
    pub fn from_page(group_key: String, page: NotificationStorefrontGroupItemsPage) -> Self {
        Self {
            group_key,
            items: page.items,
            next_cursor: page.next_cursor,
            has_more: page.has_more,
        }
    }

    pub fn append_page(&mut self, page: NotificationStorefrontGroupItemsPage) -> usize {
        let mut known = self
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<BTreeSet<_>>();
        let mut appended = 0;
        for item in page.items {
            if known.insert(item.id.clone()) {
                self.items.push(item);
                appended += 1;
            }
        }
        self.next_cursor = page.next_cursor;
        self.has_more = page.has_more;
        appended
    }
}
