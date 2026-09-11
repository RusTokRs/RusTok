#![cfg(all(feature = "mod-taxonomy", feature = "mod-flex"))]

use std::{collections::HashMap, error::Error, io, sync::Arc, time::Duration};

use flex::graphql::AttachedValuesGraphqlPort;
use flex::{
    CreateFieldDefinitionCommand, FieldDefinitionService, FlexModule,
    GenericAttachedFieldDefinitionService, TAXONOMY_CATEGORY_ENTITY_TYPE,
    UpdateFieldDefinitionCommand,
};
use rustok_api::{PortActor, PortContext, TenantLocale};
use rustok_core::{ModuleRegistry, SecurityContext, UserRole, field_schema::FieldType};
use rustok_migrations::Migrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        flex_attached_values::{FlexAttachedValuesGraphqlAdapter, FlexTaxonomyCategoryDeleteCleanup},
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        server_runtime_context::ServerRuntimeContext,
    },
};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use rustok_translation_targets::{
    ListTranslationResourcesRequest, OwnerSlug, ReadTranslationResourceRequest, ResourceKind,
    TranslationResourceLifecycle, TranslationTargetChangesRequest, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "attached_localized_value";
const FIELD_KEY: &str = "tagline";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn taxonomy_category_flex_attached_lifecycle_evidence_postgres() -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_flex_attached_lifecycle_evidence");
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
    let db = connect_postgres(database_url).await?;
    Migrator::up(&db, None).await?;

    let tenant_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await?;
    let category_id = create_category(&db, tenant_id).await?;
    let definition_id = create_localized_definition(&db, tenant_id).await?;

    let values = FlexAttachedValuesGraphqlAdapter::new(db.clone());
    values
        .update_values(
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "en",
            Some(json!({"tagline": "Lifecycle category"})),
        )
        .await?
        .ok_or_else(|| test_error("source attached values were not persisted"))?;

    let provider = registered_provider(db.clone())?;
    let identity = provider_identity(provider.as_ref(), tenant_id, category_id).await?;
    let read_request = ReadTranslationResourceRequest {
        identity: identity.clone(),
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let before_schema = provider
        .read_resource(
            read_context(tenant_id, "before-schema-read"),
            read_request.clone(),
        )
        .await?;
    if before_schema.fields.len() != 1
        || before_schema.fields[0].descriptor.key.as_str() != FIELD_KEY
        || !before_schema.fields[0].descriptor.required
        || before_schema.fields[0].source_value != "Lifecycle category"
        || before_schema.fields[0].exact_target_value.is_some()
    {
        return Err(test_error(format!(
            "unexpected attached snapshot before schema fan-out: {before_schema:?}"
        ))
        .into());
    }

    let baseline_changes = provider
        .read_changes(
            read_context(tenant_id, "baseline-cursor"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 100,
            },
        )
        .await?;
    if baseline_changes.changes.is_empty() {
        return Err(test_error("source attached write did not publish a ChangeCursor").into());
    }
    let baseline_cursor = baseline_changes
        .next_cursor
        .ok_or_else(|| test_error("baseline attached ChangeCursor is missing"))?;
    assert_progress(
        &provider
            .read_progress(
                read_context(tenant_id, "before-schema-progress"),
                progress_request()?,
            )
            .await?,
        1,
        0,
        0,
        0,
        1,
        0,
        Some(&baseline_cursor),
        "before schema fan-out",
    )?;

    let definition_service =
        GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE);
    let (updated_definition, _) = definition_service
        .update(
            &db,
            tenant_id,
            Some(Uuid::new_v4()),
            definition_id,
            UpdateFieldDefinitionCommand {
                is_required: Some(false),
                ..UpdateFieldDefinitionCommand::default()
            },
        )
        .await?;
    if updated_definition.is_required {
        return Err(test_error("attached field-definition update did not become optional").into());
    }

    let after_schema = provider
        .read_resource(
            read_context(tenant_id, "after-schema-read"),
            read_request.clone(),
        )
        .await?;
    if after_schema.summary.resource_revision == before_schema.summary.resource_revision
        || after_schema.fields.len() != 1
        || after_schema.fields[0].descriptor.required
        || after_schema.fields[0].source_value != "Lifecycle category"
    {
        return Err(test_error(format!(
            "schema update did not fan out into the registered attached provider: before={before_schema:?}, after={after_schema:?}"
        ))
        .into());
    }

    let schema_changes = provider
        .read_changes(
            read_context(tenant_id, "schema-cursor"),
            TranslationTargetChangesRequest {
                after: Some(baseline_cursor),
                limit: 100,
            },
        )
        .await?;
    if schema_changes.changes.len() != 1
        || schema_changes.changes[0].identity != identity
        || schema_changes.changes[0].resource_revision != after_schema.summary.resource_revision
        || schema_changes.changes[0].lifecycle != TranslationResourceLifecycle::Active
    {
        return Err(test_error(format!(
            "schema fan-out did not publish one active attached change: {schema_changes:?}"
        ))
        .into());
    }
    let schema_cursor = schema_changes
        .next_cursor
        .ok_or_else(|| test_error("schema fan-out ChangeCursor is missing"))?;
    assert_progress(
        &provider
            .read_progress(
                read_context(tenant_id, "after-schema-progress"),
                progress_request()?,
            )
            .await?,
        0,
        0,
        1,
        0,
        1,
        1,
        Some(&schema_cursor),
        "after schema fan-out",
    )?;

    TaxonomyService::new(db.clone())
        .delete_category_with_cleanup(
            tenant_id,
            category_id,
            admin(),
            &FlexTaxonomyCategoryDeleteCleanup,
        )
        .await?;

    let deleted_changes = provider
        .read_changes(
            read_context(tenant_id, "delete-cursor"),
            TranslationTargetChangesRequest {
                after: Some(schema_cursor.clone()),
                limit: 100,
            },
        )
        .await?;
    let deleted_revision = format!("deleted:{TAXONOMY_CATEGORY_ENTITY_TYPE}:{category_id}");
    if deleted_changes.changes.len() != 1
        || deleted_changes.changes[0].identity != identity
        || deleted_changes.changes[0].resource_revision.as_str() != deleted_revision.as_str()
        || deleted_changes.changes[0].lifecycle != TranslationResourceLifecycle::Deleted
    {
        return Err(test_error(format!(
            "Category hard-delete did not publish one final attached tombstone: {deleted_changes:?}"
        ))
        .into());
    }
    let deleted_cursor = deleted_changes
        .next_cursor
        .ok_or_else(|| test_error("hard-delete ChangeCursor is missing"))?;

    let inventory_after_delete = provider
        .list_resources(
            read_context(tenant_id, "after-delete-inventory"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    if !inventory_after_delete.resources.is_empty() {
        return Err(test_error(format!(
            "deleted Category remained in attached inventory: {inventory_after_delete:?}"
        ))
        .into());
    }

    drop(provider);
    drop(values);
    db.close().await?;

    let recovery_db = connect_postgres(database_url).await?;
    let recovery_provider = registered_provider(recovery_db.clone())?;
    let recovered_deleted = recovery_provider
        .read_changes(
            read_context(tenant_id, "recovery-delete-cursor"),
            TranslationTargetChangesRequest {
                after: Some(schema_cursor),
                limit: 100,
            },
        )
        .await?;
    if recovered_deleted.changes != deleted_changes.changes
        || recovered_deleted.next_cursor.as_ref() != Some(&deleted_cursor)
    {
        return Err(test_error(format!(
            "fresh provider did not recover the exact attached delete tombstone: expected={deleted_changes:?}, recovered={recovered_deleted:?}"
        ))
        .into());
    }

    let resumed = recovery_provider
        .read_changes(
            read_context(tenant_id, "recovery-resume"),
            TranslationTargetChangesRequest {
                after: Some(deleted_cursor.clone()),
                limit: 100,
            },
        )
        .await?;
    if !resumed.changes.is_empty() || resumed.next_cursor.as_ref() != Some(&deleted_cursor) {
        return Err(test_error(format!(
            "attached delete ChangeCursor recovery redelivered work: {resumed:?}"
        ))
        .into());
    }

    assert_progress(
        &recovery_provider
            .read_progress(
                read_context(tenant_id, "after-delete-progress"),
                progress_request()?,
            )
            .await?,
        0,
        0,
        0,
        0,
        0,
        0,
        Some(&deleted_cursor),
        "after hard-delete recovery",
    )?;

    drop(recovery_provider);
    recovery_db.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn assert_progress(
    actual: &TranslationTargetProgressFacts,
    required_units: u64,
    exact_required_units: u64,
    optional_units: u64,
    exact_optional_units: u64,
    resources: u64,
    complete_resources: u64,
    owner_change_cursor: Option<&rustok_translation_targets::OpaqueCursor>,
    phase: &str,
) -> TestResult<()> {
    if actual.required_units != required_units
        || actual.exact_required_units != exact_required_units
        || actual.optional_units != optional_units
        || actual.exact_optional_units != exact_optional_units
        || actual.resources != resources
        || actual.complete_resources != complete_resources
        || actual.owner_change_cursor.as_ref() != owner_change_cursor
    {
        return Err(test_error(format!(
            "unexpected attached aggregate progress {phase}: {actual:?}"
        ))
        .into());
    }
    Ok(())
}

fn progress_request() -> TestResult<TranslationTargetProgressRequest> {
    Ok(TranslationTargetProgressRequest {
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    })
}

fn registered_provider(db: DatabaseConnection) -> TestResult<Arc<dyn TranslationTargetProvider>> {
    let registry = ModuleRegistry::new()
        .register(FlexModule)
        .register(TaxonomyModule);
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(db, settings.clone());
    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("test-secret-key-for-flex-lifecycle-evidence-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register flex/attached_localized_value").into()
    })
}

async fn provider_identity(
    provider: &dyn TranslationTargetProvider,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TestResult<rustok_translation_targets::TranslationResourceIdentity> {
    let page = provider
        .list_resources(
            read_context(tenant_id, "inventory"),
            ListTranslationResourcesRequest {
                source_locale: TenantLocale::new("en")?,
                target_locale: TenantLocale::new("fr")?,
                cursor: None,
                limit: 10,
            },
        )
        .await?;
    let category_id = category_id.to_string();
    let resource = page
        .resources
        .into_iter()
        .find(|resource| resource.identity.resource_id.as_str() == category_id)
        .ok_or_else(|| test_error("registered provider did not list the seeded category"))?;
    Ok(resource.identity)
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
        vec![
            tenant_id.into(),
            "Flex attached lifecycle evidence".into(),
            format!("flex-attached-lifecycle-{}", tenant_id.simple()).into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn create_category(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Uuid> {
    let category = TaxonomyService::new(db.clone())
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Category,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Lifecycle Evidence Category".to_string(),
                slug: None,
                canonical_key: Some(format!("lifecycle-evidence-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?;
    Ok(category)
}

async fn create_localized_definition(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    let (definition, _) = GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE)
        .create(
            db,
            tenant_id,
            Some(Uuid::new_v4()),
            CreateFieldDefinitionCommand {
                field_key: FIELD_KEY.to_string(),
                field_type: FieldType::Text,
                label: HashMap::from([("en".to_string(), "Tagline".to_string())]),
                description: None,
                is_localized: true,
                is_required: true,
                default_value: None,
                validation: None,
                position: None,
            },
        )
        .await?;
    Ok(definition.id)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("flex-attached-lifecycle-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
