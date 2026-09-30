#![allow(unused_imports)]

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait, Value};
use semver::Version;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::fixtures::*;
use super::*;
use crate::build::{
    ModuleBuildAuthoring, ModuleBuildComponentInterface, ModuleBuildDependencyPolicy,
    ModuleBuildEvidence, ModuleBuildLimits, ModuleBuildMetrics, ModuleBuildNetworkPolicy,
    ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildPublicationReceipt,
    ModuleBuildRequest, ModuleBuildResult, ModuleBuildScenario, ModuleBuildSignatureAuthority,
    ModuleBuildSource, ModuleBuildToolchain, ModuleBuildValidationOutcome,
    ModuleBuildValidationProfile, ModuleBuildValidationResult, ModuleBuildWitContract,
};
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::{
    ArtifactBlobStore, ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef,
    ControlPlaneInfrastructure, InMemoryArtifactBlobStore, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleMarketplaceArtifactOrigin, ModuleMarketplaceArtifactRelease,
    ModuleMarketplaceEntry, ModuleMarketplaceEvidenceKind, ModuleMarketplaceEvidenceReference,
    TrustEvidenceKind, TrustEvidenceReference, MODULE_BUILD_COMPONENT_TARGET,
    MODULE_BUILD_PROTOCOL_VERSION, MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION,
    MODULE_BUILD_WIT_WORLD,
};


    #[test]
    fn governance_error_categories_and_codes_are_stable() {
        let cases = [
            (
                ModuleGovernanceError::InvalidLifecycleQuery,
                ModuleGovernanceErrorCategory::InvalidInput,
                "module_governance_invalid_input",
            ),
            (
                ModuleGovernanceError::ReleaseYankUnauthorized,
                ModuleGovernanceErrorCategory::PermissionDenied,
                "module_governance_permission_denied",
            ),
            (
                ModuleGovernanceError::PublishRequestNotFound,
                ModuleGovernanceErrorCategory::NotFound,
                "module_governance_not_found",
            ),
            (
                ModuleGovernanceError::PublicationIdempotencyConflict,
                ModuleGovernanceErrorCategory::Conflict,
                "module_governance_conflict",
            ),
            (
                ModuleGovernanceError::ReleaseYankIdempotencyConflict,
                ModuleGovernanceErrorCategory::Conflict,
                "module_governance_conflict",
            ),
            (
                ModuleGovernanceError::PublishRequestReviewIdempotencyConflict,
                ModuleGovernanceErrorCategory::Conflict,
                "module_governance_conflict",
            ),
            (
                ModuleGovernanceError::ValidationStageReportIdempotencyConflict,
                ModuleGovernanceErrorCategory::Conflict,
                "module_governance_conflict",
            ),
            (
                ModuleGovernanceError::ValidationJobEnqueueIdempotencyConflict,
                ModuleGovernanceErrorCategory::Conflict,
                "module_governance_conflict",
            ),
            (
                ModuleGovernanceError::PublishRequestArtifactIdempotencyConflict,
                ModuleGovernanceErrorCategory::Conflict,
                "module_governance_conflict",
            ),
            (
                ModuleGovernanceError::Store("persistence failure".to_string()),
                ModuleGovernanceErrorCategory::Internal,
                "module_governance_internal",
            ),
        ];

        for (error, category, code) in cases {
            assert_eq!(error.category(), category);
            assert_eq!(error.code(), code);
        }
    }


    #[test]
    fn publish_request_contract_accepts_transport_ui_object_and_derives_warnings() {
        let command = publish_request_create_command();

        assert!(command.validate().is_ok());
        assert_eq!(
            command.validation_warnings().expect("owner warnings"),
            vec![
                "No publishable admin/storefront UI packages declared; only backend contract would be published."
                    .to_string(),
                "Third-party publishing requires the configured governance and evidence gates before release."
                    .to_string(),
            ]
        );
    }


    #[test]
    fn publish_request_identity_is_deterministic_and_binds_the_full_command() {
        let command = publish_request_create_command();
        let first = publish_request_create_identity(&command).expect("request identity");
        let repeated = publish_request_create_identity(&command).expect("repeated identity");
        let mut context_changed = command.clone();
        context_changed.context.trace_id = "test:request-create:other-trace".to_string();
        context_changed.context.correlation_id = Uuid::new_v4();
        let context_changed =
            publish_request_create_identity(&context_changed).expect("context-free identity");
        let mut changed = command.clone();
        changed.description =
            "A different publish request description long enough for policy.".to_string();
        let changed = publish_request_create_identity(&changed).expect("changed identity");
        let mut privilege_changed = command;
        privilege_changed.actor_can_manage_modules = true;
        let privilege_changed =
            publish_request_create_identity(&privilege_changed).expect("privilege-bound identity");

        assert_eq!(first, repeated);
        assert_eq!(first, context_changed);
        assert_eq!(first.0, format!("rpr_{}", first.1));
        assert_ne!(first, changed);
        assert_ne!(first, privilege_changed);
    }


    #[test]
    fn follow_up_stage_contract_is_selected_by_artifact_origin() {
        let platform = publication_follow_up_stages(ModulePublicationArtifactOrigin::PlatformBuilt);
        assert_eq!(
            platform.iter().map(|stage| stage.key).collect::<Vec<_>>(),
            vec!["compile_smoke", "targeted_tests"]
        );
        assert!(
            platform
                .iter()
                .all(|stage| stage.runner_kind == "owner_evidence")
        );

        let external =
            publication_follow_up_stages(ModulePublicationArtifactOrigin::ExternalPrebuilt);
        assert_eq!(external.len(), 1);
        assert_eq!(external[0].key, "security_policy_review");
        assert_eq!(external[0].runner_kind, "owner_evidence");

        let alloy = publication_follow_up_stages(ModulePublicationArtifactOrigin::AlloyAuthored);
        assert_eq!(alloy.len(), 1);
        assert_eq!(alloy[0].key, "security_policy_review");
        assert_eq!(alloy[0].runner_kind, "owner_evidence");
    }


    #[test]
    fn lifecycle_stage_metadata_is_derived_only_by_the_owner_contract() {
        let compile = governance_validation_stage_snapshot(
            "compile_smoke",
            "queued",
            "waiting",
            0,
            "2026-07-22T00:00:00Z".to_string(),
            None,
            None,
        );
        assert_eq!(compile.execution_mode, "remote");
        assert!(compile.runnable);
        assert!(!compile.requires_manual_confirmation);
        assert_eq!(
            compile.suggested_failure_reason_code.as_deref(),
            Some("build_failure")
        );

        let security = governance_validation_stage_snapshot(
            "security_policy_review",
            "queued",
            "waiting",
            0,
            "2026-07-22T00:00:00Z".to_string(),
            None,
            None,
        );
        assert_eq!(security.execution_mode, "manual_review");
        assert!(security.runnable);
        assert!(security.requires_manual_confirmation);
        assert_eq!(
            security.suggested_pass_reason_code.as_deref(),
            Some("manual_review_complete")
        );
        assert_eq!(
            security.allowed_terminal_reason_codes,
            REGISTRY_VALIDATION_STAGE_REASON_CODES
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        );
    }


    #[test]
    fn lifecycle_actions_fail_closed_for_unpassed_approval_gates() {
        let request = governance_request_snapshot("approved", "platform_built");
        let validation_stages = vec![governance_validation_stage_snapshot(
            "compile_smoke",
            "failed",
            "failed",
            1,
            "2026-07-22T00:00:00Z".to_string(),
            None,
            Some("2026-07-22T00:00:00Z".to_string()),
        )];
        let actions = derive_governance_actions(Some(&request), None, None, &validation_stages);
        let approve = actions
            .iter()
            .find(|action| action.key == "approve")
            .expect("approve action");
        assert!(approve.reason_required);
        assert!(approve.reason_code_required);
        assert_eq!(
            approve.reason_codes,
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        );
        assert!(actions.iter().any(|action| action.key == "request_changes"));
        assert!(actions.iter().any(|action| action.key == "hold"));
        assert!(actions.iter().any(|action| action.key == "reject"));
    }


    #[test]
    fn lifecycle_rejects_unknown_durable_artifact_origin() {
        let request = GovernanceRequestRow {
            snapshot: governance_request_snapshot("approved", "unknown_origin"),
            validated_at: Some("2026-07-22T00:00:00Z".to_string()),
            approved_at: None,
        };
        let error = derive_governance_validation_stages(Some(&request), &[], &[])
            .expect_err("unknown durable origin must fail closed");
        assert!(matches!(
            error,
            ModuleGovernanceError::InvalidLifecycleArtifactOrigin(origin)
                if origin == "unknown_origin"
        ));
    }


    #[test]
    fn publish_status_next_action_is_owner_derived_and_finishes_after_required_stages_pass() {
        let request = governance_request_snapshot("approved", "external_prebuilt");
        assert_eq!(
            publish_request_next_action(&request, true).expect("pending stage action"),
            Some(ModuleGovernancePublishRequestNextAction::StageExternalPrebuilt)
        );
        assert_eq!(
            publish_request_next_action(&request, false).expect("completed stage action"),
            Some(ModuleGovernancePublishRequestNextAction::FinalizePublication)
        );

        let changes_requested =
            governance_request_snapshot("changes_requested", "external_prebuilt");
        assert_eq!(
            publish_request_next_action(&changes_requested, false).expect("revision action"),
            Some(ModuleGovernancePublishRequestNextAction::UploadArtifactRevision)
        );

        let invalid = governance_request_snapshot("unknown", "external_prebuilt");
        assert!(matches!(
            publish_request_next_action(&invalid, false),
            Err(ModuleGovernanceError::InvalidPublishRequestStatus(status)) if status == "unknown"
        ));
    }


    #[test]
    fn publication_translations_require_canonical_locales_and_the_default_row() {
        let translations = vec![(
            "en-US".to_string(),
            "Sample module".to_string(),
            "A localized description long enough for publication.".to_string(),
        )];
        assert!(valid_publication_translations("en-US", &translations));
        assert!(!valid_publication_translations("fr-FR", &translations));

        let mut invalid = translations;
        invalid[0].0 = "en-us".to_string();
        assert!(!valid_publication_translations("en-US", &invalid));

        let unsafe_text = vec![(
            "en-US".to_string(),
            "Sample\u{202e} module".to_string(),
            "A localized description long enough for publication.".to_string(),
        )];
        assert!(!valid_publication_translations("en-US", &unsafe_text));
    }


    #[test]
    fn publish_request_contract_rejects_noncanonical_locale_and_ui_array() {
        let mut command = publish_request_create_command();
        command.default_locale = "not a locale".to_string();
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand)
        ));

        let mut command = publish_request_create_command();
        command.ui_packages = serde_json::json!([]);
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand)
        ));

        let mut command = publish_request_create_command();
        command.description =
            "A long description with an invisible\u{2066} direction isolate.".to_string();
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand)
        ));

        let mut command = publish_request_create_command();
        command.marketplace = serde_json::json!({
            "category": "utilities",
            "tags": ["safe", "unsafe\u{202e}tag"],
        });
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestCreateCommand)
        ));
    }


    #[test]
    fn release_yank_contract_rejects_unrecognized_reason_codes() {
        let actor_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4();
        let command = ModuleReleaseYankCommand {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            reason: "security remediation".to_string(),
            reason_code: "unrecognized".to_string(),
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: format!("test:yank:{idempotency_key}"),
                correlation_id: idempotency_key,
                idempotency_key,
            },
            actor_principal: serde_json::json!({
                "kind": "user",
                "user_id": actor_id,
                "subject": format!("user:{actor_id}"),
            }),
            actor_can_manage_modules: false,
        };
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidYankReasonCode(_))
        ));
    }


    #[test]
    fn publication_contract_requires_structured_principals_and_reviewed_override() {
        let actor_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4();
        let command = ModulePublishRequestPublicationCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: None,
                trace_id: format!("test:publication:{idempotency_key}"),
                correlation_id: idempotency_key,
                idempotency_key,
            },
            actor_principal: serde_json::json!({
                "kind": "user",
                "user_id": actor_id,
                "subject": format!("user:{actor_id}"),
            }),
            publisher_principal: serde_json::json!("publisher"),
            allow_owner_rebind: false,
            approval_override: Some(ModulePublishApprovalOverride {
                reason: "manual review".to_string(),
                reason_code: "manual_review_complete".to_string(),
                validation_stages: serde_json::json!([]),
            }),
        };
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPublishRequestPublicationCommand)
        ));
    }


    #[test]
    fn author_signature_evidence_requires_platform_context_and_canonical_signature_digest() {
        let actor_id = Uuid::new_v4();
        let command = ModuleAuthorSignatureEvidenceCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: Some(Uuid::new_v4()),
                trace_id: "test:author-signature".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_can_manage_modules: false,
            evidence_reference: "registry://publish-requests/request-1/marketplace-approval"
                .to_string(),
            signature_digest_sha256: "a".repeat(64),
            signer_identity: "operator".to_string(),
            policy_revision: "registry-governance-v1".to_string(),
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
        };
        assert!(matches!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidAuthorSignatureEvidenceCommand)
        ));
        let mut malformed_signature = command;
        malformed_signature.context.tenant_id = None;
        malformed_signature.signature_digest_sha256 = "not-a-digest".to_string();
        assert!(matches!(
            malformed_signature.validate(),
            Err(ModuleGovernanceError::InvalidAuthorSignatureEvidenceCommand)
        ));
        assert!(matches!(
            ModulePublishArtifactAttachCommand {
                request_id: "request-1".to_string(),
                expected_revision: 1,
                context: ModuleCommandContext {
                    actor_id,
                    tenant_id: None,
                    trace_id: "test:invalid-publish-artifact".to_string(),
                    correlation_id: Uuid::new_v4(),
                    idempotency_key: Uuid::new_v4(),
                },
                actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
                actor_can_manage_modules: false,
                checksum_sha256: "SHA256:ABC".to_string(),
                artifact_size: 1,
                content_type: "application/octet-stream".to_string(),
            }
            .validate(),
            Err(ModuleGovernanceError::InvalidPublishArtifactAttachCommand)
        ));
    }


    #[test]
    fn rejected_request_retry_policy_stays_owner_derived() {
        assert!(governance_rejected_request_can_retry(
            Some("validation_failed"),
            Some("Governance rejection reason: validation failed"),
        ));
        assert!(governance_rejected_request_can_retry(
            None,
            Some("Validation job failed before bundle checks: missing artifact"),
        ));
        assert!(!governance_rejected_request_can_retry(
            Some("request_rejected"),
            Some("Governance rejection reason: owner mismatch"),
        ));
    }


    #[test]
    fn validation_stage_detail_contains_only_owner_vocabulary() {
        let detail =
            content_free_validation_stage_detail("targeted_tests", "failed", Some("test_failure"));

        assert_eq!(
            detail,
            "Registry validation stage 'targeted_tests' entered status 'failed' with reason code 'test_failure'."
        );
        assert!(!detail.contains("stdout"));
        assert!(!detail.contains("artifact"));
    }
