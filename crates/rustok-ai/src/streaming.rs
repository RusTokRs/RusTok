#![cfg(feature = "server")]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiRunStreamEventKind {
    Started,
    Delta,
    ToolCall,
    Usage,
    Completed,
    Failed,
    Cancelled,
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
    /// Canonical assembled tool call, when the event kind is `ToolCall`.
    pub tool_call: Option<crate::model::ToolCall>,
    pub usage: Option<crate::model::ProviderUsage>,
    /// Monotonic per-run sequence assigned by the stream hub.
    pub sequence: u64,
    pub created_at: DateTime<Utc>,
}

pub struct AiRunStreamHub {
    sender: broadcast::Sender<AiRunStreamEvent>,
    recent: Mutex<VecDeque<AiRunStreamEvent>>,
    run_state: Mutex<HashMap<Uuid, RunStreamState>>,
    recent_limit: usize,
}

#[derive(Default)]
struct RunStreamState {
    sequence: u64,
    terminal: bool,
}

impl AiRunStreamHub {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer.max(32));
        Self {
            sender,
            recent: Mutex::new(VecDeque::with_capacity(buffer.max(1))),
            run_state: Mutex::new(HashMap::new()),
            recent_limit: buffer.max(1),
        }
    }

    /// Publishes one event, assigning its sequence and rejecting a second terminal event.
    /// The return value reports whether subscribers received the event.
    pub fn publish(&self, mut event: AiRunStreamEvent) -> bool {
        let terminal = matches!(
            event.event_kind,
            AiRunStreamEventKind::Completed
                | AiRunStreamEventKind::Failed
                | AiRunStreamEventKind::Cancelled
        );
        let Ok(mut states) = self.run_state.lock() else {
            return false;
        };
        let state = states.entry(event.run_id).or_default();
        if state.terminal {
            return false;
        }
        state.sequence += 1;
        event.sequence = state.sequence;
        if terminal {
            state.terminal = true;
        }
        drop(states);
        let mut evicted_runs = Vec::new();
        if let Ok(mut recent) = self.recent.lock() {
            recent.push_front(event.clone());
            while recent.len() > self.recent_limit {
                if let Some(evicted) = recent.pop_back() {
                    evicted_runs.push(evicted.run_id);
                }
            }
            if !evicted_runs.is_empty() {
                let retained_runs = recent
                    .iter()
                    .map(|retained| retained.run_id)
                    .collect::<std::collections::HashSet<_>>();
                if let Ok(mut states) = self.run_state.lock() {
                    for run_id in evicted_runs {
                        if !retained_runs.contains(&run_id)
                            && states.get(&run_id).is_some_and(|state| state.terminal)
                        {
                            states.remove(&run_id);
                        }
                    }
                }
            }
        }
        let _ = self.sender.send(event);
        true
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
    use crate::model::ProviderUsage;
    use crate::model::ToolCall;
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
                tool_call: None,
                usage: None,
                sequence: 0,
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
            tool_call: None,
            usage: None,
            sequence: 0,
            created_at: Utc::now(),
        });
        hub.publish(AiRunStreamEvent {
            session_id: session_b,
            run_id: run_b,
            event_kind: AiRunStreamEventKind::Completed,
            content_delta: None,
            accumulated_content: Some("b".to_string()),
            error_message: None,
            tool_call: None,
            usage: None,
            sequence: 0,
            created_at: Utc::now(),
        });

        let recent = hub.recent_events(Some(session_a), 10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].run_id, run_a);
    }

    #[test]
    fn assigns_monotonic_sequence_and_rejects_duplicate_terminal_event() {
        let hub = AiRunStreamHub::new(8);
        let session_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        for event_kind in [
            AiRunStreamEventKind::Started,
            AiRunStreamEventKind::Completed,
        ] {
            assert!(hub.publish(AiRunStreamEvent {
                session_id,
                run_id,
                event_kind,
                content_delta: None,
                accumulated_content: None,
                error_message: None,
                tool_call: None,
                usage: None,
                sequence: 0,
                created_at: Utc::now(),
            }));
        }
        assert!(!hub.publish(AiRunStreamEvent {
            session_id,
            run_id,
            event_kind: AiRunStreamEventKind::Failed,
            content_delta: None,
            accumulated_content: None,
            error_message: None,
            tool_call: None,
            usage: None,
            sequence: 0,
            created_at: Utc::now(),
        }));
        let recent = hub.recent_events(Some(session_id), 10);
        assert_eq!(recent[0].sequence, 2);
        assert_eq!(recent[1].sequence, 1);
    }

    #[test]
    fn preserves_assembled_tool_call_payload() {
        let hub = AiRunStreamHub::new(8);
        let session_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        assert!(hub.publish(AiRunStreamEvent {
            session_id,
            run_id,
            event_kind: AiRunStreamEventKind::ToolCall,
            content_delta: None,
            accumulated_content: None,
            error_message: None,
            tool_call: Some(ToolCall {
                id: "call_1".to_string(),
                name: "inventory_lookup".to_string(),
                arguments: serde_json::json!({"sku": "A-1"}),
            }),
            usage: None,
            sequence: 0,
            created_at: Utc::now(),
        }));
        let event = hub.recent_events(Some(session_id), 1).pop().unwrap();
        assert_eq!(event.sequence, 1);
        assert_eq!(event.tool_call.unwrap().name, "inventory_lookup");
    }

    #[test]
    fn preserves_usage_payload() {
        let hub = AiRunStreamHub::new(8);
        let session_id = Uuid::new_v4();
        assert!(hub.publish(AiRunStreamEvent {
            session_id,
            run_id: Uuid::new_v4(),
            event_kind: AiRunStreamEventKind::Usage,
            content_delta: None,
            accumulated_content: None,
            error_message: None,
            tool_call: None,
            usage: Some(ProviderUsage {
                input_tokens: 3,
                output_tokens: 5,
                total_tokens: 8,
            }),
            sequence: 0,
            created_at: Utc::now(),
        }));
        assert_eq!(
            hub.recent_events(Some(session_id), 1)[0]
                .usage
                .as_ref()
                .unwrap()
                .total_tokens,
            8
        );
    }
}
