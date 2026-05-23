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

pub fn raw_body_format_summary(format: &str, char_count: usize, template: &str) -> String {
    template
        .replace("{format}", format)
        .replace("{count}", &char_count.to_string())
}
