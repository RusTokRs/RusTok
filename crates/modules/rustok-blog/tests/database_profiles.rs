mod support;

use std::sync::Arc;

use rustok_api::RichTextDocument;
use rustok_blog::{
    PostService,
    dto::{CreatePostInput, PublicPostsPageQuery},
};
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_test_utils::db::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection,
    sea_query::{Alias, Expr, Query},
};
use uuid::Uuid;

// The identical owner scenario runs on both supported profiles. PostgreSQL's
// trigger/lock enforcement still has its separate contention tests; a portable
// scenario must never imply that SQLite proves those engine-specific effects.
async fn channel_publication_and_locale_contract(db: DatabaseConnection) {
    support::apply_blog_schema(&db).await.unwrap();
    let tenant = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    for id in [tenant, foreign_tenant] {
        let mut insert = Query::insert();
        insert
            .into_table(Alias::new("tenants"))
            .columns([Alias::new("id"), Alias::new("name"), Alias::new("slug")])
            .values_panic([
                Expr::value(id),
                Expr::value("Fixture"),
                Expr::value(id.to_string()),
            ]);
        db.execute_raw(db.get_database_backend().build(&insert))
            .await
            .unwrap();
    }
    for slug in ["web", "mobile"] {
        rustok_channel::ChannelService::new(db.clone())
            .create_channel(rustok_channel::CreateChannelInput {
                tenant_id: tenant,
                slug: slug.into(),
                name: slug.into(),
                settings: None,
            })
            .await
            .unwrap();
    }
    let posts = PostService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let mut expected = Vec::new();
    for (owner, slug, published, channels, visible) in [
        (tenant, "web-a", true, Some(vec!["web".into()]), true),
        (tenant, "web-b", true, None, true),
        (
            tenant,
            "mobile-only",
            true,
            Some(vec!["mobile".into()]),
            false,
        ),
        (tenant, "draft", false, None, false),
        (foreign_tenant, "foreign", true, None, false),
    ] {
        let id = posts
            .create_post(
                owner,
                admin.clone(),
                CreatePostInput {
                    command_id: uuid::Uuid::new_v4(),
                    locale: "en".into(),
                    title: slug.into(),
                    content: RichTextDocument::single_paragraph("Fixture body"),
                    excerpt: None,
                    slug: Some(slug.into()),
                    publish: published,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: channels,
                    metadata: None,
                    scheduled_at: None,
                },
            )
            .await
            .unwrap();
        if visible {
            expected.push(id);
        }
    }
    let mut cursor = None;
    let mut actual = Vec::new();
    loop {
        let page = posts
            .list_public_visible_keyset(
                tenant,
                PublicPostsPageQuery {
                    locale: Some("fr".into()),
                    per_page: Some(1),
                    after: cursor,
                    ..Default::default()
                },
                Some("en"),
                Some("web"),
            )
            .await
            .unwrap();
        assert_eq!(
            page.items.len(),
            1,
            "hidden rows must be filtered before page limits"
        );
        let post = &page.items[0];
        assert_eq!(post.locale, "fr");
        assert_eq!(post.effective_locale, "en");
        assert_eq!(post.available_locales, ["en"]);
        actual.push(post.id);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
        assert!(
            actual.len() < 3,
            "keyset traversal must not loop or repeat rows"
        );
    }
    actual.sort();
    expected.sort();
    assert_eq!(
        actual, expected,
        "one page per visible owner row, no draft/channel/tenant leaks"
    );
}

#[tokio::test]
async fn sqlite_shipped_migrations_preserve_channel_publication_and_locale_contract() {
    channel_publication_and_locale_contract(rustok_test_utils::setup_test_db().await).await;
}

#[tokio::test]
#[ignore = "requires explicit RUSTOK_BLOG_TEST_DATABASE_URL and disposable-database permission"]
async fn postgres_shipped_migrations_preserve_channel_publication_and_locale_contract() {
    let url = support::postgres_database_url().unwrap();
    let control = connect_postgres(&url).await.unwrap();
    let name = unique_postgres_database_name("blog_profiles");
    create_postgres_database(&control, &name).await.unwrap();
    let url = postgres_database_url(&url, &name);
    let result = tokio::spawn(async move {
        channel_publication_and_locale_contract(connect_postgres(&url).await.unwrap()).await;
    })
    .await;
    drop_postgres_database_if_exists(&control, &name)
        .await
        .unwrap();
    if let Err(error) = result {
        std::panic::resume_unwind(error.into_panic());
    }
}
