use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_FORUM_TOPIC_MOVE_REASON_LEN: usize = 500;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMoveCandidate {
    pub id: String,
    pub title: String,
    pub category_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMoveCategory {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMoveCommand {
    pub operation_id: String,
    pub topic_id: String,
    pub target_category_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMoveReceipt {
    pub operation_id: String,
    pub event_id: String,
    pub topic_id: String,
    pub source_category_id: String,
    pub target_category_id: String,
    pub actor_id: String,
    pub reason: String,
    pub published_reply_count: i32,
    pub moved_at: String,
}

static OPERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn forum_topic_move_candidate_label(candidate: &ForumTopicMoveCandidate) -> String {
    format!("{} · {}", candidate.title, candidate.id)
}

pub fn forum_topic_move_category_label(category: &ForumTopicMoveCategory) -> String {
    format!("{} · {}", category.name, category.id)
}

pub fn new_forum_topic_move_operation_id(topic_id: &str, target_category_id: &str) -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = OPERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = elapsed.to_be_bytes();
    let topic_hash = fnv1a64(topic_id.as_bytes());
    let category_hash = fnv1a64(target_category_id.as_bytes());
    for (index, byte) in topic_hash
        .to_be_bytes()
        .into_iter()
        .chain(category_hash.to_be_bytes())
        .enumerate()
    {
        bytes[index] ^= byte;
    }
    for (index, byte) in counter.to_be_bytes().into_iter().enumerate() {
        bytes[index + 8] ^= byte;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid_from_bytes(bytes)
}

pub fn build_forum_topic_move_command(
    topic: &ForumTopicMoveCandidate,
    target_category_id: &str,
    reason: &str,
    operation_id: &str,
) -> Result<ForumTopicMoveCommand, String> {
    let topic_id = topic.id.trim();
    if !looks_like_uuid(topic_id) {
        return Err("Topic identity is invalid".to_string());
    }
    let target_category_id = target_category_id.trim();
    if !looks_like_uuid(target_category_id) {
        return Err("Target category identity is invalid".to_string());
    }
    if topic.category_id.trim() == target_category_id {
        return Err("Target category must differ from the topic's current category".to_string());
    }
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("Move reason is required".to_string());
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_MOVE_REASON_LEN {
        return Err(format!(
            "Move reason must not exceed {MAX_FORUM_TOPIC_MOVE_REASON_LEN} characters"
        ));
    }
    if reason.chars().any(char::is_control) {
        return Err("Move reason must not contain control characters".to_string());
    }
    let operation_id = operation_id.trim();
    if !looks_like_uuid(operation_id) {
        return Err("Move operation identity is invalid".to_string());
    }
    Ok(ForumTopicMoveCommand {
        operation_id: operation_id.to_string(),
        topic_id: topic_id.to_string(),
        target_category_id: target_category_id.to_string(),
        reason: reason.to_string(),
    })
}

fn looks_like_uuid(value: &str) -> bool {
    let mut parts = value.split('-');
    matches!(
        (
            parts.next().map(str::len),
            parts.next().map(str::len),
            parts.next().map(str::len),
            parts.next().map(str::len),
            parts.next().map(str::len),
            parts.next(),
        ),
        (Some(8), Some(4), Some(4), Some(4), Some(12), None)
    ) && value.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn uuid_from_bytes(bytes: [u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_rejects_same_category_and_empty_reason() {
        let topic = ForumTopicMoveCandidate {
            id: "11111111-1111-4111-8111-111111111111".into(),
            title: "Source".into(),
            category_id: "22222222-2222-4222-8222-222222222222".into(),
        };
        assert!(
            build_forum_topic_move_command(
                &topic,
                "22222222-2222-4222-8222-222222222222",
                "move it",
                "33333333-3333-4333-8333-333333333333",
            )
            .is_err()
        );
        assert!(
            build_forum_topic_move_command(
                &topic,
                "44444444-4444-4444-8444-444444444444",
                "   ",
                "33333333-3333-4333-8333-333333333333",
            )
            .is_err()
        );
    }
}
