use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_outbox::SysEventsMigration;
use rustok_taxonomy::TaxonomyModule;
use rustok_test_utils::setup_test_db;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use super::orchestration::*;
use crate::entities::{blog_post, blog_post_translation};
use crate::error::BlogError;

async fn setup_schema() -> DatabaseConnection {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);

    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
    db
}

#[tokio::test]
async fn test_ensure_category_exists_validation() {
    let db = setup_schema().await;
    let txn = db.begin().await.unwrap();

    let err = ensure_blog_category_exists_in_tx(&txn, Uuid::nil(), Uuid::new_v4()).await;
    assert!(matches!(err, Err(BlogError::Validation(_))));

    let err = ensure_blog_category_exists_in_tx(&txn, Uuid::new_v4(), Uuid::nil()).await;
    assert!(matches!(err, Err(BlogError::Validation(_))));

    let err = ensure_blog_category_exists_in_tx(&txn, Uuid::new_v4(), Uuid::new_v4()).await;
    assert!(matches!(err, Err(BlogError::CategoryNotFound(_))));
}

#[tokio::test]
async fn test_ensure_slug_unique_validation() {
    let db = setup_schema().await;
    let txn = db.begin().await.unwrap();

    let err = ensure_blog_slug_unique_in_tx(&txn, Uuid::nil(), "my-slug").await;
    assert!(matches!(err, Err(BlogError::Validation(_))));

    let err = ensure_blog_slug_unique_in_tx(&txn, Uuid::new_v4(), "   ").await;
    assert!(matches!(err, Err(BlogError::Validation(_))));

    let res = ensure_blog_slug_unique_in_tx(&txn, Uuid::new_v4(), "fresh-slug").await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_create_promoted_post_validation() {
    let db = setup_schema().await;
    let txn = db.begin().await.unwrap();

    let valid_input = CreatePromotedPostInput {
        post_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        category_id: None,
        status: "published".to_string(),
        slug: "valid-post".to_string(),
        metadata: serde_json::json!({}),
        comment_count: 0,
        created_at: Utc::now(),
        published_at: Some(Utc::now()),
        archived_at: None,
        translations: vec![PromotedPostTranslationInput {
            locale: "en".to_string(),
            title: "Post Title".to_string(),
            body: r#"{"type":"doc","content":[]}"#.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }],
    };

    // Nil tenant
    let mut input = valid_input.clone();
    input.tenant_id = Uuid::nil();
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));

    // Nil post_id
    let mut input = valid_input.clone();
    input.post_id = Uuid::nil();
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));

    // Nil author_id
    let mut input = valid_input.clone();
    input.author_id = Uuid::nil();
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));

    // Empty slug
    let mut input = valid_input.clone();
    input.slug = "  ".to_string();
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));

    // Nil category_id
    let mut input = valid_input.clone();
    input.category_id = Some(Uuid::nil());
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));

    // Empty translations
    let mut input = valid_input.clone();
    input.translations = vec![];
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));

    // Translation with empty title
    let mut input = valid_input.clone();
    input.translations[0].title = " ".to_string();
    assert!(matches!(
        create_promoted_post_in_tx(&txn, input).await,
        Err(BlogError::Validation(_))
    ));
}

#[tokio::test]
async fn test_orchestration_promote_and_demote_lifecycle() {
    let db = setup_schema().await;
    let txn = db.begin().await.unwrap();

    let tenant_id = Uuid::new_v4();
    let post_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let now = Utc::now();

    let input = CreatePromotedPostInput {
        post_id,
        tenant_id,
        author_id,
        category_id: None,
        status: "published".to_string(),
        slug: "promoted-slug".to_string(),
        metadata: serde_json::json!({"source": "forum"}),
        comment_count: 5,
        created_at: now,
        published_at: Some(now),
        archived_at: None,
        translations: vec![
            PromotedPostTranslationInput {
                locale: "en".to_string(),
                title: "Promoted Title EN".to_string(),
                body: r#"{"type":"doc","content":[]}"#.to_string(),
                created_at: now,
                updated_at: now,
            },
            PromotedPostTranslationInput {
                locale: "ru".to_string(),
                title: "Promoted Title RU".to_string(),
                body: r#"{"type":"doc","content":[]}"#.to_string(),
                created_at: now,
                updated_at: now,
            },
        ],
    };

    // Promote
    create_promoted_post_in_tx(&txn, input).await.unwrap();

    // Verify slug uniqueness catches duplicate
    let dup_res = ensure_blog_slug_unique_in_tx(&txn, tenant_id, "promoted-slug").await;
    assert!(matches!(dup_res, Err(BlogError::DuplicateSlug { .. })));

    // Load for demotion
    let demote_record = load_post_for_demotion_in_tx(&txn, tenant_id, post_id)
        .await
        .unwrap();
    assert_eq!(demote_record.id, post_id);
    assert_eq!(demote_record.slug, "promoted-slug");
    assert_eq!(demote_record.translations.len(), 2);

    // Delete demoted post
    delete_demoted_post_in_tx(&txn, tenant_id, post_id)
        .await
        .unwrap();

    // Verify post and translations are gone
    let missing_res = load_post_for_demotion_in_tx(&txn, tenant_id, post_id).await;
    assert!(matches!(missing_res, Err(BlogError::PostNotFound(_))));

    let post_row = blog_post::Entity::find_by_id(post_id).one(&txn).await.unwrap();
    assert!(post_row.is_none());

    let trans_rows = blog_post_translation::Entity::find()
        .filter(blog_post_translation::Column::PostId.eq(post_id))
        .all(&txn)
        .await
        .unwrap();
    assert!(trans_rows.is_empty());
}
