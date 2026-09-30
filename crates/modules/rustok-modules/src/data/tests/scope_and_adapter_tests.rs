//! Scope validation, quota, export, and sandbox adapter tests.
#![allow(unused_imports)]

use super::fixtures::*;
use super::super::*;

    #[test]
    fn scope_and_keys_reject_guest_controlled_namespace_escapes() {
        assert!(matches!(
            ArtifactDataScope {
                tenant_id: Uuid::nil(),
                data_owner_id: Uuid::nil(),
                namespace_instance_id: Uuid::nil(),
                data_contract_digest: "invalid".into(),
                module_slug: "module".into(),
                data_contract_revision: 1,
                policy_revision: 1,
            }
            .validate(),
            Err(ArtifactDataError::InvalidScope)
        ));
        for key in ["/host/path", "state/../escape", "state//key"] {
            assert!(matches!(
                validate_artifact_data_key(key),
                Err(ArtifactDataError::InvalidKey)
            ));
        }
    }

    #[tokio::test]
    async fn fixed_quota_policy_cannot_raise_platform_ceilings() {
        let invalid = ArtifactDataQuota {
            max_structured_records: MAX_ARTIFACT_DATA_NAMESPACE_RECORDS + 1,
            ..ArtifactDataQuota::default()
        };
        assert!(matches!(
            FixedArtifactDataQuotaPolicy::new(invalid),
            Err(ArtifactDataError::InvalidQuota)
        ));

        let scope = ArtifactDataScope {
            policy_revision: 2,
            ..test_scope(Uuid::new_v4(), "quota_module")
        };
        let quota = ArtifactDataQuota {
            max_structured_records: 5,
            ..ArtifactDataQuota::default()
        };
        let policy = FixedArtifactDataQuotaPolicy::new(quota).expect("valid fixed quota policy");
        assert_eq!(
            policy.quota_for(&scope).await.expect("resolved quota"),
            quota
        );
    }

    #[test]
    fn owner_export_requires_active_revision_actor_reason_and_bounded_page() {
        let scope = test_scope(Uuid::new_v4(), "sample_module");
        let mut request = ArtifactDataExportRequest {
            scope: scope.clone(),
            expected_namespace_revision: 1,
            page: ArtifactDataPageRequest {
                prefix: "state/".to_string(),
                after_key: None,
                limit: 100,
            },
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(scope.tenant_id),
                trace_id: "test:artifact-data-export".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            reason: "operator backup review".to_string(),
        };
        assert!(validate_export_request(&request).is_ok());

        request.expected_namespace_revision = 0;
        assert!(matches!(
            validate_export_request(&request),
            Err(ArtifactDataError::ExportPrecondition)
        ));
        request.expected_namespace_revision = 1;
        request.context.tenant_id = Some(Uuid::new_v4());
        assert!(matches!(
            validate_export_request(&request),
            Err(ArtifactDataError::ExportPrecondition)
        ));
        request.context.tenant_id = Some(request.scope.tenant_id);
        request.reason = " ".to_string();
        assert!(matches!(
            validate_export_request(&request),
            Err(ArtifactDataError::ExportPrecondition)
        ));
        request.reason = "operator backup review".to_string();
        request.page.limit = 101;
        assert!(matches!(
            validate_export_request(&request),
            Err(ArtifactDataError::InvalidPage)
        ));
    }

    #[test]
    fn sandbox_data_adapter_keeps_list_continuations_inside_the_prefix() {
        let mut call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "list".to_string(),
            input: json!({ "prefix": "state/", "after_key": "state/one", "limit": 10 }),
        };
        assert!(matches!(
            decode_data_capability_call(&call),
            Ok(DataCapabilityCall::List { .. })
        ));
        call.input = json!({ "prefix": "state/", "after_key": "other/one", "limit": 10 });
        assert!(decode_data_capability_call(&call).is_err());
    }

    #[test]
    fn sandbox_data_adapter_decodes_only_bounded_scalar_index_queries() {
        let mut call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "query_index".to_string(),
            input: json!({
                "index": "status",
                "value": "active",
                "prefix": "state/",
                "after_key": "state/one",
                "limit": 10,
            }),
        };
        assert!(matches!(
            decode_data_capability_call(&call),
            Ok(DataCapabilityCall::QueryIndex { .. })
        ));

        call.input["value"] = json!(["active"]);
        assert!(decode_data_capability_call(&call).is_err());
        call.input["value"] = json!("active");
        call.input["after_key"] = json!("other/one");
        assert!(decode_data_capability_call(&call).is_err());
    }

    #[test]
    fn sandbox_data_batch_requires_distinct_bounded_writes() {
        let idempotency_key = Uuid::new_v4();
        let call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                installation_id: Uuid::new_v4(),
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "put_batch".to_string(),
            input: json!({
                "writes": [
                    { "key": "state/one", "value": 1, "idempotency_key": idempotency_key },
                    { "key": "state/two", "value": 2, "idempotency_key": Uuid::new_v4() }
                ]
            }),
        };
        assert!(matches!(
            decode_data_capability_call(&call),
            Ok(DataCapabilityCall::PutBatch { .. })
        ));

        let duplicate = CapabilityCall {
            input: json!({
                "writes": [
                    { "key": "state/one", "value": 1, "idempotency_key": idempotency_key },
                    { "key": "state/two", "value": 2, "idempotency_key": idempotency_key }
                ]
            }),
            ..call
        };
        assert!(decode_data_capability_call(&duplicate).is_err());
    }

    #[test]
    fn sandbox_data_delete_requires_revision_and_idempotency() {
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
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "delete".to_string(),
            input: json!({
                "key": "state/current",
                "expected_revision": 5,
                "idempotency_key": Uuid::new_v4(),
            }),
        };
        let decoded = decode_data_capability_call(&call).expect("delete request");
        assert!(matches!(
            decoded,
            DataCapabilityCall::Delete {
                request: ArtifactDataDeleteRequest {
                    expected_revision: 5,
                    ..
                }
            }
        ));

        call.input["expected_revision"] = json!(0);
        assert!(decode_data_capability_call(&call).is_err());
        call.input["expected_revision"] = json!(5);
        call.input["idempotency_key"] = json!("not-a-uuid");
        assert!(decode_data_capability_call(&call).is_err());
    }

