use rustok_core::{Error, Result};
use rustok_profiles::{ProfilePresentationService, ProfileSummary};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) async fn load_public_author_summary(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    author_id: Option<Uuid>,
    locale: &str,
) -> Result<Option<ProfileSummary>> {
    let Some(author_id) = author_id else {
        return Ok(None);
    };

    ProfilePresentationService::new(db.clone())
        .find_profile_summary(tenant_id, author_id, Some(locale), None)
        .await
        .map_err(|error| {
            Error::External(format!(
                "Forum Search public author summary failed: {error}"
            ))
        })
}

pub(super) fn public_author_id(summary: Option<&ProfileSummary>) -> Option<Uuid> {
    summary.map(|summary| summary.user_id)
}

pub(super) fn public_author_handle(summary: Option<&ProfileSummary>) -> Option<String> {
    summary.map(|summary| summary.handle.clone())
}

pub(super) fn public_author_keywords(summary: Option<&ProfileSummary>) -> String {
    summary
        .map(|summary| format!("{} {}", summary.handle, summary.display_name))
        .unwrap_or_default()
}

pub(super) fn public_author_payload(summary: Option<&ProfileSummary>) -> Value {
    summary.map_or(Value::Null, |summary| {
        json!({
            "user_id": summary.user_id,
            "handle": summary.handle,
            "display_name": summary.display_name,
            "avatar_media_id": summary.avatar_media_id
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_profiles::ProfileVisibility;

    #[test]
    fn public_author_payload_exposes_only_the_safe_summary() {
        let user_id = Uuid::new_v4();
        let avatar_media_id = Uuid::new_v4();
        let summary = ProfileSummary {
            user_id,
            handle: "public-handle".to_string(),
            display_name: "Public Name".to_string(),
            tags: vec!["private-owner-tag".to_string()],
            avatar_media_id: Some(avatar_media_id),
            preferred_locale: Some("de".to_string()),
            visibility: ProfileVisibility::Public,
        };

        assert_eq!(
            public_author_payload(Some(&summary)),
            json!({
                "user_id": user_id,
                "handle": "public-handle",
                "display_name": "Public Name",
                "avatar_media_id": avatar_media_id
            })
        );
    }

    #[test]
    fn absent_or_denied_author_is_not_serialized() {
        assert_eq!(public_author_payload(None), Value::Null);
        assert_eq!(public_author_id(None), None);
        assert_eq!(public_author_handle(None), None);
        assert_eq!(public_author_keywords(None), "");
    }
}
