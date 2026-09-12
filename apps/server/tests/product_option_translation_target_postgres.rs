#![cfg(feature = "mod-product")]

use std::{error::Error, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::ModuleRegistry;
use rustok_migrations::Migrator;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_product::{
    CatalogService, ProductModule,
    dto::{
        CreateProductInput, CreateVariantInput, ProductOptionInput, ProductOptionTranslationInput,
        ProductTranslationInput, UpdateProductInput,
    },
    entities::product::ProductStatus,
};
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        server_runtime_context::ServerRuntimeContext,
    },
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OwnerSlug, ReadTranslationResourceRequest, ResourceKind,
    TranslationFieldPatch, TranslationPatchRequest, TranslationResourceLifecycle,
    TranslationResourceSnapshot, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "product";
const RESOURCE_KIND: &str = "option";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn product_option_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_option_translation_evidence");
    let database_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let result = run_contract(&database_url).await;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    result
}

async fn run_contract(database_url: &str) -> TestResult<()> {
    let seed_connection = connect_postgres(database_url).await?;
    Migrator::up(&seed_connection, None).await?;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    seed_tenant(&seed_connection, tenant_id, "option-main").await?;
    seed_tenant(&seed_connection, other_tenant_id, "option-isolated").await?;

    let owner = owner_service(seed_connection.clone());
    let actor_id = Uuid::new_v4();
    let product = owner
        .create_product(tenant_id, actor_id, initial_product())
        .await?;
    let product_id = product.id;
    assert_eq!(product.options.len(), 1);

    let provider = registered_provider(seed_connection.clone());
    let list_request = ListTranslationResourcesRequest {
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
        cursor: None,
        limit: 10,
    };
    let listed = provider
        .list_resources(read_context(tenant_id, "list"), list_request.clone())
        .await?;
    assert_eq!(listed.resources.len(), 1);
    assert!(listed.next_cursor.is_none());
    let identity = listed.resources[0].identity.clone();
    assert_eq!(identity.owner_slug.as_str(), OWNER_SLUG);
    assert_eq!(identity.resource_kind.as_str(), RESOURCE_KIND);
    assert_eq!(identity.resource_id.as_str(), product.options[0].id.to_string());
    assert!(identity.subresource_id.is_none());
    assert!(
        provider
            .list_resources(read_context(other_tenant_id, "isolation"), list_request)
            .await?
            .resources
            .is_empty()
    );

    let owner_change = provider
        .read_changes(
            read_context(tenant_id, "owner-create"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    assert_eq!(owner_change.changes.len(), 1);
    assert_eq!(owner_change.changes[0].identity, identity);
    assert_eq!(
        owner_change.changes[0].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let owner_cursor = owner_change.next_cursor.expect("owner cursor");

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let initial = provider
        .read_resource(read_context(tenant_id, "initial"), read_request.clone())
        .await?;
    assert_eq!(initial.summary.lifecycle, TranslationResourceLifecycle::Active);
    assert_eq!(field(&initial, "title").source_value, "Size");
    assert_eq!(initial.fields.len(), 3);
    assert!(
        initial
            .fields
            .iter()
            .all(|field| field.exact_target_value.is_none())
    );
    assert_eq!(
        owner_change.changes[0].resource_revision,
        initial.summary.resource_revision
    );

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone());
    let second_provider = registered_provider(second_connection.clone());

    let first_patch = patch(&initial, "Taille", "FR", "first");
    let first_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, "first-apply", "option-first-apply"),
            first_patch.clone(),
        )
        .await?;
    let replay = second_provider
        .apply_patch(
            apply_context(tenant_id, "first-replay", "option-first-apply"),
            first_patch,
        )
        .await?;
    assert_eq!(replay.provider_receipt_id, first_receipt.provider_receipt_id);
    assert_eq!(replay.resource_revision, first_receipt.resource_revision);
    assert_eq!(replay.target_revision, first_receipt.target_revision);

    let first_snapshot = first_provider
        .read_resource(read_context(tenant_id, "replica-one"), read_request.clone())
        .await?;
    let second_snapshot = second_provider
        .read_resource(read_context(tenant_id, "replica-two"), read_request.clone())
        .await?;
    assert_eq!(
        first_snapshot.summary.resource_revision,
        second_snapshot.summary.resource_revision
    );
    let first_apply = first_provider.apply_patch(
        apply_context(tenant_id, "race-one", "option-race-one"),
        patch(&first_snapshot, "Taille A", "A", "race-one"),
    );
    let second_apply = second_provider.apply_patch(
        apply_context(tenant_id, "race-two", "option-race-two"),
        patch(&second_snapshot, "Taille B", "B", "race-two"),
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    let winner = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            assert_eq!(error.kind, PortErrorKind::Conflict);
            receipt
        }
        other => panic!("expected exactly one concurrent CAS winner: {other:?}"),
    };

    let progress = provider
        .read_progress(
            read_context(tenant_id, "progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(progress.resources, 1);
    assert_eq!(progress.complete_resources, 1);
    assert_eq!(progress.required_units, initial.fields.len() as u64);
    assert_eq!(progress.exact_required_units, progress.required_units);
    assert_eq!(progress.optional_units, 0);
    assert_eq!(progress.exact_optional_units, 0);

    let frozen_first = provider
        .read_changes(
            read_context(tenant_id, "frozen-first"),
            TranslationTargetChangesRequest {
                after: Some(owner_cursor),
                limit: 1,
            },
        )
        .await?;
    assert_eq!(frozen_first.changes.len(), 1);
    assert_eq!(
        frozen_first.changes[0].resource_revision,
        first_receipt.resource_revision
    );
    let frozen_cursor = frozen_first.next_cursor.expect("frozen cursor");

    let before_noop = provider
        .read_resource(read_context(tenant_id, "before-noop"), read_request.clone())
        .await?;
    let progress_before_noop = provider
        .read_progress(
            read_context(tenant_id, "progress-before-noop"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    owner
        .update_product(
            tenant_id,
            actor_id,
            product_id,
            UpdateProductInput {
                vendor: Some("Vendor outside Option translation revision".into()),
                ..Default::default()
            },
        )
        .await?;
    let progress_after_noop = provider
        .read_progress(
            read_context(tenant_id, "progress-after-noop"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(
        progress_before_noop.owner_change_cursor,
        progress_after_noop.owner_change_cursor
    );
    let after_noop = provider
        .read_resource(read_context(tenant_id, "after-noop"), read_request.clone())
        .await?;
    assert_eq!(
        before_noop.summary.resource_revision,
        after_noop.summary.resource_revision
    );

    owner
        .update_product(
            tenant_id,
            actor_id,
            product_id,
            UpdateProductInput {
                status: Some(ProductStatus::Archived),
                ..Default::default()
            },
        )
        .await?;
    let archived = provider
        .read_resource(read_context(tenant_id, "archived"), read_request.clone())
        .await?;
    assert_eq!(archived.summary.lifecycle, TranslationResourceLifecycle::Archived);
    assert_ne!(
        after_noop.summary.resource_revision,
        archived.summary.resource_revision
    );
    let archived_revision = archived.summary.resource_revision.clone();

    owner
        .update_product(
            tenant_id,
            actor_id,
            product_id,
            UpdateProductInput {
                status: Some(ProductStatus::Draft),
                ..Default::default()
            },
        )
        .await?;
    let restored = provider
        .read_resource(read_context(tenant_id, "restored"), read_request.clone())
        .await?;
    assert_eq!(restored.summary.lifecycle, TranslationResourceLifecycle::Active);
    assert_ne!(archived.summary.resource_revision, restored.summary.resource_revision);
    let restored_revision = restored.summary.resource_revision.clone();

    let late_receipt = provider
        .apply_patch(
            apply_context(tenant_id, "late-apply", "option-late-apply"),
            patch(&restored, "Taille finale", "FINAL", "late"),
        )
        .await?;

    let frozen_second = provider
        .read_changes(
            read_context(tenant_id, "frozen-second"),
            TranslationTargetChangesRequest {
                after: Some(frozen_cursor),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(frozen_second.changes.len(), 1);
    assert_eq!(
        frozen_second.changes[0].resource_revision,
        winner.resource_revision
    );
    let frozen_terminal = frozen_second.next_cursor.expect("frozen terminal cursor");

    let next_window = provider
        .read_changes(
            read_context(tenant_id, "next-window"),
            TranslationTargetChangesRequest {
                after: Some(frozen_terminal),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(next_window.changes.len(), 3);
    assert_eq!(next_window.changes[0].resource_revision, archived_revision);
    assert_eq!(
        next_window.changes[0].lifecycle,
        TranslationResourceLifecycle::Archived
    );
    assert_eq!(next_window.changes[1].resource_revision, restored_revision);
    assert_eq!(
        next_window.changes[1].lifecycle,
        TranslationResourceLifecycle::Active
    );
    assert_eq!(
        next_window.changes[2].resource_revision,
        late_receipt.resource_revision
    );
    assert_eq!(
        next_window.changes[2].lifecycle,
        TranslationResourceLifecycle::Active
    );
    let pre_delete_cursor = next_window.next_cursor.expect("pre-delete cursor");

    let revision_before_delete = provider
        .read_resource(read_context(tenant_id, "pre-delete"), read_request.clone())
        .await?
        .summary
        .resource_revision;
    assert_eq!(revision_before_delete, late_receipt.resource_revision);

    owner.delete_product(tenant_id, actor_id, product_id).await?;
    let deleted = provider
        .read_changes(
            read_context(tenant_id, "deleted-change"),
            TranslationTargetChangesRequest {
                after: Some(pre_delete_cursor),
                limit: 10,
            },
        )
        .await?;
    assert_eq!(deleted.changes.len(), 1);
    assert_eq!(deleted.changes[0].identity, identity);
    assert_eq!(
        deleted.changes[0].lifecycle,
        TranslationResourceLifecycle::Deleted
    );
    assert!(
        deleted.changes[0]
            .resource_revision
            .as_str()
            .starts_with("option-deleted-v1:")
    );
    assert_ne!(deleted.changes[0].resource_revision, revision_before_delete);
    let deleted_cursor = deleted.next_cursor.expect("deleted cursor");

    let read_error = provider
        .read_resource(read_context(tenant_id, "deleted-read"), read_request)
        .await
        .expect_err("deleted Product Option must not remain readable");
    assert_eq!(read_error.kind, PortErrorKind::NotFound);
    let deleted_progress = provider
        .read_progress(
            read_context(tenant_id, "deleted-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_eq!(deleted_progress.resources, 0);
    assert_eq!(deleted_progress.complete_resources, 0);
    assert_eq!(deleted_progress.required_units, 0);
    assert_eq!(deleted_progress.exact_required_units, 0);
    assert_eq!(
        deleted_progress.owner_change_cursor.as_ref(),
        Some(&deleted_cursor)
    );

    drop(provider);
    drop(first_provider);
    drop(second_provider);
    first_connection.close().await?;
    second_connection.close().await?;
    seed_connection.close().await?;
    Ok(())
}

fn owner_service(db: DatabaseConnection) -> CatalogService {
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    CatalogService::new(db, event_bus)
}

fn registered_provider(db: DatabaseConnection) -> Arc<dyn TranslationTargetProvider> {
    let registry = ModuleRegistry::new().register(ProductModule);
    let settings = RustokSettings::default();
    let runtime = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime,
        AuthConfig::new("test-secret-key-for-option-translation-32bytes!".to_string()),
    )
    .expect("host composition must succeed");
    let targets = translation_target_registry(&extensions).expect("Translation registry");
    targets
        .get(
            &OwnerSlug::new(OWNER_SLUG).expect("owner slug"),
            &ResourceKind::new(RESOURCE_KIND).expect("resource kind"),
        )
        .expect("product/option provider")
}

fn initial_product() -> CreateProductInput {
    CreateProductInput {
        translations: vec![ProductTranslationInput {
            locale: "en".into(),
            title: "Option Evidence Product".into(),
            handle: Some("option-evidence-product".into()),
            description: Some("Product carrying Option translation evidence".into()),
            meta_title: None,
            meta_description: None,
        }],
        options: vec![ProductOptionInput {
            translations: vec![ProductOptionTranslationInput {
                locale: "en".into(),
                name: "Size".into(),
                values: vec!["Small".into(), "Large".into()],
            }],
        }],
        variants: vec![CreateVariantInput {
            sku: Some("OPTION-EVIDENCE-SKU".into()),
            barcode: None,
            shipping_profile_slug: None,
            option1: Some("Small".into()),
            option2: None,
            option3: None,
            prices: Vec::new(),
            inventory_quantity: 0,
            inventory_policy: "deny".into(),
            weight: None,
            weight_unit: None,
        }],
        seller_id: None,
        vendor: Some("Evidence Vendor".into()),
        product_type: Some("option-evidence".into()),
        shipping_profile_slug: None,
        primary_category_id: None,
        tags: Vec::new(),
        metadata: json!({"evidence": "option-translation"}),
        publish: false,
    }
}

fn patch(
    snapshot: &TranslationResourceSnapshot,
    title: &str,
    value_prefix: &str,
    suffix: &str,
) -> TranslationPatchRequest {
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: snapshot
            .fields
            .iter()
            .map(|field| TranslationFieldPatch {
                key: field.descriptor.key.clone(),
                value: if field.descriptor.key.as_str() == "title" {
                    title.to_string()
                } else if field.descriptor.key.as_str().starts_with("value:") {
                    format!("{value_prefix} {}", field.source_value)
                } else {
                    panic!(
                        "unexpected Product Option field: {}",
                        field.descriptor.key.as_str()
                    )
                },
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("option-proposal-{suffix}"),
        approval_receipt_id: format!("option-approval-{suffix}"),
    }
}

fn field<'a>(
    snapshot: &'a TranslationResourceSnapshot,
    key: &str,
) -> &'a rustok_translation_targets::TranslationFieldSnapshot {
    snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == key)
        .expect("translation field")
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("option-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(key)
}

async fn seed_tenant(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    slug_prefix: &str,
) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Product Option translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}
