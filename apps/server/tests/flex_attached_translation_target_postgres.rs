#![cfg(all(feature = "mod-taxonomy", feature = "mod-flex"))]

use std::{collections::BTreeSet, future::Future, sync::Arc, time::Duration};

use flex::graphql::AttachedValuesGraphqlPort;
use flex::{
    CreateFieldDefinitionCommand, FieldDefinitionService, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE,
};
use rustok_api::{PortActor, PortContext, PortErrorKind, TenantLocale};
use rustok_auth::AuthConfig;
use rustok_core::{
    ModuleRegistry, SecurityContext, UserRole,
    field_schema::FieldType,
};
use rustok_migrations::Migrator;
use rustok_server::{
    common::settings::RustokSettings,
    services::{
        flex_attached_values::FlexAttachedValuesGraphqlAdapter,
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
    FieldKey, ListTranslationResourcesRequest, OwnerSlug, ReadTranslationResourceRequest,
    ResourceKind, TranslationFieldPatch, TranslationPatchRequest, TranslationResourceLifecycle,
    TranslationTargetCapability, TranslationTargetChangesRequest, TranslationTargetProgressRequest,
    TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "attached_localized_value";
const FIELD_KEY: &str = "tagline";

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn flex_attached_provider_multi_replica_cas_replay_progress_and_cursor_recovery_postgres() {
    with_postgres_database(
        "rustok_flex_attached_translation_evidence",
        |replica_a, replica_b, recovery| async move {
            let tenant_id = Uuid::new_v4();
            seed_tenant(&replica_a, tenant_id).await?;

            let taxonomy = TaxonomyService::new(replica_a.clone());
            let category_id = create_category(&taxonomy, tenant_id).await?;
            GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE)
                .create(
                    &replica_a,
                    tenant_id,
                    Some(Uuid::new_v4()),
                    required_localized_text_definition(),
                )
                .await?;

            let authored = FlexAttachedValuesGraphqlAdapter::new(replica_a.clone())
                .update_values(
                    tenant_id,
                    TAXONOMY_CATEGORY_ENTITY_TYPE,
                    category_id,
                    "en",
                    Some(json!({"tagline": "Welcome"})),
                )
                .await?
                .ok_or("English attached source values were not persisted")?;
            if authored != json!({"tagline": "Welcome"}) {
                return Err(format!("unexpected authored attached source: {authored}").into());
            }

            let provider_a = production_provider(replica_a.clone())?;
            let provider_b = production_provider(replica_b.clone())?;
            assert_provider_descriptor(provider_a.as_ref())?;

            let source_locale = TenantLocale::new("en")?;
            let target_locale = TenantLocale::new("fr")?;
            let listed = provider_a
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
            if listed.resources.len() != 1 || listed.next_cursor.is_some() {
                return Err(format!("unexpected attached provider inventory: {listed:?}").into());
            }
            let identity = listed.resources[0].identity.clone();
            let expected_resource_id = category_id.to_string();
            if identity.resource_id.as_str() != expected_resource_id
                || identity.subresource_id.as_ref().map(|value| value.as_str())
                    != Some(TAXONOMY_CATEGORY_ENTITY_TYPE)
            {
                return Err(format!("unexpected attached provider identity: {identity:?}").into());
            }

            let read_request = ReadTranslationResourceRequest {
                identity: identity.clone(),
                source_locale: source_locale.clone(),
                target_locale: target_locale.clone(),
            };
            let first_snapshot = provider_a
                .read_resource(read_context(tenant_id), read_request.clone())
                .await?;
            let second_snapshot = provider_b
                .read_resource(read_context(tenant_id), read_request.clone())
                .await?;
            if first_snapshot != second_snapshot {
                return Err(format!(
                    "independent provider replicas observed different attached snapshots: first={first_snapshot:?}, second={second_snapshot:?}"
                )
                .into());
            }
            let source_field = first_snapshot
                .fields
                .iter()
                .find(|field| field.descriptor.key.as_str() == FIELD_KEY)
                .ok_or("attached source field is missing")?;
            if first_snapshot.fields.len() != 1
                || source_field.source_value != "Welcome"
                || source_field.exact_target_value.is_some()
                || first_snapshot.target_revision.is_some()
            {
                return Err(format!(
                    "unexpected untranslated attached snapshot: {first_snapshot:?}"
                )
                .into());
            }

            let baseline_changes = provider_a
                .read_changes(
                    read_context(tenant_id),
                    TranslationTargetChangesRequest {
                        after: None,
                        limit: 10,
                    },
                )
                .await?;
            if baseline_changes.changes.len() != 1
                || baseline_changes.changes[0].identity != identity
                || baseline_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
            {
                return Err(format!(
                    "unexpected attached source change page: {baseline_changes:?}"
                )
                .into());
            }
            let baseline_cursor = baseline_changes
                .next_cursor
                .clone()
                .ok_or("attached baseline ChangeCursor is missing")?;

            let baseline_progress = provider_a
                .read_progress(
                    read_context(tenant_id),
                    TranslationTargetProgressRequest {
                        source_locale: source_locale.clone(),
                        target_locale: target_locale.clone(),
                    },
                )
                .await?;
            if baseline_progress.required_units != 1
                || baseline_progress.exact_required_units != 0
                || baseline_progress.optional_units != 0
                || baseline_progress.exact_optional_units != 0
                || baseline_progress.resources != 1
                || baseline_progress.complete_resources != 0
                || baseline_progress.owner_change_cursor.as_ref() != Some(&baseline_cursor)
            {
                return Err(format!(
                    "unexpected untranslated attached progress: {baseline_progress:?}"
                )
                .into());
            }

            let left_patch = patch_for_snapshot(&first_snapshot, "Bienvenue A", "a")?;
            let right_patch = patch_for_snapshot(&second_snapshot, "Bienvenue B", "b")?;
            let left_validation = provider_a
                .validate_patch(read_context(tenant_id), left_patch.clone())
                .await?;
            let right_validation = provider_b
                .validate_patch(read_context(tenant_id), right_patch.clone())
                .await?;
            if !left_validation.accepted || !right_validation.accepted {
                return Err(format!(
                    "fresh attached patches must both validate before the CAS race: left={left_validation:?}, right={right_validation:?}"
                )
                .into());
            }

            let left_context = write_context(tenant_id, Uuid::new_v4());
            let right_context = write_context(tenant_id, Uuid::new_v4());
            let left = provider_a.apply_patch(left_context.clone(), left_patch.clone());
            let right = provider_b.apply_patch(right_context.clone(), right_patch.clone());
            let (left, right) = tokio::join!(left, right);

            let (winner_context, winner_patch, winner_receipt, loser_error) = match (left, right) {
                (Ok(receipt), Err(error)) => (left_context, left_patch, receipt, error),
                (Err(error), Ok(receipt)) => (right_context, right_patch, receipt, error),
                other => {
                    return Err(format!(
                        "exactly one concurrent attached Translation apply must win: {other:?}"
                    )
                    .into());
                }
            };
            if loser_error.kind != PortErrorKind::Conflict
                || loser_error.code != "flex.attached_translation_revision_conflict"
            {
                return Err(format!(
                    "concurrent attached Translation loser returned an unexpected error: {loser_error:?}"
                )
                .into());
            }
            let winner_value = winner_patch
                .fields
                .first()
                .ok_or("winning attached patch has no field")?
                .value
                .clone();

            let recovery_provider = production_provider(recovery.clone())?;
            let recovered = recovery_provider
                .read_resource(read_context(tenant_id), read_request.clone())
                .await?;
            let recovered_field = recovered
                .fields
                .iter()
                .find(|field| field.descriptor.key.as_str() == FIELD_KEY)
                .ok_or("recovered attached field is missing")?;
            if recovered_field.exact_target_value.as_deref() != Some(winner_value.as_str())
                || recovered.summary.resource_revision != winner_receipt.resource_revision
                || recovered.target_revision.as_ref() != Some(&winner_receipt.target_revision)
            {
                return Err(format!(
                    "fresh provider replica did not recover the winning attached target: snapshot={recovered:?}, receipt={winner_receipt:?}"
                )
                .into());
            }

            let replay = recovery_provider
                .apply_patch(winner_context, winner_patch.clone())
                .await?;
            if replay != winner_receipt {
                return Err(format!(
                    "fresh provider replica did not replay the exact owner receipt: replay={replay:?}, winner={winner_receipt:?}"
                )
                .into());
            }

            let target_changes = recovery_provider
                .read_changes(
                    read_context(tenant_id),
                    TranslationTargetChangesRequest {
                        after: Some(baseline_cursor),
                        limit: 10,
                    },
                )
                .await?;
            if target_changes.changes.len() != 1
                || target_changes.changes[0].identity != winner_patch.identity
                || target_changes.changes[0].resource_revision != winner_receipt.resource_revision
                || target_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
            {
                return Err(format!(
                    "unexpected attached target change page: {target_changes:?}"
                )
                .into());
            }
            let target_cursor = target_changes
                .next_cursor
                .clone()
                .ok_or("attached target ChangeCursor is missing")?;

            let resumed = recovery_provider
                .read_changes(
                    read_context(tenant_id),
                    TranslationTargetChangesRequest {
                        after: Some(target_cursor.clone()),
                        limit: 10,
                    },
                )
                .await?;
            if !resumed.changes.is_empty()
                || resumed.next_cursor.as_ref() != Some(&target_cursor)
            {
                return Err(format!(
                    "attached ChangeCursor recovery redelivered committed work: {resumed:?}"
                )
                .into());
            }

            let recovered_progress = recovery_provider
                .read_progress(
                    read_context(tenant_id),
                    TranslationTargetProgressRequest {
                        source_locale,
                        target_locale,
                    },
                )
                .await?;
            if recovered_progress.required_units != 1
                || recovered_progress.exact_required_units != 1
                || recovered_progress.optional_units != 0
                || recovered_progress.exact_optional_units != 0
                || recovered_progress.resources != 1
                || recovered_progress.complete_resources != 1
                || recovered_progress.owner_change_cursor.as_ref() != Some(&target_cursor)
            {
                return Err(format!(
                    "recovered attached progress is inconsistent with the committed cursor: {recovered_progress:?}"
                )
                .into());
            }

            Ok(())
        },
    )
    .await
    .unwrap_or_else(|error| panic!("Flex attached Translation PostgreSQL evidence failed: {error}"));
}

fn assert_provider_descriptor(provider: &dyn TranslationTargetProvider) -> TestResult<()> {
    let descriptor = provider.descriptor();
    let expected = BTreeSet::from([
        TranslationTargetCapability::ListResources,
        TranslationTargetCapability::ReadExactResource,
        TranslationTargetCapability::ValidatePatch,
        TranslationTargetCapability::ApplyPatch,
        TranslationTargetCapability::AggregateProgress,
        TranslationTargetCapability::ChangeCursor,
    ]);
    if descriptor.owner_slug.as_str() != OWNER_SLUG
        || descriptor.resource_kind.as_str() != RESOURCE_KIND
        || descriptor.capabilities != expected
    {
        return Err(format!("unexpected registered attached provider descriptor: {descriptor:?}").into());
    }
    Ok(())
}

fn production_provider(database: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new().register(rustok_index::IndexModule);
    let settings = RustokSettings::default();
    let runtime = ServerRuntimeContext::new(database, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime,
        AuthConfig::new("test-secret-key-for-flex-translation-evidence-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(extensions.as_ref())
        .ok_or("production composition did not publish a Translation target registry")?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets
        .get(&owner_slug, &resource_kind)
        .ok_or_else(|| "production composition did not register flex/attached_localized_value".into())
}

fn required_localized_text_definition() -> CreateFieldDefinitionCommand {
    CreateFieldDefinitionCommand {
        field_key: FIELD_KEY.to_string(),
        field_type: FieldType::Text,
        label: [("en".to_string(), "Tagline".to_string())]
            .into_iter()
            .collect(),
        description: None,
        is_localized: true,
        is_required: true,
        default_value: None,
        validation: None,
        position: None,
    }
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_category(service: &TaxonomyService, tenant_id: Uuid) -> TestResult<Uuid> {
    Ok(service
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
                canonical_key: Some(format!("flex-translation-evidence-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?)
}

fn patch_for_snapshot(
    snapshot: &rustok_translation_targets::TranslationResourceSnapshot,
    value: &str,
    suffix: &str,
) -> TestResult<TranslationPatchRequest> {
    let field = snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == FIELD_KEY)
        .ok_or("attached source field is missing")?;
    Ok(TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: snapshot.summary.resource_revision.clone(),
        expected_source_revision: snapshot.source_revision.clone(),
        expected_target_revision: snapshot.target_revision.clone(),
        fields: vec![TranslationFieldPatch {
            key: FieldKey::new(FIELD_KEY)?,
            value: value.to_string(),
            expected_source_hash: field.source_hash.clone(),
        }],
        proposal_id: format!("flex-attached-postgres-proposal-{suffix}"),
        approval_receipt_id: format!("flex-attached-postgres-approval-{suffix}"),
    })
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

fn write_context(tenant_id: Uuid, idempotency_key: Uuid) -> PortContext {
    read_context(tenant_id).with_idempotency_key(idempotency_key.to_string())
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Flex attached Translation evidence".into(),
                format!("flex-attached-translation-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

async fn with_postgres_database<T, F, Fut>(prefix: &str, test: F) -> TestResult<T>
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
