use uuid::Uuid;

use crate::model::{
    ProfilesStorefrontFollowState, ProfilesStorefrontPage, SetProfilesStorefrontFollowCommand,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfilesStorefrontTransportProfile {
    Native,
    Graphql,
}

impl ProfilesStorefrontTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfilesStorefrontInputError {
    MissingHandle,
    InvalidUserId,
    InvalidRevision,
}

pub fn selected_transport_profile(value: Option<&str>) -> ProfilesStorefrontTransportProfile {
    let normalized = value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    match normalized.as_str() {
        "" | "native" => ProfilesStorefrontTransportProfile::Native,
        "graphql" => ProfilesStorefrontTransportProfile::Graphql,
        invalid => panic!("unsupported profiles storefront transport profile: {invalid}"),
    }
}

pub fn normalize_profile_handle(value: &str) -> Result<String, ProfilesStorefrontInputError> {
    let handle = value.trim().trim_start_matches('@').trim();
    if handle.is_empty() {
        return Err(ProfilesStorefrontInputError::MissingHandle);
    }
    Ok(handle.to_string())
}

pub fn prepare_follow_command(
    user_id: &str,
    following: bool,
    expected_revision: Option<&str>,
) -> Result<SetProfilesStorefrontFollowCommand, ProfilesStorefrontInputError> {
    let user_id = Uuid::parse_str(user_id.trim())
        .map_err(|_| ProfilesStorefrontInputError::InvalidUserId)?;
    let expected_revision = expected_revision
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<i64>()
                .ok()
                .filter(|revision| *revision > 0)
                .map(|revision| revision.to_string())
                .ok_or(ProfilesStorefrontInputError::InvalidRevision)
        })
        .transpose()?;
    let action = if following { "follow" } else { "unfollow" };

    Ok(SetProfilesStorefrontFollowCommand {
        user_id: user_id.to_string(),
        following,
        expected_revision,
        idempotency_key: format!("profiles-storefront-{action}-{}", Uuid::new_v4()),
    })
}

pub fn recovered_follow_state(
    page: ProfilesStorefrontPage,
    target_user_id: &str,
) -> Option<ProfilesStorefrontFollowState> {
    let profile = page.profile?;
    if profile.user_id != target_user_id {
        return None;
    }
    page.follow_state
        .filter(|state| state.user_id == target_user_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProfilesStorefrontProfile;

    #[test]
    fn handle_normalization_accepts_at_prefix_and_rejects_empty_input() {
        assert_eq!(normalize_profile_handle("  @alice  ").unwrap(), "alice");
        assert_eq!(
            normalize_profile_handle(" @ "),
            Err(ProfilesStorefrontInputError::MissingHandle)
        );
    }

    #[test]
    fn transport_selection_accepts_supported_profiles_and_defaults_to_native() {
        assert_eq!(
            selected_transport_profile(Some("graphql")),
            ProfilesStorefrontTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            ProfilesStorefrontTransportProfile::Native
        );
        assert_eq!(
            selected_transport_profile(None),
            ProfilesStorefrontTransportProfile::Native
        );
    }

    #[test]
    #[should_panic(expected = "unsupported profiles storefront transport profile")]
    fn invalid_transport_profile_fails_closed() {
        let _ = selected_transport_profile(Some("rest"));
    }

    #[test]
    fn follow_command_binds_uuid_revision_and_unique_idempotency() {
        let command = prepare_follow_command(
            "550e8400-e29b-41d4-a716-446655440000",
            true,
            Some("42"),
        )
        .unwrap();
        assert!(command.following);
        assert_eq!(command.expected_revision.as_deref(), Some("42"));
        assert!(
            command
                .idempotency_key
                .starts_with("profiles-storefront-follow-")
        );
        assert_eq!(
            prepare_follow_command("not-a-uuid", true, None),
            Err(ProfilesStorefrontInputError::InvalidUserId)
        );
        assert_eq!(
            prepare_follow_command(
                "550e8400-e29b-41d4-a716-446655440000",
                false,
                Some("0")
            ),
            Err(ProfilesStorefrontInputError::InvalidRevision)
        );
    }

    #[test]
    fn recovery_accepts_only_the_requested_profile_state() {
        let target = "550e8400-e29b-41d4-a716-446655440000";
        let page = ProfilesStorefrontPage {
            profile: Some(ProfilesStorefrontProfile {
                user_id: target.into(),
                handle: "alice".into(),
                display_name: "Alice".into(),
                bio: None,
                tags: Vec::new(),
                avatar_media_id: None,
                banner_media_id: None,
                avatar_image: None,
                banner_image: None,
                preferred_locale: None,
                visibility: "public".into(),
            }),
            viewer_authenticated: true,
            is_self: false,
            follow_state: Some(ProfilesStorefrontFollowState {
                user_id: target.into(),
                following: true,
                revision: Some("7".into()),
            }),
        };
        let recovered = recovered_follow_state(page.clone(), target).unwrap();
        assert!(recovered.following);
        assert_eq!(recovered.revision.as_deref(), Some("7"));
        assert!(recovered_follow_state(page, "00000000-0000-0000-0000-000000000000").is_none());
    }
}
