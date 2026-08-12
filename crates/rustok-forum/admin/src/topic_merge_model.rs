use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMergeCandidate {
    pub id: String,
    pub title: String,
    pub category_id: String,
    pub reply_count: i32,
    pub solution_reply_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForumTopicMergeWinner {
    Source,
    Target,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMergeCommand {
    pub operation_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub reason: String,
    pub selected_solution_reply_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicMergeReceipt {
    pub operation_id: String,
    pub event_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub category_id: String,
    pub actor_id: String,
    pub reason: String,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub resulting_published_reply_count: i32,
    pub position_offset: i64,
    pub merged_at: String,
}

pub fn forum_topic_merge_requires_solution_choice(
    source: &ForumTopicMergeCandidate,
    target: &ForumTopicMergeCandidate,
) -> bool {
    source.solution_reply_id.is_some() && target.solution_reply_id.is_some()
}

pub fn forum_topic_merge_candidate_label(candidate: &ForumTopicMergeCandidate) -> String {
    let solved = if candidate.solution_reply_id.is_some() {
        " · solved"
    } else {
        ""
    };
    format!(
        "{} · {} replies{}",
        candidate.title, candidate.reply_count, solved
    )
}

pub fn build_forum_topic_merge_command(
    operation_id: &str,
    source: &ForumTopicMergeCandidate,
    target: &ForumTopicMergeCandidate,
    reason: &str,
    winner: Option<ForumTopicMergeWinner>,
) -> Result<ForumTopicMergeCommand, String> {
    let operation_id = operation_id.trim();
    if !looks_like_uuid(operation_id) {
        return Err("Merge operation identity is invalid".to_string());
    }
    if source.id == target.id {
        return Err("Source and retained target topics must be different".to_string());
    }

    let reason = reason.trim();
    if reason.is_empty() {
        return Err("Merge reason is required".to_string());
    }
    if reason.chars().count() > MAX_FORUM_TOPIC_MERGE_REASON_LEN {
        return Err(format!(
            "Merge reason must not exceed {MAX_FORUM_TOPIC_MERGE_REASON_LEN} characters"
        ));
    }
    if reason.chars().any(char::is_control) {
        return Err("Merge reason must not contain control characters".to_string());
    }

    let selected_solution_reply_id = match (
        source.solution_reply_id.as_ref(),
        target.solution_reply_id.as_ref(),
    ) {
        (Some(source_solution), Some(target_solution)) => match winner {
            Some(ForumTopicMergeWinner::Source) => Some(source_solution.clone()),
            Some(ForumTopicMergeWinner::Target) => Some(target_solution.clone()),
            None => {
                return Err(
                    "Choose which accepted solution must remain after the merge".to_string()
                );
            }
        },
        _ if winner.is_some() => {
            return Err(
                "A solution winner can be selected only when both topics are solved".to_string(),
            );
        }
        _ => None,
    };

    Ok(ForumTopicMergeCommand {
        operation_id: operation_id.to_string(),
        source_topic_id: source.id.clone(),
        target_topic_id: target.id.clone(),
        reason: reason.to_string(),
        selected_solution_reply_id,
    })
}

static OPERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn new_forum_topic_merge_operation_id(source_topic_id: &str, target_topic_id: &str) -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = OPERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = elapsed.to_be_bytes();
    let source_hash = fnv1a64(source_topic_id.as_bytes());
    let target_hash = fnv1a64(target_topic_id.as_bytes());
    for (index, byte) in source_hash
        .to_be_bytes()
        .into_iter()
        .chain(target_hash.to_be_bytes())
        .enumerate()
    {
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

    fn candidate(id: &str, solution: Option<&str>) -> ForumTopicMergeCandidate {
        ForumTopicMergeCandidate {
            id: id.to_string(),
            title: id.to_string(),
            category_id: "category".to_string(),
            reply_count: 2,
            solution_reply_id: solution.map(str::to_string),
        }
    }

    #[test]
    fn ordinary_merge_reuses_exact_operation_identity() {
        let source = candidate("00000000-0000-4000-8000-000000000001", None);
        let target = candidate("00000000-0000-4000-8000-000000000002", None);
        let operation_id = "00000000-0000-4000-8000-000000000003";
        let command = build_forum_topic_merge_command(
            operation_id,
            &source,
            &target,
            "  duplicate thread  ",
            None,
        )
        .expect("command");
        assert_eq!(command.operation_id, operation_id);
        assert_eq!(command.reason, "duplicate thread");
        assert_eq!(command.selected_solution_reply_id, None);
    }

    #[test]
    fn competing_solutions_require_an_explicit_exact_winner() {
        let source = candidate(
            "00000000-0000-4000-8000-000000000001",
            Some("00000000-0000-4000-8000-000000000011"),
        );
        let target = candidate(
            "00000000-0000-4000-8000-000000000002",
            Some("00000000-0000-4000-8000-000000000022"),
        );
        let operation_id = "00000000-0000-4000-8000-000000000003";
        assert!(
            build_forum_topic_merge_command(operation_id, &source, &target, "merge", None,)
                .is_err()
        );
        let command = build_forum_topic_merge_command(
            operation_id,
            &source,
            &target,
            "merge",
            Some(ForumTopicMergeWinner::Target),
        )
        .expect("resolved command");
        assert_eq!(
            command.selected_solution_reply_id.as_deref(),
            target.solution_reply_id.as_deref()
        );
    }

    #[test]
    fn command_shape_change_gets_a_new_uuid_v4_identity() {
        let first = new_forum_topic_merge_operation_id("source", "target");
        let second = new_forum_topic_merge_operation_id("source", "target");
        assert_ne!(first, second);
        assert!(looks_like_uuid(first.as_str()));
        assert_eq!(first.as_bytes()[14], b'4');
        assert!(matches!(first.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
    }
}
