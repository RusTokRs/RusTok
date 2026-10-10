//! Runtime coverage for the page body draft working copy and the append-only body
//! revision journal (`page_body_drafts`, `page_body_revisions`).
//!
//! Published pages stay online while editors save: saves land in the page body draft,
//! the current body keeps serving, and every accepted mutation appends exactly one
//! journal row. Promotion (unpublish/archive/publish) folds the draft into the current
//! body while preserving its revision token, and restore brings a journaled revision
//! back into the working copy under CAS.

use std::error::Error;

use chrono::Utc;
use rustok_api::Permission;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_pages::entities::{
    page, page_body, page_body_draft, page_body_revision, page_translation,
};
use rustok_pages::{
    PageBodyInput, PageBodyRevisionSource, PageBodyState, PageService, PagesModule,
    RestorePageBodyRevisionInput, SavePageDocumentInput,
};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, Set,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const BUILDER_DOCUMENT_TITLE: &str = "Draft and history";

#[tokio::test]
async fn published_page_saves_land_in_the_body_draft_with_one_journal_row_each() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());
    let page_token_before = page_model(&db, page_id).await?.updated_at.to_string();

    let saved = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: body_token.clone(),
                body: builder_body("v2"),
            },
        )
        .await?;
    let body = saved.body.expect("saved body");
    assert!(matches!(body.state, PageBodyState::Draft));
    assert_ne!(body.updated_at, body_token, "the draft gets its own token");

    let current = current_body(&db, tenant_id, page_id).await?;
    assert_eq!(
        current.content,
        builder_content("v1"),
        "public body untouched"
    );
    assert_eq!(current.updated_at.to_string(), body_token);

    let draft = draft_body(&db, tenant_id, page_id).await?;
    assert_eq!(draft.content, builder_content("v2"));
    assert_eq!(draft.updated_at.to_string(), body.updated_at);
    assert_eq!(draft.created_by, Some(actor_id));

    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 1, "one accepted save appends exactly one row");
    assert_eq!(rows[0].source, "draft_save");
    assert_eq!(rows[0].body_revision, body.updated_at);
    assert_eq!(rows[0].content, builder_content("v2"));
    assert_eq!(rows[0].created_by, Some(actor_id));

    let page = page_model(&db, page_id).await?;
    assert_eq!(
        page.updated_at.to_string(),
        page_token_before,
        "draft saves must not bump pages.updated_at"
    );
    Ok(())
}

#[tokio::test]
async fn rejected_saves_append_nothing_and_keep_the_working_copy() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    let error = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: "page:stale:initial".to_string(),
                body: builder_body("v2"),
            },
        )
        .await
        .expect_err("a stale expected revision must conflict");
    let message = error.to_string();
    assert!(
        message.contains("PAGE_DOCUMENT_REVISION_CONFLICT"),
        "unexpected error: {message}"
    );

    assert!(draft_lookup(&db, tenant_id, page_id).await?.is_none());
    assert!(revisions(&db, tenant_id, page_id).await?.is_empty());
    let current = current_body(&db, tenant_id, page_id).await?;
    assert_eq!(current.updated_at.to_string(), body_token);
    Ok(())
}

#[tokio::test]
async fn unpublish_promotes_the_draft_and_preserves_its_revision_token() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    let saved = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: body_token,
                body: builder_body("v2"),
            },
        )
        .await?;
    let draft_token = saved.body.expect("draft body").updated_at;

    let unpublished = service
        .unpublish(tenant_id, security.clone(), page_id)
        .await?;
    assert_eq!(
        unpublished.status,
        rustok_content::entities::node::ContentStatus::Draft
    );

    let current = current_body(&db, tenant_id, page_id).await?;
    assert_eq!(current.content, builder_content("v2"), "draft folded in");
    assert_eq!(
        current.updated_at.to_string(),
        draft_token,
        "promotion preserves the working-copy token"
    );
    assert!(draft_lookup(&db, tenant_id, page_id).await?.is_none());

    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source, "promote");
    assert_eq!(rows[0].body_revision, draft_token);

    // The preserved token stays a valid CAS handle for the next save.
    service
        .save_document(
            tenant_id,
            security,
            page_id,
            SavePageDocumentInput {
                expected_revision: draft_token,
                body: builder_body("v3"),
            },
        )
        .await?;
    Ok(())
}

#[tokio::test]
async fn restore_places_a_journaled_revision_back_into_the_draft() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    let v2 = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: body_token,
                body: builder_body("v2"),
            },
        )
        .await?;
    let v2_token = v2.body.expect("v2 body").updated_at;
    let v3 = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: v2_token.clone(),
                body: builder_body("v3"),
            },
        )
        .await?;
    let v3_token = v3.body.expect("v3 body").updated_at;

    let history = service
        .body_revision_history(tenant_id, security.clone(), page_id, "en")
        .await?;
    assert_eq!(history.len(), 2, "newest first");
    assert_eq!(history[0].body_revision, v3_token);
    assert_eq!(history[0].source, PageBodyRevisionSource::DraftSave);
    assert_eq!(history[1].body_revision, v2_token);
    let v2_revision_id = history[1].id;

    // A stale restore handle conflicts before anything is written.
    let stale = service
        .restore_body_revision(
            tenant_id,
            security.clone(),
            page_id,
            RestorePageBodyRevisionInput {
                expected_revision: v2_token,
                revision_id: v2_revision_id,
            },
        )
        .await
        .expect_err("stale expected revision must conflict");
    assert!(
        stale
            .to_string()
            .contains("PAGE_DOCUMENT_REVISION_CONFLICT")
    );

    let restored = service
        .restore_body_revision(
            tenant_id,
            security.clone(),
            page_id,
            RestorePageBodyRevisionInput {
                expected_revision: v3_token,
                revision_id: v2_revision_id,
            },
        )
        .await?;
    let body = restored.body.expect("restored body");
    assert!(matches!(body.state, PageBodyState::Draft));
    assert_eq!(body.content, builder_content("v2"));

    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].source, "restore");
    assert_eq!(rows[0].content, builder_content("v2"));
    assert_eq!(rows[0].body_revision, body.updated_at);
    Ok(())
}

#[tokio::test]
async fn duplicate_copies_current_bodies_only_and_journals_each_copy() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: body_token,
                body: builder_body("v2"),
            },
        )
        .await?;

    let copy = service
        .duplicate_page(tenant_id, security.clone(), page_id)
        .await?;
    assert_eq!(
        copy.status,
        rustok_content::entities::node::ContentStatus::Draft
    );
    let copy_page = page_model(&db, copy.id).await?;
    assert_eq!(
        copy_page.author_id,
        Some(actor_id),
        "the copy is owned by the actor"
    );
    assert_eq!(copy_page.status, "draft");
    let copy_body = copy.body.expect("copy body");
    assert_eq!(
        copy_body.content,
        builder_content("v1"),
        "current body copied, draft ignored"
    );
    assert!(matches!(copy_body.state, PageBodyState::Current));
    let copy_slug = copy
        .translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .expect("copy slug");
    assert_eq!(copy_slug, "home-copy");

    let copy_rows = revisions(&db, tenant_id, copy.id).await?;
    assert_eq!(copy_rows.len(), 1);
    assert_eq!(copy_rows[0].source, "duplicate");
    assert_eq!(copy_rows[0].created_by, Some(actor_id));

    let second_copy = service.duplicate_page(tenant_id, security, page_id).await?;
    let second_slug = second_copy
        .translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .expect("second copy slug");
    assert_eq!(second_slug, "home-copy-2");
    Ok(())
}

#[tokio::test]
async fn history_retention_keeps_the_newest_window_per_locale() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, mut expected) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    for step in 0..55 {
        let saved = service
            .save_document(
                tenant_id,
                security.clone(),
                page_id,
                SavePageDocumentInput {
                    expected_revision: expected.clone(),
                    body: builder_body(&format!("v{step}")),
                },
            )
            .await?;
        expected = saved.body.expect("draft body").updated_at;
    }

    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(
        rows.len(),
        50,
        "retention window is exactly 50 per (page, locale)"
    );
    let history = service
        .body_revision_history(tenant_id, security, page_id, "en")
        .await?;
    assert_eq!(history.len(), 50);
    assert_eq!(history[0].body_revision, expected);
    Ok(())
}

#[tokio::test]
async fn drafts_are_invisible_to_read_only_readers() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let actor_id = Uuid::new_v4();
    let editor = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    service
        .save_document(
            tenant_id,
            editor,
            page_id,
            SavePageDocumentInput {
                expected_revision: body_token,
                body: builder_body("v2"),
            },
        )
        .await?;

    let reader = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [Permission::PAGES_READ],
    );
    let seen_by_reader = service
        .get_with_locale_fallback(tenant_id, reader, page_id, "en", None)
        .await?
        .body
        .expect("reader body");
    assert!(matches!(seen_by_reader.state, PageBodyState::Current));
    assert_eq!(seen_by_reader.content, builder_content("v1"));

    let customer = SecurityContext::from_permissions(
        UserRole::Customer,
        Some(Uuid::new_v4()),
        [Permission::PAGES_READ, Permission::PAGES_UPDATE],
    );
    let seen_by_customer = service
        .get_with_locale_fallback(tenant_id, customer, page_id, "en", None)
        .await?
        .body
        .expect("customer body");
    assert!(matches!(seen_by_customer.state, PageBodyState::Current));
    Ok(())
}

fn builder_body(title: &str) -> PageBodyInput {
    PageBodyInput {
        locale: "en".to_string(),
        document: builder_document(title),
    }
}

fn builder_content(title: &str) -> String {
    builder_document(title).to_string()
}

fn builder_document(title: &str) -> serde_json::Value {
    json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": format!("{BUILDER_DOCUMENT_TITLE} {title}"),
                "description": "Body draft and revision history",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": title
                }]
            }
        }]
    })
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_body_draft_history_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn seed_published_page(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<(Uuid, String)> {
    let page_id = Uuid::new_v4();
    // Microsecond precision keeps the seed token equal to its stored round-trip on every backend.
    let now: sea_orm::prelude::DateTimeWithTimeZone =
        chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
            .expect("current timestamp")
            .into();
    page::ActiveModel {
        id: Set(page_id),
        tenant_id: Set(tenant_id),
        author_id: Set(None),
        status: Set("published".to_string()),
        template: Set("default".to_string()),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(Some(now)),
        archived_at: Set(None),
        version: Set(1),
    }
    .insert(db)
    .await?;
    page_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        page_id: Set(page_id),
        tenant_id: Set(tenant_id),
        locale: Set("en".to_string()),
        title: Set(BUILDER_DOCUMENT_TITLE.to_string()),
        slug: Set("home".to_string()),
        meta_title: Set(None),
        meta_description: Set(None),
        revision: Set(1),
    }
    .insert(db)
    .await?;
    page_body::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set("en".to_string()),
        content: Set(builder_content("v1")),
        format: Set(rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT.to_string()),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;
    Ok((page_id, now.to_string()))
}

async fn page_model(db: &DatabaseConnection, page_id: Uuid) -> TestResult<page::Model> {
    Ok(page::Entity::find_by_id(page_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("page row is missing"))?)
}

async fn current_body(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<page_body::Model> {
    Ok(page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(page_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("current body is missing"))?)
}

async fn draft_lookup(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<Option<page_body_draft::Model>> {
    Ok(page_body_draft::Entity::find()
        .filter(page_body_draft::Column::TenantId.eq(tenant_id))
        .filter(page_body_draft::Column::PageId.eq(page_id))
        .one(db)
        .await?)
}

async fn draft_body(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<page_body_draft::Model> {
    draft_lookup(db, tenant_id, page_id)
        .await?
        .ok_or_else(|| std::io::Error::other("draft body is missing").into())
}

async fn revisions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<Vec<page_body_revision::Model>> {
    Ok(page_body_revision::Entity::find()
        .filter(page_body_revision::Column::TenantId.eq(tenant_id))
        .filter(page_body_revision::Column::PageId.eq(page_id))
        .order_by_desc(page_body_revision::Column::CreatedAt)
        .order_by_desc(page_body_revision::Column::Id)
        .all(db)
        .await?)
}
