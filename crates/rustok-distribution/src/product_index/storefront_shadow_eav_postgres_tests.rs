use std::{env, error::Error, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext};
use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, IndexModule, IndexSourceLoadRequest, LocaleKey, ModuleName,
    MutationApplyOutcome, MutationDelivery, PostgresMutationStore, PostgresSchemaRegistrationStore,
    SchemaRef, SchemaVersion, SharedIndexQueryRuntime, SharedIndexSchemaRegistry,
    SharedIndexSourceRegistry, materialize_index_source_registry,
    materialize_postgres_index_query_runtime, materialize_postgres_index_sources,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{ProductCatalogReadRuntime, StorefrontProductListQuery};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

use super::{
    PRODUCT_SCHEMA_ROUTING_KEY, ProductStorefrontIndexShadowExecutor,
    channel_relation_resolver::ProductSalesChannelRelationResolver,
};

const DATABASE_ENV: &str = "RUSTOK_PRODUCT_STOREFRONT_EAV_EQUIVALENCE_DATABASE_URL";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const TENANT_ID: Uuid = Uuid::from_u128(0x6100);
const CHANNEL_ID: Uuid = Uuid::from_u128(0x6200);
const PRODUCT_A: Uuid = Uuid::from_u128(0x6301);
const PRODUCT_B: Uuid = Uuid::from_u128(0x6302);
const ATTRIBUTE_WEIGHT: Uuid = Uuid::from_u128(0x6401);
const ATTRIBUTE_LABEL: Uuid = Uuid::from_u128(0x6402);
const ATTRIBUTE_COLOR: Uuid = Uuid::from_u128(0x6403);
const ATTRIBUTE_FEATURES: Uuid = Uuid::from_u128(0x6404);
const COLOR_RED: Uuid = Uuid::from_u128(0x6501);
const COLOR_BLUE: Uuid = Uuid::from_u128(0x6502);
const FEATURE_WIFI: Uuid = Uuid::from_u128(0x6503);
const FEATURE_NFC: Uuid = Uuid::from_u128(0x6504);

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
    owner: DatabaseConnection,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product Storefront EAV equivalence packet"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_product_storefront_eav_eq_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("product_storefront_eav_eq_migration_{suffix}"),
        )
        .await?;
        create_migration_prerequisites(&migration).await?;
        ProductMigrator::up(&migration, None).await?;
        let manager = SchemaManager::new(&migration);
        for migration_step in IndexModule.migrations() {
            migration_step.up(&manager).await?;
        }
        seed_products_and_attributes(&migration).await?;
        let resolver = ProductSalesChannelRelationResolver::new(migration.clone());
        for product_id in [PRODUCT_A, PRODUCT_B] {
            resolver.reconcile_product(TENANT_ID, product_id).await?;
        }
        migration.close().await?;

        Ok(Some(Self {
            control,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eav_eq_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eav_eq_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eav_eq_query_{suffix}"),
            )
            .await?,
            owner: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eav_eq_owner_{suffix}"),
            )
            .await?,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.source.close().await?;
        self.mutation.close().await?;
        self.query.close().await?;
        self.owner.close().await?;
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

struct EvidenceRuntime {
    sources: SharedIndexSourceRegistry,
    schemas: SharedIndexSchemaRegistry,
    query: SharedIndexQueryRuntime,
    mutations: PostgresMutationStore,
}

#[tokio::test]
async fn product_storefront_eav_postgres_retains_scalar_and_localized_term_equivalence()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let outcome = run_scalar_and_localized_evidence(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

#[tokio::test]
async fn product_storefront_eav_postgres_retains_option_code_uuid_and_never_equivalence()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let outcome = run_option_evidence(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_scalar_and_localized_evidence(database: &TestDatabase) -> TestResult<()> {
    let runtime = index_runtime(database).await?;
    materialize_products(&runtime).await?;
    let executor = shadow_executor(database, runtime.query.clone());

    assert_filter_ids(&executor, "weight=7", &[PRODUCT_A], "integer scalar term").await?;
    assert_filter_ids(
        &executor,
        "label=Punainen",
        &[PRODUCT_A],
        "requested localized text term",
    )
    .await?;

    // A has a requested `fi` row with a different value, so its `en=Red` fallback must be suppressed.
    // B has no `fi` row, therefore the exact owner fallback predicate admits B only.
    assert_filter_ids(
        &executor,
        "label=Red",
        &[PRODUCT_B],
        "requested-missing localized fallback term",
    )
    .await?;

    Ok(())
}

async fn run_option_evidence(database: &TestDatabase) -> TestResult<()> {
    let runtime = index_runtime(database).await?;
    materialize_products(&runtime).await?;
    let executor = shadow_executor(database, runtime.query.clone());

    assert_filter_ids(&executor, "color=red", &[PRODUCT_A], "select option code").await?;
    assert_filter_ids(
        &executor,
        format!("color={COLOR_RED}").as_str(),
        &[PRODUCT_A],
        "select direct option UUID",
    )
    .await?;
    assert_filter_ids(
        &executor,
        "features=wifi",
        &[PRODUCT_A],
        "multiselect option code",
    )
    .await?;
    assert_filter_ids(&executor, "color=missing", &[], "missing option Never").await?;
    assert_filter_ids(
        &executor,
        "color=00000000-0000-0000-0000-000000000000",
        &[],
        "nil option UUID Never",
    )
    .await?;

    // Keep one negative control proving an active but different option is not collapsed with RED.
    assert_filter_ids(
        &executor,
        "color=blue",
        &[PRODUCT_B],
        "different option code",
    )
    .await?;
    Ok(())
}

async fn assert_filter_ids(
    executor: &ProductStorefrontIndexShadowExecutor,
    filter: &str,
    expected: &[Uuid],
    scenario: &str,
) -> TestResult<()> {
    let query = StorefrontProductListQuery::try_new_with_attribute_filters(
        None,
        None,
        Some("published_at".to_owned()),
        Some("asc".to_owned()),
        vec![filter.to_owned()],
    )?;
    let execution = executor
        .execute(
            port_context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(CHANNEL_ID),
            query,
        )
        .await?;
    let projected = execution
        .projected
        .as_ref()
        .map_err(|error| std::io::Error::other(format!("{scenario}: shadow failed: {error}")))?;
    let comparison = execution
        .comparison
        .ok_or_else(|| std::io::Error::other(format!("{scenario}: comparison missing")))?;
    if !comparison.is_match() {
        return Err(std::io::Error::other(format!(
            "{scenario}: owner/Index identity-count-page comparison failed: {comparison:?}"
        ))
        .into());
    }
    let owner_ids = execution
        .authoritative
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let index_ids = projected
        .items
        .iter()
        .map(|item| item.entity_id)
        .collect::<Vec<_>>();
    assert_eq!(owner_ids, expected, "{scenario}: owner ids");
    assert_eq!(index_ids, expected, "{scenario}: Index ids");
    assert_eq!(execution.authoritative.total, expected.len() as u64);
    assert_eq!(projected.exact_count, Some(expected.len() as u64));
    Ok(())
}

async fn index_runtime(database: &TestDatabase) -> TestResult<EvidenceRuntime> {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_channel::ChannelModule)
        .register(rustok_product::ProductModule);
    let mut extensions = crate::build_runtime_extensions(&registry)?;

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

    Ok(EvidenceRuntime {
        sources,
        schemas,
        query,
        mutations: PostgresMutationStore::new(database.mutation.clone()),
    })
}

async fn materialize_products(runtime: &EvidenceRuntime) -> TestResult<()> {
    let request = IndexSourceLoadRequest::new(vec![
        product_key(PRODUCT_A, "fi")?,
        product_key(PRODUCT_A, "en")?,
        product_key(PRODUCT_B, "en")?,
    ])?;
    let mutations = runtime.sources.load(request).await?.into_mutations();
    assert_eq!(mutations.len(), 3);
    for mutation in mutations {
        let expected_source_version = mutation.source_version();
        let delivery = MutationDelivery::from_event(PRODUCT_SOURCE, mutation)?;
        let outcome = runtime
            .mutations
            .apply(runtime.schemas.registry(), &delivery)
            .await?;
        assert!(matches!(
            outcome,
            MutationApplyOutcome::Applied { source_version }
                if source_version == expected_source_version
        ));
    }
    Ok(())
}

fn shadow_executor(
    database: &TestDatabase,
    index: SharedIndexQueryRuntime,
) -> ProductStorefrontIndexShadowExecutor {
    let transport = Arc::new(OutboxTransport::new(database.owner.clone()));
    let event_bus = TransactionalEventBus::new(transport);
    let product = ProductCatalogReadRuntime::in_process(database.owner.clone(), event_bus);
    ProductStorefrontIndexShadowExecutor::new(product, index)
}

fn port_context() -> PortContext {
    PortContext::new(
        TENANT_ID.to_string(),
        PortActor::service("product-storefront-eav-postgres-evidence"),
        "fi",
        Uuid::new_v4().to_string(),
    )
    .with_channel("online")
    .with_deadline(Duration::from_secs(5))
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
        version: SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
    })
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
CREATE TABLE channels (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, slug)
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

async fn seed_products_and_attributes(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');
INSERT INTO channels (id, tenant_id, slug) VALUES ('{CHANNEL_ID}', '{TENANT_ID}', 'online');

INSERT INTO products (id, tenant_id) VALUES
    ('{PRODUCT_A}', '{TENANT_ID}'),
    ('{PRODUCT_B}', '{TENANT_ID}');
UPDATE products
SET status = 'active',
    created_at = '2026-08-08T06:00:00Z'::timestamptz,
    updated_at = '2026-08-08T06:00:00Z'::timestamptz,
    published_at = '2026-08-08T06:00:00Z'::timestamptz,
    metadata = '{{}}'::jsonb
WHERE tenant_id = '{TENANT_ID}';

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('00000000-0000-0000-0000-000000006301', '{PRODUCT_A}', '{TENANT_ID}', 'fi', 'A fi', 'a-fi'),
    ('00000000-0000-0000-0000-000000006302', '{PRODUCT_A}', '{TENANT_ID}', 'en', 'A en', 'a-en'),
    ('00000000-0000-0000-0000-000000006303', '{PRODUCT_B}', '{TENANT_ID}', 'en', 'B en', 'b-en');

INSERT INTO product_attributes (
    id, tenant_id, code, value_type, scope, is_localized, is_filterable
) VALUES
    ('{ATTRIBUTE_WEIGHT}', '{TENANT_ID}', 'weight', 'integer', 'product', FALSE, TRUE),
    ('{ATTRIBUTE_LABEL}', '{TENANT_ID}', 'label', 'text', 'product', TRUE, TRUE),
    ('{ATTRIBUTE_COLOR}', '{TENANT_ID}', 'color', 'select', 'product', FALSE, TRUE),
    ('{ATTRIBUTE_FEATURES}', '{TENANT_ID}', 'features', 'multiselect', 'product', FALSE, TRUE);

INSERT INTO product_attribute_options (id, tenant_id, attribute_id, code, position) VALUES
    ('{COLOR_RED}', '{TENANT_ID}', '{ATTRIBUTE_COLOR}', 'red', 0),
    ('{COLOR_BLUE}', '{TENANT_ID}', '{ATTRIBUTE_COLOR}', 'blue', 1),
    ('{FEATURE_WIFI}', '{TENANT_ID}', '{ATTRIBUTE_FEATURES}', 'wifi', 0),
    ('{FEATURE_NFC}', '{TENANT_ID}', '{ATTRIBUTE_FEATURES}', 'nfc', 1);

INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id, value_integer) VALUES
    ('00000000-0000-0000-0000-000000006401', '{TENANT_ID}', '{PRODUCT_A}', '{ATTRIBUTE_WEIGHT}', 7),
    ('00000000-0000-0000-0000-000000006402', '{TENANT_ID}', '{PRODUCT_B}', '{ATTRIBUTE_WEIGHT}', 9);
INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id) VALUES
    ('00000000-0000-0000-0000-000000006403', '{TENANT_ID}', '{PRODUCT_A}', '{ATTRIBUTE_LABEL}'),
    ('00000000-0000-0000-0000-000000006404', '{TENANT_ID}', '{PRODUCT_B}', '{ATTRIBUTE_LABEL}'),
    ('00000000-0000-0000-0000-000000006405', '{TENANT_ID}', '{PRODUCT_A}', '{ATTRIBUTE_COLOR}'),
    ('00000000-0000-0000-0000-000000006406', '{TENANT_ID}', '{PRODUCT_B}', '{ATTRIBUTE_COLOR}'),
    ('00000000-0000-0000-0000-000000006407', '{TENANT_ID}', '{PRODUCT_A}', '{ATTRIBUTE_FEATURES}'),
    ('00000000-0000-0000-0000-000000006408', '{TENANT_ID}', '{PRODUCT_B}', '{ATTRIBUTE_FEATURES}');

INSERT INTO product_attribute_value_translations (id, value_id, locale, value_text) VALUES
    ('00000000-0000-0000-0000-000000006501', '00000000-0000-0000-0000-000000006403', 'fi', 'Punainen'),
    ('00000000-0000-0000-0000-000000006502', '00000000-0000-0000-0000-000000006403', 'en', 'Red'),
    ('00000000-0000-0000-0000-000000006503', '00000000-0000-0000-0000-000000006404', 'en', 'Red');

INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id) VALUES
    ('{TENANT_ID}', '00000000-0000-0000-0000-000000006405', '{COLOR_RED}'),
    ('{TENANT_ID}', '00000000-0000-0000-0000-000000006406', '{COLOR_BLUE}'),
    ('{TENANT_ID}', '00000000-0000-0000-0000-000000006407', '{FEATURE_WIFI}'),
    ('{TENANT_ID}', '00000000-0000-0000-0000-000000006407', '{FEATURE_NFC}'),
    ('{TENANT_ID}', '00000000-0000-0000-0000-000000006408', '{FEATURE_NFC}');
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
