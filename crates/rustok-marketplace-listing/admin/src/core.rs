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
    let russian = locale
        .map(|value| value.eq_ignore_ascii_case("ru") || value.starts_with("ru-"))
        .unwrap_or(false);
    if russian {
        MarketplaceListingAdminShell {
            title: "Листинги маркетплейса".to_string(),
            subtitle: "Управление публикацией, коммерческими ссылками и историей листингов"
                .to_string(),
            empty_state: "Транспорт листингов ещё не подключён к этому хосту".to_string(),
            legacy_attribution_label: "Импортированная запись: исходный оператор и язык неизвестны"
                .to_string(),
            transport_profile: profile.as_str().to_string(),
        }
    } else {
        MarketplaceListingAdminShell {
            title: "Marketplace listings".to_string(),
            subtitle: "Manage publication, commercial references, and listing history".to_string(),
            empty_state: "Listing transport is not mounted in this host yet".to_string(),
            legacy_attribution_label: "Imported record: original operator and locale are unknown"
                .to_string(),
            transport_profile: profile.as_str().to_string(),
        }
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
