use uuid::Uuid;

use crate::model::{
    AcceptGroupInvitationCommand, AcceptTargetedGroupInvitationCommand, GroupsStorefrontFilters,
};

pub const DEFAULT_GROUPS_PAGE: u64 = 1;
pub const DEFAULT_GROUPS_PER_PAGE: u64 = 24;
pub const GROUP_INVITATION_TOKEN_QUERY_KEY: &str = "invite";
pub const GROUP_TARGETED_INVITATION_QUERY_KEY: &str = "invitation";
pub const MIN_GROUP_INVITATION_TOKEN_LENGTH: usize = 32;
pub const MAX_GROUP_INVITATION_TOKEN_LENGTH: usize = 160;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsStorefrontTransportProfile {
    Native,
    Graphql,
}

impl GroupsStorefrontTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsStorefrontInvitationInputError {
    MissingToken,
    InvalidTokenLength,
    InvalidInvitationId,
}

pub fn selected_transport_profile(value: Option<&str>) -> GroupsStorefrontTransportProfile {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => GroupsStorefrontTransportProfile::Graphql,
        _ => GroupsStorefrontTransportProfile::Native,
    }
}

pub fn default_groups_storefront_filters() -> GroupsStorefrontFilters {
    GroupsStorefrontFilters {
        page: DEFAULT_GROUPS_PAGE,
        per_page: DEFAULT_GROUPS_PER_PAGE,
        search: None,
    }
}

pub fn prepare_accept_group_invitation(
    token: &str,
) -> Result<AcceptGroupInvitationCommand, GroupsStorefrontInvitationInputError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(GroupsStorefrontInvitationInputError::MissingToken);
    }
    if !(MIN_GROUP_INVITATION_TOKEN_LENGTH..=MAX_GROUP_INVITATION_TOKEN_LENGTH)
        .contains(&token.len())
    {
        return Err(GroupsStorefrontInvitationInputError::InvalidTokenLength);
    }
    Ok(AcceptGroupInvitationCommand {
        idempotency_key: format!("groups-storefront-accept-invitation-{}", Uuid::new_v4()),
        token: token.to_string(),
    })
}

pub fn prepare_accept_targeted_group_invitation(
    invitation_id: &str,
) -> Result<AcceptTargetedGroupInvitationCommand, GroupsStorefrontInvitationInputError> {
    let invitation_id = Uuid::parse_str(invitation_id.trim())
        .map_err(|_| GroupsStorefrontInvitationInputError::InvalidInvitationId)?;
    Ok(AcceptTargetedGroupInvitationCommand {
        idempotency_key: format!(
            "groups-storefront-accept-targeted-invitation-{}",
            Uuid::new_v4()
        ),
        invitation_id: invitation_id.to_string(),
    })
}

pub fn groups_storefront_error(prefix: &str, details: &str) -> String {
    if details.trim().is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}: {details}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_directory_request_is_bounded() {
        let request = default_groups_storefront_filters();
        assert_eq!(request.page, 1);
        assert_eq!(request.per_page, 24);
    }

    #[test]
    fn transport_selection_is_explicit() {
        assert_eq!(
            selected_transport_profile(Some("graphql")),
            GroupsStorefrontTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(None),
            GroupsStorefrontTransportProfile::Native
        );
    }

    #[test]
    fn invitation_acceptance_preparation_trims_and_bounds_token() {
        let token = "a".repeat(MIN_GROUP_INVITATION_TOKEN_LENGTH);
        let command = prepare_accept_group_invitation(&format!("  {token}  "))
            .expect("bounded token must be accepted");
        assert_eq!(command.token, token);
        assert!(
            command
                .idempotency_key
                .starts_with("groups-storefront-accept-invitation-")
        );
        assert_eq!(
            prepare_accept_group_invitation("   "),
            Err(GroupsStorefrontInvitationInputError::MissingToken)
        );
        assert_eq!(
            prepare_accept_group_invitation("too-short"),
            Err(GroupsStorefrontInvitationInputError::InvalidTokenLength)
        );
    }

    #[test]
    fn targeted_invitation_acceptance_requires_uuid() {
        let command =
            prepare_accept_targeted_group_invitation("550e8400-e29b-41d4-a716-446655440000")
                .expect("targeted invitation id must be accepted");
        assert_eq!(
            command.invitation_id,
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert!(
            command
                .idempotency_key
                .starts_with("groups-storefront-accept-targeted-invitation-")
        );
        assert_eq!(
            prepare_accept_targeted_group_invitation("not-a-uuid"),
            Err(GroupsStorefrontInvitationInputError::InvalidInvitationId)
        );
    }
}
