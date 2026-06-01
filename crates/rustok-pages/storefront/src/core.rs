use crate::model::{PageBlock, PageDetail};

pub fn selected_page_title(page: &PageDetail, default_title: String) -> String {
    page.translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .unwrap_or(default_title)
}

pub fn selected_page_slug(page: &PageDetail, default_slug: String) -> String {
    page.translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .unwrap_or(default_slug)
}

pub fn selected_page_effective_locale(page: &PageDetail, default_locale: String) -> String {
    page.effective_locale.clone().unwrap_or(default_locale)
}

pub fn summarize_page_content<F>(
    page: &PageDetail,
    summarize_content: F,
    empty_fallback: String,
) -> String
where
    F: Fn(&str, &str) -> String,
{
    if let Some(body) = page.body.as_ref() {
        return summarize_content(body.content.as_str(), body.format.as_str());
    }

    if !page.blocks.is_empty() {
        return summarize_legacy_blocks(&page.blocks);
    }

    empty_fallback
}

fn summarize_legacy_blocks(blocks: &[PageBlock]) -> String {
    let mut rows = Vec::with_capacity(blocks.len());
    for block in blocks {
        rows.push(format!("#{} {}", block.position + 1, block.block_type));
    }

    format!(
        "Legacy blocks are still attached to this page: {}.",
        rows.join(", ")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorefrontBuilderFallbackReadContract {
    pub profile: &'static str,
    pub read_paths_stable: bool,
    pub list_paths_stable: bool,
    pub render_requires_builder_capability: bool,
}

pub fn storefront_builder_fallback_read_contract(
    profile: &str,
) -> Option<StorefrontBuilderFallbackReadContract> {
    match profile {
        "all_on" => Some(StorefrontBuilderFallbackReadContract {
            profile: "all_on",
            read_paths_stable: true,
            list_paths_stable: true,
            render_requires_builder_capability: false,
        }),
        "publish_off" => Some(StorefrontBuilderFallbackReadContract {
            profile: "publish_off",
            read_paths_stable: true,
            list_paths_stable: true,
            render_requires_builder_capability: false,
        }),
        "preview_off" => Some(StorefrontBuilderFallbackReadContract {
            profile: "preview_off",
            read_paths_stable: true,
            list_paths_stable: true,
            render_requires_builder_capability: false,
        }),
        "builder_off" => Some(StorefrontBuilderFallbackReadContract {
            profile: "builder_off",
            read_paths_stable: true,
            list_paths_stable: true,
            render_requires_builder_capability: false,
        }),
        _ => None,
    }
}

pub fn raw_body_format_summary(format: &str, char_count: usize, template: &str) -> String {
    template
        .replace("{format}", format)
        .replace("{count}", &char_count.to_string())
}

pub fn count_label(template: &str, count: u64) -> String {
    template.replace("{count}", &count.to_string())
}

pub fn open_link_label(prefix: &str, slug: &str) -> String {
    format!("{} {}", prefix, slug)
}

pub fn label_value_pair(label: &str, value: &str) -> String {
    format!("{}: {}", label, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PageBody, PageDetail, PageTranslation};

    #[test]
    fn storefront_builder_fallback_profiles_keep_read_and_list_stable() {
        for profile in ["all_on", "publish_off", "preview_off", "builder_off"] {
            let contract = storefront_builder_fallback_read_contract(profile)
                .expect("profile must have a storefront read contract");
            assert_eq!(contract.profile, profile);
            assert!(contract.read_paths_stable);
            assert!(contract.list_paths_stable);
            assert!(!contract.render_requires_builder_capability);
        }

        assert!(storefront_builder_fallback_read_contract("unknown").is_none());
    }

    #[test]
    fn storefront_grapesjs_summary_does_not_require_builder_capability() {
        let page = PageDetail {
            effective_locale: Some("en".to_string()),
            translation: Some(PageTranslation {
                locale: "en".to_string(),
                title: Some("Landing".to_string()),
                slug: Some("landing".to_string()),
                meta_title: None,
                meta_description: None,
            }),
            body: Some(PageBody {
                locale: "en".to_string(),
                content: "{\"pages\":[]}".to_string(),
                format: "grapesjs_v1".to_string(),
            }),
            blocks: Vec::new(),
        };

        let summary = summarize_page_content(
            &page,
            |content, format| raw_body_format_summary(format, content.len(), "{format}:{count}"),
            "empty".to_string(),
        );

        assert_eq!(summary, "grapesjs_v1:12");
    }
}
