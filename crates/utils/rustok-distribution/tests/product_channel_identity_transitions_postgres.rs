#![cfg(feature = "mod-product")]

use std::{env, error::Error};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation,
    IndexQuery, IndexQueryPort, IndexQueryScope, IndexSourceLoadRequest, IndexValue, LocaleKey,
    ModuleName, MutationApplyOutcome, MutationDelivery, Pagination, PostgresMutationStore,
    PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion, SharedIndexQueryRuntime,
    SharedIndexSchemaRegistry, SharedIndexSourceRegistry, materialize_index_source_registry,
    materialize_postgres_index_query_runtime, materialize_postgres_index_sources,
};
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistrations, ModuleWorkScheduler};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const TENANT_A: Uuid = Uuid::from_u128(1);
const TENANT_B: Uuid = Uuid::from_u128(2);
const PRODUCT_A: Uuid = Uuid::from_u128(101);
const PRODUCT_B: Uuid = Uuid::from_u128(102);
const PRODUCT_A_TRANSLATION: Uuid = Uuid::from_u128(111);
const PRODUCT_B_TRANSLATION: Uuid = Uuid::from_u128(112);
const ALPHA_CHANNEL: Uuid = Uuid::from_u128(201);
const BETA_CHANNEL: Uuid = Uuid::from_u128(202);

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Channel identity transition harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_channel_identity_transitions_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_channel_identity_transitions_migration_{suffix}"),
        )
        .await?;
        create_migration_prerequisites(&migration).await?;
        let manager = SchemaManager::new(&migration);
        for migration_step in rustok_channel::migrations::migrations() {
            migration_step.up(&manager).await?;
        }
        for migration_step in rustok_product::migrations::migrations() {
            migration_step.up(&manager).await?;
        }
        for migration_step in IndexModule.migrations() {
            migration_step.up(&manager).await?;
        }
        seed_owner_rows(&migration).await?;

        Ok(Some(Self {
            control,
            migration,
            work: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_channel_identity_transitions_work_{suffix}"),
            )
            .await?,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_channel_identity_transitions_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_channel_identity_transitions_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_channel_identity_transitions_query_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_channel_identity_transitions_writer_{suffix}"),
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
async fn channel_identity_transitions_drive_exact_product_convergence() -> TestResult<()> {
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

    let baseline_a_generation = channel_generation(&database.writer, TENANT_A).await?;
    let baseline_b_generation = channel_generation(&database.writer, TENANT_B).await?;
    assert!(baseline_a_generation > 0);
    assert_eq!(baseline_b_generation, 0);
    assert_state_checkpoint(&database.writer, TENANT_A, baseline_a_generation).await?;
    assert_state_checkpoint(&database.writer, TENANT_B, baseline_b_generation).await?;
    assert_eq!(
        latest_membership(&database.writer, TENANT_A, PRODUCT_A)
            .await?
            .1,
        vec![ALPHA_CHANNEL]
    );
    assert!(
        latest_membership(&database.writer, TENANT_B, PRODUCT_B)
            .await?
            .1
            .is_empty()
    );

    materialize_current(&runtime, TENANT_A, PRODUCT_A).await?;
    materialize_current(&runtime, TENANT_B, PRODUCT_B).await?;
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, true).await?;
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, true).await?;

    // CREATE: adding beta changes tenant A unrestricted membership, so query freshness invalidates the
    // old Product row immediately. Convergence must advance relation/projection and a current Product
    // mutation is required before query authority returns.
    let a_relation_before_create = latest_membership(&database.writer, TENANT_A, PRODUCT_A)
        .await?
        .0;
    let a_projection_before_create =
        latest_projection(&database.writer, TENANT_A, PRODUCT_A).await?;
    insert_channel(&database.writer, TENANT_A, BETA_CHANNEL, "beta", "Beta").await?;
    let generation_after_create = channel_generation(&database.writer, TENANT_A).await?;
    assert!(generation_after_create > baseline_a_generation);
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, false).await?;
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, true).await?;
    run_scheduler_until_idle(&runtime.scheduler, 12).await?;
    let (a_relation_after_create, a_membership_after_create) =
        latest_membership(&database.writer, TENANT_A, PRODUCT_A).await?;
    let a_projection_after_create =
        latest_projection(&database.writer, TENANT_A, PRODUCT_A).await?;
    assert!(a_relation_after_create > a_relation_before_create);
    assert!(a_projection_after_create > a_projection_before_create);
    assert_eq!(a_membership_after_create, vec![ALPHA_CHANNEL, BETA_CHANNEL]);
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, false).await?;
    materialize_current(&runtime, TENANT_A, PRODUCT_A).await?;
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, true).await?;

    // DELETE: removing beta changes membership back to alpha. Again relation/projection must advance,
    // and the old materialized row must stay hidden until the current Product projection is applied.
    let a_relation_before_delete = a_relation_after_create;
    let a_projection_before_delete = a_projection_after_create;
    delete_channel(&database.writer, TENANT_A, BETA_CHANNEL).await?;
    let generation_after_delete = channel_generation(&database.writer, TENANT_A).await?;
    assert!(generation_after_delete > generation_after_create);
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, false).await?;
    run_scheduler_until_idle(&runtime.scheduler, 12).await?;
    let (a_relation_after_delete, a_membership_after_delete) =
        latest_membership(&database.writer, TENANT_A, PRODUCT_A).await?;
    let a_projection_after_delete =
        latest_projection(&database.writer, TENANT_A, PRODUCT_A).await?;
    assert!(a_relation_after_delete > a_relation_before_delete);
    assert!(a_projection_after_delete > a_projection_before_delete);
    assert_eq!(a_membership_after_delete, vec![ALPHA_CHANNEL]);
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, false).await?;
    materialize_current(&runtime, TENANT_A, PRODUCT_A).await?;
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, true).await?;

    // TENANT MOVE: moving alpha A -> B must advance both tenant generations. Product A loses alpha and
    // Product B gains the same Channel UUID. Both old rows are fail-closed until convergence and new
    // Product projections are materialized.
    let a_relation_before_move = a_relation_after_delete;
    let a_projection_before_move = a_projection_after_delete;
    let b_relation_before_move = latest_membership(&database.writer, TENANT_B, PRODUCT_B)
        .await?
        .0;
    let b_projection_before_move = latest_projection(&database.writer, TENANT_B, PRODUCT_B).await?;
    move_channel(&database.writer, ALPHA_CHANNEL, TENANT_A, TENANT_B).await?;
    let generation_a_after_move = channel_generation(&database.writer, TENANT_A).await?;
    let generation_b_after_move = channel_generation(&database.writer, TENANT_B).await?;
    assert!(generation_a_after_move > generation_after_delete);
    assert!(generation_b_after_move > baseline_b_generation);
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, false).await?;
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, false).await?;
    run_scheduler_until_idle(&runtime.scheduler, 20).await?;

    let (a_relation_after_move, a_membership_after_move) =
        latest_membership(&database.writer, TENANT_A, PRODUCT_A).await?;
    let a_projection_after_move = latest_projection(&database.writer, TENANT_A, PRODUCT_A).await?;
    let (b_relation_after_move, b_membership_after_move) =
        latest_membership(&database.writer, TENANT_B, PRODUCT_B).await?;
    let b_projection_after_move = latest_projection(&database.writer, TENANT_B, PRODUCT_B).await?;
    assert!(a_relation_after_move > a_relation_before_move);
    assert!(a_projection_after_move > a_projection_before_move);
    assert!(a_membership_after_move.is_empty());
    assert!(b_relation_after_move > b_relation_before_move);
    assert!(b_projection_after_move > b_projection_before_move);
    assert_eq!(b_membership_after_move, vec![ALPHA_CHANNEL]);
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, false).await?;
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, false).await?;
    materialize_current(&runtime, TENANT_A, PRODUCT_A).await?;
    materialize_current(&runtime, TENANT_B, PRODUCT_B).await?;
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, true).await?;
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, true).await?;

    // DELETE + RECREATE before convergence: current B membership returns to exactly the same alpha UUID.
    // Channel generation must still advance twice and hide the old Product row until a new freshness
    // witness is recorded. Because the final UUID set is unchanged, relation/projection must not move
    // and the exact same materialized Index row becomes admissible again without a Product mutation.
    let b_relation_before_recreate = b_relation_after_move;
    let b_projection_before_recreate = b_projection_after_move;
    let b_materialized_before_recreate =
        materialized_source_version(&database.mutation, TENANT_B, PRODUCT_B).await?;
    delete_channel(&database.writer, TENANT_B, ALPHA_CHANNEL).await?;
    let generation_after_identity_delete = channel_generation(&database.writer, TENANT_B).await?;
    insert_channel(
        &database.writer,
        TENANT_B,
        ALPHA_CHANNEL,
        "alpha",
        "Alpha recreated",
    )
    .await?;
    let generation_after_recreate = channel_generation(&database.writer, TENANT_B).await?;
    assert!(generation_after_identity_delete > generation_b_after_move);
    assert!(generation_after_recreate > generation_after_identity_delete);
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, false).await?;
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, true).await?;
    run_scheduler_until_idle(&runtime.scheduler, 16).await?;

    let (b_relation_after_recreate, b_membership_after_recreate) =
        latest_membership(&database.writer, TENANT_B, PRODUCT_B).await?;
    let b_projection_after_recreate =
        latest_projection(&database.writer, TENANT_B, PRODUCT_B).await?;
    assert_eq!(b_relation_after_recreate, b_relation_before_recreate);
    assert_eq!(b_projection_after_recreate, b_projection_before_recreate);
    assert_eq!(b_membership_after_recreate, vec![ALPHA_CHANNEL]);
    assert_freshness_generation(
        &database.writer,
        TENANT_B,
        PRODUCT_B,
        generation_after_recreate,
    )
    .await?;
    assert_eq!(
        materialized_source_version(&database.mutation, TENANT_B, PRODUCT_B).await?,
        b_materialized_before_recreate
    );
    assert_product_visible(&runtime.query, TENANT_B, PRODUCT_B, true).await?;
    assert_product_visible(&runtime.query, TENANT_A, PRODUCT_A, true).await?;
    assert_state_checkpoint(&database.writer, TENANT_A, generation_a_after_move).await?;
    assert_state_checkpoint(&database.writer, TENANT_B, generation_after_recreate).await?;
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
    for tenant_id in [TENANT_A, TENANT_B] {
        for registered in schemas.registry().iter() {
            schema_store.register(tenant_id, &registered.schema).await?;
        }
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
    tenant_id: Uuid,
    product_id: Uuid,
) -> TestResult<()> {
    let mutation = load_product_mutation(&runtime.sources, tenant_id, product_id).await?;
    let source_version = mutation.source_version();
    let delivery = MutationDelivery::from_event(PRODUCT_SOURCE, mutation)?;
    let outcome = runtime
        .mutations
        .apply(runtime.schemas.registry(), &delivery)
        .await?;
    match outcome {
        MutationApplyOutcome::Applied {
            source_version: applied,
        } if applied == source_version => Ok(()),
        other => Err(std::io::Error::other(format!(
            "expected Product mutation {source_version} to apply, got {other:?}"
        ))
        .into()),
    }
}

async fn load_product_mutation(
    sources: &SharedIndexSourceRegistry,
    tenant_id: Uuid,
    product_id: Uuid,
) -> TestResult<IndexMutation> {
    let request = IndexSourceLoadRequest::new(vec![product_key(tenant_id, product_id)?])?;
    let mut mutations = sources.load(request).await?.into_mutations();
    if mutations.len() != 1 {
        return Err(std::io::Error::other(format!(
            "expected one Product mutation, got {}",
            mutations.len()
        ))
        .into());
    }
    Ok(mutations.remove(0))
}

async fn assert_product_visible(
    query: &SharedIndexQueryRuntime,
    tenant_id: Uuid,
    product_id: Uuid,
    expected: bool,
) -> TestResult<()> {
    let page = query
        .execute_query(product_identity_query(tenant_id, product_id)?)
        .await?;
    let expected_rows = if expected { 1 } else { 0 };
    let expected_count = if expected { 1_u64 } else { 0_u64 };
    assert_eq!(page.items.len(), expected_rows);
    assert_eq!(page.exact_count, Some(expected_count));
    if expected {
        assert_eq!(page.items[0].entity_id, product_id);
    }
    Ok(())
}

fn product_identity_query(tenant_id: Uuid, product_id: Uuid) -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id,
            locale: Some(LocaleKey::new("en")?),
        },
        schema: product_schema_ref()?,
        fields: vec![FieldPath::new(FieldName::new("title")?)],
        filter: Some(FilterExpr::Eq(
            FieldPath::new(FieldName::new("id")?),
            IndexValue::Uuid(product_id),
        )),
        order_by: Vec::new(),
        pagination: Pagination::Offset {
            limit: 10,
            offset: 0,
        },
        include_exact_count: true,
    })
}

fn product_key(tenant_id: Uuid, product_id: Uuid) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id,
        schema: product_schema_ref()?,
        entity_id: product_id,
        locale: Some(LocaleKey::new("en")?),
    })
}

fn product_schema_ref() -> TestResult<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        version: SchemaVersion::new(4),
    })
}

async fn channel_generation(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT COALESCE(
    (SELECT generation FROM channel_index_identity_generations WHERE tenant_id = $1),
    0
)::bigint AS generation
"#,
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Channel generation query returned no row"))?;
    let generation: i64 = row.try_get("", "generation")?;
    Ok(u64::try_from(generation)?)
}

async fn latest_membership(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Uuid,
) -> TestResult<(u64, Vec<Uuid>)> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT relation_epoch, channel_ids
FROM product_sales_channel_index_relation_snapshots
WHERE tenant_id = $1 AND product_id = $2
ORDER BY relation_epoch DESC
LIMIT 1
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Product relation snapshot is missing"))?;
    let epoch: i64 = row.try_get("", "relation_epoch")?;
    let channel_ids: JsonValue = row.try_get("", "channel_ids")?;
    let mut decoded = channel_ids
        .as_array()
        .ok_or_else(|| std::io::Error::other("relation channel_ids is not an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| std::io::Error::other("relation channel id is not text"))
                .and_then(|value| Uuid::parse_str(value).map_err(std::io::Error::other))
        })
        .collect::<Result<Vec<_>, _>>()?;
    decoded.sort_unstable();
    Ok((u64::try_from(epoch)?, decoded))
}

async fn latest_projection(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Uuid,
) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT projection_epoch
FROM product_index_graph_projection_snapshots
WHERE tenant_id = $1 AND product_id = $2
ORDER BY projection_epoch DESC
LIMIT 1
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Product projection is missing"))?;
    let epoch: i64 = row.try_get("", "projection_epoch")?;
    Ok(u64::try_from(epoch)?)
}

async fn assert_freshness_generation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Uuid,
    expected_generation: u64,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT channel_identity_generation
FROM product_sales_channel_index_relation_freshness_snapshots
WHERE tenant_id = $1 AND product_id = $2
ORDER BY sequence_no DESC
LIMIT 1
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Product freshness witness is missing"))?;
    let generation: i64 = row.try_get("", "channel_identity_generation")?;
    assert_eq!(u64::try_from(generation)?, expected_generation);
    Ok(())
}

async fn materialized_source_version(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Uuid,
) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT CAST(source_version AS TEXT) AS source_version_text
FROM index_entities
WHERE tenant_id = $1
  AND module_name = 'rustok-product'
  AND entity_name = 'product'
  AND schema_version = 4
  AND entity_id = $2
  AND locale_key = 'en'
  AND is_deleted = FALSE
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("materialized Product row is missing"))?;
    let value: String = row.try_get("", "source_version_text")?;
    Ok(value.parse()?)
}

async fn assert_state_checkpoint(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    expected_generation: u64,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT channel_identity_generation, sweep_generation, sweep_after_product_id, lease_token
FROM product_sales_channel_index_relation_convergence_state
WHERE tenant_id = $1
"#,
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Product convergence state is missing"))?;
    let generation: Option<i64> = row.try_get("", "channel_identity_generation")?;
    let sweep_generation: Option<i64> = row.try_get("", "sweep_generation")?;
    let sweep_after: Option<Uuid> = row.try_get("", "sweep_after_product_id")?;
    let lease_token: Option<Uuid> = row.try_get("", "lease_token")?;
    assert_eq!(generation, Some(i64::try_from(expected_generation)?));
    assert!(sweep_generation.is_none());
    assert!(sweep_after.is_none());
    assert!(lease_token.is_none());
    Ok(())
}

async fn insert_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    channel_id: Uuid,
    slug: &str,
    name: &str,
) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO channels (id, tenant_id, slug, name) VALUES ($1, $2, $3, $4)",
        vec![
            channel_id.into(),
            tenant_id.into(),
            slug.to_owned().into(),
            name.to_owned().into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn delete_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    channel_id: Uuid,
) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "DELETE FROM channels WHERE tenant_id = $1 AND id = $2",
        vec![tenant_id.into(), channel_id.into()],
    ))
    .await?;
    Ok(())
}

async fn move_channel(
    db: &DatabaseConnection,
    channel_id: Uuid,
    from_tenant: Uuid,
    to_tenant: Uuid,
) -> TestResult<()> {
    let result = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE channels SET tenant_id = $3 WHERE tenant_id = $1 AND id = $2",
            vec![from_tenant.into(), channel_id.into(), to_tenant.into()],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1);
    Ok(())
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
INSERT INTO tenants (id) VALUES ('{TENANT_A}'), ('{TENANT_B}');

INSERT INTO channels (id, tenant_id, slug, name) VALUES
    ('{ALPHA_CHANNEL}', '{TENANT_A}', 'alpha', 'Alpha');

INSERT INTO products (id, tenant_id, metadata) VALUES
    ('{PRODUCT_A}', '{TENANT_A}', '{{}}'::jsonb),
    ('{PRODUCT_B}', '{TENANT_B}', '{{}}'::jsonb);

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{PRODUCT_A_TRANSLATION}', '{PRODUCT_A}', '{TENANT_A}', 'en', 'Tenant A Product', 'tenant-a-product'),
    ('{PRODUCT_B_TRANSLATION}', '{PRODUCT_B}', '{TENANT_B}', 'en', 'Tenant B Product', 'tenant-b-product');
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
