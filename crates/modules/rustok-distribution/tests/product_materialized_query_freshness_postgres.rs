#![cfg(feature = "mod-product")]

use std::{env, error::Error};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, FilterExpr, IndexModule, IndexMutation,
    IndexQuery, IndexQueryPort, IndexQueryScope, IndexSourceLoadRequest, IndexValue, LocaleKey,
    ModuleName, MutationApplyOutcome, MutationDelivery, OrderDirection, OrderExpr, Pagination,
    PostgresMutationStore, PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion,
    SharedIndexQueryRuntime, SharedIndexSchemaRegistry, SharedIndexSourceRegistry,
    materialize_index_source_registry, materialize_postgres_index_query_runtime,
    materialize_postgres_index_sources,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const TENANT_ID: Uuid = Uuid::from_u128(1);
const STALE_PRODUCT_ID: Uuid = Uuid::from_u128(101);
const CONTROL_PRODUCT_ID: Uuid = Uuid::from_u128(102);
const LOCALE_DELETE_PRODUCT_ID: Uuid = Uuid::from_u128(103);
const STALE_TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const CONTROL_TRANSLATION_ID: Uuid = Uuid::from_u128(112);
const LOCALE_DELETE_TRANSLATION_ID: Uuid = Uuid::from_u128(113);
const STALE_TITLE: &str = "A stale candidate";
const CONTROL_TITLE: &str = "B fresh control";
const LOCALE_DELETE_TITLE: &str = "C deleted locale";

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
    writer: DatabaseConnection,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product materialized query freshness harness"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_product_query_freshness_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_product_query_freshness_migration_{suffix}"),
        )
        .await?;
        create_product_migration_prerequisites(&migration).await?;
        ProductMigrator::up(&migration, None).await?;
        let manager = SchemaManager::new(&migration);
        for migration_step in IndexModule.migrations() {
            migration_step.up(&manager).await?;
        }
        seed_products(&migration).await?;
        migration.close().await?;

        Ok(Some(Self {
            control,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_query_freshness_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_query_freshness_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_query_freshness_query_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_product_query_freshness_writer_{suffix}"),
            )
            .await?,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
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

struct ProductIndexRuntime {
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    query: SharedIndexQueryRuntime,
    mutations: PostgresMutationStore,
}

#[tokio::test]
async fn product_materialized_freshness_fences_delayed_mutations_before_query_semantics()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_materialized_freshness_scenarios(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_materialized_freshness_scenarios(database: &TestDatabase) -> TestResult<()> {
    let runtime = product_runtime(database).await?;

    // Keep one genuinely fresh Product materialized as a control row. The stale candidate sorts
    // before this row, so a missing root-admission fence would change page identity and exact count.
    let control = load_product_mutation(&runtime.sources, CONTROL_PRODUCT_ID, "en").await?;
    apply_product_mutation(&runtime, control).await?;

    // Capture a valid Product mutation, then advance owner Product/projection state before applying
    // that already-produced mutation. The mutation store intentionally knows only Index source
    // version ordering, so the delayed mutation is physically accepted into index_entities.
    let delayed = load_product_mutation(&runtime.sources, STALE_PRODUCT_ID, "en").await?;
    let delayed_source_version = delayed.source_version();
    bump_stale_product_owner_revision(&database.writer).await?;
    assert_owner_projection_advanced(&database.writer, STALE_PRODUCT_ID, delayed_source_version)
        .await?;
    apply_product_mutation(&runtime, delayed).await?;
    assert_materialized_source_version(
        &database.mutation,
        STALE_PRODUCT_ID,
        "en",
        delayed_source_version,
    )
    .await?;

    // Admission happens before the user title filter, title ordering, cursor lookahead/limit, and
    // exact count. The physically stored stale A-row therefore cannot displace fresh B or inflate
    // count even though A sorts first.
    let fenced = runtime
        .query
        .execute_query(product_title_page(None)?)
        .await?;
    assert_eq!(fenced.items.len(), 1);
    assert_eq!(fenced.items[0].entity_id, CONTROL_PRODUCT_ID);
    assert_eq!(projected_string(&fenced.items[0], "title")?, CONTROL_TITLE);
    assert_eq!(fenced.exact_count, Some(1));
    assert!(!fenced.has_more);
    assert!(fenced.next_cursor.is_none());

    // Once the corrective current projection is materialized, A becomes query-admissible again.
    // Cursor pagination must then retain ordinary semantics with the same admission predicate.
    let current = load_product_mutation(&runtime.sources, STALE_PRODUCT_ID, "en").await?;
    assert!(current.source_version() > delayed_source_version);
    apply_product_mutation(&runtime, current).await?;

    let first = runtime
        .query
        .execute_query(product_title_page(None)?)
        .await?;
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].entity_id, STALE_PRODUCT_ID);
    assert_eq!(projected_string(&first.items[0], "title")?, STALE_TITLE);
    assert_eq!(first.exact_count, Some(2));
    assert!(first.has_more);
    let cursor = first
        .next_cursor
        .clone()
        .ok_or_else(|| std::io::Error::other("two admitted Products must emit a cursor"))?;

    let second = runtime
        .query
        .execute_query(product_title_page(Some(cursor))?)
        .await?;
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].entity_id, CONTROL_PRODUCT_ID);
    assert_eq!(projected_string(&second.items[0], "title")?, CONTROL_TITLE);
    assert_eq!(second.exact_count, Some(2));
    assert!(!second.has_more);

    // Repeat the delayed-apply window for locale deletion. The stale upsert is deliberately written
    // after the owner translation has disappeared; query admission must still hide the stored row
    // before the retained Product delete mutation is delivered.
    let locale_delayed =
        load_product_mutation(&runtime.sources, LOCALE_DELETE_PRODUCT_ID, "en").await?;
    let locale_delayed_source_version = locale_delayed.source_version();
    delete_product_locale(&database.writer).await?;
    assert_owner_projection_advanced(
        &database.writer,
        LOCALE_DELETE_PRODUCT_ID,
        locale_delayed_source_version,
    )
    .await?;
    apply_product_mutation(&runtime, locale_delayed).await?;
    assert_materialized_source_version(
        &database.mutation,
        LOCALE_DELETE_PRODUCT_ID,
        "en",
        locale_delayed_source_version,
    )
    .await?;

    let deleted_locale = runtime
        .query
        .execute_query(product_identity_query(LOCALE_DELETE_PRODUCT_ID)?)
        .await?;
    assert!(deleted_locale.items.is_empty());
    assert_eq!(deleted_locale.exact_count, Some(0));
    assert!(!deleted_locale.has_more);

    Ok(())
}

async fn product_runtime(database: &TestDatabase) -> TestResult<ProductIndexRuntime> {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_channel::ChannelModule)
        .register(rustok_product::ProductModule);
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)?;

    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .ok_or_else(|| std::io::Error::other("Product schema registry is missing"))?;
    let schema_store = PostgresSchemaRegistrationStore::new(database.query.clone());
    for registered in schemas.registry().iter() {
        schema_store.register(TENANT_ID, &registered.schema).await?;
    }

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
    })
}

async fn load_product_mutation(
    sources: &SharedIndexSourceRegistry,
    product_id: Uuid,
    locale: &str,
) -> TestResult<IndexMutation> {
    let request = IndexSourceLoadRequest::new(vec![product_key(product_id, locale)?])?;
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
            if source_version == expected_source_version =>
        {
            Ok(())
        }
        other => Err(std::io::Error::other(format!(
            "expected Product mutation version {expected_source_version} to apply, got {other:?}"
        ))
        .into()),
    }
}

fn product_title_page(after: Option<String>) -> TestResult<IndexQuery> {
    let title = field_path("title")?;
    Ok(IndexQuery {
        scope: product_scope("en")?,
        schema: product_schema_ref()?,
        fields: vec![title.clone()],
        filter: Some(FilterExpr::In(
            title.clone(),
            vec![
                IndexValue::String(STALE_TITLE.to_owned()),
                IndexValue::String(CONTROL_TITLE.to_owned()),
            ],
        )),
        order_by: vec![OrderExpr {
            field: title,
            direction: OrderDirection::Asc,
        }],
        pagination: Pagination::Cursor { first: 1, after },
        include_exact_count: true,
    })
}

fn product_identity_query(product_id: Uuid) -> TestResult<IndexQuery> {
    Ok(IndexQuery {
        scope: product_scope("en")?,
        schema: product_schema_ref()?,
        fields: vec![field_path("title")?],
        filter: Some(FilterExpr::Eq(
            field_path("id")?,
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

fn product_scope(locale: &str) -> TestResult<IndexQueryScope> {
    Ok(IndexQueryScope {
        tenant_id: TENANT_ID,
        locale: Some(LocaleKey::new(locale)?),
    })
}

fn product_key(product_id: Uuid, locale: &str) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: product_schema_ref()?,
        entity_id: product_id,
        locale: Some(LocaleKey::new(locale)?),
    })
}

fn product_schema_ref() -> TestResult<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")?,
        entity: EntityName::new("product")?,
        // Index core still requires one positive numeric routing/storage key. Distribution registers
        // only this current Product contract.
        version: SchemaVersion::new(4),
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

async fn assert_owner_projection_advanced(
    db: &DatabaseConnection,
    product_id: Uuid,
    delayed_source_version: u64,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT CAST(projection_epoch AS TEXT) AS projection_epoch_text
FROM product_index_graph_projection_snapshots
WHERE tenant_id = $1
  AND product_id = $2
ORDER BY projection_epoch DESC
LIMIT 1
"#,
            vec![TENANT_ID.into(), product_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("expected current Product projection"))?;
    let projection_epoch: String = row.try_get("", "projection_epoch_text")?;
    let projection_epoch = projection_epoch.parse::<u64>()?;
    assert!(
        projection_epoch > delayed_source_version,
        "owner projection {projection_epoch} must be newer than delayed mutation {delayed_source_version}"
    );
    Ok(())
}

async fn assert_materialized_source_version(
    db: &DatabaseConnection,
    product_id: Uuid,
    locale: &str,
    expected_source_version: u64,
) -> TestResult<()> {
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
  AND locale_key = $3
  AND is_deleted = FALSE
"#,
            vec![
                TENANT_ID.into(),
                product_id.into(),
                locale.to_owned().into(),
            ],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("expected stale Product row to be materialized"))?;
    let source_version: String = row.try_get("", "source_version_text")?;
    assert_eq!(source_version.parse::<u64>()?, expected_source_version);
    Ok(())
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
-- Product query admission reads the Channel-owned tenant identity watermark. This focused packet does
-- not exercise Channel convergence yet, so an empty owner-shaped table represents generation 0.
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

async fn seed_products(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');

INSERT INTO products (id, tenant_id) VALUES
    ('{STALE_PRODUCT_ID}', '{TENANT_ID}'),
    ('{CONTROL_PRODUCT_ID}', '{TENANT_ID}'),
    ('{LOCALE_DELETE_PRODUCT_ID}', '{TENANT_ID}');

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('{STALE_TRANSLATION_ID}', '{STALE_PRODUCT_ID}', '{TENANT_ID}', 'en', '{STALE_TITLE}', 'a-stale-candidate'),
    ('{CONTROL_TRANSLATION_ID}', '{CONTROL_PRODUCT_ID}', '{TENANT_ID}', 'en', '{CONTROL_TITLE}', 'b-fresh-control'),
    ('{LOCALE_DELETE_TRANSLATION_ID}', '{LOCALE_DELETE_PRODUCT_ID}', '{TENANT_ID}', 'en', '{LOCALE_DELETE_TITLE}', 'c-deleted-locale');

-- One resolved empty-membership relation per Product creates the exact current graph projection.
INSERT INTO product_sales_channel_index_relation_snapshots (
    tenant_id,
    product_id,
    relation_epoch,
    channel_ids
) VALUES
    ('{TENANT_ID}', '{STALE_PRODUCT_ID}', 1, '[]'::jsonb),
    ('{TENANT_ID}', '{CONTROL_PRODUCT_ID}', 1, '[]'::jsonb),
    ('{TENANT_ID}', '{LOCALE_DELETE_PRODUCT_ID}', 1, '[]'::jsonb);

-- Generation 0 is exact because this packet intentionally has no Channels. Product visibility is the
-- default unrestricted `all`; retained INSERT convergence requests are at or before this witness.
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

async fn bump_stale_product_owner_revision(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        "UPDATE products SET vendor = 'owner-newer-than-delayed-mutation' WHERE tenant_id = '{TENANT_ID}' AND id = '{STALE_PRODUCT_ID}'"
    ))
    .await?;
    Ok(())
}

async fn delete_product_locale(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        "DELETE FROM product_translations WHERE tenant_id = '{TENANT_ID}' AND product_id = '{LOCALE_DELETE_PRODUCT_ID}' AND locale = 'en'"
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
