#![cfg(feature = "mod-alloy")]

use std::{error::Error, io, sync::Arc, time::Duration};

use alloy::storage::{
    SeaOrmScriptAuthoringStore, SeaOrmScriptPresentationStore,
    ScriptPresentationAuthoringMutation, ScriptPresentationStore,
};
use alloy::{
    AlloyModule, RhaiWorkspace, Script, ScriptDeletionCommand, ScriptRegistry, ScriptTrigger,
    SeaOrmStorage,
};
use rustok_api::{PortActor, PortContext, PortErrorKind, RuntimeLocale, StoredLocale, TenantLocale};
use rustok_core::ModuleRegistry;
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
    TranslationDataClassification, TranslationFieldPatch, TranslationPatchRequest,
    TranslationResourceLifecycle, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, FromQueryResult, Statement};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "alloy";
const RESOURCE_KIND: &str = "script_presentation";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, FromQueryResult)]
struct CountRow {
    count: i64,
}

#[derive(Debug, FromQueryResult)]
struct RevisionRow {
    revision: i64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn alloy_script_presentation_registered_provider_multi_replica_evidence_postgres(
) -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_alloy_translation_evidence");
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
    let legacy_tenant_id = Uuid::new_v4();
    seed_tenant(&seed_connection, tenant_id, "alloy-translation").await?;
    seed_tenant(&seed_connection, other_tenant_id, "alloy-isolation").await?;
    seed_tenant(&seed_connection, legacy_tenant_id, "alloy-legacy").await?;

    verify_legacy_und_and_change_state_backfill(&seed_connection, legacy_tenant_id).await?;

    let owner = SeaOrmScriptAuthoringStore::new(seed_connection.clone(), tenant_id);
    let other_owner = SeaOrmScriptAuthoringStore::new(seed_connection.clone(), other_tenant_id);
    let mut script = create_source_script(
        &owner,
        tenant_id,
        "alloy-translation-primary",
        "Source description",
    )
    .await?;
    let other_script = create_source_script(
        &other_owner,
        other_tenant_id,
        "alloy-translation-other",
        "Other tenant description",
    )
    .await?;

    let seed_provider = registered_provider(seed_connection.clone())?;
    let descriptor = seed_provider.descriptor();
    if descriptor.owner_slug.as_str() != OWNER_SLUG
        || descriptor.resource_kind.as_str() != RESOURCE_KIND
        || !descriptor.read_permission_floor.contains("scripts:manage")
        || !descriptor.apply_permission_floor.contains("scripts:manage")
    {
        return Err(test_error(format!(
            "registered Alloy Translation descriptor is invalid: {descriptor:?}"
        ))
        .into());
    }

    let denied = seed_provider
        .list_resources(
            service_read_context(tenant_id, "denied", "scripts:read"),
            list_request()?,
        )
        .await
        .expect_err("scripts:read must not satisfy scripts:manage Translation policy");
    if denied.kind != PortErrorKind::Forbidden {
        return Err(test_error(format!(
            "Alloy Translation policy denial must be Forbidden, got {denied:?}"
        ))
        .into());
    }

    let listed = seed_provider
        .list_resources(
            service_read_context(tenant_id, "allowed-list", "scripts:manage"),
            list_request()?,
        )
        .await?;
    if listed.resources.len() != 1 || listed.next_cursor.is_some() {
        return Err(test_error(format!(
            "registered Alloy provider returned unexpected primary inventory: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    if identity.owner_slug.as_str() != OWNER_SLUG
        || identity.resource_kind.as_str() != RESOURCE_KIND
        || identity.resource_id.as_str() != script.id.to_string()
        || identity.subresource_id.is_some()
    {
        return Err(test_error(format!(
            "registered Alloy provider returned unexpected identity: {identity:?}"
        ))
        .into());
    }

    let other_listed = seed_provider
        .list_resources(read_context(other_tenant_id, "other-list"), list_request()?)
        .await?;
    if other_listed.resources.len() != 1
        || other_listed.resources[0].identity.resource_id.as_str() != other_script.id.to_string()
        || other_listed.resources[0].identity == identity
    {
        return Err(test_error(format!(
            "Alloy Translation inventory leaked across tenants: primary={listed:?} other={other_listed:?}"
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
            service_read_context(tenant_id, "initial-read", "scripts:manage"),
            read_request.clone(),
        )
        .await?;
    if initial.fields.len() != 1
        || initial.fields[0].descriptor.key.as_str() != "description"
        || initial.fields[0].source_value != "Source description"
        || initial.fields[0].descriptor.required
        || initial.fields[0].descriptor.ai_export_allowed
        || initial.fields[0].descriptor.classification
            != TranslationDataClassification::TenantPrivate
        || initial.fields[0].exact_target_value.is_some()
        || initial.summary.lifecycle != TranslationResourceLifecycle::Active
        || initial.summary.exact_locales != vec![TenantLocale::new("en")?]
    {
        return Err(test_error(format!(
            "initial Alloy exact-locale snapshot is invalid: {initial:?}"
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
        || progress_initial.complete_resources != 1
        || progress_initial.required_units != 0
        || progress_initial.exact_required_units != 0
        || progress_initial.optional_units != 1
        || progress_initial.exact_optional_units != 0
    {
        return Err(test_error(format!(
            "initial Alloy aggregate progress is invalid: {progress_initial:?}"
        ))
        .into());
    }

    let before_operational = initial.clone();
    let progress_before_operational = progress_initial.clone();
    let mut operational = script.clone();
    operational.name = "alloy-translation-primary-renamed".to_string();
    operational.permissions = vec!["orders:read".to_string()];
    script = owner.update(&script, operational, None).await?;

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
    if after_operational.summary.resource_revision
        != before_operational.summary.resource_revision
        || after_operational.source_revision != before_operational.source_revision
        || progress_after_operational.owner_change_cursor
            != progress_before_operational.owner_change_cursor
    {
        return Err(test_error(format!(
            "operational-only Alloy authoring changed Translation evidence: before={before_operational:?} after={after_operational:?} progress_before={progress_before_operational:?} progress_after={progress_after_operational:?}"
        ))
        .into());
    }

    script = update_source_description(
        &seed_connection,
        &owner,
        script,
        "Source description v2",
    )
    .await?;
    let source_updated = seed_provider
        .read_resource(
            read_context(tenant_id, "source-updated"),
            read_request.clone(),
        )
        .await?;
    if source_updated.fields[0].source_value != "Source description v2"
        || source_updated.summary.resource_revision == initial.summary.resource_revision
        || source_updated.source_revision == initial.source_revision
    {
        return Err(test_error(format!(
            "canonical Alloy presentation update did not rotate Translation revisions: {source_updated:?}"
        ))
        .into());
    }

    let first_connection = connect_postgres(database_url).await?;
    let second_connection = connect_postgres(database_url).await?;
    let first_provider = registered_provider(first_connection.clone())?;
    let second_provider = registered_provider(second_connection.clone())?;

    let initial_patch = patch_from_snapshot(&source_updated, "Description FR initial", "initial");
    let initial_receipt = first_provider
        .apply_patch(
            apply_context(tenant_id, "initial-apply", "alloy-translation-initial"),
            initial_patch.clone(),
        )
        .await?;
    let replay_receipt = second_provider
        .apply_patch(
            apply_context(tenant_id, "initial-replay", "alloy-translation-initial"),
            initial_patch.clone(),
        )
        .await?;
    assert_same_receipt("initial replay", &initial_receipt, &replay_receipt)?;

    let after_initial_apply = first_provider
        .read_resource(
            read_context(tenant_id, "after-initial-apply"),
            read_request.clone(),
        )
        .await?;
    let later_patch = patch_from_snapshot(&after_initial_apply, "Description FR later", "later");
    second_provider
        .apply_patch(
            apply_context(tenant_id, "later-apply", "alloy-translation-later"),
            later_patch,
        )
        .await?;

    let replay_after_later_change = first_provider
        .apply_patch(
            apply_context(
                tenant_id,
                "replay-after-later-change",
                "alloy-translation-initial",
            ),
            initial_patch,
        )
        .await?;
    assert_same_receipt(
        "replay after later owner change",
        &initial_receipt,
        &replay_after_later_change,
    )?;

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
        || first_snapshot.source_revision != second_snapshot.source_revision
        || first_snapshot.target_revision != second_snapshot.target_revision
    {
        return Err(test_error(
            "independent Alloy replicas did not observe identical owner revisions",
        )
        .into());
    }

    let first_patch = patch_from_snapshot(&first_snapshot, "Description FR replica A", "replica-a");
    let second_patch =
        patch_from_snapshot(&second_snapshot, "Description FR replica B", "replica-b");
    let first_apply = first_provider.apply_patch(
        apply_context(tenant_id, "replica-a-apply", "alloy-translation-replica-a"),
        first_patch,
    );
    let second_apply = second_provider.apply_patch(
        apply_context(tenant_id, "replica-b-apply", "alloy-translation-replica-b"),
        second_patch,
    );
    let (first_result, second_result) = tokio::join!(first_apply, second_apply);
    match (first_result, second_result) {
        (Ok(_), Err(error)) | (Err(error), Ok(_)) if error.kind == PortErrorKind::Conflict => {}
        (Ok(first), Ok(second)) => {
            return Err(test_error(format!(
                "concurrent Alloy replicas both applied one stale revision: {first:?} / {second:?}"
            ))
            .into());
        }
        (Err(first), Err(second)) => {
            return Err(test_error(format!(
                "both Alloy replicas failed concurrent CAS: {first:?} / {second:?}"
            ))
            .into());
        }
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => {
            return Err(test_error(format!(
                "losing Alloy replica must return Conflict, got {error:?}"
            ))
            .into());
        }
    }

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
        || progress_complete.required_units != 0
        || progress_complete.exact_required_units != 0
        || progress_complete.optional_units != 1
        || progress_complete.exact_optional_units != 1
    {
        return Err(test_error(format!(
            "Alloy progress did not converge after exact target apply: {progress_complete:?}"
        ))
        .into());
    }

    let settled = seed_provider
        .read_changes(
            read_context(tenant_id, "settled-cursor"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 200,
            },
        )
        .await?;
    let settled_cursor = settled
        .next_cursor
        .ok_or_else(|| test_error("Alloy settled ChangeCursor is missing"))?;

    script = update_source_description(&seed_connection, &owner, script, "Cursor source A").await?;
    script = update_source_description(&seed_connection, &owner, script, "Cursor source B").await?;
    let frozen_first = seed_provider
        .read_changes(
            read_context(tenant_id, "frozen-first"),
            TranslationTargetChangesRequest {
                after: Some(settled_cursor),
                limit: 1,
            },
        )
        .await?;
    if frozen_first.changes.len() != 1 || frozen_first.changes[0].identity != identity {
        return Err(test_error(format!(
            "first Alloy frozen-window page is invalid: {frozen_first:?}"
        ))
        .into());
    }
    let frozen_mid = frozen_first
        .next_cursor
        .ok_or_else(|| test_error("Alloy frozen-window intermediate cursor is missing"))?;

    script = update_source_description(&seed_connection, &owner, script, "Cursor source late").await?;
    let frozen_second = seed_provider
        .read_changes(
            read_context(tenant_id, "frozen-second"),
            TranslationTargetChangesRequest {
                after: Some(frozen_mid),
                limit: 1,
            },
        )
        .await?;
    if frozen_second.changes.len() != 1 || frozen_second.changes[0].identity != identity {
        return Err(test_error(format!(
            "second Alloy frozen-window page is invalid: {frozen_second:?}"
        ))
        .into());
    }
    let frozen_tail = frozen_second
        .next_cursor
        .ok_or_else(|| test_error("Alloy frozen-window tail cursor is missing"))?;
    let late_window = seed_provider
        .read_changes(
            read_context(tenant_id, "late-window"),
            TranslationTargetChangesRequest {
                after: Some(frozen_tail),
                limit: 10,
            },
        )
        .await?;
    if late_window.changes.len() != 1 || late_window.changes[0].identity != identity {
        return Err(test_error(format!(
            "late Alloy change did not wait for the next polling window: {late_window:?}"
        ))
        .into());
    }
    let pre_delete_cursor = late_window
        .next_cursor
        .ok_or_else(|| test_error("Alloy pre-delete tail cursor is missing"))?;

    let registry = SeaOrmStorage::new(seed_connection.clone()).for_tenant(tenant_id);
    registry
        .delete(ScriptDeletionCommand {
            script_id: script.id,
            expected_revision: script.version,
            actor_id: "alloy-translation-postgres".to_string(),
            reason: "translation lifecycle evidence".to_string(),
            idempotency_key: Uuid::new_v4(),
        })
        .await?;

    let deleted = seed_provider
        .read_changes(
            read_context(tenant_id, "deleted"),
            TranslationTargetChangesRequest {
                after: Some(pre_delete_cursor),
                limit: 10,
            },
        )
        .await?;
    if deleted.changes.len() != 1
        || deleted.changes[0].identity != identity
        || deleted.changes[0].lifecycle != TranslationResourceLifecycle::Deleted
    {
        return Err(test_error(format!(
            "canonical Alloy hard delete did not retain one Translation tombstone: {deleted:?}"
        ))
        .into());
    }
    let missing = seed_provider
        .read_resource(read_context(tenant_id, "deleted-read"), read_request)
        .await
        .expect_err("deleted Alloy presentation must not remain readable");
    if missing.kind != PortErrorKind::NotFound {
        return Err(test_error(format!(
            "deleted Alloy presentation must return NotFound, got {missing:?}"
        ))
        .into());
    }
    assert_live_owner_rows_removed(&seed_connection, tenant_id, script.id).await?;

    let other_after_delete = seed_provider
        .list_resources(read_context(other_tenant_id, "other-after-delete"), list_request()?)
        .await?;
    if other_after_delete.resources.len() != 1
        || other_after_delete.resources[0].identity.resource_id.as_str()
            != other_script.id.to_string()
    {
        return Err(test_error(format!(
            "primary Alloy delete affected another tenant: {other_after_delete:?}"
        ))
        .into());
    }

    first_connection.close().await?;
    second_connection.close().await?;
    seed_connection.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

async fn verify_legacy_und_and_change_state_backfill(
    database: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<()> {
    let mut migrations = alloy::migrations::migrations();
    let change_plane = migrations
        .pop()
        .ok_or_else(|| test_error("Alloy Translation change-plane migration is missing"))?;
    let presentations = migrations
        .pop()
        .ok_or_else(|| test_error("Alloy Script presentation migration is missing"))?;
    let manager = SchemaManager::new(database);
    change_plane.down(&manager).await?;
    presentations.down(&manager).await?;

    let legacy_owner = SeaOrmScriptAuthoringStore::new(database.clone(), tenant_id);
    let mut legacy = Script::new(
        "alloy-legacy-und",
        RhaiWorkspace::single_source("40 + 2"),
        ScriptTrigger::Manual,
    );
    legacy.tenant_id = tenant_id;
    let legacy = legacy_owner.create(legacy, None).await?;
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE scripts SET description = $2 WHERE id = $1 AND tenant_id = $3",
            vec![
                legacy.id.into(),
                "Legacy description without provenance".into(),
                tenant_id.into(),
            ],
        ))
        .await?;

    presentations.up(&manager).await?;
    let presentation_store = SeaOrmScriptPresentationStore::new(database.clone());
    let und = presentation_store
        .find_exact(tenant_id, legacy.id, &StoredLocale::new("und")?)
        .await?
        .ok_or_else(|| test_error("legacy Alloy description was not backfilled to und"))?;
    if und.description.as_deref() != Some("Legacy description without provenance")
        || !und.locale.is_unknown_provenance()
    {
        return Err(test_error(format!(
            "legacy Alloy provenance backfill is invalid: {und:?}"
        ))
        .into());
    }

    change_plane.up(&manager).await?;
    let state = RevisionRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT revision FROM alloy_script_presentation_translation_resource_state WHERE tenant_id = $1 AND script_id = $2",
        vec![tenant_id.into(), legacy.id.into()],
    ))
    .one(database)
    .await?
    .ok_or_else(|| test_error("legacy Alloy presentation did not receive resource state"))?;
    if state.revision != 1 {
        return Err(test_error(format!(
            "legacy Alloy presentation state must start at revision 1, got {}",
            state.revision
        ))
        .into());
    }
    let journal = count_rows(
        database,
        "SELECT COUNT(*) AS count FROM alloy_script_presentation_translation_change_journal WHERE tenant_id = $1",
        vec![tenant_id.into()],
    )
    .await?;
    if journal != 0 {
        return Err(test_error(format!(
            "legacy Alloy state backfill invented {journal} historical change rows"
        ))
        .into());
    }

    let provider = registered_provider(database.clone())?;
    let listed = provider
        .list_resources(read_context(tenant_id, "legacy-list"), list_request()?)
        .await?;
    if !listed.resources.is_empty() {
        return Err(test_error(format!(
            "storage-only und Alloy provenance leaked into concrete locale inventory: {listed:?}"
        ))
        .into());
    }
    let changes = provider
        .read_changes(
            read_context(tenant_id, "legacy-changes"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 10,
            },
        )
        .await?;
    if !changes.changes.is_empty() || changes.next_cursor.is_some() {
        return Err(test_error(format!(
            "legacy Alloy state backfill exposed fabricated ChangeCursor history: {changes:?}"
        ))
        .into());
    }
    Ok(())
}

fn registered_provider(db: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(AlloyModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-alloy-translation-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    targets
        .get(&OwnerSlug::new(OWNER_SLUG)?, &ResourceKind::new(RESOURCE_KIND)?)
        .ok_or_else(|| {
            test_error("host composition did not register alloy/script_presentation").into()
        })
}

async fn create_source_script(
    owner: &SeaOrmScriptAuthoringStore,
    tenant_id: Uuid,
    name: &str,
    description: &str,
) -> TestResult<Script> {
    let mut script = Script::new(
        name,
        RhaiWorkspace::single_source("40 + 2"),
        ScriptTrigger::Manual,
    );
    script.tenant_id = tenant_id;
    script.description = Some(description.to_string());
    Ok(owner
        .create(
            script,
            Some(ScriptPresentationAuthoringMutation {
                source_locale: RuntimeLocale::new("en")?,
                expected_copy_revision: None,
                description: Some(description.to_string()),
            }),
        )
        .await?)
}

async fn update_source_description(
    database: &DatabaseConnection,
    owner: &SeaOrmScriptAuthoringStore,
    current: Script,
    description: &str,
) -> TestResult<Script> {
    let presentations = SeaOrmScriptPresentationStore::new(database.clone());
    let source = presentations
        .find_exact(current.tenant_id, current.id, &StoredLocale::new("en")?)
        .await?
        .ok_or_else(|| test_error("Alloy source presentation is missing"))?;
    let mut next = current.clone();
    next.description = Some(description.to_string());
    Ok(owner
        .update(
            &current,
            next,
            Some(ScriptPresentationAuthoringMutation {
                source_locale: RuntimeLocale::new("en")?,
                expected_copy_revision: Some(source.copy_revision),
                description: Some(description.to_string()),
            }),
        )
        .await?)
}

fn list_request() -> TestResult<ListTranslationResourcesRequest> {
    Ok(ListTranslationResourcesRequest {
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
        cursor: None,
        limit: 10,
    })
}

fn patch_from_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    description: &str,
    suffix: &str,
) -> TranslationPatchRequest {
    assert_eq!(snapshot.fields.len(), 1);
    assert_eq!(snapshot.fields[0].descriptor.key.as_str(), "description");
    TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![TranslationFieldPatch {
            key: snapshot.fields[0].descriptor.key.clone(),
            value: description.to_string(),
            expected_source_hash: snapshot.fields[0].source_hash.clone(),
        }],
        proposal_id: format!("alloy-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("alloy-postgres-approval-{suffix}"),
    }
}

fn assert_same_receipt(
    label: &str,
    expected: &rustok_translation_targets::TranslationApplicationReceipt,
    actual: &rustok_translation_targets::TranslationApplicationReceipt,
) -> TestResult<()> {
    if expected.provider_receipt_id != actual.provider_receipt_id
        || expected.resource_revision != actual.resource_revision
        || expected.target_revision != actual.target_revision
        || expected.applied_field_keys != actual.applied_field_keys
    {
        return Err(test_error(format!(
            "{label} did not return the durable owner receipt: expected={expected:?} actual={actual:?}"
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
        format!("alloy-postgres-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn service_read_context(tenant_id: Uuid, suffix: &str, claim: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(Uuid::new_v4().to_string()),
        "en",
        format!("alloy-postgres-{suffix}"),
    )
    .with_claim(claim)
    .with_role("admin")
    .with_deadline(Duration::from_secs(30))
}

fn apply_context(tenant_id: Uuid, suffix: &str, idempotency_key: &str) -> PortContext {
    read_context(tenant_id, suffix).with_idempotency_key(idempotency_key)
}

async fn assert_live_owner_rows_removed(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    script_id: Uuid,
) -> TestResult<()> {
    let state = count_rows(
        database,
        "SELECT COUNT(*) AS count FROM alloy_script_presentation_translation_resource_state WHERE tenant_id = $1 AND script_id = $2",
        vec![tenant_id.into(), script_id.into()],
    )
    .await?;
    let presentations = count_rows(
        database,
        "SELECT COUNT(*) AS count FROM alloy_script_presentations WHERE tenant_id = $1 AND script_id = $2",
        vec![tenant_id.into(), script_id.into()],
    )
    .await?;
    if state != 0 || presentations != 0 {
        return Err(test_error(format!(
            "canonical Alloy hard delete left live Translation rows: state={state} presentations={presentations}"
        ))
        .into());
    }
    Ok(())
}

async fn count_rows(
    database: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<i64> {
    let row = CountRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        values,
    ))
    .one(database)
    .await?
    .ok_or_else(|| test_error("COUNT query returned no row"))?;
    Ok(row.count)
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
                "Alloy Translation evidence".into(),
                format!("{slug_prefix}-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
