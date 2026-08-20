mod support;

use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, ForumModule,
    services::ForumCategoryTranslationTargetProvider,
};
use rustok_translation_targets::{
    FieldKey, ReadTranslationResourceRequest, TranslationFieldPatch, TranslationPatchRequest,
    TranslationResourceLifecycle, TranslationResourceSnapshot, TranslationTargetChangesRequest,
    TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::Barrier;
use uuid::Uuid;

use support::{TestResult, postgres::PostgresForumTestDb, test_error};

const CHANGE_JOURNAL_MIGRATION: &str =
    "m20260820_000028_add_forum_category_translation_changes";

#[tokio::test]
async fn category_translation_change_journal_supports_postgres_down_up() -> TestResult<()> {
    let Some(test_db) = PostgresForumTestDb::setup("translation_migration").await? else {
        return Ok(());
    };
    let manager = SchemaManager::new(&test_db.db);
    let migration = ForumModule
        .migrations()
        .into_iter()
        .find(|migration| migration.name() == CHANGE_JOURNAL_MIGRATION)
        .ok_or_else(|| test_error("Forum translation change journal migration is not registered"))?;

    migration.down(&manager).await?;
    migration.up(&manager).await?;

    let tenant_id = Uuid::new_v4();
    let category_id = create_category(&test_db.db, tenant_id, "Migration", "migration").await?;
    let provider = ForumCategoryTranslationTargetProvider::new(test_db.db.clone());
    let page = provider
        .read_changes(
            read_context(tenant_id, "forum-translation-migration-cursor"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;

    assert_eq!(page.changes.len(), 1);
    assert_eq!(
        page.changes[0].identity.resource_id.as_str(),
        category_id.to_string()
    );
    assert_eq!(
        page.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert!(page.next_cursor.is_some());

    test_db.cleanup().await
}

#[tokio::test]
async fn concurrent_same_snapshot_translation_applies_commit_once() -> TestResult<()> {
    let Some(test_db) = PostgresForumTestDb::setup("translation_concurrent_cas").await? else {
        return Ok(());
    };
    let tenant_id = Uuid::new_v4();
    let category_id = create_category(&test_db.db, tenant_id, "Systems", "systems").await?;
    let provider = ForumCategoryTranslationTargetProvider::new(test_db.db.clone());
    let snapshot = provider
        .read_resource(
            read_context(tenant_id, "forum-translation-concurrent-snapshot"),
            resource_request(category_id),
        )
        .await?;

    let candidates = [
        ("Systemes alpha", "description-alpha", "alpha"),
        ("Systemes beta", "description-beta", "beta"),
    ];
    let barrier = Arc::new(Barrier::new(candidates.len()));
    let mut tasks = Vec::with_capacity(candidates.len());
    for (name, description, suffix) in candidates {
        let provider = ForumCategoryTranslationTargetProvider::new(test_db.peer().await?);
        let barrier = Arc::clone(&barrier);
        let patch = translation_patch(&snapshot, name, description, suffix);
        let context = apply_context(
            tenant_id,
            &format!("forum-translation-concurrent-{suffix}"),
            &format!("forum-translation-concurrent-idempotency-{suffix}"),
        );
        let expected_name = name.to_string();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            (expected_name, provider.apply_patch(context, patch).await)
        }));
    }

    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for task in tasks {
        let (name, result) = task.await?;
        match result {
            Ok(receipt) => successes.push((name, receipt)),
            Err(error) => failures.push(error),
        }
    }
    assert_eq!(successes.len(), 1, "exactly one same-snapshot apply must commit");
    assert_eq!(failures.len(), 1, "the competing same-snapshot apply must close");
    assert_eq!(failures[0].kind, PortErrorKind::Conflict);

    let recovered = ForumCategoryTranslationTargetProvider::new(test_db.peer().await?);
    let final_snapshot = recovered
        .read_resource(
            read_context(tenant_id, "forum-translation-concurrent-final"),
            resource_request(category_id),
        )
        .await?;
    assert_eq!(
        exact_target_value(&final_snapshot, "name"),
        Some(successes[0].0.as_str())
    );
    assert!(final_snapshot.target_revision.is_some());
    assert_ne!(
        final_snapshot.summary.resource_revision,
        snapshot.summary.resource_revision
    );

    let changes = recovered
        .read_changes(
            read_context(tenant_id, "forum-translation-concurrent-changes"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(changes.changes.len(), 2, "create plus one winning apply only");

    let progress = recovered
        .read_progress(
            read_context(tenant_id, "forum-translation-concurrent-progress"),
            progress_request(),
        )
        .await?;
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.required_units, 1);
    assert_eq!(progress.exact_required_units, 1);
    assert_eq!(progress.optional_units, 1);
    assert_eq!(progress.exact_optional_units, 1);
    assert_eq!(progress.complete_resources, 1);
    assert_eq!(progress.owner_change_cursor, changes.next_cursor);

    test_db.cleanup().await
}

#[tokio::test]
async fn cursor_and_progress_resume_across_reconstruction_archive_and_restore() -> TestResult<()> {
    let Some(test_db) = PostgresForumTestDb::setup("translation_cursor_recovery").await? else {
        return Ok(());
    };
    let tenant_id = Uuid::new_v4();
    let category_id = create_category(&test_db.db, tenant_id, "Recovery", "recovery").await?;
    let first_provider = ForumCategoryTranslationTargetProvider::new(test_db.db.clone());
    let first_page = first_provider
        .read_changes(
            read_context(tenant_id, "forum-translation-cursor-first"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(first_page.changes.len(), 1);
    let first_cursor = first_page
        .next_cursor
        .expect("category create must publish a cursor");
    drop(first_provider);

    let apply_provider = ForumCategoryTranslationTargetProvider::new(test_db.peer().await?);
    let snapshot = apply_provider
        .read_resource(
            read_context(tenant_id, "forum-translation-cursor-snapshot"),
            resource_request(category_id),
        )
        .await?;
    apply_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "forum-translation-cursor-apply",
                "forum-translation-cursor-apply-idempotency",
            ),
            translation_patch(&snapshot, "Recuperation", "Description cible", "recovery"),
        )
        .await?;
    let second_page = apply_provider
        .read_changes(
            read_context(tenant_id, "forum-translation-cursor-second"),
            TranslationTargetChangesRequest {
                after: Some(first_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(second_page.changes.len(), 1);
    assert_eq!(
        second_page.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let second_cursor = second_page
        .next_cursor
        .expect("translation apply must advance the cursor");
    assert_ne!(second_cursor, first_cursor);
    drop(apply_provider);

    CategoryService::new(test_db.peer().await?)
        .delete(tenant_id, category_id, admin())
        .await?;
    let archived_provider = ForumCategoryTranslationTargetProvider::new(test_db.peer().await?);
    let archived_page = archived_provider
        .read_changes(
            read_context(tenant_id, "forum-translation-cursor-archived"),
            TranslationTargetChangesRequest {
                after: Some(second_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(archived_page.changes.len(), 1);
    assert_eq!(
        archived_page.changes[0].lifecycle,
        TranslationResourceLifecycle::Archived
    );
    let archived_cursor = archived_page
        .next_cursor
        .expect("archive must advance the cursor");
    let archived_progress = archived_provider
        .read_progress(
            read_context(tenant_id, "forum-translation-progress-archived"),
            progress_request(),
        )
        .await?;
    assert_eq!(archived_progress.resources, 0);
    assert_eq!(archived_progress.owner_change_cursor, Some(archived_cursor.clone()));
    drop(archived_provider);

    CategoryService::new(test_db.peer().await?)
        .restore_subtree(tenant_id, category_id, admin())
        .await?;
    let restored_provider = ForumCategoryTranslationTargetProvider::new(test_db.peer().await?);
    let restored_page = restored_provider
        .read_changes(
            read_context(tenant_id, "forum-translation-cursor-restored"),
            TranslationTargetChangesRequest {
                after: Some(archived_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(restored_page.changes.len(), 1);
    assert_eq!(
        restored_page.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let restored_cursor = restored_page
        .next_cursor
        .expect("restore must advance the cursor");
    assert_ne!(restored_cursor, archived_cursor);

    let restored_progress = restored_provider
        .read_progress(
            read_context(tenant_id, "forum-translation-progress-restored"),
            progress_request(),
        )
        .await?;
    assert_eq!(restored_progress.resources, 1);
    assert_eq!(restored_progress.exact_required_units, 1);
    assert_eq!(restored_progress.exact_optional_units, 1);
    assert_eq!(restored_progress.complete_resources, 1);
    assert_eq!(restored_progress.owner_change_cursor, Some(restored_cursor));

    test_db.cleanup().await
}

async fn create_category(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
) -> TestResult<Uuid> {
    Ok(CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: name.to_string(),
                slug: slug.to_string(),
                description: Some(format!("{name} description")),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn read_context(tenant_id: Uuid, correlation_id: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        correlation_id,
    )
    .with_deadline(Duration::from_secs(5))
}

fn apply_context(tenant_id: Uuid, correlation_id: &str, idempotency_key: &str) -> PortContext {
    read_context(tenant_id, correlation_id).with_idempotency_key(idempotency_key)
}

fn resource_request(category_id: Uuid) -> ReadTranslationResourceRequest {
    ReadTranslationResourceRequest {
        identity: rustok_translation_targets::TranslationResourceIdentity {
            owner_slug: rustok_translation_targets::OwnerSlug::new("forum")
                .expect("static owner slug"),
            resource_kind: rustok_translation_targets::ResourceKind::new("category")
                .expect("static resource kind"),
            resource_id: rustok_translation_targets::ResourceId::new(category_id.to_string())
                .expect("category UUID resource id"),
            subresource_id: None,
        },
        source_locale: TenantLocale::new("en").expect("static source locale"),
        target_locale: TenantLocale::new("fr").expect("static target locale"),
    }
}

fn progress_request() -> TranslationTargetProgressRequest {
    TranslationTargetProgressRequest {
        source_locale: TenantLocale::new("en").expect("static source locale"),
        target_locale: TenantLocale::new("fr").expect("static target locale"),
    }
}

fn translation_patch(
    snapshot: &TranslationResourceSnapshot,
    name: &str,
    description: &str,
    suffix: &str,
) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![
            field_patch(snapshot, "name", name),
            field_patch(snapshot, "description", description),
        ],
        proposal_id: format!("forum-proposal-{suffix}"),
        approval_receipt_id: format!("forum-approval-{suffix}"),
    }
}

fn field_patch(
    snapshot: &TranslationResourceSnapshot,
    key: &str,
    value: &str,
) -> TranslationFieldPatch {
    let field = snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == key)
        .expect("requested Forum category field must be exposed");
    TranslationFieldPatch {
        key: FieldKey::new(key).expect("static field key"),
        value: value.to_string(),
        expected_source_hash: field.source_hash.clone(),
    }
}

fn exact_target_value<'a>(snapshot: &'a TranslationResourceSnapshot, key: &str) -> Option<&'a str> {
    snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == key)
        .and_then(|field| field.exact_target_value.as_deref())
}
