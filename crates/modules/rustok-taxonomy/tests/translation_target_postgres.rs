use std::{env, error::Error as StdError, io, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    TaxonomyTranslationTargetProvider,
};
use rustok_translation_targets::{
    FieldKey, ListTranslationResourcesRequest, ReadTranslationResourceRequest,
    TranslationFieldPatch, TranslationPatchRequest, TranslationResourceLifecycle,
    TranslationResourceSnapshot, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use tokio::sync::Barrier;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_TAXONOMY_TEST_DATABASE_URL";
const REQUIRED_CANONICAL_TABLES: &[&str] = &[
    "owner_operation_receipts",
    "taxonomy_terms",
    "taxonomy_term_translations",
    "taxonomy_translation_changes",
    "taxonomy_term_route_keys",
];

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

struct PostgresTaxonomyTranslationDb {
    db: DatabaseConnection,
    database_url: String,
}

impl PostgresTaxonomyTranslationDb {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Taxonomy translation-target evidence"
            );
            return Ok(None);
        };

        let db = connect(&database_url).await?;
        ensure_canonical_schema(&db).await?;
        Ok(Some(Self { db, database_url }))
    }

    async fn isolated_connection(&self) -> TestResult<DatabaseConnection> {
        connect(&self.database_url).await
    }
}

#[tokio::test]
async fn concurrent_same_revision_translation_applies_commit_once() -> TestResult<()> {
    let Some(test_db) = PostgresTaxonomyTranslationDb::setup().await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let term_id = create_source_term(&test_db.db, tenant_id, "Systems", "systems").await?;
    let base_provider = provider(test_db.db.clone());
    let identity = single_identity(&base_provider, tenant_id).await?;
    let initial_snapshot = base_provider
        .read_resource(
            read_context(tenant_id, "taxonomy-translation-seed-read"),
            resource_request(identity.clone()),
        )
        .await?;
    assert!(initial_snapshot.target_revision.is_none());

    let seed_receipt = base_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "taxonomy-translation-seed-apply",
                "taxonomy-translation-seed-apply-1",
            ),
            translation_patch(
                &initial_snapshot,
                "Translated Systems",
                "translated-systems",
                "taxonomy-seed",
            ),
        )
        .await?;
    assert_eq!(seed_receipt.resource_revision.as_str(), "2");
    assert_eq!(seed_receipt.target_revision.as_str(), "1");

    let shared_snapshot = base_provider
        .read_resource(
            read_context(tenant_id, "taxonomy-translation-concurrent-read"),
            resource_request(identity.clone()),
        )
        .await?;
    assert_eq!(shared_snapshot.summary.resource_revision.as_str(), "2");
    assert_eq!(
        shared_snapshot
            .target_revision
            .as_ref()
            .map(|revision| revision.as_str()),
        Some("1")
    );

    let candidates = [
        (
            "Translated Systems alpha",
            "translated-systems-alpha",
            "candidate-alpha",
        ),
        (
            "Translated Systems beta",
            "translated-systems-beta",
            "candidate-beta",
        ),
    ];
    let barrier = Arc::new(Barrier::new(candidates.len()));
    let mut tasks = Vec::with_capacity(candidates.len());
    for (name, slug, label) in candidates {
        let candidate_provider = provider(test_db.isolated_connection().await?);
        let barrier = Arc::clone(&barrier);
        let patch = translation_patch(&shared_snapshot, name, slug, label);
        let context = apply_context(
            tenant_id,
            &format!("taxonomy-translation-{label}"),
            &format!("taxonomy-translation-idempotency-{label}"),
        );
        let expected_name = name.to_string();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            (
                expected_name,
                candidate_provider.apply_patch(context, patch).await,
            )
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
    assert_eq!(
        successes.len(),
        1,
        "exactly one same-revision Taxonomy apply must commit"
    );
    assert_eq!(
        failures.len(),
        1,
        "the competing stale Taxonomy apply must close"
    );
    assert_eq!(failures[0].kind, PortErrorKind::Conflict);

    let final_provider = provider(test_db.isolated_connection().await?);
    let final_snapshot = final_provider
        .read_resource(
            read_context(tenant_id, "taxonomy-translation-concurrent-final"),
            resource_request(identity),
        )
        .await?;
    assert_eq!(final_snapshot.summary.resource_revision.as_str(), "3");
    assert_eq!(
        final_snapshot
            .target_revision
            .as_ref()
            .map(|revision| revision.as_str()),
        Some("2")
    );
    assert_eq!(
        exact_target_value(&final_snapshot, "name"),
        Some(successes[0].0.as_str())
    );

    let changes = final_provider
        .read_changes(
            read_context(tenant_id, "taxonomy-translation-concurrent-changes"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(
        changes.changes.len(),
        3,
        "source create, seed apply, and exactly one winning concurrent apply must be durable"
    );
    let winning_change = changes
        .changes
        .iter()
        .find(|change| change.resource_revision.as_str() == "3")
        .expect("the winning concurrent apply must append resource revision 3");
    assert_eq!(
        winning_change.lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert_eq!(
        winning_change.identity.resource_id.as_str(),
        term_id.to_string()
    );

    Ok(())
}

#[tokio::test]
async fn change_cursor_resumes_after_provider_reconstruction_and_delete() -> TestResult<()> {
    let Some(test_db) = PostgresTaxonomyTranslationDb::setup().await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let term_id = create_source_term(&test_db.db, tenant_id, "Recovery", "recovery").await?;
    let initial_provider = provider(test_db.db.clone());
    let identity = single_identity(&initial_provider, tenant_id).await?;
    let first_page = initial_provider
        .read_changes(
            read_context(tenant_id, "taxonomy-translation-cursor-first"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(first_page.changes.len(), 1);
    assert_eq!(first_page.changes[0].resource_revision.as_str(), "1");
    let first_cursor = first_page
        .next_cursor
        .expect("source creation must publish a Taxonomy resume cursor");
    drop(initial_provider);

    // Taxonomy change IDs are ULID-backed UUIDs. Keep retained sequential writes
    // in distinct milliseconds so this recovery fixture remains deterministic;
    // it does not claim arbitrary concurrent transaction commit ordering.
    tokio::time::sleep(Duration::from_millis(2)).await;

    let apply_provider = provider(test_db.isolated_connection().await?);
    let snapshot = apply_provider
        .read_resource(
            read_context(tenant_id, "taxonomy-translation-cursor-apply-read"),
            resource_request(identity.clone()),
        )
        .await?;
    let receipt = apply_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "taxonomy-translation-cursor-apply",
                "taxonomy-translation-cursor-apply-1",
            ),
            translation_patch(
                &snapshot,
                "Recovery target",
                "recovery-target",
                "cursor-recovery",
            ),
        )
        .await?;
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");

    let second_page = apply_provider
        .read_changes(
            read_context(tenant_id, "taxonomy-translation-cursor-second"),
            TranslationTargetChangesRequest {
                after: Some(first_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(second_page.changes.len(), 1);
    assert_eq!(second_page.changes[0].resource_revision.as_str(), "2");
    assert_eq!(
        second_page.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let second_cursor = second_page
        .next_cursor
        .expect("translation apply must advance the Taxonomy resume cursor");
    assert_ne!(second_cursor, first_cursor);
    drop(apply_provider);

    tokio::time::sleep(Duration::from_millis(2)).await;
    let delete_service = TaxonomyService::new(test_db.isolated_connection().await?);
    delete_service
        .delete_term(tenant_id, term_id, admin())
        .await?;
    drop(delete_service);

    let recovered_provider = provider(test_db.isolated_connection().await?);
    let deleted_page = recovered_provider
        .read_changes(
            read_context(tenant_id, "taxonomy-translation-cursor-deleted"),
            TranslationTargetChangesRequest {
                after: Some(second_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(deleted_page.changes.len(), 1);
    assert_eq!(deleted_page.changes[0].resource_revision.as_str(), "3");
    assert_eq!(
        deleted_page.changes[0].lifecycle,
        TranslationResourceLifecycle::Deleted
    );
    let deleted_cursor = deleted_page
        .next_cursor
        .expect("delete must advance the durable Taxonomy change cursor");
    assert_ne!(deleted_cursor, second_cursor);

    let drained = recovered_provider
        .read_changes(
            read_context(tenant_id, "taxonomy-translation-cursor-drained"),
            TranslationTargetChangesRequest {
                after: Some(deleted_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert!(drained.changes.is_empty());
    assert!(drained.next_cursor.is_none());

    let progress = recovered_provider
        .read_progress(
            read_context(tenant_id, "taxonomy-translation-cursor-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(progress.resources, 0);
    assert_eq!(progress.owner_change_cursor, Some(deleted_cursor));

    Ok(())
}

async fn create_source_term(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
) -> rustok_taxonomy::TaxonomyResult<Uuid> {
    TaxonomyService::new(db.clone())
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(slug.to_string()),
                canonical_key: Some(slug.to_string()),
                description: Some(format!("{name} description")),
                aliases: Vec::new(),
            },
        )
        .await
}

fn provider(db: DatabaseConnection) -> TaxonomyTranslationTargetProvider {
    TaxonomyTranslationTargetProvider::new(Arc::new(TaxonomyService::new(db)))
}

async fn single_identity(
    provider: &TaxonomyTranslationTargetProvider,
    tenant_id: Uuid,
) -> TestResult<rustok_translation_targets::TranslationResourceIdentity> {
    let page = provider
        .list_resources(
            read_context(tenant_id, "taxonomy-translation-list"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(page.resources.len(), 1);
    Ok(page.resources[0].identity.clone())
}

fn resource_request(
    identity: rustok_translation_targets::TranslationResourceIdentity,
) -> ReadTranslationResourceRequest {
    ReadTranslationResourceRequest {
        identity,
        source_locale: TenantLocale::new("en").expect("static source locale should be valid"),
        target_locale: TenantLocale::new("fr").expect("static target locale should be valid"),
    }
}

fn translation_patch(
    snapshot: &TranslationResourceSnapshot,
    name: &str,
    slug: &str,
    label: &str,
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
            field_patch(snapshot, "slug", slug),
        ],
        proposal_id: format!("taxonomy-proposal-{label}"),
        approval_receipt_id: format!("taxonomy-approval-{label}"),
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
        .expect("requested Taxonomy field should be exposed");
    TranslationFieldPatch {
        key: FieldKey::new(key).expect("static field key should be valid"),
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

fn postgres_database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn ensure_canonical_schema(db: &DatabaseConnection) -> TestResult<()> {
    for table in REQUIRED_CANONICAL_TABLES {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT to_regclass($1) IS NOT NULL AS present",
                [(*table).into()],
            ))
            .await?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("canonical schema probe returned no row for {table}"),
                )
            })?;
        let present: bool = row.try_get("", "present")?;
        if !present {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "canonical PostgreSQL schema is missing {table}; run `cargo run --locked -p rustok-migrations --bin rustok-migrate -- up` with DATABASE_URL before this evidence target"
                ),
            )
            .into());
        }
    }
    Ok(())
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}
