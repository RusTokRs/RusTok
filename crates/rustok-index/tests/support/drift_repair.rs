#![expect(
    dead_code,
    reason = "Shared test support module for rustok-index integration test targets"
)]

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use rustok_core::MigrationSource;
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexDriftFindingLifecycleActor,
    IndexDriftRepairAuthorization, IndexDriftRepairAuthorizer, IndexDriftRepairCommand,
    IndexDriftRepairEvidenceReader, IndexDriftRepairFailure, IndexDriftRepairOwner,
    IndexDriftRepairOwnerRegistry, IndexDriftRepairRecoveryAuthorization,
    IndexDriftRepairRecoveryAuthorizer, IndexDriftRepairRecoveryCommand,
    IndexDriftRepairRecoveryFailure, IndexDriftRepairRecoveryService, IndexDriftRepairService,
    IndexDriftRepairStore, IndexDriftRepairTarget, IndexField, IndexLink, IndexLinkValue,
    IndexMutation, IndexRecord, IndexSchema, IndexSchemaSourceCatalog, IndexSource,
    IndexSourceAbsenceCatalog, IndexSourceAbsenceProvider, IndexSourceAbsenceWatermark,
    IndexSourceCatalog, IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest,
    IndexSourcePage, IndexSourceScanRequest, IndexValue, IndexValueType, LinkCardinality, LinkName,
    LinkedEntityKey, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
    SharedIndexSourceAbsenceRegistry, SharedIndexSourceRegistry,
    infrastructure::postgres::{
        IndexDriftDigestFindingRequest, IndexDriftFindingScope, IndexDriftFindingSeverity,
        MutationDelivery, PostgresIndexDriftFindingWriter,
        PostgresIndexDriftMissingEntityEvidenceReader, PostgresIndexDriftMissingEntityRepairOwner,
        PostgresIndexDriftOrphanLinkEvidenceReader, PostgresIndexDriftOrphanLinkRepairOwner,
        PostgresIndexDriftRepairRecoveryStore, PostgresIndexDriftRepairStore,
        PostgresMutationStore, PostgresSchemaRegistrationStore, RecoveryAwareIndexDriftRepairOwner,
        RecoveryAwareIndexDriftRepairStore,
    },
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
pub const SOURCE_OWNER: &str = "repair_evidence";
pub const SOURCE_NAME: &str = "repair-evidence-source";
pub const ABSENCE_NAME: &str = "repair-evidence-absence";
pub const MISSING_DELIVERY_SOURCE: &str = "index_drift_repair_missing_entity";
pub const ORPHAN_DELIVERY_SOURCE: &str = "index_drift_repair_orphan_link";

const MISSING_CHECK: &str = "index.confirmed_missing_entity";
const ORPHAN_CHECK_PREFIX: &str = "index.confirmed_orphan_link.";
const MISSING_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_missing_entity_evidence_v1";
const ORPHAN_EVIDENCE_DOMAIN: &[u8] = b"index_confirmed_orphan_link_evidence_v1";
const ORPHAN_IDENTITY_DOMAIN: &[u8] = b"index_confirmed_orphan_link_identity_v1";

pub type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
    pub tenant_id: Uuid,
}

impl TestDatabase {
    pub async fn setup(test_name: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!("{DATABASE_ENV} is not set to a PostgreSQL URL; skipping {test_name}");
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_index_repair_evidence_{}_{}",
            test_name,
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&database_url, &schema_name).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id) VALUES ($1)",
            vec![tenant_id.into()],
        ))
        .await?;

        let manager = SchemaManager::new(&db);
        for migration in rustok_index::IndexModule.migrations() {
            migration.up(&manager).await?;
        }

        Ok(Some(Self {
            control,
            database_url,
            schema_name,
            tenant_id,
        }))
    }

    pub async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    pub async fn migrate_down(&self) -> TestResult<()> {
        let db = self.connection().await?;
        let manager = SchemaManager::new(&db);
        let migrations = rustok_index::IndexModule.migrations();
        for migration in migrations.into_iter().rev() {
            migration.down(&manager).await?;
        }
        Ok(())
    }

    pub async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct FixtureSchemas {
    pub missing: IndexSchema,
    pub source: IndexSchema,
    pub target: IndexSchema,
    pub id_field: FieldName,
    pub target_id_field: FieldName,
    pub link_name: LinkName,
}

impl FixtureSchemas {
    pub fn new() -> TestResult<Self> {
        let id_field = FieldName::new("id")?;
        let target_id_field = FieldName::new("target_id")?;
        let link_name = LinkName::new("targets")?;
        let missing_ref = schema_ref("missing");
        let source_ref = schema_ref("source");
        let target_ref = schema_ref("target");

        let missing = IndexSchema {
            reference: missing_ref,
            locale_mode: LocaleMode::None,
            fields: vec![uuid_field(id_field.clone(), false)],
            links: Vec::new(),
        };
        let target = IndexSchema {
            reference: target_ref.clone(),
            locale_mode: LocaleMode::None,
            fields: vec![uuid_field(id_field.clone(), false)],
            links: Vec::new(),
        };
        let source = IndexSchema {
            reference: source_ref,
            locale_mode: LocaleMode::None,
            fields: vec![
                uuid_field(id_field.clone(), false),
                uuid_field(target_id_field.clone(), true),
            ],
            links: vec![IndexLink {
                name: link_name.clone(),
                source_fields: vec![target_id_field.clone()],
                target_schema: target_ref,
                target_fields: vec![id_field.clone()],
                cardinality: LinkCardinality::Many,
            }],
        };
        Ok(Self {
            missing,
            source,
            target,
            id_field,
            target_id_field,
            link_name,
        })
    }

    pub fn all(&self) -> [IndexSchema; 3] {
        [
            self.missing.clone(),
            self.source.clone(),
            self.target.clone(),
        ]
    }
}

#[derive(Clone, Default)]
pub struct FixtureAuthority {
    mutations: Arc<RwLock<BTreeMap<EntityKey, IndexMutation>>>,
    absences: Arc<RwLock<BTreeMap<EntityKey, u64>>>,
}

impl FixtureAuthority {
    pub fn set_mutation(&self, mutation: IndexMutation) {
        self.mutations
            .write()
            .expect("fixture mutation lock")
            .insert(mutation.key().clone(), mutation);
    }

    pub fn clear_mutation(&self, key: &EntityKey) {
        self.mutations
            .write()
            .expect("fixture mutation lock")
            .remove(key);
    }

    pub fn set_absence(&self, key: EntityKey, source_version: u64) {
        self.absences
            .write()
            .expect("fixture absence lock")
            .insert(key, source_version);
    }

    pub fn clear_absence(&self, key: &EntityKey) {
        self.absences
            .write()
            .expect("fixture absence lock")
            .remove(key);
    }
}

#[async_trait]
impl IndexSource for FixtureAuthority {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        IndexSourcePage::new(&request, Vec::new(), None).map_err(|_| fixture_source_failure())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        let state = self.mutations.read().expect("fixture mutation lock");
        let mutations = request
            .keys()
            .iter()
            .filter_map(|key| state.get(key).cloned())
            .collect();
        IndexSourceLoadBatch::new(&request, mutations).map_err(|_| fixture_source_failure())
    }
}

#[async_trait]
impl IndexSourceAbsenceProvider for FixtureAuthority {
    async fn load_absence_watermark(
        &self,
        key: EntityKey,
    ) -> Result<Option<IndexSourceAbsenceWatermark>, IndexSourceFailure> {
        let source_version = self
            .absences
            .read()
            .expect("fixture absence lock")
            .get(&key)
            .copied();
        source_version
            .map(|version| {
                IndexSourceAbsenceWatermark::new(key, version).map_err(|_| fixture_source_failure())
            })
            .transpose()
    }
}

pub struct FixtureRuntime {
    pub schemas: Arc<rustok_index::SchemaRegistry>,
    pub sources: SharedIndexSourceRegistry,
    pub absence: SharedIndexSourceAbsenceRegistry,
    pub authority: FixtureAuthority,
    pub contracts: FixtureSchemas,
}

impl FixtureRuntime {
    pub async fn setup(database: &TestDatabase) -> TestResult<Self> {
        let contracts = FixtureSchemas::new()?;
        let mut schema_catalog = IndexSchemaSourceCatalog::new();
        for schema in contracts.all() {
            schema_catalog.register(SOURCE_OWNER, schema)?;
        }
        let shared = schema_catalog.materialize()?;
        let schemas = shared.shared();

        let authority = FixtureAuthority::default();
        let mut source_catalog = IndexSourceCatalog::new();
        source_catalog.register(
            SOURCE_OWNER,
            SOURCE_NAME,
            contracts
                .all()
                .into_iter()
                .map(|schema| schema.reference)
                .collect::<Vec<_>>(),
            authority.clone(),
        )?;
        let sources = source_catalog.materialize(&schema_catalog)?;

        let mut absence_catalog = IndexSourceAbsenceCatalog::new();
        absence_catalog.register(
            SOURCE_OWNER,
            ABSENCE_NAME,
            contracts
                .all()
                .into_iter()
                .map(|schema| schema.reference)
                .collect::<Vec<_>>(),
            authority.clone(),
        )?;
        let absence = absence_catalog.materialize(&sources)?;

        let registration = PostgresSchemaRegistrationStore::new(database.connection().await?);
        for schema in contracts.all() {
            registration.register(database.tenant_id, &schema).await?;
        }

        Ok(Self {
            schemas,
            sources,
            absence,
            authority,
            contracts,
        })
    }

    pub fn missing_key(&self, tenant_id: Uuid, entity_id: Uuid) -> EntityKey {
        EntityKey {
            tenant_id,
            schema: self.contracts.missing.reference.clone(),
            entity_id,
            locale: None,
        }
    }

    pub fn source_key(&self, tenant_id: Uuid, entity_id: Uuid) -> EntityKey {
        EntityKey {
            tenant_id,
            schema: self.contracts.source.reference.clone(),
            entity_id,
            locale: None,
        }
    }

    pub fn target_key(&self, tenant_id: Uuid, entity_id: Uuid) -> EntityKey {
        EntityKey {
            tenant_id,
            schema: self.contracts.target.reference.clone(),
            entity_id,
            locale: None,
        }
    }

    pub fn linked_target(&self, entity_id: Uuid) -> LinkedEntityKey {
        LinkedEntityKey {
            schema: self.contracts.target.reference.clone(),
            entity_id,
            locale: None,
        }
    }

    pub fn missing_record(&self, key: EntityKey, source_version: u64) -> IndexRecord {
        let mut fields = BTreeMap::new();
        fields.insert(
            self.contracts.id_field.clone(),
            IndexValue::Uuid(key.entity_id),
        );
        IndexRecord {
            key,
            source_version,
            fields,
            links: Vec::new(),
        }
    }

    pub fn target_record(&self, key: EntityKey, source_version: u64) -> IndexRecord {
        let mut fields = BTreeMap::new();
        fields.insert(
            self.contracts.id_field.clone(),
            IndexValue::Uuid(key.entity_id),
        );
        IndexRecord {
            key,
            source_version,
            fields,
            links: Vec::new(),
        }
    }

    pub fn source_record(
        &self,
        key: EntityKey,
        source_version: u64,
        targets: Vec<LinkedEntityKey>,
    ) -> IndexRecord {
        let mut fields = BTreeMap::new();
        fields.insert(
            self.contracts.id_field.clone(),
            IndexValue::Uuid(key.entity_id),
        );
        if let Some(target) = targets.first() {
            fields.insert(
                self.contracts.target_id_field.clone(),
                IndexValue::Uuid(target.entity_id),
            );
        }
        IndexRecord {
            key,
            source_version,
            fields,
            links: vec![IndexLinkValue {
                name: self.contracts.link_name.clone(),
                targets,
            }],
        }
    }
}

pub async fn apply_record(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
    record: IndexRecord,
    source_name: &str,
) -> TestResult<()> {
    let mutation = IndexMutation::Upsert {
        event_id: Uuid::new_v4(),
        record,
    };
    let delivery = MutationDelivery::from_event(source_name, mutation.clone())?;
    PostgresMutationStore::new(database.connection().await?)
        .apply(runtime.schemas.as_ref(), &delivery)
        .await?;
    runtime.authority.set_mutation(mutation);
    Ok(())
}

pub async fn create_missing_finding(
    database: &TestDatabase,
    _runtime: &FixtureRuntime,
    key: EntityKey,
    indexed_source_version: u64,
    absence_source_version: u64,
) -> TestResult<(Uuid, IndexDriftRepairTarget)> {
    let target = IndexDriftRepairTarget::missing_entity(
        key.clone(),
        indexed_source_version,
        absence_source_version,
    )?;
    let expected = missing_digest(
        &key,
        indexed_source_version,
        absence_source_version,
        b"owner_absent",
    );
    let actual = missing_digest(
        &key,
        indexed_source_version,
        absence_source_version,
        b"index_present",
    );
    let request = IndexDriftDigestFindingRequest::new(
        database.tenant_id,
        MISSING_CHECK,
        IndexDriftFindingSeverity::Error,
        IndexDriftFindingScope::EntityWithoutLocale {
            schema: key.schema.clone(),
            entity_id: key.entity_id,
        },
        expected,
        actual,
    )?;
    let outcome = PostgresIndexDriftFindingWriter::new(database.connection().await?)
        .record_digest_mismatch(&request)
        .await?;
    Ok((outcome.finding_id(), target))
}

pub async fn create_orphan_finding(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
    source_key: EntityKey,
    indexed_source_version: u64,
    ordinal: u32,
    target_key: LinkedEntityKey,
    target_absence_source_version: u64,
) -> TestResult<(Uuid, IndexDriftRepairTarget)> {
    let target = IndexDriftRepairTarget::orphan_link(
        source_key.clone(),
        indexed_source_version,
        runtime.contracts.link_name.clone(),
        ordinal,
        target_key.clone(),
        target_absence_source_version,
    )?;
    let identity = orphan_identity_digest(
        &runtime.contracts.link_name,
        ordinal,
        &target_key,
        target_absence_source_version,
    );
    let expected = orphan_digest(
        &source_key,
        indexed_source_version,
        &runtime.contracts.link_name,
        ordinal,
        &target_key,
        target_absence_source_version,
        b"target_absent",
    );
    let actual = orphan_digest(
        &source_key,
        indexed_source_version,
        &runtime.contracts.link_name,
        ordinal,
        &target_key,
        target_absence_source_version,
        b"source_link_present",
    );
    let request = IndexDriftDigestFindingRequest::new(
        database.tenant_id,
        format!("{ORPHAN_CHECK_PREFIX}{identity}"),
        IndexDriftFindingSeverity::Error,
        IndexDriftFindingScope::EntityWithoutLocale {
            schema: source_key.schema.clone(),
            entity_id: source_key.entity_id,
        },
        expected,
        actual,
    )?;
    let outcome = PostgresIndexDriftFindingWriter::new(database.connection().await?)
        .record_digest_mismatch(&request)
        .await?;
    Ok((outcome.finding_id(), target))
}

pub fn repair_command(
    tenant_id: Uuid,
    finding_id: Uuid,
    command_id: Uuid,
    target: IndexDriftRepairTarget,
    reason: &str,
) -> TestResult<IndexDriftRepairCommand> {
    Ok(IndexDriftRepairCommand::new(
        tenant_id,
        finding_id,
        command_id,
        target,
        actor(),
        reason,
    )?)
}

pub fn recovery_command(
    tenant_id: Uuid,
    finding_id: Uuid,
    command_id: Uuid,
    payload_digest: String,
    decision_id: Uuid,
    expected_revision: Option<u64>,
    action: rustok_index::IndexDriftRepairRecoveryAction,
    reason: &str,
) -> TestResult<IndexDriftRepairRecoveryCommand> {
    Ok(IndexDriftRepairRecoveryCommand::new(
        tenant_id,
        finding_id,
        command_id,
        payload_digest,
        decision_id,
        expected_revision,
        action,
        actor(),
        reason,
    )?)
}

pub fn actor() -> IndexDriftFindingLifecycleActor {
    IndexDriftFindingLifecycleActor::new("operator", "repair-evidence-owner")
        .expect("static fixture actor is valid")
}

#[derive(Clone)]
pub struct AllowRepair;

#[async_trait]
impl IndexDriftRepairAuthorizer for AllowRepair {
    async fn authorize(
        &self,
        _command: &IndexDriftRepairCommand,
    ) -> Result<IndexDriftRepairAuthorization, IndexDriftRepairFailure> {
        Ok(IndexDriftRepairAuthorization::Allowed)
    }
}

#[derive(Clone)]
pub struct AllowRecovery;

#[async_trait]
impl IndexDriftRepairRecoveryAuthorizer for AllowRecovery {
    async fn authorize(
        &self,
        _command: &IndexDriftRepairRecoveryCommand,
    ) -> Result<IndexDriftRepairRecoveryAuthorization, IndexDriftRepairRecoveryFailure> {
        Ok(IndexDriftRepairRecoveryAuthorization::Allowed)
    }
}

pub async fn recovery_store(database: &TestDatabase) -> TestResult<Arc<dyn IndexDriftRepairStore>> {
    let inner: Arc<dyn IndexDriftRepairStore> = Arc::new(PostgresIndexDriftRepairStore::new(
        database.connection().await?,
    ));
    Ok(Arc::new(RecoveryAwareIndexDriftRepairStore::new(
        database.connection().await?,
        inner,
    )?))
}

pub async fn missing_evidence(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
) -> TestResult<Arc<dyn IndexDriftRepairEvidenceReader>> {
    Ok(Arc::new(
        PostgresIndexDriftMissingEntityEvidenceReader::new(
            database.connection().await?,
            runtime.sources.clone(),
            runtime.absence.clone(),
        )?,
    ))
}

pub async fn orphan_evidence(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
) -> TestResult<Arc<dyn IndexDriftRepairEvidenceReader>> {
    Ok(Arc::new(PostgresIndexDriftOrphanLinkEvidenceReader::new(
        database.connection().await?,
        runtime.sources.clone(),
        runtime.absence.clone(),
    )?))
}

pub async fn missing_owner(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
) -> TestResult<Arc<dyn IndexDriftRepairOwner>> {
    let base: Arc<dyn IndexDriftRepairOwner> =
        Arc::new(PostgresIndexDriftMissingEntityRepairOwner::new(
            database.connection().await?,
            runtime.schemas.clone(),
        )?);
    Ok(Arc::new(RecoveryAwareIndexDriftRepairOwner::new(
        database.connection().await?,
        base,
    )?))
}

pub async fn orphan_owner(database: &TestDatabase) -> TestResult<Arc<dyn IndexDriftRepairOwner>> {
    let base: Arc<dyn IndexDriftRepairOwner> = Arc::new(
        PostgresIndexDriftOrphanLinkRepairOwner::new(database.connection().await?)?,
    );
    Ok(Arc::new(RecoveryAwareIndexDriftRepairOwner::new(
        database.connection().await?,
        base,
    )?))
}

pub fn repair_service(
    evidence: Arc<dyn IndexDriftRepairEvidenceReader>,
    owner: Arc<dyn IndexDriftRepairOwner>,
    store: Arc<dyn IndexDriftRepairStore>,
) -> TestResult<IndexDriftRepairService> {
    let owners = IndexDriftRepairOwnerRegistry::new([owner])?;
    Ok(IndexDriftRepairService::new_boxed(
        Arc::new(AllowRepair),
        evidence,
        owners,
        store,
    ))
}

pub async fn recovery_service(
    database: &TestDatabase,
) -> TestResult<IndexDriftRepairRecoveryService> {
    let store: Arc<dyn rustok_index::IndexDriftRepairRecoveryStore> = Arc::new(
        PostgresIndexDriftRepairRecoveryStore::new(database.connection().await?)?,
    );
    Ok(IndexDriftRepairRecoveryService::new_boxed(
        Arc::new(AllowRecovery),
        store,
    ))
}

pub async fn payload_digest(database: &TestDatabase, command_id: Uuid) -> TestResult<String> {
    let db = database.connection().await?;
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT payload_digest FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND command_id = $2",
            vec![database.tenant_id.into(), command_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("repair command payload digest is missing"))?;
    Ok(row.try_get("", "payload_digest")?)
}

pub async fn repair_command_state(database: &TestDatabase, command_id: Uuid) -> TestResult<String> {
    let db = database.connection().await?;
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT state FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND command_id = $2",
            vec![database.tenant_id.into(), command_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("repair command is missing"))?;
    Ok(row.try_get("", "state")?)
}

pub async fn count_repair_commands(database: &TestDatabase, finding_id: Uuid) -> TestResult<i64> {
    count_value(
        database,
        "SELECT COUNT(*)::bigint AS value FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND finding_id = $2",
        vec![database.tenant_id.into(), finding_id.into()],
    )
    .await
}

pub async fn count_recovery_decisions(
    database: &TestDatabase,
    command_id: Uuid,
) -> TestResult<i64> {
    count_value(
        database,
        "SELECT COUNT(*)::bigint AS value FROM index_consistency_finding_repair_recovery_decisions WHERE tenant_id = $1 AND command_id = $2",
        vec![database.tenant_id.into(), command_id.into()],
    )
    .await
}

pub async fn inbox_state(
    database: &TestDatabase,
    source_name: &str,
    command_id: Uuid,
) -> TestResult<Option<String>> {
    let db = database.connection().await?;
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT state FROM index_inbox WHERE tenant_id = $1 AND source_name = $2 AND delivery_id = $3",
            vec![
                database.tenant_id.into(),
                source_name.to_owned().into(),
                command_id.to_string().into(),
            ],
        ))
        .await?;
    match row {
        Some(value) => Ok(Some(value.try_get("", "state")?)),
        None => Ok(None),
    }
}

pub async fn entity_state(
    database: &TestDatabase,
    key: &EntityKey,
) -> TestResult<Option<(u64, bool)>> {
    let db = database.connection().await?;
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4 AND entity_id = $5 AND locale_key = ''",
            vec![
                key.tenant_id.into(),
                key.schema.module.as_str().to_owned().into(),
                key.schema.entity.as_str().to_owned().into(),
                i64::from(key.schema.version.get()).into(),
                key.entity_id.into(),
            ],
        ))
        .await?;
    match row {
        Some(value) => {
            let source_version: String = value.try_get("", "source_version_text")?;
            Ok(Some((
                source_version.parse::<u64>()?,
                value.try_get("", "is_deleted")?,
            )))
        }
        None => Ok(None),
    }
}

pub async fn exact_link_count(
    database: &TestDatabase,
    source_key: &EntityKey,
    source_version: u64,
    link_name: &LinkName,
    ordinal: u32,
    target: &LinkedEntityKey,
) -> TestResult<i64> {
    count_value(
        database,
        "SELECT COUNT(*)::bigint AS value FROM index_links WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = '' AND source_version = $6 AND link_name = $7 AND ordinal = $8 AND target_module = $9 AND target_entity = $10 AND target_schema_version = $11 AND target_entity_id = $12 AND target_locale_key = ''",
        vec![
            source_key.tenant_id.into(),
            source_key.schema.module.as_str().to_owned().into(),
            source_key.schema.entity.as_str().to_owned().into(),
            i64::from(source_key.schema.version.get()).into(),
            source_key.entity_id.into(),
            rust_decimal::Decimal::from(source_version).into(),
            link_name.as_str().to_owned().into(),
            i64::from(ordinal).into(),
            target.schema.module.as_str().to_owned().into(),
            target.schema.entity.as_str().to_owned().into(),
            i64::from(target.schema.version.get()).into(),
            target.entity_id.into(),
        ],
    )
    .await
}

pub async fn replace_materialized_link_target(
    database: &TestDatabase,
    source_key: &EntityKey,
    source_version: u64,
    link_name: &LinkName,
    ordinal: u32,
    target: &LinkedEntityKey,
) -> TestResult<()> {
    let db = database.connection().await?;
    let updated = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE index_links SET target_module = $9, target_entity = $10, target_schema_version = $11, target_entity_id = $12, target_locale_key = '' WHERE tenant_id = $1 AND source_module = $2 AND source_entity = $3 AND source_schema_version = $4 AND source_entity_id = $5 AND source_locale_key = '' AND source_version = $6 AND link_name = $7 AND ordinal = $8",
            vec![
                source_key.tenant_id.into(),
                source_key.schema.module.as_str().to_owned().into(),
                source_key.schema.entity.as_str().to_owned().into(),
                i64::from(source_key.schema.version.get()).into(),
                source_key.entity_id.into(),
                rust_decimal::Decimal::from(source_version).into(),
                link_name.as_str().to_owned().into(),
                i64::from(ordinal).into(),
                target.schema.module.as_str().to_owned().into(),
                target.schema.entity.as_str().to_owned().into(),
                i64::from(target.schema.version.get()).into(),
                target.entity_id.into(),
            ],
        ))
        .await?;
    if updated.rows_affected() != 1 {
        return Err(std::io::Error::other("fixture link substitution lost scope").into());
    }
    Ok(())
}

pub async fn table_exists(database: &TestDatabase, table: &str) -> TestResult<bool> {
    let db = database.connection().await?;
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NOT NULL AS value",
            vec![table.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("table existence query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn count_value(
    database: &TestDatabase,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<i64> {
    let db = database.connection().await?;
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("count query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn schema_ref(entity: &str) -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("repair-evidence").expect("static module"),
        entity: EntityName::new(entity).expect("static entity"),
        version: SchemaVersion::new(1),
    }
}

fn uuid_field(name: FieldName, nullable: bool) -> IndexField {
    IndexField {
        name,
        value_type: IndexValueType::Uuid,
        cardinality: FieldCardinality::One,
        nullable,
        selectable: true,
        filterable: true,
        sortable: true,
    }
}

fn fixture_source_failure() -> IndexSourceFailure {
    IndexSourceFailure::permanent("repair_evidence_fixture_contract")
        .expect("static source failure is valid")
}

fn missing_digest(
    key: &EntityKey,
    indexed_source_version: u64,
    absence_source_version: u64,
    state: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, MISSING_EVIDENCE_DOMAIN);
    hash_component(&mut hasher, state);
    hash_entity_key(&mut hasher, key);
    hash_component(&mut hasher, &indexed_source_version.to_be_bytes());
    hash_component(&mut hasher, &absence_source_version.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn orphan_identity_digest(
    link_name: &LinkName,
    ordinal: u32,
    target: &LinkedEntityKey,
    absence_source_version: u64,
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, ORPHAN_IDENTITY_DOMAIN);
    hash_component(&mut hasher, link_name.as_str().as_bytes());
    hash_component(&mut hasher, &ordinal.to_be_bytes());
    hash_linked_key(&mut hasher, target);
    hash_component(&mut hasher, &absence_source_version.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn orphan_digest(
    source_key: &EntityKey,
    indexed_source_version: u64,
    link_name: &LinkName,
    ordinal: u32,
    target: &LinkedEntityKey,
    absence_source_version: u64,
    state: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, ORPHAN_EVIDENCE_DOMAIN);
    hash_component(&mut hasher, state);
    hash_entity_key(&mut hasher, source_key);
    hash_component(&mut hasher, &indexed_source_version.to_be_bytes());
    hash_component(&mut hasher, link_name.as_str().as_bytes());
    hash_component(&mut hasher, &ordinal.to_be_bytes());
    hash_linked_key(&mut hasher, target);
    hash_component(&mut hasher, &absence_source_version.to_be_bytes());
    hex::encode(hasher.finalize())
}

fn hash_entity_key(hasher: &mut Sha256, key: &EntityKey) {
    hash_component(hasher, key.tenant_id.as_bytes());
    hash_schema(hasher, &key.schema);
    hash_component(hasher, key.entity_id.as_bytes());
    hash_component(hasher, b"no_locale");
}

fn hash_linked_key(hasher: &mut Sha256, key: &LinkedEntityKey) {
    hash_schema(hasher, &key.schema);
    hash_component(hasher, key.entity_id.as_bytes());
    hash_component(hasher, b"no_locale");
}

fn hash_schema(hasher: &mut Sha256, schema: &SchemaRef) {
    hash_component(hasher, schema.module.as_str().as_bytes());
    hash_component(hasher, schema.entity.as_str().as_bytes());
    hash_component(hasher, &schema.version.get().to_be_bytes());
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .expect("bounded fixture digest component")
            .to_be_bytes(),
    );
    hasher.update(value);
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}
