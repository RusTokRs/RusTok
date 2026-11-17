use std::{sync::Arc, time::Duration};

use flex::{FlexProductSchemaReadService, FlexVersionedAttachedSchemaReadPort};
use flex_ports::grpc::{FlexAttachedSchemaGrpcService, GrpcFlexAttachedSchemaProvider};
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_events::{ContractEventPayload, DomainEvent, EventEnvelope, ProductIndexRefreshEvent};
use rustok_outbox::OutboxRelay;
use rustok_outbox_storage::{OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_product::{
    migrations::m20261116_000042_add_product_flex_schema_projection::Migration,
    services::flex_schema_projection::{
        ProductFlexSchemaApplyOutcome::*, ProductFlexSchemaProjection,
    },
};
use rustok_product_ports::{
    ProductAttachedValuesWritePort,
    grpc::{GrpcProductCatalogCommandProvider, ProductCatalogGrpcBearerToken},
};
use rustok_test_utils::{
    MockEventTransport, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, TransactionTrait};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

mod definitions {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

fn context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "test-flex-projection",
    )
    .with_deadline(Duration::from_secs(5))
}

async fn verify_projection(
    source: DatabaseConnection,
    target: DatabaseConnection,
    target_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    use sea_orm::{ActiveModelTrait, Schema, Set};
    let source_manager = SchemaManager::new(&source);
    source_manager
        .create_table(
            Schema::new(source.get_database_backend())
                .create_table_from_entity(definitions::Entity),
        )
        .await?;
    flex::cache_generation::create_field_definition_cache_generation_table(&source_manager).await?;
    let legacy_trigger = rustok_product::migrations::migrations()
        .into_iter()
        .find(|migration| {
            migration.name() == "m20260716_000002_add_product_field_cache_generation_trigger"
        })
        .expect("Product must retain the shared-profile trigger migration");
    legacy_trigger.up(&source_manager).await?;
    let now = chrono::Utc::now();
    definitions::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant),
        field_key: Set("tagline".into()),
        field_type: Set("text".into()),
        label: Set(serde_json::json!({"en": "Tagline", "ru": "Слоган"})),
        description: Set(Some(serde_json::json!({"en": "Public description"}))),
        is_localized: Set(true),
        is_required: Set(false),
        default_value: Set(Some(serde_json::json!("New product"))),
        validation: Set(None),
        position: Set(0),
        is_active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&source)
    .await?;
    assert_eq!(cache_generation(&source).await?, 1);

    let target_manager = SchemaManager::new(&target);
    Migration.up(&source_manager).await?;
    Migration.up(&target_manager).await?;
    assert!(
        !target_manager
            .has_table("product_field_definitions")
            .await?
    );
    assert!(
        !target_manager
            .has_table("flex_product_attached_schema_revisions")
            .await?
    );

    let owner = Arc::new(FlexProductSchemaReadService::new(source.clone()));
    owner.verify_storage().await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let token = "flex-fixture-token";
    let service = FlexAttachedSchemaGrpcService::new(owner.clone(), token, [tenant, other_tenant])?
        .into_server();
    let task = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener)),
    );
    let provider = GrpcFlexAttachedSchemaProvider::new(&endpoint, token, true)?;
    let unauthorized = GrpcFlexAttachedSchemaProvider::new(&endpoint, "wrong-token", true)?;
    let error = unauthorized
        .read_versioned_attached_schema(context(tenant), "product")
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    let error = provider
        .read_versioned_attached_schema(context(Uuid::new_v4()), "product")
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    let error = provider
        .read_versioned_attached_schema(context(tenant), "other")
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);

    let store = ProductFlexSchemaProjection::new(target.clone());
    let initial = provider
        .read_versioned_attached_schema(context(tenant), "product")
        .await?;
    assert_eq!(
        initial,
        owner
            .read_versioned_attached_schema(context(tenant), "product")
            .await?
    );
    assert_eq!(initial.revision, 0);
    assert_eq!(initial.schema.definitions.len(), 1);
    assert_eq!(store.refresh(context(tenant), &provider).await?, Advanced);
    assert_eq!(store.refresh(context(tenant), &provider).await?, Replay);

    let txn = source.begin().await?;
    txn.execute_unprepared("UPDATE product_field_definitions SET position = 4")
        .await?;
    txn.rollback().await?;
    assert_eq!(cache_generation(&source).await?, 1);
    assert_eq!(
        provider
            .read_versioned_attached_schema(context(tenant), "product")
            .await?,
        initial
    );

    source
        .execute_unprepared("UPDATE product_field_definitions SET position = 3")
        .await?;
    assert_eq!(cache_generation(&source).await?, 2);
    let reordered = provider
        .read_versioned_attached_schema(context(tenant), "product")
        .await?;
    assert_eq!(reordered.revision, 1);
    assert_eq!(reordered.schema.definitions[0].position, 3);
    assert_eq!(store.refresh(context(tenant), &provider).await?, Advanced);
    assert_eq!(store.apply(tenant, initial.clone()).await?, Stale);
    assert!(store.apply(other_tenant, reordered.clone()).await.is_err());
    let mut conflicting = reordered.clone();
    conflicting.schema.definitions[0].position = 5;
    assert_eq!(
        store.apply(tenant, conflicting).await.unwrap_err().code,
        "product.flex_schema_revision_conflict"
    );
    assert_eq!(store.apply(tenant, reordered).await?, Replay);

    legacy_trigger.down(&source_manager).await?;
    if source.get_database_backend() == DatabaseBackend::Postgres {
        source
            .execute_unprepared("TRUNCATE product_field_definitions")
            .await?;
    } else {
        source
            .execute_unprepared("DELETE FROM product_field_definitions")
            .await?;
    }
    assert_eq!(cache_generation(&source).await?, 2);
    let removed = provider
        .read_versioned_attached_schema(context(tenant), "product")
        .await?;
    assert_eq!(removed.revision, 2);
    assert!(removed.schema.definitions.is_empty());
    assert_eq!(store.apply(tenant, removed).await?, Advanced);
    let untouched = provider
        .read_versioned_attached_schema(context(other_tenant), "product")
        .await?;
    assert_eq!(untouched.revision, 0);
    assert!(untouched.schema.definitions.is_empty());
    assert_eq!(store.apply(other_tenant, untouched).await?, Advanced);

    if cfg!(feature = "service")
        && let Some(url) = target_url
    {
        let binary = option_env!("CARGO_BIN_EXE_rustok-product-catalog-service")
            .ok_or("Product service binary unavailable for the refresh test")?;
        verify_standalone_refresh(binary, url, &source, &target, &endpoint, token, tenant).await?;
    }

    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    Migration.down(&target_manager).await?;
    Migration.down(&source_manager).await?;
    assert!(
        !target_manager
            .has_table("product_flex_schema_projections")
            .await?
    );
    assert!(
        !source_manager
            .has_table("flex_product_attached_schema_revisions")
            .await?
    );
    source
        .execute_unprepared("DELETE FROM product_field_definitions")
        .await?;
    Ok(())
}

async fn cache_generation(db: &DatabaseConnection) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(),
            "SELECT generation FROM flex_field_definition_cache_generation WHERE id = 1",
        ))
        .await?
        .expect("shared cache generation row must exist");
    row.try_get("", "generation")
}

async fn verify_standalone_refresh(
    binary: &str,
    target_url: &str,
    source: &DatabaseConnection,
    target: &DatabaseConnection,
    endpoint: &str,
    token: &str,
    tenant: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    use rustok_product::{
        entities::{Product, ProductVariant, product, product_image, product_translation},
        migrations::m20261116_000041_move_product_attached_localized_values::Migration as ValuesMigration,
    };
    use sea_orm::{ActiveModelTrait, Schema, Set};
    let manager = SchemaManager::new(target);
    let schema = Schema::new(target.get_database_backend());
    manager
        .create_type(
            schema
                .create_enum_from_active_enum::<rustok_product::entities::product::ProductStatus>()
                .ok_or("PostgreSQL Product status enum unavailable")?,
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
    manager
        .create_table(schema.create_table_from_entity(product_image::Entity))
        .await?;
    target
        .execute_unprepared("CREATE UNIQUE INDEX uq_products_tenant_id ON products (tenant_id, id)")
        .await?;
    ValuesMigration.up(&manager).await?;
    rustok_outbox_storage::SysEventsMigration
        .up(&manager)
        .await?;
    let migrations = rustok_product::migrations::migrations();
    for name in [
        "m20260730_000001_add_product_index_revision",
        "m20260730_000002_add_product_variant_index_revision",
        "m20260731_000004_add_product_index_tombstones",
        "m20260806_000005_add_product_index_locale_refresh_ledger",
        "m20260806_000006_add_product_variant_index_refresh_ledger",
        "m20260806_000007_add_product_index_refresh_relay_cursors",
        "m20260907_000022_add_product_translation_change_journal",
        "m20260908_000023_add_product_variant_translation_change_journal",
        "m20260909_000025_add_product_image_translation_change_journal",
        "m20261116_000043_order_product_index_refresh_ledger_commits",
    ] {
        migrations
            .iter()
            .find(|migration| migration.name() == name)
            .unwrap_or_else(|| panic!("missing migration {name}"))
            .up(&manager)
            .await?;
    }
    verify_index_ledger_commit_order(target).await?;
    let embedded_product = Uuid::new_v4();
    let remote_product = Uuid::new_v4();
    let now = chrono::Utc::now();
    for id in [embedded_product, remote_product] {
        product::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant),
            status: Set(product::ProductStatus::Draft),
            metadata: Set(serde_json::json!({})),
            revision: Set(1),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(target)
        .await?;
        product_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant),
            product_id: Set(id),
            locale: Set("en".into()),
            title: Set("Fixture".into()),
            handle: Set(id.to_string()),
            ..Default::default()
        }
        .insert(target)
        .await?;
    }
    target
        .execute_unprepared("DELETE FROM product_flex_schema_projections")
        .await?;

    let mut command = tokio::process::Command::new(binary);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("RUSTOK_") {
            command.env_remove(key);
        }
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let bind = listener.local_addr()?;
    drop(listener);
    let actor = Uuid::new_v4();
    let attached_token = "catalog-attached-fixture";
    let mut child = command
        .env("RUSTOK_PRODUCT_CATALOG_DATABASE_URL", target_url)
        .env("RUSTOK_PRODUCT_CATALOG_SERVICE_BIND", bind.to_string())
        .env(
            "RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK",
            "true",
        )
        .env(
            "RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN",
            "catalog-fixture",
        )
        .env(
            "RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR",
            "test.catalog",
        )
        .env("RUSTOK_PRODUCT_USE_FLEX_SCHEMA_PROJECTION", "true")
        .env("RUSTOK_PRODUCT_FLEX_SCHEMA_GRPC_ENDPOINT", endpoint)
        .env("RUSTOK_FLEX_PRODUCT_SCHEMA_GRPC_BEARER_TOKEN", token)
        .env("RUSTOK_PRODUCT_FLEX_SCHEMA_TENANTS", tenant.to_string())
        .env("RUSTOK_PRODUCT_FLEX_SCHEMA_REFRESH_SECONDS", "1")
        .env("RUSTOK_PRODUCT_FLEX_SCHEMA_ALLOW_INSECURE_LOOPBACK", "true")
        .env(
            "RUSTOK_PRODUCT_CATALOG_COMMAND_GRPC_BEARER_TOKEN",
            "catalog-command-fixture",
        )
        .env("RUSTOK_PRODUCT_CATALOG_COMMAND_ACTOR_ID", actor.to_string())
        .env(
            "RUSTOK_PRODUCT_ATTACHED_VALUES_WRITE_GRPC_BEARER_TOKEN",
            attached_token,
        )
        .kill_on_drop(true)
        .spawn()?;
    wait_for_revision(&mut child, target, tenant, 2).await?;
    source.execute_raw(sea_orm::Statement::from_sql_and_values(
        source.get_database_backend(),
        "INSERT INTO product_field_definitions (id, tenant_id, field_key, field_type, label, is_localized, is_required, position, is_active, created_at, updated_at)
         VALUES ($1, $2, 'badge', 'text', '{\"en\":\"Badge\"}', false, false, 0, true, now(), now())",
        [Uuid::new_v4().into(), tenant.into()],
    )).await?;
    wait_for_revision(&mut child, target, tenant, 3).await?;

    let mut write_context = context(tenant).with_idempotency_key("remote-write");
    write_context.actor = PortActor::service(actor.to_string());
    let patch = Some(serde_json::json!({"badge": "gold"}));
    let embedded = rustok_product::services::CatalogService::new(
        target.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(target.clone()))),
    )
    .with_projected_flex_schema();
    let embedded_context = write_context.clone().with_idempotency_key("embedded-write");
    assert_eq!(
        embedded
            .update_attached_values(embedded_context.clone(), embedded_product, patch.clone())
            .await?,
        patch
    );
    assert_eq!(
        embedded
            .update_attached_values(embedded_context, embedded_product, patch.clone())
            .await?,
        patch
    );

    let catalog_endpoint = tonic::transport::Endpoint::from_shared(format!("http://{bind}"))?;
    let auth = ProductCatalogGrpcBearerToken::new(attached_token)?;
    let remote = GrpcProductCatalogCommandProvider::connect(catalog_endpoint.clone())
        .await?
        .with_attached_values_authentication(auth.clone());
    assert_eq!(
        remote
            .update_attached_values(write_context.clone(), remote_product, patch.clone())
            .await?,
        patch
    );
    child.kill().await?;
    child = command
        .env("RUSTOK_PRODUCT_INDEX_REFRESH_TENANTS", tenant.to_string())
        .env("RUSTOK_PRODUCT_INDEX_REFRESH_POLL_MS", "100")
        .spawn()?;
    let reopened = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(provider) =
                GrpcProductCatalogCommandProvider::connect(catalog_endpoint.clone()).await
            {
                break provider;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await?
    .with_attached_values_authentication(auth);
    assert_eq!(
        reopened
            .update_attached_values(write_context, remote_product, patch.clone())
            .await?,
        patch
    );
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let published = SysEvents::find()
                .filter(
                    rustok_outbox_storage::entity::Column::EventType
                        .eq(rustok_product::PRODUCT_INDEX_LOCALE_REFRESH_REQUESTED_EVENT_TYPE),
                )
                .count(target)
                .await?;
            if published >= 2 {
                break Ok::<_, sea_orm::DbErr>(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    child.kill().await?;
    assert_written_once_and_dispatched(target, tenant, [embedded_product, remote_product]).await?;
    Ok(())
}

async fn verify_index_ledger_commit_order(
    target: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    use rustok_product::services::ProductIndexLocaleRefreshSource;
    use sea_orm::{DbErr, RuntimeErr, Statement};

    fn append(tenant: Uuid) -> Statement {
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO product_index_locale_refresh_ledger
                (tenant_id, refresh_id, root_event_id, product_id, locale, source_version)
            VALUES ($1, $2, $3, $4, 'en', 1)
        "#,
            [
                tenant.into(),
                Uuid::new_v4().into(),
                Uuid::new_v4().into(),
                Uuid::new_v4().into(),
            ],
        )
    }

    let tenant = Uuid::new_v4();
    let first = target.begin().await?;
    first.execute_raw(append(tenant)).await?;
    let competing = target.begin().await?;
    competing
        .execute_unprepared("SET LOCAL lock_timeout = '150ms'")
        .await?;
    // Another tenant can advance while the first owner transaction is open.
    competing.execute_raw(append(Uuid::new_v4())).await?;
    let error = competing
        .execute_raw(append(tenant))
        .await
        .expect_err("same tenant must wait for the earlier insert to commit");
    let (DbErr::Query(RuntimeErr::SqlxError(error)) | DbErr::Exec(RuntimeErr::SqlxError(error))) =
        error
    else {
        return Err(error.into());
    };
    assert_eq!(
        error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("55P03")
    );
    competing.rollback().await?;
    let source = ProductIndexLocaleRefreshSource::new(target.clone());
    assert!(source.list(tenant, 0, 2).await?.is_empty());
    first.commit().await?;
    let first_row = source.list(tenant, 0, 2).await?;
    assert_eq!(first_row.len(), 1);
    target.execute_raw(append(tenant)).await?;
    assert_eq!(
        source
            .list(tenant, first_row[0].sequence_no(), 2)
            .await?
            .len(),
        1
    );
    Ok(())
}

async fn assert_written_once_and_dispatched(
    target: &DatabaseConnection,
    tenant: Uuid,
    products: [Uuid; 2],
) -> Result<(), Box<dyn std::error::Error>> {
    use rustok_outbox_storage::entity::SysEventStatus;
    use rustok_product::entities::Product;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let roots = SysEvents::find()
        .filter(rustok_outbox_storage::entity::Column::EventType.eq("product.updated"))
        .all(target)
        .await?
        .into_iter()
        .map(|row| serde_json::from_value::<EventEnvelope>(row.payload))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(roots.len(), 2);
    for id in products {
        let product = Product::find_by_id(id)
            .one(target)
            .await?
            .expect("updated Product");
        assert_eq!(product.revision, 2);
        assert_eq!(product.metadata, serde_json::json!({"badge": "gold"}));
    }
    let sink = Arc::new(MockEventTransport::new());
    let relay = OutboxRelay::new(target.clone(), sink.clone());
    assert_eq!(relay.process_pending_once(None).await?, 4);
    assert_eq!(relay.process_pending_once(None).await?, 0);
    let delivered_roots = sink.events_of_type("ProductUpdated");
    let refreshes = sink.recorded_contract_events();
    assert_eq!(delivered_roots.len(), 2);
    assert_eq!(refreshes.len(), 2);
    for product_id in products {
        let root = roots
            .iter()
            .find(|root| root.event == (DomainEvent::ProductUpdated { product_id }))
            .expect("dispatched root event");
        assert!(delivered_roots.contains(&root.event));
        assert_eq!(root.tenant_id, tenant);
        let refresh = refreshes
            .iter()
            .find(|refresh| refresh.causation_id() == Some(root.id))
            .expect("dispatched refresh with exact root causation");
        assert_eq!(refresh.tenant_id(), tenant);
        assert!(
            matches!(refresh.payload()?, ContractEventPayload::ProductIndexRefresh(
            ProductIndexRefreshEvent::LocaleRefreshRequested { product_id: id, locale, .. }
        ) if *id == product_id && locale == "en")
        );
    }
    assert!(
        SysEvents::find()
            .all(target)
            .await?
            .iter()
            .all(|event| event.status == SysEventStatus::Dispatched)
    );
    Ok(())
}

async fn wait_for_revision(
    child: &mut tokio::process::Child,
    target: &DatabaseConnection,
    tenant: Uuid,
    expected: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if let Some(status) = child.try_wait()? {
                return Err(format!(
                    "Product service exited before storing Flex revision {expected}: {status}"
                )
                .into());
            }
            let row = target
                .query_one_raw(sea_orm::Statement::from_sql_and_values(
                    target.get_database_backend(),
                    "SELECT revision FROM product_flex_schema_projections WHERE tenant_id = $1",
                    [tenant.into()],
                ))
                .await?;
            if let Some(row) = row {
                let revision = row.try_get::<i64>("", "revision")?;
                if revision == expected {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await?
}

#[tokio::test]
async fn sqlite_imports_versioned_flex_schemas_across_databases()
-> Result<(), Box<dyn std::error::Error>> {
    verify_projection(
        Database::connect("sqlite::memory:").await?,
        Database::connect("sqlite::memory:").await?,
        None,
    )
    .await
}

#[tokio::test]
#[ignore = "requires RUSTOK_MIGRATION_SMOKE_ADMIN_URL for disposable PostgreSQL database creation"]
async fn postgres_imports_versioned_flex_schemas_across_databases()
-> Result<(), Box<dyn std::error::Error>> {
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    rustok_test_utils::assert_postgres_url(&admin_url);
    let admin = connect_postgres(&admin_url).await?;
    let source_name = unique_postgres_database_name("flex_snapshot_source");
    let target_name = unique_postgres_database_name("product_snapshot_target");
    create_postgres_database(&admin, &source_name).await?;
    let result = async {
        create_postgres_database(&admin, &target_name).await?;
        let source = connect_postgres(&postgres_database_url(&admin_url, &source_name)).await?;
        let target_url = postgres_database_url(&admin_url, &target_name);
        let target = connect_postgres(&target_url).await?;
        verify_projection(source, target, Some(&target_url)).await
    }
    .await;
    let source_cleanup = drop_postgres_database_if_exists(&admin, &source_name).await;
    let target_cleanup = drop_postgres_database_if_exists(&admin, &target_name).await;
    result?;
    source_cleanup?;
    target_cleanup?;
    Ok(())
}
