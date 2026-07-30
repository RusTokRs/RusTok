use rustok_api::RichTextDocument;
use rustok_blog::{
    CreatePostInput as DomainCreatePostInput,
    graphql::CreatePostInput as GraphqlCreatePostInput,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn create_post_input_conversion_preserves_transport_fields() {
    let canonical = RichTextDocument::single_paragraph("canonical content");
    let input = GraphqlCreatePostInput {
        locale: "ru".to_string(),
        title: "Заголовок".to_string(),
        body: Some("legacy body".to_string()),
        body_format: Some("markdown".to_string()),
        content_json: Some(json!({"type": "doc"})),
        content: Some(canonical.clone()),
        excerpt: Some("excerpt".to_string()),
        slug: Some("post".to_string()),
        publish: true,
        tags: vec!["tag".to_string()],
        category_id: Some(Uuid::nil()),
        featured_image_url: Some("https://example.test/image.png".to_string()),
        seo_title: Some("SEO".to_string()),
        seo_description: Some("description".to_string()),
        channel_slugs: Some(vec!["web".to_string()]),
    };

    let domain: DomainCreatePostInput = input.into();
    assert_eq!(domain.locale, "ru");
    assert_eq!(domain.title, "Заголовок");
    assert_eq!(domain.body, "legacy body");
    assert_eq!(domain.body_format, "markdown");
    assert_eq!(domain.content_json, Some(json!({"type": "doc"})));
    assert_eq!(domain.content, Some(canonical));
    assert_eq!(domain.excerpt.as_deref(), Some("excerpt"));
    assert_eq!(domain.slug.as_deref(), Some("post"));
    assert!(domain.publish);
    assert_eq!(domain.tags, vec!["tag".to_string()]);
    assert_eq!(domain.category_id, Some(Uuid::nil()));
    assert_eq!(
        domain.featured_image_url.as_deref(),
        Some("https://example.test/image.png")
    );
    assert_eq!(domain.seo_title.as_deref(), Some("SEO"));
    assert_eq!(domain.seo_description.as_deref(), Some("description"));
    assert_eq!(domain.channel_slugs, Some(vec!["web".to_string()]));
    assert!(domain.metadata.is_none());
}

#[test]
fn create_post_input_conversion_applies_legacy_defaults() {
    let input = GraphqlCreatePostInput {
        locale: "en".to_string(),
        title: "Title".to_string(),
        body: None,
        body_format: None,
        content_json: None,
        content: Some(RichTextDocument::empty()),
        excerpt: None,
        slug: None,
        publish: false,
        tags: Vec::new(),
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
    };

    let domain: DomainCreatePostInput = input.into();
    assert!(domain.body.is_empty());
    assert_eq!(domain.body_format, rustok_core::CONTENT_FORMAT_MARKDOWN);
    assert_eq!(domain.content, Some(RichTextDocument::empty()));
}
