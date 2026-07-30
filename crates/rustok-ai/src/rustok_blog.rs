use std::ops::Deref;

use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub use rustok_blog_owner::migrations;
pub use rustok_blog_owner::{CreatePostInput, UpdatePostInput};

#[derive(Debug, Clone)]
pub struct PostResponse(rustok_blog_owner::PostResponse);

impl Deref for PostResponse {
    type Target = rustok_blog_owner::PostResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct PostService {
    inner: rustok_blog_owner::PostService,
}

impl PostService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            inner: rustok_blog_owner::PostService::new(db, event_bus),
        }
    }

    pub async fn create_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut input: CreatePostInput,
    ) -> rustok_blog_owner::BlogResult<Uuid> {
        canonicalize_create_input(&mut input);
        self.inner.create_post(tenant_id, security, input).await
    }

    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        mut input: UpdatePostInput,
    ) -> rustok_blog_owner::BlogResult<()> {
        canonicalize_update_input(&mut input);
        self.inner
            .update_post(tenant_id, post_id, security, input)
            .await
    }

    pub async fn get_post_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> rustok_blog_owner::BlogResult<PostResponse> {
        let mut post = self
            .inner
            .get_post_with_locale_fallback(
                tenant_id,
                security,
                post_id,
                locale,
                fallback_locale,
            )
            .await?;

        if let Some(plain_text) = post.content_plain_text.clone() {
            post.body = plain_text;
            post.body_format = "plain_text".to_string();
            post.content_json = None;
        }

        Ok(PostResponse(post))
    }
}

impl Deref for PostService {
    type Target = rustok_blog_owner::PostService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

fn canonicalize_create_input(input: &mut CreatePostInput) {
    if input.content.is_none() {
        input.content = Some(
            rustok_blog_owner::richtext::article_document_from_plain_text(input.body.as_str()),
        );
    }
    input.body.clear();
    input.body_format = "richtext".to_string();
    input.content_json = None;
}

fn canonicalize_update_input(input: &mut UpdatePostInput) {
    if input.content.is_none() {
        if let Some(body) = input.body.as_deref() {
            input.content = Some(
                rustok_blog_owner::richtext::article_document_from_plain_text(body),
            );
        }
    }

    if input.content.is_some() {
        input.body = None;
        input.body_format = None;
        input.content_json = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreatePostInput, UpdatePostInput, canonicalize_create_input, canonicalize_update_input,
    };

    #[test]
    fn create_input_reaches_owner_as_canonical_article_content() {
        let mut input = CreatePostInput {
            locale: "ru".to_string(),
            title: "AI draft".to_string(),
            body: "First line\nsecond line\n\nNext paragraph".to_string(),
            body_format: "markdown".to_string(),
            content_json: None,
            content: None,
            excerpt: None,
            slug: None,
            publish: false,
            tags: vec![],
            category_id: None,
            featured_image_url: None,
            seo_title: None,
            seo_description: None,
            channel_slugs: None,
            metadata: None,
        };

        canonicalize_create_input(&mut input);

        assert!(input.body.is_empty());
        assert_eq!(input.body_format, "richtext");
        assert!(input.content_json.is_none());
        let document = input.content.expect("canonical article document");
        assert_eq!(document.kind, "doc");
        assert_eq!(document.content.len(), 2);
        assert_eq!(
            document.content[0].content[0].text.as_deref(),
            Some("First line second line")
        );
    }

    #[test]
    fn update_input_removes_legacy_writer_fields_before_owner_call() {
        let mut input = UpdatePostInput {
            body: Some("Updated AI draft".to_string()),
            body_format: Some("markdown".to_string()),
            ..Default::default()
        };

        canonicalize_update_input(&mut input);

        assert!(input.body.is_none());
        assert!(input.body_format.is_none());
        assert!(input.content_json.is_none());
        let document = input.content.expect("canonical article document");
        assert_eq!(
            document.content[0].content[0].text.as_deref(),
            Some("Updated AI draft")
        );
    }
}
