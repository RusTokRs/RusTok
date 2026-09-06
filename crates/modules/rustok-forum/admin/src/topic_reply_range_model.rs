use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN: usize = 500;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumReplyRangeMoveCandidate {
    pub id: String,
    pub locale: String,
    pub title: String,
    pub category_id: String,
    pub reply_count: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumReplyRangeMoveIdentity {
    pub operation_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumReplyRangeMoveCommand {
    pub operation_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub start_position: i64,
    pub end_position: i64,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumReplyRangeMoveReceipt {
    pub operation_id: String,
    pub event_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub source_category_id: String,
    pub target_category_id: String,
    pub actor_id: String,
    pub reason: String,
    pub source_start_position: i64,
    pub source_end_position: i64,
    pub target_start_position: i64,
    pub target_end_position: i64,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub source_resulting_published_reply_count: i32,
    pub target_resulting_published_reply_count: i32,
    pub moved_solution_reply_id: Option<String>,
    pub source_resulting_solution_reply_id: Option<String>,
    pub target_resulting_solution_reply_id: Option<String>,
    pub moved_at: String,
}

pub fn forum_reply_range_move_candidate_label(candidate: &ForumReplyRangeMoveCandidate) -> String {
    format!("{} · {} replies", candidate.title, candidate.reply_count)
}

pub fn build_forum_reply_range_move_command(
    identity: &ForumReplyRangeMoveIdentity,
    source_topic_id: &str,
    target_topic_id: &str,
    start_position: &str,
    end_position: &str,
    reason: &str,
) -> Result<ForumReplyRangeMoveCommand, String> {
    let operation_id = identity.operation_id.trim();
    let source_topic_id = source_topic_id.trim();
    let target_topic_id = target_topic_id.trim();

    if !looks_like_uuid(operation_id) {
        return Err("Reply-range retry identity is invalid".to_string());
    }
    if !looks_like_uuid(source_topic_id) {
        return Err("Choose the source topic".to_string());
    }
    if !looks_like_uuid(target_topic_id) {
        return Err("Choose the target topic".to_string());
    }
    if source_topic_id == target_topic_id {
        return Err("Source and target topics must differ".to_string());
    }

    let start_position = parse_positive_position(start_position, "Start position")?;
    let end_position = parse_positive_position(end_position, "End position")?;
    if start_position > end_position {
        return Err("Start position must not exceed end position".to_string());
    }
    let reason = validate_text(reason, "Move reason", MAX_FORUM_REPLY_RANGE_MOVE_REASON_LEN)?;

    Ok(ForumReplyRangeMoveCommand {
        operation_id: operation_id.to_string(),
        source_topic_id: source_topic_id.to_string(),
        target_topic_id: target_topic_id.to_string(),
        start_position,
        end_position,
        reason,
    })
}

fn parse_positive_position(value: &str, label: &str) -> Result<i64, String> {
    let value = value
        .trim()
        .parse::<i64>()
        .map_err(|_| format!("{label} must be a positive integer"))?;
    if value < 1 {
        return Err(format!("{label} must be a positive integer"));
    }
    Ok(value)
}

fn validate_text(value: &str, label: &str, maximum: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{label} is required"));
    }
    if value.chars().count() > maximum {
        return Err(format!("{label} must not exceed {maximum} characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label} must not contain control characters"));
    }
    Ok(value.to_string())
}

static IDENTITY_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn new_forum_reply_range_move_identity(seed: &str) -> ForumReplyRangeMoveIdentity {
    ForumReplyRangeMoveIdentity {
        operation_id: new_uuid_v4(seed),
    }
}

fn new_uuid_v4(seed: &str) -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = IDENTITY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = elapsed.to_be_bytes();
    let seed_hash = fnv1a64(seed.as_bytes());
    for (index, byte) in seed_hash.to_be_bytes().into_iter().enumerate() {
        bytes[index] ^= byte;
    }
    for (index, byte) in counter.to_be_bytes().into_iter().enumerate() {
        bytes[index + 8] ^= byte;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format_uuid(bytes)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn format_uuid(bytes: [u8; 16]) -> String {
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

fn looks_like_uuid(value: &str) -> bool {
    let groups = value.split('-').collect::<Vec<_>>();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, len)| {
            group.len() == len && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_command_keeps_the_retry_identity() {
        let identity = ForumReplyRangeMoveIdentity {
            operation_id: "00000000-0000-4000-8000-000000000001".to_string(),
        };
        let command = build_forum_reply_range_move_command(
            &identity,
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003",
            "7",
            "11",
            "Move the focused discussion",
        )
        .expect("command");
        assert_eq!(command.operation_id, identity.operation_id);
        assert_eq!(command.start_position, 7);
        assert_eq!(command.end_position, 11);
    }

    #[test]
    fn reversed_or_nonpositive_ranges_fail_before_transport() {
        let identity = new_forum_reply_range_move_identity("range");
        let reversed = build_forum_reply_range_move_command(
            &identity,
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003",
            "9",
            "4",
            "Move the focused discussion",
        )
        .expect_err("reversed range");
        assert!(reversed.contains("must not exceed"));

        let nonpositive = build_forum_reply_range_move_command(
            &identity,
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003",
            "0",
            "4",
            "Move the focused discussion",
        )
        .expect_err("nonpositive range");
        assert!(nonpositive.contains("positive integer"));
    }

    #[test]
    fn changed_shape_rotates_the_operation_identity() {
        let first = new_forum_reply_range_move_identity("source-target-1-2");
        let second = new_forum_reply_range_move_identity("source-target-1-2");
        assert_ne!(first, second);
        assert!(looks_like_uuid(first.operation_id.as_str()));
    }
}
