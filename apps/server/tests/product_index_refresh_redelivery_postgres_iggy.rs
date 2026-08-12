#![cfg(feature = "mod-product")]

use std::{
    env,
    error::Error,
    io::{Error as IoError, ErrorKind},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rustok_core::{MigrationSource, ModuleRegistry, events::EventTransport};
use rustok_distribution::product_index::refresh_event::{
    ProductIndexRefreshDelivery, ProductIndexRefreshDeliveryProcessError,
    ProductIndexRefreshDeliveryWorker,
};
use rustok_events::{ContractEventEnvelope, ContractEventPayload, ProductIndexRefreshEvent};
use rustok_iggy::{
    ConsumedContractEvent, ExternalConfig, IggyConfig, IggyMode, IggyTransport,
    PersistentContractConsumerGroup, PersistentContractDelivery, SerializationFormat,
    TopologyConfig,
};
use rustok_index::{
    EntityKey, EntityName, IndexModule, IndexMutation, IndexMutationAcknowledgeFailure,
    IndexMutationEventAcknowledger, IndexReplayMutationOutcome, IndexSourceLoadRequest,
    IndexSourceRefreshEventProcessError, LocaleKey, ModuleName, PostgresMutationStore,
    PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion, SharedIndexMutationEventRegistry,
    SharedIndexSchemaRegistry, SharedIndexSourceRegistry, materialize_index_mutation_event_registry,
    materialize_index_source_registry, materialize_postgres_index_sources,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use tokio::time::timeout;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL";
const IGGY_ADDRESS_ENV: &str = "RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS";
const IGGY_USERNAME_ENV: &str = "RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME";
const IGGY_PASSWORD_ENV: &str = "RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD";
const PRODUCT_INDEX_REFRESH_TOPIC: &str = "domain";
const PRODUCT_INDEX_REFRESH_CONSUMER_GROUP: &str = "rustok-product-index-refresh";
const PRODUCT_SOURCE: &str = "product-postgres-primary";
const PRODUCT_VARIANT_SOURCE: &str = "product-variant-postgres-primary";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(20);
const ACK_FAILURE_CODE: &str = "product_index.refresh.evidence_ack_failed";

const TENANT_ID: Uuid = Uuid::from_u128(1);
const PRODUCT_ID: Uuid = Uuid::from_u128(101);
const PRODUCT_TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const VARIANT_ID: Uuid = Uuid::from_u128(201);
const CHANNEL_ID: Uuid = Uuid::from_u128(301);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    migration: DatabaseConnection,
    source: DatabaseConnection,
    mutation: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup(database_url: &str, scope: &str) -> TestResult<Self> {
        let control = connect(database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!(
            "rustok_product_refresh_{}_{}",
            sanitize_identifier(scope),
            suffix
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(database_url, &schema_name).await?;
        let setup_result = async {
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
            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
        .await;
        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error);
        }

        Ok(Self {
            control,
            migration,
            source: scoped_connection(database_url, &schema_name).await?,
            mutation: scoped_connection(database_url, &schema_name).await?,
            database_url: database_url.to_owned(),
            schema_name,
        })
    }

    async fn cleanup(self) -> TestResult<()> {
        self.migration.close().await?;
        self.source.close().await?;
        self.mutation.close().await?;
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
    schemas: SharedIndexSchemaRegistry,
    sources: SharedIndexSourceRegistry,
    events: SharedIndexMutationEventRegistry,
    mutations: PostgresMutationStore,
}

#[derive(Clone)]
struct BrokerAcknowledger {
    group: Arc<PersistentContractConsumerGroup>,
    fail_next: Arc<AtomicBool>,
}

impl BrokerAcknowledger {
    fn new(group: Arc<PersistentContractConsumerGroup>, fail_first: bool) -> Self {
        Self {
            group,
            fail_next: Arc::new(AtomicBool::new(fail_first)),
        }
    }
}

#[async_trait]
impl IndexMutationEventAcknowledger for BrokerAcknowledger {
    type Token = ConsumedContractEvent;

    async fn acknowledge(
        &self,
        token: &Self::Token,
    ) -> Result<(), IndexMutationAcknowledgeFailure> {
        let mut attempted = token.clone();
        if self.fail_next.swap(false, Ordering::SeqCst) {
            let exact = attempted
                .ack_token()
                .expect("real Iggy delivery must retain an acknowledgement token");
            attempted.connector_metadata.ack_token = Some(format!("{exact}-injected-failure"));
        }
        self.group.acknowledge(&attempted).await.map_err(|_| {
            IndexMutationAcknowledgeFailure::retryable(ACK_FAILURE_CODE)
                .expect("static evidence acknowledgement failure code is valid")
        })
    }
}

#[tokio::test]
async fn product_refresh_redelivery_uses_real_postgres_and_iggy_adapters() -> TestResult<()> {
    let Some(database_url) = postgres_database_url()? else {
        eprintln!(
            "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product Index PostgreSQL/Iggy redelivery evidence"
        );
        return Ok(());
    };
    let Some(iggy) = external_iggy_inputs()? else {
        eprintln!(
            "{IGGY_ADDRESS_ENV} is not set; skipping Product Index PostgreSQL/Iggy redelivery evidence"
        );
        return Ok(());
    };

    run_ack_failure_redelivery(&database_url, &iggy).await?;
    run_behind_source_redelivery(&database_url, &iggy).await?;
    Ok(())
}

async fn run_ack_failure_redelivery(
    database_url: &str,
    iggy: &ExternalIggyInputs,
) -> TestResult<()> {
    let database = TestDatabase::setup(database_url, "ack_redelivery").await?;
    let result = async {
        let runtime = build_runtime(&database).await?;
        let product_source = load_current_source_mutation(&runtime.sources, product_key()?).await?;
        let product_version = product_source.source_version();
        let variant_source = load_current_source_mutation(&runtime.sources, variant_key()?).await?;
        let variant_version = variant_source.source_version();

        let config = iggy.config("ack-redelivery");
        let first_transport = IggyTransport::new(config.clone()).await?;
        let first_group = Arc::new(
            first_transport
                .open_persistent_contract_consumer_group(
                    PRODUCT_INDEX_REFRESH_CONSUMER_GROUP,
                    PRODUCT_INDEX_REFRESH_TOPIC,
                )
                .await?,
        );

        let locale_event_id = Uuid::new_v4();
        let variant_event_id = Uuid::new_v4();
        first_transport
            .publish_contract(locale_envelope(locale_event_id, product_version)?)
            .await?;
        first_transport
            .publish_contract(variant_envelope(variant_event_id, variant_version)?)
            .await?;

        let first = receive_event(&first_group).await?;
        ensure_delivery_identity(&first, locale_event_id)?;
        let first_offset = required_offset(&first)?;
        let first_raw = first.raw_payload().to_vec();

        let fail_once = BrokerAcknowledger::new(Arc::clone(&first_group), true);
        let worker = ProductIndexRefreshDeliveryWorker::new(runtime.mutations.clone(), fail_once);
        let first_result = worker
            .process(
                runtime.schemas.registry(),
                &runtime.sources,
                &runtime.events,
                delivery_from_consumed(&first)?,
            )
            .await;
        if !matches!(
            first_result,
            Err(ProductIndexRefreshDeliveryProcessError::Process(
                IndexSourceRefreshEventProcessError::Acknowledge(_)
            ))
        ) {
            return Err(invalid_data(format!(
                "injected post-persistence acknowledgement failure returned {first_result:?}"
            ))
            .into());
        }

        assert_entity_version(
            &database.mutation,
            "product",
            4,
            PRODUCT_ID,
            "en",
            product_version,
        )
        .await?;
        assert_applied_inbox_once(&database.mutation, PRODUCT_SOURCE, locale_event_id).await?;

        drop(first_group);
        first_transport.shutdown().await?;
        drop(first_transport);

        let restarted_transport = IggyTransport::new(config).await?;
        let restarted_group = Arc::new(
            restarted_transport
                .open_persistent_contract_consumer_group(
                    PRODUCT_INDEX_REFRESH_CONSUMER_GROUP,
                    PRODUCT_INDEX_REFRESH_TOPIC,
                )
                .await?,
        );

        let redelivered = receive_event(&restarted_group).await?;
        ensure_delivery_identity(&redelivered, locale_event_id)?;
        if required_offset(&redelivered)? != first_offset {
            return Err(invalid_data("Product refresh restart did not resume the uncommitted offset").into());
        }
        if redelivered.raw_payload() != first_raw.as_slice() {
            return Err(invalid_data("Product refresh restart changed exact broker payload bytes").into());
        }

        let restarted_worker = ProductIndexRefreshDeliveryWorker::new(
            runtime.mutations.clone(),
            BrokerAcknowledger::new(Arc::clone(&restarted_group), false),
        );
        let duplicate = restarted_worker
            .process(
                runtime.schemas.registry(),
                &runtime.sources,
                &runtime.events,
                delivery_from_consumed(&redelivered)?,
            )
            .await?;
        if duplicate.mutation_outcome() != IndexReplayMutationOutcome::Duplicate {
            return Err(invalid_data(format!(
                "redelivered Product mutation was not durable-inbox Duplicate: {:?}",
                duplicate.mutation_outcome()
            ))
            .into());
        }
        assert_applied_inbox_once(&database.mutation, PRODUCT_SOURCE, locale_event_id).await?;

        let variant = receive_event(&restarted_group).await?;
        ensure_delivery_identity(&variant, variant_event_id)?;
        let variant_offset = required_offset(&variant)?;
        if variant_offset <= first_offset {
            return Err(invalid_data("successful Product acknowledgement did not advance the Iggy group").into());
        }
        let applied_variant = restarted_worker
            .process(
                runtime.schemas.registry(),
                &runtime.sources,
                &runtime.events,
                delivery_from_consumed(&variant)?,
            )
            .await?;
        if applied_variant.mutation_outcome()
            != IndexReplayMutationOutcome::Applied { source_version: variant_version }
        {
            return Err(invalid_data(format!(
                "ProductVariant refresh did not apply current owner state: {:?}",
                applied_variant.mutation_outcome()
            ))
            .into());
        }
        assert_entity_version(
            &database.mutation,
            "product_variant",
            2,
            VARIANT_ID,
            "",
            variant_version,
        )
        .await?;
        assert_applied_inbox_once(
            &database.mutation,
            PRODUCT_VARIANT_SOURCE,
            variant_event_id,
        )
        .await?;

        restarted_transport.shutdown().await?;
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;
    let cleanup = database.cleanup().await;
    result?;
    cleanup
}

async fn run_behind_source_redelivery(
    database_url: &str,
    iggy: &ExternalIggyInputs,
) -> TestResult<()> {
    let database = TestDatabase::setup(database_url, "behind_source").await?;
    let result = async {
        let runtime = build_runtime(&database).await?;
        let source = load_current_source_mutation(&runtime.sources, product_key()?).await?;
        let actual_version = source.source_version();
        let required_version = actual_version
            .checked_add(1)
            .ok_or_else(|| invalid_data("Product source version cannot advance for behind-source proof"))?;

        let config = iggy.config("behind-source");
        let first_transport = IggyTransport::new(config.clone()).await?;
        let first_group = Arc::new(
            first_transport
                .open_persistent_contract_consumer_group(
                    PRODUCT_INDEX_REFRESH_CONSUMER_GROUP,
                    PRODUCT_INDEX_REFRESH_TOPIC,
                )
                .await?,
        );
        let event_id = Uuid::new_v4();
        first_transport
            .publish_contract(locale_envelope(event_id, required_version)?)
            .await?;

        let first = receive_event(&first_group).await?;
        ensure_delivery_identity(&first, event_id)?;
        let first_offset = required_offset(&first)?;
        let first_raw = first.raw_payload().to_vec();
        let worker = ProductIndexRefreshDeliveryWorker::new(
            runtime.mutations.clone(),
            BrokerAcknowledger::new(Arc::clone(&first_group), false),
        );
        let result = worker
            .process(
                runtime.schemas.registry(),
                &runtime.sources,
                &runtime.events,
                delivery_from_consumed(&first)?,
            )
            .await;
        if !matches!(
            result,
            Err(ProductIndexRefreshDeliveryProcessError::Process(
                IndexSourceRefreshEventProcessError::SourceVersionBehind { .. }
            ))
        ) {
            return Err(invalid_data(format!(
                "behind Product source did not fail closed before acknowledgement: {result:?}"
            ))
            .into());
        }
        assert_inbox_absent(&database.mutation, PRODUCT_SOURCE, event_id).await?;

        drop(first_group);
        first_transport.shutdown().await?;
        drop(first_transport);

        let restarted_transport = IggyTransport::new(config).await?;
        let restarted_group = restarted_transport
            .open_persistent_contract_consumer_group(
                PRODUCT_INDEX_REFRESH_CONSUMER_GROUP,
                PRODUCT_INDEX_REFRESH_TOPIC,
            )
            .await?;
        let redelivered = receive_event(&restarted_group).await?;
        ensure_delivery_identity(&redelivered, event_id)?;
        if required_offset(&redelivered)? != first_offset {
            return Err(invalid_data("behind-source restart did not return the same uncommitted offset").into());
        }
        if redelivered.raw_payload() != first_raw.as_slice() {
            return Err(invalid_data("behind-source restart changed broker payload bytes").into());
        }

        drop(restarted_group);
        restarted_transport.shutdown().await?;
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;
    let cleanup = database.cleanup().await;
    result?;
    cleanup
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
        .ok_or_else(|| invalid_data("Index schema registry is missing"))?;
    let schema_store = PostgresSchemaRegistrationStore::new(database.mutation.clone());
    for registered in schemas.registry().iter() {
        schema_store.register(TENANT_ID, &registered.schema).await?;
    }
    materialize_postgres_index_sources(&mut extensions, database.source.clone())?;
    let sources = materialize_index_source_registry(&extensions)?
        .ok_or_else(|| invalid_data("Index source registry is missing"))?;
    let events = materialize_index_mutation_event_registry(&extensions)?
        .ok_or_else(|| invalid_data("Index mutation event registry is missing"))?;
    Ok(Runtime {
        schemas,
        sources,
        events,
        mutations: PostgresMutationStore::new(database.mutation.clone()),
    })
}

async fn load_current_source_mutation(
    sources: &SharedIndexSourceRegistry,
    key: EntityKey,
) -> TestResult<IndexMutation> {
    let request = IndexSourceLoadRequest::new(vec![key])?;
    let mut mutations = sources.load(request).await?.into_mutations();
    if mutations.len() != 1 {
        return Err(invalid_data(format!(
            "expected exactly one authoritative Product source mutation, got {}",
            mutations.len()
        ))
        .into());
    }
    Ok(mutations.remove(0))
}

fn delivery_from_consumed(
    consumed: &ConsumedContractEvent,
) -> TestResult<ProductIndexRefreshDelivery<ConsumedContractEvent>> {
    let delivery = match consumed.envelope.payload()? {
        ContractEventPayload::ProductIndexRefresh(
            ProductIndexRefreshEvent::LocaleRefreshRequested {
                product_id,
                locale,
                source_version,
            },
        ) => ProductIndexRefreshDelivery::locale(
            consumed.envelope.id(),
            consumed.envelope.tenant_id(),
            *product_id,
            locale.clone(),
            *source_version,
            consumed.clone(),
        ),
        ContractEventPayload::ProductIndexRefresh(
            ProductIndexRefreshEvent::VariantRefreshRequested {
                product_id,
                variant_id,
                source_version,
            },
        ) => ProductIndexRefreshDelivery::variant(
            consumed.envelope.id(),
            consumed.envelope.tenant_id(),
            *product_id,
            *variant_id,
            *source_version,
            consumed.clone(),
        ),
        other => {
            return Err(invalid_data(format!(
                "unexpected contract payload on Product refresh evidence cursor: {other:?}"
            ))
            .into());
        }
    };
    Ok(delivery)
}

fn locale_envelope(event_id: Uuid, source_version: u64) -> TestResult<ContractEventEnvelope> {
    Ok(ContractEventEnvelope::new_with_envelope_id(
        event_id,
        TENANT_ID,
        None,
        ProductIndexRefreshEvent::LocaleRefreshRequested {
            product_id: PRODUCT_ID,
            locale: "en".to_owned(),
            source_version,
        },
    )?)
}

fn variant_envelope(event_id: Uuid, source_version: u64) -> TestResult<ContractEventEnvelope> {
    Ok(ContractEventEnvelope::new_with_envelope_id(
        event_id,
        TENANT_ID,
        None,
        ProductIndexRefreshEvent::VariantRefreshRequested {
            product_id: PRODUCT_ID,
            variant_id: VARIANT_ID,
            source_version,
        },
    )?)
}

async fn receive_event(group: &PersistentContractConsumerGroup) -> TestResult<ConsumedContractEvent> {
    let delivery = timeout(RECEIVE_TIMEOUT, group.receive_delivery())
        .await
        .map_err(|_| invalid_data("timed out waiting for Product Index refresh Iggy delivery"))??
        .ok_or_else(|| invalid_data("Product Index refresh Iggy cursor ended before delivery"))?;
    match delivery {
        PersistentContractDelivery::Event(consumed) => Ok(consumed),
        PersistentContractDelivery::DecodeFailure(failure) => Err(invalid_data(format!(
            "canonical Product refresh decoded as poison: {}",
            failure.stable_error_code()
        ))
        .into()),
    }
}

fn ensure_delivery_identity(delivery: &ConsumedContractEvent, event_id: Uuid) -> TestResult<()> {
    if delivery.topic != PRODUCT_INDEX_REFRESH_TOPIC
        || delivery.envelope.id() != event_id
        || delivery.envelope.tenant_id() != TENANT_ID
        || delivery.offset().is_none()
        || delivery.ack_token().is_none()
        || delivery.raw_payload().is_empty()
    {
        return Err(invalid_data(format!(
            "unexpected Product Index refresh broker identity: {delivery:?}"
        ))
        .into());
    }
    delivery.validate_connector_metadata()?;
    delivery.envelope.validate_registered_schema()?;
    Ok(())
}

fn required_offset(delivery: &ConsumedContractEvent) -> TestResult<u64> {
    delivery
        .offset()
        .ok_or_else(|| invalid_data("Product Index refresh delivery has no broker offset").into())
}

async fn assert_entity_version(
    db: &DatabaseConnection,
    entity_name: &str,
    schema_version: i64,
    entity_id: Uuid,
    locale_key: &str,
    expected: u64,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT CAST(source_version AS TEXT) AS source_version_text
FROM index_entities
WHERE tenant_id = $1
  AND module_name = 'rustok-product'
  AND entity_name = $2
  AND schema_version = $3
  AND entity_id = $4
  AND locale_key = $5
  AND is_deleted = FALSE
"#,
            vec![
                TENANT_ID.into(),
                entity_name.to_owned().into(),
                schema_version.into(),
                entity_id.into(),
                locale_key.to_owned().into(),
            ],
        ))
        .await?
        .ok_or_else(|| invalid_data("expected materialized Product Index entity is missing"))?;
    let actual: String = row.try_get("", "source_version_text")?;
    if actual.parse::<u64>()? != expected {
        return Err(invalid_data(format!(
            "materialized Product Index source version {actual} != {expected}"
        ))
        .into());
    }
    Ok(())
}

async fn assert_applied_inbox_once(
    db: &DatabaseConnection,
    source_name: &str,
    event_id: Uuid,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT COUNT(*) AS row_count,
       COUNT(*) FILTER (WHERE state = 'applied') AS applied_count
FROM index_inbox
WHERE tenant_id = $1 AND source_name = $2 AND delivery_id = $3
"#,
            vec![
                TENANT_ID.into(),
                source_name.to_owned().into(),
                event_id.to_string().into(),
            ],
        ))
        .await?
        .ok_or_else(|| invalid_data("Index inbox aggregate row is missing"))?;
    let row_count: i64 = row.try_get("", "row_count")?;
    let applied_count: i64 = row.try_get("", "applied_count")?;
    if row_count != 1 || applied_count != 1 {
        return Err(invalid_data(format!(
            "expected one applied Index inbox row, got rows={row_count}, applied={applied_count}"
        ))
        .into());
    }
    Ok(())
}

async fn assert_inbox_absent(
    db: &DatabaseConnection,
    source_name: &str,
    event_id: Uuid,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS row_count FROM index_inbox WHERE tenant_id = $1 AND source_name = $2 AND delivery_id = $3",
            vec![
                TENANT_ID.into(),
                source_name.to_owned().into(),
                event_id.to_string().into(),
            ],
        ))
        .await?
        .ok_or_else(|| invalid_data("Index inbox count row is missing"))?;
    let row_count: i64 = row.try_get("", "row_count")?;
    if row_count != 0 {
        return Err(invalid_data(format!(
            "behind-source delivery unexpectedly entered Index inbox: {row_count}"
        ))
        .into());
    }
    Ok(())
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
    ('{PRODUCT_TRANSLATION_ID}', '{PRODUCT_ID}', '{TENANT_ID}', 'en', 'Refresh Product', 'refresh-product');

INSERT INTO product_variants (id, product_id, tenant_id, sku) VALUES
    ('{VARIANT_ID}', '{PRODUCT_ID}', '{TENANT_ID}', 'refresh-variant');
"#
    ))
    .await?;
    Ok(())
}

#[derive(Clone)]
struct ExternalIggyInputs {
    address: String,
    username: String,
    password: String,
}

impl ExternalIggyInputs {
    fn config(&self, scope: &str) -> IggyConfig {
        IggyConfig {
            mode: IggyMode::External,
            serialization: SerializationFormat::Json,
            external: ExternalConfig {
                addresses: vec![self.address.clone()],
                protocol: "tcp".to_owned(),
                username: self.username.clone(),
                password: self.password.clone(),
                tls_enabled: false,
                tls_domain: None,
                tls_ca_file: None,
            },
            topology: TopologyConfig {
                stream_name: format!(
                    "rustok-product-refresh-{}-{}",
                    sanitize_identifier(scope),
                    Uuid::new_v4().simple()
                ),
                domain_partitions: 1,
                replication_factor: 1,
            },
            ..IggyConfig::default()
        }
    }
}

fn postgres_database_url() -> TestResult<Option<String>> {
    match env::var(DATABASE_ENV) {
        Ok(value) => {
            let value = bounded_env(DATABASE_ENV, value, 2048)?;
            if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
                return Err(invalid_data(format!(
                    "{DATABASE_ENV} must be a PostgreSQL URL"
                ))
                .into());
            }
            Ok(Some(value))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn external_iggy_inputs() -> TestResult<Option<ExternalIggyInputs>> {
    let address = match env::var(IGGY_ADDRESS_ENV) {
        Ok(value) => bounded_env(IGGY_ADDRESS_ENV, value, 255)?,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if address.contains("://") || address.contains('@') || address.contains('?') {
        return Err(invalid_data(format!(
            "{IGGY_ADDRESS_ENV} must be host:port without scheme, credentials, or query"
        ))
        .into());
    }
    let username = optional_bounded_env(IGGY_USERNAME_ENV, 191)?;
    let password = optional_bounded_env(IGGY_PASSWORD_ENV, 191)?;
    if username.is_empty() != password.is_empty() {
        return Err(invalid_data(
            "Product refresh Iggy username and password must both be set or both be empty",
        )
        .into());
    }
    Ok(Some(ExternalIggyInputs {
        address,
        username,
        password,
    }))
}

fn optional_bounded_env(name: &'static str, max_len: usize) -> TestResult<String> {
    match env::var(name) {
        Ok(value) => Ok(bounded_env(name, value, max_len)?),
        Err(env::VarError::NotPresent) => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

fn bounded_env(name: &'static str, value: String, max_len: usize) -> Result<String, IoError> {
    if value.trim() != value || value.is_empty() {
        return Err(invalid_data(format!(
            "{name} must be non-empty and have no surrounding whitespace"
        )));
    }
    if value.len() > max_len {
        return Err(invalid_data(format!("{name} exceeds the evidence limit")));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_data(format!(
            "{name} must not contain control characters"
        )));
    }
    Ok(value)
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
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn invalid_data(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidData, message.into())
}
