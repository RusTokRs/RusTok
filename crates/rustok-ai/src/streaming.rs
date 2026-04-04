#![cfg(feature = "server")]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiRunStreamEventKind {
    Started,
    Delta,
    Completed,
    Failed,
    WaitingApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRunStreamEvent {
    pub session_id: Uuid,
    pub run_id: Uuid,
    pub event_kind: AiRunStreamEventKind,
    pub content_delta: Option<String>,
    pub accumulated_content: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct AiRunStreamHub {
    sender: broadcast::Sender<AiRunStreamEvent>,
    recent: Mutex<VecDeque<AiRunStreamEvent>>,
    recent_limit: usize,
}

impl AiRunStreamHub {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer.max(32));
        Self {
            sender,
            recent: Mutex::new(VecDeque::with_capacity(buffer.max(1))),
            recent_limit: buffer.max(1),
        }
    }

    pub fn publish(&self, event: AiRunStreamEvent) {
        if let Ok(mut recent) = self.recent.lock() {
            recent.push_front(event.clone());
            while recent.len() > self.recent_limit {
                recent.pop_back();
            }
        }
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AiRunStreamEvent> {
        self.sender.subscribe()
    }

    pub fn recent_events(&self, session_id: Option<Uuid>, limit: usize) -> Vec<AiRunStreamEvent> {
        let Ok(recent) = self.recent.lock() else {
            return Vec::new();
        };
        recent
            .iter()
            .filter(|event| session_id.is_none_or(|value| event.session_id == value))
            .take(limit.max(1))
            .cloned()
            .collect()
    }
}

static AI_RUN_STREAM_HUB: Lazy<Arc<AiRunStreamHub>> =
    Lazy::new(|| Arc::new(AiRunStreamHub::new(512)));

pub fn ai_run_stream_hub() -> Arc<AiRunStreamHub> {
    Arc::clone(&AI_RUN_STREAM_HUB)
}

#[cfg(test)]
mod tests {
    use super::{AiRunStreamEvent, AiRunStreamEventKind, AiRunStreamHub};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn recent_events_are_bounded_and_newest_first() {
        let hub = AiRunStreamHub::new(2);
        let session_id = Uuid::new_v4();
        let run_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

        for run_id in run_ids {
            hub.publish(AiRunStreamEvent {
                session_id,
                run_id,
                event_kind: AiRunStreamEventKind::Started,
                content_delta: None,
                accumulated_content: None,
                error_message: None,
                created_at: Utc::now(),
            });
        }

        let recent = hub.recent_events(Some(session_id), 10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].run_id, run_ids[2]);
        assert_eq!(recent[1].run_id, run_ids[1]);
    }

    #[test]
    fn recent_events_can_filter_by_session() {
        let hub = AiRunStreamHub::new(8);
        let session_a = Uuid::new_v4();
        let session_b = Uuid::new_v4();
        let run_a = Uuid::new_v4();
        let run_b = Uuid::new_v4();

        hub.publish(AiRunStreamEvent {
            session_id: session_a,
            run_id: run_a,
            event_kind: AiRunStreamEventKind::Completed,
            content_delta: None,
            accumulated_content: Some("a".to_string()),
            error_message: None,
            created_at: Utc::now(),
        });
        hub.publish(AiRunStreamEvent {
            session_id: session_b,
            run_id: run_b,
            event_kind: AiRunStreamEventKind::Completed,
            content_delta: None,
            accumulated_content: Some("b".to_string()),
            error_message: None,
            created_at: Utc::now(),
        });

        let recent = hub.recent_events(Some(session_a), 10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].run_id, run_a);
    }
}
