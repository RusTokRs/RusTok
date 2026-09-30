    use std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use async_trait::async_trait;
    use rustok_api::{Permission, PortActor, PortError, TenantLocale};
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use rustok_tenant::{
        ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyProjection,
    };
    use rustok_translation_targets::{
        ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug, ReadTranslationResourceRequest,
        ResourceId, ResourceKind, TranslationApplicationReceipt, TranslationDataClassification,
        TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
        TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSummary,
        TranslationStrategy, TranslationTargetCapability, TranslationTargetProvider,
        TranslationTargetProviderDescriptor, TranslationValueProfile,
    };
    use sea_orm::{
        ColumnTrait, ConnectOptions, ConnectionTrait, Database, DbBackend, EntityTrait,
        PaginatorTrait, QueryFilter, Statement, sea_query::Expr,
    };
    use sea_orm_migration::SchemaManager;
    use tokio::process::Command;

    use chrono::Utc;
    use rustok_api::{Action, Resource, manifest_hash::hash_manifest};

    use super::*;
    use crate::{
        MachineTranslationAttemptEvidence, MachineTranslationBatchExecution,
        MachineTranslationBatchRequest, MachineTranslationBatchResult,
        MachineTranslationDiagnostic, MachineTranslationExecutionEvidence,
        MachineTranslationExecutionStatus, MachineTranslationExecutionStatusEvidence,
        MachineTranslationPort, MachineTranslationProviderDescriptor,
        MachineTranslationProviderState, MachineTranslationUnit, MachineTranslationUnitResult,
        MachineTranslationUsage, PurgeMemoryEntryInput, TranslationMemoryService,
        entities::{
            job, job_item, machine_memory_binding, machine_operation, machine_recovery,
            memory_entry,
        },
        migrations,
    };

    struct CancellationMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        calls: AtomicUsize,
    }

    struct InProgressMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        translate_calls: AtomicUsize,
    }

    struct RecoveryMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        recover_calls: AtomicUsize,
        recovered_result: Option<MachineTranslationBatchResult>,
    }

    struct EstimatingMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        estimate_calls: AtomicUsize,
    }

    struct RecoveryTargetProvider;

    struct SqliteTestFileGuard(PathBuf);

    struct RecoveryTenantLocalePolicies;

    impl Drop for SqliteTestFileGuard {
        fn drop(&mut self) {
            for path in sqlite_test_files(&self.0) {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    #[async_trait]
    impl TenantLocalePolicyPort for RecoveryTenantLocalePolicies {
        async fn read_locale_policy(
            &self,
            context: PortContext,
        ) -> Result<TenantLocalePolicyProjection, PortError> {
            Ok(TenantLocalePolicyProjection {
                tenant_id: Uuid::parse_str(&context.tenant_id).unwrap(),
                revision: 1,
                default_locale: TenantLocale::new("en").unwrap(),
                locales: ["en", "de"]
                    .into_iter()
                    .map(|locale| TenantLocalePolicyEntry {
                        locale: TenantLocale::new(locale).unwrap(),
                        name: locale.to_string(),
                        native_name: locale.to_string(),
                        is_default: locale == "en",
                        is_enabled: true,
                        fallback_locale: (locale != "en").then(|| TenantLocale::new("en").unwrap()),
                    })
                    .collect(),
            })
        }

        async fn replace_locale_policy(
            &self,
            _context: PortContext,
            _request: ReplaceTenantLocalePolicyRequest,
        ) -> Result<TenantLocalePolicyProjection, PortError> {
            unreachable!("machine recovery test does not replace locale policy")
        }
    }

    impl CancellationMachinePort {
        fn new() -> Self {
            Self {
                descriptor: descriptor(100),
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl MachineTranslationPort for CancellationMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, rustok_api::PortError> {
            unreachable!("cancellation test does not check health")
        }

        async fn estimate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, rustok_api::PortError> {
            unreachable!("cancellation test does not estimate")
        }

        async fn translate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchExecution, rustok_api::PortError> {
            unreachable!("cancellation test does not translate")
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, rustok_api::PortError> {
            assert!(execution_idempotency_key.starts_with("translation-machine:machine-port:"));
            Ok(MachineTranslationExecutionStatusEvidence {
                execution_id: Some("execution-a".to_string()),
                status: MachineTranslationExecutionStatus::Running,
            })
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, rustok_api::PortError> {
            unreachable!("cancellation test does not recover")
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, rustok_api::PortError> {
            assert!(execution_idempotency_key.starts_with("translation-machine:machine-port:"));
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(MachineTranslationExecutionStatusEvidence {
                execution_id: Some("execution-a".to_string()),
                status: if call == 0 {
                    MachineTranslationExecutionStatus::CancellationRequested
                } else {
                    MachineTranslationExecutionStatus::Cancelled
                },
            })
        }
    }

    #[async_trait]
    impl MachineTranslationPort for InProgressMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, PortError> {
            Ok(crate::MachineTranslationProviderHealth {
                state: MachineTranslationProviderState::Available,
                reason_code: None,
                retry_after_ms: None,
            })
        }

        async fn estimate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, PortError> {
            unreachable!("in-progress generation test does not estimate")
        }

        async fn translate_batch(
            &self,
            context: PortContext,
            request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchExecution, PortError> {
            request.validate(&context)?;
            self.translate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(MachineTranslationBatchExecution::InProgress(
                MachineTranslationExecutionStatusEvidence {
                    execution_id: Some("execution-in-progress".to_string()),
                    status: MachineTranslationExecutionStatus::Running,
                },
            ))
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("in-progress generation test returns status with the command outcome")
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, PortError> {
            unreachable!("in-progress generation test does not recover")
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("in-progress generation test does not cancel")
        }
    }

    #[async_trait]
    impl MachineTranslationPort for RecoveryMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, PortError> {
            unreachable!("machine recovery never checks provider health")
        }

        async fn estimate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, PortError> {
            unreachable!("machine recovery test does not estimate")
        }

        async fn translate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchExecution, PortError> {
            unreachable!("machine recovery must never start another translation")
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("machine recovery test does not read status")
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, PortError> {
            assert!(execution_idempotency_key.starts_with("translation-machine:machine-port:"));
            self.recover_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.recovered_result.clone())
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("machine recovery test does not cancel")
        }
    }

    #[async_trait]
    impl MachineTranslationPort for EstimatingMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, PortError> {
            unreachable!("estimate does not use the health endpoint")
        }

        async fn estimate_batch(
            &self,
            context: PortContext,
            request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, PortError> {
            request.validate(&context)?;
            self.estimate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(crate::MachineTranslationEstimate {
                input_tokens_upper_bound: 256,
                output_tokens_upper_bound: 1_048_576,
                attempts_upper_bound: 2,
                cost_minor_units_upper_bound: 17,
                currency_code: "USD".to_string(),
                price_snapshot_digest: "9".repeat(64),
                review_required: true,
            })
        }

        async fn translate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchExecution, PortError> {
            unreachable!("estimate must not execute machine translation")
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("estimate does not read execution status")
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, PortError> {
            unreachable!("estimate does not recover an execution")
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("estimate does not cancel an execution")
        }
    }

    #[async_trait]
    impl TranslationTargetProvider for RecoveryTargetProvider {
        fn descriptor(&self) -> TranslationTargetProviderDescriptor {
            TranslationTargetProviderDescriptor {
                owner_slug: OwnerSlug::new("media").unwrap(),
                resource_kind: ResourceKind::new("asset").unwrap(),
                display_name: "Recovery test media asset".to_string(),
                capabilities: BTreeSet::from([TranslationTargetCapability::ValidatePatch]),
                read_permission_floor: BTreeSet::new(),
                apply_permission_floor: BTreeSet::new(),
            }
        }

        async fn list_resources(
            &self,
            _context: PortContext,
            _request: ListTranslationResourcesRequest,
        ) -> Result<TranslationResourcePage, PortError> {
            Err(PortError::unavailable(
                "translation.test_unavailable",
                "recovery fixture does not list resources",
            ))
        }

        async fn read_resource(
            &self,
            _context: PortContext,
            _request: ReadTranslationResourceRequest,
        ) -> Result<TranslationResourceSnapshot, PortError> {
            Err(PortError::unavailable(
                "translation.test_unavailable",
                "recovery fixture does not read resources",
            ))
        }

        async fn validate_patch(
            &self,
            _context: PortContext,
            request: TranslationPatchRequest,
        ) -> Result<TranslationPatchValidation, PortError> {
            request.validate().map_err(|error| {
                PortError::validation("translation.test_patch", error.to_string())
            })?;
            Ok(TranslationPatchValidation {
                accepted: true,
                issues: Vec::new(),
            })
        }

        async fn apply_patch(
            &self,
            _context: PortContext,
            _request: TranslationPatchRequest,
        ) -> Result<TranslationApplicationReceipt, PortError> {
            Err(PortError::unavailable(
                "translation.test_unavailable",
                "recovery fixture does not apply patches",
            ))
        }
    }

    fn recovery_registry() -> Arc<TranslationTargetRegistry> {
        let mut registry = TranslationTargetRegistry::default();
        registry.register(RecoveryTargetProvider).unwrap();
        Arc::new(registry)
    }

    fn request() -> MachineTranslationBatchRequest {
        MachineTranslationBatchRequest {
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            resource: MachineTranslationResourceContext {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                resource_id: "asset-a".to_string(),
                subresource_id: None,
            },
            units: vec![MachineTranslationUnit {
                unit_id: "alt_text".to_string(),
                field_key: "alt_text".to_string(),
                source_value: "Hello {name} {count}".to_string(),
                source_hash: "a".repeat(64),
                source_revision: "revision-a".to_string(),
                profile: TranslationValueProfile::TemplateText,
                strategy: TranslationStrategy::TranslateWithPlaceholders,
                classification: TranslationDataClassification::TenantPrivate,
                ai_export_allowed: true,
                max_characters: Some(200),
                preserves_whitespace: false,
                protected_tokens: vec!["{name}".to_string(), "{count}".to_string()],
            }],
            glossary_revision: None,
            glossary_digest: None,
            glossary_terms: Vec::new(),
            memory_digest: None,
            memory_suggestions: Vec::new(),
            tone: None,
            domain: None,
            style: None,
            adapter_policy_digest: "b".repeat(64),
            evidence: BTreeMap::new(),
        }
    }

    fn descriptor(max_batch_units: u16) -> MachineTranslationProviderDescriptor {
        MachineTranslationProviderDescriptor {
            slug: "ai".to_string(),
            display_name: "AI".to_string(),
            policy_digest: "b".repeat(64),
            supported_profiles: vec![TranslationValueProfile::TemplateText],
            supported_classifications: vec![TranslationDataClassification::TenantPrivate],
            max_batch_units,
            max_batch_characters: 1_000,
            review_required: true,
        }
    }

    fn result() -> MachineTranslationBatchResult {
        MachineTranslationBatchResult {
            provider_slug: "provider-a".to_string(),
            units: vec![MachineTranslationUnitResult {
                unit_id: "alt_text".to_string(),
                translated_value: "Hallo {name} {count}".to_string(),
                protected_tokens: vec!["{count}".to_string(), "{name}".to_string()],
                diagnostics: vec![MachineTranslationDiagnostic {
                    code: "translation.machine.review".to_string(),
                    blocking: false,
                    unit_id: Some("alt_text".to_string()),
                }],
            }],
            execution: MachineTranslationExecutionEvidence {
                execution_id: "execution-a".to_string(),
                request_digest: "c".repeat(64),
                prompt_policy_digest: "b".repeat(64),
                attempts: vec![MachineTranslationAttemptEvidence {
                    attempt: 1,
                    provider_profile_id: "profile-a".to_string(),
                    provider_slug: "provider-a".to_string(),
                    model: "model-a".to_string(),
                    fallback: false,
                }],
                usage: MachineTranslationUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                    cost_minor_units: 2,
                    currency_code: "USD".to_string(),
                    price_snapshot_digest: "e".repeat(64),
                },
            },
            review_required: true,
        }
    }

    fn snapshot() -> TranslationResourceSnapshot {
        TranslationResourceSnapshot {
            summary: TranslationResourceSummary {
                identity: TranslationResourceIdentity {
                    owner_slug: OwnerSlug::new("media").unwrap(),
                    resource_kind: ResourceKind::new("asset").unwrap(),
                    resource_id: ResourceId::new("asset-a").unwrap(),
                    subresource_id: None,
                },
                display_label: "Asset".to_string(),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: OpaqueRevision::new("resource-a").unwrap(),
                exact_locales: vec![TenantLocale::new("en").unwrap()],
            },
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            rendered_fallback_locale: None,
            source_revision: OpaqueRevision::new("source-a").unwrap(),
            target_revision: None,
            fields: vec![rustok_translation_targets::TranslationFieldSnapshot {
                descriptor: rustok_translation_targets::TranslationFieldDescriptor {
                    key: FieldKey::new("alt_text").unwrap(),
                    profile: TranslationValueProfile::TemplateText,
                    strategy: TranslationStrategy::TranslateWithPlaceholders,
                    classification: TranslationDataClassification::TenantPrivate,
                    required: true,
                    ai_export_allowed: true,
                    max_characters: Some(200),
                    preserves_whitespace: false,
                },
                source_value: "Hello {name} {count}".to_string(),
                exact_target_value: None,
                source_hash: "a".repeat(64),
                protected_tokens: vec!["{name}".to_string(), "{count}".to_string()],
            }],
        }
    }

    #[test]
    fn generation_requires_unique_explicit_fields() {
        let input = GenerateMachineProposalInput {
            item_id: Uuid::new_v4(),
            field_keys: vec![
                FieldKey::new("title").unwrap(),
                FieldKey::new("title").unwrap(),
            ],
            minimum_memory_similarity_basis_points: 7_000,
            tone: None,
            domain: None,
            style: None,
        };
        assert!(matches!(
            validate_generation_input(&input),
            Err(TranslationError::InvalidRequest(_))
        ));
    }

    #[test]
    fn provider_capacity_is_checked_before_export() {
        assert!(matches!(
            validate_provider_compatibility(&request(), &descriptor(0)),
            Err(TranslationError::Provider {
                retryable: false,
                ..
            })
        ));
    }

    #[test]
    fn result_accepts_reordered_but_exact_protected_tokens() {
        validate_machine_result(&request(), &result()).unwrap();
    }

    #[test]
    fn result_rejects_changed_protected_tokens() {
        let mut result = result();
        result.units[0].protected_tokens = vec!["{name}".to_string()];
        assert!(matches!(
            validate_machine_result(&request(), &result),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    #[test]
    fn result_rejects_duplicate_protected_token_occurrences() {
        let mut result = result();
        result.units[0].translated_value = "Hallo {name} {count} {count}".to_string();
        assert!(matches!(
            validate_machine_result(&request(), &result),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    #[test]
    fn result_rejects_changed_required_whitespace_shape() {
        let mut request = request();
        request.units[0].source_value = "  Hello {name} {count}\r\n".to_string();
        request.units[0].preserves_whitespace = true;
        let result = result();
        assert!(matches!(
            validate_machine_result(&request, &result),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    #[test]
    fn result_rejects_unbound_policy_or_missing_review_requirement() {
        let mut wrong_policy = result();
        wrong_policy.execution.prompt_policy_digest = "f".repeat(64);
        assert!(matches!(
            validate_machine_result(&request(), &wrong_policy),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));

        let mut missing_review_requirement = result();
        missing_review_requirement.execution.prompt_policy_digest = request().adapter_policy_digest;
        missing_review_requirement.review_required = false;
        assert!(matches!(
            validate_machine_result(&request(), &missing_review_requirement),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    async fn persistence_fixture(tombstoned: bool) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        initialize_persistence_fixture(database, tombstoned).await
    }

    #[tokio::test]
    async fn estimate_does_not_register_operation_proposal_or_memory_pin() {
        let (database, tenant_id, actor_id, operation_id, _memory_entry_id) =
            persistence_fixture(false).await;
        let port = Arc::new(EstimatingMachinePort {
            descriptor: descriptor(100),
            estimate_calls: AtomicUsize::new(0),
        });
        let service = recovery_service(database.clone(), port.clone());
        let operation_count = machine_operation::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let proposal_count = crate::entities::proposal::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let binding_count = machine_memory_binding::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let item_id = find_operation(&database, tenant_id, operation_id)
            .await
            .unwrap()
            .item_id;

        let context = recovery_context(tenant_id, actor_id);
        assert!(
            !context
                .claims
                .contains(&Permission::new(Resource::TranslationMemory, Action::Read).to_string())
        );
        let estimate = service
            .estimate_proposal(
                context.with_idempotency_key("estimate-machine-translation"),
                recovery_proposal(item_id),
            )
            .await
            .unwrap();

        assert_eq!(estimate.cost_minor_units_upper_bound, 17);
        assert_eq!(port.estimate_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            machine_operation::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            operation_count
        );
        assert_eq!(
            crate::entities::proposal::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            proposal_count
        );
        assert_eq!(
            machine_memory_binding::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            binding_count
        );
    }

    #[tokio::test]
    async fn generation_returns_pollable_operation_when_machine_execution_is_in_progress() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let port = Arc::new(InProgressMachinePort {
            descriptor: descriptor(100),
            translate_calls: AtomicUsize::new(0),
        });
        let service = recovery_service(database.clone(), port.clone());
        let input = prepare_registered_generation(&service, tenant_id, operation_id).await;
        let operation_count = machine_operation::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let proposal_count = crate::entities::proposal::Entity::find()
            .count(&database)
            .await
            .unwrap();

        let outcome = service
            .generate_proposal(
                machine_control_context(tenant_id, actor_id, "fixture-machine").with_claim(
                    Permission::new(Resource::Translations, Action::Update).to_string(),
                ),
                input,
            )
            .await
            .unwrap();

        let MachineProposalOutcome::InProgress(status) = outcome else {
            panic!("expected a pollable in-progress machine operation");
        };
        assert_eq!(status.operation_id, operation_id);
        assert_eq!(status.status, "registered");
        assert_eq!(status.provider_status, "running");
        assert_eq!(
            status.provider_execution_id.as_deref(),
            Some("execution-in-progress")
        );
        assert_eq!(port.translate_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            machine_operation::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            operation_count
        );
        assert_eq!(
            crate::entities::proposal::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            proposal_count
        );
    }

    async fn persistence_fixture_at(
        database_path: &Path,
        tombstoned: bool,
    ) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        let database = connect_persistence_file(database_path, true).await;
        initialize_persistence_fixture(database, tombstoned).await
    }

    async fn connect_persistence_file(
        database_path: &Path,
        create_if_missing: bool,
    ) -> DatabaseConnection {
        let database_path = database_path.to_path_buf();
        let mut options =
            ConnectOptions::new("sqlite://translation-recovery-placeholder.sqlite?mode=rwc");
        options
            .max_connections(4)
            .min_connections(1)
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts(move |options| {
                options
                    .filename(database_path.clone())
                    .create_if_missing(create_if_missing)
            });
        let database = Database::connect(options).await.unwrap();
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        database
    }

    async fn initialize_persistence_fixture(
        database: DatabaseConnection,
        tombstoned: bool,
    ) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        database
            .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .unwrap();
        let manager = SchemaManager::new(&database);
        for migration in migrations::migrations() {
            migration.up(&manager).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)",
                [tenant_id.into()],
            ))
            .await
            .unwrap();
        let now = Utc::now().fixed_offset();
        let job_id = Uuid::new_v4();
        job::Entity::insert(job::ActiveModel {
            id: Set(job_id),
            tenant_id: Set(tenant_id),
            source_locale: Set("en".to_string()),
            target_locale: Set("de".to_string()),
            glossary_id: Set(None),
            glossary_revision: Set(None),
            status: Set("open".to_string()),
            created_by_actor_kind: Set("user".to_string()),
            created_by_actor_id: Set(Uuid::new_v4().to_string()),
            idempotency_key: Set("fixture-job".to_string()),
            request_hash: Set("a".repeat(64)),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        let item_id = Uuid::new_v4();
        job_item::Entity::insert(job_item::ActiveModel {
            id: Set(item_id),
            tenant_id: Set(tenant_id),
            job_id: Set(job_id),
            owner_slug: Set("media".to_string()),
            resource_kind: Set("asset".to_string()),
            resource_id: Set("asset-a".to_string()),
            subresource_key: Set(String::new()),
            resource_revision: Set("resource-a".to_string()),
            source_revision: Set("source-a".to_string()),
            target_revision: Set(None),
            source_snapshot: Set(serde_json::to_value(snapshot()).unwrap()),
            source_digest: Set(hash_manifest(&snapshot()).unwrap()),
            status: Set("missing".to_string()),
            current_proposal_id: Set(None),
            active_apply_operation_id: Set(None),
            assigned_actor_kind: Set(None),
            assigned_actor_id: Set(None),
            idempotency_key: Set("fixture-item".to_string()),
            request_hash: Set("c".repeat(64)),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        let memory_entry_id = Uuid::new_v4();
        memory_entry::Entity::insert(memory_entry::ActiveModel {
            id: Set(memory_entry_id),
            tenant_id: Set(tenant_id),
            source_locale: Set("en".to_string()),
            target_locale: Set("de".to_string()),
            owner_slug: Set("media".to_string()),
            resource_kind: Set("asset".to_string()),
            resource_id: Set("asset-memory".to_string()),
            subresource_id: Set(None),
            field_key: Set("alt_text".to_string()),
            source_text: Set("Hello".to_string()),
            target_text: Set("Hallo".to_string()),
            source_key: Set("d".repeat(64)),
            source_hash: Set("e".repeat(64)),
            target_hash: Set("f".repeat(64)),
            context_fingerprint: Set("1".repeat(64)),
            segmentation_version: Set("owner-field-v1".to_string()),
            origin: Set("manual".to_string()),
            quality_state: Set("human_approved_applied".to_string()),
            reviewer_actor_kind: Set("user".to_string()),
            reviewer_actor_id: Set(Uuid::new_v4().to_string()),
            proposal_id: Set(Uuid::new_v4()),
            apply_receipt_id: Set(Uuid::new_v4()),
            retention_policy: Set("owner_lifecycle".to_string()),
            retain_until: Set(None),
            owner_lifecycle_revision: Set(None),
            owner_deleted_at: Set(None),
            tombstoned_at: Set(tombstoned.then_some(now)),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        let actor_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        machine_operation::Entity::insert(machine_operation::ActiveModel {
            id: Set(operation_id),
            tenant_id: Set(tenant_id),
            item_id: Set(item_id),
            proposal_id: Set(None),
            status: Set("registered".to_string()),
            command_hash: Set("2".repeat(64)),
            machine_request_digest: Set("3".repeat(64)),
            adapter_slug: Set("ai".to_string()),
            provider_slug: Set(None),
            provider_policy_digest: Set("4".repeat(64)),
            glossary_revision: Set(None),
            glossary_digest: Set(None),
            memory_digest: Set(Some("5".repeat(64))),
            execution_id: Set(None),
            execution_request_digest: Set(None),
            prompt_policy_digest: Set(None),
            attempts: Set(serde_json::json!([])),
            usage: Set(None),
            diagnostics: Set(serde_json::json!([])),
            review_required: Set(None),
            requested_by_actor_kind: Set("user".to_string()),
            requested_by_actor_id: Set(actor_id.to_string()),
            idempotency_key: Set("fixture-machine".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        machine_memory_binding::Entity::insert(machine_memory_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            operation_id: Set(operation_id),
            unit_id: Set("alt_text".to_string()),
            batch_ordinal: Set(0),
            unit_ordinal: Set(0),
            memory_entry_id: Set(memory_entry_id),
            score_basis_points: Set(9_000),
            created_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        (database, tenant_id, actor_id, operation_id, memory_entry_id)
    }

    fn machine_control_context(
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: &str,
    ) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            format!("machine-control-{idempotency_key}"),
        )
        .with_claim(Permission::new(Resource::Translations, Action::Run).to_string())
        .with_role("manager")
        .with_idempotency_key(idempotency_key)
        .with_deadline(Duration::from_secs(5))
    }

    fn recovery_context(tenant_id: Uuid, actor_id: Uuid) -> PortContext {
        machine_control_context(tenant_id, actor_id, "recover-machine-restart")
            .with_claim(Permission::new(Resource::Translations, Action::Manage).to_string())
            .with_claim(Permission::new(Resource::Translations, Action::Update).to_string())
    }

    fn recovery_proposal(item_id: Uuid) -> GenerateMachineProposalInput {
        GenerateMachineProposalInput {
            item_id,
            field_keys: vec![FieldKey::new("alt_text").unwrap()],
            minimum_memory_similarity_basis_points: 7_000,
            tone: None,
            domain: None,
            style: None,
        }
    }

    fn recovery_service(
        database: DatabaseConnection,
        machine_port: Arc<dyn MachineTranslationPort>,
    ) -> TranslationMachineService {
        TranslationMachineService::new(
            database.clone(),
            recovery_registry(),
            Arc::new(RecoveryTenantLocalePolicies),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database))),
            machine_port,
        )
    }

    async fn prepare_saving_recovery(
        service: &TranslationMachineService,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> RecoverMachineOperationInput {
        let operation = find_operation(&service.database, tenant_id, operation_id)
            .await
            .unwrap();
        let proposal = recovery_proposal(operation.item_id);
        let item = find_item(&service.database, tenant_id, proposal.item_id)
            .await
            .unwrap();
        let request = service
            .build_request(tenant_id, &item, &snapshot(), &proposal, Some(&operation))
            .await
            .unwrap();
        let observed_updated_at = Utc::now().fixed_offset();
        machine_operation::Entity::update_many()
            .col_expr(machine_operation::Column::Status, Expr::value("saving"))
            .col_expr(
                machine_operation::Column::CommandHash,
                Expr::value(hash_manifest(&proposal).unwrap()),
            )
            .col_expr(
                machine_operation::Column::MachineRequestDigest,
                Expr::value(hash_manifest(&request).unwrap()),
            )
            .col_expr(
                machine_operation::Column::ProviderPolicyDigest,
                Expr::value(descriptor(100).policy_digest),
            )
            .col_expr(
                machine_operation::Column::UpdatedAt,
                Expr::value(observed_updated_at),
            )
            .filter(machine_operation::Column::Id.eq(operation_id))
            .exec(&service.database)
            .await
            .unwrap();
        RecoverMachineOperationInput {
            operation_id,
            expected_updated_at: observed_updated_at,
            proposal,
            reason: "Recover the completed provider result after an interrupted save".to_string(),
        }
    }

    async fn prepare_registered_generation(
        service: &TranslationMachineService,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> GenerateMachineProposalInput {
        let operation = find_operation(&service.database, tenant_id, operation_id)
            .await
            .unwrap();
        let proposal = recovery_proposal(operation.item_id);
        let item = find_item(&service.database, tenant_id, proposal.item_id)
            .await
            .unwrap();
        let request = service
            .build_request(tenant_id, &item, &snapshot(), &proposal, Some(&operation))
            .await
            .unwrap();
        machine_operation::Entity::update_many()
            .col_expr(
                machine_operation::Column::CommandHash,
                Expr::value(hash_manifest(&proposal).unwrap()),
            )
            .col_expr(
                machine_operation::Column::MachineRequestDigest,
                Expr::value(hash_manifest(&request).unwrap()),
            )
            .col_expr(
                machine_operation::Column::ProviderPolicyDigest,
                Expr::value(descriptor(100).policy_digest),
            )
            .filter(machine_operation::Column::Id.eq(operation_id))
            .exec(&service.database)
            .await
            .unwrap();
        proposal
    }

    async fn save_proposal_without_completing_operation(
        service: &TranslationMachineService,
        context: PortContext,
        operation: &machine_operation::Model,
        input: &RecoverMachineOperationInput,
    ) -> Uuid {
        let mut save_context = context;
        save_context.idempotency_key =
            Some(child_idempotency_key(&operation.idempotency_key, "save-proposal").unwrap());
        service
            .workflow
            .save_recovered_machine_proposal(
                save_context,
                SaveProposalInput {
                    item_id: input.proposal.item_id,
                    origin: ProposalOrigin::Ai,
                    values: vec![ProposalValue {
                        key: FieldKey::new("alt_text").unwrap(),
                        value: "Hallo {name} {count}".to_string(),
                    }],
                },
            )
            .await
            .unwrap()
            .id
    }

    fn sqlite_test_files(database_path: &Path) -> [PathBuf; 4] {
        [
            database_path.to_path_buf(),
            PathBuf::from(format!("{}-journal", database_path.display())),
            PathBuf::from(format!("{}-shm", database_path.display())),
            PathBuf::from(format!("{}-wal", database_path.display())),
        ]
    }

    #[tokio::test]
    async fn pinned_memory_projection_survives_tombstone() {
        let (database, tenant_id, _, operation_id, _) = persistence_fixture(true).await;
        let operation = find_operation(&database, tenant_id, operation_id)
            .await
            .unwrap();
        let snapshot = snapshot();
        let suggestions = read_pinned_memory_suggestions(
            &database,
            tenant_id,
            &operation,
            &snapshot,
            &request().units,
        )
        .await
        .unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].target_value, "Hallo");
        assert_eq!(suggestions[0].score_basis_points, 9_000);
    }

    #[tokio::test]
    async fn pinned_memory_cannot_be_purged() {
        let (database, tenant_id, actor_id, _, memory_entry_id) = persistence_fixture(true).await;
        let context = machine_control_context(tenant_id, actor_id, "purge-pinned")
            .with_claim(Permission::new(Resource::TranslationMemory, Action::Delete).to_string())
            .with_claim(Permission::new(Resource::TranslationMemory, Action::Manage).to_string());
        let error = TranslationMemoryService::new(database)
            .purge_entry(
                context,
                PurgeMemoryEntryInput {
                    entry_id: memory_entry_id,
                    expected_revision: 1,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            TranslationError::MemoryRetentionConflict(_)
        ));
    }

    #[tokio::test]
    async fn cancellation_is_actor_bound_replay_safe_and_releases_pins() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let context = machine_control_context(tenant_id, actor_id, "cancel-machine");
        let input = CancelMachineOperationInput {
            operation_id,
            reason: "Operator cancelled the pending generation".to_string(),
        };
        let first = cancel_machine_operation(&database, None, context.clone(), input.clone())
            .await
            .unwrap();
        let replay = cancel_machine_operation(&database, None, context, input)
            .await
            .unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.provider_status, "unavailable");
        assert_eq!(
            find_operation(&database, tenant_id, operation_id)
                .await
                .unwrap()
                .status,
            "cancelled"
        );
        assert!(
            machine_memory_binding::Entity::find()
                .filter(machine_memory_binding::Column::OperationId.eq(operation_id))
                .one(&database)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn cancellation_propagation_is_retried_by_the_same_receipt() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let context = machine_control_context(tenant_id, actor_id, "cancel-machine-provider");
        let input = CancelMachineOperationInput {
            operation_id,
            reason: "Operator cancelled the pending generation".to_string(),
        };
        let machine_port = CancellationMachinePort::new();

        let first = cancel_machine_operation(
            &database,
            Some(&machine_port),
            context.clone(),
            input.clone(),
        )
        .await
        .unwrap();
        assert_eq!(first.provider_status, "cancellation_requested");
        assert_eq!(first.provider_execution_id.as_deref(), Some("execution-a"));

        let replay = cancel_machine_operation(&database, Some(&machine_port), context, input)
            .await
            .unwrap();
        assert_eq!(replay.cancellation_id, first.cancellation_id);
        assert_eq!(replay.provider_status, "cancelled");
        assert_eq!(machine_port.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn operation_status_resolves_provider_by_stable_execution_key() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let context = machine_control_context(tenant_id, actor_id, "read-machine-status")
            .with_claim(Permission::new(Resource::Translations, Action::Read).to_string());
        let machine_port = CancellationMachinePort::new();

        let status =
            read_machine_operation_status(&database, Some(&machine_port), context, operation_id)
                .await
                .unwrap();
        assert_eq!(status.status, "registered");
        assert_eq!(status.provider_status, "running");
        assert_eq!(status.provider_execution_id.as_deref(), Some("execution-a"));
    }

    #[tokio::test]
    async fn separate_process_recovers_both_machine_save_crash_boundaries() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace path");
        let evidence_dir = workspace.join("target/translation-recovery-process-tests");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Translation recovery process evidence directory");
        let evidence_dir = evidence_dir
            .canonicalize()
            .expect("Translation recovery process evidence path");
        assert!(evidence_dir.starts_with(workspace.join("target")));

        for proposal_was_saved in [false, true] {
            let database_path = evidence_dir.join(format!(
                "rustok-translation-recovery-{}.sqlite",
                Uuid::new_v4()
            ));
            let _database_cleanup = SqliteTestFileGuard(database_path.clone());
            let (database, tenant_id, actor_id, operation_id, _) =
                persistence_fixture_at(&database_path, false).await;
            let machine_port = Arc::new(RecoveryMachinePort {
                descriptor: descriptor(100),
                recover_calls: AtomicUsize::new(0),
                recovered_result: Some(result()),
            });
            let service = recovery_service(database.clone(), machine_port);
            let context = recovery_context(tenant_id, actor_id);
            let input = prepare_saving_recovery(&service, tenant_id, operation_id).await;
            let expected_proposal_id = if proposal_was_saved {
                let operation = find_operation(&database, tenant_id, operation_id)
                    .await
                    .unwrap();
                Some(
                    save_proposal_without_completing_operation(
                        &service,
                        context.clone(),
                        &operation,
                        &input,
                    )
                    .await,
                )
            } else {
                None
            };
            drop(service);
            database.close().await.unwrap();

            let output =
                Command::new(std::env::current_exe().expect("Translation test executable"))
                    .args([
                        "--exact",
                        "machine_service::tests::machine_recovery_child_process",
                        "--ignored",
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env(
                        "RUSTOK_TRANSLATION_TEST_MACHINE_RECOVERY_DB_PATH",
                        &database_path,
                    )
                    .output()
                    .await
                    .expect("Translation recovery child process");
            assert!(
                output.status.success(),
                "Translation recovery child failed for proposal_was_saved={proposal_was_saved}:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            let database = connect_persistence_file(&database_path, false).await;
            let operation = find_operation(&database, tenant_id, operation_id)
                .await
                .unwrap();
            assert_eq!(operation.status, "completed");
            assert_eq!(operation.provider_slug.as_deref(), Some("provider-a"));
            assert_eq!(operation.execution_id.as_deref(), Some("execution-a"));
            let proposal_id = operation.proposal_id.unwrap();
            if let Some(expected_proposal_id) = expected_proposal_id {
                assert_eq!(proposal_id, expected_proposal_id);
            }
            assert_eq!(
                crate::entities::proposal::Entity::find()
                    .filter(crate::entities::proposal::Column::TenantId.eq(tenant_id))
                    .filter(crate::entities::proposal::Column::ItemId.eq(operation.item_id))
                    .count(&database)
                    .await
                    .unwrap(),
                1
            );
            assert_eq!(
                machine_recovery::Entity::find()
                    .filter(machine_recovery::Column::OperationId.eq(operation_id))
                    .count(&database)
                    .await
                    .unwrap(),
                1
            );
            assert!(
                machine_memory_binding::Entity::find()
                    .filter(machine_memory_binding::Column::OperationId.eq(operation_id))
                    .one(&database)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                find_item(&database, tenant_id, operation.item_id)
                    .await
                    .unwrap()
                    .current_proposal_id,
                Some(proposal_id)
            );

            let replay_machine_port = Arc::new(RecoveryMachinePort {
                descriptor: descriptor(100),
                recover_calls: AtomicUsize::new(0),
                recovered_result: Some(result()),
            });
            let replay_service = recovery_service(database.clone(), replay_machine_port.clone());
            let replay = replay_service
                .recover_operation(context, input)
                .await
                .unwrap();
            assert_eq!(replay.proposal_id, proposal_id);
            assert_eq!(replay_machine_port.recover_calls.load(Ordering::SeqCst), 0);
            drop(replay_service);
            database.close().await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "internal child process for Translation machine recovery evidence"]
    async fn machine_recovery_child_process() {
        let Some(database_path) =
            std::env::var_os("RUSTOK_TRANSLATION_TEST_MACHINE_RECOVERY_DB_PATH")
        else {
            return;
        };
        let database = connect_persistence_file(Path::new(&database_path), false).await;
        let operation = machine_operation::Entity::find()
            .filter(machine_operation::Column::Status.eq("saving"))
            .one(&database)
            .await
            .unwrap()
            .expect("child process must observe a saving operation");
        let tenant_id = operation.tenant_id;
        let actor_id = Uuid::parse_str(&operation.requested_by_actor_id)
            .expect("fixture machine operation actor");
        let context = recovery_context(tenant_id, actor_id);
        let input = RecoverMachineOperationInput {
            operation_id: operation.id,
            expected_updated_at: operation.updated_at,
            proposal: recovery_proposal(operation.item_id),
            reason: "Recover the completed provider result after an interrupted save".to_string(),
        };
        let machine_port = Arc::new(RecoveryMachinePort {
            descriptor: descriptor(100),
            recover_calls: AtomicUsize::new(0),
            recovered_result: Some(result()),
        });
        let service = recovery_service(database.clone(), machine_port.clone());

        let recovered = service.recover_operation(context, input).await.unwrap();
        assert_eq!(recovered.operation_id, operation.id);
        assert_eq!(machine_port.recover_calls.load(Ordering::SeqCst), 1);
        drop(service);
        database.close().await.unwrap();
    }

    #[tokio::test]
    async fn stuck_save_recovery_is_audited_and_never_starts_a_new_execution() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let machine_port = Arc::new(RecoveryMachinePort {
            descriptor: descriptor(100),
            recover_calls: AtomicUsize::new(0),
            recovered_result: None,
        });
        let service = TranslationMachineService::new(
            database.clone(),
            Arc::new(TranslationTargetRegistry::default()),
            Arc::new(RecoveryTenantLocalePolicies),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone()))),
            machine_port.clone(),
        );
        let context = machine_control_context(tenant_id, actor_id, "recover-machine")
            .with_claim(Permission::new(Resource::Translations, Action::Manage).to_string())
            .with_claim(Permission::new(Resource::Translations, Action::Update).to_string());
        let operation = find_operation(&database, tenant_id, operation_id)
            .await
            .unwrap();
        let proposal = GenerateMachineProposalInput {
            item_id: operation.item_id,
            field_keys: vec![FieldKey::new("alt_text").unwrap()],
            minimum_memory_similarity_basis_points: 7_000,
            tone: None,
            domain: None,
            style: None,
        };
        let item = find_item(&database, tenant_id, proposal.item_id)
            .await
            .unwrap();
        let request = service
            .build_request(tenant_id, &item, &snapshot(), &proposal, Some(&operation))
            .await
            .unwrap();
        let observed_updated_at = Utc::now().fixed_offset();
        machine_operation::Entity::update_many()
            .col_expr(machine_operation::Column::Status, Expr::value("saving"))
            .col_expr(
                machine_operation::Column::CommandHash,
                Expr::value(hash_manifest(&proposal).unwrap()),
            )
            .col_expr(
                machine_operation::Column::MachineRequestDigest,
                Expr::value(hash_manifest(&request).unwrap()),
            )
            .col_expr(
                machine_operation::Column::ProviderPolicyDigest,
                Expr::value(descriptor(100).policy_digest),
            )
            .col_expr(
                machine_operation::Column::UpdatedAt,
                Expr::value(observed_updated_at),
            )
            .filter(machine_operation::Column::Id.eq(operation_id))
            .exec(&database)
            .await
            .unwrap();
        let input = RecoverMachineOperationInput {
            operation_id,
            expected_updated_at: observed_updated_at,
            proposal,
            reason: "Recover the completed provider result after an interrupted save".to_string(),
        };

        for attempt_context in [context.clone(), context] {
            let error = service
                .recover_operation(attempt_context, input.clone())
                .await
                .unwrap_err();
            assert!(matches!(
                error,
                TranslationError::MachineRecoveryResultUnavailable
            ));
        }
        assert_eq!(machine_port.recover_calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            machine_recovery::Entity::find()
                .filter(machine_recovery::Column::OperationId.eq(operation_id))
                .count(&database)
                .await
                .unwrap(),
            1
        );
    }
