use std::{env, error::Error as StdError, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_blog::{
    BlogCategory, BlogCategoryTranslation, BlogCategoryTranslationTargetProvider, BlogModule,
    BlogTranslationChange, CategoryService, CreateCategoryInput,
    entities::{blog_category, blog_category_translation, translation_change},
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use rustok_translation_targets::{
    FieldKey, ReadTranslationResourceRequest, TranslationFieldPatch, TranslationPatchRequest,
    TranslationResourceLifecycle, TranslationResourceSnapshot, TranslationTargetChangesRequest,
    TranslationTargetProgressRequest, TranslationTargetProvider,
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    QueryFilter,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::Barrier;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_BLOG_TRANSLATION_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

struct PostgresBlogTranslationTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresBlogTranslationTestDb {
    async fn empty(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog category translation evidence"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_blog_translation_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let db = scoped_connection(&database_url, &schema_name).await?;
        Ok(Some(Self {
            control,
            db,
            database_url,
            schema_name,
        }))
    }

    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(test_db) = Self::empty(prefix).await? else {
            return Ok(None);
        };
        apply_dependency_migrations(&test_db.db).await?;
        let manager = SchemaManager::new(&test_db.db);
        for migration in BlogModule.migrations() {
            migration.up(&manager).await?;
        }
        Ok(Some(test_db))
    }

    async fn isolated_connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn category_translation_target_migration_supports_postgres_up_down_up() -> TestResult<()> {
    let Some(test_db) = PostgresBlogTranslationTestDb::empty("migration").await? else {
        return Ok(());
    };
    apply_dependency_migrations(&test_db.db).await?;
    let manager = SchemaManager::new(&test_db.db);
    let mut blog_migrations = BlogModule.migrations();
    let translation_target_migration = blog_migrations
        .pop()
        .expect("Blog translation target migration must remain registered last");
    for migration in blog_migrations {
        migration.up(&manager).await?;
    }

    translation_target_migration.up(&manager).await?;
    translation_target_migration.down(&manager).await?;
    translation_target_migration.up(&manager).await?;

    let tenant_id = Uuid::new_v4();
    let service = category_service(test_db.db.clone());
    let category_id = service
        .create(
            tenant_id,
            admin(),
            category_input("PostgreSQL migration", "postgres-migration"),
        )
        .await?;

    let category = BlogCategory::find_by_id(category_id)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(&test_db.db)
        .await?
        .expect("reapplied migration must retain category revision storage");
    assert_eq!(category.revision, 1);
    let source = BlogCategoryTranslation::find()
        .filter(blog_category_translation::Column::TenantId.eq(tenant_id))
        .filter(blog_category_translation::Column::CategoryId.eq(category_id))
        .filter(blog_category_translation::Column::Locale.eq("en"))
        .one(&test_db.db)
        .await?
        .expect("reapplied migration must retain exact-locale revision storage");
    assert_eq!(source.revision, 1);

    let provider = BlogCategoryTranslationTargetProvider::new(service);
    let changes = provider
        .read_changes(
            read_context(tenant_id, "blog-translation-postgres-migration"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(changes.changes.len(), 1);
    assert_eq!(changes.changes[0].resource_revision.as_str(), "1");
    assert_eq!(
        changes.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert!(changes.next_cursor.is_some());

    test_db.cleanup().await
}

#[tokio::test]
async fn concurrent_same_revision_translation_applies_commit_once() -> TestResult<()> {
    let Some(test_db) = PostgresBlogTranslationTestDb::setup("concurrent_cas").await? else {
        return Ok(());
    };
    let tenant_id = Uuid::new_v4();
    let service = category_service(test_db.db.clone());
    let category_id = service
        .create(tenant_id, admin(), category_input("Systems", "systems"))
        .await?;
    let provider = BlogCategoryTranslationTargetProvider::new(service);
    let snapshot = provider
        .read_resource(
            read_context(tenant_id, "blog-translation-concurrent-snapshot"),
            resource_request(category_id),
        )
        .await?;

    let candidates = [
        ("Systemes alpha", "systemes-alpha", "candidate-alpha"),
        ("Systemes beta", "systemes-beta", "candidate-beta"),
    ];
    let barrier = Arc::new(Barrier::new(candidates.len()));
    let mut tasks = Vec::with_capacity(candidates.len());
    for (name, slug, label) in candidates {
        let db = test_db.isolated_connection().await?;
        let provider = BlogCategoryTranslationTargetProvider::new(category_service(db));
        let barrier = Arc::clone(&barrier);
        let patch = translation_patch(&snapshot, name, slug, label);
        let context = apply_context(
            tenant_id,
            &format!("blog-translation-{label}"),
            &format!("blog-translation-idempotency-{label}"),
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
    assert_eq!(
        successes.len(),
        1,
        "exactly one same-revision apply must commit"
    );
    assert_eq!(failures.len(), 1, "the competing stale apply must close");
    assert_eq!(failures[0].kind, PortErrorKind::Conflict);

    let final_provider = BlogCategoryTranslationTargetProvider::new(category_service(
        test_db.isolated_connection().await?,
    ));
    let final_snapshot = final_provider
        .read_resource(
            read_context(tenant_id, "blog-translation-concurrent-final"),
            resource_request(category_id),
        )
        .await?;
    assert_eq!(final_snapshot.summary.resource_revision.as_str(), "2");
    assert_eq!(
        final_snapshot
            .target_revision
            .as_ref()
            .map(|revision| revision.as_str()),
        Some("1")
    );
    assert_eq!(
        exact_target_value(&final_snapshot, "name"),
        Some(successes[0].0.as_str())
    );

    let changes = BlogTranslationChange::find()
        .filter(translation_change::Column::TenantId.eq(tenant_id))
        .all(&test_db.db)
        .await?;
    assert_eq!(
        changes.len(),
        2,
        "source create plus one winning apply only"
    );
    let outbox = SysEvents::find().all(&test_db.db).await?;
    assert_eq!(
        outbox.len(),
        1,
        "only the winning apply may publish reindex"
    );
    assert_eq!(outbox[0].event_type, "index.reindex_requested");

    test_db.cleanup().await
}

#[tokio::test]
async fn change_cursor_resumes_after_provider_reconstruction_and_delete() -> TestResult<()> {
    let Some(test_db) = PostgresBlogTranslationTestDb::setup("cursor_recovery").await? else {
        return Ok(());
    };
    let tenant_id = Uuid::new_v4();
    let service = category_service(test_db.db.clone());
    let category_id = service
        .create(tenant_id, admin(), category_input("Recovery", "recovery"))
        .await?;
    let provider = BlogCategoryTranslationTargetProvider::new(service);
    let first_page = provider
        .read_changes(
            read_context(tenant_id, "blog-translation-cursor-first"),
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
        .expect("source creation must publish a resume cursor");
    drop(provider);

    // Blog change IDs are ULID-backed UUIDs. Keep independent retained writes in
    // distinct milliseconds so this recovery target is deterministic without
    // claiming a concurrent commit-order guarantee from the cursor itself.
    tokio::time::sleep(Duration::from_millis(2)).await;

    let apply_provider = BlogCategoryTranslationTargetProvider::new(category_service(
        test_db.isolated_connection().await?,
    ));
    let snapshot = apply_provider
        .read_resource(
            read_context(tenant_id, "blog-translation-cursor-apply-read"),
            resource_request(category_id),
        )
        .await?;
    let receipt = apply_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "blog-translation-cursor-apply",
                "blog-translation-cursor-apply-1",
            ),
            translation_patch(&snapshot, "Recuperation", "recuperation", "cursor-recovery"),
        )
        .await?;
    assert_eq!(receipt.resource_revision.as_str(), "2");
    assert_eq!(receipt.target_revision.as_str(), "1");

    let second_page = apply_provider
        .read_changes(
            read_context(tenant_id, "blog-translation-cursor-second"),
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
        .expect("translation apply must advance the resume cursor");
    assert_ne!(second_cursor, first_cursor);
    drop(apply_provider);

    tokio::time::sleep(Duration::from_millis(2)).await;
    let delete_service = category_service(test_db.isolated_connection().await?);
    delete_service
        .delete(tenant_id, category_id, admin())
        .await?;
    drop(delete_service);

    let recovered_provider = BlogCategoryTranslationTargetProvider::new(category_service(
        test_db.isolated_connection().await?,
    ));
    let deleted_page = recovered_provider
        .read_changes(
            read_context(tenant_id, "blog-translation-cursor-deleted"),
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
        .expect("delete must advance the durable Blog change cursor");
    assert_ne!(deleted_cursor, second_cursor);

    let drained = recovered_provider
        .read_changes(
            read_context(tenant_id, "blog-translation-cursor-drained"),
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
            read_context(tenant_id, "blog-translation-cursor-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(progress.resources, 0);
    assert_eq!(progress.owner_change_cursor, Some(deleted_cursor));

    test_db.cleanup().await
}

async fn apply_dependency_migrations(db: &DatabaseConnection) -> TestResult<()> {
    let manager = SchemaManager::new(db);
    for migration in OutboxModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(())
}

fn category_service(db: DatabaseConnection) -> Arc<CategoryService> {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    Arc::new(CategoryService::new(db, event_bus))
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn category_input(name: &str, slug: &str) -> CreateCategoryInput {
    CreateCategoryInput {
        locale: "en".to_string(),
        name: name.to_string(),
        slug: Some(slug.to_string()),
        description: Some(format!("{name} description")),
        parent_id: None,
        position: None,
        settings: serde_json::json!({}),
    }
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
            owner_slug: rustok_translation_targets::OwnerSlug::new("blog")
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

fn translation_patch(
    snapshot: &TranslationResourceSnapshot,
    name: &str,
    slug: &str,
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
            field_patch(snapshot, "slug", slug),
            field_patch(snapshot, "description", &format!("description-{suffix}")),
        ],
        proposal_id: format!("blog-proposal-{suffix}"),
        approval_receipt_id: format!("blog-approval-{suffix}"),
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
        .expect("requested Blog category field must be exposed");
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

fn postgres_database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("RUSTOK_BLOG_TEST_DATABASE_URL"))
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}
