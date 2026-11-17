#![cfg(feature = "service")]

use std::{path::Path, time::Duration};

use anyhow::{Context, Result, bail};
use rustok_events::{ContractEventPayload, DomainEvent, ProductIndexRefreshEvent};
use rustok_iggy::{
    ExternalConfig, IggyConfig, IggyMode, IggyTransport, PersistentContractConsumerGroup,
    TopologyConfig,
};
use rustok_outbox::{SysEvents, SysEventsMigration, TransactionalEventBus, entity::SysEventStatus};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, EntityTrait, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tokio::process::{Child, Command};
use url::Url;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and Iggy fixtures"]
async fn standalone_relay_delivers_committed_product_facts_through_iggy() -> Result<()> {
    let admin_url = std::env::var("RUSTOK_OUTBOX_TEST_POSTGRES_ADMIN_URL")?;
    let database_name = format!("outbox_{}", Uuid::new_v4().simple());
    let admin = Database::connect(&admin_url).await?;
    let mut url = Url::parse(&admin_url)?;
    url.set_path(&database_name);

    let config = IggyConfig {
        mode: IggyMode::External,
        external: ExternalConfig {
            addresses: vec![std::env::var("RUSTOK_OUTBOX_TEST_IGGY_ADDRESS")?],
            username: std::env::var("RUSTOK_OUTBOX_TEST_IGGY_USERNAME")?,
            password: std::env::var("RUSTOK_OUTBOX_TEST_IGGY_PASSWORD")?,
            ..Default::default()
        },
        topology: TopologyConfig {
            stream_name: format!("outbox-{}", Uuid::new_v4().simple()),
            domain_partitions: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let config_path = std::env::temp_dir().join(format!("{database_name}.json"));
    tokio::fs::write(&config_path, serde_json::to_vec(&config)?).await?;
    let result = async {
        admin
            .execute_unprepared(&format!("CREATE DATABASE {database_name}"))
            .await?;
        verify_delivery(url.as_str(), &config_path, config).await
    }
    .await;
    let remove_database = admin
        .execute_unprepared(&format!(
            "DROP DATABASE IF EXISTS {database_name} WITH (FORCE)"
        ))
        .await;
    let remove_config = tokio::fs::remove_file(config_path).await;
    result?;
    remove_database?;
    remove_config?;
    Ok(())
}

async fn verify_delivery(database_url: &str, config_path: &Path, config: IggyConfig) -> Result<()> {
    let database = Database::connect(database_url).await?;
    SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await?;
    let broker = tokio::time::timeout(Duration::from_secs(15), IggyTransport::new(config))
        .await
        .context("Iggy fixture did not become ready")??;
    let consumer = broker
        .open_persistent_contract_consumer_group("outbox-service-test", "domain")
        .await?;
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let mut relay = start_relay(database_url, config_path)?;

    let first = commit_product_events(&database, tenant_id, product_id, 1).await?;
    verify_received(&consumer, &mut relay, &database, first).await?;
    relay.kill().await?;

    let second = commit_product_events(&database, tenant_id, product_id, 2).await?;
    assert_eq!(
        SysEvents::find_by_id(second.refresh)
            .one(&database)
            .await?
            .context("pending refresh")?
            .status,
        SysEventStatus::Pending
    );
    let mut relay = start_relay(database_url, config_path)?;
    verify_received(&consumer, &mut relay, &database, second).await?;
    relay.kill().await?;
    broker.shutdown().await?;
    Ok(())
}

fn start_relay(database_url: &str, config_path: &Path) -> Result<Child> {
    Ok(Command::new(env!("CARGO_BIN_EXE_rustok-outbox-service"))
        .env("RUSTOK_OUTBOX_DATABASE_URL", database_url)
        .env("RUSTOK_OUTBOX_IGGY_CONFIG_PATH", config_path)
        .kill_on_drop(true)
        .spawn()?)
}

struct ProductEvents {
    tenant_id: Uuid,
    product_id: Uuid,
    root: Uuid,
    refresh: Uuid,
    source_version: u64,
}

async fn commit_product_events(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Uuid,
    source_version: u64,
) -> Result<ProductEvents> {
    let txn = database.begin().await?;
    let root = TransactionalEventBus::publish_root_in_tx_with_envelope_id(
        &txn,
        tenant_id,
        None,
        DomainEvent::ProductUpdated { product_id },
    )
    .await?;
    let refresh = Uuid::new_v4();
    TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation(
        &txn,
        refresh,
        tenant_id,
        None,
        root,
        ProductIndexRefreshEvent::LocaleRefreshRequested {
            product_id,
            locale: "en".to_owned(),
            source_version,
        },
    )
    .await
    .map_err(|error| anyhow::anyhow!("refresh write failed: {error:?}"))?;
    txn.commit().await?;
    Ok(ProductEvents {
        tenant_id,
        product_id,
        root,
        refresh,
        source_version,
    })
}

async fn verify_received(
    consumer: &PersistentContractConsumerGroup,
    relay: &mut Child,
    database: &DatabaseConnection,
    events: ProductEvents,
) -> Result<()> {
    let ProductEvents {
        tenant_id,
        product_id,
        root,
        refresh,
        source_version,
    } = events;
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut root_received = false;
        let mut refresh_received = false;
        while !(root_received && refresh_received) {
            if let Some(exit) = relay.try_wait()? {
                bail!("Outbox relay exited: {exit}");
            }
            let Some(delivered) = consumer.receive().await? else {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            };
            let envelope = &delivered.envelope;
            assert_eq!(delivered.partition, 1);
            assert_eq!(envelope.tenant_id(), tenant_id);
            if envelope.id() == root && !root_received {
                assert_eq!(
                    envelope.payload()?,
                    &ContractEventPayload::Root(DomainEvent::ProductUpdated { product_id })
                );
                root_received = true;
            } else if envelope.id() == refresh && !refresh_received {
                assert_eq!(envelope.causation_id(), Some(root));
                assert_eq!(
                    envelope.payload()?,
                    &ContractEventPayload::ProductIndexRefresh(
                        ProductIndexRefreshEvent::LocaleRefreshRequested {
                            product_id,
                            locale: "en".to_owned(),
                            source_version
                        }
                    )
                );
                refresh_received = true;
            } else {
                bail!(
                    "Unexpected Outbox envelope in the isolated Iggy stream: {}",
                    envelope.id()
                );
            }
            consumer.acknowledge(&delivered).await?;
        }
        for id in [root, refresh] {
            loop {
                let status = SysEvents::find_by_id(id)
                    .one(database)
                    .await?
                    .context("Outbox event is missing")?
                    .status;
                if status == SysEventStatus::Dispatched {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        Ok(())
    })
    .await
    .context("Outbox events did not reach Iggy")?
}
