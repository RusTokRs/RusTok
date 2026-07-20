pub mod governance;
pub mod governance_form;
pub mod json_editor;
pub mod metadata;
pub mod metadata_checklist_view;
pub mod version_trail;

use crate::Locale;
use crate::entities::module::MarketplaceModule;
use crate::entities::module::model::MarketplaceModuleVersion;

pub fn tr(locale: Locale, en: &'static str, ru: &'static str) -> &'static str {
    match locale {
        Locale::ru => ru,
        _ => en,
    }
}

pub fn humanize_token(value: &str) -> String {
    value
        .split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn humanize_setting_key(value: &str) -> String {
    let mut rendered = String::new();
    let mut previous_was_lowercase = false;

    for ch in value.chars() {
        if (ch == '_' || ch == '-') && !rendered.ends_with(' ') {
            rendered.push(' ');
            previous_was_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_was_lowercase && !rendered.ends_with(' ') {
            rendered.push(' ');
        }

        rendered.push(ch);
        previous_was_lowercase = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }

    humanize_token(rendered.trim())
}

pub fn short_checksum(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.len() > 16 {
        Some(format!("{}...", &value[..12]))
    } else {
        Some(value.to_string())
    }
}

pub fn latest_active_registry_version(
    module: &MarketplaceModule,
) -> Option<&MarketplaceModuleVersion> {
    module.versions.iter().find(|version| !version.yanked)
}

pub fn looks_like_absolute_http_url(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("https://") || value.starts_with("http://")
}

pub fn asset_path_without_query(value: &str) -> &str {
    value.split(['?', '#']).next().unwrap_or(value)
}

pub fn looks_like_svg_url(value: &str) -> bool {
    looks_like_absolute_http_url(value) && asset_path_without_query(value).ends_with(".svg")
}

pub fn looks_like_image_url(value: &str) -> bool {
    if !looks_like_absolute_http_url(value) {
        return false;
    }

    let lower = asset_path_without_query(value).to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp", ".svg"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}
