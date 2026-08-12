#![cfg(feature = "mod-product")]

use std::{
    env,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation,
    IndexQuery, IndexQueryPort, IndexQueryScope, IndexReplayCheckpoint, IndexReplayCheckpointKey,
    IndexReplayCheckpointStore, IndexReplayError, IndexReplayFailure, IndexReplayMutationOutcome,
    IndexReplayMutationSink, IndexReplayPageRequest, IndexReplayPageStatus, IndexReplayWorker,
    IndexSourceLoadRequest, IndexValue, LinkName, LocaleKey, ModuleName, MutationApplyOutcome,
    MutationDelivery, Pagination, PostgresMutationStore, PostgresSchemaRegistrationStore,
    SchemaRef, SchemaVersion, SharedIndexQueryRuntime, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry, materialize_index_source_registry,
    materialize_postgres_index_query_runtime, materialize_postgres_index_sources,
};
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistrations, ModuleWorkScheduler};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const PRODUCT_VARIANT_SOURCE: &str = "product-variant-postgres-primary";

const TENANT_ID: Uuid = Uuid::from_u128(1);
const PRODUCT_ID: Uuid = Uuid::from_u128(101);
const PRODUCT_TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const VARIANT_ID: Uuid = Uuid::from_u128(201);
const CHANNEL_ID: Uuid = Uuid::from_u128(301);

const INITIAL_SKU: &str = "variant-v1";
const HISTORICAL_SKU: &str = "variant-v2-never-applied";
const CURRENT_SKU: &str = "variant-v3-current";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct FailOnceCheckpointStore {
    checkpoint: Arc<Mutex<Option<IndexReplayCheckpoint>>>,
    fail_next_commit: Arc<AtomicBool>,
}

impl FailOnceCheckpointStore {
    fn new() -> Self {
        Self {
            checkpoint: Arc::new(Mutex::new(None)),
            fail_next_commit: Arc::new(AtomicBool::new(true)),
        }
    }

    fn checkpoint(&self) -> Option<IndexReplayCheckpoint> {
        self.checkpoint
            .lock()
            .expect("checkpoint test mutex must not be poisoned")
            .clone()
    }
}

#[async_trait]
impl IndexReplayCheckpointStore for FailOnceCheckpointStore {
    async fn load_replay_checkpoint(
        &self,
        _key: &IndexReplayCheckpointKey,
    ) -> Result<Option<IndexReplayCheckpoint>, IndexReplayFailure> {
        Ok(self.checkpoint())
    }

    async fn commit_replay_checkpoint(
        &self,
        checkpoint: &IndexReplayCheckpoint,
    ) -> Result<(), IndexReplayFailure> {
        if self.fail_next_commit.swap(false, Ordering::SeqCst) {
            return Err(
                IndexReplayFailure::retryable("injected_checkpoint_commit_failure")
                    .expect("static replay failure code is valid"),
            );
        }
        *self
            .checkpoint
            .lock()
            .expect("checkpoint test mutex must not be poisoned") = Some(checkpoint.clone());
        Ok(())
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    migration: DatabaseConnection,
    work: DatabaseConnection,
    source: DatabaseConnection,
    mutation: DatabaseConnection,
    query: DatabaseConnection,
    writer: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping linked-target replay/redelivery harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_link_replay_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_link_replay_migration_{suffix}"),
        )
        .await?;
        create_migration_prerequisites(&migration).await?;
        let manager = SchemaManager::new(&migration);
        for step in rustok_channel::migrations::migrations() {
            step.up(&manager).await?;
        }
        for step in rustok_product::migrations::migrations() {
            step.up(&manager).await?;
        }
        for step in IndexModule.migrations() {
            step.up(&manager).await?;
        }
        seed_owner_rows(&migration).await?;

        Ok(Some(Self {
            control,
            migration,
            work: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_replay_work_{suffix}"),
            )
            .await?,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_replay_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_replay_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_replay_query_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_replay_writer_{suffix}"),
            )
            .await?,
            database_url,
            schema_name,
        }))
    }

    async fn fresh_query_runtime(&self) -> TestResult<SharedIndexQueryRuntime> {
        let suffix = Uuid::new_v4().simple().to_string();
        let query = scoped_connection(
            &self.database_url,
            &self.schema_name,
            &format!("idx_link_replay_restart_query_{suffix}"),
        )
        .await?;
        let registry = ModuleRegistry::new()
            .register(IndexModule)
            .register(rustok_channel::ChannelModule)
            .register(rustok_product::ProductModule);
        let mut extensions = rustok_distribution::build_runtime_extensions(&registry)?;
        materialize_postgres_index_query_runtime(&mut extensions, query)?
            .ok_or_else(|| std::io::Error::other("fresh Index query runtime is missing").into())
    }

    async fn cleanup(self) -> TestResult<()> {
        self.migration.close().await?;
        self.work.close().await?;
        self.source.close().await?;
        self.mutation.close().await?;
        self.query.close().await?;
        self.writer.close().await?;
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

struct Runtime {
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    query: SharedIndexQueryRuntime,
    mutations: PostgresMutationStore,
    scheduler: ModuleWorkScheduler,
}

#[tokio::test]
async fn replay_checkpoint_failure_duplicate_retry_and_late_stale_target_keep_graph_authoritative()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let result = run_scenario(&database).await;
    let cleanup = database.cleanup().await;
    result?;
    cleanup
}

async fn run_scenario(database: &TestDatabase) -> TestResult<()> {
    let runtime = build_runtime(database).await?;
    run_scheduler_until_idle(&runtime.scheduler, 24).await?;

    apply_current_source_mutation(&runtime, PRODUCT_SOURCE, product_key()?).await?;
    let initial_variant = load_current_source_mutation(&runtime.sources, variant_key()?).await?;
    let initial_version = initial_variant.source_version();
    assert_eq!(initial_version, 1);
    assert_eq!(
        apply_mutation(&runtime, PRODUCT_VARIANT_SOURCE, initial_variant.clone(),).await?,
        MutationApplyOutcome::Applied {
            source_version: initial_version,
        }
    );
    assert_graph_payload(&runtime.query, INITIAL_SKU).await?;

    // Produce a canonical intermediate owner mutation but intentionally never deliver it.
    update_variant_sku(&database.writer, HISTORICAL_SKU).await?;
    let historical = load_current_source_mutation(&runtime.sources, variant_key()?).await?;
    let historical_version = historical.source_version();
    assert!(historical_version > initial_version);

    // Advance the owner again. Product membership/projection is unchanged, so the Product root remains
    // current while its Variant target is now two revisions behind and the linked graph must fail closed.
    update_variant_sku(&database.writer, CURRENT_SKU).await?;
    let current = load_current_source_mutation(&runtime.sources, variant_key()?).await?;
    let current_version = current.source_version();
    assert!(current_version > historical_version);
    assert_scalar_product_visible(&runtime.query, true).await?;
    assert_graph_visible(&runtime.query, false).await?;
    assert_eq!(
        materialized_variant_version(&database.mutation).await?,
        initial_version
    );

    let checkpoint_store = FailOnceCheckpointStore::new();
    let request = IndexReplayPageRequest::new(TENANT_ID, variant_schema_ref()?, 32)?;
    let worker = IndexReplayWorker::new(
        runtime.sources.clone(),
        runtime.schemas.shared(),
        runtime.mutations.clone(),
        checkpoint_store.clone(),
    );

    // Canonical source scan returns only current owner state. PostgreSQL mutation durability succeeds,
    // then the injected checkpoint commit failure simulates process loss after mutation commit.
    assert!(matches!(
        worker.run_next_page(request.clone()).await,
        Err(IndexReplayError::CheckpointCommitFailed(_))
    ));
    assert!(checkpoint_store.checkpoint().is_none());
    assert_eq!(
        materialized_variant_version(&database.mutation).await?,
        current_version
    );
    assert_graph_payload(&runtime.query, CURRENT_SKU).await?;

    // A separately composed query runtime observes the durable current target immediately even though
    // replay cursor durability failed. Query authority has no process-local checkpoint dependency.
    let restarted_query = database.fresh_query_runtime().await?;
    assert_graph_payload(&restarted_query, CURRENT_SKU).await?;

    // Recreate the replay worker after the simulated crash. With no checkpoint, it scans the same
    // current owner row and derives the same stable event UUID. The PostgreSQL inbox reports Duplicate;
    // only then does the retry commit the completed checkpoint.
    let restarted_worker = IndexReplayWorker::new(
        runtime.sources.clone(),
        runtime.schemas.shared(),
        runtime.mutations.clone(),
        checkpoint_store.clone(),
    );
    let retry = restarted_worker.run_next_page(request).await?;
    assert_eq!(retry.status(), IndexReplayPageStatus::Complete);
    assert_eq!(retry.mutation_count(), 1);
    assert_eq!(retry.applied_count(), 0);
    assert_eq!(retry.duplicate_count(), 1);
    assert_eq!(retry.stale_count(), 0);
    let checkpoint = checkpoint_store
        .checkpoint()
        .ok_or_else(|| std::io::Error::other("replay checkpoint was not committed on retry"))?;
    assert!(checkpoint.is_complete());
    assert_eq!(checkpoint.source_version(), Some(current_version));
    let current_delivery_id = current.event_id().to_string();
    assert_eq!(
        checkpoint.last_delivery_id(),
        Some(current_delivery_id.as_str())
    );

    // Deliver the never-before-applied intermediate canonical mutation after current v3 is durable.
    // The replay sink must classify it as stale rather than regressing target payload or source version.
    assert_eq!(
        runtime
            .mutations
            .apply_replay_mutation(
                runtime.schemas.registry(),
                PRODUCT_VARIANT_SOURCE,
                &historical,
            )
            .await?,
        IndexReplayMutationOutcome::StaleIgnored
    );
    assert_eq!(
        materialized_variant_version(&database.mutation).await?,
        current_version
    );
    assert_graph_payload(&runtime.query, CURRENT_SKU).await?;
    assert_graph_payload(&restarted_query, CURRENT_SKU).await?;

    // Exact current redelivery remains Duplicate and equally cannot disturb graph authority.
    assert_eq!(
        runtime
            .mutations
            .apply_replay_mutation(runtime.schemas.registry(), PRODUCT_VARIANT_SOURCE, &current,)
            .await?,
        IndexReplayMutationOutcome::Duplicate
    );
    assert_eq!(
        materialized_variant_version(&database.mutation).await?,
        current_version
    );
    assert_graph_payload(&runtime.query, CURRENT_SKU).await?;
    Ok(())
}

async fn build_runtime(database: &TestDatabase) -> TestResult<Runtime> {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_channel::ChannelModule)
        .register(rustok_product::ProductModule);
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)?;
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or_else(|| std::io::Error::other("Index schema registry is missing"))?;
    let schema_store = PostgresSchemaRegistrationStore::new(database.query.clone());
    for registered in schemas.registry().iter() {
        schema_store.register(TENANT_ID, &registered.schema).await?;
    }
    materialize_postgres_index_sources(&mut extensions, database.source.clone())?;
    let sources = materialize_index_source_registry(&extensions)?
        .ok_or_else(|| std::io::Error::other("Index source registry is missing"))?;
    let query = materialize_postgres_index_query_runtime(&mut extensions, database.query.clone())?
        .ok_or_else(|| std::io::Error::other("Index query runtime is missing"))?;
    let registrations = extensions
        .get::<ModuleWorkRegistrations>()
        .cloned()
        .ok_or_else(|| std::io::Error::other("Product convergence work is missing"))?;
    let scheduler = ModuleWorkScheduler::new();
    registrations
        .register_all(&HostRuntimeContext::new(database.work.clone()), &scheduler)
        .await?;
    Ok(Runtime {
        sources,
        schemas,
        query,
        mutations: PostgresMutationStore::new(database.mutation.clone()),
        scheduler,
    })
}

async fn run_scheduler_until_idle(
    scheduler: &ModuleWorkScheduler,
    maximum_iterations: usize,
) -> TestResult<usize> {
    let mut executed = 0;
    for _ in 0..maximum_iterations {
        let current = scheduler.run_once().await?;
        if current == 0 {
            return Ok(executed);
        }
        executed += current;
    }
    Err(std::io::Error::other(format!(
        "ModuleWork scheduler did not become idle after {maximum_iterations} bounded iterations"
    ))
    .into())
}

async fn load_current_source_mutation(
    sources: &SharedIndexSourceRegistry,
    key: EntityKey,
) -> TestResult<IndexMutation> {
    let request = IndexSourceLoadRequest::new(vec![key])?;
    let mut mutations = sources.load(request).await?.into_mutations();
    if mutations.len() != 1 {
        return Err(std::io::Error::other(format!(
            "expected exactly one source mutation, got {}",
            mutations.len()
        ))
        .into());
    }
    Ok(mutations.remove(0))
}

async fn apply_current_source_mutation(
    runtime: &Runtime,
    source_name: &str,
    key: EntityKey,
) -> TestResult<u64> {
    let mutation = load_current_source_mutation(&runtime.sources, key).await?;
    let source_version = mutation.source_version();
    match apply_mutation(runtime, source_name, mutation).await? {
        MutationApplyOutcome::Applied {
            source_version: applied,
        } if applied == source_version => Ok(source_version),
        other => Err(std::io::Error::other(format!(
            "expected source mutation {source_version} to apply, got {other:?}"
        ))
        .into()),
    }
}

async fn apply_mutation(
    runtime: &Runtime,
    source_name: &str,
    mutation: IndexMutation,
) -> TestResult<MutationApplyOutcome> {
    let delivery = MutationDelivery::from_event(source_name, mutation)?;
    Ok(runtime
        .mutations
        .apply(runtime.schemas.registry(), &delivery)
        .await?)
}

fn product_key() -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: product_schema_ref()?,
        entity_id: PRODUCT_ID,
        locale: Some(LocaleKey::new("en")?),
    })
}

fn variant_key() -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: variant_schema_ref()?,
        entity_id: VARIANT_ID,
        locale: None,
    })
}

fn product_schema_ref() -> TestResult<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        version: SchemaVersion::new(4),
    })
}

fn variant_schema_ref() -> TestResult<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product_variant")?,
        version: SchemaVersion::new(2),
    })
}

fn scalar_product_query() -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT_ID,
            locale: Some(LocaleKey::new("en")?),
        },
        schema: product_schema_ref()?,
        fields: vec![FieldPath::new(FieldName::new("title")?)],
        filter: Some(FilterExpr::Eq(
            FieldPath::new(FieldName::new("id")?),
            IndexValue::Uuid(PRODUCT_ID),
        )),
        order_by: Vec::new(),
        pagination: Pagination::Offset {
            limit: 10,
            offset: 0,
        },
        include_exact_count: true,
    })
}

fn variant_graph_query() -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT_ID,
            locale: Some(LocaleKey::new("en")?),
        },
        schema: product_schema_ref()?,
        fields: vec![
            FieldPath::new(FieldName::new("title")?),
            FieldPath::linked([LinkName::new("variants")?], FieldName::new("sku")?),
        ],
        filter: Some(FilterExpr::Eq(
            FieldPath::new(FieldName::new("id")?),
            IndexValue::Uuid(PRODUCT_ID),
        )),
        order_by: Vec::new(),
        pagination: Pagination::Offset {
            limit: 10,
            offset: 0,
        },
        include_exact_count: true,
    })
}

async fn assert_scalar_product_visible(
    query: &SharedIndexQueryRuntime,
    expected: bool,
) -> TestResult<()> {
    let page = query.execute_query(scalar_product_query()?).await?;
    let expected_rows = if expected { 1 } else { 0 };
    let expected_count = if expected { 1 } else { 0 };
    assert_eq!(page.items.len(), expected_rows);
    assert_eq!(page.exact_count, Some(expected_count));
    Ok(())
}

async fn assert_graph_visible(query: &SharedIndexQueryRuntime, expected: bool) -> TestResult<()> {
    let page = query.execute_query(variant_graph_query()?).await?;
    let expected_rows = if expected { 1 } else { 0 };
    let expected_count = if expected { 1 } else { 0 };
    assert_eq!(page.items.len(), expected_rows);
    assert_eq!(page.exact_count, Some(expected_count));
    Ok(())
}

async fn assert_graph_payload(
    query: &SharedIndexQueryRuntime,
    expected_sku: &str,
) -> TestResult<()> {
    let page = query.execute_query(variant_graph_query()?).await?;
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.exact_count, Some(1));
    let variants_link = LinkName::new("variants")?;
    let variants = page.items[0]
        .nested_relations
        .iter()
        .find(|projection| projection.path == vec![variants_link.clone()])
        .ok_or_else(|| std::io::Error::other("variants nested projection is missing"))?;
    assert_eq!(variants.items.len(), 1);
    let sku_path = FieldPath::linked([variants_link], FieldName::new("sku")?);
    let sku = variants.items[0]
        .fields
        .iter()
        .find(|field| field.path == sku_path)
        .ok_or_else(|| std::io::Error::other("variants.sku projection is missing"))?;
    assert_eq!(&sku.value, &IndexValue::String(expected_sku.to_owned()));
    Ok(())
}

async fn update_variant_sku(db: &DatabaseConnection, sku: &str) -> TestResult<()> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE product_variants SET sku = $3 WHERE tenant_id = $1 AND id = $2",
            vec![TENANT_ID.into(), VARIANT_ID.into(), sku.to_owned().into()],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1);
    Ok(())
}

async fn materialized_variant_version(db: &DatabaseConnection) -> TestResult<u64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT CAST(source_version AS TEXT) AS source_version_text
FROM index_entities
WHERE tenant_id = $1
  AND module_name = 'rustok-product'
  AND entity_name = 'product_variant'
  AND schema_version = 2
  AND entity_id = $2
  AND locale_key = ''
  AND is_deleted = FALSE
"#,
            vec![TENANT_ID.into(), VARIANT_ID.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("materialized ProductVariant row is missing"))?;
    let value: String = row.try_get("", "source_version_text")?;
    Ok(value.parse()?)
}

async fn create_migration_prerequisites(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id UUID PRIMARY KEY
);
CREATE TABLE taxonomy_terms (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    UNIQUE (tenant_id, id)
);
CREATE TABLE oauth_apps (
    id UUID PRIMARY KEY
);
"#,
    )
    .await?;
    let manager = SchemaManager::new(db);
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn seed_owner_rows(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');

INSERT INTO channels (id, tenant_id, slug, name) VALUES
    ('{CHANNEL_ID}', '{TENANT_ID}', 'alpha', 'Alpha');

INSERT INTO products (id, tenant_id, metadata) VALUES
    ('{PRODUCT_ID}', '{TENANT_ID}', '{{"channel_visibility":{{"allowed_channel_slugs":["alpha"]}}}}'::jsonb);

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{PRODUCT_TRANSLATION_ID}', '{PRODUCT_ID}', '{TENANT_ID}', 'en', 'Replay Product', 'replay-product');

INSERT INTO product_variants (id, product_id, tenant_id, sku) VALUES
    ('{VARIANT_ID}', '{PRODUCT_ID}', '{TENANT_ID}', '{INITIAL_SKU}');
"#
    ))
    .await?;
    Ok(())
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
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
    application_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    db.execute_unprepared(&format!("SET application_name TO '{application_name}'"))
        .await?;
    Ok(db)
}
