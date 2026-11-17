/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::{env, error::Error, sync::Arc, time::Duration};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    IndexModule, IndexSourceRefreshEventDelivery, IndexSourceRefreshEventProcessError,
    PostgresMutationStore, PostgresSchemaRegistrationStore, SharedIndexMutationEventRegistry,
    SharedIndexSchemaRegistry, materialize_index_source_registry,
    materialize_postgres_index_sources, persist_index_source_refresh,
};
use rustok_product::{
    PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE, ProductModule,
    entities::{Product, ProductVariant, product::ProductStatus, product_translation},
    migrations::m20261116_000041_move_product_attached_localized_values::Migration as ValuesMigration,
};
use rustok_product_ports::grpc::{
    GrpcProductCatalogReadConnectionConfig, ProductCatalogGrpcBearerToken,
};
use rustok_test_utils::{
    connect_postgres, create_postgres_database, drop_postgres_database_if_exists,
    postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DbBackend, Schema, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tokio::{net::TcpListener, process::Command};

use super::*;

const READ_TOKEN: &str = "catalog-read-fixture";
const REPLAY_TOKEN: &str = "variant-replay-fixture";
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[tokio::test]
#[ignore = "requires disposable PostgreSQL databases and the compiled Product service binary"]
async fn standalone_product_variant_replay_materializes_in_a_separate_index_database() -> TestResult
{
    let admin_url = env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    let service_path = env::var_os("RUSTOK_PRODUCT_VARIANT_REPLAY_TEST_BINARY")
        .ok_or("RUSTOK_PRODUCT_VARIANT_REPLAY_TEST_BINARY is required")?;
    let admin = connect_postgres(&admin_url).await?;
    let owner_name = unique_postgres_database_name("variant_replay_owner");
    let index_name = unique_postgres_database_name("variant_replay_index");
    create_postgres_database(&admin, &owner_name).await?;
    let result = async {
        create_postgres_database(&admin, &index_name).await?;
        let owner_url = postgres_database_url(&admin_url, &owner_name);
        let owner = connect_postgres(&owner_url).await?;
        let index = connect_postgres(&postgres_database_url(&admin_url, &index_name)).await?;
        let tenant = Uuid::new_v4();
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        prepare_owner(&owner, tenant, [first, second]).await?;
        index
            .execute_unprepared("CREATE TABLE tenants (id uuid PRIMARY KEY)")
            .await?;
        index
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO tenants (id) VALUES ($1)",
                [tenant.into()],
            ))
            .await?;
        let index_manager = SchemaManager::new(&index);
        for migration in IndexModule.migrations() {
            migration.up(&index_manager).await?;
        }
        assert!(!index_manager.has_table("product_variants").await?);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let bind = listener.local_addr()?;
        drop(listener);
        let mut command = Command::new(service_path);
        for (key, _) in
            env::vars_os().filter(|(key, _)| key.to_string_lossy().starts_with("RUSTOK_"))
        {
            command.env_remove(key);
        }
        let mut child = command
            .env("RUSTOK_PRODUCT_CATALOG_DATABASE_URL", &owner_url)
            .env("RUSTOK_PRODUCT_CATALOG_SERVICE_BIND", bind.to_string())
            .env(
                "RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK",
                "true",
            )
            .env("RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN", READ_TOKEN)
            .env(
                "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
                "test.catalog",
            )
            .env(
                "RUSTOK_PRODUCT_VARIANT_REPLAY_GRPC_BEARER_TOKEN",
                REPLAY_TOKEN,
            )
            .kill_on_drop(true)
            .spawn()?;

        let connection = GrpcProductCatalogReadConnectionConfig::new(format!("http://{bind}"))
            .allow_insecure_loopback(true);
        let remote = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(status) = child.try_wait()? {
                    return Err(std::io::Error::other(format!(
                        "Product service exited: {status}"
                    )));
                }
                if let Ok(remote) = connection.clone().connect().await {
                    return Ok(remote);
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await??;

        let denied = connection
            .connect()
            .await?
            .with_variant_replay_authentication(ProductCatalogGrpcBearerToken::new(READ_TOKEN)?);
        let error = denied
            .load_variant_replay(replay_context(tenant), vec![first])
            .await
            .unwrap_err();
        assert_eq!(error.kind, rustok_api::PortErrorKind::Forbidden);
        let remote =
            Arc::new(remote.with_variant_replay_authentication(
                ProductCatalogGrpcBearerToken::new(REPLAY_TOKEN)?,
            ));
        assert!(
            remote
                .load_variant_replay(replay_context(Uuid::new_v4()), vec![first])
                .await?
                .is_empty()
        );

        let outcome = verify_index(&owner, &index, remote, tenant, first, second).await;
        let stop = child.kill().await;
        outcome?;
        stop?;
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let index_cleanup = drop_postgres_database_if_exists(&admin, &index_name).await;
    let owner_cleanup = drop_postgres_database_if_exists(&admin, &owner_name).await;
    result?;
    index_cleanup?;
    owner_cleanup?;
    Ok(())
}

async fn prepare_owner(
    owner: &DatabaseConnection,
    tenant: Uuid,
    variants: [Uuid; 2],
) -> TestResult {
    let manager = SchemaManager::new(owner);
    let schema = Schema::new(DbBackend::Postgres);
    manager
        .create_type(
            schema
                .create_enum_from_active_enum::<ProductStatus>()
                .ok_or("Product status enum unavailable")?,
        )
        .await?;
    manager
        .create_table(schema.create_table_from_entity(Product))
        .await?;
    manager
        .create_table(schema.create_table_from_entity(ProductVariant))
        .await?;
    manager
        .create_table(schema.create_table_from_entity(product_translation::Entity))
        .await?;
    owner
        .execute_unprepared("CREATE UNIQUE INDEX uq_products_tenant_id ON products (tenant_id, id)")
        .await?;
    ValuesMigration.up(&manager).await?;
    rustok_outbox::SysEventsMigration.up(&manager).await?;
    let migrations = rustok_product::migrations::migrations();
    for name in [
        "m20260730_000001_add_product_index_revision",
        "m20260730_000002_add_product_variant_index_revision",
        "m20260731_000004_add_product_index_tombstones",
    ] {
        migrations
            .iter()
            .find(|migration| migration.name() == name)
            .ok_or_else(|| format!("missing migration {name}"))?
            .up(&manager)
            .await?;
    }

    let product_id = Uuid::new_v4();
    owner.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO products (id, tenant_id, status, metadata, revision, created_at, updated_at)
         VALUES ($1, $2, 'draft', '{}'::jsonb, 1, now(), now())",
        [product_id.into(), tenant.into()],
    )).await?;
    for variant in variants {
        owner
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO product_variants (id, product_id, tenant_id, sku, inventory_policy,
               inventory_management, inventory_quantity, position, created_at, updated_at)
             VALUES ($1, $2, $3, 'before', 'deny', 'native', 5, 0, now(), now())",
                [variant.into(), product_id.into(), tenant.into()],
            ))
            .await?;
    }
    Ok(())
}

async fn verify_index(
    owner: &DatabaseConnection,
    index: &DatabaseConnection,
    remote: Arc<dyn ProductVariantReplayPort>,
    tenant: Uuid,
    first: Uuid,
    second: Uuid,
) -> TestResult {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_channel::ChannelModule)
        .register(ProductModule);
    let mut extensions = crate::build_runtime_extensions(&registry)?;
    extensions.insert(remote);
    materialize_postgres_index_sources(&mut extensions, index.clone())?;
    let sources =
        materialize_index_source_registry(&extensions)?.ok_or("missing source registry")?;
    let schema_registry = extensions
        .get::<SharedIndexSchemaRegistry>()
        .ok_or("missing schema registry")?;
    let event_registry = extensions
        .get::<SharedIndexMutationEventRegistry>()
        .ok_or("missing event registry")?;
    let schema = product_variant_schema()?;
    PostgresSchemaRegistrationStore::new(index.clone())
        .register(tenant, &schema)
        .await?;
    let first_page = sources
        .scan(IndexSourceScanRequest::new(
            tenant,
            schema.reference.clone(),
            None,
            1,
        )?)
        .await?;
    assert_eq!(first_page.mutations()[0].key().entity_id, first);
    let second_page = sources
        .scan(IndexSourceScanRequest::new(
            tenant,
            schema.reference.clone(),
            first_page.next_cursor().cloned(),
            1,
        )?)
        .await?;
    assert_eq!(second_page.mutations()[0].key().entity_id, second);
    assert!(second_page.next_cursor().is_none());

    let local = ProductVariantIndexSource {
        port: Arc::new(PostgresProductVariantReplay::new(owner.clone())),
    };
    let key = EntityKey {
        tenant_id: tenant,
        schema: schema.reference,
        entity_id: first,
        locale: None,
    };
    let request = IndexSourceLoadRequest::new(vec![key.clone()])?;
    assert_eq!(
        sources.load(request.clone()).await?.into_mutations(),
        local.load(request).await?.into_mutations()
    );

    let sink = PostgresMutationStore::new(index.clone());
    let refresh = |version| {
        IndexSourceRefreshEventDelivery::new(
            PRODUCT_INDEX_VARIANT_REFRESH_REQUESTED_EVENT_TYPE,
            Uuid::new_v4(),
            key.clone(),
            version,
            (),
        )
    };
    let first_refresh = refresh(1)?;
    persist_index_source_refresh(
        &sink,
        schema_registry.registry(),
        &sources,
        event_registry,
        &first_refresh,
    )
    .await?;
    let row = stored_row(index, first).await?;
    assert_eq!(
        row.try_get::<Option<String>>("", "sku")?.as_deref(),
        Some("before")
    );
    assert_eq!(row.try_get::<String>("", "revision")?, "1");
    let update_refresh = refresh(2)?;
    assert!(matches!(
        persist_index_source_refresh(
            &sink,
            schema_registry.registry(),
            &sources,
            event_registry,
            &update_refresh
        )
        .await,
        Err(IndexSourceRefreshEventProcessError::SourceVersionBehind {
            minimum: 2,
            actual: 1,
            ..
        })
    ));
    owner
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE product_variants SET sku = 'after' WHERE id = $1",
            [first.into()],
        ))
        .await?;
    persist_index_source_refresh(
        &sink,
        schema_registry.registry(),
        &sources,
        event_registry,
        &update_refresh,
    )
    .await?;
    let row = stored_row(index, first).await?;
    assert_eq!(
        row.try_get::<Option<String>>("", "sku")?.as_deref(),
        Some("after")
    );
    assert_eq!(row.try_get::<String>("", "revision")?, "2");
    owner
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM product_variants WHERE id = $1",
            [first.into()],
        ))
        .await?;
    persist_index_source_refresh(
        &sink,
        schema_registry.registry(),
        &sources,
        event_registry,
        &refresh(3)?,
    )
    .await?;
    let row = stored_row(index, first).await?;
    assert!(row.try_get::<bool>("", "is_deleted")?);
    assert_eq!(row.try_get::<String>("", "revision")?, "3");
    Ok(())
}

async fn stored_row(index: &DatabaseConnection, id: Uuid) -> TestResult<sea_orm::QueryResult> {
    Ok(index
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload#>>'{sku,value}' AS sku, source_version::text AS revision, is_deleted
         FROM index_entities WHERE entity_id = $1 AND entity_name = 'product_variant'",
            [id.into()],
        ))
        .await?
        .ok_or("missing materialized variant")?)
}
