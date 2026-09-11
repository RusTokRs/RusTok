#![cfg(feature = "mod-flex")]

use std::{error::Error, io, sync::Arc, time::Duration};

use async_graphql::{EmptySubscription, Request, Response, Schema};
use async_trait::async_trait;
use flex::graphql::{FlexGraphqlRuntime, FlexMutation, FlexQuery};
use flex::{
    CreateFlexEntryCommand, CreateFlexSchemaCommand, FieldDefRegistry, FieldDefinitionCachePort,
    FieldDefinitionView, FlexModule, FlexStandaloneService, UpdateFlexSchemaCommand,
    resolve_standalone_field_policy_resolutions,
};
use rustok_api::{AuthContext, Permission, PortActor, PortContext, TenantContext, TenantLocale};
use rustok_core::{
    ModuleRegistry,
    field_schema::{FieldDefinition, FieldType},
};
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
const RESOURCE_KIND: &str = "standalone_localized_value";
const FIELD_KEY: &str = "tagline";
const INELIGIBLE_FIELD_KEY: &str = "metadata";

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
async fn flex_standalone_policy_rbac_and_db_integrity_evidence_postgres() -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);

    let database_name = unique_postgres_database_name("rustok_flex_standalone_policy_evidence");
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
    let other_tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    seed_tenant(&db, tenant_id, "standalone-policy").await?;
    seed_tenant(&db, other_tenant_id, "standalone-policy-other").await?;

    let standalone = FlexStandaloneSeaOrmService::new(db.clone());
    let schema = standalone
        .create_schema(
            tenant_id,
            Some(actor_id),
            CreateFlexSchemaCommand {
                slug: "policy_evidence".to_string(),
                name: "Policy Evidence".to_string(),
                description: None,
                fields_config: vec![eligible_definition(), ineligible_definition()],
                settings: None,
                is_active: Some(true),
            },
        )
        .await?;
    let entry = standalone
        .create_entry(
            tenant_id,
            Some(actor_id),
            CreateFlexEntryCommand {
                schema_id: schema.id,
                entity_type: None,
                entity_id: None,
                data: json!({
                    FIELD_KEY: "Governed standalone entry",
                    INELIGIBLE_FIELD_KEY: {"owner": "flex"}
                }),
                status: Some("published".to_string()),
            },
        )
        .await?;

    let provider = registered_provider(db.clone())?;
    let identity = provider_identity(provider.as_ref(), tenant_id, entry.id).await?;
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
        .ok_or_else(|| test_error("source standalone write did not publish a ChangeCursor"))?;

    let graphql = graphql_schema(db.clone());
    let default_policy = successful_graphql(
        execute_graphql(
            &graphql,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_LIST],
            policy_query(schema.id),
        )
        .await,
        "default standalone policy query",
    )?;
    let default_rows = default_policy["standaloneFieldPolicies"]
        .as_array()
        .ok_or_else(|| test_error("standalone policy query did not return an array"))?;
    if default_rows.len() != 1 {
        return Err(test_error(format!(
            "standalone policy query exposed ineligible fields: {default_rows:?}"
        ))
        .into());
    }
    assert_policy_object(&default_rows[0], schema.id, "TENANT_PRIVATE", false, false)?;

    let query_without_list = execute_graphql(
        &graphql,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        policy_query(schema.id),
    )
    .await;
    assert_graphql_error_code(&query_without_list, "PERMISSION_DENIED")?;

    let denied_set = execute_graphql(
        &graphql,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_LIST],
        set_policy_mutation(schema.id, FIELD_KEY, "PUBLIC", true),
    )
    .await;
    assert_graphql_error_code(&denied_set, "PERMISSION_DENIED")?;

    let ineligible_set = execute_graphql(
        &graphql,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        set_policy_mutation(schema.id, INELIGIBLE_FIELD_KEY, "PUBLIC", true),
    )
    .await;
    assert_graphql_error_code(&ineligible_set, "BAD_USER_INPUT")?;

    let unknown_set = execute_graphql(
        &graphql,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        set_policy_mutation(schema.id, "missing_field", "PUBLIC", true),
    )
    .await;
    assert_graphql_error_code(&unknown_set, "BAD_USER_INPUT")?;

    let invalid_secret = execute_graphql(
        &graphql,
        tenant_id,
        actor_id,
        &[Permission::FLEX_SCHEMAS_UPDATE],
        set_policy_mutation(schema.id, FIELD_KEY, "SECRET", true),
    )
    .await;
    assert_graphql_error_code(&invalid_secret, "BAD_USER_INPUT")?;

    let set_public = successful_graphql(
        execute_graphql(
            &graphql,
            tenant_id,
            actor_id,
            &[Permission::FLEX_SCHEMAS_UPDATE],
            set_policy_mutation(schema.id, FIELD_KEY, "PUBLIC", true),
        )
        .await,
        "set standalone public policy mutation",
    )?;
    assert_policy_object(
        &set_public["setStandaloneFieldPolicy"],
        schema.id,
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
    assert_provider_policy(&public_snapshot, TranslationDataClassification::Public, true)?;
    assert_content_revisions_unchanged(
        &public_snapshot,
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
            "standalone policy governance manufactured Translation content changes: {post_policy_changes:?}"
        ))
        .into());
    }

    let cross_tenant_error = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO flex_standalone_field_policies (tenant_id, schema_id, field_key, classification, ai_export_allowed) VALUES ($1, $2, $3, $4, $5)",
            vec![
                other_tenant_id.into(),
                schema.id.into(),
                FIELD_KEY.into(),
                "public".into(),
                false.into(),
            ],
        ))
        .await
        .expect_err("cross-tenant direct policy write must fail at the PostgreSQL boundary");
    if !cross_tenant_error.to_string().contains("does not belong to tenant") {
        return Err(test_error(format!(
            "cross-tenant direct policy write failed for the wrong reason: {cross_tenant_error}"
        ))
        .into());
    }

    let ineligible_direct_error = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO flex_standalone_field_policies (tenant_id, schema_id, field_key, classification, ai_export_allowed) VALUES ($1, $2, $3, $4, $5)",
            vec![
                tenant_id.into(),
                schema.id.into(),
                INELIGIBLE_FIELD_KEY.into(),
                "public".into(),
                false.into(),
            ],
        ))
        .await
        .expect_err("ineligible direct policy write must fail at the PostgreSQL boundary");
    if !ineligible_direct_error
        .to_string()
        .contains("is not an active localized text field")
    {
        return Err(test_error(format!(
            "ineligible direct policy write failed for the wrong reason: {ineligible_direct_error}"
        ))
        .into());
    }

    standalone
        .update_schema(
            tenant_id,
            Some(actor_id),
            schema.id,
            UpdateFlexSchemaCommand {
                fields_config: Some(vec![ineligible_tagline_definition(), ineligible_definition()]),
                ..Default::default()
            },
        )
        .await?;

    let pruned = resolve_standalone_field_policy_resolutions(
        &db,
        tenant_id,
        schema.id,
        &[FIELD_KEY.to_string()],
    )
    .await?;
    let resolution = pruned
        .get(FIELD_KEY)
        .ok_or_else(|| test_error("standalone policy resolver omitted requested field"))?;
    if resolution.explicit {
        return Err(test_error(
            "schema field-type transition did not prune the explicit standalone policy row",
        )
        .into());
    }

    let stale_reinsert = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO flex_standalone_field_policies (tenant_id, schema_id, field_key, classification, ai_export_allowed) VALUES ($1, $2, $3, $4, $5)",
            vec![
                tenant_id.into(),
                schema.id.into(),
                FIELD_KEY.into(),
                "public".into(),
                false.into(),
            ],
        ))
        .await
        .expect_err("policy reinsert after field-type transition must fail");
    if !stale_reinsert
        .to_string()
        .contains("is not an active localized text field")
    {
        return Err(test_error(format!(
            "post-transition direct policy write failed for the wrong reason: {stale_reinsert}"
        ))
        .into());
    }

    drop(provider);
    db.close().await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(())
}

fn graphql_schema(db: DatabaseConnection) -> FlexPolicySchema {
    let runtime = FlexGraphqlRuntime::new(
        Arc::new(FlexStandaloneSeaOrmService::new(db.clone())),
        db.clone(),
        FieldDefRegistry::new(),
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
        name: "Flex standalone policy RBAC evidence".to_string(),
        slug: format!("flex-standalone-policy-{}", tenant_id.simple()),
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
        permissions: permissions.to_vec(),
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

fn policy_query(schema_id: Uuid) -> String {
    format!(
        r#"query {{
  standaloneFieldPolicies(schemaId: "{schema_id}") {{
    schemaId
    fieldKey
    classification
    aiExportAllowed
    explicit
  }}
}}"#
    )
}

fn set_policy_mutation(
    schema_id: Uuid,
    field_key: &str,
    classification: &str,
    ai_export_allowed: bool,
) -> String {
    format!(
        r#"mutation {{
  setStandaloneFieldPolicy(input: {{
    schemaId: "{schema_id}",
    fieldKey: "{field_key}",
    classification: {classification},
    aiExportAllowed: {ai_export_allowed}
  }}) {{
    schemaId
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
    schema_id: Uuid,
    classification: &str,
    ai_export_allowed: bool,
    explicit: bool,
) -> TestResult<()> {
    if value["schemaId"].as_str() != Some(schema_id.to_string().as_str())
        || value["fieldKey"].as_str() != Some(FIELD_KEY)
        || value["classification"].as_str() != Some(classification)
        || value["aiExportAllowed"].as_bool() != Some(ai_export_allowed)
        || value["explicit"].as_bool() != Some(explicit)
    {
        return Err(test_error(format!("unexpected standalone policy object: {value}")).into());
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
            "registered provider exposed unexpected standalone policy metadata: {:?}",
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
            "live standalone governance policy changed Translation content revisions: {snapshot:?}"
        ))
        .into());
    }
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
        AuthConfig::new("test-secret-key-for-flex-standalone-policy-evidence-32bytes!".to_string()),
    )?;
    let targets = translation_target_registry(&extensions)
        .ok_or_else(|| test_error("host composition did not publish TranslationTargetRegistry"))?;
    let owner_slug = OwnerSlug::new(OWNER_SLUG)?;
    let resource_kind = ResourceKind::new(RESOURCE_KIND)?;
    targets.get(&owner_slug, &resource_kind).ok_or_else(|| {
        test_error("host composition did not register flex/standalone_localized_value").into()
    })
}

async fn provider_identity(
    provider: &dyn TranslationTargetProvider,
    tenant_id: Uuid,
    entry_id: Uuid,
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
    let entry_id = entry_id.to_string();
    let resource = page
        .resources
        .into_iter()
        .find(|resource| resource.identity.resource_id.as_str() == entry_id)
        .ok_or_else(|| test_error("registered standalone provider did not list the seeded entry"))?;
    Ok(resource.identity)
}

async fn seed_tenant(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug_prefix: &str,
) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)",
        vec![
            tenant_id.into(),
            "Flex standalone policy RBAC evidence".into(),
            format!("{slug_prefix}-{}", tenant_id.simple()).into(),
        ],
    ))
    .await?;
    Ok(())
}

fn eligible_definition() -> FieldDefinition {
    FieldDefinition {
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
        position: 0,
        is_active: true,
    }
}

fn ineligible_definition() -> FieldDefinition {
    FieldDefinition {
        field_key: INELIGIBLE_FIELD_KEY.to_string(),
        field_type: FieldType::Json,
        label: [("en".to_string(), "Metadata".to_string())]
            .into_iter()
            .collect(),
        description: None,
        is_localized: true,
        is_required: false,
        default_value: None,
        validation: None,
        position: 1,
        is_active: true,
    }
}

fn ineligible_tagline_definition() -> FieldDefinition {
    FieldDefinition {
        field_type: FieldType::Json,
        ..eligible_definition()
    }
}

fn read_context(tenant_id: Uuid, suffix: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("flex-standalone-policy-rbac-{suffix}"),
    )
    .with_deadline(Duration::from_secs(30))
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
