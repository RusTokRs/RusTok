use std::time::Duration;

use rustok_api::{PortActor, PortContext, PortErrorKind};
use uuid::Uuid;

use crate::{
    BLOG_PUBLISHED_POST_READ_CLAIM, MAX_BLOG_POST_SLUG_BYTES, ReadBlogPublishedPost,
    published_post_read_scope,
};

#[test]
fn published_read_requires_service_authority_and_a_tenant_bound_deadline() {
    let tenant_id = Uuid::new_v4();
    let request = ReadBlogPublishedPost {
        slug: "article".into(),
        fallback_locale: Some("en".into()),
    };
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("storefront"),
        "fr",
        "test",
    )
    .with_deadline(Duration::from_secs(1));
    assert_eq!(
        published_post_read_scope(&context, &request)
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden
    );

    let mut context = context.with_claim(BLOG_PUBLISHED_POST_READ_CLAIM);
    assert_eq!(
        published_post_read_scope(&context, &request).unwrap(),
        (tenant_id, Duration::from_secs(1))
    );

    context.actor = PortActor::user("visitor");
    assert_eq!(
        published_post_read_scope(&context, &request)
            .unwrap_err()
            .kind,
        PortErrorKind::Forbidden
    );
    context.actor = PortActor::service("storefront");
    context.tenant_id = Uuid::nil().to_string();
    assert_eq!(
        published_post_read_scope(&context, &request)
            .unwrap_err()
            .kind,
        PortErrorKind::Validation
    );
}

#[test]
fn published_read_rejects_an_unbounded_deadline_or_slug() {
    let context = PortContext::new(
        Uuid::new_v4().to_string(),
        PortActor::service("storefront"),
        "en",
        "test",
    )
    .with_claim(BLOG_PUBLISHED_POST_READ_CLAIM);
    let mut request = ReadBlogPublishedPost {
        slug: "article".into(),
        fallback_locale: None,
    };
    assert_eq!(
        published_post_read_scope(&context, &request)
            .unwrap_err()
            .kind,
        PortErrorKind::Timeout
    );
    assert_eq!(
        published_post_read_scope(
            &context.clone().with_deadline(Duration::from_secs(31)),
            &request
        )
        .unwrap_err()
        .kind,
        PortErrorKind::Validation
    );

    let context = context.with_deadline(Duration::from_secs(1));
    request.slug = "a".repeat(MAX_BLOG_POST_SLUG_BYTES);
    assert!(published_post_read_scope(&context, &request).is_ok());
    request.slug.push('a');
    assert_eq!(
        published_post_read_scope(&context, &request)
            .unwrap_err()
            .kind,
        PortErrorKind::Validation
    );
}
