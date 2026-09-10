#![cfg(all(feature = "mod-taxonomy", feature = "mod-flex"))]

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use flex::{
    CreateFieldDefinitionCommand, FieldDefinitionService, FlexAttachedTranslationChangeOwnerPort,
    FlexAttachedTranslationChangeReader, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE, load_attached_translation_resource_revisions,
    persist_localized_values,
};
use rustok_api::{PortActor, PortContext, TenantLocale};
use rustok_core::{ModuleRegistry, SecurityContext, UserRole, field_schema::FieldType};
use rustok_migrations::Migrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        flex_attached_values::FlexTaxonomyCategoryDeleteCleanup,
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        server_runtime_context::ServerRuntimeContext,
    },
};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug, ReadTranslationResourceRequest,
    ResourceKind, TranslationFieldPatch, TranslationPatchRequest, TranslationResourceLifecycle,
    TranslationTargetCapability, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, provider_support::field_hash, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const JOURNAL_MIGRATION: &str = "m20260909_000003_add_attached_translation_change_journal";
const FIELD_KEY: &str = "headline";

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn attached_translation_registered_provider_retains_owner_revision_cas_progress_and_cursor_evidence() {
    with_flex_translation_postgres_database(
        "rustok_flex_attached_translation_evidence",
        |replica_a, replica_b, recovery| async move {
            let tenant_id = Uuid::new_v4();
            seed_tenant(&replica_a, tenant_id).await?;
            let category_id = create_category(&replica_a, tenant_id).await?;
            seed_localized_definition_and_source(&replica_a, tenant_id, category_id).await?;

            apply_journal_migration(&replica_a).await?;
            assert_backfill_without_historical_change(&replica_a, tenant_id, category_id).await?;
            Migrator::up(&replica_a, None).await?;

            let provider_a = registered_provider(replica_a.clone())?;
            let provider_b = registered_provider(replica_b.clone())?;
            let recovered_provider = registered_provider(recovery.clone())?;
            assert_registered_capabilities(provider_a.as_ref())?;

            let source_locale = TenantLocale::new("en")?;
            let target_locale = TenantLocale::new("fr")?;
            let initial_page = provider_a
                .list_resources(
                    read_context(tenant_id),
                    ListTranslationResourcesRequest {
                        source_locale: source_locale.clone(),
                        target_locale: target_locale.clone(),
                        cursor: None,
                        limit: 10,
                    },
                )
                .await?;
            if initial_page.resources.len() != 1 || initial_page.next_cursor.is_some() {
                return Err(format!("unexpected initial attached provider page: {initial_page:?}").into());
            }
            let identity = initial_page.resources[0].identity.clone();
            if identity.subresource_id.as_ref().map(|value| value.as_str())
                != Some(TAXONOMY_CATEGORY_ENTITY_TYPE)
                || initial_page.resources[0].resource_revision.as_str() != "attached:1"
            {
                return Err(format!(
                    "registered provider did not expose the backfilled Category identity/revision: {:?}",
                    initial_page.resources[0]
                )
                .into());
            }

            let initial_read = ReadTranslationResourceRequest {
                identity: identity.clone(),
                source_locale: source_locale.clone(),
                target_locale: target_locale.clone(),
            };
            let snapshot_a = provider_a
                .read_resource(read_context(tenant_id), initial_read.clone())
                .await?;
            let snapshot_b = provider_b
                .read_resource(read_context(tenant_id), initial_read.clone())
                .await?;
            if snapshot_a != snapshot_b
                || snapshot_a.summary.resource_revision.as_str() != "attached:1"
                || snapshot_a.target_revision.is_some()
                || snapshot_a.fields.len() != 1
                || snapshot_a.fields[0].source_value != "Hello"
                || snapshot_a.fields[0].exact_target_value.is_some()
            {
                return Err(format!(
                    "independent registered providers disagreed on the exact owner snapshot: a={snapshot_a:?}, b={snapshot_b:?}"
                )
                .into());
            }

            let initial_progress = provider_a
                .read_progress(
                    read_context(tenant_id),
                    TranslationTargetProgressRequest {
                        source_locale: source_locale.clone(),
                        target_locale: target_locale.clone(),
                    },
                )
                .await?;
            if initial_progress.required_units != 1
                || initial_progress.exact_required_units != 0
                || initial_progress.optional_units != 0
                || initial_progress.exact_optional_units != 0
                || initial_progress.resources != 1
                || initial_progress.complete_resources != 0
                || initial_progress.owner_change_cursor.is_some()
            {
                return Err(format!("unexpected initial attached progress: {initial_progress:?}").into());
            }

            let first_patch = patch_for_snapshot(&snapshot_a, "Bonjour A", "a")?;
            let second_patch = patch_for_snapshot(&snapshot_b, "Bonjour B", "b")?;
            let validation = provider_a
                .validate_patch(read_context(tenant_id), first_patch.clone())
                .await?;
            if !validation.accepted {
                return Err(format!("valid attached provider patch was rejected: {validation:?}").into());
            }

            let first_context = write_context(tenant_id, "flex-attached-postgres-a");
            let second_context = write_context(tenant_id, "flex-attached-postgres-b");
            let (left, right) = tokio::join!(
                provider_a.apply_patch(first_context.clone(), first_patch.clone()),
                provider_b.apply_patch(second_context.clone(), second_patch.clone()),
            );
            let (winner_context, winner_patch, winner_value, winner_receipt, loser_error) =
                match (left, right) {
                    (Ok(receipt), Err(error)) => (
                        first_context,
                        first_patch,
                        "Bonjour A",
                        receipt,
                        error,
                    ),
                    (Err(error), Ok(receipt)) => (
                        second_context,
                        second_patch,
                        "Bonjour B",
                        receipt,
                        error,
                    ),
                    other => {
                        return Err(format!(
                            "exactly one concurrent registered-provider apply must win: {other:?}"
                        )
                        .into());
                    }
                };
            if loser_error.code != "flex.attached_translation_revision_conflict" {
                return Err(format!(
                    "concurrent attached provider loser returned an unexpected error: {loser_error:?}"
                )
                .into());
            }
            if winner_receipt.resource_revision.as_str() != "attached:2" {
                return Err(format!(
                    "winning attached provider apply did not advance exactly one durable resource revision: {winner_receipt:?}"
                )
                .into());
            }

            let replay = recovered_provider
                .apply_patch(winner_context.clone(), winner_patch.clone())
                .await?;
            if replay != winner_receipt {
                return Err("fresh provider replica did not replay the exact durable owner receipt".into());
            }

            let applied = recovered_provider
                .read_resource(read_context(tenant_id), initial_read.clone())
                .await?;
            if applied.summary.resource_revision != winner_receipt.resource_revision
                || applied.target_revision.as_ref() != Some(&winner_receipt.target_revision)
                || applied.fields[0].exact_target_value.as_deref() != Some(winner_value)
            {
                return Err(format!(
                    "fresh provider replica did not recover the winning exact target: {applied:?}"
                )
                .into());
            }
            let applied_progress = recovered_provider
                .read_progress(
                    read_context(tenant_id),
                    TranslationTargetProgressRequest {
                        source_locale: source_locale.clone(),
                        target_locale: target_locale.clone(),
                    },
                )
                .await?;
            if applied_progress.required_units != 1
                || applied_progress.exact_required_units != 1
                || applied_progress.resources != 1
                || applied_progress.complete_resources != 1
                || applied_progress.owner_change_cursor.is_none()
            {
                return Err(format!("unexpected applied attached progress: {applied_progress:?}").into());
            }

            assert_revision_cas_matrix(
                recovered_provider.as_ref(),
                tenant_id,
                &applied,
                winner_value,
            )
            .await?;

            let updated = recovery
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "UPDATE flex_attached_field_definitions SET is_required = FALSE \
                     WHERE tenant_id = $1 AND entity_type = $2 AND field_key = $3",
                    vec![
                        tenant_id.into(),
                        TAXONOMY_CATEGORY_ENTITY_TYPE.into(),
                        FIELD_KEY.into(),
                    ],
                ))
                .await?;
            if updated.rows_affected() != 1 {
                return Err("attached schema fan-out fixture did not update exactly one field".into());
            }

            let after_schema = recovered_provider
                .read_resource(read_context(tenant_id), initial_read.clone())
                .await?;
            if after_schema.summary.resource_revision.as_str() != "attached:3"
                || after_schema.fields[0].descriptor.required
                || after_schema.fields[0].exact_target_value.as_deref() != Some(winner_value)
            {
                return Err(format!(
                    "OLD/NEW schema fan-out did not collapse to one resource revision: {after_schema:?}"
                )
                .into());
            }
            let schema_progress = recovered_provider
                .read_progress(
                    read_context(tenant_id),
                    TranslationTargetProgressRequest {
                        source_locale: source_locale.clone(),
                        target_locale: target_locale.clone(),
                    },
                )
                .await?;
            if schema_progress.required_units != 0
                || schema_progress.exact_required_units != 0
                || schema_progress.optional_units != 1
                || schema_progress.exact_optional_units != 1
                || schema_progress.resources != 1
                || schema_progress.complete_resources != 1
            {
                return Err(format!(
                    "aggregate progress did not observe the same schema/resource revision: {schema_progress:?}"
                )
                .into());
            }

            TaxonomyService::new(recovery.clone())
                .delete_category_with_cleanup(
                    tenant_id,
                    category_id,
                    admin(),
                    &FlexTaxonomyCategoryDeleteCleanup,
                )
                .await?;

            let (changes, terminal_cursor) =
                drain_changes_one_at_a_time(recovered_provider.as_ref(), tenant_id).await?;
            if changes.len() != 3
                || changes[0].resource_revision.as_str() != "attached:2"
                || changes[0].lifecycle != TranslationResourceLifecycle::Active
                || changes[1].resource_revision.as_str() != "attached:3"
                || changes[1].lifecycle != TranslationResourceLifecycle::Active
                || changes[2].lifecycle != TranslationResourceLifecycle::Deleted
                || !changes[2]
                    .resource_revision
                    .as_str()
                    .starts_with("deleted:taxonomy.category:")
            {
                return Err(format!(
                    "bounded attached ChangeCursor did not retain apply/schema/delete order: {changes:?}"
                )
                .into());
            }

            let resumed = recovered_provider
                .read_changes(
                    read_context(tenant_id),
                    TranslationTargetChangesRequest {
                        after: Some(terminal_cursor.clone()),
                        limit: 1,
                    },
                )
                .await?;
            if !resumed.changes.is_empty() || resumed.next_cursor.as_ref() != Some(&terminal_cursor) {
                return Err(format!(
                    "attached ChangeCursor recovery redelivered committed work: {resumed:?}"
                )
                .into());
            }

            let deleted_progress = recovered_provider
                .read_progress(
                    read_context(tenant_id),
                    TranslationTargetProgressRequest {
                        source_locale,
                        target_locale,
                    },
                )
                .await?;
            if deleted_progress.required_units != 0
                || deleted_progress.exact_required_units != 0
                || deleted_progress.optional_units != 0
                || deleted_progress.exact_optional_units != 0
                || deleted_progress.resources != 0
                || deleted_progress.complete_resources != 0
                || deleted_progress.owner_change_cursor.as_ref() != Some(&terminal_cursor)
            {
                return Err(format!(
                    "post-delete aggregate progress/cursor evidence is inconsistent: {deleted_progress:?}"
                )
                .into());
            }

            Ok(())
        },
    )
    .await
    .unwrap_or_else(|error| panic!("Flex attached Translation PostgreSQL evidence failed: {error}"));
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Flex attached translation evidence".into(),
                format!("flex-attached-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

async fn create_category(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Uuid> {
    Ok(TaxonomyService::new(database.clone())
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Category,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Flex Translation Evidence".to_string(),
                slug: None,
                canonical_key: Some(format!("flex-translation-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?)
}

async fn seed_localized_definition_and_source(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<()> {
    GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE)
        .create(
            database,
            tenant_id,
            Some(Uuid::new_v4()),
            CreateFieldDefinitionCommand {
                field_key: FIELD_KEY.to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("en".to_string(), "Headline".to_string())]),
                description: None,
                is_localized: true,
                is_required: true,
                default_value: None,
                validation: None,
                position: None,
            },
        )
        .await?;
    persist_localized_values(
        database,
        tenant_id,
        TAXONOMY_CATEGORY_ENTITY_TYPE,
        category_id,
        "en",
        &json!({"headline": "Hello"}),
    )
    .await?;
    Ok(())
}

async fn apply_journal_migration(database: &DatabaseConnection) -> TestResult<()> {
    Migrator::up(database, Some(1)).await?;
    Ok(())
}

async fn assert_backfill_without_historical_change(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<()> {
    let revisions = load_attached_translation_resource_revisions(
        database,
        tenant_id,
        TAXONOMY_CATEGORY_ENTITY_TYPE,
        &[category_id],
    )
    .await?;
    if revisions.get(&category_id).map(String::as_str) != Some("attached:1") {
        return Err(format!("unexpected migration backfill revision: {revisions:?}").into());
    }
    let reader = FlexAttachedTranslationChangeReader::new(
        database.clone(),
        TAXONOMY_CATEGORY_ENTITY_TYPE,
    )?;
    if reader.read_change_highwater(tenant_id).await?.is_some() {
        return Err("migration backfill fabricated historical ChangeCursor evidence".into());
    }
    Ok(())
}

fn registered_provider(database: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(rustok_index::IndexModule);
    let settings = RustokSettings::default();
    let runtime = ServerRuntimeContext::new(database, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime,
        AuthConfig::new("test-secret-key-for-unit-tests-only-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or("host composition did not publish TranslationTargetRegistry")?;
    targets
        .get(
            &OwnerSlug::new("flex")?,
            &ResourceKind::new("attached_localized_value")?,
        )
        .ok_or_else(|| "registered flex/attached_localized_value provider is missing".into())
}

fn assert_registered_capabilities(provider: &dyn TranslationTargetProvider) -> TestResult<()> {
    let descriptor = provider.descriptor();
    if descriptor.owner_slug.as_str() != "flex"
        || descriptor.resource_kind.as_str() != "attached_localized_value"
        || !descriptor
            .capabilities
            .contains(&TranslationTargetCapability::AggregateProgress)
        || !descriptor
            .capabilities
            .contains(&TranslationTargetCapability::ChangeCursor)
    {
        return Err(format!("unexpected registered attached provider descriptor: {descriptor:?}").into());
    }
    Ok(())
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(30))
}

fn write_context(tenant_id: Uuid, key: &str) -> PortContext {
    read_context(tenant_id).with_idempotency_key(format!("{key}-{}", Uuid::new_v4()))
}

fn patch_for_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    value: &str,
    suffix: &str,
) -> TestResult<TranslationPatchRequest> {
    let field = snapshot.fields.first().ok_or("attached source field is missing")?;
    Ok(TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![TranslationFieldPatch {
            key: field.descriptor.key.clone(),
            value: value.to_string(),
            expected_source_hash: field_hash(&field.source_value),
        }],
        proposal_id: format!("flex-attached-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("flex-attached-postgres-approval-{suffix}"),
    })
}

async fn assert_revision_cas_matrix(
    provider: &dyn TranslationTargetProvider,
    tenant_id: Uuid,
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    value: &str,
) -> TestResult<()> {
    let mut resource = patch_for_snapshot(snapshot, value, "stale-resource")?;
    resource.expected_resource_revision = OpaqueRevision::new("attached:1")?;
    assert_revision_conflict(
        provider,
        write_context(tenant_id, "stale-resource"),
        resource,
        "resource",
    )
    .await?;

    let mut source = patch_for_snapshot(snapshot, value, "stale-source")?;
    source.expected_source_revision = OpaqueRevision::new("stale-source-revision")?;
    assert_revision_conflict(
        provider,
        write_context(tenant_id, "stale-source"),
        source,
        "source",
    )
    .await?;

    let mut target = patch_for_snapshot(snapshot, value, "stale-target")?;
    target.expected_target_revision = Some(OpaqueRevision::new("stale-target-revision")?);
    assert_revision_conflict(
        provider,
        write_context(tenant_id, "stale-target"),
        target,
        "target",
    )
    .await?;
    Ok(())
}

async fn assert_revision_conflict(
    provider: &dyn TranslationTargetProvider,
    context: PortContext,
    patch: TranslationPatchRequest,
    revision: &str,
) -> TestResult<()> {
    let error = provider
        .apply_patch(context, patch)
        .await
        .expect_err("stale attached revision must fail closed");
    if error.code != "flex.attached_translation_revision_conflict" {
        return Err(format!("unexpected {revision} CAS error: {error:?}").into());
    }
    Ok(())
}

async fn drain_changes_one_at_a_time(
    provider: &dyn TranslationTargetProvider,
    tenant_id: Uuid,
) -> TestResult<(
    Vec<rustok_translation_targets::TranslationTargetChange>,
    rustok_translation_targets::OpaqueCursor,
)> {
    let mut cursor = None;
    let mut changes = Vec::new();
    loop {
        let page = provider
            .read_changes(
                read_context(tenant_id),
                TranslationTargetChangesRequest {
                    after: cursor.clone(),
                    limit: 1,
                },
            )
            .await?;
        let next = page
            .next_cursor
            .ok_or("non-empty attached journal must retain a terminal cursor")?;
        if page.changes.is_empty() {
            return Ok((changes, next));
        }
        if page.changes.len() != 1 {
            return Err(format!("bounded attached page exceeded limit=1: {page:?}").into());
        }
        changes.extend(page.changes);
        cursor = Some(next);
    }
}

async fn with_flex_translation_postgres_database<T, F, Fut>(
    prefix: &str,
    test: F,
) -> TestResult<T>
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
        let journal_position = Migrator::migrations()
            .iter()
            .position(|migration| migration.name() == JOURNAL_MIGRATION)
            .ok_or("Flex attached Translation journal migration is missing")?;
        Migrator::up(&replica_a, Some(u32::try_from(journal_position)?)).await?;

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
