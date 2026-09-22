use rustok_ui_core::normalize_ui_text;

use crate::model::BundleAdminShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleAdminTransportProfile {
    Native,
    Graphql,
}

impl BundleAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> BundleAdminTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => BundleAdminTransportProfile::Graphql,
        _ => BundleAdminTransportProfile::Native,
    }
}

pub fn build_bundle_admin_shell(
    locale: Option<&str>,
    profile: BundleAdminTransportProfile,
) -> BundleAdminShell {
    let russian = locale
        .map(|value| value.eq_ignore_ascii_case("ru") || value.starts_with("ru-"))
        .unwrap_or(false);
    if russian {
        BundleAdminShell {
            title: "Комплекты товаров".to_string(),
            subtitle: "Управление комплектами, наборами и скидками на комплекты".to_string(),
            empty_state: "Транспорт комплектов ещё не подключён к этому хосту".to_string(),
            transport_profile: profile.as_str().to_string(),
        }
    } else {
        BundleAdminShell {
            title: "Product Bundles".to_string(),
            subtitle: "Manage product bundles, kits, and package discounts".to_string(),
            empty_state: "Bundle transport is not mounted in this host yet".to_string(),
            transport_profile: profile.as_str().to_string(),
        }
    }
}

pub fn validate_bundle_slug(slug: &str) -> Result<(), &'static str> {
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

pub fn validate_bundle_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty");
    }
    if trimmed.len() > 255 {
        return Err("Name cannot exceed 255 characters");
    }
    Ok(())
}

pub fn validate_bundle_discount(
    discount_type: &str,
    discount_value: &str,
) -> Result<(), &'static str> {
    if discount_type == "none" || discount_type.is_empty() {
        return Ok(());
    }

    let val = discount_value.trim().parse::<f64>().map_err(|_| "Discount value must be a number")?;
    if val < 0.0 {
        return Err("Discount value cannot be negative");
    }

    if discount_type == "percentage" && val > 100.0 {
        return Err("Percentage discount cannot exceed 100%");
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
            BundleAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            BundleAdminTransportProfile::Native
        );
        assert_eq!(
            selected_transport_profile(None),
            BundleAdminTransportProfile::Native
        );
    }

    #[test]
    fn slug_validation_rules() {
        assert!(validate_bundle_slug("starter-kit").is_ok());
        assert!(validate_bundle_slug("combo_2026").is_ok());
        assert!(validate_bundle_slug("").is_err());
        assert!(validate_bundle_slug("spaces in slug").is_err());
        assert!(validate_bundle_slug("bad@slug").is_err());
    }

    #[test]
    fn discount_validation_rules() {
        assert!(validate_bundle_discount("none", "").is_ok());
        assert!(validate_bundle_discount("percentage", "15").is_ok());
        assert!(validate_bundle_discount("percentage", "105").is_err());
        assert!(validate_bundle_discount("fixed_amount", "500").is_ok());
        assert!(validate_bundle_discount("fixed_amount", "-10").is_err());
        assert!(validate_bundle_discount("percentage", "not_a_number").is_err());
    }
}
