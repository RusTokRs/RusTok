#![cfg(all(feature = "mod-taxonomy", feature = "mod-flex"))]
#![recursion_limit = "512"]

use std::{collections::HashMap, error::Error, io, sync::Arc, time::Duration};

use async_graphql::{EmptySubscription, Request, Response, Schema};
use async_trait::async_trait;
use flex::graphql::{AttachedValuesGraphqlPort, FlexGraphqlRuntime, FlexMutation, FlexQuery};
use flex::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionCachePort,
    FieldDefinitionService, FieldDefinitionView, FlexModule, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE,
};
use rustok_api::{AuthContext, Permission, PortActor, PortContext, TenantContext, TenantLocale};
use rustok_core::{ModuleRegistry, SecurityContext, UserRole, field_schema::FieldType};
use rustok_migrations::Migrator;
use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    services::{
        flex_attached_values::FlexAttachedValuesGraphqlAdapter,
        flex_standalone_service::FlexStandaloneSeaOrmService,
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
    ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug, ReadTranslationResourceRequest,
    ResourceKind, TranslationDataClassification, TranslationResourceSnapshot,
    TranslationTargetChangesRequest, TranslationTargetProvider, translation_target_registry,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use serde_json::{Value, json};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const OWNER_SLUG: &str = "flex";
const RESOURCE_KIND: &str = "attached_localized_value";
const FIELD_KEY: &str = "tagline";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
type FlexPolicySchema = Schema<FlexQuery, FlexMutation, EmptySubscription>;

#[derive(Default)]
struct UncachedFieldDefinitions;

#[async_trait]
impl FieldDefinitionCachePort for UncachedFieldDefinitions {
    async fn get(&self, _tenant_id: Uuid, _entity_type: &str) -> Option<Vec<FieldDefinitionView>> {
        None
    }

    async fn set(&self, _tenant_id: Uuid, _entity_type: &str, _rows: Vec<FieldDefinitionView>) {}

    async fn invalidate(&self, _tenant_id: Uuid, _entity_type: &str) {}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access"]
async fn taxonomy_category_flex_attached_policy_rbac_evidence_postgres() -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_flex_attached_policy_rbac_evidence");
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
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id).await?;
    let category_id = create_category(&db, tenant_id).await?;
    create_localized_definition(&db, tenant_id).await?;

    let values = FlexAttachedValuesGraphqlAdapter::new(db.clone());
    values
        .update_values(
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "en",
            Some(json!({"tagline": "Governed category"})),
        )
        .await?
        .ok_or_else(|| test_error("source attached values were not persisted"))?;

    let provider = registered_provider(db.clone())?;
    let identity = provider_identity(provider.as_ref(), tenant_id, category_id).await?;
    let read_request = ReadTranslationResourceRequest {
        identity,
        source_locale: TenantLocale::new("en")?,
        target_locale: TenantLocale::new("fr")?,
    };
    let default_snapshot = provider
        .read_resource(
            read_context(tenant_id, "default-provider-read"),
            read_request.clone(),
        )
        .await?;
    assert_provider_policy(
        &default_snapshot,
        TranslationDataClassification::TenantPrivate,
        false,
    )?;
    let content_revision = default_snapshot.summary.resource_revision.clone();
    let source_revision = default_snapshot.source_revision.clone();
    let target_revision = default_snapshot.target_revision.clone();

    let baseline_changes = provider
        .read_changes(
            read_context(tenant_id, "baseline-cursor"),
            TranslationTargetChangesRequest {
                after: None,
                limit: 100,
            },
        )
        .await?;
    let baseline_cursor = baseline_changes
        .next_cursor
        .ok_or_else(|| test_error("source attached write did not publish a ChangeCursor"))?;

    let schema = graphql_schema(db.clone());
    let default_policy = successful_graphql(
        execute_graphql(
            &schema,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_LIST],
            policy_query(),
        )
        .await,
        "default policy query",
    )?;
    assert_policy_object(
        &default_policy["attachedFieldPolicies"][0],
        "TENANT_PRIVATE",
        false,
        false,
    )?;

    let query_without_list = execute_graphql(
        &schema,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        policy_query(),
    )
    .await;
    assert_graphql_error_code(&query_without_list, "PERMISSION_DENIED")?;

    let denied_set = execute_graphql(
        &schema,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_LIST],
        set_policy_mutation("PUBLIC", true, FIELD_KEY),
    )
    .await;
    assert_graphql_error_code(&denied_set, "PERMISSION_DENIED")?;
    let still_default = provider
        .read_resource(
            read_context(tenant_id, "denied-provider-read"),
            read_request.clone(),
        )
        .await?;
    assert_provider_policy(
        &still_default,
        TranslationDataClassification::TenantPrivate,
        false,
    )?;

    let set_public = successful_graphql(
        execute_graphql(
            &schema,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_UPDATE],
            set_policy_mutation("PUBLIC", true, FIELD_KEY),
        )
        .await,
        "set public policy mutation",
    )?;
    assert_policy_object(&set_public["setAttachedFieldPolicy"], "PUBLIC", true, true)?;

    let explicit_query = successful_graphql(
        execute_graphql(
            &schema,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_LIST],
            policy_query(),
        )
        .await,
        "explicit policy query",
    )?;
    assert_policy_object(
        &explicit_query["attachedFieldPolicies"][0],
        "PUBLIC",
        true,
        true,
    )?;

    let public_snapshot = provider
        .read_resource(
            read_context(tenant_id, "public-provider-read"),
            read_request.clone(),
        )
        .await?;
    assert_provider_policy(
        &public_snapshot,
        TranslationDataClassification::Public,
        true,
    )?;
    assert_content_revisions_unchanged(
        &public_snapshot,
        &content_revision,
        &source_revision,
        &target_revision,
    )?;

    let unknown_field = execute_graphql(
        &schema,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        set_policy_mutation("PUBLIC", true, "not_registered"),
    )
    .await;
    assert_graphql_error_code(&unknown_field, "BAD_USER_INPUT")?;

    let invalid_secret = execute_graphql(
        &schema,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        set_policy_mutation("SECRET", true, FIELD_KEY),
    )
    .await;
    assert_graphql_error_code(&invalid_secret, "BAD_USER_INPUT")?;
    let after_invalid = provider
        .read_resource(
            read_context(tenant_id, "invalid-policy-provider-read"),
            read_request.clone(),
        )
        .await?;
    assert_provider_policy(&after_invalid, TranslationDataClassification::Public, true)?;

    let reset = successful_graphql(
        execute_graphql(
            &schema,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_UPDATE],
            reset_policy_mutation(),
        )
        .await,
        "reset policy mutation",
    )?;
    assert_policy_object(
        &reset["resetAttachedFieldPolicy"],
        "TENANT_PRIVATE",
        false,
        false,
    )?;

    let reset_query = successful_graphql(
        execute_graphql(
            &schema,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_LIST],
            policy_query(),
        )
        .await,
        "reset policy query",
    )?;
    assert_policy_object(
        &reset_query["attachedFieldPolicies"][0],
        "TENANT_PRIVATE",
        false,
        false,
    )?;

    let reset_snapshot = provider
        .read_resource(read_context(tenant_id, "reset-provider-read"), read_request)
        .await?;
    assert_provider_policy(
        &reset_snapshot,
        TranslationDataClassification::TenantPrivate,
        false,
    )?;
    assert_content_revisions_unchanged(
        &reset_snapshot,
        &content_revision,
        &source_revision,
        &target_revision,
    )?;

    let post_policy_changes = provider
        .read_changes(
            read_context(tenant_id, "post-policy-cursor"),
            TranslationTargetChangesRequest {
                after: Some(baseline_cursor),
                limit: 100,
            },
        )
        .await?;
    if !post_policy_changes.changes.is_empty() {
        return Err(test_error(format!(
            "policy governance manufactured Translation content changes: {post_policy_changes:?}"
        ))
        .into());
    }

    drop(provider);
    db.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn graphql_schema(db: DatabaseConnection) -> FlexPolicySchema {
    let mut field_registry = FieldDefRegistry::new();
    field_registry.register(Arc::new(GenericAttachedFieldDefinitionService::new(
        TAXONOMY_CATEGORY_ENTITY_TYPE,
    )));
    let runtime = FlexGraphqlRuntime::new(
        Arc::new(FlexStandaloneSeaOrmService::new(db.clone())),
        db.clone(),
        field_registry,
        Arc::new(UncachedFieldDefinitions),
        Arc::new(FlexAttachedValuesGraphqlAdapter::new(db)),
    );

    Schema::build(
        FlexQuery::default(),
        FlexMutation::default(),
        EmptySubscription,
    )
    .data(runtime)
    .finish()
}

async fn execute_graphql(
    schema: &FlexPolicySchema,
    tenant_id: Uuid,
    actor_id: Uuid,
    permissions: &[Permission],
    document: String,
) -> Response {
    schema
        .execute(
            Request::new(document)
                .data(tenant_context(tenant_id))
                .data(auth_context(tenant_id, actor_id, permissions)),
        )
        .await
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Flex attached policy RBAC evidence".to_string(),
        slug: format!("flex-attached-policy-{}", tenant_id.simple()),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn auth_context(tenant_id: Uuid, user_id: Uuid, permissions: &[Permission]) -> AuthContext {
    AuthContext {
        user_id,
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions: permissions.iter().map(ToString::to_string).collect(),
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

fn policy_query() -> String {
    format!(
        r#"query {{
  attachedFieldPolicies(entityType: "{TAXONOMY_CATEGORY_ENTITY_TYPE}") {{
    entityType
    fieldKey
    classification
    aiExportAllowed
    explicit
  }}
}}"#
    )
}

fn set_policy_mutation(classification: &str, ai_export_allowed: bool, field_key: &str) -> String {
    format!(
        r#"mutation {{
  setAttachedFieldPolicy(input: {{
    entityType: "{TAXONOMY_CATEGORY_ENTITY_TYPE}",
    fieldKey: "{field_key}",
    classification: {classification},
    aiExportAllowed: {ai_export_allowed}
  }}) {{
    entityType
    fieldKey
    classification
    aiExportAllowed
    explicit
  }}
}}"#
    )
}

fn reset_policy_mutation() -> String {
    format!(
        r#"mutation {{
  resetAttachedFieldPolicy(input: {{
    entityType: "{TAXONOMY_CATEGORY_ENTITY_TYPE}",
    fieldKey: "{FIELD_KEY}"
  }}) {{
    entityType
    fieldKey
    classification
    aiExportAllowed
    explicit
  }}
}}"#
    )
}

fn successful_graphql(response: Response, operation: &str) -> TestResult<Value> {
    if !response.errors.is_empty() {
        return Err(test_error(format!(
            "{operation} failed unexpectedly: {:?}",
            response.errors
        ))
        .into());
    }
    response
        .data
        .into_json()
        .map_err(|error| {
            test_error(format!(
                "{operation} response did not convert to JSON: {error}"
            ))
        })
        .map_err(Into::into)
}

fn assert_graphql_error_code(response: &Response, expected: &str) -> TestResult<()> {
    let Some(error) = response.errors.first() else {
        return Err(test_error(format!(
            "expected GraphQL error code {expected}, request succeeded"
        ))
        .into());
    };
    let actual = error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("code"))
        .cloned()
        .and_then(|value| value.into_json().ok())
        .and_then(|value| value.as_str().map(ToOwned::to_owned));
    if actual.as_deref() != Some(expected) {
        return Err(test_error(format!(
            "unexpected GraphQL error code: expected {expected}, got {actual:?}; error={error:?}"
        ))
        .into());
    }
    Ok(())
}

fn assert_policy_object(
    value: &Value,
    classification: &str,
    ai_export_allowed: bool,
    explicit: bool,
) -> TestResult<()> {
    if value["entityType"].as_str() != Some(TAXONOMY_CATEGORY_ENTITY_TYPE)
        || value["fieldKey"].as_str() != Some(FIELD_KEY)
        || value["classification"].as_str() != Some(classification)
        || value["aiExportAllowed"].as_bool() != Some(ai_export_allowed)
        || value["explicit"].as_bool() != Some(explicit)
    {
        return Err(test_error(format!("unexpected attached policy object: {value}")).into());
    }
    Ok(())
}

fn assert_provider_policy(
    snapshot: &TranslationResourceSnapshot,
    classification: TranslationDataClassification,
    ai_export_allowed: bool,
) -> TestResult<()> {
    let field = snapshot
        .fields
        .iter()
        .find(|field| field.descriptor.key.as_str() == FIELD_KEY)
        .ok_or_else(|| test_error("provider snapshot did not expose the governed field"))?;
    if field.descriptor.classification != classification
        || field.descriptor.ai_export_allowed != ai_export_allowed
    {
        return Err(test_error(format!(
            "registered provider exposed unexpected policy metadata: {:?}",
            field.descriptor
        ))
        .into());
    }
    Ok(())
}

fn assert_content_revisions_unchanged(
    snapshot: &TranslationResourceSnapshot,
    resource_revision: &OpaqueRevision,
    source_revision: &OpaqueRevision,
    target_revision: &Option<OpaqueRevision>,
) -> TestResult<()> {
    if &snapshot.summary.resource_revision != resource_revision
        || &snapshot.source_revision != source_revision
        || &snapshot.target_revision != target_revision
    {
        return Err(test_error(format!(
            "live governance policy changed Translation content revisions: {snapshot:?}"
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
        AuthConfig::new("test-secret-key-for-flex-policy-rbac-evidence-32bytes!".to_string()),
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
            "Flex attached policy RBAC evidence".into(),
            format!("flex-attached-policy-{}", tenant_id.simple()).into(),
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
                name: "Policy Evidence Category".to_string(),
                slug: None,
                canonical_key: Some(format!("policy-evidence-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?;
    Ok(category)
}

async fn create_localized_definition(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE)
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
        format!("flex-attached-policy-rbac-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
