use rustok_api::normalize_locale_tag;
use uuid::Uuid;

use crate::model::{
    ChangeGroupRoleCommand, DeleteGroupTranslationCommand, GroupsAdminAssignableRole,
    GroupsAdminFilters, GroupsAdminTranslationQuery, TransferGroupOwnershipCommand,
    UpsertGroupTranslationCommand,
};

pub const DEFAULT_GROUPS_PAGE: u64 = 1;
pub const DEFAULT_GROUPS_PER_PAGE: u64 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminTransportProfile {
    Native,
    Graphql,
}

impl GroupsAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> GroupsAdminTransportProfile {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "graphql" => GroupsAdminTransportProfile::Graphql,
        _ => GroupsAdminTransportProfile::Native,
    }
}

pub fn default_groups_admin_filters() -> GroupsAdminFilters {
    GroupsAdminFilters {
        page: DEFAULT_GROUPS_PAGE,
        per_page: DEFAULT_GROUPS_PER_PAGE,
        search: None,
        include_non_public: true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupsAdminHeaderViewModel {
    pub title: String,
    pub body: String,
    pub badge: String,
}

pub fn groups_admin_header(
    title: impl Into<String>,
    body: impl Into<String>,
    badge: impl Into<String>,
) -> GroupsAdminHeaderViewModel {
    GroupsAdminHeaderViewModel {
        title: title.into(),
        body: body.into(),
        badge: badge.into(),
    }
}

pub fn groups_admin_error(prefix: &str, details: &str) -> String {
    if details.trim().is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}: {details}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminGovernanceInputError {
    InvalidGroupId,
    InvalidTargetUserId,
    InvalidNewOwnerUserId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupsAdminLocalizationInputError {
    InvalidGroupId,
    InvalidLocale,
    MissingTitle,
    TitleTooLong,
    SummaryTooLong,
}

pub fn prepare_change_group_role(
    group_id: &str,
    target_user_id: &str,
    role: GroupsAdminAssignableRole,
) -> Result<ChangeGroupRoleCommand, GroupsAdminGovernanceInputError> {
    let group_id = normalize_uuid(group_id)
        .map_err(|_| GroupsAdminGovernanceInputError::InvalidGroupId)?;
    let target_user_id = normalize_uuid(target_user_id)
        .map_err(|_| GroupsAdminGovernanceInputError::InvalidTargetUserId)?;
    Ok(ChangeGroupRoleCommand {
        idempotency_key: format!("groups-admin-change-role-{}", Uuid::new_v4()),
        group_id,
        target_user_id,
        role,
    })
}

pub fn prepare_transfer_group_ownership(
    group_id: &str,
    new_owner_user_id: &str,
) -> Result<TransferGroupOwnershipCommand, GroupsAdminGovernanceInputError> {
    let group_id = normalize_uuid(group_id)
        .map_err(|_| GroupsAdminGovernanceInputError::InvalidGroupId)?;
    let new_owner_user_id = normalize_uuid(new_owner_user_id)
        .map_err(|_| GroupsAdminGovernanceInputError::InvalidNewOwnerUserId)?;
    Ok(TransferGroupOwnershipCommand {
        idempotency_key: format!("groups-admin-transfer-owner-{}", Uuid::new_v4()),
        group_id,
        new_owner_user_id,
    })
}

pub fn prepare_group_translation_query(
    group_id: &str,
) -> Result<GroupsAdminTranslationQuery, GroupsAdminLocalizationInputError> {
    let group_id = normalize_uuid(group_id)
        .map_err(|_| GroupsAdminLocalizationInputError::InvalidGroupId)?;
    Ok(GroupsAdminTranslationQuery { group_id })
}

pub fn prepare_upsert_group_translation(
    group_id: &str,
    locale: &str,
    title: &str,
    summary: Option<String>,
    body: Option<String>,
) -> Result<UpsertGroupTranslationCommand, GroupsAdminLocalizationInputError> {
    let group_id = normalize_uuid(group_id)
        .map_err(|_| GroupsAdminLocalizationInputError::InvalidGroupId)?;
    let locale = normalize_locale_tag(locale)
        .ok_or(GroupsAdminLocalizationInputError::InvalidLocale)?;
    let title = title.trim();
    if title.is_empty() {
        return Err(GroupsAdminLocalizationInputError::MissingTitle);
    }
    if title.chars().count() > 240 {
        return Err(GroupsAdminLocalizationInputError::TitleTooLong);
    }
    let summary = normalize_optional_text(summary);
    if summary
        .as_deref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(GroupsAdminLocalizationInputError::SummaryTooLong);
    }
    Ok(UpsertGroupTranslationCommand {
        idempotency_key: format!("groups-admin-upsert-translation-{}", Uuid::new_v4()),
        group_id,
        locale,
        title: title.to_string(),
        summary,
        body: normalize_optional_text(body),
    })
}

pub fn prepare_delete_group_translation(
    group_id: &str,
    locale: &str,
) -> Result<DeleteGroupTranslationCommand, GroupsAdminLocalizationInputError> {
    let group_id = normalize_uuid(group_id)
        .map_err(|_| GroupsAdminLocalizationInputError::InvalidGroupId)?;
    let locale = normalize_locale_tag(locale)
        .ok_or(GroupsAdminLocalizationInputError::InvalidLocale)?;
    Ok(DeleteGroupTranslationCommand {
        idempotency_key: format!("groups-admin-delete-translation-{}", Uuid::new_v4()),
        group_id,
        locale,
    })
}

fn normalize_uuid(value: &str) -> Result<String, uuid::Error> {
    Uuid::parse_str(value.trim()).map(|value| value.to_string())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_directory_request_is_bounded() {
        let request = default_groups_admin_filters();
        assert_eq!(request.page, 1);
        assert_eq!(request.per_page, 24);
        assert!(request.include_non_public);
    }

    #[test]
    fn transport_selection_has_no_implicit_fallback() {
        assert_eq!(
            selected_transport_profile(Some("graphql")),
            GroupsAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            GroupsAdminTransportProfile::Native
        );
    }

    #[test]
    fn localization_preparation_normalizes_locale() {
        let command = prepare_upsert_group_translation(
            "550e8400-e29b-41d4-a716-446655440000",
            "pt_br",
            "Grupo",
            None,
            None,
        )
        .expect("valid localization command");
        assert_eq!(command.locale, "pt-BR");
    }
}
