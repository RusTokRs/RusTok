use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MAX_FORUM_TOPIC_FORK_REASON_LEN: usize = 500;
pub const MAX_FORUM_TOPIC_FORK_TITLE_LEN: usize = 500;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicForkCandidate {
    pub id: String,
    pub locale: String,
    pub title: String,
    pub category_id: String,
    pub reply_count: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicForkReply {
    pub id: String,
    pub content_preview: String,
    pub status: String,
    pub parent_reply_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicForkReplyPage {
    pub total: i64,
    pub items: Vec<ForumTopicForkReply>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForumTopicForkIdentity {
    pub operation_id: String,
    pub target_topic_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicForkCommand {
    pub operation_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub root_reply_id: String,
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicForkReceipt {
    pub operation_id: String,
    pub event_id: String,
    pub source_topic_id: String,
    pub target_topic_id: String,
    pub root_reply_id: String,
    pub category_id: String,
    pub actor_id: String,
    pub reason: String,
    pub copied_reply_count: i32,
    pub copied_published_reply_count: i32,
    pub copied_body_count: i32,
    pub copied_reply_revision_count: i32,
    pub copied_relation_revision_count: i32,
    pub copied_mention_count: i32,
    pub copied_quote_count: i32,
    pub forked_at: String,
}

pub fn forum_topic_fork_candidate_label(candidate: &ForumTopicForkCandidate) -> String {
    format!("{} · {} replies", candidate.title, candidate.reply_count)
}

pub fn forum_topic_fork_reply_label(reply: &ForumTopicForkReply) -> String {
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

pub fn build_forum_topic_fork_command(
    identity: &ForumTopicForkIdentity,
    source_topic_id: &str,
    replies: &ForumTopicForkReplyPage,
    root_reply_id: &str,
    locale: &str,
    title: &str,
    slug: &str,
    reason: &str,
) -> Result<ForumTopicForkCommand, String> {
    let operation_id = identity.operation_id.trim();
    let target_topic_id = identity.target_topic_id.trim();
    let source_topic_id = source_topic_id.trim();
    let root_reply_id = root_reply_id.trim();

    if !looks_like_uuid(operation_id) || !looks_like_uuid(target_topic_id) {
        return Err("Fork retry identity is invalid".to_string());
    }
    if !looks_like_uuid(source_topic_id) {
        return Err("Choose the source topic to fork".to_string());
    }
    if source_topic_id == target_topic_id {
        return Err("The new topic identity must differ from the source topic".to_string());
    }
    if !looks_like_uuid(root_reply_id) {
        return Err("Choose the root reply to fork".to_string());
    }
    if !replies.items.iter().any(|reply| reply.id == root_reply_id) {
        return Err("The selected root reply is not loaded for this topic".to_string());
    }

    let locale = validate_text(locale, "Target locale", 64)?;
    let title = validate_text(title, "Target title", MAX_FORUM_TOPIC_FORK_TITLE_LEN)?;
    let reason = validate_text(reason, "Fork reason", MAX_FORUM_TOPIC_FORK_REASON_LEN)?;
    let slug = normalize_optional_text(slug, "Target slug", 255)?;

    Ok(ForumTopicForkCommand {
        operation_id: operation_id.to_string(),
        source_topic_id: source_topic_id.to_string(),
        target_topic_id: target_topic_id.to_string(),
        root_reply_id: root_reply_id.to_string(),
        locale,
        title,
        slug,
        reason,
    })
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

pub fn new_forum_topic_fork_identity(source_topic_id: &str) -> ForumTopicForkIdentity {
    ForumTopicForkIdentity {
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

    fn reply(id: &str) -> ForumTopicForkReply {
        ForumTopicForkReply {
            id: id.to_string(),
            content_preview: id.to_string(),
            status: "approved".to_string(),
            parent_reply_id: None,
            created_at: "2026-08-05T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn exact_command_keeps_retry_and_target_identities() {
        let source = "00000000-0000-4000-8000-000000000001";
        let root = "00000000-0000-4000-8000-000000000011";
        let identity = ForumTopicForkIdentity {
            operation_id: "00000000-0000-4000-8000-000000000002".to_string(),
            target_topic_id: "00000000-0000-4000-8000-000000000003".to_string(),
        };
        let replies = ForumTopicForkReplyPage {
            total: 1,
            items: vec![reply(root)],
        };
        let command = build_forum_topic_fork_command(
            &identity,
            source,
            &replies,
            root,
            "en",
            "Forked branch",
            "forked-branch",
            "Preserve a focused branch",
        )
        .expect("command");
        assert_eq!(command.operation_id, identity.operation_id);
        assert_eq!(command.target_topic_id, identity.target_topic_id);
        assert_eq!(command.root_reply_id, root);
    }

    #[test]
    fn unloaded_root_is_rejected_before_transport() {
        let replies = ForumTopicForkReplyPage {
            total: 1,
            items: vec![reply("00000000-0000-4000-8000-000000000011")],
        };
        let identity = new_forum_topic_fork_identity("source");
        let error = build_forum_topic_fork_command(
            &identity,
            "00000000-0000-4000-8000-000000000001",
            &replies,
            "00000000-0000-4000-8000-000000000012",
            "en",
            "Forked branch",
            "",
            "Preserve a focused branch",
        )
        .expect_err("unloaded root must fail");
        assert!(error.contains("not loaded"));
    }

    #[test]
    fn changed_shape_rotates_both_identities() {
        let first = new_forum_topic_fork_identity("source");
        let second = new_forum_topic_fork_identity("source");
        assert_ne!(first, second);
        assert!(looks_like_uuid(first.operation_id.as_str()));
        assert!(looks_like_uuid(first.target_topic_id.as_str()));
    }
}
