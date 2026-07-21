use crate::model::GroupsStorefrontFilters;

pub const DEFAULT_GROUPS_PAGE: u64 = 1;
pub const DEFAULT_GROUPS_PER_PAGE: u64 = 24;

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

pub fn selected_transport_profile(value: Option<&str>) -> GroupsStorefrontTransportProfile {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
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
}
