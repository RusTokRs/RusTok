use std::error::Error;

use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventEnvelope, ForumSearchProjectionEvent,
};
use rustok_search::{
    ForumSearchContractIngress, ForumSearchContractIngressError,
    ForumSearchContractIngressOutcome, SearchModule,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult,
    Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const FORUM_SOURCE_MODULE: &str = "forum";

struct PostgresSearchTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum versioned invalidation ingress evidence"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_search_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;

        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;

        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InboxSnapshot {
    tenant_id: Uuid,
    source_module: String,
    scope_key: String,
    event_type: String,
    ingest_sequence: i64,
    envelope_json: JsonValue,
}

#[tokio::test]
async fn legacy_first_then_typed_restart_reuses_one_exact_root_row() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_contract_legacy_first").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let owner_revision = 9_000_001_i64;
    let target_type = "forum_category";
    let target_id = Some(category_id);
    let scope_key = format!("forum_category:{category_id}");
    let root = legacy_root_envelope(tenant_id, root_event_id, target_type, target_id)?;
    insert_legacy_root(&test_db.db, &root, &scope_key).await?;
    let before = load_snapshot(&test_db.db, root_event_id).await?;

    let typed = typed_invalidation(
        tenant_id,
        root_event_id,
        owner_revision,
        target_type,
        target_id,
    )?;
    let first = ForumSearchContractIngress::new(test_db.db.clone())
        .ingest(&typed)
        .await?;
    assert_eq!(
        first,
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id,
            owner_revision,
        }
    );

    let after_first = load_snapshot(&test_db.db, root_event_id).await?;
    assert_eq!(after_first, before);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    let restarted = ForumSearchContractIngress::new(test_db.db.clone());
    let redelivery = restarted.ingest(&typed).await?;
    assert_eq!(redelivery, first);
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, before);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn typed_first_then_legacy_delivery_keeps_search_owned_sequence() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_contract_typed_first").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let owner_revision = 8_000_003_i64;
    let target_type = "forum_topic";
    let target_id = Some(Uuid::new_v4());
    let typed = typed_invalidation(
        tenant_id,
        root_event_id,
        owner_revision,
        target_type,
        target_id,
    )?;

    let accepted = ForumSearchContractIngress::new(test_db.db.clone())
        .ingest(&typed)
        .await?;
    assert!(matches!(
        accepted,
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id: accepted_id,
            owner_revision: accepted_revision,
        } if accepted_id == root_event_id && accepted_revision == owner_revision
    ));

    let typed_first = load_snapshot(&test_db.db, root_event_id).await?;
    assert_eq!(typed_first.source_module, FORUM_SOURCE_MODULE);
    assert_eq!(typed_first.scope_key, "forum");
    assert_eq!(typed_first.event_type, ROOT_EVENT_TYPE);
    assert!(typed_first.ingest_sequence > 0);
    assert_ne!(typed_first.ingest_sequence, owner_revision);

    let root = legacy_root_envelope(tenant_id, root_event_id, target_type, target_id)?;
    insert_legacy_root(&test_db.db, &root, "forum").await?;
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, typed_first);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    let restarted = ForumSearchContractIngress::new(test_db.db.clone());
    restarted.ingest(&typed).await?;
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, typed_first);

    test_db.cleanup().await
}

#[tokio::test]
async fn conflicting_legacy_identity_is_non_retryable_semantic_poison() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_contract_conflict").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let requested_category_id = Uuid::new_v4();
    let conflicting_category_id = Uuid::new_v4();
    let conflicting_root = legacy_root_envelope(
        tenant_id,
        root_event_id,
        "forum_category",
        Some(conflicting_category_id),
    )?;
    insert_legacy_root(
        &test_db.db,
        &conflicting_root,
        &format!("forum_category:{conflicting_category_id}"),
    )
    .await?;
    let before = load_snapshot(&test_db.db, root_event_id).await?;

    let typed = typed_invalidation(
        tenant_id,
        root_event_id,
        42,
        "forum_category",
        Some(requested_category_id),
    )?;
    let error = ForumSearchContractIngress::new(test_db.db.clone())
        .ingest(&typed)
        .await
        .expect_err("conflicting root identity must fail closed");
    assert_eq!(error, ForumSearchContractIngressError::InboxIdentityConflict);
    assert_eq!(
        error.stable_code(),
        "forum.search_projection.contract_inbox_identity_conflict"
    );
    assert!(!error.is_retryable());
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, before);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    test_db.cleanup().await
}

fn typed_invalidation(
    tenant_id: Uuid,
    root_event_id: Uuid,
    owner_revision: i64,
    target_type: &str,
    target_id: Option<Uuid>,
) -> TestResult<ContractEventEnvelope> {
    Ok(ContractEventEnvelope::new_caused_by(
        tenant_id,
        None,
        root_event_id,
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision,
            target_type: target_type.to_string(),
            target_id,
        },
    )?)
}

fn legacy_root_envelope(
    tenant_id: Uuid,
    root_event_id: Uuid,
    target_type: &str,
    target_id: Option<Uuid>,
) -> TestResult<EventEnvelope> {
    let envelope = EventEnvelope {
        id: root_event_id,
        event_type: ROOT_EVENT_TYPE.to_string(),
        schema_version: 1,
        correlation_id: root_event_id,
        causation_id: None,
        tenant_id,
        trace_id: None,
        timestamp: Utc::now(),
        actor_id: None,
        event: DomainEvent::ReindexRequested {
            target_type: target_type.to_string(),
            target_id,
        },
        retry_count: 0,
    };
    envelope.validate_registered_schema()?;
    Ok(envelope)
}

async fn insert_legacy_root(
    db: &DatabaseConnection,
    envelope: &EventEnvelope,
    scope_key: &str,
) -> Result<(), sea_orm::DbErr> {
    let envelope_json = serde_json::to_value(envelope)
        .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO search_projection_inbox (
            event_id, tenant_id, source_module, scope_key, event_type,
            revision_at, envelope_json
        ) VALUES ($1, $2, $3"ÂCBÂCRÂCbÂCr¢ôâ4ôädÄ”5B†WfVçEö–B’DòäõD„”är"À¢fV5°¢VçfVÆ÷Ræ–Bæ–çFò‚’À¢VçfVÆ÷RçFVæçEö–Bæ–çFò‚’À¢dõ%TÕõ4õU$4UôÔôETÄRæ–çFò‚’À¢66÷Uö¶W’çFõ÷7G&–ær‚’æ–çFò‚’À¢$ôõEôUdTåEõE•Ræ–çFò‚’À¢VçfVÆ÷RçF–ÖW7F×æ–çFò‚’À¢7ÅfÇVS£¤§6öâ…6öÖR„&÷ƒ£¦æWr†VçfVÆ÷Uö§6öâ’’’À¢ÒÀ¢’¢æv—Có°¢ö²‚‚’§Ğ ¦7–æ2fâÆöE÷6æ6†÷B€¢F#¢dFF&6T6öææV7F–öâÀ¢&ö÷EöWfVçEö–C¢WV–BÀ¢’Óâ&W7VÇCÄ–æ&÷…6æ6†÷BÂ6Vö÷&Ó£¤F$W'#â°¢ÆWB&÷rÒF ¢çVW'•ööæR…7FFVÖVçC£¦g&öÕ÷7ÅöæE÷fÇVW2€¢F$&6¶VæC£¥÷7Fw&W2À¢"2 ¢4TÄT5BFVæçEö–BÂ6÷W&6UöÖöGVÆRÂ66÷Uö¶W’ÂWfVçE÷G—RÀ¢–ævW7E÷6WVVæ6RÂVçfVÆ÷Uö§6öà¢e$ôÒ6V&6…÷&ö¦V7F–öåö–æ&÷€¢t„U$RWfVçEö–BÒC¢"2À¢fV6·&ö÷EöWfVçEö–Bæ–çFò‚•ÒÀ¢’¢æv—Cğ¢æW‡V7B‚&W‡V7FVBGW&&ÆRf÷'VÒ6V&6‚–æ&÷‚&÷r"“°¢6æ6†÷Eög&öÕ÷&÷r‚g&÷r§Ğ ¦fâ6æ6†÷Eög&öÕ÷&÷r‡&÷s¢eVW'•&W7VÇB’Óâ&W7VÇCÄ–æ&÷…6æ6†÷BÂ6Vö÷&Ó£¤F$W'#â°¢ö²„–æ&÷…6æ6†÷B°¢FVæçEö–C¢&÷rçG'•övWB‚""Â'FVæçEö–B"“òÀ¢6÷W&6UöÖöGVÆS¢&÷rçG'•övWB‚""Â'6÷W&6UöÖöGVÆR"“òÀ¢66÷Uö¶W“¢&÷rçG'•övWB‚""Â'66÷Uö¶W’"“òÀ¢WfVçE÷G—S¢&÷rçG'•övWB‚""Â&WfVçE÷G—R"“òÀ¢–ævW7E÷6WVVæ6S¢&÷rçG'•övWB‚""Â&–ævW7E÷6WVVæ6R"“òÀ¢VçfVÆ÷Uö§6öã¢&÷rçG'•övWB‚""Â&VçfVÆ÷Uö§6öâ"“òÀ¢Ò§Ğ ¦7–æ2fâ6÷VçE÷&ö÷E÷&÷w2€¢F#¢dFF&6T6öææV7F–öâÀ¢&ö÷EöWfVçEö–C¢WV–BÀ¢’Óâ&W7VÇCÆ“cBÂ6Vö÷&Ó£¤F$W'#â°¢ÆWB&÷rÒF ¢çVW'•ööæR…7FFVÖVçC£¦g&öÕ÷7ÅöæE÷fÇVW2€¢F$&6¶VæC£¥÷7Fw&W2À¢%4TÄT5B4õTåB‚¢“£¦&–v–çB26÷VçBe$ôÒ6V&6…÷&ö¦V7F–öåö–æ&÷‚t„U$RWfVçEö–BÒC"À¢fV5·&ö÷EöWfVçEö–Bæ–çFò‚•ÒÀ¢’¢æv—Cğ¢æW‡V7B‚&6÷VçBVW'’&WGW&ç2öæR&÷r"“°¢&÷rçG'•övWB‚""Â&6÷VçB"§Ğ ¦fâ÷7Fw&W5öFF&6U÷W&Â‚’Óâ÷F–öãÅ7G&–æsâ°¢7FC£¦Vçc£§f"…4T$4…õDU5EôDD$4UôTåb¢æ÷%öVÇ6R‡Å÷Â7FC£¦Vçc£§f"‚$DD$4UõU$Â"’¢æö²‚¢æf–ÇFW"‡ÇW&ÇÂW&Âç7F'G5÷v—F‚‚'÷7Fw&W3¢òò"’ÇÂW&Âç7F'G5÷v—F‚‚'÷7Fw&W7Ã¢òò"’§Ğ ¦7–æ2fâ6öææV7B†FF&6U÷W&Ã¢g7G"’ÓâFW7E&W7VÇCÄFF&6T6öææV7F–öãâ°¢ÆWB×WB÷F–öç2Ò6öææV7D÷F–öç3£¦æWr†FF&6U÷W&ÂçFõö÷væVB‚’“°¢÷F–öç0¢æÖ…ö6öææV7F–öç2ƒ¢æÖ–åö6öææV7F–öç2ƒ¢ç7Ç…öÆövv–ær†fÇ6R“°¢ö²„FF&6S£¦6öææV7B†÷F–öç2’æv—Cò§Ğ ¦7–æ2fâ6WE÷6V&6…÷F‚†F#¢dFF&6T6öææV7F–öâÂ66†VÖöæÖS¢g7G"’ÓâFW7E&W7VÇCÂ‚“â°¢F"æW†V7WFU÷Vç&W&VB‚ff÷&ÖB‡"2%4UB6V&6…÷F‚Dò'·66†VÖöæÖWÒ"ÂV&Æ–2"2’¢æv—Có°¢ö²‚‚’§Ğ ¦fâ6æ—F—¦Uö–FVçF–f–W"‡fÇVS¢g7G"’Óâ7G&–ær°¢ÆWBæ÷&ÖÆ—¦VBÒfÇVP¢æ6†'2‚¢æÖ‡Æ6†&7FW'Â°¢–b6†&7FW"æ—5ö66–•öÇ†çVÖW&–2‚’°¢6†&7FW"çFõö66–•öÆ÷vW&66R‚¢ÒVÇ6R°¢uòp¢Ğ¢Ò¢æ6öÆÆV7C££Å7G&–æsâ‚“°¢ÆWBæ÷&ÖÆ—¦VBÒæ÷&ÖÆ—¦VBçG&–ÕöÖF6†W2‚uòr“°¢–bæ÷&ÖÆ—¦VBæ—5öV×G’‚’°¢'FW7B"çFõ÷7G&–ær‚¢ÒVÇ6R°¢æ÷&ÖÆ—¦VBçFõ÷7G&–ær‚¢Ğ§Ğ 