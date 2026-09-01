use std::{collections::BTreeMap, error::Error, io, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::{
    CreatePageInput, PageService, PageTranslationInput, PagesMetadataTranslationTargetProvider,
    PagesModule,
};
use rustok_translation_targets::{
    ReadTranslationResourceRequest, TranslationFieldPatch, TranslationPatchRequest,
    TranslationTargetChangesRequest, TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PAGES_TRANSLATION_TEST_POSTGRES_URL";
type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Self> {
        let database_url = std::env::var(DATABASE_ENV)
            .map_err(|_| test_error(format!("{DATABASE_ENV} must be configured")))?;
        let control = connect(&database_url).await?;
        let schema_name = format!("pages_translation_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(&database_url, &schema_name).await?;
        let manager = SchemaManager::new(&migration);
        SysEventsMigration.up(&manager).await?;
        for step in PagesModule.migrations() {
            step.up(&manager).await?;
        }
        migration.close().await?;

        Ok(Self {
            control,
            database_url,
            schema_name,
        })
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        self.control.close().await?;
        Ok(())
    }
}

#[tokio::test]
#[ignore = "requires RUSTOK_PAGES_TRANSLATION_TEST_POSTGRES_URL"]
async fn pages_translation_target_concurrent_metadata_and_cursor_recovery_postgres()
-> TestResult<()> {
    let database = TestDatabase::setup().await?;
    let result = run_contract(&database).await;
    let cleanup = database.cleanup().await;
    result?;
    cleanup
}

async fn run_contract(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let seed_connection = database.connection().await?;
    let page = service(seed_connection.clone())
        .create(tenant_id, SecurityContext::system(), source_page_input())
        .await?;
    let seed_provider = provider(seed_connection.clone());
    let source_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "source-cursor"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(source_changes.changes.len(), 1);
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("source Pages change cursor is missing"))?;
    seed_connection.close().await?;

    tokio::time::sleep(Duration::from_millis(2)).await;

    let first_connection = database.connection().await?;
    let second_connection = database.connection().await?;
    let first_provider = provider(first_connection.clone());
    let second_provider = provider(second_connection.clone());
    let request = ReadTranslationResourceRequest {
        identity: rustok_translation_targets::TranslationResourceIdentity {
            owner_slug: rustok_translation_targets::OwnerSlug::new("pages")?,
            resource_kind: rustok_translation_targets::ResourceKind::new("page_metadata")?,
            resource_id: rustok_translation_targets::ResourceId::new(page.id.to_string())?,
            subresource_id: None,
        },
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let first_snapshot = first_provider
        .read_resource(read_context(tenant_id, "replica-one-read"), request.clone())
        .await?;
    let second_snapshot = second_provider
        .read_resource(read_context(tenant_id, "replica-two-read"), request.clone())
        .await?;
    assert_eq!(
        first_snapshot.summary.resource_revision,
        second_snapshot.summary.resource_revision
    );
    assert_eq!(
        first_snapshot.source_revision,
        second_snapshot.source_revision
    );
    assert!(first_snapshot.target_revision.is_none());
    assert!(second_snapshot.target_revision.is_none());

    let left_patch = patch_from_snapshot(&first_snapshot, "a", "proposal-a", "approval-a");
    let right_patch = patch_from_snapshot(&second_snapshot, "b", "proposal-b", "approval-b");
    let left_key = "pages-postgres-apply-a";
    let right_key = "pages-postgres-apply-b";
    let left = first_provider.apply_patch(
        apply_context(tenant_id, "replica-one-apply", left_key),
        left_patch.clone(),
    );
    let right = second_provider.apply_patch(
        apply_context(tenant_id, "replica-two-apply", right_key),
        right_patch.clone(),
    );
    let (left, right) = tokio::join!(left, right);

    let (winner_patch, winner_key, receipt, loser) = match (left, right) {
        (Ok(receipt), Err(loser)) => (left_patch, left_key, receipt, loser),
        (Err(loser), Ok(receipt)) => (right_patch, right_key, receipt, loser),
        other => panic!("exactly one concurrent Pages metadata apply must win: {other:?}"),
    };
    assert_eq!(loser.kind, PortErrorKind::Conflict);
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");

    first_connection.close().await?;
    second_connection.close().await?;

    let observer_connection = database.connection().await?;
    let observer_provider = provider(observer_connection.clone());
    let applied = observer_provider
        .read_resource(read_context(tenant_id, "observer-read"), request.clone())
        .await?;
    let expected_values = winner_patch
        .fields
        .iter()
        .map(|field| (field.key.as_str().to_string(), field.value.clone()))
        .collect::<BTreeMap<_, _>>();
    for field in &applied.fields {
        assert_eq!(
            field.exact_target_value.as_deref(),
            expected_values
                .get(field.descriptor.key.as_str())
                .map(String::as_str)
        );
    }

    let target_changes = observer_provider
        .read_changes(
            read_context(tenant_id, "observer-cursor"),
            TranslationTargetChangesRequest {
                after: Some(source_cursor),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(target_changes.changes.len(), 1);
    assert_eq!(
        target_changes.changes[0].resource_revision,
        receipt.resource_revision
    );
    let target_cursor = target_changes
        .next_cursor
        .ok_or_else(|| test_error("target Pages change cursor is missing"))?;
    observer_connection.close().await?;

    let recovery_connection = database.connection().await?;
    let recovery_provider = provider(recovery_connection.clone());
    let replay = recovery_provider
        .apply_patch(
            apply_context(tenant_id, "recovery-replay", winner_key),
            winner_patch,
        )
        .await?;
    assert_eq!(replay, receipt);
    let resumed = recovery_provider
        .read_changes(
            read_context(tenant_id, "recovery-cursor"),
            TranslationTargetChangesRequest {
                after: Some(target_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    assert!(resumed.changes.is_empty());

    let progress = recovery_provider
        .read_progress(
            read_context(tenant_id, "recovery-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.complete_resources, 1);
    assert_eq!(progress.required_units, progress.exact_required_units);
    assert_eq!(progress.optional_units, progress.exact_optional_units);
    assert_eq!(progress.owner_change_cursor, Some(target_cursor));
    recovery_connection.close().await?;
    Ok(())
}

fn service(database: DatabaseConnection) -> Arc<PageService> {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    Arc::new(PageService::new(database, event_bus))
}

fn provider(database: DatabaseConnection) -> PagesMetadataTranslationTargetProvider {
    PagesMetadataTranslationTargetProvider::new(service(database))
}

fn source_page_input() -> CreatePageInput {
    CreatePageInput {
        translations: vec![PageTranslationInput {
            locale: "en".to_string(),
            title: "About us".to_string(),
            slug: Some("about-us".to_string()),
            meta_title: Some("About RusTok".to_string()),
            meta_description: Some("Learn about RusTok".to_string()),
        }],
        template: Some("default".to_string()),
        body: None,
        channel_slugs: None,
        publish: false,
    }
}

fn patch_from_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    candidate: &str,
    proposal_id: &str,
    approval_receipt_id: &str,
) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: snapshot
            .fields
            .iter()
            .map(|field| {
                let value = match field.descriptor.key.as_str() {
                    "title" => format!("A propos {candidate}"),
                    "slug" => format!("a-propos-{candidate}"),
                    "meta_title" => format!("A propos de RusTok {candidate}"),
                    "meta_description" => format!("Decouvrez RusTok {candidate}"),
                    other => panic!("unexpected Pages metadata field {other}"),
                };
                TranslationFieldPatch {
                    key: field.descriptor.key.clone(),
                    value,
                    expected_source_hash: field.source_hash.clone(),
                }
            })
            .collect(),
        proposal_id: proposal_id.to_string(),
        approval_receipt_id: approval_receipt_id.to_string(),
    }
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("pages-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, idempotency_key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(idempotency_key)
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_string());
    options
        .max_connections(8)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    let scoped_url =
        format!("{database_url}{separator}options=-csearch_path%3D{schema_name}%2Cpublic");
    connect(&scoped_url).await
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
