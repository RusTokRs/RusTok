#![cfg(feature = "mod-pricing")]

use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::ModuleRegistry;
use rustok_migrations::Migrator;
use rustok_pricing::{
    CreatePriceListOwnerInput, PriceListOwnerService, PriceListOwnerTranslationInput, PricingModule,
    UpdatePriceListOwnerInput,
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
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "pricing";
const RESOURCE_KIND: &str = "price_list_copy";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn pricing_price_list_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_price_list_translation_evidence");
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
    seed_tenant(&seed_connection, tenant_id, "pricing-price-list").await?;
    seed_tenant(&seed_connection, other_tenant_id, "pricing-price-list-isolation").await?;

    let owner = PriceListOwnerService::new(seed_connection.clone());
    let price_list = owner
        .create_price_list(
            tenant_id,
            CreatePriceListOwnerInput {
                translations: vec![PriceListOwnerTranslationInput {
                    locale: "en".to_string(),
                    name: "Autumn Sale".to_string(),
                    description: Some("Seasonal pricing".to_string()),
                }],
                list_type: "sale".to_string(),
                status: "active".to_string(),
                channel_id: None,
                channel_slug: None,
                starts_at: None,
                ends_at: None,
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
            "registered Pricing provider returned unexpected resource page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != price_list.id.to_string()
        || identity.subresource_id.is_some()
    {
        return Err(test_error(format!(
            "registered Pricing provider returned unexpected identity: {identity:?}"
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
            "Pricing Translation inventory leaked across tenants: {isolated:?}"
        ))
        .into());
    }

    let source_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "owner-create-cursor"),
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
            "Price List owner create did not retain one active Translation change: {source_changes:?}"
        ))
        .into());
    }
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("Price List owner-create cursor is missing"))?;

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let initial_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "initial-read"),
            read_request.clone(),
        )
        .await?;
    assert_snapshot_values(
        &initial_snapshot,
        "Autumn Sale",
        Some("Seasonal pricing"),
        None,
        None,
    )?;

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;

    let initial_patch = patch_from_snapshot(
        &initial_snapshot,
        "Vente d'automne",
        Some("Tarification saisonnière"),
        "initial",
    );
    let initial_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, "initial-apply", "price-list-initial-apply"),
            initial_patch.clone(),
        )
        .await?;
    let replay_receipt = second_provider
        .apply_patch(
            apply_context(tenant_id, "initial-replay", "price-list-initial-apply"),
            initial_patch,
        )
        .await?;
    if replay_receipt.provider_receipt_id != initial_receipt.provider_receipt_id
        || replay_receipt.resource_revision != initial_receipt.resource_revision
        || replay_receipt.target_revision != initial_receipt.target_revision
    {
        return Err(test_error(format!(
            "same-key Price List replay did not return the stable owner receipt: initial={initial_receipt:?} replay={replay_receipt:?}"
        ))
        .into());
    }

    let first_snapshot = first_provider
        .read_resource(
            read_context(tenant_id, "replica-one-read"),
            read_request.clone(),
        )
        .await?;
    let second_snapshot = second_provider
        .read_resource(
            read_context(tenant_id, "replica-two-read"),
            read_request.clone(),
        )
        .await?;
    if first_snapshot.summary.resource_revision != second_snapshot.summary.resource_revision {
        return Err(test_error("independent Pricing replicas observed different revisions").into());
    }

    let first_patch = patch_from_snapshot(&first_snapshot, "Vente A", Some("Description A"), "one");
    let second_patch =
        patch_from_snapshot(&second_snapshot, "Vente B", Some("Description B"), "two");
    let first_apply = first_provider.apply_patch(
        apply_context(tenant_id, "replica-one-apply", "price-list-replica-one"),
        first_patch,
    );
    let second_apply = second_provider.apply_patch(
        apply_context(tenant_id, "replica-two-apply", "price-list-replica-two"),
        second_patch,
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    let winner_receipt = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            if error.kind != PortErrorKind::Conflict {
                return Err(test_error(format!(
                    "losing Pricing replica must return Conflict, got {error:?}"
                ))
                .into());
            }
            receipt
        }
        (Ok(first), Ok(second)) => {
            return Err(test_error(format!(
                "concurrent Pricing replicas both applied one stale revision: {first:?} / {second:?}"
            ))
            .into());
        }
        (Err(first), Err(second)) => {
            return Err(test_error(format!(
                "both Pricing replicas failed concurrent CAS: {first:?} / {second:?}"
            ))
            .into());
        }
    };

    let progress_before_owner_updates = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-before-owner-updates"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if progress_before_owner_updates.resources != 1
        || progress_before_owner_updates.complete_resources != 1
        || progress_before_owner_updates.required_units != 1
        || progress_before_owner_updates.exact_required_units != 1
        || progress_before_owner_updates.optional_units != 1
        || progress_before_owner_updates.exact_optional_units != 1
    {
        return Err(test_error(format!(
            "Pricing aggregate progress did not converge after exact apply: {progress_before_owner_updates:?}"
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
            "first frozen Price List change page is invalid: {frozen_first:?}"
        ))
        .into());
    }
    let frozen_cursor = frozen_first
        .next_cursor
        .ok_or_else(|| test_error("first frozen Price List cursor is missing"))?;

    let copy_before_operational = seed_provider
        .read_resource(
            read_context(tenant_id, "before-operational"),
            read_request.clone(),
        )
        .await?;
    owner
        .update_price_list(
            tenant_id,
            price_list.id,
            UpdatePriceListOwnerInput {
                list_type: Some("promotion".to_string()),
                status: Some("draft".to_string()),
                ..UpdatePriceListOwnerInput::default()
            },
        )
        .await?;
    let copy_after_operational = seed_provider
        .read_resource(
            read_context(tenant_id, "after-operational"),
            read_request.clone(),
        )
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
            != progress_before_owner_updates.owner_change_cursor
    {
        return Err(test_error(format!(
            "operational Price List fields changed Translation revision/cursor: before={copy_before_operational:?} after={copy_after_operational:?} progress_before={progress_before_owner_updates:?} progress_after={progress_after_operational:?}"
        ))
        .into());
    }

    owner
        .update_price_list(
            tenant_id,
            price_list.id,
            UpdatePriceListOwnerInput {
                translations: Some(owner_translations_from_snapshot(&copy_after_operational)?),
                ..UpdatePriceListOwnerInput::default()
            },
        )
        .await?;
    let progress_after_semantic_noop = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-after-semantic-noop"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if progress_after_semantic_noop.owner_change_cursor
        != progress_before_owner_updates.owner_change_cursor
    {
        return Err(test_error(format!(
            "semantic no-op Price List owner update manufactured a cursor event: before={progress_before_owner_updates:?} after={progress_after_semantic_noop:?}"
        ))
        .into());
    }

    let late_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "late-read"),
            read_request.clone(),
        )
        .await?;
    let late_receipt = seed_provider
        .apply_patch(
            apply_context(tenant_id, "late-apply", "price-list-late-apply"),
            patch_from_snapshot(
                &late_snapshot,
                "Vente finale",
                Some("Description finale"),
                "late",
            ),
        )
        .await?;

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
            "frozen Price List cursor window leaked the later write: {frozen_second:?}"
        ))
        .into());
    }
    let frozen_terminal = frozen_second
        .next_cursor
        .ok_or_else(|| test_error("frozen Price List terminal cursor is missing"))?;

    let next_window = seed_provider
        .read_changes(
            read_context(tenant_id, "next-window"),
            TranslationTargetChangesRequest {
                after: Some(frozen_terminal),
                limit: 10,
            },
        )
        .await?;
    if next_window.changes.len() != 1
        || next_window.changes[0].identity != identity
        || next_window.changes[0].lifecycle != TranslationResourceLifecycle::Active
        || next_window.changes[0].resource_revision != late_receipt.resource_revision
    {
        return Err(test_error(format!(
            "next Price List cursor window did not expose the later write: {next_window:?}"
        ))
        .into());
    }
    let pre_delete_cursor = next_window
        .next_cursor
        .ok_or_else(|| test_error("pre-delete Price List cursor is missing"))?;

    owner.delete_price_list(tenant_id, price_list.id).await?;
    let deletion_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "delete-window"),
            TranslationTargetChangesRequest {
                after: Some(pre_delete_cursor),
                limit: 10,
            },
        )
        .await?;
    if deletion_changes.changes.len() != 1
        || deletion_changes.changes[0].identity != identity
        || deletion_changes.changes[0].lifecycle != TranslationResourceLifecycle::Deleted
        || !deletion_changes.changes[0]
            .resource_revision
            .as_str()
            .starts_with("deleted:")
    {
        return Err(test_error(format!(
            "Price List owner delete did not retain final tombstone: {deletion_changes:?}"
        ))
        .into());
    }
    let deletion_cursor = deletion_changes
        .next_cursor
        .ok_or_else(|| test_error("deleted Price List cursor is missing"))?;

    let deleted_read = seed_provider
        .read_resource(
            read_context(tenant_id, "deleted-read"),
            read_request.clone(),
        )
        .await
        .expect_err("deleted Price List must not remain readable through Translation");
    if deleted_read.kind != PortErrorKind::NotFound {
        return Err(test_error(format!(
            "deleted Price List returned unexpected read error: {deleted_read:?}"
        ))
        .into());
    }
    let deleted_list = seed_provider
        .list_resources(
            read_context(tenant_id, "deleted-list"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    if !deleted_list.resources.is_empty() {
        return Err(test_error(format!(
            "deleted Price List remained in Translation inventory: {deleted_list:?}"
        ))
        .into());
    }
    let deleted_progress = seed_provider
        .read_progress(
            read_context(tenant_id, "deleted-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if deleted_progress.resources != 0
        || deleted_progress.complete_resources != 0
        || deleted_progress.required_units != 0
        || deleted_progress.exact_required_units != 0
        || deleted_progress.optional_units != 0
        || deleted_progress.exact_optional_units != 0
        || deleted_progress.owner_change_cursor.as_ref() != Some(&deletion_cursor)
    {
        return Err(test_error(format!(
            "deleted Price List remained in aggregate progress: {deleted_progress:?}"
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
    let registry = ModuleRegistry::new().register(PricingModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-pricing-translation-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets
        .get(&owner_slug, &resource_kind)
        .ok_or_else(|| test_error("host composition did not register pricing/price_list_copy").into())
}

fn patch_from_snapshot(
    snapshot: &TranslationResourceSnapshot,
    name: &str,
    description: Option<&str>,
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
                value: match field.descriptor.key.as_str() {
                    "name" => name.to_string(),
                    "description" => description.unwrap_or_default().to_string(),
                    other => panic!("unexpected Pricing translation field: {other}"),
                },
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("price-list-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("price-list-postgres-approval-{suffix}"),
    }
}

fn owner_translations_from_snapshot(
    snapshot: &TranslationResourceSnapshot,
) -> TestResult<Vec<PriceListOwnerTranslationInput>> {
    let source_name = field(snapshot, "name")?.source_value.clone();
    let source_description = field(snapshot, "description")?.source_value.clone();
    let target_name = field(snapshot, "name")?
        .exact_target_value
        .clone()
        .ok_or_else(|| test_error("Price List target name is missing"))?;
    let target_description = field(snapshot, "description")?
        .exact_target_value
        .clone()
        .filter(|value| !value.is_empty());
    Ok(vec![
        PriceListOwnerTranslationInput {
            locale: snapshot.source_locale.as_str().to_string(),
            name: source_name,
            description: (!source_description.is_empty()).then_some(source_description),
        },
        PriceListOwnerTranslationInput {
            locale: snapshot.target_locale.as_str().to_string(),
            name: target_name,
            description: target_description,
        },
    ])
}

fn assert_snapshot_values(
    snapshot: &TranslationResourceSnapshot,
    source_name: &str,
    source_description: Option<&str>,
    target_name: Option<&str>,
    target_description: Option<&str>,
) -> TestResult<()> {
    let name = field(snapshot, "name")?;
    let description = field(snapshot, "description")?;
    if name.source_value != source_name
        || description.source_value != source_description.unwrap_or_default()
        || name.exact_target_value.as_deref() != target_name
        || description.exact_target_value.as_deref() != target_description
        || snapshot.summary.lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "Price List exact-locale snapshot is invalid: {snapshot:?}"
        ))
        .into());
    }
    Ok(())
}

fn field<'a>(
    snapshot: &'a TranslationResourceSnapshot,
    key: &str,
) -> TestResult<&'a rustok_translation_targets::TranslationFieldSnapshot> {
    snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == key)
        .ok_or_else(|| test_error(format!("Price List Translation field {key} is missing")).into())
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("price-list-postgres-{suffix}"),
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
                "Pricing Price List translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
