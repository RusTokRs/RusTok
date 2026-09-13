#![cfg(feature = "mod-marketplace_seller")]

use std::{error::Error, io, sync::Arc, time::Duration};

use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::ModuleRegistry;
use rustok_marketplace_seller::{
    CreateMarketplaceSellerInput, MarketplaceSellerCommandPort, MarketplaceSellerModule,
    MarketplaceSellerService, ReviewMarketplaceSellerOnboardingInput,
    ReviewMarketplaceSellerOnboardingRequest, SubmitMarketplaceSellerOnboardingInput,
    SubmitMarketplaceSellerOnboardingRequest, SuspendMarketplaceSellerInput,
    SuspendMarketplaceSellerRequest, UpdateMarketplaceSellerProfileInput,
    UpdateMarketplaceSellerProfileRequest,
};
use rustok_marketplace_seller::entities::{seller, seller_translation};
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
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "marketplace_seller";
const RESOURCE_KIND: &str = "seller_presentation";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn marketplace_seller_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_marketplace_seller_translation_evidence");
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
    let backfill_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&seed_connection, tenant_id, "seller-translation").await?;
    seed_tenant(&seed_connection, other_tenant_id, "seller-isolation").await?;
    seed_tenant(&seed_connection, backfill_tenant_id, "seller-backfill").await?;

    verify_translation_journal_backfill(&seed_connection, backfill_tenant_id).await?;

    let owner = MarketplaceSellerService::new(seed_connection.clone());
    let created = owner
        .create_seller(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "create",
                "seller-create-evidence",
            ),
            CreateMarketplaceSellerInput {
                handle: format!("seller-{}", Uuid::new_v4().simple()),
                display_name: "Seller Source".to_string(),
                legal_name: Some("Seller Legal LLC".to_string()),
                owner_user_id: actor_id,
                metadata: json!({"source": "seller-translation-evidence"}),
            },
        )
        .await?;
    let seller_id = created.id;

    let other_actor_id = Uuid::new_v4();
    let other_owner = MarketplaceSellerService::new(seed_connection.clone());
    let other_seller = other_owner
        .create_seller(
            owner_context(
                other_tenant_id,
                other_actor_id,
                "en",
                "other-create",
                "seller-other-create-evidence",
            ),
            CreateMarketplaceSellerInput {
                handle: format!("seller-{}", Uuid::new_v4().simple()),
                display_name: "Other Tenant Seller".to_string(),
                legal_name: Some("Other Tenant Legal LLC".to_string()),
                owner_user_id: other_actor_id,
                metadata: json!({"tenant": "other"}),
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
            "registered Marketplace Seller provider returned unexpected inventory page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != seller_id.to_string()
        || identity.subresource_id.is_some()
    {
        return Err(test_error(format!(
            "registered Marketplace Seller provider returned unexpected identity: {identity:?}"
        ))
        .into());
    }

    let other_listed = seed_provider
        .list_resources(
            read_context(other_tenant_id, "other-list"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    if other_listed.resources.len() != 1
        || other_listed.resources[0].identity.resource_id.as_str() != other_seller.id.to_string()
        || other_listed.resources[0].identity == identity
    {
        return Err(test_error(format!(
            "Marketplace Seller Translation inventory leaked across tenants: primary={listed:?} other={other_listed:?}"
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
            "Seller create did not retain one active Translation change: {source_changes:?}"
        ))
        .into());
    }
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("source Seller change cursor is missing"))?;

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
    if initial_snapshot.fields.len() != 1
        || initial_snapshot.fields[0].descriptor.key.as_str() != "display_name"
        || initial_snapshot.fields[0].source_value != "Seller Source"
        || initial_snapshot.fields[0].exact_target_value.is_some()
        || !initial_snapshot.fields[0].descriptor.ai_export_allowed
        || initial_snapshot.summary.lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "initial Marketplace Seller exact-locale snapshot is invalid: {initial_snapshot:?}"
        ))
        .into());
    }

    let progress_initial = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-initial"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if progress_initial.resources != 1
        || progress_initial.complete_resources != 0
        || progress_initial.required_units != 1
        || progress_initial.exact_required_units != 0
    {
        return Err(test_error(format!(
            "initial Marketplace Seller progress is invalid: {progress_initial:?}"
        ))
        .into());
    }

    owner
        .update_seller_profile(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "operational-update",
                "seller-operational-update-evidence",
            ),
            UpdateMarketplaceSellerProfileRequest {
                seller_id,
                input: UpdateMarketplaceSellerProfileInput {
                    display_name: None,
                    legal_name: Some("Seller Legal BV".to_string()),
                    metadata: Some(json!({"operational": true})),
                },
            },
        )
        .await?;
    owner
        .submit_seller_onboarding(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "onboarding-submit",
                "seller-onboarding-submit-evidence",
            ),
            SubmitMarketplaceSellerOnboardingRequest {
                seller_id,
                input: SubmitMarketplaceSellerOnboardingInput {
                    note: Some("review this seller".to_string()),
                },
            },
        )
        .await?;
    owner
        .review_seller_onboarding(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "onboarding-review",
                "seller-onboarding-review-evidence",
            ),
            ReviewMarketplaceSellerOnboardingRequest {
                seller_id,
                input: ReviewMarketplaceSellerOnboardingInput {
                    approved: true,
                    note: Some("approved".to_string()),
                },
            },
        )
        .await?;

    let after_operational = seed_provider
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
    if after_operational.summary.resource_revision != initial_snapshot.summary.resource_revision
        || after_operational.source_revision != initial_snapshot.source_revision
        || progress_after_operational.owner_change_cursor != progress_initial.owner_change_cursor
        || after_operational.summary.lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "legal/metadata/onboarding state changed Seller Translation copy evidence: before={initial_snapshot:?} after={after_operational:?} progress_before={progress_initial:?} progress_after={progress_after_operational:?}"
        ))
        .into());
    }

    owner
        .update_seller_profile(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "source-copy-update",
                "seller-source-copy-update-evidence",
            ),
            UpdateMarketplaceSellerProfileRequest {
                seller_id,
                input: UpdateMarketplaceSellerProfileInput {
                    display_name: Some("Seller Source v2".to_string()),
                    legal_name: None,
                    metadata: None,
                },
            },
        )
        .await?;
    let source_updated = seed_provider
        .read_resource(
            read_context(tenant_id, "source-updated"),
            read_request.clone(),
        )
        .await?;
    if source_updated.fields[0].source_value != "Seller Source v2"
        || source_updated.summary.resource_revision == initial_snapshot.summary.resource_revision
        || source_updated.source_revision == initial_snapshot.source_revision
    {
        return Err(test_error(format!(
            "canonical Seller display_name update did not rotate Translation revisions: {source_updated:?}"
        ))
        .into());
    }

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;

    let initial_patch = patch_from_snapshot(&source_updated, "Vendeur FR", "initial");
    let initial_receipt = first_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "initial-apply",
                "seller-translation-initial-apply",
            ),
            initial_patch.clone(),
        )
        .await?;
    let replay_receipt = second_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "initial-replay",
                "seller-translation-initial-apply",
            ),
            initial_patch,
        )
        .await?;
    if replay_receipt.provider_receipt_id != initial_receipt.provider_receipt_id
        || replay_receipt.resource_revision != initial_receipt.resource_revision
        || replay_receipt.target_revision != initial_receipt.target_revision
    {
        return Err(test_error(format!(
            "same-key Seller replay did not return the stable owner receipt: initial={initial_receipt:?} replay={replay_receipt:?}"
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
    if first_snapshot.summary.resource_revision != second_snapshot.summary.resource_revision
        || first_snapshot.target_revision != second_snapshot.target_revision
    {
        return Err(test_error(
            "independent Seller replicas did not observe the same exact revisions",
        )
        .into());
    }

    let first_patch = patch_from_snapshot(&first_snapshot, "Vendeur A", "replica-one");
    let second_patch = patch_from_snapshot(&second_snapshot, "Vendeur B", "replica-two");
    let first_apply = first_provider.apply_patch(
        apply_context(
            tenant_id,
            "replica-one-apply",
            "seller-translation-replica-one",
        ),
        first_patch,
    );
    let second_apply = second_provider.apply_patch(
        apply_context(
            tenant_id,
            "replica-two-apply",
            "seller-translation-replica-two",
        ),
        second_patch,
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    let winner_receipt = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            if error.kind != PortErrorKind::Conflict {
                return Err(test_error(format!(
                    "losing Marketplace Seller replica must return Conflict, got {error:?}"
                ))
                .into());
            }
            receipt
        }
        (Ok(first), Ok(second)) => {
            return Err(test_error(format!(
                "concurrent Marketplace Seller replicas both applied one stale revision: {first:?} / {second:?}"
            ))
            .into());
        }
        (Err(first), Err(second)) => {
            return Err(test_error(format!(
                "both Marketplace Seller replicas failed concurrent CAS: {first:?} / {second:?}"
            ))
            .into());
        }
    };

    let progress_complete = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-complete"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if progress_complete.resources != 1
        || progress_complete.complete_resources != 1
        || progress_complete.required_units != 1
        || progress_complete.exact_required_units != 1
    {
        return Err(test_error(format!(
            "Marketplace Seller aggregate progress did not converge after exact apply: {progress_complete:?}"
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
            "first frozen Seller change page is invalid: {frozen_first:?}"
        ))
        .into());
    }
    let frozen_cursor = frozen_first
        .next_cursor
        .ok_or_else(|| test_error("first frozen Seller cursor is missing"))?;

    owner
        .update_seller_profile(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "late-source-update",
                "seller-late-source-update-evidence",
            ),
            UpdateMarketplaceSellerProfileRequest {
                seller_id,
                input: UpdateMarketplaceSellerProfileInput {
                    display_name: Some("Seller Source late".to_string()),
                    legal_name: None,
                    metadata: None,
                },
            },
        )
        .await?;
    let late_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "late-read"),
            read_request.clone(),
        )
        .await?;
    if late_snapshot.summary.resource_revision == winner_receipt.resource_revision {
        return Err(test_error(
            "late Seller source mutation did not rotate the resource revision",
        )
        .into());
    }

    let frozen_second = seed_provider
        .read_changes(
            read_context(tenant_id, "frozen-second"),
            TranslationTargetChangesRequest {
                after: Some(frozen_cursor),
                limit: 10,
            },
        )
        .await?;
    if frozen_second.changes.len() != 2
        || frozen_second.changes.iter().any(|change| {
            change.resource_revision == late_snapshot.summary.resource_revision
        })
        || frozen_second.changes.last().map(|change| &change.resource_revision)
            != Some(&winner_receipt.resource_revision)
    {
        return Err(test_error(format!(
            "frozen Seller window leaked a post-high-water change or lost pre-window evidence: {frozen_second:?}"
        ))
        .into());
    }
    let frozen_terminal = frozen_second
        .next_cursor
        .ok_or_else(|| test_error("frozen Seller terminal cursor is missing"))?;

    let late_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "late-window"),
            TranslationTargetChangesRequest {
                after: Some(frozen_terminal),
                limit: 10,
            },
        )
        .await?;
    if late_changes.changes.len() != 1
        || late_changes.changes[0].identity != identity
        || late_changes.changes[0].resource_revision != late_snapshot.summary.resource_revision
        || late_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "next Seller ChangeCursor poll did not recover the post-high-water source mutation: {late_changes:?}"
        ))
        .into());
    }

    let progress_before_suspend = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-before-suspend"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    owner
        .suspend_seller(
            owner_context(
                tenant_id,
                actor_id,
                "en",
                "suspend",
                "seller-suspend-evidence",
            ),
            SuspendMarketplaceSellerRequest {
                seller_id,
                input: SuspendMarketplaceSellerInput {
                    reason: "operational review".to_string(),
                },
            },
        )
        .await?;
    let suspended_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "suspended-read"),
            read_request,
        )
        .await?;
    let progress_after_suspend = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-after-suspend"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if suspended_snapshot.summary.lifecycle != TranslationResourceLifecycle::Active
        || suspended_snapshot.summary.resource_revision != late_snapshot.summary.resource_revision
        || progress_after_suspend.owner_change_cursor != progress_before_suspend.owner_change_cursor
        || progress_after_suspend.complete_resources != 1
    {
        return Err(test_error(format!(
            "suspension-only Seller state changed Translation copy/lifecycle evidence: snapshot={suspended_snapshot:?} before={progress_before_suspend:?} after={progress_after_suspend:?}"
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

async fn verify_translation_journal_backfill(
    database: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<()> {
    let mut migrations = rustok_marketplace_seller::migrations::migrations();
    let migration = migrations
        .pop()
        .ok_or_else(|| test_error("Marketplace Seller Translation migration is missing"))?;
    let manager = SchemaManager::new(database);
    migration.down(&manager).await?;

    let seller_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    seller::ActiveModel {
        id: Set(seller_id),
        tenant_id: Set(tenant_id),
        handle: Set(format!("legacy-{}", seller_id.simple())),
        legal_name: Set(Some("Legacy Seller Legal".to_string())),
        status: Set("draft".to_string()),
        onboarding_status: Set("draft".to_string()),
        metadata: Set(json!({"legacy": true})),
        created_at: Set(now),
        updated_at: Set(now),
        activated_at: Set(None),
        suspended_at: Set(None),
    }
    .insert(database)
    .await?;
    seller_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        locale: Set("en".to_string()),
        display_name: Set("Legacy Seller".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await?;

    migration.up(&manager).await?;
    let provider = registered_provider(database.clone())?;
    let changes = provider
        .read_changes(
            read_context(tenant_id, "backfill-read"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    if changes.changes.len() != 1
        || changes.changes[0].identity.resource_id.as_str() != seller_id.to_string()
        || changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
        || changes.next_cursor.is_none()
    {
        return Err(test_error(format!(
            "Marketplace Seller Translation migration did not backfill existing localized resources: {changes:?}"
        ))
        .into());
    }
    Ok(())
}

fn registered_provider(db: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(MarketplaceSellerModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-seller-translation-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register marketplace_seller/seller_presentation")
            .into()
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
        proposal_id: format!("seller-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("seller-postgres-approval-{suffix}"),
    }
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("seller-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, idempotency_key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(idempotency_key)
}

fn owner_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    suffix: &str,
    idempotency_key: &str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        locale,
        format!("seller-owner-postgres-{suffix}"),
    )
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(30))
}

async fn seed_tenant(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    slug_prefix: &str,
) -> TestResult<()> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Marketplace Seller translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
