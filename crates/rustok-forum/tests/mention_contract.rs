use std::collections::HashMap;

use async_trait::async_trait;
use rustok_forum::{
    ForumContentTarget, ForumMentionAudience, ForumMentionCandidates, ForumMentionEventTarget,
    ForumMentionPolicy, ForumMentionRevisionProjection, ForumQuoteReference, ForumRevisionIdentity,
    diff_forum_mentions, extract_forum_mention_candidates, resolve_forum_mentions,
    validate_forum_quote_references,
};
use rustok_profiles::{
    ProfileError, ProfileRecord, ProfileResult, ProfileStatus, ProfileSummary, ProfileVisibility,
    ProfilesReader,
};
use serde_json::json;
use uuid::Uuid;

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
        tenant_id: Uuid,
        handle: &str,
        _requested_locale: Option<&str>,
        _tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        self.records
            .get(handle)
            .filter(|record| record.tenant_id == tenant_id)
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

    assert_eq!(result.handles(), &["alice".to_string()]);
    assert_eq!(result.audiences(), &[ForumMentionAudience::Moderators]);
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

    let result =
        extract_forum_mention_candidates(&body, "rt_json_v1", "en", ForumMentionPolicy::default())
            .expect("mentions should parse");
    assert_eq!(result.handles(), &["alice".to_string()]);
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

    let manual_audience = ForumMentionCandidates::new(
        std::iter::empty(),
        [ForumMentionAudience::Moderators],
        ForumMentionPolicy::default(),
    )
    .expect_err("manual candidate construction must enforce audience permission");
    assert_eq!(manual_audience.stable_code(), "FORUM_FORBIDDEN");
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
        ForumMentionCandidates::new(
            ["alice".to_string()],
            std::iter::empty(),
            ForumMentionPolicy::default(),
        )
        .expect("candidates"),
        Some("en"),
        Some("en"),
    )
    .await
    .expect("public active profile should resolve");
    assert_eq!(resolved.users()[0].user_id(), public.user_id);

    let error = resolve_forum_mentions(
        &reader,
        tenant_id,
        ForumMentionCandidates::new(
            ["private-user".to_string()],
            std::iter::empty(),
            ForumMentionPolicy::default(),
        )
        .expect("candidates"),
        Some("en"),
        Some("en"),
    )
    .await
    .expect_err("private profile must fail closed");
    assert_eq!(error.stable_code(), "FORUM_MENTION_TARGET_UNAVAILABLE");
    assert!(!error.to_string().contains("private-user"));
}

#[tokio::test]
async fn revision_diff_emits_only_new_targets_and_replay_is_immutable() {
    let tenant_id = Uuid::new_v4();
    let target = ForumContentTarget::topic(Uuid::new_v4());
    let alice = profile(
        tenant_id,
        "alice",
        ProfileVisibility::Public,
        ProfileStatus::Active,
    );
    let bob = profile(
        tenant_id,
        "bob",
        ProfileVisibility::Authenticated,
        ProfileStatus::Active,
    );
    let reader = FakeProfilesReader {
        records: [
            (alice.handle.clone(), alice.clone()),
            (bob.handle.clone(), bob.clone()),
        ]
        .into_iter()
        .collect(),
    };

    let previous_resolved = resolve_forum_mentions(
        &reader,
        tenant_id,
        ForumMentionCandidates::new(
            ["alice".to_string()],
            [ForumMentionAudience::Moderators],
            ForumMentionPolicy {
                allow_moderator_audience: true,
                ..ForumMentionPolicy::default()
            },
        )
        .expect("previous candidates"),
        Some("en"),
        Some("en"),
    )
    .await
    .expect("previous resolution");
    let current_resolved = resolve_forum_mentions(
        &reader,
        tenant_id,
        ForumMentionCandidates::new(
            ["alice".to_string(), "bob".to_string()],
            std::iter::empty(),
            ForumMentionPolicy::default(),
        )
        .expect("current candidates"),
        Some("en"),
        Some("en"),
    )
    .await
    .expect("current resolution");

    let previous =
        ForumMentionRevisionProjection::new(revision(tenant_id, target, 10), previous_resolved)
            .expect("previous projection");
    let current =
        ForumMentionRevisionProjection::new(revision(tenant_id, target, 11), current_resolved)
            .expect("current projection");

    let diff = diff_forum_mentions(Some(&previous), &current).expect("valid diff");
    assert_eq!(diff.added_users().len(), 1);
    assert_eq!(diff.added_users()[0].user_id(), bob.user_id);
    assert_eq!(diff.unchanged_users().len(), 1);
    assert_eq!(diff.unchanged_users()[0].user_id(), alice.user_id);
    assert_eq!(
        diff.removed_audiences(),
        &[ForumMentionAudience::Moderators]
    );
    let candidates = diff.event_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source(), current.source());
    assert_eq!(
        candidates[0].target(),
        &ForumMentionEventTarget::User(bob.user_id)
    );

    let identical_replay = diff_forum_mentions(Some(&current), &current)
        .expect("identical revision replay must be idempotent");
    assert!(identical_replay.event_candidates().is_empty());

    let changed_replay_resolved = resolve_forum_mentions(
        &reader,
        tenant_id,
        ForumMentionCandidates::new(
            ["bob".to_string()],
            std::iter::empty(),
            ForumMentionPolicy::default(),
        )
        .expect("changed replay candidates"),
        Some("en"),
        Some("en"),
    )
    .await
    .expect("changed replay resolution");
    let replay_changed =
        ForumMentionRevisionProjection::new(current.source().clone(), changed_replay_resolved)
            .expect("changed replay projection");
    assert!(diff_forum_mentions(Some(&current), &replay_changed).is_err());
}

#[test]
fn quote_references_are_revision_bound_deduplicated_and_non_recursive() {
    let tenant_id = Uuid::new_v4();
    let source = revision(tenant_id, ForumContentTarget::reply(Uuid::new_v4()), 20);
    let quoted = ForumQuoteReference::new(ForumContentTarget::reply(Uuid::new_v4()), 4)
        .expect("valid quote reference");
    let result = validate_forum_quote_references(&source, [quoted.clone(), quoted.clone()])
        .expect("duplicate quote input should normalize");
    assert_eq!(result, vec![quoted]);

    let self_reference = ForumQuoteReference::new(source.target(), source.revision_id())
        .expect("valid self target shape");
    assert!(validate_forum_quote_references(&source, [self_reference]).is_err());
}
