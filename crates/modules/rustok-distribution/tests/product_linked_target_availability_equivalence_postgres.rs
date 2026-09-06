#![cfg(feature = "mod-product")]

use std::{collections::BTreeSet, env, error::Error};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation,
    IndexQuery, IndexQueryPage, IndexQueryPort, IndexQueryScope, IndexSourceLoadRequest,
    IndexValue, LinkName, LocaleKey, ModuleName, MutationApplyOutcome, MutationDelivery,
    OrderDirection, OrderExpr, Pagination, PostgresMutationStore, PostgresSchemaRegistrationStore,
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
const SALES_CHANNEL_SOURCE: &str = "sales-channel-postgres-primary";

const TENANT_ID: Uuid = Uuid::from_u128(1);
const PRODUCT_A_ID: Uuid = Uuid::from_u128(101);
const PRODUCT_B_ID: Uuid = Uuid::from_u128(102);
const PRODUCT_A_TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const PRODUCT_B_TRANSLATION_ID: Uuid = Uuid::from_u128(112);
const VARIANT_A_ID: Uuid = Uuid::from_u128(201);
const VARIANT_B_ID: Uuid = Uuid::from_u128(202);
const CHANNEL_A_ID: Uuid = Uuid::from_u128(301);
const CHANNEL_B_ID: Uuid = Uuid::from_u128(302);

const VARIANT_A_OLD_SKU: &str = "middle-stale-sku";
const VARIANT_A_CURRENT_SKU: &str = "alpha-current-sku";
const VARIANT_B_SKU: &str = "zulu-stable-sku";
const CHANNEL_A_OLD_NAME: &str = "Middle stale channel";
const CHANNEL_A_CURRENT_NAME: &str = "Alpha current channel";
const CHANNEL_B_NAME: &str = "Zulu stable channel";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping linked-target availability equivalence harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_link_availability_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_link_availability_migration_{suffix}"),
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
                &format!("idx_link_availability_work_{suffix}"),
            )
            .await?,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_availability_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_availability_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_availability_query_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_link_availability_writer_{suffix}"),
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
            &format!("idx_link_availability_restart_query_{suffix}"),
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
async fn linked_target_availability_preserves_filter_order_count_and_runtime_restart_parity()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let result = run_scenarios(&database).await;
    let cleanup = database.cleanup().await;
    result?;
    cleanup
}

async fn run_scenarios(database: &TestDatabase) -> TestResult<()> {
    let runtime = build_runtime(database).await?;
    run_scheduler_until_idle(&runtime.scheduler, 40).await?;

    for product_id in [PRODUCT_A_ID, PRODUCT_B_ID] {
        materialize_current(&runtime, PRODUCT_SOURCE, product_key(product_id)?).await?;
    }
    let variant_a_materialized =
        materialize_current(&runtime, PRODUCT_VARIANT_SOURCE, variant_key(VARIANT_A_ID)?).await?;
    materialize_current(&runtime, PRODUCT_VARIANT_SOURCE, variant_key(VARIANT_B_ID)?).await?;
    let channel_a_materialized =
        materialize_current(&runtime, SALES_CHANNEL_SOURCE, channel_key(CHANNEL_A_ID)?).await?;
    materialize_current(&runtime, SALES_CHANNEL_SOURCE, channel_key(CHANNEL_B_ID)?).await?;

    assert_ids_unordered(
        runtime
            .query
            .execute_query(variant_stale_match_query()?)
            .await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
    assert_ids_ordered(
        runtime.query.execute_query(variant_order_query()?).await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
    assert_ids_unordered(
        runtime
            .query
            .execute_query(channel_stale_match_query()?)
            .await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
    assert_ids_ordered(
        runtime.query.execute_query(channel_order_query()?).await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );

    // A target-only ProductVariant update changes target source_version but not Product link membership
    // or Product projection. Keep the old Variant mutation physically materialized.
    update_variant_a_sku(&database.writer).await?;
    let variant_a_current = live_variant_revision(&database.writer, VARIANT_A_ID).await?;
    assert!(variant_a_current > variant_a_materialized);
    assert_eq!(
        materialized_target_version(
            &database.mutation,
            "rustok-product",
            "product_variant",
            2,
            VARIANT_A_ID,
        )
        .await?,
        variant_a_materialized
    );

    // The stale old SKU must not leak through linked filtering. Product B remains fully authoritative,
    // so page and exact count contain only B rather than hiding the whole query result.
    assert_ids_unordered(
        runtime
            .query
            .execute_query(variant_stale_match_query()?)
            .await?,
        &[PRODUCT_B_ID],
        1,
    );
    assert_ids_ordered(
        runtime.query.execute_query(variant_order_query()?).await?,
        &[PRODUCT_B_ID],
        1,
    );

    // Recompose the immutable query runtime on a fresh PostgreSQL session while the target is stale.
    // The same availability boundary must survive runtime restart without owner-specific recovery state.
    let restarted_query = database.fresh_query_runtime().await?;
    assert_ids_unordered(
        restarted_query
            .execute_query(variant_stale_match_query()?)
            .await?,
        &[PRODUCT_B_ID],
        1,
    );
    assert_ids_ordered(
        restarted_query
            .execute_query(variant_order_query()?)
            .await?,
        &[PRODUCT_B_ID],
        1,
    );

    let applied_variant =
        materialize_current(&runtime, PRODUCT_VARIANT_SOURCE, variant_key(VARIANT_A_ID)?).await?;
    assert_eq!(applied_variant, variant_a_current);
    assert_ids_unordered(
        runtime
            .query
            .execute_query(variant_current_match_query()?)
            .await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
    assert_ids_ordered(
        runtime.query.execute_query(variant_order_query()?).await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
    assert_ids_unordered(
        restarted_query
            .execute_query(variant_current_match_query()?)
            .await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );

    // Channel name changes bump SalesChannel index_revision but deliberately do not bump Channel
    // identity generation, so Product owner/relation freshness stays current. Availability alone must
    // fence the stale target payload.
    let generation_before_name_update = channel_generation(&database.writer).await?;
    update_channel_a_name(&database.writer).await?;
    let generation_after_name_update = channel_generation(&database.writer).await?;
    assert_eq!(generation_after_name_update, generation_before_name_update);
    let channel_a_current = live_channel_revision(&database.writer, CHANNEL_A_ID).await?;
    assert!(channel_a_current > channel_a_materialized);
    assert_eq!(
        materialized_target_version(
            &database.mutation,
            "rustok-channel",
            "sales_channel",
            1,
            CHANNEL_A_ID,
        )
        .await?,
        channel_a_materialized
    );

    assert_ids_unordered(
        runtime
            .query
            .execute_query(channel_stale_match_query()?)
            .await?,
        &[PRODUCT_B_ID],
        1,
    );
    assert_ids_ordered(
        runtime.query.execute_query(channel_order_query()?).await?,
        &[PRODUCT_B_ID],
        1,
    );

    let applied_channel =
        materialize_current(&runtime, SALES_CHANNEL_SOURCE, channel_key(CHANNEL_A_ID)?).await?;
    assert_eq!(applied_channel, channel_a_current);
    assert_ids_unordered(
        runtime
            .query
            .execute_query(channel_current_match_query()?)
            .await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
    assert_ids_ordered(
        runtime.query.execute_query(channel_order_query()?).await?,
        &[PRODUCT_A_ID, PRODUCT_B_ID],
        2,
    );
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

async fn materialize_current(
    runtime: &Runtime,
    source_name: &str,
    key: EntityKey,
) -> TestResult<u64> {
    let request = IndexSourceLoadRequest::new(vec![key])?;
    let mut mutations = runtime.sources.load(request).await?.into_mutations();
    if mutations.len() != 1 {
        return Err(std::io::Error::other(format!(
            "expected one mutation from {source_name}, got {}",
            mutations.len()
        ))
        .into());
    }
    let mutation: IndexMutation = mutations.remove(0);
    let source_version = mutation.source_version();
    let delivery = MutationDelivery::from_event(source_name, mutation)?;
    let outcome = runtime
        .mutations
        .apply(runtime.schemas.registry(), &delivery)
        .await?;
    match outcome {
        MutationApplyOutcome::Applied {
            source_version: applied,
        } if applied == source_version => Ok(source_version),
        other => Err(std::io::Error::other(format!(
            "expected {source_name} mutation {source_version} to apply, got {other:?}"
        ))
        .into()),
    }
}

fn product_key(product_id: Uuid) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: product_schema_ref()?,
        entity_id: product_id,
        locale: Some(LocaleKey::new("en")?),
    })
}

fn variant_key(variant_id: Uuid) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: variant_schema_ref()?,
        entity_id: variant_id,
        locale: None,
    })
}

fn channel_key(channel_id: Uuid) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: channel_schema_ref()?,
        entity_id: channel_id,
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

fn channel_schema_ref() -> TestResult<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-channel")?,
        entity: EntityName::new("sales_channel")?,
        version: SchemaVersion::INITIAL,
    })
}

fn variants_sku_path() -> TestResult<FieldPath> {
    Ok(FieldPath::linked(
        [LinkName::new("variants")?],
        FieldName::new("sku")?,
    ))
}

fn sales_channels_name_path() -> TestResult<FieldPath> {
    Ok(FieldPath::linked(
        [LinkName::new("sales_channels")?],
        FieldName::new("name")?,
    ))
}

fn base_product_query() -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT_ID,
            locale: Some(LocaleKey::new("en")?),
        },
        schema: product_schema_ref()?,
        fields: vec![
            FieldPath::new(FieldName::new("id")?),
            FieldPath::new(FieldName::new("title")?),
        ],
        filter: None,
        order_by: Vec::new(),
        pagination: Pagination::Offset {
            limit: 10,
            offset: 0,
        },
        include_exact_count: true,
    })
}

fn variant_stale_match_query() -> TestResult<IndexQuery> {
    let mut query = base_product_query()?;
    query.filter = Some(FilterExpr::In(
        variants_sku_path()?,
        vec![
            IndexValue::String(VARIANT_A_OLD_SKU.to_owned()),
            IndexValue::String(VARIANT_B_SKU.to_owned()),
        ],
    ));
    Ok(query)
}

fn variant_current_match_query() -> TestResult<IndexQuery> {
    let mut query = base_product_query()?;
    query.filter = Some(FilterExpr::In(
        variants_sku_path()?,
        vec![
            IndexValue::String(VARIANT_A_CURRENT_SKU.to_owned()),
            IndexValue::String(VARIANT_B_SKU.to_owned()),
        ],
    ));
    Ok(query)
}

fn variant_order_query() -> TestResult<IndexQuery> {
    let mut query = base_product_query()?;
    query.order_by = vec![OrderExpr {
        field: variants_sku_path()?,
        direction: OrderDirection::MinAsc,
    }];
    Ok(query)
}

fn channel_stale_match_query() -> TestResult<IndexQuery> {
    let mut query = base_product_query()?;
    query.filter = Some(FilterExpr::In(
        sales_channels_name_path()?,
        vec![
            IndexValue::String(CHANNEL_A_OLD_NAME.to_owned()),
            IndexValue::String(CHANNEL_B_NAME.to_owned()),
        ],
    ));
    Ok(query)
}

fn channel_current_match_query() -> TestResult<IndexQuery> {
    let mut query = base_product_query()?;
    query.filter = Some(FilterExpr::In(
        sales_channels_name_path()?,
        vec![
            IndexValue::String(CHANNEL_A_CURRENT_NAME.to_owned()),
            IndexValue::String(CHANNEL_B_NAME.to_owned()),
        ],
    ));
    Ok(query)
}

fn channel_order_query() -> TestResult<IndexQuery> {
    let mut query = base_product_query()?;
    query.order_by = vec![OrderExpr {
        field: sales_channels_name_path()?,
        direction: OrderDirection::MinAsc,
    }];
    Ok(query)
}

fn assert_ids_unordered(page: IndexQueryPage, expected: &[Uuid], exact_count: u64) {
    let actual = page
        .items
        .iter()
        .map(|item| item.entity_id)
        .collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(page.exact_count, Some(exact_count));
}

fn assert_ids_ordered(page: IndexQueryPage, expected: &[Uuid], exact_count: u64) {
    let actual = page
        .items
        .iter()
        .map(|item| item.entity_id)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(page.exact_count, Some(exact_count));
}

async fn update_variant_a_sku(db: &DatabaseConnection) -> TestResult<()> {
    let result = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE product_variants SET sku = $3 WHERE tenant_id = $1 AND id = $2",
            vec![
                TENANT_ID.into(),
                VARIANT_A_ID.into(),
                VARIANT_A_CURRENT_SKU.to_owned().into(),
            ],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1);
    Ok(())
}

async fn update_channel_a_name(db: &DatabaseConnection) -> TestResult<()> {
    let result = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE channels SET name = $3 WHERE tenant_id = $1 AND id = $2",
            vec![
                TENANT_ID.into(),
                CHANNEL_A_ID.into(),
                CHANNEL_A_CURRENT_NAME.to_owned().into(),
            ],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1);
    Ok(())
}

async fn live_variant_revision(db: &DatabaseConnection, variant_id: Uuid) -> TestResult<u64> {
    live_revision(
        db,
        "SELECT index_revision FROM product_variants WHERE tenant_id = $1 AND id = $2",
        variant_id,
        "ProductVariant",
    )
    .await
}

async fn live_channel_revision(db: &DatabaseConnection, channel_id: Uuid) -> TestResult<u64> {
    live_revision(
        db,
        "SELECT index_revision FROM channels WHERE tenant_id = $1 AND id = $2",
        channel_id,
        "SalesChannel",
    )
    .await
}

async fn live_revision(
    db: &DatabaseConnection,
    sql: &str,
    entity_id: Uuid,
    label: &str,
) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![TENANT_ID.into(), entity_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other(format!("{label} owner row is missing")))?;
    let revision: i64 = row.try_get("", "index_revision")?;
    Ok(u64::try_from(revision)?)
}

async fn channel_generation(db: &DatabaseConnection) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT generation FROM channel_index_identity_generations WHERE tenant_id = $1",
            vec![TENANT_ID.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Channel identity generation is missing"))?;
    let generation: i64 = row.try_get("", "generation")?;
    Ok(u64::try_from(generation)?)
}

async fn materialized_target_version(
    db: &DatabaseConnection,
    module_name: &str,
    entity_name: &str,
    schema_version: i64,
    entity_id: Uuid,
) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT CAST(source_version AS TEXT) AS source_version_text
FROM index_entities
WHERE tenant_id = $1
  AND module_name = $2
  AND entity_name = $3
  AND schema_version = $4
  AND entity_id = $5
  AND is_deleted = FALSE
"#,
            vec![
                TENANT_ID.into(),
                module_name.to_owned().into(),
                entity_name.to_owned().into(),
                schema_version.into(),
                entity_id.into(),
            ],
        ))
        .await?
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "materialized {module_name}/{entity_name} row is missing"
            ))
        })?;
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
    ('{CHANNEL_A_ID}', '{TENANT_ID}', 'alpha', '{CHANNEL_A_OLD_NAME}'),
    ('{CHANNEL_B_ID}', '{TENANT_ID}', 'beta', '{CHANNEL_B_NAME}');

INSERT INTO products (id, tenant_id, metadata) VALUES
    ('{PRODUCT_A_ID}', '{TENANT_ID}', '{{"channel_visibility":{{"allowed_channel_slugs":["alpha"]}}}}'::jsonb),
    ('{PRODUCT_B_ID}', '{TENANT_ID}', '{{"channel_visibility":{{"allowed_channel_slugs":["beta"]}}}}'::jsonb);

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{PRODUCT_A_TRANSLATION_ID}', '{PRODUCT_A_ID}', '{TENANT_ID}', 'en', 'Product A', 'product-a'),
    ('{PRODUCT_B_TRANSLATION_ID}', '{PRODUCT_B_ID}', '{TENANT_ID}', 'en', 'Product B', 'product-b');

INSERT INTO product_variants (id, product_id, tenant_id, sku) VALUES
    ('{VARIANT_A_ID}', '{PRODUCT_A_ID}', '{TENANT_ID}', '{VARIANT_A_OLD_SKU}'),
    ('{VARIANT_B_ID}', '{PRODUCT_B_ID}', '{TENANT_ID}', '{VARIANT_B_SKU}');
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
