use chrono::{DateTime, Datelike, Timelike, Utc};
use uuid::Uuid;

use crate::model::{ConversationItemPayload, MessageItemPayload, SendMessagePayload};

/// Maximum message length allowed by the client validation.
pub const MAX_MESSAGE_LENGTH: usize = 4_096;

/// Tab/Filter for categorizing conversation items in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConversationFilter {
    #[default]
    All,
    Channels,
    Direct,
    EntityThreads,
}

impl ConversationFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Channels => "channels",
            Self::Direct => "direct",
            Self::EntityThreads => "entity_threads",
        }
    }

    pub fn matches(&self, conv: &ConversationItemViewModel) -> bool {
        match self {
            Self::All => true,
            Self::Channels => conv.conversation_type == "channel" || conv.conversation_type == "group",
            Self::Direct => conv.conversation_type == "direct",
            Self::EntityThreads => conv.is_entity_thread,
        }
    }
}

/// Message delivery lifecycle status for optimistic UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeliveryStatus {
    #[default]
    Sending,
    Sent,
    Read,
    Failed,
}

/// Active tab in the right-side context drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextDrawerTab {
    Members,
    Media,
    Threads,
    Search,
}

/// Computable view-model for conversation list items in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationItemViewModel {
    pub id: Uuid,
    pub conversation_type: String,
    pub title: String,
    pub topic: Option<String>,
    pub last_message_snippet: String,
    pub last_message_time: String,
    pub unread_count: usize,
    pub unread_badge: Option<String>,
    pub is_encrypted: bool,
    pub is_entity_thread: bool,
    pub entity_badge: Option<String>,
}

impl ConversationItemViewModel {
    pub fn from_payload(payload: &ConversationItemPayload) -> Self {
        let is_entity = payload.entity_type.is_some() || payload.conversation_type == "entity_thread";
        let title = payload
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                if payload.conversation_type == "direct" {
                    "Direct Message".to_string()
                } else if is_entity {
                    "Entity Thread".to_string()
                } else {
                    "Channel".to_string()
                }
            });

        let snippet = payload
            .last_message_content
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.chars().count() > 48 {
                    let truncated: String = s.chars().take(45).collect();
                    format!("{truncated}...")
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "No messages yet".to_string());

        let time_str = payload
            .last_message_at
            .map(format_short_time)
            .unwrap_or_default();

        let unread_badge = if payload.unread_count > 0 {
            if payload.unread_count > 99 {
                Some("99+".to_string())
            } else {
                Some(payload.unread_count.to_string())
            }
        } else {
            None
        };

        let entity_badge = payload.entity_type.map(|t| match t {
            101 => "CRM Deal".to_string(),
            102 => "Order".to_string(),
            103 => "Support Ticket".to_string(),
            other => format!("Entity #{other}"),
        });

        Self {
            id: payload.id,
            conversation_type: payload.conversation_type.clone(),
            title,
            topic: payload.topic.clone(),
            last_message_snippet: snippet,
            last_message_time: time_str,
            unread_count: payload.unread_count,
            unread_badge,
            is_encrypted: payload.is_encrypted,
            is_entity_thread: is_entity,
            entity_badge,
        }
    }
}

/// Computable view-model for an individual message bubble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessageViewModel {
    pub id: i64,
    pub conversation_id: Uuid,
    pub sender_id: Uuid,
    pub sender_display_name: String,
    pub content: String,
    pub formatted_time: String,
    pub created_at: DateTime<Utc>,
    pub is_outgoing: bool,
    pub delivery_status: DeliveryStatus,
    pub reply_to_id: Option<i64>,
    pub is_encrypted: bool,
    pub client_msg_id: Option<String>,
}

impl ChatMessageViewModel {
    pub fn from_payload(payload: &MessageItemPayload, current_user_id: Uuid) -> Self {
        let is_outgoing = payload.sender_id == current_user_id;
        let display_name = payload
            .sender_name
            .clone()
            .unwrap_or_else(|| if is_outgoing { "You".to_string() } else { "Colleague".to_string() });

        Self {
            id: payload.id,
            conversation_id: payload.conversation_id,
            sender_id: payload.sender_id,
            sender_display_name: display_name,
            content: payload.content.clone(),
            formatted_time: format_short_time(payload.created_at),
            created_at: payload.created_at,
            is_outgoing,
            delivery_status: DeliveryStatus::Sent,
            reply_to_id: payload.reply_to_id,
            is_encrypted: payload.is_encrypted,
            client_msg_id: None,
        }
    }
}

/// Messages grouped under a calendar date separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDateGroup {
    pub date_label: String,
    pub messages: Vec<ChatMessageViewModel>,
}

/// Pure framework-agnostic client state machine for the Messenger workstation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessengerWorkspaceState {
    pub current_user_id: Uuid,
    pub active_conversation_id: Option<Uuid>,
    pub conversations: Vec<ConversationItemViewModel>,
    pub messages: Vec<ChatMessageViewModel>,
    pub filter: ConversationFilter,
    pub search_query: String,
    pub active_drawer_tab: Option<ContextDrawerTab>,
    pub replying_to: Option<ChatMessageViewModel>,
}

impl MessengerWorkspaceState {
    pub fn new(current_user_id: Uuid) -> Self {
        Self {
            current_user_id,
            active_conversation_id: None,
            conversations: Vec::new(),
            messages: Vec::new(),
            filter: ConversationFilter::All,
            search_query: String::new(),
            active_drawer_tab: None,
            replying_to: None,
        }
    }

    pub fn set_conversations(&mut self, payloads: &[ConversationItemPayload]) {
        self.conversations = payloads.iter().map(ConversationItemViewModel::from_payload).collect();
        if self.active_conversation_id.is_none() {
            self.active_conversation_id = self.conversations.first().map(|c| c.id);
        }
    }

    pub fn set_messages(&mut self, payloads: &[MessageItemPayload]) {
        let mut vms: Vec<ChatMessageViewModel> = payloads
            .iter()
            .map(|p| ChatMessageViewModel::from_payload(p, self.current_user_id))
            .collect();
        vms.sort_by_key(|m| m.id);
        self.messages = vms;
    }

    pub fn select_conversation(&mut self, id: Uuid) {
        self.active_conversation_id = Some(id);
        self.messages.clear();
        self.replying_to = None;
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == id) {
            conv.unread_count = 0;
            conv.unread_badge = None;
        }
    }

    pub fn set_filter(&mut self, filter: ConversationFilter) {
        self.filter = filter;
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into().trim().to_string();
    }

    pub fn toggle_drawer_tab(&mut self, tab: ContextDrawerTab) {
        if self.active_drawer_tab == Some(tab) {
            self.active_drawer_tab = None;
        } else {
            self.active_drawer_tab = Some(tab);
        }
    }

    pub fn set_replying_to(&mut self, message: Option<ChatMessageViewModel>) {
        self.replying_to = message;
    }

    /// Returns conversation items filtered by active category tab and search query.
    pub fn filtered_conversations(&self) -> Vec<&ConversationItemViewModel> {
        let q = self.search_query.to_lowercase();
        self.conversations
            .iter()
            .filter(|c| self.filter.matches(c))
            .filter(|c| {
                if q.is_empty() {
                    true
                } else {
                    c.title.to_lowercase().contains(&q)
                        || c.topic.as_deref().unwrap_or_default().to_lowercase().contains(&q)
                        || c.last_message_snippet.to_lowercase().contains(&q)
                }
            })
            .collect()
    }

    /// Returns the active conversation view-model if one is currently selected.
    pub fn active_conversation(&self) -> Option<&ConversationItemViewModel> {
        self.active_conversation_id
            .and_then(|id| self.conversations.iter().find(|c| c.id == id))
    }

    /// Validates raw user input and creates an optimistic message appended to the state.
    pub fn submit_optimistic_message(
        &mut self,
        raw_content: &str,
    ) -> Result<(SendMessagePayload, ChatMessageViewModel), &'static str> {
        let trimmed = raw_content.trim();
        if trimmed.is_empty() {
            return Err("Message content cannot be empty");
        }
        if trimmed.len() > MAX_MESSAGE_LENGTH {
            return Err("Message exceeds maximum allowed length of 4096 characters");
        }
        let conv_id = self
            .active_conversation_id
            .ok_or("No active conversation selected")?;

        let client_id = format!("client_{}", Uuid::now_v7());
        let reply_to_id = self.replying_to.as_ref().map(|m| m.id);
        let now = Utc::now();

        let optimistic_vm = ChatMessageViewModel {
            id: 0,
            conversation_id: conv_id,
            sender_id: self.current_user_id,
            sender_display_name: "You".to_string(),
            content: trimmed.to_string(),
            formatted_time: format_short_time(now),
            created_at: now,
            is_outgoing: true,
            delivery_status: DeliveryStatus::Sending,
            reply_to_id,
            is_encrypted: false,
            client_msg_id: Some(client_id.clone()),
        };

        let outbound_payload = SendMessagePayload {
            conversation_id: conv_id,
            content: trimmed.to_string(),
            reply_to_id,
            client_msg_id: Some(client_id),
        };

        self.messages.push(optimistic_vm.clone());
        self.replying_to = None;

        // Update snippet in conversations sidebar
        if let Some(conv) = self.conversations.iter_mut().find(|c| c.id == conv_id) {
            conv.last_message_snippet = trimmed.to_string();
            conv.last_message_time = format_short_time(now);
        }

        Ok((outbound_payload, optimistic_vm))
    }

    /// Transition optimistic message status from Sending to Sent upon receiving server ACK.
    pub fn apply_ack(&mut self, client_msg_id: &str, server_msg_id: i64) {
        if let Some(msg) = self
            .messages
            .iter_mut()
            .find(|m| m.client_msg_id.as_deref() == Some(client_msg_id))
        {
            msg.id = server_msg_id;
            msg.delivery_status = DeliveryStatus::Sent;
        }
    }

    /// Advance read horizon for outgoing messages up to `last_read_message_id`.
    pub fn advance_read_horizon(&mut self, up_to_id: i64) {
        for msg in &mut self.messages {
            if msg.is_outgoing && msg.id > 0 && msg.id <= up_to_id {
                msg.delivery_status = DeliveryStatus::Read;
            }
        }
    }

    /// Groups the current messages chronologically into calendar day groups.
    pub fn grouped_messages(&self) -> Vec<MessageDateGroup> {
        let mut groups: Vec<MessageDateGroup> = Vec::new();
        let mut current_date: Option<(i32, u32, u32)> = None;

        for msg in &self.messages {
            let date_key = (
                msg.created_at.year(),
                msg.created_at.month(),
                msg.created_at.day(),
            );

            if current_date != Some(date_key) {
                current_date = Some(date_key);
                let label = format_date_header(msg.created_at);
                groups.push(MessageDateGroup {
                    date_label: label,
                    messages: vec![msg.clone()],
                });
            } else if let Some(last_group) = groups.last_mut() {
                last_group.messages.push(msg.clone());
            }
        }

        groups
    }

    /// Sum of unread badges across all conversations.
    pub fn total_unread_count(&self) -> usize {
        self.conversations.iter().map(|c| c.unread_count).sum()
    }
}

/// Localized string labels bundle for the Storefront Messenger interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessengerLabels {
    pub title: String,
    pub search_placeholder: String,
    pub filter_all: String,
    pub filter_channels: String,
    pub filter_direct: String,
    pub filter_entities: String,
    pub composer_placeholder: String,
    pub composer_send: String,
    pub composer_replying_to: String,
    pub composer_cancel_reply: String,
    pub status_online: String,
    pub status_members: String,
    pub drawer_members: String,
    pub drawer_media: String,
    pub drawer_threads: String,
    pub drawer_close: String,
    pub widget_title: String,
    pub widget_support: String,
    pub widget_seller: String,
    pub empty_conversations: String,
    pub empty_messages: String,
}

impl MessengerLabels {
    pub fn resolve(locale: Option<&str>) -> Self {
        let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
        if is_ru {
            Self {
                title: "Мессенджер".into(),
                search_placeholder: "Поиск чатов и сообщений...".into(),
                filter_all: "Все чаты".into(),
                filter_channels: "Каналы".into(),
                filter_direct: "Личные".into(),
                filter_entities: "Сделки и Заказы".into(),
                composer_placeholder: "Напишите сообщение... (Enter — отправить)".into(),
                composer_send: "Отправить".into(),
                composer_replying_to: "В ответ на".into(),
                composer_cancel_reply: "Отмена".into(),
                status_online: "В сети".into(),
                status_members: "участников".into(),
                drawer_members: "Участники".into(),
                drawer_media: "Медиа и файлы".into(),
                drawer_threads: "Ветки".into(),
                drawer_close: "Закрыть".into(),
                widget_title: "Онлайн-чат".into(),
                widget_support: "Служба поддержки".into(),
                widget_seller: "Чат с продавцом".into(),
                empty_conversations: "Нет доступных диалогов".into(),
                empty_messages: "Сообщений пока нет".into(),
            }
        } else {
            Self {
                title: "Messenger".into(),
                search_placeholder: "Search conversations or messages...".into(),
                filter_all: "All Chats".into(),
                filter_channels: "Channels".into(),
                filter_direct: "Direct".into(),
                filter_entities: "Deals & Orders".into(),
                composer_placeholder: "Write a message... (Enter to send)".into(),
                composer_send: "Send".into(),
                composer_replying_to: "Replying to".into(),
                composer_cancel_reply: "Cancel".into(),
                status_online: "Online".into(),
                status_members: "members".into(),
                drawer_members: "Members".into(),
                drawer_media: "Media & Files".into(),
                drawer_threads: "Threads".into(),
                drawer_close: "Close".into(),
                widget_title: "Live Chat".into(),
                widget_support: "Contact Support".into(),
                widget_seller: "Chat with Seller".into(),
                empty_conversations: "No conversations found".into(),
                empty_messages: "No messages yet".into(),
            }
        }
    }
}

fn format_short_time(dt: DateTime<Utc>) -> String {
    format!("{:02}:{:02}", dt.hour(), dt.minute())
}

fn format_date_header(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    if dt.year() == now.year() && dt.month() == now.month() && dt.day() == now.day() {
        "Today".to_string()
    } else {
        format!("{}-{:02}-{:02}", dt.year(), dt.month(), dt.day())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_message_validation_fails() {
        let mut state = MessengerWorkspaceState::new(Uuid::now_v7());
        state.active_conversation_id = Some(Uuid::now_v7());

        assert!(state.submit_optimistic_message("").is_err());
        assert!(state.submit_optimistic_message("   \n\t  ").is_err());
    }

    #[test]
    fn message_length_boundary_validation() {
        let mut state = MessengerWorkspaceState::new(Uuid::now_v7());
        state.active_conversation_id = Some(Uuid::now_v7());

        let oversized = "a".repeat(MAX_MESSAGE_LENGTH + 1);
        assert!(state.submit_optimistic_message(&oversized).is_err());

        let valid = "a".repeat(MAX_MESSAGE_LENGTH);
        assert!(state.submit_optimistic_message(&valid).is_ok());
    }

    #[test]
    fn optimistic_message_lifecycle_and_ack() {
        let user_id = Uuid::now_v7();
        let conv_id = Uuid::now_v7();
        let mut state = MessengerWorkspaceState::new(user_id);
        state.active_conversation_id = Some(conv_id);

        let (payload, opt_msg) = state
            .submit_optimistic_message("Hello team!")
            .expect("Message submits");

        assert_eq!(opt_msg.content, "Hello team!");
        assert_eq!(opt_msg.delivery_status, DeliveryStatus::Sending);
        assert_eq!(state.messages.len(), 1);

        let client_id = payload.client_msg_id.unwrap();
        state.apply_ack(&client_id, 100500);

        assert_eq!(state.messages[0].id, 100500);
        assert_eq!(state.messages[0].delivery_status, DeliveryStatus::Sent);
    }

    #[test]
    fn read_horizon_advancement() {
        let user_id = Uuid::now_v7();
        let conv_id = Uuid::now_v7();
        let mut state = MessengerWorkspaceState::new(user_id);
        state.active_conversation_id = Some(conv_id);

        state.messages.push(ChatMessageViewModel {
            id: 101,
            conversation_id: conv_id,
            sender_id: user_id,
            sender_display_name: "You".into(),
            content: "First".into(),
            formatted_time: "10:00".into(),
            created_at: Utc::now(),
            is_outgoing: true,
            delivery_status: DeliveryStatus::Sent,
            reply_to_id: None,
            is_encrypted: false,
            client_msg_id: None,
        });

        state.messages.push(ChatMessageViewModel {
            id: 102,
            conversation_id: conv_id,
            sender_id: user_id,
            sender_display_name: "You".into(),
            content: "Second".into(),
            formatted_time: "10:01".into(),
            created_at: Utc::now(),
            is_outgoing: true,
            delivery_status: DeliveryStatus::Sent,
            reply_to_id: None,
            is_encrypted: false,
            client_msg_id: None,
        });

        // Advance up to 101
        state.advance_read_horizon(101);
        assert_eq!(state.messages[0].delivery_status, DeliveryStatus::Read);
        assert_eq!(state.messages[1].delivery_status, DeliveryStatus::Sent);

        // Advance up to 105
        state.advance_read_horizon(105);
        assert_eq!(state.messages[1].delivery_status, DeliveryStatus::Read);
    }

    #[test]
    fn conversation_filter_matches() {
        let conv1 = ConversationItemViewModel {
            id: Uuid::now_v7(),
            conversation_type: "channel".into(),
            title: "General".into(),
            topic: None,
            last_message_snippet: "hi".into(),
            last_message_time: "12:00".into(),
            unread_count: 0,
            unread_badge: None,
            is_encrypted: false,
            is_entity_thread: false,
            entity_badge: None,
        };

        let conv2 = ConversationItemViewModel {
            id: Uuid::now_v7(),
            conversation_type: "direct".into(),
            title: "Alice".into(),
            topic: None,
            last_message_snippet: "hey".into(),
            last_message_time: "12:05".into(),
            unread_count: 2,
            unread_badge: Some("2".into()),
            is_encrypted: true,
            is_entity_thread: false,
            entity_badge: None,
        };

        let conv3 = ConversationItemViewModel {
            id: Uuid::now_v7(),
            conversation_type: "entity_thread".into(),
            title: "Order #42".into(),
            topic: None,
            last_message_snippet: "dispatched".into(),
            last_message_time: "12:10".into(),
            unread_count: 1,
            unread_badge: Some("1".into()),
            is_encrypted: false,
            is_entity_thread: true,
            entity_badge: Some("Order".into()),
        };

        assert!(ConversationFilter::All.matches(&conv1));
        assert!(ConversationFilter::All.matches(&conv2));
        assert!(ConversationFilter::All.matches(&conv3));

        assert!(ConversationFilter::Channels.matches(&conv1));
        assert!(!ConversationFilter::Channels.matches(&conv2));

        assert!(ConversationFilter::Direct.matches(&conv2));
        assert!(!ConversationFilter::Direct.matches(&conv1));

        assert!(ConversationFilter::EntityThreads.matches(&conv3));
        assert!(!ConversationFilter::EntityThreads.matches(&conv1));
    }

    #[test]
    fn total_unread_calculation() {
        let mut state = MessengerWorkspaceState::new(Uuid::now_v7());
        state.set_conversations(&[
            ConversationItemPayload {
                id: Uuid::now_v7(),
                conversation_type: "direct".into(),
                title: Some("Alice".into()),
                topic: None,
                last_message_content: None,
                last_message_at: None,
                unread_count: 3,
                is_encrypted: false,
                entity_type: None,
                entity_id: None,
            },
            ConversationItemPayload {
                id: Uuid::now_v7(),
                conversation_type: "channel".into(),
                title: Some("General".into()),
                topic: None,
                last_message_content: None,
                last_message_at: None,
                unread_count: 5,
                is_encrypted: false,
                entity_type: None,
                entity_id: None,
            },
        ]);

        assert_eq!(state.total_unread_count(), 8);
    }

    #[test]
    fn chronological_message_sorting() {
        let user_id = Uuid::now_v7();
        let conv_id = Uuid::now_v7();
        let mut state = MessengerWorkspaceState::new(user_id);

        // Payloads arriving in descending order (latest first)
        let payloads = vec![
            MessageItemPayload {
                id: 300,
                conversation_id: conv_id,
                sender_id: user_id,
                sender_name: None,
                message_type: 1,
                content: "Third message".into(),
                blocks_json: None,
                reply_to_id: None,
                is_encrypted: false,
                created_at: Utc::now(),
            },
            MessageItemPayload {
                id: 100,
                conversation_id: conv_id,
                sender_id: user_id,
                sender_name: None,
                message_type: 1,
                content: "First message".into(),
                blocks_json: None,
                reply_to_id: None,
                is_encrypted: false,
                created_at: Utc::now(),
            },
            MessageItemPayload {
                id: 200,
                conversation_id: conv_id,
                sender_id: user_id,
                sender_name: None,
                message_type: 1,
                content: "Second message".into(),
                blocks_json: None,
                reply_to_id: None,
                is_encrypted: false,
                created_at: Utc::now(),
            },
        ];

        state.set_messages(&payloads);

        assert_eq!(state.messages.len(), 3);
        assert_eq!(state.messages[0].id, 100);
        assert_eq!(state.messages[1].id, 200);
        assert_eq!(state.messages[2].id, 300);
    }
}

