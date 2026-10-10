//! Runtime coverage for the append-only scenario baseline revision journal
//! (`page_builder_scenario_baseline_revisions`).
//!
//! The active baseline row keeps a single `previous_baseline_hash`, so the journal is the only
//! place the review trail survives. These tests pin the two properties the trail depends on:
//! every accepted mutation appends exactly one revision row describing it, and a rejected
//! mutation appends none — including the removed payload, so a cleared baseline stays
//! reconstructible.

use std::error::Error;

use chrono::Utc;
use fly::{GrapesJsCodec, PageSelection, RenderPolicy};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_page_builder::RuntimeContextScenario;
use rustok_pages::entities::{
    page, page_builder_scenario_baseline, page_builder_scenario_baseline_revision,
};
use rustok_pages::{
    PageBuilderScenarioBaselineService, PagesModule, SaveIfCurrentScenarioBaselineRequest,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, Set,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;

const SCENARIO_BASELINE_CONFLICT: &str = "SCENARIO_BASELINE_CONFLICT";
const SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED: &str = "SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED";
const REVIEW_NOTE: &str = "reviewed in the scenario regression editor";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn baseline_mutations_append_one_revision_row_each() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let page_id = insert_page(&db, tenant_id).await?;
    let service = PageBuilderScenarioBaselineService::new(db.clone());
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let first = baseline("baseline-1")?;
    assert!(
        first.is_valid(),
        "the fixture baseline must pass validate()"
    );
    service
        .save(tenant_id, security.clone(), page_id, first.clone())
        .await?;
    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 1, "a first save appends exactly one revision");
    let created = rows
        .iter()
        .find(|row| row.operation == "create")
        .expect("create revision");
    assert_eq!(created.operation, "create");
    assert_eq!(created.baseline_id, first.baseline_id);
    assert_eq!(created.baseline_hash, first.baseline_hash);
    assert_eq!(created.source_project_hash, first.source_project_hash);
    assert_eq!(created.previous_baseline_hash, None);
    assert_eq!(created.note, None);
    assert_eq!(created.baseline, serde_json::to_value(&first)?);

    // A CAS promotion appends a `replace` row that links the baseline it replaced.
    let second = baseline("baseline-2")?;
    service
        .save_if_current(SaveIfCurrentScenarioBaselineRequest {
            tenant_id,
            security: security.clone(),
            page_id,
            baseline: second.clone(),
            expected_baseline_hash: Some(first.baseline_hash.clone()),
            promoted_by: actor_id,
            promotion_note: Some(REVIEW_NOTE.to_string()),
        })
        .await?;
    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 2, "a promotion appends exactly one revision");
    let replaced = rows
        .iter()
        .find(|row| row.operation == "replace")
        .expect("replace revision");
    assert_eq!(replaced.baseline_id, second.baseline_id);
    assert_eq!(replaced.baseline_hash, second.baseline_hash);
    assert_eq!(
        replaced.previous_baseline_hash.as_deref(),
        Some(first.baseline_hash.as_str())
    );
    assert_eq!(replaced.actor_id, Some(actor_id));
    assert_eq!(replaced.note.as_deref(), Some(REVIEW_NOTE));
    assert_eq!(replaced.baseline, serde_json::to_value(&second)?);

    // A stale CAS value is rejected and must not leave a journal row behind.
    let stale = service
        .save_if_current(SaveIfCurrentScenarioBaselineRequest {
            tenant_id,
            security: security.clone(),
            page_id,
            baseline: baseline("baseline-3")?,
            expected_baseline_hash: Some(first.baseline_hash.clone()),
            promoted_by: actor_id,
            promotion_note: Some("stale promotion".to_string()),
        })
        .await
        .expect_err("a stale expected baseline hash must be rejected");
    assert!(
        stale.to_string().contains(SCENARIO_BASELINE_CONFLICT),
        "unexpected rejection: {stale}"
    );
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 2);

    // Clearing the baseline keeps the removed row reconstructible from the journal alone.
    let removed = service
        .delete_if_current(
            tenant_id,
            security.clone(),
            page_id,
            Some(second.baseline_hash.as_str()),
        )
        .await?;
    assert!(removed, "the matching baseline must be removed");
    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 3, "a clear appends exactly one revision");
    let deleted = rows
        .iter()
        .find(|row| row.operation == "delete")
        .expect("delete revision");
    assert_eq!(deleted.baseline_id, second.baseline_id);
    assert_eq!(deleted.baseline_hash, second.baseline_hash);
    assert_eq!(deleted.source_project_hash, second.source_project_hash);
    assert_eq!(deleted.previous_baseline_hash, None);
    assert_eq!(deleted.actor_id, Some(actor_id));
    assert_eq!(deleted.baseline, serde_json::to_value(&second)?);
    assert!(
        page_builder_scenario_baseline::Entity::find()
            .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
            .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
            .one(&db)
            .await?
            .is_none(),
        "the active baseline row must be gone"
    );
    Ok(())
}

#[tokio::test]
async fn rejected_promotion_without_review_note_leaves_the_journal_untouched() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let page_id = insert_page(&db, tenant_id).await?;
    let service = PageBuilderScenarioBaselineService::new(db.clone());
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let first = baseline("baseline-1")?;
    service
        .save(tenant_id, security.clone(), page_id, first.clone())
        .await?;
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 1);

    let error = service
        .save_if_current(SaveIfCurrentScenarioBaselineRequest {
            tenant_id,
            security: security.clone(),
            page_id,
            baseline: baseline("baseline-2")?,
            expected_baseline_hash: Some(first.baseline_hash.clone()),
            promoted_by: actor_id,
            promotion_note: None,
        })
        .await
        .expect_err("replacing a baseline without a review note must be rejected");
    assert!(
        error
            .to_string()
            .contains(SCENARIO_BASELINE_PROMOTION_NOTE_REQUIRED),
        "unexpected rejection: {error}"
    );

    assert_eq!(
        revisions(&db, tenant_id, page_id).await?.len(),
        1,
        "a rejected promotion must not append a revision"
    );
    let active = page_builder_scenario_baseline::Entity::find()
        .filter(page_builder_scenario_baseline::Column::TenantId.eq(tenant_id))
        .filter(page_builder_scenario_baseline::Column::PageId.eq(page_id))
        .one(&db)
        .await?
        .expect("the rejected promotion must leave the active baseline in place");
    assert_eq!(active.baseline_hash, first.baseline_hash);
    Ok(())
}

#[tokio::test]
async fn clear_paths_record_only_the_deletions_they_perform() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db().await?;
    let page_id = insert_page(&db, tenant_id).await?;
    let service = PageBuilderScenarioBaselineService::new(db.clone());
    let actor_id = Uuid::new_v4();
    let security = SecurityContext::new(UserRole::Admin, Some(actor_id));

    let first = baseline("baseline-1")?;
    service
        .save(tenant_id, security.clone(), page_id, first.clone())
        .await?;
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 1);

    // Without an expected hash a clear cannot tell what it is clearing, so it is rejected.
    let error = service
        .delete_if_current(tenant_id, security.clone(), page_id, None)
        .await
        .expect_err("clearing an existing baseline without an expected hash is a conflict");
    assert!(
        error.to_string().contains(SCENARIO_BASELINE_CONFLICT),
        "unexpected rejection: {error}"
    );
    assert_eq!(
        revisions(&db, tenant_id, page_id).await?.len(),
        1,
        "a rejected clear must not append a revision"
    );

    // The unconditional clear is allowed and is journaled with the acting user.
    assert!(service.delete(tenant_id, security.clone(), page_id).await?);
    let rows = revisions(&db, tenant_id, page_id).await?;
    assert_eq!(rows.len(), 2);
    let deleted = rows
        .iter()
        .find(|row| row.operation == "delete")
        .expect("delete revision");
    assert_eq!(deleted.baseline_hash, first.baseline_hash);
    assert_eq!(deleted.actor_id, Some(actor_id));

    // Clearing an absent baseline reports that nothing was removed and records nothing.
    assert!(
        !service
            .delete_if_current(tenant_id, security, page_id, None)
            .await?,
        "clearing an absent baseline reports that nothing was removed"
    );
    assert_eq!(revisions(&db, tenant_id, page_id).await?.len(), 2);
    Ok(())
}

/// A renderable document plus the scenario context its binding reads, mirroring the fixture
/// `fly`'s own release-baseline tests use to assert `is_valid()`.
fn baseline(baseline_id: &str) -> TestResult<RuntimeScenarioReleaseBaseline> {
    let document = GrapesJsCodec::decode_value(json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{ "id": "title", "type": "text" }]
            }
        }],
        "flyRuntimeBindings": [{
            "id": "title-content",
            "component_id": "title",
            "path": "page.title",
            "target": "field",
            "name": "content"
        }]
    }))?;
    Ok(RuntimeScenarioReleaseBaseline::capture(
        baseline_id,
        &document,
        &PageSelection::First,
        &RenderPolicy::default(),
        &[RuntimeContextScenario::new(
            "production",
            "Production",
            json!({ "page": { "title": "Scenario journal" } }),
        )],
    ))
}

async fn revisions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<Vec<page_builder_scenario_baseline_revision::Model>> {
    Ok(page_builder_scenario_baseline_revision::Entity::find()
        .filter(page_builder_scenario_baseline_revision::Column::TenantId.eq(tenant_id))
        .filter(page_builder_scenario_baseline_revision::Column::PageId.eq(page_id))
        .order_by_asc(page_builder_scenario_baseline_revision::Column::CreatedAt)
        .all(db)
        .await?)
}

async fn insert_page(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Uuid> {
    let page_id = Uuid::new_v4();
    let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
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
    Ok(page_id)
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_scenario_baseline_revision_{}?mode=memory&cache=shared",
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
