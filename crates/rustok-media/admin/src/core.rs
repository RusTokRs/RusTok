use crate::model::{
    MediaListItem, MediaTranslationPayload, MediaUsageSnapshot, UpsertTranslationPayload,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaAdminBusyKey {
    Upload,
    Translation,
    Delete(String),
}

impl MediaAdminBusyKey {
    pub fn as_storage_key(&self) -> String {
        match self {
            Self::Upload => "upload".to_string(),
            Self::Translation => "translation".to_string(),
            Self::Delete(media_id) => format!("delete:{media_id}"),
        }
    }
}

pub fn is_busy_key(current: Option<&str>, expected: MediaAdminBusyKey) -> bool {
    current == Some(expected.as_storage_key().as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAdminErrorMessage {
    pub message: String,
}

pub fn media_admin_context_error(
    context: &str,
    err: impl std::fmt::Display,
) -> MediaAdminErrorMessage {
    MediaAdminErrorMessage {
        message: format!("{context}: {err}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaListCardLabels {
    pub bytes_template: String,
    pub dimensions_not_available: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaListCardViewModel {
    pub original_name: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_label: String,
    pub dimensions_label: String,
    pub storage_driver: String,
}

pub fn media_list_card_view_model(
    item: &MediaListItem,
    labels: MediaListCardLabels,
) -> MediaListCardViewModel {
    MediaListCardViewModel {
        original_name: item.original_name.clone(),
        public_url: item.public_url.clone(),
        mime_type: item.mime_type.clone(),
        size_label: labels
            .bytes_template
            .replace("{count}", &item.size.to_string()),
        dimensions_label: media_dimensions_label(
            item.width,
            item.height,
            &labels.dimensions_not_available,
        ),
        storage_driver: item.storage_driver.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaUploadSuccessState {
    pub selected_media_id: String,
    pub should_refresh: bool,
}

pub fn media_upload_success_state(media_id: impl Into<String>) -> MediaUploadSuccessState {
    MediaUploadSuccessState {
        selected_media_id: media_id.into(),
        should_refresh: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDetailLineViewModel {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaDetailLabels {
    pub original_name: String,
    pub id: String,
    pub mime: String,
    pub storage: String,
    pub public_url: String,
    pub size: String,
    pub created: String,
}

pub fn media_detail_lines(
    item: &MediaListItem,
    labels: MediaDetailLabels,
    bytes_template: &str,
) -> Vec<MediaDetailLineViewModel> {
    vec![
        MediaDetailLineViewModel {
            label: labels.original_name,
            value: item.original_name.clone(),
        },
        MediaDetailLineViewModel {
            label: labels.id,
            value: item.id.clone(),
        },
        MediaDetailLineViewModel {
            label: labels.mime,
            value: item.mime_type.clone(),
        },
        MediaDetailLineViewModel {
            label: labels.storage,
            value: item.storage_driver.clone(),
        },
        MediaDetailLineViewModel {
            label: labels.public_url,
            value: item.public_url.clone(),
        },
        MediaDetailLineViewModel {
            label: labels.size,
            value: bytes_template.replace("{count}", &item.size.to_string()),
        },
        MediaDetailLineViewModel {
            label: labels.created,
            value: item.created_at.clone(),
        },
    ]
}

/// Trims user-entered optional metadata and keeps the transport payload free of
/// empty strings. This helper is framework-agnostic so future FFA adapters can
/// reuse the same form-to-command policy without depending on framework-specific signals.
pub fn non_empty_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Builds the asset dimensions label used by UI adapters. Missing partial
/// dimensions intentionally fall back to the host-localized `not_available`
/// label instead of exposing inconsistent `width × ?` strings.
pub fn media_dimensions_label(
    width: Option<i32>,
    height: Option<i32>,
    not_available: &str,
) -> String {
    width
        .zip(height)
        .map(|(width, height)| format!("{width}×{height}"))
        .unwrap_or_else(|| not_available.to_string())
}

/// Applies the admin pagination label template to a concrete page number.
pub fn page_count_label(template: &str, page: i32) -> String {
    template.replace("{count}", &page.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTranslationFormState {
    pub title: String,
    pub alt_text: String,
    pub caption: String,
}

impl MediaTranslationFormState {
    pub fn empty() -> Self {
        Self {
            title: String::new(),
            alt_text: String::new(),
            caption: String::new(),
        }
    }

    pub fn from_translation(translation: &MediaTranslationPayload) -> Self {
        Self {
            title: translation.title.clone().unwrap_or_default(),
            alt_text: translation.alt_text.clone().unwrap_or_default(),
            caption: translation.caption.clone().unwrap_or_default(),
        }
    }

    pub fn to_upsert_payload(&self, locale: String) -> UpsertTranslationPayload {
        UpsertTranslationPayload {
            locale,
            title: non_empty_option(&self.title),
            alt_text: non_empty_option(&self.alt_text),
            caption: non_empty_option(&self.caption),
        }
    }
}

pub fn selected_translation_form_state(
    translations: &[MediaTranslationPayload],
    selected_locale: &str,
) -> MediaTranslationFormState {
    translations
        .iter()
        .find(|item| item.locale == selected_locale)
        .map(MediaTranslationFormState::from_translation)
        .unwrap_or_else(MediaTranslationFormState::empty)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaUsageLabels {
    pub files: String,
    pub total_bytes: String,
    pub tenant: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaUsageStatCard {
    pub label: String,
    pub value: String,
}

pub fn media_usage_stat_cards(
    snapshot: MediaUsageSnapshot,
    labels: MediaUsageLabels,
) -> [MediaUsageStatCard; 3] {
    [
        MediaUsageStatCard {
            label: labels.files,
            value: snapshot.file_count.to_string(),
        },
        MediaUsageStatCard {
            label: labels.total_bytes,
            value: snapshot.total_bytes.to_string(),
        },
        MediaUsageStatCard {
            label: labels.tenant,
            value: snapshot.tenant_id,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media_item() -> MediaListItem {
        MediaListItem {
            id: "media-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            uploaded_by: None,
            filename: "hero.webp".to_string(),
            original_name: "Hero.webp".to_string(),
            mime_type: "image/webp".to_string(),
            size: 2048,
            storage_driver: "s3".to_string(),
            public_url: "https://cdn.example.test/hero.webp".to_string(),
            width: Some(1200),
            height: Some(630),
            created_at: "2026-06-08T11:43:16Z".to_string(),
        }
    }

    fn translation(locale: &str, title: Option<&str>) -> MediaTranslationPayload {
        MediaTranslationPayload {
            id: format!("translation-{locale}"),
            media_id: "media-1".to_string(),
            locale: locale.to_string(),
            title: title.map(str::to_string),
            alt_text: Some(format!("alt-{locale}")),
            caption: None,
        }
    }

    #[test]
    fn busy_key_helpers_keep_transport_action_keys_stable() {
        assert_eq!(MediaAdminBusyKey::Upload.as_storage_key(), "upload");
        assert_eq!(
            MediaAdminBusyKey::Translation.as_storage_key(),
            "translation"
        );
        assert_eq!(
            MediaAdminBusyKey::Delete("media-1".to_string()).as_storage_key(),
            "delete:media-1"
        );
        assert!(is_busy_key(Some("upload"), MediaAdminBusyKey::Upload));
        assert!(!is_busy_key(Some("translation"), MediaAdminBusyKey::Upload));
    }

    #[test]
    fn upload_success_state_selects_uploaded_asset_and_requests_refresh() {
        assert_eq!(
            media_upload_success_state("media-1"),
            MediaUploadSuccessState {
                selected_media_id: "media-1".to_string(),
                should_refresh: true,
            }
        );
    }

    #[test]
    fn detail_lines_preserve_admin_detail_order_and_size_format() {
        let lines = media_detail_lines(
            &media_item(),
            MediaDetailLabels {
                original_name: "Original".to_string(),
                id: "ID".to_string(),
                mime: "MIME".to_string(),
                storage: "Storage".to_string(),
                public_url: "URL".to_string(),
                size: "Size".to_string(),
                created: "Created".to_string(),
            },
            "{count} bytes",
        );

        assert_eq!(lines[0].value, "Hero.webp");
        assert_eq!(lines[5].value, "2048 bytes");
        assert_eq!(lines[6].label, "Created");
    }

    #[test]
    fn non_empty_option_trims_and_drops_empty_values() {
        assert_eq!(
            non_empty_option("  Alt text  "),
            Some("Alt text".to_string())
        );
        assert_eq!(non_empty_option("   "), None);
    }

    #[test]
    fn media_dimensions_label_requires_both_dimensions() {
        assert_eq!(
            media_dimensions_label(Some(640), Some(480), "n/a"),
            "640×480"
        );
        assert_eq!(media_dimensions_label(Some(640), None, "n/a"), "n/a");
        assert_eq!(media_dimensions_label(None, Some(480), "n/a"), "n/a");
    }

    #[test]
    fn page_count_label_replaces_count_placeholder() {
        assert_eq!(page_count_label("Page {count}", 3), "Page 3");
    }

    #[test]
    fn selected_translation_form_state_uses_matching_locale_or_empty_state() {
        let translations = vec![translation("en", Some("English")), translation("ru", None)];

        assert_eq!(
            selected_translation_form_state(&translations, "en"),
            MediaTranslationFormState {
                title: "English".to_string(),
                alt_text: "alt-en".to_string(),
                caption: String::new(),
            }
        );
        assert_eq!(
            selected_translation_form_state(&translations, "de"),
            MediaTranslationFormState::empty()
        );
    }

    #[test]
    fn translation_form_state_builds_trimmed_upsert_payload() {
        let state = MediaTranslationFormState {
            title: "  Title  ".to_string(),
            alt_text: " ".to_string(),
            caption: "Caption".to_string(),
        };

        assert_eq!(
            state.to_upsert_payload("en".to_string()),
            UpsertTranslationPayload {
                locale: "en".to_string(),
                title: Some("Title".to_string()),
                alt_text: None,
                caption: Some("Caption".to_string()),
            }
        );
    }

    #[test]
    fn media_usage_stat_cards_preserve_label_order() {
        let cards = media_usage_stat_cards(
            MediaUsageSnapshot {
                tenant_id: "tenant-a".to_string(),
                file_count: 2,
                total_bytes: 2048,
            },
            MediaUsageLabels {
                files: "Files".to_string(),
                total_bytes: "Total".to_string(),
                tenant: "Tenant".to_string(),
            },
        );

        assert_eq!(cards[0].value, "2");
        assert_eq!(cards[1].value, "2048");
        assert_eq!(cards[2].value, "tenant-a");
    }

    #[test]
    fn list_card_view_model_formats_reusable_display_policy() {
        let vm = media_list_card_view_model(
            &media_item(),
            MediaListCardLabels {
                bytes_template: "{count} bytes".to_string(),
                dimensions_not_available: "n/a".to_string(),
            },
        );

        assert_eq!(vm.original_name, "Hero.webp");
        assert_eq!(vm.size_label, "2048 bytes");
        assert_eq!(vm.dimensions_label, "1200×630");
        assert_eq!(vm.storage_driver, "s3");
    }

    #[test]
    fn context_error_keeps_ui_error_prefix_policy_outside_leptos() {
        let message = media_admin_context_error("Failed to load media library", "timeout");
        assert_eq!(message.message, "Failed to load media library: timeout");
    }
}
