use crate::core::{BlogPostEditorFormState, BlogPostFormInput, build_blog_post_draft};
use crate::model::{BlogCategoryDetail, BlogPostDetail, BlogPostDraft};

pub(super) fn editor_post() -> BlogPostDetail {
    serde_json::from_value(serde_json::json!({
        "id":"post-1", "requestedLocale":"en", "effectiveLocale":"en", "availableLocales":["en"],
        "title":"Source title", "slug":"source", "excerpt":"Source excerpt", "content":null,
        "contentPlainText":"Source body", "status":"draft", "createdAt":"2026-11-05T00:00:00Z",
        "updatedAt":"2026-11-05T00:00:00Z", "publishedAt":null, "scheduledAt":null,
        "isPinned":false, "pinnedAt":null, "tags":[], "categoryId":"old-category", "channelSlugs":[],
        "featuredImageUrl":null, "seoTitle":"Source SEO", "seoDescription":null, "version":8
    })).unwrap()
}

pub(super) fn editor_draft(post: &BlogPostDetail) -> BlogPostDraft {
    let state = BlogPostEditorFormState::from_post(post);
    build_blog_post_draft(BlogPostFormInput {
        locale: &state.locale,
        version: state.version,
        title: &state.title,
        slug: &state.slug,
        excerpt: &state.excerpt,
        content: &state.content,
        publish: false,
        tags: &state.tags_input,
        category_id: &state.category_id,
        channel_slugs: &state.channel_slugs_input,
        featured_image_url: &state.featured_image_url,
        seo_title: &state.seo_title,
        seo_description: &state.seo_description,
        scheduled_at: &state.scheduled_at,
        schedule_editable: state.schedule_editable,
    })
}

pub(super) fn category_snapshot() -> BlogCategoryDetail {
    serde_json::from_value(serde_json::json!({"id":"11111111-1111-1111-1111-111111111111", "locale":"en", "effectiveLocale":"en", "name":"News", "slug":"news", "description":"loaded text", "parentId":null, "position":0, "createdAt":"2026-11-05T00:00:00Z", "updatedAt":null})).unwrap()
}
