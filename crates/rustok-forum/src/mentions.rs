use std::collections::{BTreeMap, BTreeSet};

use rustok_api::normalize_locale_tag;
use rustok_core::{
    normalize_content_format, validate_and_sanitize_rt_json, RtJsonValidationConfig,
    CONTENT_FORMAT_MARKDOWN, CONTENT_FORMAT_RT_JSON_V1,
};
use rustok_profiles::{
    ProfileError, ProfileRecord, ProfileService, ProfileStatus, ProfileVisibility, ProfilesReader,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub const FORUM_MAX_MENTION_TARGETS_PER_REVISION: usize = 32;
pub const FORUM_MAX_QUOTE_REFERENCES_PER_REVISION: usize = 32;
pub const FORUM_MENTION_PROFILES_CAPABILITY: &str = "forum.mentions.profiles";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForumMentionPolicy {
    pub max_targets: usize,
    pub allow_moderator_audience: bool,
}

impl Default for ForumMentionPolicy {
    fn default() -> Self {
        Self {
            max_targets: FORUM_MAX_MENTION_TARGETS_PER_REVISION,
            allow_moderator_audience: false,
        }
    }
}

impl ForumMentionPolicy {
    pub fn validated(self) -> ForumResult<Self> {
        if !(1..=FORUM_MAX_MENTION_TARGETS_PER_REVISION).contains(&self.max_targets) {
            return Err(ForumError::Validation(format!(
                "Forum mention limit must be between 1 and {FORUM_MAX_MENTION_TARGETS_PER_REVISION}"
            )));
        }
        Ok(self)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ForumMentionAudience {
    Moderators,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumMentionCandidates {
    handles: Vec<String>,
    audiences: Vec<ForumMentionAudience>,
}

impl ForumMentionCandidates {
    pub fn new(
        handles: impl IntoIterator<Item = String>,
        audiences: impl IntoIterator<Item = ForumMentionAudience>,
    ) -> ForumResult<Self> {
        let handles = handles
            .into_iter()
            .filter_map(|handle| ProfileService::normalize_handle(&handle).ok())
            .collect::<BTreeSet<_>>();
        let audiences = audiences.into_iter().collect::<BTreeSet<_>>();
        ensure_mention_limit(
            handles.len() + audiences.len(),
            FORUM_MAX_MENTION_TARGETS_PER_REVISION,
        )?;
        Ok(Self {
            handles: handles.into_iter().collect(),
            audiences: audiences.into_iter().collect(),
        })
    }

    pub fn handles(&self) -> &[String] {
        &self.handles
    }

    pub fn audiences(&self) -> &[ForumMentionAudience] {
        &self.audiences
    }

    pub fn target_count(&self) -> usize {
        self.handles.len() + self.audiences.len()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ForumContentTargetKind {
    Topic,
    Reply,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ForumContentTarget {
    pub kind: ForumContentTargetKind,
    pub id: Uuid,
}

impl ForumContentTarget {
    pub fn topic(id: Uuid) -> Self {
        Self {
            kind: ForumContentTargetKind::Topic,
            id,
        }
    }

    pub fn reply(id: Uuid) -> Self {
        Self {
            kind: ForumContentTargetKind::Reply,
            id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ForumRevisionIdentity {
    pub tenant_id: Uuid,
    pub target: ForumContentTarget,
    pub revision_id: i64,
    pub locale: String,
}

impl ForumRevisionIdentity {
    pub fn new(
        tenant_id: Uuid,
        target: ForumContentTarget,
        revision_id: i64,
        locale: impl Into<String>,
    ) -> ForumResult<Self> {
        if tenant_id.is_nil() || target.id.is_nil() {
            return Err(ForumError::Validation(
                "Forum revision identity requires non-nil tenant and target IDs".to_string(),
            ));
        }
        if revision_id <= 0 {
            return Err(ForumError::Validation(
                "Forum revision identity requires a positive revision ID".to_string(),
            ));
        }
        let locale = locale.into();
        let locale = normalize_locale_tag(&locale).ok_or_else(|| {
            ForumError::Validation("Forum revision identity requires a valid locale".to_string())
        })?;
        Ok(Self {
            tenant_id,
            target,
            revision_id,
            locale,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ForumQuoteReference {
    pub target: ForumContentTarget,
    pub revision_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResolvedForumMention {
    pub user_id: Uuid,
    pub handle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumResolvedMentions {
    pub users: Vec<ResolvedForumMention>,
    pub audiences: Vec<ForumMentionAudience>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumMentionRevisionProjection {
    source: ForumRevisionIdentity,
    users: Vec<ResolvedForumMention>,
    audiences: Vec<ForumMentionAudience>,
}

impl ForumMentionRevisionProjection {
    pub fn new(
        source: ForumRevisionIdentity,
        users: impl IntoIterator<Item = ResolvedForumMention>,
        audiences: impl IntoIterator<Item = ForumMentionAudience>,
    ) -> ForumResult<Self> {
        let users = users.into_iter().collect::<BTreeSet<_>>();
        let audiences = audiences.into_iter().collect::<BTreeSet<_>>();
        ensure_mention_limit(
            users.len() + audiences.len(),
            FORUM_MAX_MENTION_TARGETS_PER_REVISION,
        )?;
        Ok(Self {
            source,
            users: users.into_iter().collect(),
            audiences: audiences.into_iter().collect(),
        })
    }

    pub fn source(&self) -> &ForumRevisionIdentity {
        &self.source
    }

    pub fn users(&self) -> &[ResolvedForumMention] {
        &self.users
    }

    pub fn audiences(&self) -> &[ForumMentionAudience] {
        &self.audiences
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumMentionDiff {
    pub added_users: Vec<ResolvedForumMention>,
    pub removed_users: Vec<ResolvedForumMention>,
    pub unchanged_users: Vec<ResolvedForumMention>,
    pub added_audiences: Vec<ForumMentionAudience>,
    pub removed_audiences: Vec<ForumMentionAudience>,
    pub unchanged_audiences: Vec<ForumMentionAudience>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ForumMentionEventTarget {
    User(Uuid),
    Audience(ForumMentionAudience),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ForumMentionEventCandidate {
    pub source: ForumRevisionIdentity,
    pub target: ForumMentionEventTarget,
}

pub fn extract_forum_mention_candidates(
    body: &str,
    body_format: &str,
    locale: &str,
    policy: ForumMentionPolicy,
) -> ForumResult<ForumMentionCandidates> {
    let policy = policy.validated()?;
    let format = normalize_content_format(Some(body_format)).map_err(ForumError::Validation)?;
    let mut handles = BTreeSet::new();
    let mut audiences = BTreeSet::new();

    match format.as_str() {
        CONTENT_FORMAT_MARKDOWN => {
            collect_markdown_mentions(body, policy, &mut handles, &mut audiences)?;
        }
        CONTENT_FORMAT_RT_JSON_V1 => {
            let payload: Value = serde_json::from_str(body).map_err(|_| {
                ForumError::Validation("Forum rt_json_v1 body must be valid JSON".to_string())
            })?;
            let sanitized = validate_and_sanitize_rt_json(
                &payload,
                &RtJsonValidationConfig::for_locale(locale),
            )
            .map_err(ForumError::Validation)?
            .sanitized;
            collect_rt_json_mentions(&sanitized, policy, &mut handles, &mut audiences)?;
        }
        _ => {
            return Err(ForumError::Validation(
                "Forum mentions support only markdown and rt_json_v1 content".to_string(),
            ));
        }
    }

    ensure_mention_limit(handles.len() + audiences.len(), policy.max_targets)?;
    ForumMentionCandidates::new(handles, audiences)
}

pub async fn resolve_forum_mentions(
    profiles: &dyn ProfilesReader,
    tenant_id: Uuid,
    candidates: ForumMentionCandidates,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> ForumResult<ForumResolvedMentions> {
    ensure_mention_limit(
        candidates.target_count(),
        FORUM_MAX_MENTION_TARGETS_PER_REVISION,
    )?;
    let mut users = BTreeMap::new();
    for handle in candidates.handles {
        let profile = profiles
            .get_profile_by_handle(
                tenant_id,
                &handle,
                requested_locale,
                tenant_default_locale,
            )
            .await
            .map_err(|error| map_profile_mention_error(&handle, error))?;
        validate_resolved_profile(tenant_id, &handle, &profile)?;
        users.insert(
            profile.user_id,
            ResolvedForumMention {
                user_id: profile.user_id,
                handle: profile.handle,
            },
        );
    }
    Ok(ForumResolvedMentions {
        users: users.into_values().collect(),
        audiences: candidates.audiences,
    })
}

pub fn validate_forum_quote_references(
    source: &ForumRevisionIdentity,
    references: impl IntoIterator<Item = ForumQuoteReference>,
) -> ForumResult<Vec<ForumQuoteReference>> {
    let references = references.into_iter().collect::<BTreeSet<_>>();
    if references.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
        return Err(ForumError::Validation(format!(
            "Forum revision exceeds the {FORUM_MAX_QUOTE_REFERENCES_PER_REVISION}-quote limit"
        )));
    }
    for reference in &references {
        if reference.target.id.is_nil() || reference.revision_id <= 0 {
            return Err(ForumError::Validation(
                "Forum quote references require a non-nil target and positive revision ID"
                    .to_string(),
            ));
        }
        if reference.target == source.target && reference.revision_id == source.revision_id {
            return Err(ForumError::Validation(
                "Forum revision cannot quote itself".to_string(),
            ));
        }
    }
    Ok(references.into_iter().collect())
}

pub fn diff_forum_mentions(
    previous: Option<&ForumMentionRevisionProjection>,
    current: &ForumMentionRevisionProjection,
) -> ForumResult<ForumMentionDiff> {
    if let Some(previous) = previous {
        ensure_same_revision_stream(previous.source(), current.source())?;
        if previous.source() == current.source() {
            if previous != current {
                return Err(ForumError::Validation(
                    "Forum mention replay changed an existing revision projection".to_string(),
                ));
            }
        } else if current.source().revision_id <= previous.source().revision_id {
            return Err(ForumError::Validation(
                "Forum mention diff cannot move revision identity backwards".to_string(),
            ));
        }
    }

    let previous_users = previous
        .map(|value| {
            value
                .users()
                .iter()
                .cloned()
                .map(|mention| (mention.user_id, mention))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let current_users = current
        .users()
        .iter()
        .cloned()
        .map(|mention| (mention.user_id, mention))
        .collect::<BTreeMap<_, _>>();
    let previous_audiences = previous
        .map(|value| value.audiences().iter().copied().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let current_audiences = current
        .audiences()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    Ok(ForumMentionDiff {
        added_users: current_users
            .iter()
            .filter(|(user_id, _)| !previous_users.contains_key(user_id))
            .map(|(_, mention)| mention.clone())
            .collect(),
        removed_users: previous_users
            .iter()
            .filter(|(user_id, _)| !current_users.contains_key(user_id))
            .map(|(_, mention)| mention.clone())
            .collect(),
        unchanged_users: current_users
            .iter()
            .filter(|(user_id, _)| previous_users.contains_key(user_id))
            .map(|(_, mention)| mention.clone())
            .collect(),
        added_audiences: current_audiences
            .difference(&previous_audiences)
            .copied()
            .collect(),
        removed_audiences: previous_audiences
            .difference(&current_audiences)
            .copied()
            .collect(),
        unchanged_audiences: current_audiences
            .intersection(&previous_audiences)
            .copied()
            .collect(),
    })
}

impl ForumMentionDiff {
    pub fn event_candidates(
        &self,
        source: &ForumRevisionIdentity,
    ) -> Vec<ForumMentionEventCandidate> {
        self.added_users
            .iter()
            .map(|mention| ForumMentionEventCandidate {
                source: source.clone(),
                target: ForumMentionEventTarget::User(mention.user_id),
            })
            .chain(self.added_audiences.iter().map(|audience| {
                ForumMentionEventCandidate {
                    source: source.clone(),
                    target: ForumMentionEventTarget::Audience(*audience),
                }
            }))
            .collect()
    }
}

fn collect_markdown_mentions(
    body: &str,
    policy: ForumMentionPolicy,
    handles: &mut BTreeSet<String>,
    audiences: &mut BTreeSet<ForumMentionAudience>,
) -> ForumResult<()> {
    let mut fence: Option<(u8, usize)> = None;
    for line in body.lines() {
        if let Some((character, length)) = fence {
            if let Some(value) = markdown_fence(line) {
                if value.0 == character && value.1 >= length {
                    fence = None;
                }
            }
            continue;
        }
        if let Some(opening) = markdown_fence(line) {
            fence = Some(opening);
            continue;
        }
        scan_text_mentions(line, true, policy, handles, audiences)?;
    }
    Ok(())
}

fn markdown_fence(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start();
    let character = *trimmed.as_bytes().first()?;
    if character != b'`' && character != b'~' {
        return None;
    }
    let length = trimmed
        .as_bytes()
        .iter()
        .take_while(|value| **value == character)
        .count();
    (length >= 3).then_some((character, length))
}

fn collect_rt_json_mentions(
    value: &Value,
    policy: ForumMentionPolicy,
    handles: &mut BTreeSet<String>,
    audiences: &mut BTreeSet<ForumMentionAudience>,
) -> ForumResult<()> {
    let Some(node) = value.as_object() else {
        return Ok(());
    };
    let node_type = node.get("type").and_then(Value::as_str);
    if node_type == Some("code_block") {
        return Ok(());
    }
    if node_type == Some("text") && !text_node_has_code_mark(node) {
        if let Some(text) = node.get("text").and_then(Value::as_str) {
            scan_text_mentions(text, false, policy, handles, audiences)?;
        }
    }
    if let Some(content) = node.get("content").and_then(Value::as_array) {
        for child in content {
            collect_rt_json_mentions(child, policy, handles, audiences)?;
        }
    }
    if let Some(doc) = node.get("doc") {
        collect_rt_json_mentions(doc, policy, handles, audiences)?;
    }
    Ok(())
}

fn text_node_has_code_mark(node: &serde_json::Map<String, Value>) -> bool {
    node.get("marks")
        .and_then(Value::as_array)
        .map(|marks| {
            marks
                .iter()
                .any(|mark| mark.get("type").and_then(Value::as_str) == Some("code"))
        })
        .unwrap_or(false)
}

fn scan_text_mentions(
    text: &str,
    skip_inline_code: bool,
    policy: ForumMentionPolicy,
    handles: &mut BTreeSet<String>,
    audiences: &mut BTreeSet<ForumMentionAudience>,
) -> ForumResult<()> {
    let mut index = 0;
    while index < text.len() {
        let byte = text.as_bytes()[index];
        if byte == b'\\' {
            index += 1;
            if index < text.len() {
                index += text[index..].chars().next().map(char::len_utf8).unwrap_or(1);
            }
            continue;
        }
        if skip_inline_code && byte == b'`' {
            let delimiter_length = text.as_bytes()[index..]
                .iter()
                .take_while(|value| **value == b'`')
                .count();
            let delimiter = "`".repeat(delimiter_length);
            let search_start = index + delimiter_length;
            index = text[search_start..]
                .find(&delimiter)
                .map(|offset| search_start + offset + delimiter_length)
                .unwrap_or(text.len());
            continue;
        }
        if byte == b'@' && mention_boundary(text, index) {
            let start = index + 1;
            let end = text.as_bytes()[start..]
                .iter()
                .take_while(|value| {
                    value.is_ascii_alphanumeric() || **value == b'_' || **value == b'-'
                })
                .count()
                + start;
            if end > start {
                classify_mention_token(&text[start..end], policy, handles, audiences)?;
                ensure_mention_limit(handles.len() + audiences.len(), policy.max_targets)?;
                index = end;
                continue;
            }
        }
        index += text[index..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    Ok(())
}

fn mention_boundary(text: &str, at_index: usize) -> bool {
    match text[..at_index].chars().next_back() {
        None => true,
        Some(character) => {
            !character.is_ascii_alphanumeric()
                && character != '_'
                && character != '-'
                && character != '.'
                && character != '@'
        }
    }
}

fn classify_mention_token(
    token: &str,
    policy: ForumMentionPolicy,
    handles: &mut BTreeSet<String>,
    audiences: &mut BTreeSet<ForumMentionAudience>,
) -> ForumResult<()> {
    let normalized = token.to_ascii_lowercase();
    if normalized == "moderators" {
        if !policy.allow_moderator_audience {
            return Err(ForumError::forbidden(
                "Mentioning the forum moderator audience requires moderation permission",
            ));
        }
        audiences.insert(ForumMentionAudience::Moderators);
        return Ok(());
    }
    if let Ok(handle) = ProfileService::normalize_handle(&normalized) {
        handles.insert(handle);
    }
    Ok(())
}

fn ensure_mention_limit(count: usize, max_targets: usize) -> ForumResult<()> {
    if count > max_targets {
        return Err(ForumError::Validation(format!(
            "Forum revision exceeds the {max_targets}-target mention limit"
        )));
    }
    Ok(())
}

fn validate_resolved_profile(
    tenant_id: Uuid,
    requested_handle: &str,
    profile: &ProfileRecord,
) -> ForumResult<()> {
    let visible = profile.tenant_id == tenant_id
        && profile.handle == requested_handle
        && profile.status == ProfileStatus::Active
        && matches!(
            profile.visibility,
            ProfileVisibility::Public | ProfileVisibility::Authenticated
        );
    if !visible {
        return Err(ForumError::mention_target_unavailable(requested_handle));
    }
    Ok(())
}

fn map_profile_mention_error(handle: &str, error: ProfileError) -> ForumError {
    match error {
        ProfileError::ProfileByHandleNotFound(_) | ProfileError::ProfileNotFound(_) => {
            ForumError::mention_target_unavailable(handle)
        }
        ProfileError::Database(_) => ForumError::capability_failure(
            FORUM_MENTION_PROFILES_CAPABILITY,
            "profiles.database",
            "Profile mention lookup failed",
            true,
        ),
        _ => ForumError::capability_failure(
            FORUM_MENTION_PROFILES_CAPABILITY,
            "profiles.lookup_failed",
            "Profile mention lookup failed",
            false,
        ),
    }
}

fn ensure_same_revision_stream(
    previous: &ForumRevisionIdentity,
    current: &ForumRevisionIdentity,
) -> ForumResult<()> {
    if previous.tenant_id != current.tenant_id
        || previous.target != current.target
        || previous.locale != current.locale
    {
        return Err(ForumError::Validation(
            "Forum mention diff requires the same tenant, target and locale".to_string(),
        ));
    }
    Ok(())
}
