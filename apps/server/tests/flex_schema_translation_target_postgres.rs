#![cfg(feature = "mod-flex")]

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    io,
    sync::Arc,
    time::Duration,
};

use flex::{
    CreateFlexSchemaCommand, FlexModule, FlexStandaloneService, UpdateFlexSchemaCommand,
};
use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_core::{
    ModuleRegistry,
    field_schema::{FieldDefinition, FieldType, SelectOption, ValidationRule},
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
    TranslationDataClassification, TranslationFieldPatch, TranslationPatchRequest,
    TranslationResourceLifecycle, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, TranslationValueProfile, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "schema_copy";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn flex_schema_copy_registered_translation_provider_multi_replica_evidence_postgres()
-> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_flex_schema_copy_translation_evidence");
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
    let isolated_tenant_id = Uuid::new_v4();
    seed_tenant(&seed_connection, tenant_id, "flex-schema-copy").await?;
    seed_tenant(&seed_connection, isolated_tenant_id, "flex-schema-copy-isolated").await?;

    let standalone = FlexStandaloneSeaOrmService::new(seed_connection.clone());
    let schema = standalone
        .create_schema(
            tenant_id,
            Some(Uuid::new_v4()),
            CreateFlexSchemaCommand {
                slug: "schema_copy_evidence".to_string(),
                name: "Schema Evidence".to_string(),
                description: Some("Schema description".to_string()),
                fields_config: vec![schema_copy_definition()],
                settings: Some(json!({"operational": "initial"})),
                is_active: Some(true),
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
            "registered Flex schema-copy provider returned unexpected inventory page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != schema.id.to_string()
        || identity.subresource_id.is_some()
    {
        return Err(test_error(format!(
            "registered Flex schema-copy provider returned unexpected identity: {identity:?}"
        ))
        .into());
    }

    let isolated = seed_provider
        .list_resources(
            read_context(isolated_tenant_id, "isolated-list"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    if !isolated.resources.is_empty() || isolated.next_cursor.is_some() {
        return Err(test_error(format!(
            "Flex schema-copy inventory leaked across tenants: {isolated:?}"
        ))
        .into());
    }

    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let initial = seed_provider
        .read_resource(
            read_context(tenant_id, "seed-read"),
            read_request.clone(),
        )
        .await?;
    assert_schema_copy_shape(&initial)?;
    if initial.target_revision.is_some()
        || initial.fields.iter().any(|field| field.exact_target_value.is_some())
    {
        return Err(test_error(format!(
            "fresh Flex schema-copy resource unexpectedly had exact target state: {initial:?}"
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
        || source_changes.changes[0].resource_revision != initial.summary.resource_revision
        || source_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "canonical Flex schema create did not retain one exact active schema-copy change: {source_changes:?}"
        ))
        .into());
    }
    let source_cursor = source_changes
        .next_cursor
        .ok_or_else(|| test_error("source Flex schema-copy change cursor is missing"))?;

    let initial_progress = seed_provider
        .read_progress(
            read_context(tenant_id, "initial-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_progress(&initial_progress, 1, 0, 3, 0, 3, 0)?;
    if initial_progress.owner_change_cursor.as_ref() != Some(&source_cursor) {
        return Err(test_error(format!(
            "initial Flex schema-copy progress cursor diverged from owner journal: {initial_progress:?}"
        ))
        .into());
    }
    drop(seed_provider);

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;
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
    if first_snapshot != second_snapshot {
        return Err(test_error(
            "independent registered Flex schema-copy providers observed different owner snapshots",
        )
        .into());
    }

    let left_patch = patch_from_snapshot(&first_snapshot, "a", "A");
    let right_patch = patch_from_snapshot(&second_snapshot, "b", "B");
    let validation = first_provider
        .validate_patch(
            read_context(tenant_id, "replica-one-validate"),
            left_patch.clone(),
        )
        .await?;
    if !validation.accepted || !validation.issues.is_empty() {
        return Err(test_error(format!(
            "registered Flex schema-copy provider rejected a complete exact patch: {validation:?}"
        ))
        .into());
    }

    let left_key = "flex-schema-copy-postgres-apply-a";
    let right_key = "flex-schema-copy-postgres-apply-b";
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
                "exactly one concurrent Flex schema-copy Translation apply must win: {other:?}"
            ))
            .into());
        }
    };
    if loser.kind != PortErrorKind::Conflict {
        return Err(test_error(format!(
            "concurrent Flex schema-copy loser returned unexpected error: {loser:?}"
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
    assert_schema_copy_shape(&applied)?;
    assert_target_values(&applied, &winner_patch)?;
    if applied.summary.resource_revision != receipt.resource_revision
        || applied.target_revision.as_ref() != Some(&receipt.target_revision)
        || !applied
            .summary
            .exact_locales
            .iter()
            .any(|locale| locale.as_str() == "fr")
    {
        return Err(test_error(format!(
            "fresh registered Flex schema-copy provider did not recover the winning exact target: {applied:?}"
        ))
        .into());
    }

    let replay = observer_provider
        .apply_patch(
            apply_context(tenant_id, "observer-replay", winner_key),
            winner_patch.clone(),
        )
        .await?;
    if replay != receipt {
        return Err(test_error(
            "fresh registered Flex schema-copy provider did not replay the durable owner receipt",
        )
        .into());
    }

    let translated_progress = observer_provider
        .read_progress(
            read_context(tenant_id, "translated-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    assert_progress(&translated_progress, 1, 1, 3, 3, 3, 3)?;
    let translated_cursor = translated_progress
        .owner_change_cursor
        .clone()
        .ok_or_else(|| test_error("translated Flex schema-copy progress cursor is missing"))?;

    let before_settings = observer_provider
        .read_resource(
            read_context(tenant_id, "before-settings"),
            read_request.clone(),
        )
        .await?;
    standalone
        .update_schema(
            tenant_id,
            Some(Uuid::new_v4()),
            schema.id,
            UpdateFlexSchemaCommand {
                settings: Some(json!({"operational": "changed-outside-schema-copy"})),
                ..Default::default()
            },
        )
        .await?;
    let after_settings = observer_provider
        .read_resource(
            read_context(tenant_id, "after-settings"),
            read_request.clone(),
        )
        .await?;
    let settings_progress = observer_provider
        .read_progress(
            read_context(tenant_id, "settings-progress"),
            TranslationTargetProgressRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
            },
        )
        .await?;
    if before_settings.summary.resource_revision != after_settings.summary.resource_revision
        || after_settings.summary.resource_revision != receipt.resource_revision
        || settings_progress.owner_change_cursor.as_ref() != Some(&translated_cursor)
    {
        return Err(test_error(format!(
            "settings-only Flex schema update manufactured schema-copy revision/cursor churn: before={before_settings:?}, after={after_settings:?}, progress={settings_progress:?}"
        ))
        .into());
    }

    standalone
        .update_schema(
            tenant_id,
            Some(Uuid::new_v4()),
            schema.id,
            UpdateFlexSchemaCommand {
                is_active: Some(false),
                ..Default::default()
            },
        )
        .await?;
    let archived = observer_provider
        .read_resource(
            read_context(tenant_id, "archived-read"),
            read_request.clone(),
        )
        .await?;
    if archived.summary.lifecycle != TranslationResourceLifecycle::Archived
        || archived.summary.resource_revision == receipt.resource_revision
    {
        return Err(test_error(format!(
            "canonical Flex schema archive did not change lifecycle/revision: {archived:?}"
        ))
        .into());
    }
    let archived_revision = archived.summary.resource_revision.clone();

    // Open one bounded window after both the target apply and archive already exist. The
    // later restore must not leak into that frozen `(after, through]` window.
    let frozen_first = observer_provider
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
        || frozen_first.changes[0].resource_revision != receipt.resource_revision
        || frozen_first.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "first frozen Flex schema-copy page did not expose the winning apply: {frozen_first:?}"
        ))
        .into());
    }
    let frozen_cursor = frozen_first
        .next_cursor
        .ok_or_else(|| test_error("first frozen Flex schema-copy cursor is missing"))?;

    standalone
        .update_schema(
            tenant_id,
            Some(Uuid::new_v4()),
            schema.id,
            UpdateFlexSchemaCommand {
                is_active: Some(true),
                ..Default::default()
            },
        )
        .await?;
    let restored = observer_provider
        .read_resource(
            read_context(tenant_id, "restored-read"),
            read_request.clone(),
        )
        .await?;
    if restored.summary.lifecycle != TranslationResourceLifecycle::Active
        || restored.summary.resource_revision != receipt.resource_revision
    {
        return Err(test_error(format!(
            "canonical Flex schema restore did not return to the active copy revision: {restored:?}"
        ))
        .into());
    }

    let frozen_second = observer_provider
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
        || frozen_second.changes[0].resource_revision != archived_revision
        || frozen_second.changes[0].lifecycle != TranslationResourceLifecycle::Archived
    {
        return Err(test_error(format!(
            "second frozen Flex schema-copy page leaked or lost lifecycle work: {frozen_second:?}"
        ))
        .into());
    }
    let frozen_terminal = frozen_second
        .next_cursor
        .ok_or_else(|| test_error("terminal frozen Flex schema-copy cursor is missing"))?;

    let next_window = observer_provider
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
        || next_window.changes[0].resource_revision != receipt.resource_revision
        || next_window.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "next Flex schema-copy window did not expose the later restore exactly once: {next_window:?}"
        ))
        .into());
    }
    let restored_cursor = next_window
        .next_cursor
        .ok_or_else(|| test_error("restored Flex schema-copy cursor is missing"))?;

    standalone
        .delete_schema(tenant_id, Some(Uuid::new_v4()), schema.id)
        .await?;

    drop(observer_provider);
    observer_connection.close().await?;

    let deletion_connection = connect_postgres(database_url).await?;
    let deletion_provider = registered_provider(deletion_connection.clone())?;
    let deletion_changes = deletion_provider
        .read_changes(
            read_context(tenant_id, "deletion-cursor"),
            TranslationTargetChangesRequest {
                after: Some(restored_cursor),
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
            .starts_with("flex-schema-deleted-v1:")
    {
        return Err(test_error(format!(
            "canonical Flex schema delete did not retain its final schema-copy tombstone: {deletion_changes:?}"
        ))
        .into());
    }
    let deletion_cursor = deletion_changes
        .next_cursor
        .ok_or_else(|| test_error("deleted Flex schema-copy cursor is missing"))?;

    let deleted_read = deletion_provider
        .read_resource(
            read_context(tenant_id, "deletion-read"),
            read_request,
        )
        .await
        .expect_err("hard-deleted Flex schema must not remain readable through Translation");
    if deleted_read.kind != PortErrorKind::NotFound {
        return Err(test_error(format!(
            "hard-deleted Flex schema-copy returned unexpected read error: {deleted_read:?}"
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
    if !deleted_list.resources.is_empty() || deleted_list.next_cursor.is_some() {
        return Err(test_error(format!(
            "hard-deleted Flex schema remained in Translation inventory: {deleted_list:?}"
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
    assert_progress(&deleted_progress, 0, 0, 0, 0, 0, 0)?;
    if deleted_progress.owner_change_cursor.as_ref() != Some(&deletion_cursor) {
        return Err(test_error(format!(
            "deleted Flex schema-copy progress lost the owner delete high-water: {deleted_progress:?}"
        ))
        .into());
    }

    drop(deletion_provider);
    deletion_connection.close().await?;
    seed_connection.close().await?;
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
        AuthConfig::new("test-secret-key-for-flex-schema-copy-evidence-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    targets
        .get(&OwnerSlug::new(OWNER_SLUG)?, &ResourceKind::new(RESOURCE_KIND)?)
        .ok_or_else(|| test_error("host composition did not register flex/schema_copy").into())
}

fn schema_copy_definition() -> FieldDefinition {
    FieldDefinition {
        field_key: "audience".to_string(),
        field_type: FieldType::Select,
        label: HashMap::from([("en".to_string(), "Audience".to_string())]),
        description: Some(HashMap::from([(
            "en".to_string(),
            "Target audience".to_string(),
        )])),
        is_localized: false,
        is_required: false,
        default_value: None,
        validation: Some(ValidationRule {
            options: Some(vec![SelectOption {
                value: "professional".to_string(),
                label: HashMap::from([("en".to_string(), "Professional".to_string())]),
            }]),
            error_message: Some(HashMap::from([(
                "en".to_string(),
                "Choose an audience".to_string(),
            )])),
            ..Default::default()
        }),
        position: 0,
        is_active: true,
    }
}

fn assert_schema_copy_shape(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
) -> TestResult<()> {
    if snapshot.fields.len() != 6 {
        return Err(test_error(format!(
            "Flex schema-copy snapshot must expose six declared presentation leaves: {snapshot:?}"
        ))
        .into());
    }
    let required = snapshot
        .fields
        .iter()
        .filter(|field| field.descriptor.required)
        .count();
    let optional = snapshot.fields.len().saturating_sub(required);
    let plain = snapshot
        .fields
        .iter()
        .filter(|field| field.descriptor.profile == TranslationValueProfile::PlainText)
        .count();
    let localized_scalar = snapshot
        .fields
        .iter()
        .filter(|field| field.descriptor.profile == TranslationValueProfile::LocalizedScalar)
        .count();
    if required != 3
        || optional != 3
        || plain != 2
        || localized_scalar != 4
        || snapshot.fields.iter().any(|field| {
            field.descriptor.classification != TranslationDataClassification::TenantPrivate
                || !field.descriptor.ai_export_allowed
        })
    {
        return Err(test_error(format!(
            "Flex schema-copy field descriptors do not match the declared owner contract: {snapshot:?}"
        ))
        .into());
    }
    Ok(())
}

fn patch_from_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    suffix: &str,
    variant: &str,
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
                value: format!("{} FR {variant}", field.source_value),
                expected_source_hash: field.source_hash.clone(),
            })
            .collect(),
        proposal_id: format!("flex-schema-copy-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("flex-schema-copy-postgres-approval-{suffix}"),
    }
}

fn assert_target_values(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    patch: &TranslationPatchRequest,
) -> TestResult<()> {
    let expected = patch
        .fields
        .iter()
        .map(|field| (field.key.as_str().to_string(), field.value.clone()))
        .collect::<BTreeMap<_, _>>();
    for field in &snapshot.fields {
        if field.exact_target_value.as_ref() != expected.get(field.descriptor.key.as_str()) {
            return Err(test_error(format!(
                "Flex schema-copy exact target value mismatch for {}: snapshot={snapshot:?}, patch={patch:?}",
                field.descriptor.key.as_str()
            ))
            .into());
        }
    }
    Ok(())
}

fn assert_progress(
    progress: &rustok_translation_targets::TranslationTargetProgressFacts,
    resources: u64,
    complete_resources: u64,
    required_units: u64,
    exact_required_units: u64,
    optional_units: u64,
    exact_optional_units: u64,
) -> TestResult<()> {
    if progress.resources != resources
        || progress.complete_resources != complete_resources
        || progress.required_units != required_units
        || progress.exact_required_units != exact_required_units
        || progress.optional_units != optional_units
        || progress.exact_optional_units != exact_optional_units
    {
        return Err(test_error(format!(
            "unexpected Flex schema-copy aggregate progress: {progress:?}"
        ))
        .into());
    }
    Ok(())
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("flex-schema-copy-postgres-{suffix}"),
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
                "Flex schema-copy translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
