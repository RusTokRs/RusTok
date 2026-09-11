#![cfg(feature = "mod-flex")]

use std::{collections::HashMap, error::Error, io, sync::Arc, time::Duration};

use flex::{
    CreateFlexEntryCommand, CreateFlexSchemaCommand, FlexModule, FlexStandaloneService,
    UpdateFlexSchemaCommand,
};
use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{
    ModuleRegistry,
    field_schema::{FieldDefinition, FieldType},
};
use rustok_migrations::Migrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        flex_standalone_service::FlexStandaloneSeaOrmService,
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
const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "standalone_localized_value";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn flex_standalone_registered_translation_provider_multi_replica_evidence_postgres()
-> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_flex_standalone_translation_evidence");
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
    seed_tenant(&seed_connection, tenant_id).await?;
    let standalone = FlexStandaloneSeaOrmService::new(seed_connection.clone());
    let schema = standalone
        .create_schema(
            tenant_id,
            Some(Uuid::new_v4()),
            CreateFlexSchemaCommand {
                slug: "translation_evidence".to_string(),
                name: "Translation Evidence".to_string(),
                description: None,
                fields_config: vec![localized_definition(true)],
                settings: None,
                is_active: Some(true),
            },
        )
        .await?;
    let entry = standalone
        .create_entry(
            tenant_id,
            Some(Uuid::new_v4()),
            CreateFlexEntryCommand {
                schema_id: schema.id,
                entity_type: None,
                entity_id: None,
                data: json!({"tagline": "Launch entry"}),
                status: Some("published".to_string()),
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
            "registered Flex standalone provider returned unexpected inventory page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    let schema_id = schema.id.to_string();
    let entry_id = entry.id.to_string();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != entry_id.as_str()
        || identity.subresource_id.as_ref().map(|value| value.as_str())
            != Some(schema_id.as_str())
    {
        return Err(test_error(format!(
            "registered Flex standalone provider returned unexpected identity: {identity:?}"
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
            "initial standalone source write did not retain one active Translation change: {source_changes:?}"
        ))
        .into());
    }
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("source Flex standalone change cursor is missing"))?;
    drop(seed_provider);

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;
    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
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
    if first_snapshot != second_snapshot
        || first_snapshot.fields.len() != 1
        || first_snapshot.fields[0].source_value != "Launch entry"
        || first_snapshot.fields[0].exact_target_value.is_some()
        || first_snapshot.target_revision.is_some()
        || !first_snapshot.fields[0].descriptor.required
    {
        return Err(test_error(format!(
            "independent registered standalone providers did not read the same untranslated snapshot: first={first_snapshot:?}, second={second_snapshot:?}"
        ))
        .into());
    }

    let left_patch = patch_from_snapshot(&first_snapshot, "Entree de lancement A", "a");
    let right_patch = patch_from_snapshot(&second_snapshot, "Entree de lancement B", "b");
    let validation = first_provider
        .validate_patch(
            read_context(tenant_id, "replica-one-validate"),
            left_patch.clone(),
        )
        .await?;
    if !validation.accepted || !validation.issues.is_empty() {
        return Err(test_error(format!(
            "registered Flex standalone provider rejected a valid exact patch: {validation:?}"
        ))
        .into());
    }

    let left_key = "flex-standalone-postgres-apply-a";
    let right_key = "flex-standalone-postgres-apply-b";
    let left = first_provider.apply_patch(
        apply_context(tenant_id, "replica-one-apply", left_key),
        left_patch.clone(),
    );
    let right = second_provider.apply_patch(
        apply_context(tenant_id, "replica-two-apply", right_key),
        right_patch.clone(),
    );
    let (left, right) = tokio::join!(left, right);

    let (winner_patch, winner_key, receipt, loser) = match (left, right) {
        (Ok(receipt), Err(loser)) => (left_patch, left_key, receipt, loser),
        (Err(loser), Ok(receipt)) => (right_patch, right_key, receipt, loser),
        other => {
            return Err(test_error(format!(
                "exactly one concurrent Flex standalone translation apply must win: {other:?}"
            ))
            .into());
        }
    };
    if loser.kind != PortErrorKind::Conflict {
        return Err(test_error(format!(
            "concurrent Flex standalone loser returned unexpected error: {loser:?}"
        ))
        .into());
    }

    drop(first_provider);
    drop(second_provider);
    first_connection.close().await?;
    second_connection.close().await?;

    let observer_connection = connect_postgres(database_url).await?;
    let observer_provider = registered_provider(observer_connection.clone())?;
    let applied = observer_provider
        .read_resource(
            read_context(tenant_id, "observer-read"),
            read_request.clone(),
        )
        .await?;
    let expected_value = winner_patch
        .fields
        .first()
        .ok_or_else(|| test_error("winning Flex standalone patch has no field"))?
        .value
        .as_str()
        .to_string();
    if applied.fields.len() != 1
        || applied.fields[0].exact_target_value.as_deref() != Some(expected_value.as_str())
        || applied.summary.resource_revision != receipt.resource_revision
        || applied.target_revision.as_ref() != Some(&receipt.target_revision)
    {
        return Err(test_error(format!(
            "fresh registered standalone provider did not recover the winning exact target: {applied:?}"
        ))
        .into());
    }

    let target_changes = observer_provider
        .read_changes(
            read_context(tenant_id, "observer-cursor"),
            TranslationTargetChangesRequest {
                after: Some(source_cursor),
                limit: 10,
            },
        )
        .await?;
    if target_changes.changes.len() != 1
        || target_changes.changes[0].identity != identity
        || target_changes.changes[0].resource_revision != receipt.resource_revision
        || target_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "unexpected bounded Flex standalone target change page: {target_changes:?}"
        ))
        .into());
    }
    let target_cursor = target_changes
        .next_cursor
        .ok_or_else(|| test_error("target Flex standalone change cursor is missing"))?;
    drop(observer_provider);
    observer_connection.close().await?;

    let recovery_connection = connect_postgres(database_url).await?;
    let recovery_provider = registered_provider(recovery_connection.clone())?;
    let replay = recovery_provider
        .apply_patch(
            apply_context(tenant_id, "recovery-replay", winner_key),
            winner_patch,
        )
        .await?;
    if replay != receipt {
        return Err(test_error(
            "fresh registered standalone provider did not replay the exact owner receipt",
        )
        .into());
    }

    let resumed = recovery_provider
        .read_changes(
            read_context(tenant_id, "recovery-cursor"),
            TranslationTargetChangesRequest {
                after: Some(target_cursor.clone()),
                limit: 10,
            },
        )
        .await?;
    if !resumed.changes.is_empty() {
        return Err(test_error(format!(
            "Flex standalone ChangeCursor recovery redelivered committed work: {resumed:?}"
        ))
        .into());
    }

    let progress = recovery_provider
        .read_progress(
            read_context(tenant_id, "recovery-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if progress.resources != 1
        || progress.complete_resources != 1
        || progress.required_units != 1
        || progress.exact_required_units != 1
        || progress.optional_units != 0
        || progress.exact_optional_units != 0
        || progress.owner_change_cursor.as_ref() != Some(&target_cursor)
    {
        return Err(test_error(format!(
            "recovered Flex standalone aggregate progress is inconsistent: {progress:?}"
        ))
        .into());
    }

    standalone
        .update_schema(
            tenant_id,
            Some(Uuid::new_v4()),
            schema.id,
            UpdateFlexSchemaCommand {
                fields_config: Some(vec![localized_definition(false)]),
                ..Default::default()
            },
        )
        .await?;

    let schema_changes = recovery_provider
        .read_changes(
            read_context(tenant_id, "schema-fanout-cursor"),
            TranslationTargetChangesRequest {
                after: Some(target_cursor),
                limit: 10,
            },
        )
        .await?;
    if schema_changes.changes.len() != 1
        || schema_changes.changes[0].identity != identity
        || schema_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
        || schema_changes.changes[0].resource_revision == receipt.resource_revision
    {
        return Err(test_error(format!(
            "translation-relevant standalone schema update did not fan out one active resource change: {schema_changes:?}"
        ))
        .into());
    }
    let schema_revision = schema_changes.changes[0].resource_revision.clone();
    let schema_cursor = schema_changes
        .next_cursor
        .ok_or_else(|| test_error("schema fan-out Flex standalone change cursor is missing"))?;

    let schema_snapshot = recovery_provider
        .read_resource(
            read_context(tenant_id, "schema-fanout-read"),
            read_request.clone(),
        )
        .await?;
    if schema_snapshot.summary.resource_revision != schema_revision
        || schema_snapshot.fields.len() != 1
        || schema_snapshot.fields[0].descriptor.required
        || schema_snapshot.fields[0].exact_target_value.as_deref() != Some(expected_value.as_str())
    {
        return Err(test_error(format!(
            "schema fan-out did not preserve exact values under the new standalone aggregate revision: {schema_snapshot:?}"
        ))
        .into());
    }

    let schema_progress = recovery_provider
        .read_progress(
            read_context(tenant_id, "schema-fanout-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if schema_progress.resources != 1
        || schema_progress.complete_resources != 1
        || schema_progress.required_units != 0
        || schema_progress.exact_required_units != 0
        || schema_progress.optional_units != 1
        || schema_progress.exact_optional_units != 1
        || schema_progress.owner_change_cursor.as_ref() != Some(&schema_cursor)
    {
        return Err(test_error(format!(
            "schema fan-out did not recompute Flex standalone progress from the live definition: {schema_progress:?}"
        ))
        .into());
    }

    seed_connection
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE flex_entry_localized_values SET data = $1 WHERE tenant_id = $2 AND entry_id = $3 AND locale = $4",
            vec![
                json!({"tagline": "Launch entry revised"}).into(),
                tenant_id.into(),
                entry.id.into(),
                "en".into(),
            ],
        ))
        .await?;

    let direct_changes = recovery_provider
        .read_changes(
            read_context(tenant_id, "direct-owner-cursor"),
            TranslationTargetChangesRequest {
                after: Some(schema_cursor),
                limit: 10,
            },
        )
        .await?;
    if direct_changes.changes.len() != 1
        || direct_changes.changes[0].identity != identity
        || direct_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
        || direct_changes.changes[0].resource_revision == schema_revision
    {
        return Err(test_error(format!(
            "direct standalone localized owner write bypassed durable Translation change evidence: {direct_changes:?}"
        ))
        .into());
    }
    let direct_revision = direct_changes.changes[0].resource_revision.clone();
    let direct_cursor = direct_changes
        .next_cursor
        .ok_or_else(|| test_error("direct-write Flex standalone change cursor is missing"))?;

    let direct_snapshot = recovery_provider
        .read_resource(
            read_context(tenant_id, "direct-owner-read"),
            read_request.clone(),
        )
        .await?;
    if direct_snapshot.summary.resource_revision != direct_revision
        || direct_snapshot.fields.len() != 1
        || direct_snapshot.fields[0].source_value != "Launch entry revised"
        || direct_snapshot.fields[0].exact_target_value.as_deref() != Some(expected_value.as_str())
    {
        return Err(test_error(format!(
            "direct standalone owner write was not visible through the registered provider: {direct_snapshot:?}"
        ))
        .into());
    }

    standalone
        .delete_entry(
            tenant_id,
            Some(Uuid::new_v4()),
            schema.id,
            entry.id,
        )
        .await?;

    drop(recovery_provider);
    recovery_connection.close().await?;

    let deletion_connection = connect_postgres(database_url).await?;
    let deletion_provider = registered_provider(deletion_connection.clone())?;
    let deletion_changes = deletion_provider
        .read_changes(
            read_context(tenant_id, "deletion-cursor"),
            TranslationTargetChangesRequest {
                after: Some(direct_cursor),
                limit: 10,
            },
        )
        .await?;
    let expected_deleted_revision = format!("deleted:{}:{}", schema.id, entry.id);
    if deletion_changes.changes.len() != 1
        || deletion_changes.changes[0].identity != identity
        || deletion_changes.changes[0].lifecycle != TranslationResourceLifecycle::Deleted
        || deletion_changes.changes[0].resource_revision.as_str()
            != expected_deleted_revision.as_str()
    {
        return Err(test_error(format!(
            "standalone hard delete did not retain the final tombstone: {deletion_changes:?}"
        ))
        .into());
    }
    let deletion_cursor = deletion_changes
        .next_cursor
        .ok_or_else(|| test_error("deleted Flex standalone change cursor is missing"))?;

    let deleted_read = deletion_provider
        .read_resource(
            read_context(tenant_id, "deletion-read"),
            read_request.clone(),
        )
        .await
        .expect_err("hard-deleted standalone entry must not remain readable through Translation");
    if deleted_read.kind != PortErrorKind::NotFound {
        return Err(test_error(format!(
            "hard-deleted Flex standalone resource returned unexpected read error: {deleted_read:?}"
        ))
        .into());
    }

    let deleted_list = deletion_provider
        .list_resources(
            read_context(tenant_id, "deletion-list"),
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
            "hard-deleted standalone entry remained in Translation inventory: {deleted_list:?}"
        ))
        .into());
    }

    let deleted_progress = deletion_provider
        .read_progress(
            read_context(tenant_id, "deletion-progress"),
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
            "hard-deleted Flex standalone resource remained in aggregate progress: {deleted_progress:?}"
        ))
        .into());
    }

    drop(deletion_provider);
    deletion_connection.close().await?;
    seed_connection.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn registered_provider(db: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(FlexModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-flex-standalone-evidence-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register flex/standalone_localized_value").into()
    })
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Flex standalone translation evidence".into(),
                format!("flex-standalone-translation-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn localized_definition(required: bool) -> FieldDefinition {
    FieldDefinition {
        field_key: "tagline".to_string(),
        field_type: FieldType::Text,
        label: HashMap::from([("en".to_string(), "Tagline".to_string())]),
        description: None,
        is_localized: true,
        is_required: required,
        default_value: None,
        validation: None,
        position: 0,
        is_active: true,
    }
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
        proposal_id: format!("flex-standalone-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("flex-standalone-postgres-approval-{suffix}"),
    }
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("flex-standalone-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, idempotency_key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(idempotency_key)
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
