use std::{collections::{BTreeMap, BTreeSet, HashMap}, future::Future, time::Duration};

use rustok_api::TenantLocale;
use rustok_migrations::Migrator;
use rustok_modules::{
    ModuleCommandContext, ModuleSettingSpec, StaticSettingsLocalizationError,
    StaticSettingsLocalizationRegistry, StaticSettingsLocalizationService,
    StaticSettingsSourceLocaleAssignCommand, StaticSettingsSourceLocaleService,
    StaticTenantLifecycleStore,
    static_settings_translation_read::{
        StaticSettingsChangeReadRequest, StaticSettingsExactLocaleSnapshot,
        StaticSettingsTranslationReadService,
    },
};
use rustok_modules_translation::{
    StaticSettingsTranslationIdentity, StaticSettingsTranslationPrepareResult,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use rustok_translation_targets::{
    TranslationFieldPatch, TranslationPatchRequest, provider_support::field_hash,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const MODULE_SLUG: &str = "storefront";
const FIELD_ID: &str = "storefront.title";

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn settings_translation_multi_replica_apply_replay_and_cursor_recovery_postgres() {
    with_settings_postgres_database(
        "rustok_settings_translation_evidence",
        |replica_a, replica_b, recovery| async move {
            let tenant_id = Uuid::new_v4();
            let registry = settings_registry()?;
            seed_tenant(&replica_a, tenant_id).await?;
            seed_source_settings(&replica_a, tenant_id).await?;

            let source_locale_key = Uuid::new_v4();
            let source_locale = StaticSettingsSourceLocaleService::new(replica_a.clone())
                .assign_source_locale(
                    &registry,
                    StaticSettingsSourceLocaleAssignCommand {
                        tenant_id,
                        locale: "en".to_string(),
                        expected_owner_revision: 1,
                        context: command_context(tenant_id, source_locale_key),
                    },
                )
                .await?;
            if source_locale.locale != "en" || source_locale.base_projection_revision != 2 {
                return Err(format!(
                    "unexpected Settings source-locale receipt: {source_locale:?}"
                )
                .into());
            }

            let source_reader = StaticSettingsTranslationReadService::new(replica_a.clone());
            let source_changes = source_reader
                .read_changes(
                    &registry,
                    StaticSettingsChangeReadRequest {
                        tenant_id,
                        after_seq: None,
                        through_seq: None,
                        limit: 10,
                    },
                )
                .await?;
            let source_cursor = source_changes
                .through_seq
                .ok_or("Settings source change cursor is missing")?;
            if source_changes.changes.is_empty() || source_changes.next_after_seq.is_some() {
                return Err("Settings source change evidence was not fully drained".into());
            }

            let first_reader = StaticSettingsTranslationReadService::new(replica_a.clone());
            let second_reader = StaticSettingsTranslationReadService::new(replica_b.clone());
            let first_snapshot = first_reader
                .exact_locale_snapshot(tenant_id, &registry, "fr")
                .await?;
            let second_snapshot = second_reader
                .exact_locale_snapshot(tenant_id, &registry, "fr")
                .await?;
            assert_same_untranslated_snapshot(&first_snapshot, &second_snapshot)?;

            let identity = StaticSettingsTranslationIdentity::from_registry(&registry)?;
            let first_patch = patch_for_snapshot(&identity, &first_snapshot, "Bienvenue A", "a")?;
            let second_patch = patch_for_snapshot(&identity, &second_snapshot, "Bienvenue B", "b")?;
            let first_command = single_owner_command(
                &identity,
                &first_patch,
                &first_snapshot,
                command_context(tenant_id, Uuid::new_v4()),
            )?;
            let second_command = single_owner_command(
                &identity,
                &second_patch,
                &second_snapshot,
                command_context(tenant_id, Uuid::new_v4()),
            )?;

            let first_service = StaticSettingsLocalizationService::new(replica_a.clone());
            let second_service = StaticSettingsLocalizationService::new(replica_b.clone());
            let left = first_service.apply_exact(&registry, first_command.clone());
            let right = second_service.apply_exact(&registry, second_command.clone());
            let (left, right) = tokio::join!(left, right);

            let (winner_command, winner_record, loser_error) = match (left, right) {
                (Ok(record), Err(error)) => (first_command, record, error),
                (Err(error), Ok(record)) => (second_command, record, error),
                other => {
                    return Err(format!(
                        "exactly one concurrent Settings localized apply must win: {other:?}"
                    )
                    .into());
                }
            };
            if !matches!(
                loser_error,
                StaticSettingsLocalizationError::OwnerRevisionConflict { .. }
                    | StaticSettingsLocalizationError::OwnerOperationInProgress(_)
                    | StaticSettingsLocalizationError::TargetRevisionConflict { .. }
            ) {
                return Err(format!(
                    "concurrent Settings loser returned an unexpected error: {loser_error}"
                )
                .into());
            }
            if winner_record.target_revision != 1 || winner_record.owner_revision != 3 {
                return Err(format!(
                    "unexpected winning Settings revision receipt: {winner_record:?}"
                )
                .into());
            }

            let observer = StaticSettingsTranslationReadService::new(recovery.clone());
            let applied = observer
                .exact_locale_snapshot(tenant_id, &registry, "fr")
                .await?;
            let field = applied
                .fields
                .iter()
                .find(|field| field.field_id == FIELD_ID)
                .ok_or("recovered Settings field is missing")?;
            if field.exact_target_value.as_deref() != Some(winner_record.value.as_str())
                || field.target_revision != Some(1)
                || field.target_owner_revision != Some(3)
            {
                return Err(format!(
                    "fresh Settings replica did not recover the winning exact target: {field:?}"
                )
                .into());
            }

            let target_changes = observer
                .read_changes(
                    &registry,
                    StaticSettingsChangeReadRequest {
                        tenant_id,
                        after_seq: Some(source_cursor),
                        through_seq: None,
                        limit: 10,
                    },
                )
                .await?;
            let target_cursor = target_changes
                .through_seq
                .ok_or("Settings target change cursor is missing")?;
            if target_changes.changes.len() != 1
                || target_changes.changes[0].field_id.as_deref() != Some(FIELD_ID)
                || target_changes.changes[0].locale.as_deref() != Some("fr")
                || target_changes.changes[0].target_revision != Some(1)
                || target_changes.next_after_seq.is_some()
            {
                return Err(format!(
                    "unexpected bounded Settings target change page: {target_changes:?}"
                )
                .into());
            }

            let replay = StaticSettingsLocalizationService::new(recovery.clone())
                .apply_exact(&registry, winner_command)
                .await?;
            if replay != winner_record {
                return Err("fresh Settings replica did not replay the exact owner receipt".into());
            }

            let resumed = observer
                .read_changes(
                    &registry,
                    StaticSettingsChangeReadRequest {
                        tenant_id,
                        after_seq: Some(target_cursor),
                        through_seq: None,
                        limit: 10,
                    },
                )
                .await?;
            if !resumed.changes.is_empty() || resumed.next_after_seq.is_some() {
                return Err(format!(
                    "Settings change-cursor recovery redelivered committed work: {resumed:?}"
                )
                .into());
            }

            let recovered = observer
                .exact_locale_snapshot(tenant_id, &registry, "fr")
                .await?;
            let progress = recovered.progress();
            if recovered.source_locale != "en"
                || progress.source_units != 1
                || progress.exact_units != 1
                || progress.missing_units != 0
                || !progress.complete
                || progress.owner_change_seq != Some(target_cursor)
            {
                return Err(format!(
                    "recovered Settings exact progress is inconsistent: {progress:?}"
                )
                .into());
            }
            let revisions = identity.revisions_for_snapshot(&recovered)?;
            if revisions.target_revision.is_none() {
                return Err("neutral Settings target revision was not recovered".into());
            }

            Ok(())
        },
    )
    .await
    .unwrap_or_else(|error| panic!("Settings Translation PostgreSQL evidence failed: {error}"));
}

fn settings_registry() -> TestResult<StaticSettingsLocalizationRegistry> {
    Ok(StaticSettingsLocalizationRegistry::new(
        MODULE_SLUG,
        HashMap::from([(
            "title".to_string(),
            ModuleSettingSpec {
                value_type: "string".to_string(),
                ..Default::default()
            },
        )]),
        BTreeMap::from([(FIELD_ID.to_string(), "title".to_string())]),
        BTreeSet::new(),
    )?)
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Settings translation evidence".into(),
                format!("settings-translation-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

async fn seed_source_settings(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    let idempotency_key = Uuid::new_v4();
    StaticTenantLifecycleStore::claim(
        database,
        tenant_id,
        MODULE_SLUG,
        0,
        idempotency_key,
    )
    .await?;

    let write_result = database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings) \
             VALUES ($1, $2, $3, TRUE, $4)",
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                MODULE_SLUG.into(),
                serde_json::json!({"title": "Welcome"}).into(),
            ],
        ))
        .await;
    if let Err(error) = write_result {
        let _ = StaticTenantLifecycleStore::release(
            database,
            tenant_id,
            MODULE_SLUG,
            idempotency_key,
        )
        .await;
        return Err(error.into());
    }

    let revision = StaticTenantLifecycleStore::advance(
        database,
        tenant_id,
        MODULE_SLUG,
        0,
        idempotency_key,
    )
    .await?;
    StaticTenantLifecycleStore::release(database, tenant_id, MODULE_SLUG, idempotency_key).await?;
    if revision != 1 {
        return Err(format!("unexpected seeded Settings owner revision: {revision}").into());
    }
    Ok(())
}

fn assert_same_untranslated_snapshot(
    first: &StaticSettingsExactLocaleSnapshot,
    second: &StaticSettingsExactLocaleSnapshot,
) -> TestResult<()> {
    if first != second
        || first.source_locale != "en"
        || first.target_locale != "fr"
        || first.owner_revision != 2
        || first.fields.len() != 1
        || first.fields[0].field_id != FIELD_ID
        || first.fields[0].source_value != "Welcome"
        || first.fields[0].exact_target_value.is_some()
        || first.fields[0].target_revision.is_some()
    {
        return Err(format!(
            "independent Settings replicas did not read the same untranslated snapshot: first={first:?}, second={second:?}"
        )
        .into());
    }
    Ok(())
}

fn patch_for_snapshot(
    identity: &StaticSettingsTranslationIdentity,
    snapshot: &StaticSettingsExactLocaleSnapshot,
    value: &str,
    suffix: &str,
) -> TestResult<TranslationPatchRequest> {
    let revisions = identity.revisions_for_snapshot(snapshot)?;
    let field = snapshot.fields.first().ok_or("Settings source field is missing")?;
    Ok(TranslationPatchRequest {
        identity: identity.resource().clone(),
        source_locale: TenantLocale::new(snapshot.source_locale.clone())?,
        target_locale: TenantLocale::new(snapshot.target_locale.clone())?,
        expected_resource_revision: revisions.resource_revision,
        expected_source_revision: revisions.source_revision,
        expected_target_revision: revisions.target_revision,
        fields: vec![TranslationFieldPatch {
            key: rustok_translation_targets::FieldKey::new(field.field_id.clone())?,
            value: value.to_string(),
            expected_source_hash: field_hash(&field.source_value),
        }],
        proposal_id: format!("settings-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("settings-postgres-approval-{suffix}"),
    })
}

fn single_owner_command(
    identity: &StaticSettingsTranslationIdentity,
    patch: &TranslationPatchRequest,
    snapshot: &StaticSettingsExactLocaleSnapshot,
    context: ModuleCommandContext,
) -> TestResult<rustok_modules::StaticLocalizedSettingApplyCommand> {
    let plan = match identity.prepare_apply_plan(patch, snapshot, &context)? {
        StaticSettingsTranslationPrepareResult::Ready(plan) => plan,
        StaticSettingsTranslationPrepareResult::Rejected(validation) => {
            return Err(format!("Settings neutral apply plan was rejected: {validation:?}").into());
        }
    };
    if plan.commands.len() != 1 || plan.final_owner_revision != snapshot.owner_revision + 1 {
        return Err(format!("unexpected Settings owner apply plan: {plan:?}").into());
    }
    plan.commands
        .into_iter()
        .next()
        .ok_or_else(|| "Settings owner apply command is missing".into())
}

fn command_context(tenant_id: Uuid, idempotency_key: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: Some(tenant_id),
        trace_id: Uuid::new_v4().to_string(),
        correlation_id: Uuid::new_v4(),
        idempotency_key,
    }
}

async fn with_settings_postgres_database<T, F, Fut>(prefix: &str, test: F) -> TestResult<T>
where
    F: FnOnce(DatabaseConnection, DatabaseConnection, DatabaseConnection) -> Fut,
    Fut: Future<Output = TestResult<T>>,
{
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name(prefix);
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url)
        .await
        .map_err(|error| format!("PostgreSQL admin database must be reachable: {error}"))?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let test_result = async {
        let replica_a = connect_postgres(&target_url).await?;
        Migrator::up(&replica_a, None).await?;
        let replica_b = connect_postgres(&target_url).await?;
        let recovery = connect_postgres(&target_url).await?;
        let result = test(replica_a.clone(), replica_b.clone(), recovery.clone()).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        replica_a.close().await?;
        replica_b.close().await?;
        recovery.close().await?;
        result
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    test_result
}
