use rustok_ui_core::normalize_ui_text;

use crate::model::MarketplaceListingAdminShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplaceListingAdminTransportProfile {
    Native,
    Graphql,
}

impl MarketplaceListingAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> MarketplaceListingAdminTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => MarketplaceListingAdminTransportProfile::Graphql,
        _ => MarketplaceListingAdminTransportProfile::Native,
    }
}

pub fn build_marketplace_listing_admin_shell(
    locale: Option<&str>,
    profile: MarketplaceListingAdminTransportProfile,
) -> MarketplaceListingAdminShell {
    use crate::i18n::t;

    MarketplaceListingAdminShell {
        title: t(locale, "marketplaceListing.title", "Marketplace listings"),
        subtitle: t(
            locale,
            "marketplaceListing.shell.subtitle",
            "Manage publication, commercial references, and listing history",
        ),
        empty_state: t(
            locale,
            "marketplaceListing.shell.emptyState",
            "Listing transport is not mounted in this host yet",
        ),
        legacy_attribution_label: t(
            locale,
            "marketplaceListing.legacyAttribution",
            "Imported record: original operator and locale are unknown",
        ),
        transport_profile: profile.as_str().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_selection_is_explicit_without_automatic_fallback() {
        assert_eq!(
            selected_transport_profile(Some("graphql")),
            MarketplaceListingAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            MarketplaceListingAdminTransportProfile::Native
        );
    }
}
