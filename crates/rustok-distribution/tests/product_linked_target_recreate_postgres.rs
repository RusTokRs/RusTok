#![cfg(feature = "mod-product")]

use std::{env, error::Error};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation,
    IndexQuery, IndexQueryPort, IndexQueryScope, IndexSourceLoadRequest, IndexValue, LinkName,
    LocaleKey, ModuleName, MutationApplyOutcome, MutationDelivery, Pagination,
    PostgresMutationStore, PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion,
    SharedIndexQueryRuntime, SharedIndexSchemaRegistry, SharedIndexSourceRegistry,
    materialize_index_source_registry, materialize_postgres_index_query_runtime,
    materialize_postgres_index_sources,
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
const PRODUCT_ID: Uuid = Uuid::from_u128(101);
const PRODUCT_TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const VARIANT_ID: Uuid = Uuid::from_u128(201);
const CHANNEL_ID: Uuid = Uuid::from_u128(301);
const OLD_VARIANT_SKU: &str = "variant-before-recreate";
const NEW_VARIANT_SKU: &str = "variant-after-recreate";
const OLD_CHANNEL_NAME: &str = "Channel before recreate";
const NEW_CHANNEL_NAME: &str = "Channel after recreate";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    migration: DatabaseConnection,
    work: DatabaseConnection,
    source: DatabaseConnection,
    mutation: DatabaseConnection,
    query: DatabaseConnection,
    writer: DatabaseConnection,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping linked-target recreate harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_linked_target_recreate_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_linked_target_recreate_migration_{suffix}"),
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
                &format!("idx_linked_target_recreate_work_{suffix}"),
            )
            .await?,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_linked_target_recreate_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_linked_target_recreate_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_linked_target_recreate_query_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_linked_target_recreate_writer_{suffix}"),
            )
            .await?,
            schema_name,
        }))
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
async fn linked_targets_remain_revision_monotonic_and_graph_queries_fail_closed_across_recreate()
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
    run_scheduler_until_idle(&runtime.scheduler, 24).await?;

    materialize_current(&runtime, PRODUCT_SOURCE, product_key()?).await?;
    let old_variant_version =
        materialize_current(&runtime, PRODUCT_VARIANT_SOURCE, variant_key()?).await?;
    let old_channel_version =
        materialize_current(&runtime, SALES_CHANNEL_SOURCE, channel_key()?).await?;
    assert_scalar_product_visible(&runtime.query, true).await?;
    assert_graph_payloads(&runtime.query, &[OLD_VARIANT_SKU], &[OLD_CHANNEL_NAME]).await?;

    // ProductVariant delete -> recreate keeps the same UUID but must not reuse an old source version.
    // The target delete/current mutation is intentionally not delivered yet, so old target payload
    // remains physically materialized while the owner moves through retained tombstone history.
    delete_variant(&database.writer).await?;
    let variant_tombstone = variant_tombstone_version(&database.writer)
        .await?
        .ok_or_else(|| std::io::Error::other("ProductVariant tombstone was not retained"))?;
    assert!(variant_tombstone > old_variant_version);
    recreate_variant(&database.writer).await?;
    let recreated_variant_version = live_variant_revision(&database.writer).await?;
    assert!(recreated_variant_version > variant_tombstone);
    assert!(recreated_variant_version > old_variant_version);
    assert!(variant_tombstone_version(&database.writer).await?.is_none());

    // Variant membership delete/insert advances Product revision/projection. Refresh only Product and
    // keep the old Variant Index target row in place. Scalar Product authority is current, but a query
    // that actually traverses `variants` must fail closed instead of presenting authoritative empty
    // relation data while that current link target is unavailable.
    materialize_current(&runtime, PRODUCT_SOURCE, product_key()?).await?;
    assert_materialized_target_version(
        &database.mutation,
        "rustok-product",
        "product_variant",
        2,
        VARIANT_ID,
        old_variant_version,
    )
    .await?;
    assert_scalar_product_visible(&runtime.query, true).await?;
    assert_graph_query_visible(&runtime.query, false).await?;

    let applied_variant_version =
        materialize_current(&runtime, PRODUCT_VARIANT_SOURCE, variant_key()?).await?;
    assert_eq!(applied_variant_version, recreated_variant_version);
    assert_graph_payloads(&runtime.query, &[NEW_VARIANT_SKU], &[OLD_CHANNEL_NAME]).await?;

    // SalesChannel delete -> recreate likewise seeds live index_revision above the retained delete
    // tombstone. Product membership returns to the same Channel UUID before convergence. Product root
    // owner authority is initially stale on Channel generation, then freshness-only convergence makes
    // scalar Product queries current again while graph queries stay fail-closed until the target row is
    // current.
    let relation_before_channel_recreate = latest_relation_epoch(&database.writer).await?;
    let projection_before_channel_recreate = latest_projection_epoch(&database.writer).await?;
    let product_materialized_before_channel_recreate =
        materialized_product_version(&database.mutation).await?;
    let generation_before_channel_recreate = channel_generation(&database.writer).await?;

    delete_channel(&database.writer).await?;
    let channel_tombstone = channel_tombstone_version(&database.writer)
        .await?
        .ok_or_else(|| std::io::Error::other("SalesChannel tombstone was not retained"))?;
    assert!(channel_tombstone > old_channel_version);
    let generation_after_channel_delete = channel_generation(&database.writer).await?;
    assert!(generation_after_channel_delete > generation_before_channel_recreate);

    recreate_channel(&database.writer).await?;
    let recreated_channel_version = live_channel_revision(&database.writer).await?;
    assert!(recreated_channel_version > channel_tombstone);
    assert!(recreated_channel_version > old_channel_version);
    assert!(channel_tombstone_version(&database.writer).await?.is_none());
    let generation_after_channel_recreate = channel_generation(&database.writer).await?;
    assert!(generation_after_channel_recreate > generation_after_channel_delete);

    assert_scalar_product_visible(&runtime.query, false).await?;
    assert_graph_query_visible(&runtime.query, false).await?;
    run_scheduler_until_idle(&runtime.scheduler, 20).await?;
    assert_eq!(
        latest_relation_epoch(&database.writer).await?,
        relation_before_channel_recreate
    );
    assert_eq!(
        latest_projection_epoch(&database.writer).await?,
        projection_before_channel_recreate
    );
    assert_eq!(
        materialized_product_version(&database.mutation).await?,
        product_materialized_before_channel_recreate
    );
    assert_eq!(
        latest_freshness_generation(&database.writer).await?,
        generation_after_channel_recreate
    );
    assert_materialized_target_version(
        &database.mutation,
        "rustok-channel",
        "sales_channel",
        1,
        CHANNEL_ID,
        old_channel_version,
    )
    .await?;
    assert_scalar_product_visible(&runtime.query, true).await?;
    assert_graph_query_visible(&runtime.query, false).await?;

    let applied_channel_version =
        materialize_current(&runtime, SALES_CHANNEL_SOURCE, channel_key()?).await?;
    assert_eq!(applied_channel_version, recreated_channel_version);
    assert_graph_payloads(&runtime.query, &[NEW_VARIANT_SKU], &[NEW_CHANNEL_NAME]).await?;
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

fn channel_key() -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: channel_schema_ref()?,
        entity_id: CHANNEL_ID,
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

fn product_graph_query() -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT_ID,
            locale: Some(LocaleKey::new("en")?),
        },
        schema: product_schema_ref()?,
        fields: vec![
            FieldPath::new(FieldName::new("title")?),
            FieldPath::linked([LinkName::new("variants")?], FieldName::new("sku")?),
            FieldPath::linked([LinkName::new("sales_channels")?], FieldName::new("name")?),
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
    assert_query_visibility(
        query.execute_query(scalar_product_query()?).await?,
        expected,
    );
    Ok(())
}

async fn assert_graph_query_visible(
    query: &SharedIndexQueryRuntime,
    expected: bool,
) -> TestResult<()> {
    assert_query_visibility(query.execute_query(product_graph_query()?).await?, expected);
    Ok(())
}

fn assert_query_visibility(page: rustok_index::IndexQueryPage, expected: bool) {
    let expected_rows = if expected { 1 } else { 0 };
    let expected_count = if expected { 1 } else { 0 };
    assert_eq!(page.items.len(), expected_rows);
    assert_eq!(page.exact_count, Some(expected_count));
}

async fn assert_graph_payloads(
    query: &SharedIndexQueryRuntime,
    expected_variant_skus: &[&str],
    expected_channel_names: &[&str],
) -> TestResult<()> {
    let page = query.execute_query(product_graph_query()?).await?;
    assert_eq!(
        page.items.len(),
        1,
        "Product graph must be query-admissible"
    );
    assert_eq!(page.exact_count, Some(1));
    let item = &page.items[0];
    let variant_values = nested_strings(item, "variants", "sku")?;
    let channel_values = nested_strings(item, "sales_channels", "name")?;
    let expected_variant_values = expected_variant_skus
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let expected_channel_values = expected_channel_names
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(variant_values, expected_variant_values);
    assert_eq!(channel_values, expected_channel_values);
    Ok(())
}

fn nested_strings(
    item: &rustok_index::IndexQueryItem,
    link: &str,
    field: &str,
) -> TestResult<Vec<String>> {
    let link_name = LinkName::new(link)?;
    let field_path = FieldPath::linked([link_name.clone()], FieldName::new(field)?);
    let projection = item
        .nested_relations
        .iter()
        .find(|projection| projection.path == vec![link_name.clone()])
        .ok_or_else(|| std::io::Error::other(format!("missing nested projection {link}")))?;
    let values = projection
        .items
        .iter()
        .map(|nested| {
            let projected = nested
                .fields
                .iter()
                .find(|projected| projected.path == field_path)
                .ok_or_else(|| {
                    std::io::Error::other(format!("missing nested field {link}.{field}"))
                })?;
            match &projected.value {
                IndexValue::String(value) => Ok(value.clone()),
                other => Err(std::io::Error::other(format!(
                    "nested field {link}.{field} is not a string: {other:?}"
                ))),
            }
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    Ok(values)
}

async fn delete_variant(db: &DatabaseConnection) -> TestResult<()> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM product_variants WHERE tenant_id = $1 AND id = $2",
            vec![TENANT_ID.into(), VARIANT_ID.into()],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1);
    Ok(())
}

async fn recreate_variant(db: &DatabaseConnection) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO product_variants (id, product_id, tenant_id, sku) VALUES ($1, $2, $3, $4)",
        vec![
            VARIANT_ID.into(),
            PRODUCT_ID.into(),
            TENANT_ID.into(),
            NEW_VARIANT_SKU.to_owned().into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn delete_channel(db: &DatabaseConnection) -> TestResult<()> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM channels WHERE tenant_id = $1 AND id = $2",
            vec![TENANT_ID.into(), CHANNEL_ID.into()],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1);
    Ok(())
}

async fn recreate_channel(db: &DatabaseConnection) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO channels (id, tenant_id, slug, name) VALUES ($1, $2, 'alpha', $3)",
        vec![
            CHANNEL_ID.into(),
            TENANT_ID.into(),
            NEW_CHANNEL_NAME.to_owned().into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn live_variant_revision(db: &DatabaseConnection) -> TestResult<u64> {
    required_revision(
        db,
        "SELECT index_revision FROM product_variants WHERE tenant_id = $1 AND id = $2",
        "index_revision",
        VARIANT_ID,
        "live ProductVariant revision",
    )
    .await
}

async fn live_channel_revision(db: &DatabaseConnection) -> TestResult<u64> {
    required_revision(
        db,
        "SELECT index_revision FROM channels WHERE tenant_id = $1 AND id = $2",
        "index_revision",
        CHANNEL_ID,
        "live SalesChannel revision",
    )
    .await
}

async fn variant_tombstone_version(db: &DatabaseConnection) -> TestResult<Option<u64>> {
    optional_revision(
        db,
        "SELECT source_version FROM product_variant_index_tombstones WHERE tenant_id = $1 AND variant_id = $2",
        "source_version",
        VARIANT_ID,
    )
    .await
}

async fn channel_tombstone_version(db: &DatabaseConnection) -> TestResult<Option<u64>> {
    optional_revision(
        db,
        "SELECT source_version FROM channel_index_tombstones WHERE tenant_id = $1 AND channel_id = $2",
        "source_version",
        CHANNEL_ID,
    )
    .await
}

async fn required_revision(
    db: &DatabaseConnection,
    sql: &str,
    column: &str,
    entity_id: Uuid,
    label: &str,
) -> TestResult<u64> {
    optional_revision(db, sql, column, entity_id)
        .await?
        .ok_or_else(|| std::io::Error::other(format!("{label} is missing")).into())
}

async fn optional_revision(
    db: &DatabaseConnection,
    sql: &str,
    column: &str,
    entity_id: Uuid,
) -> TestResult<Option<u64>> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![TENANT_ID.into(), entity_id.into()],
        ))
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let value: i64 = row.try_get("", column)?;
    Ok(Some(u64::try_from(value)?))
}

async fn channel_generation(db: &DatabaseConnection) -> TestResult<u64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT generation FROM channel_index_identity_generations WHERE tenant_id = $1",
            vec![TENANT_ID.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Channel identity generation is missing"))?;
    let generation: i64 = row.try_get("", "generation")?;
    Ok(u64::try_from(generation)?)
}

async fn latest_relation_epoch(db: &DatabaseConnection) -> TestResult<u64> {
    product_epoch(
        db,
        "SELECT relation_epoch FROM product_sales_channel_index_relation_snapshots WHERE tenant_id = $1 AND product_id = $2 ORDER BY relation_epoch DESC LIMIT 1",
        "relation_epoch",
    )
    .await
}

async fn latest_projection_epoch(db: &DatabaseConnection) -> TestResult<u64> {
    product_epoch(
        db,
        "SELECT projection_epoch FROM product_index_graph_projection_snapshots WHERE tenant_id = $1 AND product_id = $2 ORDER BY projection_epoch DESC LIMIT 1",
        "projection_epoch",
    )
    .await
}

async fn latest_freshness_generation(db: &DatabaseConnection) -> TestResult<u64> {
    product_epoch(
        db,
        "SELECT channel_identity_generation FROM product_sales_channel_index_relation_freshness_snapshots WHERE tenant_id = $1 AND product_id = $2 ORDER BY sequence_no DESC LIMIT 1",
        "channel_identity_generation",
    )
    .await
}

async fn product_epoch(db: &DatabaseConnection, sql: &str, column: &str) -> TestResult<u64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![TENANT_ID.into(), PRODUCT_ID.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other(format!("{column} is missing")))?;
    let value: i64 = row.try_get("", column)?;
    Ok(u64::try_from(value)?)
}

async fn materialized_product_version(db: &DatabaseConnection) -> TestResult<u64> {
    materialized_target_version(db, "rustok-product", "product", 4, PRODUCT_ID).await
}

async fn assert_materialized_target_version(
    db: &DatabaseConnection,
    module_name: &str,
    entity_name: &str,
    schema_version: i64,
    entity_id: Uuid,
    expected_source_version: u64,
) -> TestResult<()> {
    assert_eq!(
        materialized_target_version(db, module_name, entity_name, schema_version, entity_id)
            .await?,
        expected_source_version
    );
    Ok(())
}

async fn materialized_target_version(
    db: &DatabaseConnection,
    module_name: &str,
    entity_name: &str,
    schema_version: i64,
    entity_id: Uuid,
) -> TestResult<u64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
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
    ('{CHANNEL_ID}', '{TENANT_ID}', 'alpha', '{OLD_CHANNEL_NAME}');

INSERT INTO products (id, tenant_id, metadata) VALUES
    ('{PRODUCT_ID}', '{TENANT_ID}', '{{}}'::jsonb);

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{PRODUCT_TRANSLATION_ID}', '{PRODUCT_ID}', '{TENANT_ID}', 'en', 'Linked target Product', 'linked-target-product');

INSERT INTO product_variants (id, product_id, tenant_id, sku) VALUES
    ('{VARIANT_ID}', '{PRODUCT_ID}', '{TENANT_ID}', '{OLD_VARIANT_SKU}');
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
