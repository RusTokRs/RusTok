use std::{env, error::Error, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext};
use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, FieldName, FieldPath, IndexModule, IndexSourceLoadRequest, IndexValue,
    LocaleKey, ModuleName, MutationApplyOutcome, MutationDelivery, PostgresMutationStore,
    PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion, SharedIndexQueryRuntime,
    SharedIndexSchemaRegistry, SharedIndexSourceRegistry, materialize_index_source_registry,
    materialize_postgres_index_query_runtime, materialize_postgres_index_sources,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    ProductCatalogReadRuntime, StorefrontProductListQuery, StorefrontProductSortDirection,
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

use super::{
    PRODUCT_SCHEMA_ROUTING_KEY, ProductStorefrontIndexShadowExecutor,
    channel_relation_resolver::ProductSalesChannelRelationResolver,
};

const DATABASE_ENV: &str = "RUSTOK_PRODUCT_STOREFRONT_EQUIVALENCE_DATABASE_URL";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const TENANT_ID: Uuid = Uuid::from_u128(0x5100);
const CHANNEL_ID: Uuid = Uuid::from_u128(0x5200);
const PRODUCT_A: Uuid = Uuid::from_u128(0x5301);
const PRODUCT_B: Uuid = Uuid::from_u128(0x5302);
const PRODUCT_C: Uuid = Uuid::from_u128(0x5303);
const FIXED_TIME: &str = "2026-08-08T06:00:00Z";

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product Storefront localized equivalence packet"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_product_storefront_eq_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("product_storefront_eq_migration_{suffix}"),
        )
        .await?;
        create_migration_prerequisites(&migration).await?;
        ProductMigrator::up(&migration, None).await?;
        let manager = SchemaManager::new(&migration);
        for migration_step in IndexModule.migrations() {
            migration_step.up(&manager).await?;
        }
        seed_products(&migration).await?;
        let resolver = ProductSalesChannelRelationResolver::new(migration.clone());
        for product_id in [PRODUCT_A, PRODUCT_B, PRODUCT_C] {
            resolver.reconcile_product(TENANT_ID, product_id).await?;
        }
        migration.close().await?;

        Ok(Some(Self {
            control,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eq_source_{suffix}"),
            )
            .await?,
            mutation: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eq_mutation_{suffix}"),
            )
            .await?,
            query: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eq_query_{suffix}"),
            )
            .await?,
            owner: scoped_connection(
                &database_url,
                &schema_name,
                &format!("product_storefront_eq_owner_{suffix}"),
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
async fn product_storefront_localized_postgres_retains_owner_shadow_identity_and_projection_evidence(
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let outcome = run_localized_projection_and_search_evidence(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

#[tokio::test]
async fn product_storefront_localized_postgres_retains_wildcard_and_equal_timestamp_paging_evidence(
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let outcome = run_wildcard_and_paging_evidence(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_localized_projection_and_search_evidence(database: &TestDatabase) -> TestResult<()> {
    let runtime = index_runtime(database).await?;
    materialize_products(&runtime).await?;
    let executor = shadow_executor(database, runtime.query.clone());

    // `%` is intentionally owner-visible wildcard input. The resulting owner pattern `%Needle%%`
    // matches a third-locale title on A, fallback-locale title on B, and unrelated third-locale title
    // on C. Identity folding must still return one row per Product.
    let query = storefront_query(Some("Needle%"), StorefrontProductSortDirection::Desc, 1, 12)?;
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
        .map_err(|error| std::io::Error::other(format!("shadow projection failed: {error}")))?;
    let comparison = execution
        .comparison
        .ok_or_else(|| std::io::Error::other("successful shadow projection must compare"))?;
    assert!(comparison.is_match());
    assert_eq!(execution.authoritative.total, 3);
    assert_eq!(projected.exact_count, Some(3));
    assert_eq!(
        execution
            .authoritative
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![PRODUCT_C, PRODUCT_B, PRODUCT_A]
    );

    // Search matched A through `fr`, but projection still selects requested `fi`.
    let owner_a = owner_item(&execution.authoritative, PRODUCT_A)?;
    let index_a = index_item(projected, PRODUCT_A)?;
    assert_eq!(owner_a.title, "Requested A");
    assert_eq!(owner_a.handle, "requested-a");
    assert_eq!(projected_string(index_a, "title")?, Some("Requested A"));
    assert_eq!(projected_string(index_a, "handle")?, Some("requested-a"));

    // B has no requested `fi`, so both owner and Index projection use fallback `en`.
    let owner_b = owner_item(&execution.authoritative, PRODUCT_B)?;
    let index_b = index_item(projected, PRODUCT_B)?;
    assert_eq!(owner_b.title, "NeedleXFR");
    assert_eq!(owner_b.handle, "fallback-b");
    assert_eq!(projected_string(index_b, "title")?, Some("NeedleXFR"));
    assert_eq!(projected_string(index_b, "handle")?, Some("fallback-b"));

    // C has only `de`: owner applies its public placeholder while the generic localized fold exposes
    // SQL null. This is retained evidence for the final Storefront projection adapter and must not be
    // misreported as raw field equivalence.
    let owner_c = owner_item(&execution.authoritative, PRODUCT_C)?;
    let index_c = index_item(projected, PRODUCT_C)?;
    assert_eq!(owner_c.title, "Untitled product");
    assert_eq!(owner_c.handle, "");
    assert_eq!(projected_string(index_c, "title")?, None);
    assert_eq!(projected_string(index_c, "handle")?, None);

    Ok(())
}

async fn run_wildcard_and_paging_evidence(database: &TestDatabase) -> TestResult<()> {
    let runtime = index_runtime(database).await?;
    materialize_products(&runtime).await?;
    let executor = shadow_executor(database, runtime.query.clone());

    // `_` remains a one-character wildcard on both owner and Index paths.
    let wildcard = executor
        .execute(
            port_context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(CHANNEL_ID),
            storefront_query(
                Some("Needle_FR"),
                StorefrontProductSortDirection::Asc,
                1,
                12,
            )?,
        )
        .await?;
    assert!(
        wildcard
            .comparison
            .ok_or_else(|| std::io::Error::other("wildcard comparison missing"))?
            .is_match()
    );
    assert_eq!(
        wildcard
            .authoritative
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![PRODUCT_A, PRODUCT_B]
    );

    // Backslash escapes `_`, leaving only A's exact third-locale `Needle_FR` title.
    let escaped = executor
        .execute(
            port_context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(CHANNEL_ID),
            storefront_query(
                Some(r"Needle\_FR"),
                StorefrontProductSortDirection::Asc,
                1,
                12,
            )?,
        )
        .await?;
    assert!(
        escaped
            .comparison
            .ok_or_else(|| std::io::Error::other("escaped comparison missing"))?
            .is_match()
    );
    assert_eq!(escaped.authoritative.total, 1);
    assert_eq!(escaped.authoritative.items[0].id, PRODUCT_A);

    // All Products share both owner sort timestamps. DESC therefore proves the Product-ID DESC
    // tie-break and offset/page boundary; ASC proves the opposite identity direction.
    let desc_first = executor
        .execute(
            port_context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(CHANNEL_ID),
            storefront_query(None, StorefrontProductSortDirection::Desc, 1, 2)?,
        )
        .await?;
    assert!(
        desc_first
            .comparison
            .ok_or_else(|| std::io::Error::other("DESC page comparison missing"))?
            .is_match()
    );
    assert_eq!(desc_first.authoritative.total, 3);
    assert!(desc_first.authoritative.has_next);
    assert_eq!(
        desc_first
            .authoritative
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![PRODUCT_C, PRODUCT_B]
    );

    let desc_second = executor
        .execute(
            port_context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(CHANNEL_ID),
            storefront_query(None, StorefrontProductSortDirection::Desc, 2, 2)?,
        )
        .await?;
    assert!(
        desc_second
            .comparison
            .ok_or_else(|| std::io::Error::other("DESC second page comparison missing"))?
            .is_match()
    );
    assert_eq!(desc_second.authoritative.total, 3);
    assert!(!desc_second.authoritative.has_next);
    assert_eq!(desc_second.authoritative.items[0].id, PRODUCT_A);

    let asc_first = executor
        .execute(
            port_context(),
            "en".to_owned(),
            Some("online".to_owned()),
            Some(CHANNEL_ID),
            storefront_query(None, StorefrontProductSortDirection::Asc, 1, 2)?,
        )
        .await?;
    assert!(
        asc_first
            .comparison
            .ok_or_else(|| std::io::Error::other("ASC page comparison missing"))?
            .is_match()
    );
    assert_eq!(
        asc_first
            .authoritative
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![PRODUCT_A, PRODUCT_B]
    );

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
        product_key(PRODUCT_A, "fr")?,
        product_key(PRODUCT_B, "en")?,
        product_key(PRODUCT_C, "de")?,
    ])?;
    let mutations = runtime.sources.load(request).await?.into_mutations();
    assert_eq!(mutations.len(), 5);
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

fn storefront_query(
    search: Option<&str>,
    direction: StorefrontProductSortDirection,
    page: u64,
    per_page: u64,
) -> TestResult<StorefrontProductListQuery> {
    Ok(StorefrontProductListQuery::try_new_with_attribute_filters(
        search.map(ToOwned::to_owned),
        None,
        Some("published_at".to_owned()),
        Some(match direction {
            StorefrontProductSortDirection::Asc => "asc".to_owned(),
            StorefrontProductSortDirection::Desc => "desc".to_owned(),
        }),
        Vec::new(),
    )?
    .with_pagination(page, per_page))
}

fn port_context() -> PortContext {
    PortContext::new(
        TENANT_ID.to_string(),
        PortActor::service("product-storefront-postgres-evidence"),
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

fn owner_item(
    list: &rustok_product::StorefrontProductList,
    product_id: Uuid,
) -> TestResult<&rustok_product::StorefrontProductListItem> {
    list.items
        .iter()
        .find(|item| item.id == product_id)
        .ok_or_else(|| std::io::Error::other(format!("owner Product {product_id} missing")).into())
}

fn index_item(
    page: &rustok_index::IndexQueryPage,
    product_id: Uuid,
) -> TestResult<&rustok_index::IndexQueryItem> {
    page.items
        .iter()
        .find(|item| item.entity_id == product_id)
        .ok_or_else(|| std::io::Error::other(format!("Index Product {product_id} missing")).into())
}

fn projected_string<'a>(
    item: &'a rustok_index::IndexQueryItem,
    field: &str,
) -> TestResult<Option<&'a str>> {
    let path = FieldPath::new(FieldName::new(field)?);
    let projected = item
        .fields
        .iter()
        .find(|projected| projected.path == path)
        .ok_or_else(|| std::io::Error::other(format!("missing projected field {field}")))?;
    match &projected.value {
        IndexValue::String(value) => Ok(Some(value.as_str())),
        IndexValue::Null => Ok(None),
        other => Err(std::io::Error::other(format!(
            "projected field {field} has unexpected value {other:?}"
        ))
        .into()),
    }
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

async fn seed_products(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');
INSERT INTO channels (id, tenant_id, slug) VALUES ('{CHANNEL_ID}', '{TENANT_ID}', 'online');

INSERT INTO products (id, tenant_id) VALUES
    ('{PRODUCT_A}', '{TENANT_ID}'),
    ('{PRODUCT_B}', '{TENANT_ID}'),
    ('{PRODUCT_C}', '{TENANT_ID}');

UPDATE products
SET status = 'active',
    created_at = '{FIXED_TIME}'::timestamptz,
    updated_at = '{FIXED_TIME}'::timestamptz,
    published_at = '{FIXED_TIME}'::timestamptz,
    metadata = '{{}}'::jsonb
WHERE tenant_id = '{TENANT_ID}';

INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle) VALUES
    ('00000000-0000-0000-0000-000000005401', '{PRODUCT_A}', '{TENANT_ID}', 'fi', 'Requested A', 'requested-a'),
    ('00000000-0000-0000-0000-000000005402', '{PRODUCT_A}', '{TENANT_ID}', 'en', 'Fallback A', 'fallback-a'),
    ('00000000-0000-0000-0000-000000005403', '{PRODUCT_A}', '{TENANT_ID}', 'fr', 'Needle_FR', 'needle-a-fr'),
    ('00000000-0000-0000-0000-000000005404', '{PRODUCT_B}', '{TENANT_ID}', 'en', 'NeedleXFR', 'fallback-b'),
    ('00000000-0000-0000-0000-000000005405', '{PRODUCT_C}', '{TENANT_ID}', 'de', 'NeedleZZ', 'third-c');
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
