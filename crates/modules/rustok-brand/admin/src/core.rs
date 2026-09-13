use rustok_ui_core::normalize_ui_text;

use crate::model::BrandAdminShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrandAdminTransportProfile {
    Native,
    Graphql,
}

impl BrandAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> BrandAdminTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => BrandAdminTransportProfile::Graphql,
        _ => BrandAdminTransportProfile::Native,
    }
}

pub fn build_brand_admin_shell(
    locale: Option<&str>,
    profile: BrandAdminTransportProfile,
) -> BrandAdminShell {
    let russian = locale
        .map(|value| value.eq_ignore_ascii_case("ru") || value.starts_with("ru-"))
        .unwrap_or(false);
    if russian {
        BrandAdminShell {
            title: "Бренды".to_string(),
            subtitle: "Управление каталогом брендов, производителями и логотипами".to_string(),
            empty_state: "Транспорт брендов ещё не подключён к этому хосту".to_string(),
            transport_profile: profile.as_str().to_string(),
        }
    } else {
        BrandAdminShell {
            title: "Brands".to_string(),
            subtitle: "Manage brand catalog, manufacturers, and media presentation".to_string(),
            empty_state: "Brand transport is not mounted in this host yet".to_string(),
            transport_profile: profile.as_str().to_string(),
        }
    }
}

pub fn validate_brand_slug(slug: &str) -> Result<(), &'static str> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return Err("Slug cannot be empty");
    }
    if trimmed.len() > 100 {
        return Err("Slug cannot exceed 100 characters");
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Slug can only contain alphanumeric characters, hyphens, and underscores");
    }
    Ok(())
}

pub fn validate_brand_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty");
    }
    if trimmed.len() > 255 {
        return Err("Name cannot exceed 255 characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_selection_is_explicit_without_automatic_fallback() {
        assert_eq!(
            selected_transport_profile(Some("graphql")),
            BrandAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            BrandAdminTransportProfile::Native
        );
    }

    #[test]
    fn slug_validation_rules() {
        assert!(validate_brand_slug("apple").is_ok());
        assert!(validate_brand_slug("sony-electronics").is_ok());
        assert!(validate_brand_slug("samsung_2026").is_ok());
        assert!(validate_brand_slug("").is_err());
        assert!(validate_brand_slug("invalid slug with spaces").is_err());
        assert!(validate_brand_slug("bad@slug").is_err());
    }

    #[test]
    fn name_validation_rules() {
        assert!(validate_brand_name("Sony").is_ok());
        assert!(validate_brand_name("").is_err());
        assert!(validate_brand_name("   ").is_err());
    }
}
