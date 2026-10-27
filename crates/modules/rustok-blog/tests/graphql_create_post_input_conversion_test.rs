use rustok_api::RichTextDocument;
use rustok_blog::{
    CreatePostInput as DomainCreatePostInput, graphql::CreatePostInput as GraphqlCreatePostInput,
};
use uuid::Uuid;

#[test]
fn create_post_input_conversion_preserves_canonical_content() {
    let canonical = RichTextDocument::single_paragraph("canonical content");
    let input = GraphqlCreatePostInput {
        locale: "ru".to_string(),
        title: "Заголовок".to_string(),
        content: canonical.clone(),
        excerpt: Some("excerpt".to_string()),
        slug: Some("post".to_string()),
        publish: true,
        tags: vec!["tag".to_string()],
        category_id: Some(Uuid::nil()),
        featured_image_url: Some("https://example.test/image.png".to_string()),
        seo_title: Some("SEO".to_string()),
        seo_description: Some("description".to_string()),
        channel_slugs: Some(vec!["web".to_string()]),
        scheduled_at: None,
    };
    let domain: DomainCreatePostInput = input.try_into().expect("valid input converts");
    assert_eq!(domain.content, canonical);
    assert_eq!(domain.category_id, Some(Uuid::nil()));
    assert!(domain.metadata.is_none());
}

#[test]
fn create_post_input_conversion_rejects_invalid_scheduled_at() {
    let input = GraphqlCreatePostInput {
        locale: "en".to_string(),
        title: "Title".to_string(),
        content: RichTextDocument::single_paragraph("body"),
        excerpt: None,
        slug: None,
        publish: false,
        tags: Vec::new(),
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
        scheduled_at: Some("next friday".to_string()),
    };
    let result = DomainCreatePostInput::try_from(input);
    assert!(
        matches!(result, Err(rustok_blog::BlogError::Validation(_))),
        "an invalid scheduled_at must be a validation error, not a dropped schedule"
    );
}
