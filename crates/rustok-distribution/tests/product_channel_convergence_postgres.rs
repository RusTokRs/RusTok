#![cfg(feature = "mod-product")]

use std::{env, error::Error, time::Duration};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation, IndexQuery,
    IndexQueryPort, IndexQueryScope, IndexSourceLoadRequest, IndexValue, LocaleKey, ModuleName,
    MutationApplyOutcome, MutationDelivery, Pagination, PostgresMutationStore,
    PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion, SharedIndexQueryRuntime,
    SharedIndexSchemaRegistry, SharedIndexSourceRegistry, materialize_index_source_registry,
    materialize_postgres_index_query_runtime, materialize_postgres_index_sources,
};
use rustok_product::{
    ProductSalesChannelIndexRelationConvergenceClaimOutcome,
    ProductSalesChannelIndexRelationConvergenceStore,
    ProductSalesChannelIndexRelationConvergenceWork,
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
const TENANT_ID: Uuid = Uuid::from_u128(1);
const UNRESTRICTED_PRODUCT_ID: Uuid = Uuid::from_u128(101);
const MALFORMED_PRODUCT_ID: Uuid = Uuid::from_u128(102);
const VALID_AFTER_MALFORMED_PRODUCT_ID: Uuid = Uuid::from_u128(103);
const RESTRICTED_PRODUCT_ID: Uuid = Uuid::from_u128(104);
const ALPHA_CHANNEL_ID: Uuid = Uuid::from_u128(201);
const BETA_CHANNEL_ID: Uuid = Uuid::from_u128(202);
const UNRESTRICTED_TRANSLATION_ID: Uuid = Uuid::from_u128(301);
const MALFORMED_TRANSLATION_ID: Uuid = Uuid::from_u128(302);
const VALID_AFTER_MALFORMED_TRANSLATION_ID: Uuid = Uuid::from_u128(303);
const RESTRICTED_TRANSLATION_ID: Uuid = Uuid::from_u128(304);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    migration: DatabaseConnection,
    host_a: DatabaseConnection,
    host_b: DatabaseConnection,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product/Channel convergence harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_product_channel_convergence_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_product_channel_convergence_migration_{suffix}"),
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
            host_a: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_channel_convergence_host_a_{suffix}"),
            )
            .await?,
            host_b: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_channel_convergence_host_b_{suffix}"),
            )
            .await?,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_channel_convergence_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_channel_convergence_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_channel_convergence_query_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_channel_convergence_writer_{suffix}"),
            )
            .await?,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.migration.close().await?;
        self.host_a.close().await?;
        self.host_b.close().await?;
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

struct ProductRuntime {
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    query: SharedIndexQueryRuntime,
    mutations: PostgresMutationStore,
    scheduler_a: ModuleWorkScheduler,
    scheduler_b: ModuleWorkScheduler,
}

#[tokio::test]
async fn product_channel_convergence_is_restartable_and_query_fenced() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let result = run_scenarios(&database).await;
    let cleanup = database.cleanup().await;
    result?;
    cleanup
}

async fn run_scenarios(database: &TestDatabase) -> TestResult<()> {
    let runtime = build_product_runtime(database).await?;
    let initial_generation = current_channel_generation(&database.writer).await?;
    assert!(initial_generation > 0);

    // Host A claims through the same Product-owned store used by the registered ModuleWork source,
    // then disappears before handler execution. Host B must not claim while the lease is live and
    // must reclaim the exact durable request after expiry without losing tenant progress.
    let store_a = ProductSalesChannelIndexRelationConvergenceStore::new(database.host_a.clone());
    let first_claim = match store_a
        .claim(TENANT_ID, initial_generation, Duration::from_secs(1))
        .await?
    {
        ProductSalesChannelIndexRelationConvergenceClaimOutcome::Claimed(claim) => claim,
        other => {
            return Err(std::io::Error::other(format!(
                "expected initial convergence claim, got {other:?}"
            ))
            .into());
        }
    };
    let first_sequence = match first_claim.work() {
        ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest { sequence_no, .. } => {
            *sequence_no
        }
        other => {
            return Err(std::io::Error::other(format!(
                "expected first Product visibility request, got {other:?}"
            ))
            .into());
        }
    };
    assert_eq!(runtime.scheduler_b.run_once().await?, 0);
    assert_active_lease(&database.writer, first_claim.lease_token(), 1).await?;
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert_eq!(runtime.scheduler_b.run_once().await?, 1);
    assert_reclaimed_progress(&database.writer, first_sequence).await?;

    // Finish all initial Product requests plus the Channel-generation baseline sweep through the
    // real generic scheduler. The malformed Product sorts before a valid Product in tenant sweep
    // order; valid convergence must continue even though malformed owner visibility stays fail-closed.
    run_scheduler_until_idle(&runtime.scheduler_b, 32).await?;
    let baseline_generation = current_channel_generation(&database.writer).await?;
    assert_eq!(baseline_generation, initial_generation);
    assert_state_checkpoint(&database.writer, baseline_generation).await?;
    assert_no_relation_or_freshness(&database.writer, MALFORMED_PRODUCT_ID).await?;
    assert_freshness_generation(
        &database.writer,
        VALID_AFTER_MALFORMED_PRODUCT_ID,
        baseline_generation,
    )
    .await?;

    assert_eq!(
        latest_relation_membership(&database.writer, UNRESTRICTED_PRODUCT_ID).await?.1,
        vec![ALPHA_CHANNEL_ID, BETA_CHANNEL_ID]
    );
    assert_eq!(
        latest_relation_membership(&database.writer, RESTRICTED_PRODUCT_ID).await?.1,
        vec![ALPHA_CHANNEL_ID]
    );

    // Materialize only query-valid baseline Products. The malformed Product deliberately has no
    // Product relation/freshness evidence and is never allowed to become an Index authority.
    for product_id in [UNRESTRICTED_PRODUCT_ID, VALID_AFTER_MALFORMED_PRODUCT_ID] {
        let mutation = load_product_mutation(&runtime.sources, product_id).await?;
        apply_product_mutation(&runtime, mutation).await?;
        assert_product_visible(&runtime.query, product_id, true).await?;
    }

    // Visibility race: read a valid restricted Product mutation, then switch alpha -> beta before
    // applying it. The old mutation is physically accepted but root query admission must hide it.
    let delayed_visibility = load_product_mutation(&runtime.sources, RESTRICTED_PRODUCT_ID).await?;
    let delayed_visibility_version = delayed_visibility.source_version();
    let (before_visibility_relation, _) =
        latest_relation_membership(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    let before_visibility_projection =
        latest_projection_epoch(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    update_restricted_visibility(&database.writer, "beta").await?;
    assert!(
        latest_projection_epoch(&database.writer, RESTRICTED_PRODUCT_ID).await?
            > delayed_visibility_version
    );
    apply_product_mutation(&runtime, delayed_visibility).await?;
    assert_materialized_source_version(
        &database.mutation,
        RESTRICTED_PRODUCT_ID,
        delayed_visibility_version,
    )
    .await?;
    assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, false).await?;

    // Host A can resume the same durable queue after Host B performed restart recovery.
    run_scheduler_until_idle(&runtime.scheduler_a, 16).await?;
    let (after_visibility_relation, visibility_membership) =
        latest_relation_membership(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    let after_visibility_projection =
        latest_projection_epoch(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    assert!(after_visibility_relation > before_visibility_relation);
    assert!(after_visibility_projection > before_visibility_projection);
    assert_eq!(visibility_membership, vec![BETA_CHANNEL_ID]);
    assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, false).await?;
    let current_restricted = load_product_mutation(&runtime.sources, RESTRICTED_PRODUCT_ID).await?;
    assert!(current_restricted.source_version() > delayed_visibility_version);
    apply_product_mutation(&runtime, current_restricted).await?;
    assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, true).await?;

    // Channel identity generation changes while unrestricted UUID membership stays identical. The
    // query fence hides the already-materialized Product until convergence refreshes the witness.
    // Relation/projection clocks must not move, and the same Index mutation becomes admissible again.
    let unrestricted_relation_before =
        latest_relation_membership(&database.writer, UNRESTRICTED_PRODUCT_ID).await?.0;
    let unrestricted_projection_before =
        latest_projection_epoch(&database.writer, UNRESTRICTED_PRODUCT_ID).await?;
    rename_channel(&database.writer, ALPHA_CHANNEL_ID, "alpha-renamed").await?;
    let generation_after_alpha_rename = current_channel_generation(&database.writer).await?;
    assert!(generation_after_alpha_rename > baseline_generation);
    assert_product_visible(&runtime.query, UNRESTRICTED_PRODUCT_ID, false).await?;
    run_scheduler_until_idle(&runtime.scheduler_b, 16).await?;
    assert_eq!(
        latest_relation_membership(&database.writer, UNRESTRICTED_PRODUCT_ID).await?.0,
        unrestricted_relation_before
    );
    assert_eq!(
        latest_projection_epoch(&database.writer, UNRESTRICTED_PRODUCT_ID).await?,
        unrestricted_projection_before
    );
    assert_freshness_generation(
        &database.writer,
        UNRESTRICTED_PRODUCT_ID,
        generation_after_alpha_rename,
    )
    .await?;
    assert_freshness_generation(
        &database.writer,
        VALID_AFTER_MALFORMED_PRODUCT_ID,
        generation_after_alpha_rename,
    )
    .await?;
    assert_no_relation_or_freshness(&database.writer, MALFORMED_PRODUCT_ID).await?;
    assert_product_visible(&runtime.query, UNRESTRICTED_PRODUCT_ID, true).await?;

    // A second Channel rename removes beta from the restricted visibility result. Convergence must
    // advance relation/projection for the restricted Product while leaving unrestricted membership
    // unchanged. The old restricted Index row stays hidden until its new projection is applied.
    let restricted_relation_before_beta =
        latest_relation_membership(&database.writer, RESTRICTED_PRODUCT_ID).await?.0;
    let restricted_projection_before_beta =
        latest_projection_epoch(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    let unrestricted_relation_before_beta =
        latest_relation_membership(&database.writer, UNRESTRICTED_PRODUCT_ID).await?.0;
    let unrestricted_projection_before_beta =
        latest_projection_epoch(&database.writer, UNRESTRICTED_PRODUCT_ID).await?;
    rename_channel(&database.writer, BETA_CHANNEL_ID, "beta-renamed").await?;
    let generation_after_beta_rename = current_channel_generation(&database.writer).await?;
    assert!(generation_after_beta_rename > generation_after_alpha_rename);
    assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, false).await?;
    run_scheduler_until_idle(&runtime.scheduler_a, 16).await?;

    let (restricted_relation_after_beta, restricted_membership_after_beta) =
        latest_relation_membership(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    let restricted_projection_after_beta =
        latest_projection_epoch(&database.writer, RESTRICTED_PRODUCT_ID).await?;
    assert!(restricted_relation_after_beta > restricted_relation_before_beta);
    assert!(restricted_projection_after_beta > restricted_projection_before_beta);
    assert!(restricted_membership_after_beta.is_empty());
    assert_eq!(
        latest_relation_membership(&database.writer, UNRESTRICTED_PRODUCT_ID).await?.0,
        unrestricted_relation_before_beta
    );
    assert_eq!(
        latest_projection_epoch(&database.writer, UNRESTRICTED_PRODUCT_ID).await?,
        unrestricted_projection_before_beta
    );
    assert_freshness_generation(
        &database.writer,
        UNRESTRICTED_PRODUCT_ID,
        generation_after_beta_rename,
    )
    .await?;
    assert_product_visible(&runtime.query, UNRESTRICTED_PRODUCT_ID, true).await?;
    assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, false).await?;

    let current_after_beta = load_product_mutation(&runtime.sources, RESTRICTED_PRODUCT_ID).await?;
    assert_eq!(current_after_beta.source_version(), restricted_projection_after_beta);
    apply_product_mutation(&runtime, current_after_beta).await?;
    assert_product_visible(&runtime.query, RESTRICTED_PRODUCT_ID, true).await?;
    assert_state_checkpoint(&database.writer, generation_after_beta_rename).await?;
    Ok(())
}

async fn build_product_runtime(database: &TestDatabase) -> TestResult<ProductRuntime> {
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
        .ok_or_else(|| std::io::Error::other("Product convergence work is not registered"))?;
    let scheduler_a = ModuleWorkScheduler::new();
    let scheduler_b = ModuleWorkScheduler::new();
    registrations
        .register_all(&HostRuntimeContext::new(database.host_a.clone()), &scheduler_a)
        .await?;
    registrations
        .register_all(&HostRuntimeContext::new(database.host_b.clone()), &scheduler_b)
        .await?;
    Ok(ProductRuntime {
        sources,
        schemas,
        query,
        mutations: PostgresMutationStore::new(database.mutation.clone()),
        scheduler_a,
        scheduler_b,
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

async fn load_product_mutation(
    sources: &SharedIndexSourceRegistry,
    product_id: Uuid,
) -> TestResult<IndexMutation> {
    let request = IndexSourceLoadRequest::new(vec![product_key(product_id)?])?;
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

async fn apply_product_mutation(runtime: &ProductRuntime, mutation: IndexMutation) -> TestResult<()> {
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

async fn assert_product_visible(
    query: &SharedIndexQueryRuntime,
    product_id: Uuid,
    expected: bool,
) -> TestResult<()> {
    let page = query.execute_query(product_identity_query(product_id)?).await?;
    let expected_rows = if expected { 1 } else { 0 };
    let expected_count = if expected { 1_u64 } else { 0_u64 };
    assert_eq!(page.items.len(), expected_rows);
    assert_eq!(page.exact_count, Some(expected_count));
    if expected {
        assert_eq!(page.items[0].entity_id, product_id);
    }
    Ok(())
}

fn product_identity_query(product_id: Uuid) -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: IndexQueryScope {
            tenant_id: TENANT_ID,
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

fn product_key(product_id: Uuid) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
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

async fn current_channel_generation(db: &DatabaseConnection) -> TestResult<u64> {
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

async fn latest_relation_membership(
    db: &DatabaseConnection,
    product_id: Uuid,
) -> TestResult<(u64, Vec<Uuid>)> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT relation_epoch, channel_ids
FROM product_sales_channel_index_relation_snapshots
WHERE tenant_id = $1 AND product_id = $2
ORDER BY relation_epoch DESC
LIMIT 1
"#,
            vec![TENANT_ID.into(), product_id.into()],
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

async fn latest_projection_epoch(db: &DatabaseConnection, product_id: Uuid) -> TestResult<u64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT projection_epoch
FROM product_index_graph_projection_snapshots
WHERE tenant_id = $1 AND product_id = $2
ORDER BY projection_epoch DESC
LIMIT 1
"#,
            vec![TENANT_ID.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Product projection is missing"))?;
    let epoch: i64 = row.try_get("", "projection_epoch")?;
    Ok(u64::try_from(epoch)?)
}

async fn assert_freshness_generation(
    db: &DatabaseConnection,
    product_id: Uuid,
    expected_generation: u64,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT channel_identity_generation
FROM product_sales_channel_index_relation_freshness_snapshots
WHERE tenant_id = $1 AND product_id = $2
ORDER BY sequence_no DESC
LIMIT 1
"#,
            vec![TENANT_ID.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("Product freshness witness is missing"))?;
    let generation: i64 = row.try_get("", "channel_identity_generation")?;
    assert_eq!(u64::try_from(generation)?, expected_generation);
    Ok(())
}

async fn assert_no_relation_or_freshness(
    db: &DatabaseConnection,
    product_id: Uuid,
) -> TestResult<()> {
    for table in [
        "product_sales_channel_index_relation_snapshots",
        "product_sales_channel_index_relation_freshness_snapshots",
    ] {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                format!(
                    "SELECT COUNT(*)::bigint AS row_count FROM {table} WHERE tenant_id = $1 AND product_id = $2"
                ),
                vec![TENANT_ID.into(), product_id.into()],
            ))
            .await?
            .ok_or_else(|| std::io::Error::other("count query returned no row"))?;
        let count: i64 = row.try_get("", "row_count")?;
        assert_eq!(count, 0, "malformed Product unexpectedly wrote {table}");
    }
    Ok(())
}

async fn assert_active_lease(
    db: &DatabaseConnection,
    expected_token: Uuid,
    expected_attempt_count: i64,
) -> TestResult<()> {
    let row = convergence_state(db).await?;
    let lease_token: Option<Uuid> = row.try_get("", "lease_token")?;
    let attempt_count: i64 = row.try_get("", "attempt_count")?;
    assert_eq!(lease_token, Some(expected_token));
    assert_eq!(attempt_count, expected_attempt_count);
    Ok(())
}

async fn assert_reclaimed_progress(db: &DatabaseConnection, first_sequence: i64) -> TestResult<()> {
    let row = convergence_state(db).await?;
    let visibility_cursor: i64 = row.try_get("", "visibility_cursor")?;
    let lease_token: Option<Uuid> = row.try_get("", "lease_token")?;
    let attempt_count: i64 = row.try_get("", "attempt_count")?;
    assert_eq!(visibility_cursor, first_sequence);
    assert!(lease_token.is_none());
    assert_eq!(attempt_count, 2);
    Ok(())
}

async fn assert_state_checkpoint(db: &DatabaseConnection, generation: u64) -> TestResult<()> {
    let row = convergence_state(db).await?;
    let channel_generation: Option<i64> = row.try_get("", "channel_identity_generation")?;
    let sweep_generation: Option<i64> = row.try_get("", "sweep_generation")?;
    let sweep_after: Option<Uuid> = row.try_get("", "sweep_after_product_id")?;
    let lease_token: Option<Uuid> = row.try_get("", "lease_token")?;
    assert_eq!(channel_generation, Some(i64::try_from(generation)?));
    assert!(sweep_generation.is_none());
    assert!(sweep_after.is_none());
    assert!(lease_token.is_none());
    Ok(())
}

async fn convergence_state(db: &DatabaseConnection) -> TestResult<sea_orm::QueryResult> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT visibility_cursor, channel_identity_generation, sweep_generation, sweep_after_product_id,
       lease_token, lease_expires_at, attempt_count, last_error
FROM product_sales_channel_index_relation_convergence_state
WHERE tenant_id = $1
"#,
        vec![TENANT_ID.into()],
    ))
    .await?
    .ok_or_else(|| std::io::Error::other("Product convergence state is missing").into())
}

async fn assert_materialized_source_version(
    db: &DatabaseConnection,
    product_id: Uuid,
    expected_source_version: u64,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
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
            vec![TENANT_ID.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("materialized Product row is missing"))?;
    let source_version: String = row.try_get("", "source_version_text")?;
    assert_eq!(source_version.parse::<u64>()?, expected_source_version);
    Ok(())
}

async fn update_restricted_visibility(db: &DatabaseConnection, slug: &str) -> TestResult<()> {
    let metadata = serde_json::json!({
        "channel_visibility": {"allowed_channel_slugs": [slug]}
    });
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE products SET metadata = $3 WHERE tenant_id = $1 AND id = $2",
        vec![TENANT_ID.into(), RESTRICTED_PRODUCT_ID.into(), metadata.into()],
    ))
    .await?;
    Ok(())
}

async fn rename_channel(db: &DatabaseConnection, channel_id: Uuid, slug: &str) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE channels SET slug = $3 WHERE tenant_id = $1 AND id = $2",
        vec![TENANT_ID.into(), channel_id.into(), slug.to_owned().into()],
    ))
    .await?;
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
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');

INSERT INTO channels (id, tenant_id, slug, name) VALUES
    ('{ALPHA_CHANNEL_ID}', '{TENANT_ID}', 'alpha', 'Alpha'),
    ('{BETA_CHANNEL_ID}', '{TENANT_ID}', 'beta', 'Beta');

INSERT INTO products (id, tenant_id, metadata) VALUES
    ('{UNRESTRICTED_PRODUCT_ID}', '{TENANT_ID}', '{{}}'::jsonb),
    ('{MALFORMED_PRODUCT_ID}', '{TENANT_ID}',
        '{{"channel_visibility":{{"allowed_channel_slugs":[" Alpha "]}}}}'::jsonb),
    ('{VALID_AFTER_MALFORMED_PRODUCT_ID}', '{TENANT_ID}', '{{}}'::jsonb),
    ('{RESTRICTED_PRODUCT_ID}', '{TENANT_ID}',
        '{{"channel_visibility":{{"allowed_channel_slugs":["alpha"]}}}}'::jsonb);

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{UNRESTRICTED_TRANSLATION_ID}', '{UNRESTRICTED_PRODUCT_ID}', '{TENANT_ID}', 'en', 'Unrestricted', 'unrestricted'),
    ('{MALFORMED_TRANSLATION_ID}', '{MALFORMED_PRODUCT_ID}', '{TENANT_ID}', 'en', 'Malformed', 'malformed'),
    ('{VALID_AFTER_MALFORMED_TRANSLATION_ID}', '{VALID_AFTER_MALFORMED_PRODUCT_ID}', '{TENANT_ID}', 'en', 'Valid after malformed', 'valid-after-malformed'),
    ('{RESTRICTED_TRANSLATION_ID}', '{RESTRICTED_PRODUCT_ID}', '{TENANT_ID}', 'en', 'Restricted', 'restricted');
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
