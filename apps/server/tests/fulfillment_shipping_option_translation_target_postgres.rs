#![cfg(feature = "mod-fulfillment")]

use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::ModuleRegistry;
use rustok_fulfillment::{
    CreateShippingOptionInput, FulfillmentModule, FulfillmentService, ShippingOptionTranslationInput,
    UpdateShippingOptionInput,
};
use rustok_migrations::Migrator;
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
    TranslationTargetChangesRequest, TranslationTargetProgressRequest, TranslationTargetProvider,
    translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "fulfillment";
const RESOURCE_KIND: &str = "shipping_option_copy";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn fulfillment_shipping_option_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_shipping_option_translation_evidence");
    let database_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await.map_err(|error| {
        test_error(format!(
            "PostgreSQL admin database must be reachable: {error}"
        ))
    })?;
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
    seed_tenant(&seed_connection, tenant_id, "shipping-option-translation").await?;
    seed_tenant(&seed_connection, other_tenant_id, "shipping-option-isolation").await?;

    let owner = FulfillmentService::new(seed_connection.clone());
    let option = owner
        .create_shipping_option(
            tenant_id,
            CreateShippingOptionInput {
                translations: vec![ShippingOptionTranslationInput {
                    locale: "en".to_string(),
                    name: "Express".to_string(),
                }],
                currency_code: "usd".to_string(),
                amount: 12.into(),
                provider_id: None,
                allowed_shipping_profile_slugs: None,
                metadata: json!({"source": "shipping-option-translation-evidence"}),
            },
        )
        .await?;

    let seed_provider = registered_provider(seed_connection.clone())?;
    let listed = seed_provider
        .list_resources(
            read_context(tenant_id, "seed-list"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    if listed.resources.len() != 1 || listed.next_cursor.is_some() {
        return Err(test_error(format!(
            "registered Fulfillment provider returned unexpected inventory page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != option.id.to_string()
        || identity.subresource_id.is_some()
    {
        return Err(test_error(format!(
            "registered Fulfillment provider returned unexpected identity: {identity:?}"
        ))
        .into());
    }

    let isolated = seed_provider
        .list_resources(
            read_context(other_tenant_id, "tenant-isolation"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    if !isolated.resources.is_empty() {
        return Err(test_error(format!(
            "Fulfillment Translation inventory leaked across tenants: {isolated:?}"
        ))
        .into());
    }

    let source_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "source-cursor"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    if source_changes.changes.len() != 1
        || source_changes.changes[0].identity != identity
        || source_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "Shipping Option create did not retain one active Translation change: {source_changes:?}"
        ))
        .into());
    }
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("source Shipping Option change cursor is missing"))?;

    let initial_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "initial-read"),
            ReadTranslationResourceRequest {
                identity: identity.clone(),
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if initial_snapshot.fields.len() != 1
        || initial_snapshot.fields[0].source_value != "Express"
        || initial_snapshot.fields[0].exact_target_value.is_some()
        || initial_snapshot.summary.lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "initial Shipping Option exact-locale snapshot is invalid: {initial_snapshot:?}"
        ))
        .into());
    }

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;

    let initial_patch = patch_from_snapshot(&initial_snapshot, "Express FR", "initial");
    let initial_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, "initial-apply", "shipping-option-initial-apply"),
            initial_patch.clone(),
        )
        .await?;
    let replay_receipt = second_provider
        .apply_patch(
            apply_context(tenant_id, "initial-replay", "shipping-option-initial-apply"),
            initial_patch,
        )
        .await?;
    if replay_receipt.provider_receipt_id != initial_receipt.provider_receipt_id
        || replay_receipt.resource_revision != initial_receipt.resource_revision
        || replay_receipt.target_revision != initial_receipt.target_revision
    {
        return Err(test_error(format!(
            "same-key Shipping Option replay did not return the stable owner receipt: initial={initial_receipt:?} replay={replay_receipt:?}"
        ))
        .into());
    }

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let first_snapshot = first_provider
        .read_resource(read_context(tenant_id, "replica-one-read"), read_request.clone())
        .await?;
    let second_snapshot = second_provider
        .read_resource(read_context(tenant_id, "replica-two-read"), read_request.clone())
        .await?;
    if first_snapshot.summary.resource_revision != second_snapshot.summary.resource_revision {
        return Err(test_error("independent Fulfillment replicas did not observe the same revision").into());
    }

    let first_patch = patch_from_snapshot(&first_snapshot, "Rapide A", "replica-one");
    let second_patch = patch_from_snapshot(&second_snapshot, "Rapide B", "replica-two");
    let first_apply = first_provider.apply_patch(
        apply_context(tenant_id, "replica-one-apply", "shipping-option-replica-one"),
        first_patch,
    );
    let second_apply = second_provider.apply_patch(
        apply_context(tenant_id, "replica-two-apply", "shipping-option-replica-two"),
        second_patch,
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    let winner_receipt = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            if error.kind != PortErrorKind::Conflict {
                return Err(test_error(format!(
                    "losing Fulfillment replica must return Conflict, got {error:?}"
                ))
                .into());
            }
            receipt
        }
        (Ok(first), Ok(second)) => {
            return Err(test_error(format!(
                "concurrent Fulfillment replicas both applied the same stale revision: {first:?} / {second:?}"
            ))
            .into());
        }
        (Err(first), Err(second)) => {
            return Err(test_error(format!(
                "both Fulfillment replicas failed concurrent CAS: {first:?} / {second:?}"
            ))
            .into());
        }
    };

    let progress_before_operational = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-before-operational"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if progress_before_operational.resources != 1
        || progress_before_operational.complete_resources != 1
        || progress_before_operational.required_units != 1
        || progress_before_operational.exact_required_units != 1
    {
        return Err(test_error(format!(
            "Fulfillment aggregate progress did not converge after exact apply: {progress_before_operational:?}"
        ))
        .into());
    }

    let frozen_first = seed_provider
        .read_changes(
            read_context(tenant_id, "frozen-first"),
            TranslationTargetChangesRequest {
                after: Some(source_cursor),
                limit: 1,
            },
        )
        .await?;
    if frozen_first.changes.len() != 1
        || frozen_first.changes[0].identity != identity
        || frozen_first.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "first frozen Shipping Option change page is invalid: {frozen_first:?}"
        ))
        .into());
    }
    let frozen_cursor = frozen_first
        .next_cursor
        .ok_or_else(|| test_error("first frozen Shipping Option cursor is missing"))?;

    let copy_before_operational = seed_provider
        .read_resource(read_context(tenant_id, "before-operational"), read_request.clone())
        .await?;
    owner
        .update_shipping_option(
            tenant_id,
            option.id,
            UpdateShippingOptionInput {
                translations: None,
                currency_code: Some("eur".to_string()),
                amount: Some(19.into()),
                provider_id: Some("manual".to_string()),
                allowed_shipping_profile_slugs: None,
                metadata: Some(json!({"operational": true})),
            },
        )
        .await?;
    let copy_after_operational = seed_provider
        .read_resource(read_context(tenant_id, "after-operational"), read_request.clone())
        .await?;
    let progress_after_operational = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-after-operational"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if copy_after_operational.summary.resource_revision
        != copy_before_operational.summary.resource_revision
        || progress_after_operational.owner_change_cursor
            != progress_before_operational.owner_change_cursor
    {
        return Err(test_error(format!(
            "operational Shipping Option fields changed Translation revision/cursor: before={copy_before_operational:?} after={copy_after_operational:?} progress_before={progress_before_operational:?} progress_after={progress_after_operational:?}"
        ))
        .into());
    }

    owner.deactivate_shipping_option(tenant_id, option.id).await?;

    let frozen_second = seed_provider
        .read_changes(
            read_context(tenant_id, "frozen-second"),
            TranslationTargetChangesRequest {
                after: Some(frozen_cursor),
                limit: 10,
            },
        )
        .await?;
    if frozen_second.changes.len() != 1
        || frozen_second.changes[0].identity != identity
        || frozen_second.changes[0].lifecycle != TranslationResourceLifecycle::Active
        || frozen_second.changes[0].resource_revision != winner_receipt.resource_revision
    {
        return Err(test_error(format!(
            "frozen Shipping Option window leaked later lifecycle state: {frozen_second:?}"
        ))
        .into());
    }
    let frozen_terminal = frozen_second
        .next_cursor
        .ok_or_else(|| test_error("frozen Shipping Option terminal cursor is missing"))?;

    let archived_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "archived-window"),
            TranslationTargetChangesRequest {
                after: Some(frozen_terminal),
                limit: 10,
            },
        )
        .await?;
    if archived_changes.changes.len() != 1
        || archived_changes.changes[0].identity != identity
        || archived_changes.changes[0].lifecycle != TranslationResourceLifecycle::Archived
        || archived_changes.changes[0].resource_revision != winner_receipt.resource_revision
    {
        return Err(test_error(format!(
            "Shipping Option deactivation did not retain Archived lifecycle with copy-only revision: {archived_changes:?}"
        ))
        .into());
    }

    let archived_snapshot = seed_provider
        .read_resource(read_context(tenant_id, "archived-read"), read_request)
        .await?;
    if archived_snapshot.summary.lifecycle != TranslationResourceLifecycle::Archived
        || archived_snapshot.summary.resource_revision != winner_receipt.resource_revision
    {
        return Err(test_error(format!(
            "archived Shipping Option snapshot changed copy revision or lifecycle: {archived_snapshot:?}"
        ))
        .into());
    }

    drop(seed_provider);
    drop(first_provider);
    drop(second_provider);
    first_connection.close().await?;
    second_connection.close().await?;
    seed_connection.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn registered_provider(db: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(FulfillmentModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-fulfillment-translation-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register fulfillment/shipping_option_copy").into()
    })
}

fn patch_from_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    value: &str,
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
                value: value.to_string(),
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("shipping-option-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("shipping-option-postgres-approval-{suffix}"),
    }
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("shipping-option-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, idempotency_key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(idempotency_key)
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
                "Fulfillment Shipping Option translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
