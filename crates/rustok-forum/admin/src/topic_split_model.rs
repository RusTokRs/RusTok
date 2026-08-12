use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_FORUM_TOPIC_SPLIT_REPLIES: usize = 500;
pub const MAX_FORUM_TOPIC_SPLIT_REASON_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_SPLIT_TITLE_LEN: usize = 500;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSplitCandidate {
    pub id: String,
    pub title: String,
    pub category_id: String,
    pub reply_count: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSplitReply {
    pub id: String,
    pub content_preview: String,
    pub status: String,
    pub parent_reply_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSplitReplyPage {
    pub total: i64,
    pub items: Vec<ForumTopicSplitReply>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumTopicSplitIdentity {
    pub operation_id: String,
    pub target_topic_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSplitCommand {
    pub operation_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub reply_ids: Vec<String>,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSplitReceipt {
    pub operation_id: String,
    pub event_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub category_id: String,
    pub actor_id: String,
    pub reason: String,
    pub moved_reply_count: i32,
    pub moved_published_reply_count: i32,
    pub source_resulting_published_reply_count: i32,
    pub target_resulting_published_reply_count: i32,
    pub solution_reply_id: Option<String>,
    pub split_at: String,
}

pub fn forum_topic_split_candidate_label(candidate: &ForumTopicSplitCandidate) -> String {
    format!("{} · {} replies", candidate.title, candidate.reply_count)
}

pub fn forum_topic_split_reply_label(reply: &ForumTopicSplitReply) -> String {
    let preview = reply.content_preview.trim();
    let preview = if preview.is_empty() {
        "(empty reply)"
    } else {
        preview
    };
    let parent = reply
        .parent_reply_id
        .as_deref()
        .map(|_| " · child")
        .unwrap_or_default();
    format!("{} · {}{}", preview, reply.status, parent)
}

pub fn build_forum_topic_split_command(
    identity: &ForumTopicSplitIdentity,
    source_topic_id: &str,
    replies: &ForumTopicSplitReplyPage,
    selected_reply_ids: &[String],
    locale: &str,
    title: &str,
    slug: &str,
    reason: &str,
) -> Result<ForumTopicSplitCommand, String> {
    let operation_id = identity.operation_id.trim();
    let target_topic_id = identity.target_topic_id.trim();
    let source_topic_id = source_topic_id.trim();
    if !looks_like_uuid(operation_id) || !looks_like_uuid(target_topic_id) {
        return Err("Split retry identity is invalid".to_string());
    }
    if !looks_like_uuid(source_topic_id) {
        return Err("Choose the source topic to split".to_string());
    }
    if source_topic_id == target_topic_id {
        return Err("The new topic identity must differ from the source topic".to_string());
    }

    let locale = validate_text(locale, "Target locale", 64)?;
    let title = validate_text(title, "Target title", MAX_FORUM_TOPIC_SPLIT_TITLE_LEN)?;
    let reason = validate_text(reason, "Split reason", MAX_FORUM_TOPIC_SPLIT_REASON_LEN)?;
    let slug = normalize_optional_text(slug, "Target slug", 255)?;

    if selected_reply_ids.is_empty() {
        return Err("Select at least one reply to move".to_string());
    }
    if selected_reply_ids.len() > MAX_FORUM_TOPIC_SPLIT_REPLIES {
        return Err(format!(
            "A split may move at most {MAX_FORUM_TOPIC_SPLIT_REPLIES} replies"
        ));
    }

    let mut reply_ids = selected_reply_ids
        .iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    if reply_ids.iter().any(|value| !looks_like_uuid(value)) {
        return Err("Every selected reply identity must be a UUID".to_string());
    }
    reply_ids.sort();
    reply_ids.dedup();
    if reply_ids.len() != selected_reply_ids.len() {
        return Err("Selected reply identities must be unique".to_string());
    }
    if i64::try_from(reply_ids.len()).unwrap_or(i64::MAX) >= replies.total {
        return Err("The source topic must retain at least one reply".to_string());
    }

    validate_parent_closed_selection(replies, &reply_ids)?;

    Ok(ForumTopicSplitCommand {
        operation_id: operation_id.to_string(),
        source_topic_id: source_topic_id.to_string(),
        target_topic_id: target_topic_id.to_string(),
        reply_ids,
        locale,
        title,
        slug,
        reason,
    })
}

fn validate_parent_closed_selection(
    replies: &ForumTopicSplitReplyPage,
    selected_reply_ids: &[String],
) -> Result<(), String> {
    let by_id = replies
        .items
        .iter()
        .map(|reply| (reply.id.as_str(), reply))
        .collect::<HashMap<_, _>>();
    let selected = selected_reply_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    for reply_id in &selected {
        let Some(reply) = by_id.get(reply_id).copied() else {
            return Err(format!("Selected reply is not loaded: {reply_id}"));
        };
        if let Some(parent_reply_id) = reply.parent_reply_id.as_deref()
            && by_id.contains_key(parent_reply_id)
            && !selected.contains(parent_reply_id)
        {
            return Err("A selected child reply requires its parent to be selected".to_string());
        }
    }

    for reply in &replies.items {
        if let Some(parent_reply_id) = reply.parent_reply_id.as_deref()
            && selected.contains(parent_reply_id)
            && !selected.contains(reply.id.as_str())
        {
            return Err(
                "Selecting a parent requires every loaded child to be selected".to_string(),
            );
        }
    }

    Ok(())
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

fn normalize_optional_text(
    value: &str,
    label: &str,
    maximum: usize,
) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > maximum {
        return Err(format!("{label} must not exceed {maximum} characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{label} must not contain control characters"));
    }
    Ok(Some(value.to_string()))
}

static IDENTITY_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn new_forum_topic_split_identity(source_topic_id: &str) -> ForumTopicSplitIdentity {
    ForumTopicSplitIdentity {
        operation_id: new_uuid_v4(source_topic_id, b"operation"),
        target_topic_id: new_uuid_v4(source_topic_id, b"target"),
    }
}

fn new_uuid_v4(seed: &str, discriminator: &[u8]) -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = IDENTITY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = elapsed.to_be_bytes();
    let seed_hash = fnv1a64(seed.as_bytes());
    let discriminator_hash = fnv1a64(discriminator);
    for (index, byte) in seed_hash
        .to_be_bytes()
        .into_iter()
        .chain(discriminator_hash.to_be_bytes())
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

    fn reply(id: &str, parent_reply_id: Option<&str>) -> ForumTopicSplitReply {
        ForumTopicSplitReply {
            id: id.to_string(),
            content_preview: id.to_string(),
            status: "approved".to_string(),
            parent_reply_id: parent_reply_id.map(str::to_string),
            created_at: "2026-08-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn exact_command_keeps_retry_and_target_identities() {
        let source = "00000000-0000-4000-8000-000000000001";
        let identity = ForumTopicSplitIdentity {
            operation_id: "00000000-0000-4000-8000-000000000002".to_string(),
            target_topic_id: "00000000-0000-4000-8000-000000000003".to_string(),
        };
        let replies = ForumTopicSplitReplyPage {
            total: 3,
            items: vec![
                reply("00000000-0000-4000-8000-000000000011", None),
                reply(
                    "00000000-0000-4000-8000-000000000012",
                    Some("00000000-0000-4000-8000-000000000011"),
                ),
                reply("00000000-0000-4000-8000-000000000013", None),
            ],
        };
        let command = build_forum_topic_split_command(
            &identity,
            source,
            &replies,
            &[
                "00000000-0000-4000-8000-000000000011".to_string(),
                "00000000-0000-4000-8000-000000000012".to_string(),
            ],
            "en",
            "New topic",
            "new-topic",
            "Focused discussion",
        )
        .expect("command");
        assert_eq!(command.operation_id, identity.operation_id);
        assert_eq!(command.target_topic_id, identity.target_topic_id);
        assert_eq!(command.reply_ids.len(), 2);
    }

    #[test]
    fn parent_boundary_is_rejected_before_transport() {
        let replies = ForumTopicSplitReplyPage {
            total: 3,
            items: vec![
                reply("00000000-0000-4000-8000-000000000011", None),
                reply(
                    "00000000-0000-4000-8000-000000000012",
                    Some("00000000-0000-4000-8000-000000000011"),
                ),
                reply("00000000-0000-4000-8000-000000000013", None),
            ],
        };
        let identity = new_forum_topic_split_identity("source");
        let error = build_forum_topic_split_command(
            &identity,
            "00000000-0000-4000-8000-000000000001",
            &replies,
            &["00000000-0000-4000-8000-000000000012".to_string()],
            "en",
            "New topic",
            "",
            "Focused discussion",
        )
        .expect_err("child without parent must fail");
        assert!(error.contains("parent"));
    }

    #[test]
    fn changed_shape_rotates_both_identities() {
        let first = new_forum_topic_split_identity("source");
        let second = new_forum_topic_split_identity("source");
        assert_ne!(first, second);
        assert!(looks_like_uuid(first.operation_id.as_str()));
        assert!(looks_like_uuid(first.target_topic_id.as_str()));
    }
}
