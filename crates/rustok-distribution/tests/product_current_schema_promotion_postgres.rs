#![cfg(feature = "mod-product")]

use std::{env, error::Error, sync::Arc};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation, IndexQuery,
    IndexQueryExecutionError, IndexQueryPort, IndexQueryScope, IndexSchema,
    IndexSchemaReadinessError, IndexSchemaReadinessRequest, IndexSourceLoadRequest, IndexValue,
    LocaleKey, MutationApplyOutcome, MutationDelivery, Pagination, PersistedSchemaReadinessFailure,
    PostgresIndexQueryPort, PostgresIndexSchemaReadinessStore, PostgresMutationStore,
    PostgresSchemaRegistrationStore, SchemaRef, SchemaRegistry, SchemaVersion,
    SharedIndexQueryRuntime, SharedIndexSchemaRegistry, SharedIndexSourceRegistry,
    derive_index_schema_source_event_id, materialize_index_source_registry,
    materialize_postgres_index_query_runtime, materialize_postgres_index_sources,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PRODUCT_KEY4_PROMOTION_DATABASE_URL";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const PRODUCT_EVENT_DOMAIN: &str = "rustok-product.product-replay";
const TENANT_ID: Uuid = Uuid::from_u128(1);
const PRODUCT_ID: Uuid = Uuid::from_u128(101);
const TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const TITLE: &str = "Promoted Product";
const CURRENT_PRODUCT_SCHEMA_VERSION: u32 = 4;
const HISTORICAL_PRODUCT_SCHEMA_VERSION: u32 = 3;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    source: DatabaseConnection,
    mutation: DatabaseConnection,
    query: DatabaseConnection,
    restart_query: DatabaseConnection,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} (or RUSTOK_INDEX_TEST_DATABASE_URL / DATABASE_URL) is not set to a PostgreSQL URL; skipping Product key4 promotion packet"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_product_key4_promotion_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_product_key4_promotion_migration_{suffix}"),
        )
        .await?;
        create_product_migration_prerequisites(&migration).await?;
        ProductMigrator::up(&migration, None).await?;
        let manager = SchemaManager::new(&migration);
        for migration_step in IndexModule.migrations() {
            migration_step.up(&manager).await?;
        }
        seed_product(&migration).await?;
        migration.close().await?;

        Ok(Some(Self {
            control,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_key4_promotion_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_key4_promotion_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_key4_promotion_query_{suffix}"),
            )
            .await?,
            restart_query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_key4_promotion_restart_{suffix}"),
            )
            .await?,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.source.close().await?;
        self.mutation.close().await?;
        self.query.close().await?;
        self.restart_query.close().await?;
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

struct ProductIndexRuntime {
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    query: SharedIndexQueryRuntime,
    mutations: PostgresMutationStore,
    current_product_schema: IndexSchema,
    historical_product_schema: IndexSchema,
}

#[tokio::test]
async fn product_key4_stages_rebuilds_promotes_and_restarts_without_key3_runtime_compatibility()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_product_key4_promotion_scenario(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_product_key4_promotion_scenario(database: &TestDatabase) -> TestResult<()> {
    let runtime = staged_product_runtime(database).await?;
    let current_ref = runtime.current_product_schema.reference.clone();
    let historical_ref = runtime.historical_product_schema.reference.clone();

    assert_eq!(
        current_ref.version,
        SchemaVersion::new(CURRENT_PRODUCT_SCHEMA_VERSION)
    );
    assert_eq!(
        historical_ref.version,
        SchemaVersion::new(HISTORICAL_PRODUCT_SCHEMA_VERSION)
    );
    assert_eq!(
        schema_status(&database.query, &historical_ref).await?,
        "active"
    );
    assert_eq!(
        schema_status(&database.query, &current_ref).await?,
        "active"
    );

    // Rebuild one real Product row through the selected current Product source and the production mutation
    // store while both persisted contracts are still active. The runtime itself publishes only key4.
    let mutation = load_product_mutation(&runtime.sources, &current_ref).await?;
    let locale = LocaleKey::new("en")?;
    let source_version = mutation.source_version();
    let current_event_id = mutation.event_id();
    assert_eq!(
        current_event_id,
        derive_index_schema_source_event_id(
            PRODUCT_EVENT_DOMAIN,
            TENANT_ID,
            &current_ref,
            PRODUCT_ID,
            Some(&locale),
            source_version,
        )?
    );
    let historical_event_id = derive_index_schema_source_event_id(
        PRODUCT_EVENT_DOMAIN,
        TENANT_ID,
        &historical_ref,
        PRODUCT_ID,
        Some(&locale),
        source_version,
    )?;
    assert_ne!(current_event_id, historical_event_id);

    apply_product_mutation(&runtime, mutation).await?;
    assert_current_product_query(&runtime.query, &current_ref).await?;

    // Final authority transition uses the already-staged exact key4 contract. It must not reinsert key4,
    // rewrite historical materialization, or keep key3 authoritative.
    let schema_store = PostgresSchemaRegistrationStore::new(database.query.clone());
    let promoted = schema_store
        .register_current(TENANT_ID, &runtime.current_product_schema)
        .await?;
    assert_eq!(promoted.retired_schema_count(), 1);
    assert_eq!(
        schema_status(&database.query, &historical_ref).await?,
        "retired"
    );
    assert_eq!(
        schema_status(&database.query, &current_ref).await?,
        "active"
    );

    let repeated = schema_store
        .register_current(TENANT_ID, &runtime.current_product_schema)
        .await?;
    assert_eq!(repeated.retired_schema_count(), 0);

    // Build a test-only immutable probe registry containing the persisted historical contract plus every
    // current runtime schema. No historical Product source/factory is registered or selected.
    let probe_registry =
        historical_probe_registry(&runtime.schemas, runtime.historical_product_schema.clone())?;
    assert_historical_readiness_is_inactive(database, &probe_registry, &historical_ref).await?;
    assert_historical_query_is_inactive(database, probe_registry, &historical_ref).await?;

    // Current key4 remains queryable after lower-key retirement.
    assert_current_product_query(&runtime.query, &current_ref).await?;

    // Simulate a fresh process composition against the same persisted database. The rebuilt runtime must
    // publish only current Product key4 and must read the retained key4 materialization successfully.
    let restart_query = restart_current_product_query_runtime(database).await?;
    assert_current_product_query(&restart_query, &current_ref).await?;

    Ok(())
}

async fn staged_product_runtime(database: &TestDatabase) -> TestResult<ProductIndexRuntime> {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_channel::ChannelModule)
        .register(rustok_product::ProductModule);
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)?;

    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or_else(|| std::io::Error::other("Product schema registry is missing"))?;
    let current_product_schema = current_product_schema(&schemas)?;
    assert_eq!(
        current_product_schema.reference.version,
        SchemaVersion::new(CURRENT_PRODUCT_SCHEMA_VERSION)
    );
    let mut historical_product_schema = current_product_schema.clone();
    historical_product_schema.reference.version =
        SchemaVersion::new(HISTORICAL_PRODUCT_SCHEMA_VERSION);

    let schema_store = PostgresSchemaRegistrationStore::new(database.query.clone());
    schema_store
        .register(TENANT_ID, &historical_product_schema)
        .await?;
    for registered in schemas.registry().iter() {
        schema_store.register(TENANT_ID, &registered.schema).await?;
    }

    assert_eq!(
        schema_status(&database.query, &historical_product_schema.reference).await?,
        "active"
    );
    assert_eq!(
        schema_status(&database.query, &current_product_schema.reference).await?,
        "active"
    );

    materialize_postgres_index_sources(&mut extensions, database.source.clone())?;
    let sources = materialize_index_source_registry(&extensions)?
        .ok_or_else(|| std::io::Error::other("Product source registry is missing"))?;
    let query = materialize_postgres_index_query_runtime(&mut extensions, database.query.clone())?
        .ok_or_else(|| std::io::Error::other("Product query runtime is missing"))?;

    Ok(ProductIndexRuntime {
        sources,
        schemas,
        query,
        mutations: PostgresMutationStore::new(database.mutation.clone()),
        current_product_schema,
        historical_product_schema,
    })
}

async fn restart_current_product_query_runtime(
    database: &TestDatabase,
) -> TestResult<SharedIndexQueryRuntime> {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_channel::ChannelModule)
        .register(rustok_product::ProductModule);
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)?;
    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or_else(|| std::io::Error::other("restart Product schema registry is missing"))?;

    let product_schemas = schemas
        .registry()
        .iter()
        .filter(|registered| {
            registered.schema.reference.module.as_str() == "rustok-product"
                && registered.schema.reference.entity.as_str() == "product"
        })
        .collect::<Vec<_>>();
    assert_eq!(product_schemas.len(), 1);
    assert_eq!(
        product_schemas[0].schema.reference.version,
        SchemaVersion::new(CURRENT_PRODUCT_SCHEMA_VERSION)
    );

    materialize_postgres_index_query_runtime(&mut extensions, database.restart_query.clone())?
        .ok_or_else(|| std::io::Error::other("restart Product query runtime is missing").into())
}

fn current_product_schema(schemas: &SharedIndexSchemaRegistry) -> TestResult<IndexSchema> {
    let matches = schemas
        .registry()
        .iter()
        .filter(|registered| {
            registered.schema.reference.module.as_str() == "rustok-product"
                && registered.schema.reference.entity.as_str() == "product"
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(std::io::Error::other(format!(
            "expected exactly one current Product schema, found {}",
            matches.len()
        ))
        .into());
    }
    Ok(matches[0].schema.clone())
}

fn historical_probe_registry(
    current: &SharedIndexSchemaRegistry,
    historical_product_schema: IndexSchema,
) -> TestResult<Arc<SchemaRegistry>> {
    let mut schemas = current
        .registry()
        .iter()
        .map(|registered| registered.schema.clone())
        .collect::<Vec<_>>();
    schemas.push(historical_product_schema);
    let mut registry = SchemaRegistry::new();
    registry.register_batch(schemas)?;
    Ok(Arc::new(registry))
}

async fn assert_historical_readiness_is_inactive(
    database: &TestDatabase,
    probe_registry: &Arc<SchemaRegistry>,
    historical_ref: &SchemaRef,
) -> TestResult<()> {
    let readiness = PostgresIndexSchemaReadinessStore::new(database.query.clone());
    let request = IndexSchemaReadinessRequest::new(TENANT_ID, [historical_ref.clone()])?;
    let error = readiness
        .require(&request, probe_registry.as_ref())
        .await
        .expect_err("retired Product key3 must fail persisted readiness");
    match error {
        IndexSchemaReadinessError::NotReady { failures }
            if failures.len() == 1
                && failures[0].reference == *historical_ref
                && failures[0].reason == PersistedSchemaReadinessFailure::Inactive =>
        {
            Ok(())
        }
        other => Err(std::io::Error::other(format!(
            "expected inactive Product key3 readiness failure, got {other:?}"
        ))
        .into()),
    }
}

async fn assert_historical_query_is_inactive(
    database: &TestDatabase,
    probe_registry: Arc<SchemaRegistry>,
    historical_ref: &SchemaRef,
) -> TestResult<()> {
    let query = PostgresIndexQueryPort::new(database.query.clone(), probe_registry);
    let error = query
        .execute_query(product_identity_query(historical_ref)?)
        .await
        .expect_err("retired Product key3 must fail query readiness before page SQL");
    match error {
        IndexQueryExecutionError::SchemaNotReady { reference, reason }
            if reference == *historical_ref
                && reason == PersistedSchemaReadinessFailure::Inactive =>
        {
            Ok(())
        }
        other => Err(std::io::Error::other(format!(
            "expected inactive Product key3 query failure, got {other:?}"
        ))
        .into()),
    }
}

async fn load_product_mutation(
    sources: &SharedIndexSourceRegistry,
    current_ref: &SchemaRef,
) -> TestResult<IndexMutation> {
    let request = IndexSourceLoadRequest::new(vec![EntityKey {
        tenant_id: TENANT_ID,
        schema: current_ref.clone(),
        entity_id: PRODUCT_ID,
        locale: Some(LocaleKey::new("en")?),
    }])?;
    let mut mutations = sources.load(request).await?.into_mutations();
    if mutations.len() != 1 {
        return Err(std::io::Error::other(format!(
            "expected one current Product mutation, got {}",
            mutations.len()
        ))
        .into());
    }
    let mutation = mutations.remove(0);
    assert_eq!(mutation.key().schema, *current_ref);
    Ok(mutation)
}

async fn apply_product_mutation(
    runtime: &ProductIndexRuntime,
    mutation: IndexMutation,
) -> TestResult<()> {
    let expected_source_version = mutation.source_version();
    let delivery = MutationDelivery::from_event(PRODUCT_SOURCE, mutation)?;
    let outcome = runtime
        .mutations
        .apply(runtime.schemas.registry(), &delivery)
        .await?;
    match outcome {
        MutationApplyOutcome::Applied { source_version }
            if source_version == expected_source_version => Ok(()),
        other => Err(std::io::Error::other(format!(
            "expected Product key4 mutation version {expected_source_version} to apply, got {other:?}"
        ))
        .into()),
    }
}

async fn assert_current_product_query(
    query: &impl IndexQueryPort,
    current_ref: &SchemaRef,
) -> TestResult<()> {
    let page = query
        .execute_query(product_identity_query(current_ref)?)
        .await?;
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].entity_id, PRODUCT_ID);
    assert_eq!(projected_string(&page.items[0], "title")?, TITLE);
    assert_eq!(page.exact_count, Some(1));
    assert!(!page.has_more);
    Ok(())
}

fn product_identity_query(schema: &SchemaRef) -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT_ID,
            locale: Some(LocaleKey::new("en")?),
        },
        schema: schema.clone(),
        fields: vec![field_path("title")?],
        filter: Some(FilterExpr::Eq(
            field_path("id")?,
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

fn field_path(name: &str) -> TestResult<FieldPath> {
    Ok(FieldPath::new(FieldName::new(name)?))
}

fn projected_string<'a>(
    item: &'a rustok_index::IndexQueryItem,
    field: &str,
) -> TestResult<&'a str> {
    let path = field_path(field)?;
    let projected = item
        .fields
        .iter()
        .find(|projected| projected.path == path)
        .ok_or_else(|| std::io::Error::other(format!("missing projected field {field}")))?;
    match &projected.value {
        IndexValue::String(value) => Ok(value.as_str()),
        other => Err(std::io::Error::other(format!(
            "projected field {field} is not a string: {other:?}"
        ))
        .into()),
    }
}

async fn schema_status(db: &DatabaseConnection, reference: &SchemaRef) -> TestResult<String> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT status
FROM index_schemas
WHERE tenant_id = $1
  AND module_name = $2
  AND entity_name = $3
  AND schema_version = $4
"#,
            vec![
                TENANT_ID.into(),
                reference.module.as_str().to_owned().into(),
                reference.entity.as_str().to_owned().into(),
                i64::from(reference.version.get()).into(),
            ],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other(format!("missing persisted schema {reference}")))?;
    Ok(row.try_get("", "status")?)
}

async fn create_product_migration_prerequisites(db: &DatabaseConnection) -> TestResult<()> {
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
CREATE TABLE channel_index_identity_generations (
    tenant_id UUID PRIMARY KEY,
    generation BIGINT NOT NULL CHECK (generation > 0)
);
"#,
    )
    .await?;
    let manager = SchemaManager::new(db);
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn seed_product(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');

INSERT INTO products (id, tenant_id) VALUES ('{PRODUCT_ID}', '{TENANT_ID}');

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{TRANSLATION_ID}', '{PRODUCT_ID}', '{TENANT_ID}', 'en', '{TITLE}', 'promoted-product');

INSERT INTO product_sales_channel_index_relation_snapshots (
    tenant_id,
    product_id,
    relation_epoch,
    channel_ids
) VALUES ('{TENANT_ID}', '{PRODUCT_ID}', 1, '[]'::jsonb);

INSERT INTO product_sales_channel_index_relation_freshness_snapshots (
    tenant_id,
    product_id,
    relation_epoch,
    product_source_version,
    visibility_key,
    channel_identity_generation
)
SELECT
    product.tenant_id,
    product.id,
    relation.relation_epoch,
    product.index_revision,
    'all',
    0
FROM products product
JOIN LATERAL (
    SELECT relation_epoch
    FROM product_sales_channel_index_relation_snapshots relation
    WHERE relation.tenant_id = product.tenant_id
      AND relation.product_id = product.id
    ORDER BY relation.relation_epoch DESC
    LIMIT 1
) relation ON TRUE
WHERE product.tenant_id = '{TENANT_ID}';
"#
    ))
    .await?;
    Ok(())
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("RUSTOK_INDEX_TEST_DATABASE_URL"))
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
