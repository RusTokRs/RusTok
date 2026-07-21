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
    pub handles: Vec<String>,
    pub audiences: Vec<ForumMentionAudience>,
}

impl ForumMentionCandidates {
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
    pub source: ForumRevisionIdentity,
    pub users: Vec<ResolvedForumMention>,
    pub audiences: Vec<ForumMentionAudience>,
}

impl ForumMentionRevisionProjection {
    pub fn new(
        source: ForumRevisionIdentity,
        users: impl IntoIterator<Item = ResolvedForumMention>,
        audiences: impl IntoIterator<Item = ForumMentionAudience>,
    ) -> ForumResult<Self> {
        let users = users.into_iter().collect::<BTreeSet<_>>();
        let audiences = audiences.into_iter().collect::<BTreeSet<_>>();
        if users.len() + audiences.len() > FORUM_MAX_MENTION_TARGETS_PER_REVISION {
            return Err(ForumError::Validation(format!(
                "Forum revision exceeds the {FORUM_MAX_MENTION_TARGETS_PER_REVISION}-target mention limit"
            )));
        }

        Ok(Self {
            source,
            users: users.into_iter().collect(),
            audiences: audiences.into_iter().collect(),
        })
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
    Ok(ForumMentionCandidates {
        handles: handles.into_iter().collect(),
        audiences: audiences.into_iter().collect(),
    })
}

pub async fn resolve_forum_mentions(
    profiles: &dyn ProfilesReader,
    tenant_id: Uuid,
    candidates: ForumMentionCandidates,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> ForumResult<ForumResolvedMentions> {
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
        ensure_revision_successor(&previous.source, &current.source)?;
    }

    let previous_users = previous
        .map(|value| {
            value
                .users
                .iter()
                .cloned()
                .map(|mention| (mention.user_id, mention))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let current_users = current
        .users
        .iter()
        .cloned()
        .map(|mention| (mention.user_id, mention))
        .collect::<BTreeMap<_, _>>();
    let previous_audiences = previous
        .map(|value| value.audiences.iter().copied().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let current_audiences = current.audiences.iter().copied().collect::<BTreeSet<_>>();

    let added_users = current_users
        .iter()
        .filter(|(user_id, _)| !previous_users.contains_key(user_id))
        .map(|(_, mention)| mention.clone())
        .collect();
    let removed_users = previous_users
        .iter()
        .filter(|(user_id, _)| !current_users.contains_key(user_id))
        .map(|(_, mention)| mention.clone())
        .collect();
    let unchanged_users = current_users
        .iter()
        .filter(|(user_id, _)| previous_users.contains_key(user_id))
        .map(|(_, mention)| mention.clone())
        .collect();

    Ok(ForumMentionDiff {
        added_users,
        removed_users,
        unchanged_users,
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
            if markdown_fence(line).is_some_and(|value| value.0 == character && value.1 >= length) {
                fence = None;
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
        .is_some_and(|marks| {
            marks.iter().any(|mark| {
                mark.get("type").and_then(Value::as_str) == Some("code")
            })
        })
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
    text[..at_index].chars().next_back().is_none_or(|character| {
        !character.is_ascii_alphanumeric()
            && character != '_'
            && character != '-'
            && character != '.'
            && character != '@'
    })
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

fn ensure_revision_successor(
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
    if current.revision_id < previous.revision_id {
        return Err(ForumError::Validation(
            "Forum mention diff cannot move revision identity backwards".to_string(),
        ));
    }
    if current.revision_id == previous.revision_id && previous != current {
        return Err(ForumError::Validation(
            "Forum mention replay changed an existing revision identity".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use rustok_profiles::{ProfileResult, ProfileSummary};
    use serde_json::json;

    use super::*;

    struct FakeProfilesReader {
        records: HashMap<String, ProfileRecord>,
    }

    #[async_trait]
    impl ProfilesReader for FakeProfilesReader {
        async fn find_profile_summary(
            &self,
            _tenant_id: Uuid,
            _user_id: Uuid,
            _requested_locale: Option<&str>,
            _tenant_default_locale: Option<&str>,
        ) -> ProfileResult<Option<ProfileSummary>> {
            unreachable!("not used by mention resolution")
        }

        async fn find_profile_summaries(
            &self,
            _tenant_id: Uuid,
            _user_ids: &[Uuid],
            _requested_locale: Option<&str>,
            _tenant_default_locale: Option<&str>,
        ) -> ProfileResult<HashMap<Uuid, ProfileSummary>> {
            unreachable!("not used by mention resolution")
        }

        async fn get_profile_by_handle(
            &self,
            _tenant_id: Uuid,
            handle: &str,
            _requested_locale: Option<&str>,
            _tenant_default_locale: Option<&str>,
        ) -> ProfileResult<ProfileRecord> {
            self.records
                .get(handle)
                .cloned()
                .ok_or_else(|| ProfileError::ProfileByHandleNotFound(handle.to_string()))
        }
    }

    fn profile(
        tenant_id: Uuid,
        handle: &str,
        visibility: ProfileVisibility,
        status: ProfileStatus,
    ) -> ProfileRecord {
        ProfileRecord {
            tenant_id,
            user_id: Uuid::new_v4(),
            handle: handle.to_string(),
            display_name: handle.to_string(),
            bio: None,
            tags: Vec::new(),
            avatar_media_id: None,
            banner_media_id: None,
            preferred_locale: Some("en".to_string()),
            visibility,
            status,
        }
    }

    fn revision(
        tenant_id: Uuid,
        target: ForumContentTarget,
        revision_id: i64,
    ) -> ForumRevisionIdentity {
        ForumRevisionIdentity::new(tenant_id, target, revision_id, "en")
            .expect("valid revision identity")
    }

    #[test]
    fn markdown_extraction_ignores_code_escaping_and_email_addresses() {
        let body = r#"Hello @Alice and @alice.
\@escaped `@inline` user@example.com
```rust
@fenced
```
@moderators"#;
        let result = extract_forum_mention_candidates(
            body,
            "markdown",
            "en",
            ForumMentionPolicy {
                allow_moderator_audience: true,
                ..ForumMentionPolicy::default()
            },
        )
        .expect("mentions should parse");

        assert_eq!(result.handles, vec!["alice"]);
        assert_eq!(result.audiences, vec![ForumMentionAudience::Moderators]);
    }

    #[test]
    fn rt_json_extraction_ignores_code_blocks_and_code_marks() {
        let body = json!({
            "version": "rt_json_v1",
            "locale": "en",
            "doc": {
                "type": "doc",
                "content": [
                    {"type": "paragraph", "content": [{"type": "text", "text": "Hi @alice"}]},
                    {"type": "code_block", "content": [{"type": "text", "text": "@blocked"}]},
                    {"type": "paragraph", "content": [
                        {"type": "text", "text": "@inline", "marks": [{"type": "code"}]},
                        {"type": "text", "text": " \\@escaped"}
                    ]}
                ]
            }
        })
        .to_string();

        let result = extract_forum_mention_candidates(
            &body,
            "rt_json_v1",
            "en",
            ForumMentionPolicy::default(),
        )
        .expect("mentions should parse");
        assert_eq!(result.handles, vec!["alice"]);
    }

    #[test]
    fn extraction_enforces_caps_and_special_audience_permission() {
        let denied = extract_forum_mention_candidates(
            "@moderators",
            "markdown",
            "en",
            ForumMentionPolicy::default(),
        )
        .expect_err("special audience must be permission gated");
        assert_eq!(denied.stable_code(), "FORUM_FORBIDDEN");

        let capped = extract_forum_mention_candidates(
            "@alice @bob",
            "markdown",
            "en",
            ForumMentionPolicy {
                max_targets: 1,
                allow_moderator_audience: false,
            },
        )
        .expect_err("mention cap must be enforced");
        assert_eq!(capped.stable_code(), "FORUM_VALIDATION_FAILED");
    }

    #[tokio::test]
    async fn profile_resolution_is_tenant_scoped_and_fail_closed() {
        let tenant_id = Uuid::new_v4();
        let public = profile(
            tenant_id,
            "alice",
            ProfileVisibility::Public,
            ProfileStatus::Active,
        );
        let private = profile(
            tenant_id,
            "private-user",
            ProfileVisibility::Private,
            ProfileStatus::Active,
        );
        let reader = FakeProfilesReader {
            records: [
                (public.handle.clone(), public.clone()),
                (private.handle.clone(), private),
            ]
            .into_iter()
            .collect(),
        };

        let resolved = resolve_forum_mentions(
            &reader,
            tenant_id,
            ForumMentionCandidates {
                handles: vec!["alice".to_string()],
                audiences: Vec::new(),
            },
            Some("en"),
            Some("en"),
        )
        .await
        .expect("public active profile should resolve");
        assert_eq!(resolved.users[0].user_id, public.user_id);

        let error = resolve_forum_mentions(
            &reader,
            tenant_id,
            ForumMentionCandidates {
                handles: vec!["private-user".to_string()],
                audiences: Vec::new(),
            },
            Some("en"),
            Some("en"),
        )
        .await
        .expect_err("private profile must fail closed");
        assert_eq!(error.stable_code(), "FORUM_MENTION_TARGET_UNAVAILABLE");
        assert!(!error.to_string().contains("private-user"));
    }

    #[test]
    fn revision_diff_emits_only_new_targets() {
        let tenant_id = Uuid::new_v4();
        let target = ForumContentTarget::topic(Uuid::new_v4());
        let alice = ResolvedForumMention {
            user_id: Uuid::new_v4(),
            handle: "alice".to_string(),
        };
        let bob = ResolvedForumMention {
            user_id: Uuid::new_v4(),
            handle: "bob".to_string(),
        };
        let previous = ForumMentionRevisionProjection::new(
            revision(tenant_id, target, 10),
            [alice.clone()],
            [ForumMentionAudience::Moderators],
        )
        .expect("previous projection");
        let current = ForumMentionRevisionProjection::new(
            revision(tenant_id, target, 11),
            [alice.clone(), bob.clone()],
            [],
        )
        .expect("current projection");

        let diff = diff_forum_mentions(Some(&previous), &current).expect("valid diff");
        assert_eq!(diff.added_users, vec![bob.clone()]);
        assert_eq!(diff.unchanged_users, vec![alice]);
        assert_eq!(diff.removed_audiences, vec![ForumMentionAudience::Moderators]);
        let candidates = diff.event_candidates(&current.source);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].target, ForumMentionEventTarget::User(bob.user_id));
    }

    #[test]
    fn quote_references_are_revision_bound_deduplicated_and_non_recursive() {
        let tenant_id = Uuid::new_v4();
        let source = revision(
            tenant_id,
            ForumContentTarget::reply(Uuid::new_v4()),
            20,
        );
        let quoted = ForumQuoteReference {
            target: ForumContentTarget::reply(Uuid::new_v4()),
            revision_id: 4,
        };
        let result = validate_forum_quote_references(
            &source,
            [quoted.clone(), quoted.clone()],
        )
        .expect("duplicate quote input should normalize");
        assert_eq!(result, vec![quoted]);

        let self_reference = ForumQuoteReference {
            target: source.target,
            revision_id: source.revision_id,
        };
        assert!(validate_forum_quote_references(&source, [self_reference]).is_err());
    }
}
