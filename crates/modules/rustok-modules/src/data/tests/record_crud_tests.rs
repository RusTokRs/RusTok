//! Record deletion, idempotency, indexing, and quota release tests.
#![allow(unused_imports)]

use super::super::*;
use super::fixtures::*;

#[tokio::test]
async fn record_delete_is_revisioned_idempotent_and_removes_indexes() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&sea_orm_migration::SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&sea_orm_migration::SchemaManager::new(&database))
            .await
            .expect("module migration");
    }
    let scope = ArtifactDataScope {
        policy_revision: 7,
        data_contract_digest: crate::promotion::digest_json(&ArtifactPersistenceContract {
            revision: 1,
            schema_digest: canonical_schema_digest(&json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
            })),
            indexes: Vec::new(),
        })
        .expect("purge fixture contract digest"),
        ..test_scope(Uuid::new_v4(), "sample_module")
    };
    setup_test_serving_namespace(&database, &scope).await;
    let indexes = vec![ArtifactDataIndexField {
        name: "status".to_string(),
        json_pointer: "/status".to_string(),
        value_type: ArtifactDataIndexValueType::String,
    }];
    let broker = SeaOrmArtifactDataBroker::with_indexes(
        database.clone(),
        ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        },
        AllowSchemaValidator,
        indexes.clone(),
    );
    let put_idempotency_key = Uuid::new_v4();
    let record = broker
        .put(
            &scope,
            ArtifactDataWrite {
                key: "state/current".to_string(),
                value: json!({ "status": "active" }),
                expected_revision: None,
                create_only: false,
                idempotency_key: put_idempotency_key,
            },
        )
        .await
        .expect("put record");
    let request = ArtifactDataDeleteRequest {
        key: record.key.clone(),
        expected_revision: record.revision,
        idempotency_key: Uuid::new_v4(),
    };
    let deleted = broker
        .delete(&scope, request.clone())
        .await
        .expect("delete record");
    assert_eq!(deleted.key, record.key);
    assert_eq!(deleted.deleted_revision, record.revision);
    assert_eq!(
        broker
            .delete(&scope, request.clone())
            .await
            .expect("replay delete"),
        deleted
    );
    assert!(matches!(
        broker
            .delete(
                &scope,
                ArtifactDataDeleteRequest {
                    key: "state/other".to_string(),
                    expected_revision: record.revision,
                    idempotency_key: request.idempotency_key,
                },
            )
            .await,
        Err(ArtifactDataError::IdempotencyConflict)
    ));
    assert!(
        broker
            .get(&scope, &record.key)
            .await
            .expect("get deleted record")
            .is_none()
    );

    for (table, count_column) in [
        ("module_artifact_data_indexes", "index_count"),
        ("module_artifact_data_delete_operations", "operation_count"),
    ] {
        let row = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                format!("SELECT COUNT(*) AS {count_column} FROM {table}"),
            ))
            .await
            .expect("query count")
            .expect("count row");
        let count = row.try_get::<i64>("", count_column).expect("row count");
        let expected = if table == "module_artifact_data_delete_operations" {
            1
        } else {
            0
        };
        assert_eq!(count, expected);
    }
    assert!(matches!(
        broker
            .delete(
                &scope,
                ArtifactDataDeleteRequest {
                    key: record.key.clone(),
                    expected_revision: record.revision,
                    idempotency_key: Uuid::new_v4(),
                },
            )
            .await,
        Err(ArtifactDataError::RevisionConflict)
    ));

    let next_scope = ArtifactDataScope {
        policy_revision: 8,
        ..scope.clone()
    };
    let next_broker = SeaOrmArtifactDataBroker::with_indexes(
        database.clone(),
        ExactArtifactDataAuthorizer {
            scope: next_scope.clone(),
        },
        AllowSchemaValidator,
        indexes,
    );
    next_broker
        .put(
            &next_scope,
            ArtifactDataWrite {
                key: record.key.clone(),
                value: json!({ "status": "active" }),
                expected_revision: None,
                create_only: false,
                idempotency_key: put_idempotency_key,
            },
        )
        .await
        .expect("same idempotency key under a new policy revision");
    assert!(
        next_broker
            .get(&next_scope, &record.key)
            .await
            .expect("get recreated record")
            .is_some()
    );

    let export_context = ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: Some(next_scope.tenant_id),
        trace_id: "test:artifact-data-export".to_string(),
        correlation_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
    };
    SeaOrmArtifactDataExportService::new(database.clone(), AllowExportAuthorizer)
        .export(ArtifactDataExportRequest {
            scope: next_scope.clone(),
            expected_namespace_revision: 1,
            page: ArtifactDataPageRequest {
                prefix: "state/".to_string(),
                after_key: None,
                limit: 10,
            },
            context: export_context.clone(),
            reason: "verify policy-scoped export evidence".to_string(),
        })
        .await
        .expect("export data");
    let export_audit = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT actor_id, trace_id, correlation_id, idempotency_key \
                 FROM module_artifact_data_exports"
                .to_string(),
        ))
        .await
        .expect("export audit query")
        .expect("export audit");
    assert_eq!(
        export_audit
            .try_get::<String>("", "actor_id")
            .expect("export audit actor"),
        export_context.actor_id.to_string()
    );
    assert_eq!(
        export_audit
            .try_get::<String>("", "trace_id")
            .expect("export audit trace"),
        export_context.trace_id
    );
    assert_eq!(
        export_audit
            .try_get::<String>("", "correlation_id")
            .expect("export audit correlation"),
        export_context.correlation_id.to_string()
    );
    assert_eq!(
        export_audit
            .try_get::<String>("", "idempotency_key")
            .expect("export audit idempotency"),
        export_context.idempotency_key.to_string()
    );
    let export_event = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events \
                 WHERE event_type = 'module.artifact.data_exported'"
                .to_string(),
        ))
        .await
        .expect("export event query")
        .expect("export event");
    let export_payload: Value = export_event
        .try_get("", "payload")
        .expect("export event payload");
    let export_envelope: rustok_events::EventEnvelope =
        serde_json::from_value(export_payload).expect("export event envelope");
    assert_eq!(export_envelope.actor_id, Some(export_context.actor_id));
    assert_eq!(export_envelope.tenant_id, next_scope.tenant_id);
    assert_eq!(
        export_envelope.correlation_id,
        export_context.correlation_id
    );
    assert_eq!(
        export_envelope.trace_id.as_deref(),
        Some(export_context.trace_id.as_str())
    );
    let purge_installation_id = seed_retired_data_purge_installation(&database, &next_scope).await;
    let purge_request = ArtifactDataPurgeRequest {
        installation_id: purge_installation_id,
        expected_namespace_revision: 1,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Some(next_scope.tenant_id),
            trace_id: "test:artifact-data-purge".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        reason: "verify policy-scoped purge evidence".to_string(),
    };
    let purge_context = purge_request.context.clone();
    let purge = SeaOrmArtifactDataPurgeService::new(
        database.clone(),
        DataPurgeFixturePolicy {
            scope: next_scope.clone(),
            actor_id: purge_request.context.actor_id,
            installation_id: purge_installation_id,
        },
    );
    assert!(matches!(
        purge
            .purge(ArtifactDataPurgeRequest {
                context: ModuleCommandContext {
                    tenant_id: Some(Uuid::new_v4()),
                    ..purge_request.context.clone()
                },
                ..purge_request.clone()
            })
            .await,
        Err(ArtifactDataError::PurgePrecondition)
    ));
    let purged = purge
        .purge(purge_request.clone())
        .await
        .expect("purge data");
    assert_eq!(
        purge
            .purge(purge_request.clone())
            .await
            .expect("replay purge"),
        purged
    );
    assert!(matches!(
        purge
            .purge(ArtifactDataPurgeRequest {
                context: ModuleCommandContext {
                    trace_id: "test:conflicting-artifact-data-purge".to_string(),
                    ..purge_request.context.clone()
                },
                ..purge_request.clone()
            })
            .await,
        Err(ArtifactDataError::IdempotencyConflict)
    ));
    assert!(matches!(
        purge
            .purge(ArtifactDataPurgeRequest {
                context: ModuleCommandContext {
                    correlation_id: Uuid::new_v4(),
                    ..purge_request.context.clone()
                },
                ..purge_request.clone()
            })
            .await,
        Err(ArtifactDataError::IdempotencyConflict)
    ));
    let purge_receipt = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT installation_id, actor_id, trace_id, correlation_id \
                 FROM module_artifact_data_purge_operations"
                .to_string(),
        ))
        .await
        .expect("purge receipt query")
        .expect("purge receipt");
    assert_eq!(
        purge_receipt
            .try_get::<String>("", "actor_id")
            .expect("purge receipt actor"),
        purge_context.actor_id.to_string()
    );
    assert_eq!(
        purge_receipt
            .try_get::<String>("", "trace_id")
            .expect("purge receipt trace"),
        purge_context.trace_id
    );
    assert_eq!(
        purge_receipt
            .try_get::<String>("", "correlation_id")
            .expect("purge receipt correlation"),
        purge_context.correlation_id.to_string()
    );
    assert_eq!(
        purge_receipt
            .try_get::<String>("", "installation_id")
            .expect("purge receipt installation"),
        purge_installation_id.to_string()
    );
    let purge_event = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT payload FROM sys_events \
                 WHERE event_type = 'module.artifact.data_purged'"
                .to_string(),
        ))
        .await
        .expect("purge event query")
        .expect("purge event");
    let purge_payload: Value = purge_event
        .try_get("", "payload")
        .expect("purge event payload");
    let purge_envelope: rustok_events::EventEnvelope =
        serde_json::from_value(purge_payload).expect("purge event envelope");
    assert_eq!(purge_envelope.actor_id, Some(purge_context.actor_id));
    assert_eq!(purge_envelope.tenant_id, next_scope.tenant_id);
    assert_eq!(purge_envelope.correlation_id, purge_context.correlation_id);
    assert_eq!(
        purge_envelope.trace_id.as_deref(),
        Some(purge_context.trace_id.as_str())
    );
    for table in [
        "module_artifact_data_exports",
        "module_artifact_data_purge_operations",
    ] {
        let row = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                format!("SELECT policy_revision FROM {table}"),
            ))
            .await
            .expect("query policy revision")
            .expect("policy revision row");
        assert_eq!(
            row.try_get::<i64>("", "policy_revision")
                .expect("policy revision"),
            i64::try_from(next_scope.policy_revision).expect("policy revision fits i64")
        );
    }
}

#[tokio::test]
async fn structured_quota_is_projected_atomically_and_delete_releases_capacity() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    for migration in ModulesModule.migrations() {
        migration
            .up(&sea_orm_migration::SchemaManager::new(&database))
            .await
            .expect("module migration");
    }
    let scope = ArtifactDataScope {
        policy_revision: 3,
        ..test_scope(Uuid::new_v4(), "quota_module")
    };
    setup_test_serving_namespace(&database, &scope).await;
    let quota = ArtifactDataQuota {
        max_structured_records: 2,
        max_structured_bytes: 8,
        ..ArtifactDataQuota::default()
    };
    let broker = SeaOrmArtifactDataBroker::with_indexes_and_quota(
        database,
        ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        },
        AllowSchemaValidator,
        Vec::new(),
        quota,
    );
    let first = broker
        .put(
            &scope,
            ArtifactDataWrite {
                key: "state/one".to_string(),
                value: json!("1234"),
                expected_revision: None,
                create_only: false,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("first quota-bound write");

    let batch_error = broker
        .put_batch(
            &scope,
            ArtifactDataBatchWrite {
                writes: vec![
                    ArtifactDataWrite {
                        key: "state/two".to_string(),
                        value: json!(1),
                        expected_revision: None,
                        create_only: false,
                        idempotency_key: Uuid::new_v4(),
                    },
                    ArtifactDataWrite {
                        key: "state/three".to_string(),
                        value: json!(2),
                        expected_revision: None,
                        create_only: false,
                        idempotency_key: Uuid::new_v4(),
                    },
                ],
            },
        )
        .await
        .expect_err("batch must exceed record quota");
    assert!(matches!(
        batch_error,
        ArtifactDataError::QuotaExceeded {
            resource: "structured_records",
            limit: 2,
            attempted: 3,
        }
    ));
    for key in ["state/two", "state/three"] {
        assert!(
            broker
                .get(&scope, key)
                .await
                .expect("read rolled-back batch key")
                .is_none()
        );
    }

    let byte_error = broker
        .put(
            &scope,
            ArtifactDataWrite {
                key: first.key.clone(),
                value: json!("1234567"),
                expected_revision: Some(first.revision),
                create_only: false,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect_err("overwrite must exceed byte quota");
    assert!(matches!(
        byte_error,
        ArtifactDataError::QuotaExceeded {
            resource: "structured_bytes",
            limit: 8,
            attempted: 9,
        }
    ));
    assert_eq!(
        broker
            .get(&scope, &first.key)
            .await
            .expect("read record after rejected overwrite"),
        Some(first.clone())
    );

    broker
        .delete(
            &scope,
            ArtifactDataDeleteRequest {
                key: first.key,
                expected_revision: first.revision,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("delete releases quota");
    let records = broker
        .put_batch(
            &scope,
            ArtifactDataBatchWrite {
                writes: ["state/two", "state/three"]
                    .into_iter()
                    .map(|key| ArtifactDataWrite {
                        key: key.to_string(),
                        value: json!(1),
                        expected_revision: None,
                        create_only: false,
                        idempotency_key: Uuid::new_v4(),
                    })
                    .collect(),
            },
        )
        .await
        .expect("batch fits after delete");
    assert_eq!(records.len(), 2);
}
