use crate::model::GroupsAdminFilters;

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
}
