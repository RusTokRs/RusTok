#![cfg(feature = "mod-translation")]

use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_api::{Permission, PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_auth::{
    AuthAdminMutationContext, AuthModule, CreateOAuthAppCommand, OAuthAdminPort,
    OAuthAppMutationRecord, OAuthAppSecretResult, UpdateOAuthAppCommand,
};
use rustok_core::{ModuleRegistry, UserRole};
use rustok_migrations::Migrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        auth_admin_mutation_provider::ServerAuthAdminMutationProvider,
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        rbac_request_scope::{RbacRequestScope, with_rbac_request_scope},
        server_runtime_context::ServerRuntimeContext,
    },
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OwnerSlug, ReadTranslationResourceRequest, ResourceKind,
    TranslationDataClassification, TranslationFieldPatch, TranslationPatchRequest,
    TranslationResourceLifecycle, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "auth";
const RESOURCE_KIND: &str = "oauth_application_copy";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn oauth_application_registered_translation_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_oauth_translation_evidence");
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
    let other_actor_id = Uuid::new_v4();
    seed_tenant(&seed_connection, tenant_id, "oauth-translation").await?;
    seed_tenant(&seed_connection, other_tenant_id, "oauth-isolation").await?;
    seed_tenant(&seed_connection, backfill_tenant_id, "oauth-backfill").await?;

    verify_translation_journal_backfill(&seed_connection, backfill_tenant_id).await?;

    let owner = ServerAuthAdminMutationProvider::new(seed_connection.clone());
    let created = create_app(
        &owner,
        tenant_id,
        actor_id,
        "en",
        "OAuth Source",
        "Primary source description",
        "oauth-primary",
    )
    .await?;
    let app_id = created.app.id;

    let other_created = create_app(
        &owner,
        other_tenant_id,
        other_actor_id,
        "en",
        "Other Tenant OAuth",
        "Other tenant description",
        "oauth-other",
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
            "registered OAuth provider returned unexpected inventory page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != app_id.to_string()
        || identity.subresource_id.is_some()
    {
        return Err(test_error(format!(
            "registered OAuth provider returned unexpected identity: {identity:?}"
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
        || other_listed.resources[0].identity.resource_id.as_str()
            != other_created.app.id.to_string()
        || other_listed.resources[0].identity == identity
    {
        return Err(test_error(format!(
            "OAuth Translation inventory leaked across tenants: primary={listed:?} other={other_listed:?}"
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
            "OAuth create did not retain one active Translation change: {source_changes:?}"
        ))
        .into());
    }
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("source OAuth change cursor is missing"))?;

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
    if initial_snapshot.fields.len() != 2
        || initial_snapshot.fields[0].descriptor.key.as_str() != "name"
        || initial_snapshot.fields[0].source_value != "OAuth Source"
        || !initial_snapshot.fields[0].descriptor.required
        || initial_snapshot.fields[0].descriptor.ai_export_allowed
        || initial_snapshot.fields[0].descriptor.classification
            != TranslationDataClassification::Sensitive
        || initial_snapshot.fields[1].descriptor.key.as_str() != "description"
        || initial_snapshot.fields[1].source_value != "Primary source description"
        || initial_snapshot.fields[1].descriptor.required
        || initial_snapshot.fields[1].descriptor.ai_export_allowed
        || initial_snapshot.fields[1].descriptor.classification
            != TranslationDataClassification::Sensitive
        || initial_snapshot
            .fields
            .iter()
            .any(|field| field.exact_target_value.is_some())
        || initial_snapshot.summary.lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "initial OAuth exact-locale snapshot is invalid: {initial_snapshot:?}"
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
        || progress_initial.optional_units != 1
        || progress_initial.exact_optional_units != 0
    {
        return Err(test_error(format!(
            "initial OAuth progress is invalid: {progress_initial:?}"
        ))
        .into());
    }

    rotate_secret(&owner, tenant_id, actor_id, app_id, "en").await?;
    let after_secret_rotation = seed_provider
        .read_resource(
            read_context(tenant_id, "after-secret-rotation"),
            read_request.clone(),
        )
        .await?;
    let progress_after_secret_rotation = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-after-secret-rotation"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if after_secret_rotation.summary.resource_revision
        != initial_snapshot.summary.resource_revision
        || after_secret_rotation.source_revision != initial_snapshot.source_revision
        || progress_after_secret_rotation.owner_change_cursor != progress_initial.owner_change_cursor
    {
        return Err(test_error(format!(
            "secret-only OAuth mutation changed Translation copy evidence: before={initial_snapshot:?} after={after_secret_rotation:?} progress_before={progress_initial:?} progress_after={progress_after_secret_rotation:?}"
        ))
        .into());
    }

    update_app(
        &owner,
        tenant_id,
        actor_id,
        app_id,
        "en",
        "OAuth Source v2",
        "Primary source description v2",
    )
    .await?;
    let source_updated = seed_provider
        .read_resource(
            read_context(tenant_id, "source-updated"),
            read_request.clone(),
        )
        .await?;
    if source_updated.fields[0].source_value != "OAuth Source v2"
        || source_updated.fields[1].source_value != "Primary source description v2"
        || source_updated.summary.resource_revision == initial_snapshot.summary.resource_revision
        || source_updated.source_revision == initial_snapshot.source_revision
    {
        return Err(test_error(format!(
            "canonical OAuth display-copy update did not rotate Translation revisions: {source_updated:?}"
        ))
        .into());
    }

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;

    let initial_patch = patch_from_snapshot(
        &source_updated,
        "Application OAuth FR",
        "Description OAuth FR",
        "initial",
    );
    let initial_receipt = first_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "initial-apply",
                "oauth-translation-initial-apply",
            ),
            initial_patch.clone(),
        )
        .await?;
    let replay_receipt = second_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "initial-replay",
                "oauth-translation-initial-apply",
            ),
            initial_patch,
        )
        .await?;
    if replay_receipt.provider_receipt_id != initial_receipt.provider_receipt_id
        || replay_receipt.resource_revision != initial_receipt.resource_revision
        || replay_receipt.target_revision != initial_receipt.target_revision
    {
        return Err(test_error(format!(
            "same-key OAuth replay did not return the stable owner receipt: initial={initial_receipt:?} replay={replay_receipt:?}"
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
            "independent OAuth replicas did not observe the same exact revisions",
        )
        .into());
    }

    let first_patch = patch_from_snapshot(
        &first_snapshot,
        "Application OAuth A",
        "Description OAuth A",
        "replica-one",
    );
    let second_patch = patch_from_snapshot(
        &second_snapshot,
        "Application OAuth B",
        "Description OAuth B",
        "replica-two",
    );
    let first_apply = first_provider.apply_patch(
        apply_context(
            tenant_id,
            "replica-one-apply",
            "oauth-translation-replica-one",
        ),
        first_patch,
    );
    let second_apply = second_provider.apply_patch(
        apply_context(
            tenant_id,
            "replica-two-apply",
            "oauth-translation-replica-two",
        ),
        second_patch,
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    let winner_receipt = match (first_result, second_result) {
        (Ok(receipt), Err(error)) | (Err(error), Ok(receipt)) => {
            if error.kind != PortErrorKind::Conflict {
                return Err(test_error(format!(
                    "losing OAuth replica must return Conflict, got {error:?}"
                ))
                .into());
            }
            receipt
        }
        (Ok(first), Ok(second)) => {
            return Err(test_error(format!(
                "concurrent OAuth replicas both applied one stale revision: {first:?} / {second:?}"
            ))
            .into());
        }
        (Err(first), Err(second)) => {
            return Err(test_error(format!(
                "both OAuth replicas failed concurrent CAS: {first:?} / {second:?}"
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
        || progress_complete.optional_units != 1
        || progress_complete.exact_optional_units != 1
    {
        return Err(test_error(format!(
            "OAuth aggregate progress did not converge after exact apply: {progress_complete:?}"
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
            "first frozen OAuth change page is invalid: {frozen_first:?}"
        ))
        .into());
    }
    let frozen_cursor = frozen_first
        .next_cursor
        .ok_or_else(|| test_error("first frozen OAuth cursor is missing"))?;

    update_app(
        &owner,
        tenant_id,
        actor_id,
        app_id,
        "en",
        "OAuth Source late",
        "Primary source description late",
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
            "late OAuth source mutation did not rotate the resource revision",
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
        || frozen_second
            .changes
            .iter()
            .any(|change| change.resource_revision == late_snapshot.summary.resource_revision)
        || frozen_second
            .changes
            .last()
            .map(|change| &change.resource_revision)
            != Some(&winner_receipt.resource_revision)
    {
        return Err(test_error(format!(
            "frozen OAuth window leaked a post-high-water change or lost pre-window evidence: {frozen_second:?}"
        ))
        .into());
    }
    let frozen_terminal = frozen_second
        .next_cursor
        .ok_or_else(|| test_error("frozen OAuth terminal cursor is missing"))?;

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
            "next OAuth ChangeCursor poll did not recover the post-high-water source mutation: {late_changes:?}"
        ))
        .into());
    }
    let late_terminal = late_changes
        .next_cursor
        .ok_or_else(|| test_error("late OAuth terminal cursor is missing"))?;

    let pre_revoke_revision = late_snapshot.summary.resource_revision.clone();
    let pre_revoke_source_revision = late_snapshot.source_revision.clone();
    revoke_app(&owner, tenant_id, actor_id, app_id, "en").await?;

    let archived_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "archived-read"),
            read_request,
        )
        .await?;
    if archived_snapshot.summary.lifecycle != TranslationResourceLifecycle::Archived
        || archived_snapshot.summary.resource_revision != pre_revoke_revision
        || archived_snapshot.source_revision != pre_revoke_source_revision
    {
        return Err(test_error(format!(
            "OAuth revoke did not archive the Translation resource without changing copy revisions: {archived_snapshot:?}"
        ))
        .into());
    }

    let archived_changes = seed_provider
        .read_changes(
            read_context(tenant_id, "archived-window"),
            TranslationTargetChangesRequest {
                after: Some(late_terminal),
                limit: 10,
            },
        )
        .await?;
    if archived_changes.changes.len() != 1
        || archived_changes.changes[0].identity != identity
        || archived_changes.changes[0].resource_revision != pre_revoke_revision
        || archived_changes.changes[0].lifecycle != TranslationResourceLifecycle::Archived
    {
        return Err(test_error(format!(
            "OAuth revoke did not retain one Archived ChangeCursor event: {archived_changes:?}"
        ))
        .into());
    }

    let listed_after_revoke = seed_provider
        .list_resources(
            read_context(tenant_id, "list-after-revoke"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    let progress_after_revoke = seed_provider
        .read_progress(
            read_context(tenant_id, "progress-after-revoke"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if !listed_after_revoke.resources.is_empty()
        || progress_after_revoke.resources != 0
        || progress_after_revoke.required_units != 0
        || progress_after_revoke.optional_units != 0
        || progress_after_revoke.complete_resources != 0
    {
        return Err(test_error(format!(
            "Archived OAuth application remained in active Translation inventory/progress: list={listed_after_revoke:?} progress={progress_after_revoke:?}"
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
    let mut migrations = rustok_auth::migrations::migrations();
    let migration = migrations
        .pop()
        .ok_or_else(|| test_error("OAuth application Translation migration is missing"))?;
    let manager = SchemaManager::new(database);
    migration.down(&manager).await?;

    let app_id = Uuid::new_v4();
    let client_id = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO oauth_apps (id, tenant_id, slug, app_type, client_id) VALUES ($1, $2, $3, $4, $5)",
            vec![
                app_id.into(),
                tenant_id.into(),
                format!("legacy-{}", app_id.simple()).into(),
                "third_party".into(),
                client_id.into(),
            ],
        ))
        .await?;
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description) VALUES ($1, $2, $3, $4, $5, $6)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                app_id.into(),
                "en".into(),
                "Legacy OAuth".into(),
                Some("Legacy OAuth description".to_string()).into(),
            ],
        ))
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
        || changes.changes[0].identity.resource_id.as_str() != app_id.to_string()
        || changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
        || changes.next_cursor.is_none()
    {
        return Err(test_error(format!(
            "OAuth Translation migration did not backfill existing localized resources: {changes:?}"
        ))
        .into());
    }
    Ok(())
}

fn registered_provider(db: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(AuthModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-oauth-translation-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register auth/oauth_application_copy").into()
    })
}

async fn create_app(
    owner: &ServerAuthAdminMutationProvider,
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    name: &str,
    description: &str,
    slug: &str,
) -> TestResult<OAuthAppSecretResult> {
    let context = owner_context(tenant_id, actor_id, locale, "create");
    let command = CreateOAuthAppCommand {
        name: name.to_string(),
        slug: format!("{slug}-{}", tenant_id.simple()),
        description: Some(description.to_string()),
        icon_url: None,
        app_type: "third_party".to_string(),
        redirect_uris: vec![format!("https://{}.example.test/callback", tenant_id.simple())],
        scopes: vec!["openid".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        granted_permissions: Vec::new(),
    };
    Ok(with_rbac_request_scope(
        Some(admin_scope(tenant_id, actor_id)),
        owner.create_oauth_app(&context, command),
    )
    .await?)
}

async fn update_app(
    owner: &ServerAuthAdminMutationProvider,
    tenant_id: Uuid,
    actor_id: Uuid,
    app_id: Uuid,
    locale: &str,
    name: &str,
    description: &str,
) -> TestResult<OAuthAppMutationRecord> {
    let context = owner_context(tenant_id, actor_id, locale, "update");
    let command = UpdateOAuthAppCommand {
        id: app_id,
        name: name.to_string(),
        description: Some(description.to_string()),
        icon_url: None,
        redirect_uris: vec![format!("https://{}.example.test/callback", tenant_id.simple())],
        scopes: vec!["openid".to_string()],
        grant_types: vec!["authorization_code".to_string()],
        granted_permissions: Vec::new(),
    };
    Ok(with_rbac_request_scope(
        Some(admin_scope(tenant_id, actor_id)),
        owner.update_oauth_app(&context, command),
    )
    .await?)
}

async fn rotate_secret(
    owner: &ServerAuthAdminMutationProvider,
    tenant_id: Uuid,
    actor_id: Uuid,
    app_id: Uuid,
    locale: &str,
) -> TestResult<OAuthAppSecretResult> {
    let context = owner_context(tenant_id, actor_id, locale, "rotate-secret");
    Ok(with_rbac_request_scope(
        Some(admin_scope(tenant_id, actor_id)),
        owner.rotate_oauth_app_secret(&context, app_id),
    )
    .await?)
}

async fn revoke_app(
    owner: &ServerAuthAdminMutationProvider,
    tenant_id: Uuid,
    actor_id: Uuid,
    app_id: Uuid,
    locale: &str,
) -> TestResult<OAuthAppMutationRecord> {
    let context = owner_context(tenant_id, actor_id, locale, "revoke");
    Ok(with_rbac_request_scope(
        Some(admin_scope(tenant_id, actor_id)),
        owner.revoke_oauth_app(&context, app_id),
    )
    .await?)
}

fn admin_scope(tenant_id: Uuid, actor_id: Uuid) -> RbacRequestScope {
    RbacRequestScope::new(
        tenant_id,
        actor_id,
        vec![Permission::SETTINGS_MANAGE],
        UserRole::Admin,
    )
}

fn owner_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    locale: &str,
    suffix: &str,
) -> AuthAdminMutationContext {
    AuthAdminMutationContext {
        actor_id,
        tenant_id,
        request_id: Some(format!("oauth-owner-postgres-{suffix}")),
        locale: Some(locale.to_string()),
    }
}

fn patch_from_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    name: &str,
    description: &str,
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
                    "description" => description.to_string(),
                    other => panic!("unexpected OAuth Translation field `{other}`"),
                },
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("oauth-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("oauth-postgres-approval-{suffix}"),
    }
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("oauth-postgres-{suffix}"),
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
                "OAuth Translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
