#![cfg(all(feature = "mod-taxonomy", feature = "mod-flex"))]

use std::{collections::HashMap, error::Error, io, sync::Arc, time::Duration};

use flex::graphql::AttachedValuesGraphqlPort;
use flex::{
    CreateFieldDefinitionCommand, FieldDefinitionService, FlexAttachedFieldPolicy,
    FlexDataClassification, FlexModule, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE, upsert_attached_field_policy,
};
use rustok_api::{PortActor, PortContext, TenantLocale};
use rustok_core::{ModuleRegistry, SecurityContext, UserRole, field_schema::FieldType};
use rustok_migrations::Migrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        flex_attached_values::FlexAttachedValuesGraphqlAdapter,
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
    TranslationDataClassification, TranslationResourceSnapshot, TranslationTargetProvider,
    translation_target_registry,
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
async fn taxonomy_category_flex_translation_policy_governance_evidence_postgres()
-> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_flex_attached_translation_policy");
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
    let category_id = create_category(&seed_connection, tenant_id).await?;
    create_localized_definition(&seed_connection, tenant_id).await?;

    let values = FlexAttachedValuesGraphqlAdapter::new(seed_connection.clone());
    let source = values
        .update_values(
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "en",
            Some(json!({"tagline": "Policy-governed category"})),
        )
        .await?
        .ok_or_else(|| test_error("source attached values were not persisted"))?;
    if source != json!({"tagline": "Policy-governed category"}) {
        return Err(test_error(format!("unexpected source attached values: {source}")).into());
    }

    let seed_provider = registered_provider(seed_connection.clone())?;
    let listed = seed_provider
        .list_resources(
            read_context(tenant_id, "default-list"),
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
            "registered Flex provider returned unexpected inventory page: {listed:?}"
        ))
        .into());
    }
    let identity = listed.resources[0].identity.clone();
    let read_request = ReadTranslationResourceRequest {
        identity,
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };

    let default_snapshot = seed_provider
        .read_resource(
            read_context(tenant_id, "default-policy-read"),
            read_request.clone(),
        )
        .await?;
    assert_policy(
        &default_snapshot,
        TranslationDataClassification::TenantPrivate,
        false,
        "missing policy must remain fail-closed",
    )?;

    upsert_attached_field_policy(
        &seed_connection,
        tenant_id,
        TAXONOMY_CATEGORY_ENTITY_TYPE,
        FIELD_KEY,
        FlexAttachedFieldPolicy {
            classification: FlexDataClassification::Public,
            ai_export_allowed: true,
        },
    )
    .await?;

    drop(seed_provider);
    drop(values);
    seed_connection.close().await?;

    let recovery_connection = connect_postgres(database_url).await?;
    let recovery_provider = registered_provider(recovery_connection.clone())?;
    let recovered_snapshot = recovery_provider
        .read_resource(
            read_context(tenant_id, "recovered-policy-read"),
            read_request,
        )
        .await?;
    assert_policy(
        &recovered_snapshot,
        TranslationDataClassification::Public,
        true,
        "persisted explicit policy must survive a fresh host composition",
    )?;

    drop(recovery_provider);
    recovery_connection.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn assert_policy(
    snapshot: &TranslationResourceSnapshot,
    classification: TranslationDataClassification,
    ai_export_allowed: bool,
    expectation: &str,
) -> TestResult<()> {
    if snapshot.fields.len() != 1 || snapshot.fields[0].descriptor.key.as_str() != FIELD_KEY {
        return Err(test_error(format!(
            "{expectation}: unexpected translated field set: {:?}",
            snapshot.fields
        ))
        .into());
    }
    let descriptor = &snapshot.fields[0].descriptor;
    if descriptor.classification != classification
        || descriptor.ai_export_allowed != ai_export_allowed
    {
        return Err(test_error(format!(
            "{expectation}: unexpected policy projection: {descriptor:?}"
        ))
        .into());
    }
    Ok(())
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
        AuthConfig::new("test-secret-key-for-flex-translation-policy-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register flex/attached_localized_value").into()
    })
}

async fn seed_tenant(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
            vec![
                tenant_id.into(),
                "Flex attached translation policy".into(),
                format!("flex-attached-policy-{}", tenant_id.simple()).into(),
            ],
        ))
        .await?;
    Ok(())
}

async fn create_category(database: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Uuid> {
    let category = TaxonomyService::new(database.clone())
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Category,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: "Translation Policy Category".to_string(),
                slug: None,
                canonical_key: Some(format!("translation-policy-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?;
    Ok(category)
}

async fn create_localized_definition(
    database: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<()> {
    GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE)
        .create(
            database,
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
    Ok(())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("flex-attached-policy-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
