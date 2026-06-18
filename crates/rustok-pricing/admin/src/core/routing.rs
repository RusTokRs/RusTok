use crate::i18n::t;
use crate::model::PricingChannelOption;

pub(crate) fn format_channel_scope_text(
    locale: Option<&str>,
    channel_id: Option<&str>,
    channel_slug: Option<&str>,
) -> Option<String> {
    let channel_slug = channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let channel_id = channel_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    if channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let channel_label = t(locale, "pricing.detail.channelInput", "channel");
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => {
            Some(format!("{channel_label} {channel_slug} ({channel_id})"))
        }
        (Some(channel_slug), None) => Some(format!("{channel_label} {channel_slug}")),
        (None, Some(channel_id)) => Some(format!("{channel_label} {channel_id}")),
        (None, None) => None,
    }
}

pub(crate) const GLOBAL_CHANNEL_KEY: &str = "__global__";
pub(crate) const LEGACY_CHANNEL_KEY: &str = "__legacy__";

pub(crate) fn normalize_channel_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn selected_channel_key(
    channel_id: &str,
    channel_slug: &str,
    available_channels: &[PricingChannelOption],
) -> String {
    let normalized_channel_id = normalize_channel_value(channel_id);
    let normalized_channel_slug = normalize_channel_value(channel_slug);

    if normalized_channel_id.is_none() && normalized_channel_slug.is_none() {
        return GLOBAL_CHANNEL_KEY.to_string();
    }

    if let Some(option) = available_channels.iter().find(|option| {
        normalized_channel_id.as_deref() == Some(option.id.as_str())
            || normalized_channel_slug.as_deref() == Some(option.slug.as_str())
    }) {
        return option.id.clone();
    }

    LEGACY_CHANNEL_KEY.to_string()
}

pub(crate) fn apply_selected_channel_option(
    selected_key: &str,
    fallback_channel_id: &str,
    fallback_channel_slug: &str,
    available_channels: &[PricingChannelOption],
) -> (String, String) {
    match selected_key {
        GLOBAL_CHANNEL_KEY => (String::new(), String::new()),
        LEGACY_CHANNEL_KEY => (
            fallback_channel_id.to_string(),
            fallback_channel_slug.to_string(),
        ),
        _ => available_channels
            .iter()
            .find(|option| option.id == selected_key)
            .map(|option| (option.id.clone(), option.slug.clone()))
            .unwrap_or_default(),
    }
}

pub(crate) fn legacy_channel_option_label(
    locale: Option<&str>,
    channel_id: &str,
    channel_slug: &str,
) -> String {
    format!(
        "{}: {}",
        t(locale, "pricing.channel.legacy", "Legacy scope"),
        format_channel_scope_text(
            None,
            normalize_channel_value(channel_id).as_deref(),
            normalize_channel_value(channel_slug).as_deref(),
        )
        .unwrap_or_else(|| t(locale, "pricing.common.notSet", "not set")),
    )
}

pub(crate) fn format_channel_option_label(
    locale: Option<&str>,
    option: &PricingChannelOption,
) -> String {
    let mut label = format!("{} ({})", option.name, option.slug);
    if option.is_default {
        label.push_str(format!(" | {}", t(locale, "pricing.channel.default", "default")).as_str());
    }
    if !option.is_active {
        label
            .push_str(format!(" | {}", t(locale, "pricing.channel.inactive", "inactive")).as_str());
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_channel_key_preserves_global_known_and_legacy_scopes() {
        let channels = [PricingChannelOption {
            id: "channel-id".to_string(),
            slug: "web".to_string(),
            name: "Web".to_string(),
            is_active: true,
            is_default: true,
            status: "ACTIVE".to_string(),
        }];

        assert_eq!(selected_channel_key("", "", &channels), GLOBAL_CHANNEL_KEY);
        assert_eq!(selected_channel_key("", " web ", &channels), "channel-id");
        assert_eq!(
            selected_channel_key("unknown", "", &channels),
            LEGACY_CHANNEL_KEY
        );
    }

    #[test]
    fn legacy_channel_option_label_formats_scope_or_not_set() {
        assert_eq!(
            legacy_channel_option_label(Some("en"), "channel-id", "web"),
            "Legacy scope: channel web (channel-id)"
        );
        assert_eq!(
            legacy_channel_option_label(Some("en"), "", ""),
            "Legacy scope: not set"
        );
    }
}
