//! Object upload, delete, revisioning, quota, and continuation tests.
#![allow(unused_imports)]

use super::super::*;
use super::fixtures::*;

#[test]
fn sandbox_object_data_adapter_accepts_only_bounded_base64_payloads() {
    let mut call = CapabilityCall {
        execution_id: Uuid::new_v4(),
        subject: SandboxSubject::ModuleArtifact {
            installation_id: Uuid::new_v4(),
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:sample".to_string(),
        },
        context: CapabilityCallContext {
            phase: ExecutionPhase::Manual,
            tenant_id: Some(Uuid::new_v4()),
            actor_id: None,
            trace_id: None,
        },
        capability: CapabilityName::new("platform.data.objects").expect("capability name"),
        operation: "put".to_string(),
        input: json!({
            "name": "exports/report.json",
            "content_type": "application/json",
            "data_base64": "e30=",
            "idempotency_key": Uuid::new_v4(),
        }),
    };
    assert!(matches!(
        decode_object_data_capability_call(&call),
        Ok(ObjectDataCapabilityCall::Put { .. })
    ));

    call.input = json!({
        "name": "exports/report.json",
        "content_type": "application/json",
        "data_base64": "not-base64",
        "idempotency_key": Uuid::new_v4(),
    });
    assert!(decode_object_data_capability_call(&call).is_err());
}

#[test]
fn sandbox_object_delete_requires_revision_and_idempotency() {
    let mut call = CapabilityCall {
        execution_id: Uuid::new_v4(),
        subject: SandboxSubject::ModuleArtifact {
            installation_id: Uuid::new_v4(),
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:sample".to_string(),
        },
        context: CapabilityCallContext {
            phase: ExecutionPhase::Manual,
            tenant_id: Some(Uuid::new_v4()),
            actor_id: None,
            trace_id: None,
        },
        capability: CapabilityName::new("platform.data.objects").expect("capability name"),
        operation: "delete".to_string(),
        input: json!({
            "name": "exports/report.json",
            "expected_revision": 4,
            "idempotency_key": Uuid::new_v4(),
        }),
    };
    let decoded = decode_object_data_capability_call(&call).expect("delete request");
    assert!(matches!(
        decoded,
        ObjectDataCapabilityCall::Delete {
            request: ArtifactDataObjectDeleteRequest {
                expected_revision: 4,
                ..
            }
        }
    ));

    call.input["expected_revision"] = json!(0);
    assert!(decode_object_data_capability_call(&call).is_err());
    call.input["expected_revision"] = json!(4);
    call.input["idempotency_key"] = json!("not-a-uuid");
    assert!(decode_object_data_capability_call(&call).is_err());
}

#[tokio::test]
async fn object_delete_is_revisioned_idempotent_and_queues_private_bytes() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    for migration in ModulesModule.migrations() {
        migration
            .up(&sea_orm_migration::SchemaManager::new(&database))
            .await
            .expect("module migration");
    }
    let directory = std::env::temp_dir().join(format!("rustok-artifact-data-{}", Uuid::new_v4()));
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: directory.to_string_lossy().into_owned(),
        base_url: "/private".to_string(),
        fsync: false,
    })
    .expect("local storage");
    let scope = test_scope(Uuid::new_v4(), "sample_module");
    setup_test_serving_namespace(&database, &scope).await;
    let broker = SeaOrmArtifactDataObjectBroker::new(
        database.clone(),
        storage.clone(),
        ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        },
    );
    let put_idempotency_key = Uuid::new_v4();
    let object = broker
        .put_object(
            &scope,
            ArtifactDataObjectUpload {
                name: "exports/report.json".to_string(),
                content_type: "application/json".to_string(),
                data: Bytes::from_static(b"{}"),
                expected_revision: None,
                idempotency_key: put_idempotency_key,
            },
        )
        .await
        .expect("put object");
    let request = ArtifactDataObjectDeleteRequest {
        name: object.name.clone(),
        expected_revision: object.revision,
        idempotency_key: Uuid::new_v4(),
    };
    let deleted = broker
        .delete_object(&scope, request.clone())
        .await
        .expect("delete object");
    assert_eq!(deleted.name, object.name);
    assert_eq!(deleted.deleted_revision, object.revision);
    assert_eq!(
        broker
            .delete_object(&scope, request.clone())
            .await
            .expect("replay delete"),
        deleted
    );
    assert!(matches!(
        broker
            .delete_object(
                &scope,
                ArtifactDataObjectDeleteRequest {
                    name: "exports/other.json".to_string(),
                    expected_revision: object.revision,
                    idempotency_key: request.idempotency_key,
                },
            )
            .await,
        Err(ArtifactDataError::IdempotencyConflict)
    ));
    assert!(
        broker
            .get_object(&scope, &object.name)
            .await
            .expect("get deleted object")
            .is_none()
    );

    let row = database
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS candidate_count FROM module_artifact_data_object_gc_candidates"
                .to_string(),
        ))
        .await
        .expect("query GC candidates")
        .expect("GC count row");
    assert_eq!(
        row.try_get::<i64>("", "candidate_count")
            .expect("GC candidate count"),
        1
    );
    assert!(matches!(
        broker
            .delete_object(
                &scope,
                ArtifactDataObjectDeleteRequest {
                    name: object.name.clone(),
                    expected_revision: object.revision,
                    idempotency_key: Uuid::new_v4(),
                },
            )
            .await,
        Err(ArtifactDataError::RevisionConflict)
    ));

    let next_scope = ArtifactDataScope {
        policy_revision: 2,
        ..scope.clone()
    };
    let next_broker = SeaOrmArtifactDataObjectBroker::new(
        database,
        storage,
        ExactArtifactDataAuthorizer {
            scope: next_scope.clone(),
        },
    );
    next_broker
        .put_object(
            &next_scope,
            ArtifactDataObjectUpload {
                name: object.name.clone(),
                content_type: "application/json".to_string(),
                data: Bytes::from_static(b"{}"),
                expected_revision: None,
                idempotency_key: put_idempotency_key,
            },
        )
        .await
        .expect("same idempotency key under a new policy revision");
    assert!(
        next_broker
            .get_object(&next_scope, &object.name)
            .await
            .expect("get recreated object")
            .is_some()
    );
    drop(broker);
    drop(next_broker);
    if directory.exists() {
        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove test storage");
    }
}

#[tokio::test]
async fn object_and_staging_quotas_are_namespace_wide_and_reclaimable() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    for migration in ModulesModule.migrations() {
        migration
            .up(&sea_orm_migration::SchemaManager::new(&database))
            .await
            .expect("module migration");
    }
    let directory =
        std::env::temp_dir().join(format!("rustok-artifact-data-quota-{}", Uuid::new_v4()));
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: directory.to_string_lossy().into_owned(),
        base_url: "/private".to_string(),
        fsync: false,
    })
    .expect("local storage");
    let scope = ArtifactDataScope {
        policy_revision: 5,
        ..test_scope(Uuid::new_v4(), "quota_module")
    };
    setup_test_serving_namespace(&database, &scope).await;
    let quota = ArtifactDataQuota {
        max_objects: 1,
        max_object_bytes: 4,
        max_upload_sessions: 1,
        max_staging_bytes: 4,
        ..ArtifactDataQuota::default()
    };
    let authorizer = ExactArtifactDataAuthorizer {
        scope: scope.clone(),
    };
    let broker = SeaOrmArtifactDataObjectBroker::with_infrastructure_and_quota(
        database.clone(),
        storage.clone(),
        authorizer.clone(),
        ControlPlaneInfrastructure::default(),
        quota,
    );
    let first = broker
        .put_object(
            &scope,
            ArtifactDataObjectUpload {
                name: "objects/one.bin".to_string(),
                content_type: "application/octet-stream".to_string(),
                data: Bytes::from_static(b"1234"),
                expected_revision: None,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("first object");
    let object_count_error = broker
        .put_object(
            &scope,
            ArtifactDataObjectUpload {
                name: "objects/two.bin".to_string(),
                content_type: "application/octet-stream".to_string(),
                data: Bytes::from_static(b"1"),
                expected_revision: None,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect_err("second object must exceed count quota");
    assert!(matches!(
        object_count_error,
        ArtifactDataError::QuotaExceeded {
            resource: "objects",
            limit: 1,
            attempted: 2,
        }
    ));
    let object_byte_error = broker
        .put_object(
            &scope,
            ArtifactDataObjectUpload {
                name: first.name.clone(),
                content_type: first.content_type.clone(),
                data: Bytes::from_static(b"12345"),
                expected_revision: Some(first.revision),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect_err("replacement must exceed object byte quota");
    assert!(matches!(
        object_byte_error,
        ArtifactDataError::QuotaExceeded {
            resource: "object_bytes",
            limit: 4,
            attempted: 5,
        }
    ));
    broker
        .delete_object(
            &scope,
            ArtifactDataObjectDeleteRequest {
                name: first.name,
                expected_revision: first.revision,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("object delete releases quota");
    broker
        .put_object(
            &scope,
            ArtifactDataObjectUpload {
                name: "objects/two.bin".to_string(),
                content_type: "application/octet-stream".to_string(),
                data: Bytes::from_static(b"1234"),
                expected_revision: None,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("object fits after delete");

    let uploads = SeaOrmArtifactDataObjectUploadService::with_infrastructure_and_quota(
        database,
        storage,
        authorizer,
        ControlPlaneInfrastructure::default(),
        quota,
    );
    let session = uploads
        .begin(
            &scope,
            ArtifactDataObjectUploadSessionRequest {
                name: "uploads/one.bin".to_string(),
                content_type: "application/octet-stream".to_string(),
                expected_revision: None,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect("first upload session");
    let session_error = uploads
        .begin(
            &scope,
            ArtifactDataObjectUploadSessionRequest {
                name: "uploads/two.bin".to_string(),
                content_type: "application/octet-stream".to_string(),
                expected_revision: None,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
        .expect_err("second active session must exceed quota");
    assert!(matches!(
        session_error,
        ArtifactDataError::QuotaExceeded {
            resource: "upload_sessions",
            limit: 1,
            attempted: 2,
        }
    ));
    uploads
        .append_chunk(
            &scope,
            ArtifactDataObjectUploadChunk {
                session_id: session.session_id,
                sequence: 0,
                data: Bytes::from_static(b"1234"),
            },
        )
        .await
        .expect("staging chunk at quota");
    let staging_error = uploads
        .append_chunk(
            &scope,
            ArtifactDataObjectUploadChunk {
                session_id: session.session_id,
                sequence: 1,
                data: Bytes::from_static(b"5"),
            },
        )
        .await
        .expect_err("staging bytes must exceed quota");
    assert!(matches!(
        staging_error,
        ArtifactDataError::QuotaExceeded {
            resource: "staging_bytes",
            limit: 4,
            attempted: 5,
        }
    ));
    drop(broker);
    drop(uploads);
    if directory.exists() {
        tokio::fs::remove_dir_all(directory)
            .await
            .expect("remove quota test storage");
    }
}

#[test]
fn sandbox_object_data_adapter_keeps_list_continuations_inside_the_prefix() {
    let mut call = CapabilityCall {
        execution_id: Uuid::new_v4(),
        subject: SandboxSubject::ModuleArtifact {
            installation_id: Uuid::new_v4(),
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:sample".to_string(),
        },
        context: CapabilityCallContext {
            phase: ExecutionPhase::Manual,
            tenant_id: Some(Uuid::new_v4()),
            actor_id: None,
            trace_id: None,
        },
        capability: CapabilityName::new("platform.data.objects").expect("capability name"),
        operation: "list".to_string(),
        input: json!({
            "prefix": "exports/",
            "after_name": "exports/report.json",
            "limit": 10
        }),
    };
    assert!(matches!(
        decode_object_data_capability_call(&call),
        Ok(ObjectDataCapabilityCall::List { .. })
    ));

    call.input = json!({
        "prefix": "exports/",
        "after_name": "private/report.json",
        "limit": 10
    });
    assert!(decode_object_data_capability_call(&call).is_err());
}
