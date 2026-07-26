#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::model::{
    ProfilesStorefrontFollowState, ProfilesStorefrontImage, ProfilesStorefrontPage,
    ProfilesStorefrontProfile, SetProfilesStorefrontFollowCommand,
};

pub type GraphqlProfilesStorefrontError = String;

const PROFILE_QUERY: &str = "query ProfilesStorefrontProfile($handle: String!, $locale: String) { profile: profileByHandle(handle: $handle, locale: $locale) { user_id: userId handle display_name: displayName bio tags avatar_media_id: avatarMediaId banner_media_id: bannerMediaId avatar_image: avatarImage { url alt width height mime_type: mimeType } banner_image: bannerImage { url alt width height mime_type: mimeType } preferred_locale: preferredLocale visibility } }";
const FOLLOW_STATE_QUERY: &str = "query ProfilesStorefrontFollowState($userId: UUID!) { state: followState(userId: $userId) { user_id: userId following revision } }";
const FOLLOW_MUTATION: &str = "mutation ProfilesStorefrontFollow($idempotencyKey: String!, $userId: UUID!, $expectedRevision: String) { state: followUser(idempotencyKey: $idempotencyKey, userId: $userId, expectedRevision: $expectedRevision) { user_id: userId following revision } }";
const UNFOLLOW_MUTATION: &str = "mutation ProfilesStorefrontUnfollow($idempotencyKey: String!, $userId: UUID!, $expectedRevision: String) { state: unfollowUser(idempotencyKey: $idempotencyKey, userId: $userId, expectedRevision: $expectedRevision) { user_id: userId following revision } }";

#[derive(Debug, Serialize)]
struct ProfileVariables {
    handle: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct FollowQueryVariables {
    #[serde(rename = "userId")]
    user_id: String,
}

#[derive(Debug, Serialize)]
struct FollowMutationVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "expectedRevision")]
    expected_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProfileResponse {
    profile: Option<ProfileWire>,
}

#[derive(Debug, Deserialize)]
struct FollowQueryResponse {
    state: FollowReadStateWire,
}

#[derive(Debug, Deserialize)]
struct FollowMutationResponse {
    state: FollowMutationStateWire,
}

#[derive(Debug, Deserialize)]
struct ProfileImageWire {
    url: String,
    alt: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProfileWire {
    user_id: String,
    handle: String,
    display_name: String,
    bio: Option<String>,
    tags: Vec<String>,
    avatar_media_id: Option<String>,
    banner_media_id: Option<String>,
    avatar_image: Option<ProfileImageWire>,
    banner_image: Option<ProfileImageWire>,
    preferred_locale: Option<String>,
    visibility: String,
}

#[derive(Debug, Deserialize)]
struct FollowReadStateWire {
    user_id: String,
    following: bool,
    revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FollowMutationStateWire {
    user_id: String,
    following: bool,
    revision: String,
}

pub async fn load_profile(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    current_user_id: Option<String>,
    handle: String,
    locale: Option<String>,
) -> Result<ProfilesStorefrontPage, GraphqlProfilesStorefrontError> {
    let response: ProfileResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(PROFILE_QUERY, Some(ProfileVariables { handle, locale })),
        access_token.clone(),
        tenant_slug.clone(),
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    let viewer_authenticated = access_token.is_some() && current_user_id.is_some();
    let Some(profile) = response.profile.map(ProfilesStorefrontProfile::from) else {
        return Ok(ProfilesStorefrontPage {
            profile: None,
            viewer_authenticated,
            is_self: false,
            follow_state: None,
        });
    };
    let is_self = current_user_id.as_deref() == Some(profile.user_id.as_str());
    let follow_state = if viewer_authenticated && !is_self {
        execute_graphql::<_, FollowQueryResponse>(
            &graphql_url(),
            GraphqlRequest::new(
                FOLLOW_STATE_QUERY,
                Some(FollowQueryVariables {
                    user_id: profile.user_id.clone(),
                }),
            ),
            access_token,
            tenant_slug,
            None,
        )
        .await
        .ok()
        .map(|response| response.state.into())
    } else {
        None
    };

    Ok(ProfilesStorefrontPage {
        profile: Some(profile),
        viewer_authenticated,
        is_self,
        follow_state,
    })
}

pub async fn set_follow(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    command: SetProfilesStorefrontFollowCommand,
) -> Result<ProfilesStorefrontFollowState, GraphqlProfilesStorefrontError> {
    let query = if command.following {
        FOLLOW_MUTATION
    } else {
        UNFOLLOW_MUTATION
    };
    let response: FollowMutationResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            query,
            Some(FollowMutationVariables {
                idempotency_key: command.idempotency_key,
                user_id: command.user_id,
                expected_revision: command.expected_revision,
            }),
        ),
        access_token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(response.state.into())
}

impl From<ProfileImageWire> for ProfilesStorefrontImage {
    fn from(value: ProfileImageWire) -> Self {
        Self {
            url: value.url,
            alt: value.alt,
            width: value.width,
            height: value.height,
            mime_type: value.mime_type,
        }
    }
}

impl From<ProfileWire> for ProfilesStorefrontProfile {
    fn from(value: ProfileWire) -> Self {
        Self {
            user_id: value.user_id,
            handle: value.handle,
            display_name: value.display_name,
            bio: value.bio,
            tags: value.tags,
            avatar_media_id: value.avatar_media_id,
            banner_media_id: value.banner_media_id,
            avatar_image: value.avatar_image.map(Into::into),
            banner_image: value.banner_image.map(Into::into),
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.to_ascii_lowercase(),
        }
    }
}

impl From<FollowReadStateWire> for ProfilesStorefrontFollowState {
    fn from(value: FollowReadStateWire) -> Self {
        Self {
            user_id: value.user_id,
            following: value.following,
            revision: value.revision,
        }
    }
}

impl From<FollowMutationStateWire> for ProfilesStorefrontFollowState {
    fn from(value: FollowMutationStateWire) -> Self {
        Self {
            user_id: value.user_id,
            following: value.following,
            revision: Some(value.revision),
        }
    }
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_contract_uses_owner_fields_and_revision_reads() {
        assert!(PROFILE_QUERY.contains("profileByHandle"));
        assert!(PROFILE_QUERY.contains("avatarImage"));
        assert!(PROFILE_QUERY.contains("bannerImage"));
        assert!(FOLLOW_STATE_QUERY.contains("followState"));
        assert!(FOLLOW_STATE_QUERY.contains("revision"));
        assert!(FOLLOW_MUTATION.contains("expectedRevision: String"));
        assert!(UNFOLLOW_MUTATION.contains("unfollowUser"));
    }
}
