use rustok_ui_core::normalize_ui_text;

use crate::model::MarketplaceSellerAdminShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketplaceSellerAdminTransportProfile {
    Native,
    Graphql,
}

impl MarketplaceSellerAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> MarketplaceSellerAdminTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => MarketplaceSellerAdminTransportProfile::Graphql,
        _ => MarketplaceSellerAdminTransportProfile::Native,
    }
}

pub fn build_marketplace_seller_admin_shell(
    locale: Option<&str>,
    profile: MarketplaceSellerAdminTransportProfile,
) -> MarketplaceSellerAdminShell {
    use crate::i18n::t;

    MarketplaceSellerAdminShell {
        title: t(locale, "marketplaceSeller.title", "Marketplace sellers"),
        subtitle: t(
            locale,
            "marketplaceSeller.shell.subtitle",
            "Manage seller profiles, onboarding, and seller-scoped members",
        ),
        empty_state: t(
            locale,
            "marketplaceSeller.shell.emptyState",
            "Seller transport is not mounted in this host yet",
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
            MarketplaceSellerAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            MarketplaceSellerAdminTransportProfile::Native
        );
    }
}
