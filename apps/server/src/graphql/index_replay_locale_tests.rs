use std::{collections::BTreeMap, sync::Arc};

use async_graphql::{EmptySubscription, Request, Schema};
use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    MigrationSource, ModuleRegistry, ModuleRuntimeExtensions, RusToKModule, UserRole,
};
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexSchema, IndexSource, IndexSourceCursor, IndexSourceFailure,
    IndexSourceLoadBatch, IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest,
    IndexValue, IndexValueType, LocaleKey, LocaleMode, ModuleName, PostgresSchemaRegistrationStore,
    SchemaRef, SchemaVersion, SharedIndexSchemaRegistry, register_index_schema_source,
    register_index_source,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use super::index_replay::IndexReplayMutation;
use crate::context::{AuthContext, TenantContext};
use crate::services::app_lifecycle::StopHandle;
use crate::services::index_replay_runtime_composition::materialize_index_replay_runtime;
use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

const TENANT_ID: Uuid = Uuid::from_u128(1);
const ACTOR_ID: Uuid = Uuid::from_u128(2);
const SOURCE_NAME: &str = "locale-evidence-primary";
const EN_PAGE_COUNT: usize = 9;
const DE_PAGE_COUNT: usize = 2;

struct LocaleReplayModule;
struct LocaleReplaySource;

#[async_trait]
impl IndexSource for LocaleReplaySource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        let (mutations, next_cursor) = match request.locale().map(LocaleKey::as_str) {
            Some("en-US") => locale_page(&request, locale("en-US"), EN_PAGE_COUNT, 1_000, 10_000),
            Some("de") => locale_page(&request, locale("de"), DE_PAGE_COUNT, 2_000, 20_000),
            Some(other) => panic!("unexpected locale replay scope: {other}"),
            None => {
                let mut mutations = Vec::with_capacity(EN_PAGE_COUNT + DE_PAGE_COUNT);
                for ordinal in 0..EN_PAGE_COUNT {
                    mutations.push(locale_mutation(
                        request.tenant_id(),
                        &locale("en-US"),
                        ordinal,
                        1_000,
                        10_000,
                    ));
                }
                for ordinal in 0..DE_PAGE_COUNT {
                    mutations.push(locale_mutation(
                        request.tenant_id(),
                        &locale("de"),
                        ordinal,
                        2_000,
                        20_000,
                    ));
                }
                (mutations, None)
            }
        };

        Ok(IndexSourcePage::new(&request, mutations, next_cursor)
            .expect("locale evidence page should satisfy source scope"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new())
            .expect("empty targeted load should satisfy source contract"))
    }
}

fn locale_page(
    request: &IndexSourceScanRequest,
    locale: LocaleKey,
    page_count: usize,
    entity_base: u128,
    event_base: u128,
) -> (Vec<IndexMutation>, Option<IndexSourceCursor>) {
    let ordinal = request
        .cursor()
        .map(|cursor| {
            cursor
                .value()
                .as_u64()
                .expect("locale evidence cursor should be an integer") as usize
        })
        .unwrap_or(0);
    assert!(
        ordinal < page_count,
        "locale cursor must remain within fixture pages"
    );
    let mutation = locale_mutation(
        request.tenant_id(),
        &locale,
        ordinal,
        entity_base,
        event_base,
    );
    let next_cursor = if ordinal + 1 < page_count {
        Some(IndexSourceCursor::new(json!(ordinal + 1)).expect("bounded locale cursor"))
    } else {
        None
    };
    (vec![mutation], next_cursor)
}

fn locale_mutation(
    tenant_id: Uuid,
    locale: &LocaleKey,
    ordinal: usize,
    entity_base: u128,
    event_base: u128,
) -> IndexMutation {
    let entity_id = Uuid::from_u128(entity_base + ordinal as u128);
    IndexMutation::Upsert {
        event_id: Uuid::from_u128(event_base + ordinal as u128),
        record: IndexRecord {
            key: EntityKey {
                tenant_id,
                schema: locale_schema_ref(),
                entity_id,
                locale: Some(locale.clone()),
            },
            source_version: 1,
            fields: BTreeMap::from([(
                FieldName::new("id").expect("field name"),
                IndexValue::Uuid(entity_id),
            )]),
            links: Vec::new(),
        },
    }
}

impl MigrationSource for LocaleReplayModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for LocaleReplayModule {
    fn slug(&self) -> &'static str {
        "locale_evidence"
    }

    fn name(&self) -> &'static str {
        "Locale replay evidence"
    }

    fn description(&self) -> &'static str {
        "Deterministic locale replay GraphQL restart and scope-isolation evidence source"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let schema = locale_schema();
        register_index_schema_source(extensions, self.slug(), schema.clone()).map_err(|error| {
            rustok_core::Error::Validation(format!(
                "locale evidence schema registration failed: {error}"
            ))
        })?;
        register_index_source(
            extensions,
            self.slug(),
            SOURCE_NAME,
            [schema.reference],
            LocaleReplaySource,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "locale evidence source registration failed: {error}"
            ))
        })
    }
}

#[derive(Default)]
struct ReplayTestQuery;

#[async_graphql::Object]
impl ReplayTestQuery {
    async fn _empty(&self) -> bool {
        true
    }
}

type ReplayTestSchema = Schema<ReplayTestQuery, IndexReplayMutation, EmptySubscription>;

struct ReplayGraphqlRuntime {
    schema: ReplayTestSchema,
    _stop_handle: StopHandle,
    _stop_receiver: tokio::sync::watch::Receiver<bool>,
}

#[tokio::test]
async fn graphql_locale_replay_yields_isolates_scopes_and_fresh_runtime_resumes_same_job() {
    let db = replay_database().await;
    let first_runtime = graphql_runtime(&db).await;

    let first_en = execute_replay(&first_runtime.schema, Some("EN-us")).await;
    assert_eq!(first_en["status"], "YIELDED");
    assert_eq!(first_en["pagesProcessed"], 8);
    assert_eq!(first_en["mutationsProcessed"], 8);
    assert_eq!(first_en["appliedCount"], 8);
    assert_eq!(first_en["duplicateCount"], 0);
    let en_job_id = response_job_id(&first_en);
    assert_eq!(job_state(&db, en_job_id).await, "pending");
    assert_eq!(job_attempt_count(&db, en_job_id).await, 1);
    assert_eq!(job_scope_count(&db, "locale", Some("en-US")).await, 1);
    assert_eq!(checkpoint_cursor_text(&db, "en-US").await, "8");

    let de = execute_replay(&first_runtime.schema, Some("de")).await;
    assert_eq!(de["status"], "COMPLETE");
    assert_eq!(de["pagesProcessed"], DE_PAGE_COUNT as i64);
    assert_eq!(de["mutationsProcessed"], DE_PAGE_COUNT as i64);
    assert_eq!(de["appliedCount"], DE_PAGE_COUNT as i64);
    assert_eq!(de["duplicateCount"], 0);
    let de_job_id = response_job_id(&de);
    assert_ne!(de_job_id, en_job_id);
    assert_eq!(job_state(&db, de_job_id).await, "succeeded");
    assert_eq!(job_attempt_count(&db, de_job_id).await, 1);
    assert_eq!(job_scope_count(&db, "locale", Some("de")).await, 1);
    assert_eq!(checkpoint_json_type(&db, "de").await, "null");
    assert_eq!(job_state(&db, en_job_id).await, "pending");
    assert_eq!(checkpoint_cursor_text(&db, "en-US").await, "8");

    let schema_wide = execute_replay(&first_runtime.schema, None).await;
    assert_eq!(schema_wide["status"], "COMPLETE");
    assert_eq!(schema_wide["pagesProcessed"], 1);
    assert_eq!(
        schema_wide["mutationsProcessed"],
        (EN_PAGE_COUNT + DE_PAGE_COUNT) as i64
    );
    assert_eq!(schema_wide["appliedCount"], 1);
    assert_eq!(
        schema_wide["duplicateCount"],
        (EN_PAGE_COUNT + DE_PAGE_COUNT - 1) as i64
    );
    let schema_job_id = response_job_id(&schema_wide);
    assert_ne!(schema_job_id, en_job_id);
    assert_ne!(schema_job_id, de_job_id);
    assert_eq!(job_state(&db, schema_job_id).await, "succeeded");
    assert_eq!(job_attempt_count(&db, schema_job_id).await, 1);
    assert_eq!(job_scope_count(&db, "schema", None).await, 1);
    assert_eq!(checkpoint_json_type(&db, "").await, "null");
    assert_eq!(checkpoint_count(&db).await, 3);
    assert_eq!(job_state(&db, en_job_id).await, "pending");
    assert_eq!(checkpoint_cursor_text(&db, "en-US").await, "8");
    assert_eq!(materialized_entity_count(&db).await, 11);
    assert_eq!(applied_inbox_count(&db).await, 11);

    // A new GraphQL/operator/runtime composition over the same durable database must reclaim only
    // the pending en-US job. The schema-wide run already delivered the last en-US owner event, so
    // attempt 2 proves restart-safe redelivery by observing one Duplicate before committing the
    // en-US completion checkpoint.
    let restarted_runtime = graphql_runtime(&db).await;
    let resumed_en = execute_replay(&restarted_runtime.schema, Some("en-US")).await;
    assert_eq!(resumed_en["status"], "COMPLETE");
    assert_eq!(resumed_en["jobId"], en_job_id.to_string());
    assert_eq!(resumed_en["pagesProcessed"], 1);
    assert_eq!(resumed_en["mutationsProcessed"], 1);
    assert_eq!(resumed_en["appliedCount"], 0);
    assert_eq!(resumed_en["duplicateCount"], 1);
    assert_eq!(job_state(&db, en_job_id).await, "succeeded");
    assert_eq!(job_attempt_count(&db, en_job_id).await, 2);
    assert_eq!(checkpoint_json_type(&db, "en-US").await, "null");
    assert_eq!(checkpoint_count(&db).await, 3);
    assert_eq!(succeeded_replay_job_count(&db).await, 3);
    assert_eq!(materialized_entity_count(&db).await, 11);
    assert_eq!(applied_inbox_count(&db).await, 11);
}

async fn replay_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory SQLite should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should be enabled");
    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
        .await
        .expect("tenant fixture should be created");
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{TENANT_ID}')"))
        .await
        .expect("tenant fixture should be inserted");

    let manager = SchemaManager::new(&db);
    for migration in IndexModule.migrations() {
        migration
            .up(&manager)
            .await
            .unwrap_or_else(|error| panic!("{} should apply: {error}", migration.name()));
    }
    db
}

async fn graphql_runtime(db: &DatabaseConnection) -> ReplayGraphqlRuntime {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(LocaleReplayModule);
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)
        .expect("locale evidence runtime extensions should compose");

    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .expect("locale evidence schema registry should be present");
    let schema_store = PostgresSchemaRegistrationStore::new(db.clone());
    for registered in schemas.registry().iter() {
        schema_store
            .register(TENANT_ID, &registered.schema)
            .await
            .expect("locale evidence schema should persist idempotently");
    }

    materialize_index_replay_runtime(&mut extensions, db.clone())
        .expect("guarded locale replay runtime should materialize");
    let extensions = Arc::new(extensions);
    let (stop_handle, stop_receiver) = StopHandle::new();
    let schema = Schema::build(ReplayTestQuery, IndexReplayMutation, EmptySubscription)
        .data(extensions)
        .data(stop_handle.clone())
        .finish();

    ReplayGraphqlRuntime {
        schema,
        _stop_handle: stop_handle,
        _stop_receiver: stop_receiver,
    }
}

async fn execute_replay(schema: &ReplayTestSchema, locale: Option<&str>) -> JsonValue {
    let response = with_rbac_request_scope(
        Some(operator_scope()),
        schema.execute(replay_request(locale)),
    )
    .await;
    assert!(
        response.errors.is_empty(),
        "locale replay GraphQL response must not contain errors: {:?}",
        response.errors
    );
    response
        .data
        .into_json()
        .expect("locale replay GraphQL response should convert to JSON")["runIndexReplay"]
        .clone()
}

fn replay_request(locale: Option<&str>) -> Request {
    let locale_field = locale
        .map(|locale| format!("\n    locale: \"{locale}\""))
        .unwrap_or_default();
    Request::new(format!(
        r#"
mutation {{
  runIndexReplay(input: {{
    moduleName: "locale-evidence"
    entityName: "item"
    schemaVersion: "1"{locale_field}
  }}) {{
    status
    jobId
    pagesProcessed
    mutationsProcessed
    appliedCount
    duplicateCount
    staleCount
  }}
}}
"#
    ))
    .data(AuthContext {
        user_id: ACTOR_ID,
        session_id: Uuid::new_v4(),
        tenant_id: TENANT_ID,
        permissions: vec![Permission::MODULES_MANAGE],
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    })
    .data(TenantContext {
        id: TENANT_ID,
        name: "Locale replay evidence".to_string(),
        slug: "locale-evidence".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en-US".to_string(),
        is_active: true,
    })
}

fn operator_scope() -> RbacRequestScope {
    RbacRequestScope::new(
        TENANT_ID,
        ACTOR_ID,
        vec![Permission::MODULES_MANAGE],
        UserRole::Admin,
    )
}

fn locale_schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("locale-evidence").expect("module name"),
        entity: EntityName::new("item").expect("entity name"),
        version: SchemaVersion::INITIAL,
    }
}

fn locale_schema() -> IndexSchema {
    IndexSchema {
        reference: locale_schema_ref(),
        locale_mode: LocaleMode::Required,
        fields: vec![IndexField {
            name: FieldName::new("id").expect("field name"),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

fn locale(value: &str) -> LocaleKey {
    LocaleKey::new(value).expect("locale evidence locale should be canonicalizable")
}

fn response_job_id(run: &JsonValue) -> Uuid {
    Uuid::parse_str(
        run["jobId"]
            .as_str()
            .expect("locale replay response should expose job id"),
    )
    .expect("locale replay job id should be a UUID")
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should be integer")
}

async fn scalar_string(db: &DatabaseConnection, sql: &str) -> String {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should be text")
}

async fn job_state(db: &DatabaseConnection, job_id: Uuid) -> String {
    scalar_string(
        db,
        &format!("SELECT state AS value FROM index_jobs WHERE job_id = '{job_id}'"),
    )
    .await
}

async fn job_attempt_count(db: &DatabaseConnection, job_id: Uuid) -> i64 {
    scalar_i64(
        db,
        &format!("SELECT attempt_count AS value FROM index_jobs WHERE job_id = '{job_id}'"),
    )
    .await
}

async fn job_scope_count(db: &DatabaseConnection, scope_kind: &str, locale: Option<&str>) -> i64 {
    let locale_clause = match locale {
        Some(locale) => format!("locale_key = '{locale}'"),
        None => "locale_key IS NULL".to_owned(),
    };
    scalar_i64(
        db,
        &format!(
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND scope_kind = '{scope_kind}' AND {locale_clause}"
        ),
    )
    .await
}

async fn checkpoint_cursor_text(db: &DatabaseConnection, locale_key: &str) -> String {
    scalar_string(
        db,
        &format!(
            "SELECT CAST(cursor AS TEXT) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild' AND source_name = '{SOURCE_NAME}' AND locale_key = '{locale_key}'"
        ),
    )
    .await
}

async fn checkpoint_json_type(db: &DatabaseConnection, locale_key: &str) -> String {
    scalar_string(
        db,
        &format!(
            "SELECT json_type(cursor) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild' AND source_name = '{SOURCE_NAME}' AND locale_key = '{locale_key}'"
        ),
    )
    .await
}

async fn checkpoint_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        &format!(
            "SELECT COUNT(*) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild' AND source_name = '{SOURCE_NAME}'"
        ),
    )
    .await
}

async fn succeeded_replay_job_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'succeeded'",
    )
    .await
}

async fn materialized_entity_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM index_entities WHERE tenant_id = '00000000-0000-0000-0000-000000000001' AND module_name = 'locale-evidence' AND entity_name = 'item' AND schema_version = 1 AND is_deleted = 0",
    )
    .await
}

async fn applied_inbox_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        &format!(
            "SELECT COUNT(*) AS value FROM index_inbox WHERE tenant_id = '00000000-0000-0000-0000-000000000001' AND source_name = '{SOURCE_NAME}' AND state = 'applied'"
        ),
    )
    .await
}
