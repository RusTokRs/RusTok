use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::{TransactionTrait, sqlx};
use serde_json::{Value, json};

use super::*;
use crate::{
    IndexQueryExecutionError, PostgresIndexQueryAdmissionCatalog, PostgresQueryEntityAdmission,
    PostgresSecondaryIndexManager, SecondaryIndexClaimOutcome, SecondaryIndexError,
    SecondaryIndexOperation, SecondaryIndexPlan, SecondaryIndexRequest,
};

async fn save(
    db: &DatabaseConnection,
    registry: &SchemaRegistry,
    record: IndexRecord,
) -> TestResult<()> {
    let delivery = MutationDelivery::from_event(
        "query-hardening",
        IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record,
        },
    )?;
    PostgresMutationStore::new(db.clone())
        .apply(registry, &delivery)
        .await?;
    Ok(())
}

fn timestamp(value: &str) -> IndexValue {
    IndexValue::Timestamp(DateTime::<Utc>::from_str(value).unwrap())
}

#[tokio::test]
async fn postgres_timestamp_and_decimal_keys_match_reference_through_null_pages() -> TestResult<()>
{
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let schema = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field(
                "id",
                IndexValueType::Uuid,
                FieldCardinality::One,
                false,
                true,
            ),
            field(
                "stamp",
                IndexValueType::Timestamp,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "amount",
                IndexValueType::Decimal,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "rates",
                IndexValueType::Decimal,
                FieldCardinality::Many,
                false,
                false,
            ),
        ],
        links: vec![],
    };
    let mut registry = SchemaRegistry::new();
    registry.register(schema.clone())?;
    let registry = Arc::new(registry);
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, &schema)
        .await?;
    let decimal = |value: &str| IndexValue::Decimal(Decimal::from_str(value).unwrap());
    let mut records = Vec::new();
    let earlier = "2026-08-01T10:11:12.000000001Z";
    let later = "2026-08-01T10:11:12.000000002Z";
    for (id, stamp, amount) in [
        (
            1,
            timestamp(later),
            decimal("1000000000000000000.000000002"),
        ),
        (
            2,
            timestamp(earlier),
            decimal("1000000000000000000.000000001"),
        ),
        (
            3,
            timestamp("2026-08-01T10:11:12Z"),
            decimal("-0.000000001"),
        ),
        (4, IndexValue::Null, IndexValue::Null),
    ] {
        let id = Uuid::from_u128(id);
        let record = IndexRecord {
            key: EntityKey {
                tenant_id: TENANT,
                schema: schema.reference.clone(),
                entity_id: id,
                locale: Some(locale()),
            },
            source_version: 1,
            fields: BTreeMap::from([
                (field_name("id"), IndexValue::Uuid(id)),
                (field_name("stamp"), stamp),
                (field_name("amount"), amount),
                (
                    field_name("rates"),
                    IndexValue::List(vec![decimal("0.10000")]),
                ),
            ]),
            links: vec![],
        };
        save(&test.db, &registry, record.clone()).await?;
        records.push(record);
    }
    let port = PostgresIndexQueryPort::new(test.db.clone(), registry.clone());
    let reference = ReferenceFixture::new(&registry, &records);
    for field in ["amount", "stamp"] {
        for direction in [OrderDirection::Asc, OrderDirection::Desc] {
            let mut q = query(
                vec![root_path("id"), root_path("stamp"), root_path("amount")],
                Some(FilterExpr::Contains(root_path("rates"), decimal("0.1"))),
                vec![OrderExpr {
                    field: root_path(field),
                    direction,
                }],
            );
            let mut after = None;
            for page in 0..4 {
                q.pagination = Pagination::Cursor { first: 1, after };
                let result = assert_equivalent(&port, &reference, q.clone()).await?;
                assert_eq!(result.exact_count, Some(4));
                assert_eq!(result.items.len(), 1);
                after = result.next_cursor;
                assert_eq!(after.is_some(), page < 3);
            }
        }
    }
    for filter in [
        FilterExpr::Eq(root_path("stamp"), timestamp(earlier)),
        FilterExpr::Not(Box::new(FilterExpr::Lt(
            root_path("stamp"),
            timestamp(later),
        ))),
    ] {
        assert_equivalent(
            &port,
            &reference,
            query(vec![root_path("id")], Some(filter), vec![]),
        )
        .await?;
    }
    test.cleanup().await
}

#[tokio::test]
async fn postgres_nested_locale_identity_and_target_admission_agree_in_page_and_count()
-> TestResult<()> {
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    for schema in &fixture.schemas {
        PostgresSchemaRegistrationStore::new(test.db.clone())
            .register(TENANT, schema)
            .await?;
    }
    let mut records = fixture.records;
    let mut translated = records
        .iter()
        .find(|record| record.key.entity_id == Uuid::from_u128(11))
        .unwrap()
        .clone();
    translated.key.locale = Some(LocaleKey::new("fr")?);
    let root_id = Uuid::from_u128(101);
    let root = records
        .iter_mut()
        .find(|record| record.key.entity_id == root_id)
        .unwrap();
    root.links
        .iter_mut()
        .find(|link_value| link_value.name == link("variants"))
        .unwrap()
        .targets
        .push(LinkedEntityKey {
            schema: translated.key.schema.clone(),
            entity_id: translated.key.entity_id,
            locale: translated.key.locale.clone(),
        });
    records.push(translated);
    for record in records.iter().cloned() {
        save(&test.db, &fixture.registry, record).await?;
    }
    let mut admissions = PostgresIndexQueryAdmissionCatalog::new();
    admissions.require_current_link_targets("catalog", schema_ref("product"))?;
    admissions.register(
        "catalog",
        schema_ref("variant"),
        PostgresQueryEntityAdmission::new("{{entity}}.source_version < 2")?,
    )?;
    admissions.ensure_runtime_schema(schema_ref("product"))?;
    let port = PostgresIndexQueryPort::with_admissions(
        test.db.clone(),
        fixture.registry.clone(),
        admissions,
    );
    let reference = ReferenceFixture::new(&fixture.registry, &records);
    let filter = Some(FilterExpr::Eq(root_path("id"), IndexValue::Uuid(root_id)));
    let linked_query = query(
        vec![root_path("id"), linked_path(&["variants"], "title")],
        filter.clone(),
        vec![],
    );
    let initial = assert_equivalent(&port, &reference, linked_query.clone()).await?;
    assert_eq!(initial.exact_count, Some(1));
    let translations = initial.items[0].nested_relations[0]
        .items
        .iter()
        .filter_map(|item| item.relations.first())
        .filter(|relation| relation.entity_id == Some(Uuid::from_u128(11)))
        .map(|relation| relation.locale.as_ref().unwrap().as_str())
        .collect::<Vec<_>>();
    assert_eq!(translations, ["en-US", "fr"]);

    test.db.execute_unprepared("UPDATE index_entities SET source_version=2 WHERE entity_name='variant' AND locale_key='fr'").await?;
    let excluded = port.execute_query(linked_query.clone()).await?;
    assert_eq!(excluded.exact_count, Some(0));
    assert!(excluded.items.is_empty());
    test.db.execute_unprepared("UPDATE index_entities SET source_version=1, schema_fingerprint=repeat('0',64) WHERE entity_name='variant' AND locale_key='fr'").await?;
    let excluded = port.execute_query(linked_query).await?;
    assert_eq!(excluded.exact_count, Some(0));
    assert!(excluded.items.is_empty());
    assert_eq!(
        port.execute_query(query(vec![root_path("id")], filter, vec![]))
            .await?
            .exact_count,
        Some(1)
    );
    test.cleanup().await
}

#[tokio::test]
async fn postgres_nested_projection_over_budget_returns_no_partial_page() -> TestResult<()> {
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    let registration = PostgresSchemaRegistrationStore::new(test.db.clone());
    for schema in &fixture.schemas {
        registration.register(TENANT, schema).await?;
    }
    let mut record = fixture
        .records
        .iter()
        .find(|r| r.key.entity_id == Uuid::from_u128(11))
        .unwrap()
        .clone();
    record.fields.insert(
        field_name("title"),
        IndexValue::String("x".repeat(132 * 1024)),
    );
    save(&test.db, &fixture.registry, record).await?;
    let root = fixture
        .records
        .iter()
        .find(|r| r.key.entity_id == Uuid::from_u128(101))
        .unwrap()
        .clone();
    save(&test.db, &fixture.registry, root).await?;
    let port = PostgresIndexQueryPort::new(test.db.clone(), fixture.registry);
    let over_budget = port
        .execute_query(query(
            vec![root_path("id"), linked_path(&["variants"], "title")],
            None,
            vec![],
        ))
        .await;
    assert!(matches!(
        over_budget,
        Err(IndexQueryExecutionError::BudgetExceeded {
            stage: "nested projection"
        })
    ));
    test.cleanup().await
}

fn has_index(node: &Value, name: &str) -> bool {
    match node {
        Value::Object(entries) => {
            entries.get("Index Name").and_then(Value::as_str) == Some(name)
                || entries.values().any(|child| has_index(child, name))
        }
        Value::Array(entries) => entries.iter().any(|child| has_index(child, name)),
        _ => false,
    }
}

#[tokio::test]
async fn postgres_compiled_queries_use_owned_btree_and_gin_in_custom_and_generic_plans()
-> TestResult<()> {
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let schema = IndexSchema {
        reference: schema_ref("product"),
        locale_mode: LocaleMode::Required,
        fields: vec![
            field(
                "id",
                IndexValueType::Uuid,
                FieldCardinality::One,
                false,
                false,
            ),
            field(
                "score",
                IndexValueType::Integer,
                FieldCardinality::One,
                false,
                true,
            ),
            field(
                "tags",
                IndexValueType::String,
                FieldCardinality::Many,
                false,
                false,
            ),
        ],
        links: vec![],
    };
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, &schema)
        .await?;
    let mut registry = SchemaRegistry::new();
    registry.register(schema.clone())?;
    let fingerprint = schema.fingerprint()?;
    // Dense common tags, sparse sought tags. IDs and scores intentionally do not correlate.
    test.db.execute_unprepared(&format!(
        "INSERT INTO index_entities (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key,
                source_version, schema_fingerprint, record_fingerprint, payload)
         SELECT '{TENANT}', 'catalog', 'product', 1, md5(n::text)::uuid, '{LOCALE}', 1,
                '{fingerprint}', repeat('0',64), jsonb_build_object(
                    'score', jsonb_build_object('type','integer','value',n),
                    'id', jsonb_build_object('type','uuid','value',md5(n::text)::uuid),
                    'tags', jsonb_build_object('type','list','value',jsonb_build_array(jsonb_build_object(
                        'type','string','value', CASE WHEN n % 5000 = 0 THEN 'rare' ELSE 'common' END))))
         FROM generate_series(1, 20000) n"
    )).await?;
    let plan = SecondaryIndexPlan::from_schema(TENANT, &schema)?;
    let manager = PostgresSecondaryIndexManager::new(test.db.clone());
    let field_spec = |field: &str| {
        plan.indexes()
            .iter()
            .find(|spec| spec.field_name().as_str() == field)
            .unwrap()
    };
    for field in ["score", "tags"] {
        let req = SecondaryIndexRequest::new(
            field_spec(field).clone(),
            SecondaryIndexOperation::Ensure,
            "m4-plan-regression",
            Duration::from_secs(60),
        )?;
        assert!(manager.operate(&req).await?.is_some());
    }
    test.db.execute_unprepared("ANALYZE index_entities").await?;
    let range = registry.compile_postgres_query(&query(
        vec![root_path("id")],
        Some(FilterExpr::Gt(
            root_path("score"),
            IndexValue::Integer(19990),
        )),
        vec![OrderExpr {
            field: root_path("score"),
            direction: OrderDirection::Asc,
        }],
    ))?;
    let membership = registry.compile_postgres_query(&query(
        vec![root_path("id")],
        Some(FilterExpr::Contains(
            root_path("tags"),
            IndexValue::String("rare".to_owned()),
        )),
        vec![],
    ))?;
    let membership_count = membership.exact_count.expect("exact count requested");
    let tx = test.db.begin().await?;
    tx.execute_unprepared(&format!("PREPARE score_query AS {}", range.sql))
        .await?;
    // A small ordered page can favor a btree over GIN plus sorting. Use the real
    // exact-count query to verify containment index use without that competing order.
    tx.execute_unprepared(&format!("PREPARE tags_count AS {}", membership_count.sql))
        .await?;
    let count = r#"tags_count('en-US', '[{"type":"string","value":"rare"}]'::jsonb)"#;
    for mode in ["force_custom_plan", "force_generic_plan"] {
        tx.execute_unprepared(&format!("SET LOCAL plan_cache_mode = {mode}"))
            .await?;
        for (field, execution) in [
            ("score", "score_query('en-US', 19990, 10)"),
            ("tags", count),
        ] {
            let row = tx
                .query_one_raw(Statement::from_string(
                    DbBackend::Postgres,
                    format!("EXPLAIN (FORMAT JSON) EXECUTE {execution}"),
                ))
                .await?
                .unwrap();
            let plan: Value = row.try_get("", "QUERY PLAN")?;
            assert!(
                has_index(&plan, field_spec(field).index_name()),
                "{mode} {field}: {plan}"
            );
        }
        let row = tx
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("EXECUTE {count}"),
            ))
            .await?
            .expect("exact count row");
        assert_eq!(row.try_get::<i64>("", "__exact_count")?, 4, "{mode}");
    }
    tx.execute_unprepared("DEALLOCATE score_query; DEALLOCATE tags_count")
        .await?;
    tx.commit().await?;
    test.cleanup().await
}

#[tokio::test]
async fn postgres_ensure_only_recovers_unmarked_create_with_original_job_intent() -> TestResult<()>
{
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    let schema = fixture
        .schemas
        .iter()
        .find(|schema| schema.reference == schema_ref("product"))
        .unwrap();
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, schema)
        .await?;
    let plan = SecondaryIndexPlan::from_schema(TENANT, schema)?;
    let spec = plan
        .indexes()
        .iter()
        .find(|spec| spec.field_name().as_str() == "score")
        .unwrap();
    let request = |worker| {
        SecondaryIndexRequest::new(
            spec.clone(),
            SecondaryIndexOperation::Ensure,
            worker,
            Duration::from_secs(60),
        )
    };
    let manager = PostgresSecondaryIndexManager::new(test.db.clone());
    let SecondaryIndexClaimOutcome::Acquired(lease) = manager.claim(&request("first")?).await?
    else {
        panic!("ensure did not acquire original job");
    };
    test.db
        .execute_unprepared(&spec.create_statement(DbBackend::Postgres)?)
        .await?;
    assert!(matches!(
        manager.execute(&lease).await,
        Err(SecondaryIndexError::IndexOwnershipConflict { .. })
    ));
    // Simulate the durable boundary after a reserved CREATE succeeds, before the ownership
    // marker and success transition. The same definition without its reservation is refused above.
    let pool = test.db.get_postgres_connection_pool();
    let oid: i64 = sqlx::query_scalar("SELECT 'index_entities'::regclass::bigint")
        .fetch_one(pool)
        .await?;
    sqlx::query("UPDATE index_jobs SET cursor=$1, lease_expires_at=clock_timestamp() - INTERVAL '1 second' WHERE tenant_id=$2 AND job_id=$3")
        .bind(json!({"phase":"creating_index", "table_oid":oid, "definition_hash":spec.definition_hash()}))
        .bind(TENANT).bind(lease.job_id()).execute(pool).await?;
    let retire = SecondaryIndexRequest::new(
        spec.clone(),
        SecondaryIndexOperation::Retire,
        "retire-during-ensure",
        Duration::from_secs(60),
    )?;
    assert!(matches!(
        manager.claim(&retire).await?,
        SecondaryIndexClaimOutcome::Busy
    ));
    let SecondaryIndexClaimOutcome::Acquired(reclaimed) =
        manager.claim(&request("second")?).await?
    else {
        panic!("existing job must be reclaimable");
    };
    assert_eq!(reclaimed.job_id(), lease.job_id());
    assert_eq!(reclaimed.attempt_count(), lease.attempt_count() + 1);
    assert!(matches!(
        manager.execute(&reclaimed).await?,
        crate::SecondaryIndexExecutionOutcome::Ready { created: false, .. }
    ));
    manager.succeed(&reclaimed).await?;
    test.cleanup().await
}

#[tokio::test]
async fn dropped_postgres_ensure_can_be_reclaimed_without_reusing_its_ddl_session() -> TestResult<()>
{
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    let schema = fixture
        .schemas
        .iter()
        .find(|schema| schema.reference == schema_ref("product"))
        .unwrap();
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, schema)
        .await?;
    let plan = SecondaryIndexPlan::from_schema(TENANT, schema)?;
    let spec = plan
        .indexes()
        .iter()
        .find(|spec| spec.field_name().as_str() == "score")
        .unwrap();
    let request = |worker| {
        SecondaryIndexRequest::new(
            spec.clone(),
            SecondaryIndexOperation::Ensure,
            worker,
            Duration::from_secs(90),
        )
    };
    let manager = PostgresSecondaryIndexManager::new(test.db.clone());
    let pool = test.db.get_postgres_connection_pool();
    let blocker = test.db.begin().await?;
    // Permit read-only definition verification, but hold CREATE INDEX CONCURRENTLY at
    // its physical table lock after the job has durably recorded creation intent.
    blocker
        .execute_unprepared("LOCK TABLE index_entities IN SHARE UPDATE EXCLUSIVE MODE")
        .await?;
    let first = request("interrupted-ensure")?;
    let task = tokio::spawn({
        let manager = manager.clone();
        async move { manager.operate(&first).await }
    });
    let observed = tokio::time::timeout(Duration::from_secs(10), async {
        let mut poll = tokio::time::interval(Duration::from_millis(20));
        loop {
            poll.tick().await;
            let blocked = sqlx::query_as::<_, (Uuid, i32)>(
                "SELECT j.job_id, l.pid FROM index_jobs AS j
                 JOIN pg_catalog.pg_locks AS l ON l.relation = to_regclass('index_entities')
                   AND l.mode = 'ShareUpdateExclusiveLock' AND NOT l.granted AND l.pid IS NOT NULL
                 WHERE j.tenant_id = $1 AND j.kind = 'secondary_index'
                   AND j.state = 'running' AND j.cursor->>'phase' = 'creating_index'",
            )
            .bind(TENANT)
            .fetch_optional(pool)
            .await?;
            if let Some(blocked) = blocked {
                return Ok::<_, sqlx::Error>(blocked);
            }
        }
    })
    .await;
    task.abort();
    assert!(
        task.await
            .expect_err("the DDL request was dropped while blocked")
            .is_cancelled()
    );
    blocker.rollback().await?;
    let (job_id, discarded_pid) = observed??;
    // A pool returning the cancelled connection (rather than closing it) would leave this
    // backend alive. Observe its exit independently of whether PostgreSQL finished CREATE.
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut poll = tokio::time::interval(Duration::from_millis(20));
        loop {
            poll.tick().await;
            let still_open: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_stat_activity WHERE pid = $1)",
            )
            .bind(discarded_pid)
            .fetch_one(pool)
            .await?;
            if !still_open {
                return Ok::<_, sqlx::Error>(());
            }
        }
    })
    .await??;

    // Simulate clock passage rather than waiting for the 90-second lease in this regression.
    let expired = sqlx::query(
        "UPDATE index_jobs SET lease_expires_at = clock_timestamp() - INTERVAL '1 second'
         WHERE tenant_id = $1 AND job_id = $2 AND state = 'running' AND attempt_count = 1",
    )
    .bind(TENANT)
    .bind(job_id)
    .execute(pool)
    .await?;
    assert_eq!(expired.rows_affected(), 1);
    let resumed = request("resumed-ensure")?;
    let SecondaryIndexClaimOutcome::Acquired(lease) = manager.claim(&resumed).await? else {
        panic!("the expired job must be reclaimable after the original DDL session exits");
    };
    assert_eq!(lease.job_id(), job_id);
    assert_eq!(lease.attempt_count(), 2);
    assert!(matches!(
        manager.execute(&lease).await?,
        crate::SecondaryIndexExecutionOutcome::Ready { .. }
    ));
    manager.succeed(&lease).await?;
    test.cleanup().await
}

#[tokio::test]
async fn postgres_locale_job_blocks_downgrade_without_weakening_its_constraint() -> TestResult<()> {
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let pool = test.db.get_postgres_connection_pool();
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO index_jobs (tenant_id, job_id, kind, scope_kind, module_name, entity_name, schema_version, locale_key, request, retry_epoch, failure_count)
         VALUES ($1, $2, 'rebuild', 'locale', 'catalog', 'product', 1, 'en-US', '{}'::jsonb, 2, 1)"
    ).bind(TENANT).bind(id).execute(pool).await?;
    let manager = SchemaManager::new(&test.db);
    let migrations = IndexModule.migrations();
    let locale_scope = migrations
        .last()
        .expect("the locale-scope migration is registered");
    assert_eq!(
        locale_scope.name(),
        "m20260808_000009_add_index_job_locale_scope"
    );
    assert!(locale_scope.down(&manager).await.is_err());
    let saved: (String, i32, i32) = sqlx::query_as(
        "SELECT locale_key, retry_epoch, failure_count FROM index_jobs WHERE tenant_id = $1 AND job_id = $2"
    ).bind(TENANT).bind(id).fetch_one(pool).await?;
    assert_eq!(saved, ("en-US".into(), 2, 1));
    // Locale key must remain mandatory for a locale-scoped job after the failed transition.
    assert!(
        sqlx::query("UPDATE index_jobs SET locale_key = NULL WHERE tenant_id = $1 AND job_id = $2")
            .bind(TENANT)
            .bind(id)
            .execute(pool)
            .await
            .is_err()
    );
    test.cleanup().await
}

struct EmptySource;

#[async_trait::async_trait]
impl crate::IndexSource for EmptySource {
    async fn scan(
        &self,
        request: crate::IndexSourceScanRequest,
    ) -> Result<crate::IndexSourcePage, crate::IndexSourceFailure> {
        Ok(crate::IndexSourcePage::new(&request, vec![], None).unwrap())
    }

    async fn load(
        &self,
        request: crate::IndexSourceLoadRequest,
    ) -> Result<crate::IndexSourceLoadBatch, crate::IndexSourceFailure> {
        Ok(crate::IndexSourceLoadBatch::new(&request, vec![]).unwrap())
    }
}

#[tokio::test]
async fn postgres_concurrent_reconciliation_requeue_keeps_generation_and_one_immutable_audit()
-> TestResult<()> {
    use super::super::source_job_recovery::PostgresIndexJobRecoveryStore;
    use crate::{
        INDEX_RECONCILIATION_WORKER, IndexReconciliationSchedulerPolicy,
        PostgresIndexReconciliationWorkAdapter,
    };
    use crate::{
        IndexJobRequeueOutcome, IndexJobRequeueRequest, materialize_index_schema_registry,
        materialize_index_source_registry, register_index_schema_source, register_index_source,
    };
    use rustok_runtime::{ModuleWorkHandler, ModuleWorkOutcome, ModuleWorkSource};
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let mut schema = Fixture::new()
        .schemas
        .into_iter()
        .find(|schema| schema.reference == schema_ref("product"))
        .unwrap();
    schema.links.clear();
    let mut extensions = rustok_core::ModuleRuntimeExtensions::default();
    let source = "catalog-product-hardening";
    register_index_schema_source(&mut extensions, "catalog", schema.clone())?;
    register_index_source(
        &mut extensions,
        "catalog",
        source,
        [schema.reference.clone()],
        EmptySource,
    )?;
    let schemas = materialize_index_schema_registry(&extensions)?.unwrap();
    let sources = materialize_index_source_registry(&extensions)?.unwrap();
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, &schema)
        .await?;
    let job = Uuid::new_v4();
    let request =
        json!({"contract":"index_reconciliation_job_v1","source_name":source,"pass_count":2});
    let pool = test.db.get_postgres_connection_pool();
    sqlx::query("INSERT INTO index_jobs (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name,
        schema_version, request, cursor, attempt_count, failure_count, retry_epoch, completed_at)
        VALUES ($1,$2,'reconcile','failed','schema','catalog','product',1,$3,'{}'::jsonb,5,3,2,CURRENT_TIMESTAMP)")
        .bind(TENANT).bind(job).bind(&request).execute(pool).await?;
    let store = PostgresIndexJobRecoveryStore::new(test.db.clone());
    let attempt = |actor| IndexJobRequeueRequest::new(TENANT, job, actor, "Owner repaired");
    let invocation = attempt(Uuid::new_v4())?;
    assert_eq!(
        store
            .requeue_replay(invocation.clone(), &sources, schemas.registry())
            .await?,
        IndexJobRequeueOutcome::NotFound
    );
    sqlx::query("UPDATE index_schemas SET status='retired'")
        .execute(pool)
        .await?;
    assert_eq!(
        store
            .requeue_reconciliation(invocation.clone(), &sources, schemas.registry())
            .await?,
        IndexJobRequeueOutcome::ScopeUnavailable
    );
    sqlx::query("UPDATE index_schemas SET status='active'")
        .execute(pool)
        .await?;
    let (one, two) = tokio::join!(
        store.requeue_reconciliation(invocation, &sources, schemas.registry()),
        store.requeue_reconciliation(attempt(Uuid::new_v4())?, &sources, schemas.registry())
    );
    let outcomes = [one?, two?];
    assert_eq!(outcomes.iter().filter(|outcome| matches!(outcome, IndexJobRequeueOutcome::Requeued { job_id, retry_epoch: 3, .. } if *job_id == job)).count(), 1);
    assert!(outcomes.contains(&IndexJobRequeueOutcome::NotFailed));
    let row: (String, i32, i32, i32, Value, Value) = sqlx::query_as(
        "SELECT state, attempt_count, failure_count, retry_epoch, request, cursor FROM index_jobs WHERE job_id=$1")
        .bind(job).fetch_one(pool).await?;
    assert_eq!((row.0.as_str(), row.1, row.2, row.3), ("pending", 5, 0, 3));
    assert_eq!(row.4, request);
    assert_eq!(row.5["completed_passes"], json!(0));
    let aud: (String, i64) =
        sqlx::query_as("SELECT MIN(job_kind), COUNT(*) FROM index_job_recovery_audits")
            .fetch_one(pool)
            .await?;
    assert_eq!(aud, ("reconcile".to_owned(), 1));

    // Verify the real Postgres discovery, claim and acknowledgment path after recovery.
    let adapter = PostgresIndexReconciliationWorkAdapter::new(
        test.db.clone(),
        sources,
        schemas.shared(),
        IndexReconciliationSchedulerPolicy::default(),
    );
    let item = adapter.claim(INDEX_RECONCILIATION_WORKER).await?.unwrap();
    assert_eq!(item.id, job);
    assert!(
        adapter
            .complete(&item, ModuleWorkOutcome::Completed)
            .await
            .is_err()
    );
    let result = adapter.execute(item.clone()).await?;
    assert_eq!(result, ModuleWorkOutcome::Completed);
    adapter.complete(&item, result).await?;
    let stale = |field: &str, value: i64| {
        let mut stale = item.clone();
        stale.payload[field] = json!(value);
        stale
    };
    assert!(
        adapter
            .complete(&stale("retry_epoch", 2), ModuleWorkOutcome::Completed)
            .await
            .is_err()
    );
    assert!(
        adapter
            .complete(&stale("lease_generation", 4), ModuleWorkOutcome::Completed)
            .await
            .is_err()
    );
    sqlx::query("UPDATE index_jobs SET cursor = jsonb_set(cursor, '{completed_passes}', '1') WHERE tenant_id = $1 AND job_id = $2")
        .bind(TENANT).bind(job).execute(pool).await?;
    assert!(
        adapter
            .complete(&item, ModuleWorkOutcome::Completed)
            .await
            .is_err()
    );
    for sql in [
        "UPDATE index_job_recovery_audits SET reason='changed'",
        "DELETE FROM index_job_recovery_audits",
        "TRUNCATE index_job_recovery_audits",
    ] {
        assert!(test.db.execute_unprepared(sql).await.is_err());
    }
    test.cleanup().await
}

#[tokio::test]
async fn postgres_mutation_and_checkpoint_rollback_if_lease_expires_during_write() -> TestResult<()>
{
    use super::super::mutation_store::JobMutationFence;
    use crate::{
        IndexReplayCheckpoint, IndexReplayCheckpointKey, IndexReplayCheckpointStore,
        IndexReplayJobAcquireOutcome, IndexReplayJobLeaseRequest, MutationStorageError,
        PostgresIndexReplayCheckpointStore, PostgresIndexReplayJobStore,
    };
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    let schema = fixture
        .schemas
        .iter()
        .find(|schema| schema.reference == schema_ref("channel"))
        .unwrap();
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, schema)
        .await?;
    let jobs = PostgresIndexReplayJobStore::new(test.db.clone());
    let source = "hardening-fence";
    let request = |worker| {
        IndexReplayJobLeaseRequest::new(
            TENANT,
            schema.reference.clone(),
            source,
            worker,
            Duration::from_secs(2),
        )
    };
    // Force a wall-clock expiry AFTER the job row has been locked and the storage write has run.
    // CURRENT_TIMESTAMP would still say the lease is valid at the second guard in this transaction.
    test.db.execute_unprepared("CREATE FUNCTION wait_for_job_expiry() RETURNS trigger AS $$
        BEGIN
            PERFORM pg_sleep_until(lease_expires_at + INTERVAL '20 milliseconds')
              FROM index_jobs WHERE tenant_id=NEW.tenant_id AND kind='rebuild' AND state='running';
            RETURN NEW;
        END; $$ LANGUAGE plpgsql;
        CREATE TRIGGER delay_entity AFTER INSERT ON index_entities FOR EACH ROW EXECUTE FUNCTION wait_for_job_expiry();
        CREATE TRIGGER delay_checkpoint AFTER INSERT ON index_checkpoints FOR EACH ROW EXECUTE FUNCTION wait_for_job_expiry();").await?;
    let IndexReplayJobAcquireOutcome::Acquired(first) = jobs.acquire(&request("first")?).await?
    else {
        panic!("first lease");
    };
    let store = PostgresMutationStore::new(test.db.clone()).with_job_fence(JobMutationFence {
        tenant_id: TENANT,
        job_id: first.job_id(),
        schema: schema.reference.clone(),
        locale: None,
        source_name: source.into(),
        worker_id: first.worker_id().into(),
        attempt_count: first.attempt_count(),
        kind: "rebuild",
    });
    let delivery = MutationDelivery::from_event(
        source,
        IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record: channel_record(&schema.reference, Uuid::new_v4(), "expiry"),
        },
    )?;
    assert_eq!(
        store.apply(&fixture.registry, &delivery).await.unwrap_err(),
        MutationStorageError::JobLeaseLost
    );
    let pool = test.db.get_postgres_connection_pool();
    for count_sql in [
        "SELECT COUNT(*) FROM index_entities",
        "SELECT COUNT(*) FROM index_inbox",
    ] {
        let rows: i64 = sqlx::query_scalar(count_sql).fetch_one(pool).await?;
        assert_eq!(rows, 0, "{count_sql}");
    }
    let IndexReplayJobAcquireOutcome::Acquired(second) = jobs.acquire(&request("second")?).await?
    else {
        panic!("second lease");
    };
    assert_eq!(second.job_id(), first.job_id());
    let checkpoint = IndexReplayCheckpoint::new(
        IndexReplayCheckpointKey::new(TENANT, source, schema.reference.clone())?,
        None,
        None,
        None,
    )?;
    let checkpoints = PostgresIndexReplayCheckpointStore::new(test.db.clone(), second);
    assert_eq!(
        checkpoints
            .commit_replay_checkpoint(&checkpoint)
            .await
            .unwrap_err()
            .code(),
        "checkpoint_lease_lost"
    );
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM index_checkpoints")
        .fetch_one(pool)
        .await?;
    assert_eq!(rows, 0);

    let IndexReplayJobAcquireOutcome::Acquired(cancelled) =
        jobs.acquire(&request("third")?).await?
    else {
        panic!("third lease");
    };
    sqlx::query("UPDATE index_jobs SET cancel_requested=TRUE WHERE job_id=$1")
        .bind(cancelled.job_id())
        .execute(pool)
        .await?;
    let checkpoints = PostgresIndexReplayCheckpointStore::new(test.db.clone(), cancelled);
    assert_eq!(
        checkpoints
            .commit_replay_checkpoint(&checkpoint)
            .await
            .unwrap_err()
            .code(),
        "checkpoint_lease_lost"
    );
    test.cleanup().await
}

#[tokio::test]
async fn postgres_waiting_checkpoint_and_heartbeat_cannot_use_an_expired_lease() -> TestResult<()> {
    use crate::{
        IndexReplayCheckpointKey, IndexReplayCheckpointStore, IndexReplayJobAcquireOutcome,
        IndexReplayJobError, IndexReplayJobLeaseRequest, PostgresIndexReplayCheckpointStore,
        PostgresIndexReplayJobStore,
    };
    let Some(test) = PostgresTestDb::setup().await? else {
        return Ok(());
    };
    let fixture = Fixture::new();
    let schema = fixture
        .schemas
        .iter()
        .find(|schema| schema.reference == schema_ref("channel"))
        .unwrap();
    PostgresSchemaRegistrationStore::new(test.db.clone())
        .register(TENANT, schema)
        .await?;
    let jobs = PostgresIndexReplayJobStore::new(test.db.clone());
    let pool = test.db.get_postgres_connection_pool();
    let source = "hardening-lock-order";
    let request = IndexReplayJobLeaseRequest::new(
        TENANT,
        schema.reference.clone(),
        source,
        "worker",
        Duration::from_secs(4),
    )?;
    let mut previous = None;
    for heartbeat in [false, true] {
        let IndexReplayJobAcquireOutcome::Acquired(lease) = jobs.acquire(&request).await? else {
            panic!("expected due job");
        };
        if let Some(job_id) = previous {
            assert_eq!(lease.job_id(), job_id);
            assert_eq!(lease.attempt_count(), 2);
        }
        previous = Some(lease.job_id());
        // Take only a row lock, not an UPDATE: PostgreSQL does not need to re-evaluate an
        // earlier scan filter against a newer row version when this transaction releases it.
        let mut blocker = pool.begin().await?;
        let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *blocker)
            .await?;
        sqlx::query(
            "SELECT job_id FROM index_jobs WHERE tenant_id = $1 AND job_id = $2 FOR UPDATE",
        )
        .bind(TENANT)
        .bind(lease.job_id())
        .fetch_one(&mut *blocker)
        .await?;
        let waiter_lease = lease.clone();
        let waiter_db = test.db.clone();
        let checkpoint = IndexReplayCheckpointKey::new(TENANT, source, schema.reference.clone())?;
        let waiter = tokio::spawn(async move {
            if heartbeat {
                let result = PostgresIndexReplayJobStore::new(waiter_db)
                    .heartbeat(&waiter_lease, Duration::from_secs(60))
                    .await;
                assert!(
                    matches!(result, Err(IndexReplayJobError::LeaseLost)),
                    "{result:?}"
                );
            } else {
                let result = PostgresIndexReplayCheckpointStore::new(waiter_db, waiter_lease)
                    .load_replay_checkpoint(&checkpoint)
                    .await;
                assert_eq!(result.unwrap_err().code(), "checkpoint_lease_lost");
            }
        });
        // Prove the query was waiting while the lease was still valid, then cross the actual
        // persisted expiry before releasing the lock. Sleeping on the application clock alone
        // could let the old SQL start after expiry and make this regression pass incorrectly.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let waiting: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
                )
                .bind(blocker_pid)
                .fetch_one(pool)
                .await?;
                if waiting {
                    break Ok::<(), sqlx::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await??;
        let active: bool = sqlx::query_scalar(
            "SELECT lease_expires_at > clock_timestamp() FROM index_jobs WHERE tenant_id = $1 AND job_id = $2",
        )
        .bind(TENANT)
        .bind(lease.job_id())
        .fetch_one(&mut *blocker)
        .await?;
        assert!(active, "waiter must be blocked before expiry");
        sqlx::query("SELECT pg_sleep_until(lease_expires_at + INTERVAL '20 milliseconds') FROM index_jobs WHERE tenant_id = $1 AND job_id = $2")
            .bind(TENANT)
            .bind(lease.job_id())
            .execute(&mut *blocker)
            .await?;
        blocker.commit().await?;
        tokio::time::timeout(Duration::from_secs(5), waiter).await??;
        let expired: bool = sqlx::query_scalar(
            "SELECT lease_expires_at <= clock_timestamp() FROM index_jobs WHERE tenant_id = $1 AND job_id = $2",
        )
        .bind(TENANT)
        .bind(lease.job_id())
        .fetch_one(pool)
        .await?;
        assert!(expired, "an expired generation must stay expired");
    }
    test.cleanup().await
}
