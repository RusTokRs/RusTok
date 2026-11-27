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
    CreatePageInput, PageBodyInput, PageBodyRevisionSource, PageBodyState, PageService,
    PageTranslationInput, PagesError, PagesModule, RestorePageBodyRevisionInput,
    SavePageDocumentInput,
};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, Set,
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
async fn save_and_restore_revision_tokens_advance_after_clock_rollback() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, _) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    // Simulate an existing write committed by a host whose clock was ahead.
    let ahead = Utc::now() + chrono::Duration::minutes(5);
    let mut active: page_body::ActiveModel = current_body(&db, tenant_id, page_id).await?.into();
    active.updated_at = Set(ahead.into());
    active.update(&db).await?;
    let initial = current_body(&db, tenant_id, page_id)
        .await?
        .updated_at
        .to_string();

    let saved = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: initial.clone(),
                body: builder_body("future-v2"),
            },
        )
        .await?;
    let saved = saved.body.expect("saved draft");
    assert!(saved.updated_at > initial, "save must advance the token");
    assert_eq!(
        draft_body(&db, tenant_id, page_id)
            .await?
            .updated_at
            .to_string(),
        saved.updated_at
    );

    let journal = revisions(&db, tenant_id, page_id).await?;
    let restored = service
        .restore_body_revision(
            tenant_id,
            security.clone(),
            page_id,
            RestorePageBodyRevisionInput {
                expected_revision: saved.updated_at.clone(),
                revision_id: journal[0].id,
            },
        )
        .await?;
    let restored = restored.body.expect("restored draft");
    assert!(
        restored.updated_at > saved.updated_at,
        "restore must advance the token"
    );
    assert_eq!(
        draft_body(&db, tenant_id, page_id)
            .await?
            .updated_at
            .to_string(),
        restored.updated_at
    );

    let error = service
        .save_document(
            tenant_id,
            security,
            page_id,
            SavePageDocumentInput {
                expected_revision: saved.updated_at,
                body: builder_body("stale"),
            },
        )
        .await
        .expect_err("stale revision must conflict");
    assert!(
        error
            .to_string()
            .contains("PAGE_DOCUMENT_REVISION_CONFLICT")
    );
    Ok(())
}

#[tokio::test]
async fn save_receipts_replay_the_original_response_after_a_newer_save() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let user = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let other_user = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, revision) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());
    let first_request = SavePageDocumentInput {
        expected_revision: revision.clone(),
        body: builder_body("receipt-first"),
    };
    let first = service
        .save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            first_request.clone(),
            Some("fly:op-1".to_string()),
        )
        .await?;
    let original = serde_json::to_value(&first)?;
    let first_token = first.body.expect("first saved body").updated_at;
    let replay = service
        .save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            first_request.clone(),
            Some("fly:op-1".to_string()),
        )
        .await?;
    assert_eq!(serde_json::to_value(&replay)?, original);
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 1);

    service
        .save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: first_token,
                body: builder_body("receipt-later"),
            },
            Some("fly:op-2".to_string()),
        )
        .await?;
    let replay = service
        .save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            first_request.clone(),
            Some("fly:op-1".to_string()),
        )
        .await?;
    assert_eq!(serde_json::to_value(&replay)?, original);
    let receipt = service
        .document_save_receipt(tenant_id, user.clone(), page_id, "fly:op-1")
        .await?
        .expect("durable acknowledgement");
    assert_eq!(serde_json::to_value(&receipt)?, original);
    assert_eq!(
        draft_body(&db, tenant_id, page_id).await?.content,
        builder_content("receipt-later")
    );
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 2);

    let changed_payload = service
        .save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: revision,
                body: builder_body("different"),
            },
            Some("fly:op-1".to_string()),
        )
        .await
        .expect_err("key reuse for another payload");
    assert!(
        changed_payload
            .to_string()
            .contains("PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT")
    );
    let changed_actor = service
        .save_document_with_receipt(
            tenant_id,
            other_user,
            page_id,
            first_request,
            Some("fly:op-1".to_string()),
        )
        .await
        .expect_err("key reuse for another user");
    assert!(
        changed_actor
            .to_string()
            .contains("PAGE_DOCUMENT_SAVE_IDEMPOTENCY_CONFLICT")
    );
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 2);
    assert!(
        service
            .document_save_receipt(tenant_id, user, page_id, "fly:missing",)
            .await?
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_identical_saves_on_two_sqlite_connections_have_one_receipt_and_journal()
-> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let url = format!(
        "sqlite:file:pages_concurrent_receipts_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let primary = setup_db_at_url(url.clone()).await?;
    // Independent pools and connections: a clone of a single max(1) pool
    // would serialize the futures and hide inter-connection lock behavior.
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let secondary = Database::connect(options).await?;
    let (page_id, revision) = seed_published_page(&primary, tenant_id).await?;
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let a = PageService::new(primary.clone(), mock_transactional_event_bus());
    let b = PageService::new(secondary.clone(), mock_transactional_event_bus());
    let request = SavePageDocumentInput {
        expected_revision: revision,
        body: builder_body("one-request-two-sockets"),
    };

    let (left, right) = tokio::join!(
        a.save_document_with_receipt(
            tenant_id,
            security.clone(),
            page_id,
            request.clone(),
            Some("two-sockets-identical".to_string()),
        ),
        b.save_document_with_receipt(
            tenant_id,
            security.clone(),
            page_id,
            request.clone(),
            Some("two-sockets-identical".to_string()),
        )
    );
    // SQLite may report SQLITE_BUSY/SQLITE_LOCKED for one of the racing
    // transactions. A retry with the same identity must converge to the
    // single committed acknowledgement without appending another journal.
    let canonical = a
        .save_document_with_receipt(
            tenant_id,
            security,
            page_id,
            request,
            Some("two-sockets-identical".to_string()),
        )
        .await?;
    let canonical_json = serde_json::to_value(&canonical)?;
    for accepted in [left, right].into_iter().flatten() {
        assert_eq!(serde_json::to_value(accepted)?, canonical_json);
    }
    assert_eq!(revisions(&primary, tenant_id, page_id).await?.len(), 1);
    assert_eq!(
        draft_body(&secondary, tenant_id, page_id).await?.content,
        builder_content("one-request-two-sockets")
    );
    Ok(())
}

#[tokio::test]
async fn competing_distinct_saves_never_both_advance_same_sqlite_revision() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let url = format!(
        "sqlite:file:pages_concurrent_distinct_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let primary = setup_db_at_url(url.clone()).await?;
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let secondary = Database::connect(options).await?;
    let (page_id, revision) = seed_published_page(&primary, tenant_id).await?;
    let user = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let left_service = PageService::new(primary.clone(), mock_transactional_event_bus());
    let right_service = PageService::new(secondary.clone(), mock_transactional_event_bus());
    let left_request = SavePageDocumentInput {
        expected_revision: revision.clone(),
        body: builder_body("distinct-left"),
    };
    let right_request = SavePageDocumentInput {
        expected_revision: revision,
        body: builder_body("distinct-right"),
    };
    let (left, right) = tokio::join!(
        left_service.save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            left_request.clone(),
            Some("two-sockets-left".to_string()),
        ),
        right_service.save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            right_request.clone(),
            Some("two-sockets-right".to_string()),
        )
    );
    assert!(
        !(left.is_ok() && right.is_ok()),
        "two different commands cannot accept one observed revision"
    );
    // A racing SQLite writer can fail busy instead of receiving a typed CAS
    // conflict. Complete one operation if neither succeeded, then retry the
    // other sequentially: stale expected revision must never be accepted.
    if left.is_err() && right.is_err() {
        left_service
            .save_document_with_receipt(
                tenant_id,
                user.clone(),
                page_id,
                left_request.clone(),
                Some("two-sockets-left".to_string()),
            )
            .await?;
    }
    let final_body = draft_body(&primary, tenant_id, page_id).await?;
    let latest = final_body.content.clone();
    assert!(
        latest == builder_content("distinct-left") || latest == builder_content("distinct-right")
    );
    assert_eq!(revisions(&primary, tenant_id, page_id).await?.len(), 1);
    let (loser_request, loser_key) = if latest == builder_content("distinct-left") {
        (right_request, "two-sockets-right")
    } else {
        (left_request, "two-sockets-left")
    };
    let losing_retry = right_service
        .save_document_with_receipt(
            tenant_id,
            user,
            page_id,
            loser_request,
            Some(loser_key.to_string()),
        )
        .await
        .expect_err("losing stale command must not commit");
    assert!(
        losing_retry
            .to_string()
            .contains("PAGE_DOCUMENT_REVISION_CONFLICT"),
        "retry after the winner has committed must fail CAS: {losing_retry}"
    );
    assert_eq!(revisions(&secondary, tenant_id, page_id).await?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn older_receipts_remain_tombstones_after_full_response_expiration() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let user = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, mut revision) = seed_published_page(&db, tenant_id).await?;
    let first = revision.clone();
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    for index in 0..65 {
        let response = service
            .save_document_with_receipt(
                tenant_id,
                user.clone(),
                page_id,
                SavePageDocumentInput {
                    expected_revision: revision,
                    body: builder_body(&format!("revision-{index}")),
                },
                Some(format!("fly:receipt-{index}")),
            )
            .await?;
        revision = response.body.expect("saved body").updated_at;
    }
    let revisions_before = revisions(&db, tenant_id, page_id).await?.len();
    let error = service
        .save_document_with_receipt(
            tenant_id,
            user.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: first,
                body: builder_body("revision-0"),
            },
            Some("fly:receipt-0".to_string()),
        )
        .await
        .expect_err("expired acknowledgement must not rerun save");
    assert!(
        error
            .to_string()
            .contains("PAGE_DOCUMENT_SAVE_RECEIPT_EXPIRED")
    );
    assert_eq!(
        draft_body(&db, tenant_id, page_id)
            .await?
            .updated_at
            .to_string(),
        revision
    );
    assert_eq!(
        revisions(&db, tenant_id, page_id).await?.len(),
        revisions_before
    );
    let recent = service
        .document_save_receipt(tenant_id, user, page_id, "fly:receipt-64")
        .await?
        .expect("recent acknowledgement remains replayable");
    assert_eq!(recent.body.expect("body").updated_at, revision);
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

    let restore_input = RestorePageBodyRevisionInput {
        expected_revision: v3_token,
        revision_id: v2_revision_id,
    };
    let restored = service
        .restore_body_revision(tenant_id, security.clone(), page_id, restore_input.clone())
        .await?;
    let body = restored.body.expect("restored body");
    assert!(matches!(body.state, PageBodyState::Draft));
    assert_eq!(body.content, builder_content("v2"));

    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].source, "restore");
    assert_eq!(rows[0].content, builder_content("v2"));
    assert_eq!(rows[0].body_revision, body.updated_at);

    // Advance the working copy after the restore committed. A retry of the original
    // restore must replay its receipt before CAS instead of restoring a second time.
    let later_save = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: body.updated_at.clone(),
                body: builder_body("v4"),
            },
        )
        .await?;
    let later_revision = later_save.body.expect("later draft body").updated_at;
    let count_before_replay = revisions(&db, tenant_id, page_id).await?.len();
    assert_eq!(count_before_replay, 4);

    let replayed = service
        .restore_body_revision(tenant_id, security, page_id, restore_input)
        .await?;
    let replayed_body = replayed.body.expect("replayed restore body");
    assert_eq!(replayed_body.updated_at, body.updated_at);
    assert_eq!(replayed_body.content, builder_content("v2"));
    assert_ne!(replayed_body.updated_at, later_revision);
    assert_eq!(
        revisions(&db, tenant_id, page_id).await?.len(),
        count_before_replay,
        "replaying a committed restore must not append another journal row"
    );
    Ok(())
}

#[tokio::test]
async fn sqlite_duplicate_restores_are_serialized_across_independent_pools() -> TestResult<()> {
    // A file-backed database is required here: two pools must own independent SQLite
    // connections while contending for the same page write reservation.
    let database_path =
        std::env::temp_dir().join(format!("pages_restore_receipt_{}.sqlite", Uuid::new_v4()));
    let database_url = format!("sqlite:file:{}?mode=rwc", database_path.display());
    let db = setup_db_at_url(database_url.clone()).await?;
    let mut peer_options = ConnectOptions::new(database_url);
    peer_options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let peer_db = Database::connect(peer_options).await?;

    let tenant_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let (page_id, base_revision) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());
    let peer_service = PageService::new(peer_db.clone(), mock_transactional_event_bus());

    let v2 = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: base_revision,
                body: builder_body("v2"),
            },
        )
        .await?;
    let v2_revision = v2.body.expect("v2 body").updated_at;
    let v3 = service
        .save_document(
            tenant_id,
            security.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: v2_revision,
                body: builder_body("v3"),
            },
        )
        .await?;
    let v3_revision = v3.body.expect("v3 body").updated_at;
    let history = service
        .body_revision_history(tenant_id, security.clone(), page_id, "en")
        .await?;
    let restore = RestorePageBodyRevisionInput {
        expected_revision: v3_revision,
        revision_id: history[1].id,
    };

    let (left, right) = tokio::join!(
        service.restore_body_revision(tenant_id, security.clone(), page_id, restore.clone()),
        peer_service.restore_body_revision(tenant_id, security, page_id, restore),
    );
    let first = left?;
    let second = right?;
    assert_eq!(serde_json::to_value(first)?, serde_json::to_value(second)?);
    assert_eq!(
        revisions(&db, tenant_id, page_id).await?.len(),
        3,
        "independent SQLite writers must commit one restore and one receipt"
    );

    drop(service);
    drop(peer_service);
    drop(peer_db);
    drop(db);
    std::fs::remove_file(&database_path)?;
    Ok(())
}

#[tokio::test]
async fn page_create_receipt_replays_the_original_result_after_later_writes() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let service = PageService::new(db.clone(), mock_transactional_event_bus());
    let input = CreatePageInput {
        translations: vec![PageTranslationInput {
            locale: "en".to_string(),
            title: "Receipt-backed page".to_string(),
            slug: Some("receipt-backed-page".to_string()),
            meta_title: None,
            meta_description: None,
        }],
        template: Some("default".to_string()),
        body: Some(builder_body("initial")),
        channel_slugs: None,
        publish: false,
    };

    let created = service
        .create_with_receipt(
            tenant_id,
            security.clone(),
            input.clone(),
            true,
            "pages-create-receipt-1".to_string(),
        )
        .await?;
    let created_body = created.body.expect("initial body");
    assert_eq!(created.version, 1);
    assert_eq!(created_body.content, builder_content("initial"));

    let saved = service
        .save_document(
            tenant_id,
            security.clone(),
            created.id,
            SavePageDocumentInput {
                expected_revision: created_body.updated_at,
                body: builder_body("later"),
            },
        )
        .await?;
    assert_eq!(
        saved.body.expect("later body").content,
        builder_content("later")
    );

    let replayed = service
        .create_with_receipt(
            tenant_id,
            security.clone(),
            input.clone(),
            true,
            "pages-create-receipt-1".to_string(),
        )
        .await?;
    assert_eq!(replayed.id, created.id);
    assert_eq!(replayed.version, 1);
    assert_eq!(
        replayed.body.expect("original receipt body").content,
        builder_content("initial"),
        "a retry returns the original acknowledgement, not a later page snapshot"
    );
    assert_eq!(
        page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await?
            .len(),
        1,
        "a retry must not create a second page"
    );

    let mut changed_input = input;
    changed_input.translations[0].title = "Different request".to_string();
    let error = service
        .create_with_receipt(
            tenant_id,
            security,
            changed_input,
            true,
            "pages-create-receipt-1".to_string(),
        )
        .await
        .expect_err("a key cannot be reused for another create request");
    assert!(error.to_string().contains("idempotency key"));
    Ok(())
}

#[tokio::test]
async fn concurrent_page_create_replays_across_independent_sqlite_pools() -> TestResult<()> {
    let database_path =
        std::env::temp_dir().join(format!("pages_create_receipt_{}.sqlite", Uuid::new_v4()));
    let database_url = format!("sqlite:file:{}?mode=rwc", database_path.display());
    let db = setup_db_at_url(database_url.clone()).await?;
    let mut peer_options = ConnectOptions::new(database_url);
    peer_options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let peer_db = Database::connect(peer_options).await?;

    let tenant_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let service = PageService::new(db.clone(), mock_transactional_event_bus());
    let peer_service = PageService::new(peer_db.clone(), mock_transactional_event_bus());
    let input = CreatePageInput {
        translations: vec![PageTranslationInput {
            locale: "en".to_string(),
            title: "Concurrent create".to_string(),
            slug: Some("concurrent-create".to_string()),
            meta_title: None,
            meta_description: None,
        }],
        template: Some("default".to_string()),
        body: Some(builder_body("initial")),
        channel_slugs: None,
        publish: false,
    };

    let (left, right) = tokio::join!(
        service.create_with_receipt(
            tenant_id,
            security.clone(),
            input.clone(),
            true,
            "pages-create-concurrent-1".to_string(),
        ),
        peer_service.create_with_receipt(
            tenant_id,
            security,
            input,
            true,
            "pages-create-concurrent-1".to_string(),
        ),
    );
    let first = left?;
    let second = right?;
    assert_eq!(first.id, second.id);
    assert_eq!(first.version, second.version);
    assert_eq!(
        page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .all(&db)
            .await?
            .len(),
        1,
        "independent SQLite writers must create one page for one key"
    );

    drop(service);
    drop(peer_service);
    drop(peer_db);
    drop(db);
    std::fs::remove_file(&database_path)?;
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
async fn page_source_follows_save_and_publish_authority() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let actor_id = Uuid::new_v4();
    let editor = SecurityContext::new(UserRole::Admin, Some(actor_id));
    let (page_id, body_token) = seed_published_page(&db, tenant_id).await?;
    let service = PageService::new(db.clone(), mock_transactional_event_bus());

    service
        .save_document(
            tenant_id,
            editor.clone(),
            page_id,
            SavePageDocumentInput {
                expected_revision: body_token,
                body: builder_body("v2"),
            },
        )
        .await?;

    // The editor reads the working copy by id and the current body through the public slug.
    let seen_by_editor = service
        .get_with_locale_fallback(tenant_id, editor.clone(), page_id, "en", None)
        .await?
        .body
        .expect("editor body");
    assert!(matches!(seen_by_editor.state, PageBodyState::Draft));
    let slug_body_for_editor = service
        .get_by_slug_with_locale_fallback(tenant_id, editor, "en", "home", None)
        .await?
        .expect("published page resolves for the editor")
        .body
        .expect("editor slug body");
    assert!(matches!(slug_body_for_editor.state, PageBodyState::Current));
    assert_eq!(slug_body_for_editor.content, builder_content("v1"));

    // The body is the authoring document. Read authority alone resolves the page but never
    // returns its source: neither the current body nor the draft.
    let reader = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [Permission::PAGES_READ],
    );
    let seen_by_reader = service
        .get_with_locale_fallback(tenant_id, reader.clone(), page_id, "en", None)
        .await?;
    assert_eq!(seen_by_reader.id, page_id);
    assert!(seen_by_reader.translation.is_some());
    assert!(seen_by_reader.body.is_none());
    // The response says that the document is withheld, not that the page has none.
    assert!(!seen_by_reader.document_readable);
    assert!(matches!(
        service
            .body_revision_history(tenant_id, reader, page_id, "en")
            .await,
        Err(PagesError::Forbidden(_))
    ));

    // A publish folds the working copy into the current body, so publish authority reads the
    // working copy too.
    let publisher = SecurityContext::from_permissions(
        UserRole::Manager,
        Some(Uuid::new_v4()),
        [
            Permission::PAGES_READ,
            Permission::new(rustok_api::Resource::Pages, rustok_api::Action::Publish),
        ],
    );
    let seen_by_publisher = service
        .get_with_locale_fallback(tenant_id, publisher.clone(), page_id, "en", None)
        .await?;
    assert!(seen_by_publisher.document_readable);
    let seen_by_publisher = seen_by_publisher.body.expect("publisher body");
    assert!(matches!(seen_by_publisher.state, PageBodyState::Draft));
    // The revision journal describes the document and follows the same rule.
    assert!(
        !service
            .body_revision_history(tenant_id, publisher, page_id, "en")
            .await?
            .is_empty()
    );

    // A non-staff principal holding `pages:update` may save, and therefore read, only the
    // documents of its own pages. This page is not its own.
    let own_pages_editor = SecurityContext::from_permissions(
        UserRole::Customer,
        Some(Uuid::new_v4()),
        [Permission::PAGES_READ, Permission::PAGES_UPDATE],
    );
    let seen_by_own_pages_editor = service
        .get_with_locale_fallback(tenant_id, own_pages_editor, page_id, "en", None)
        .await?;
    assert_eq!(seen_by_own_pages_editor.id, page_id);
    assert!(seen_by_own_pages_editor.body.is_none());
    assert!(!seen_by_own_pages_editor.document_readable);

    let public_by_id = service
        .get_with_locale_fallback(
            tenant_id,
            SecurityContext::public_read(),
            page_id,
            "en",
            None,
        )
        .await?;
    assert_eq!(public_by_id.id, page_id);
    assert!(public_by_id.body.is_none());
    assert!(!public_by_id.document_readable);

    let public_by_slug = service
        .get_by_slug_with_locale_fallback(
            tenant_id,
            SecurityContext::public_read(),
            "en",
            "home",
            None,
        )
        .await?
        .expect("published page resolves for public readers");
    assert_eq!(public_by_slug.id, page_id);
    assert_eq!(public_by_slug.effective_locale.as_deref(), Some("en"));
    assert!(public_by_slug.translation.is_some());
    assert!(public_by_slug.body.is_none());
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
    setup_db_at_url(format!(
        "sqlite:file:pages_body_draft_history_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ))
    .await
}

async fn setup_db_at_url(database_url: String) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            settings TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await?;
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
