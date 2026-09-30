use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

use super::fixtures::*;
use super::*;
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::publication_evidence::{
    ModulePublicationTrustEvidence, ModulePublicationTrustReport,
    ModulePublicationTrustSubject, ModulePublicationTrustVerifier,
};
use crate::publish_validation::{
    ModulePublishValidationCheck, ModulePublishValidationDetails,
    ModulePublishValidationResultOutcome,
};
use crate::{ControlPlaneInfrastructure, ModuleCommandContext};


    #[test]
    fn publish_request_authorization_is_derived_from_durable_owner_facts() {
        let publisher = serde_json::json!({ "kind": "user", "id": "publisher" });
        let owner = serde_json::json!({ "kind": "user", "id": "owner" });
        let unrelated = serde_json::json!({ "kind": "user", "id": "unrelated" });
        let worker = serde_json::json!({ "kind": "service", "id": "worker" });

        assert!(governance_actor_can_create_publish_request(
            None, &publisher, false,
        ));
        assert!(!governance_actor_can_create_publish_request(
            None, &worker, false,
        ));
        assert!(governance_actor_can_create_publish_request(
            Some(&owner),
            &owner,
            false,
        ));
        assert!(!governance_actor_can_create_publish_request(
            Some(&owner),
            &publisher,
            false,
        ));
        assert!(governance_actor_can_create_publish_request(
            Some(&owner),
            &unrelated,
            true,
        ));

        assert!(governance_actor_can_manage_request_principals(
            &publisher,
            Some(&publisher),
            None,
            &publisher,
            false,
        ));
        assert!(!governance_actor_can_manage_request_principals(
            &publisher,
            Some(&publisher),
            Some(&owner),
            &publisher,
            false,
        ));
        assert!(governance_actor_can_manage_request_principals(
            &publisher,
            Some(&publisher),
            Some(&owner),
            &owner,
            false,
        ));
        assert!(governance_actor_can_manage_request_principals(
            &publisher,
            Some(&publisher),
            Some(&owner),
            &unrelated,
            true,
        ));
        assert!(governance_actor_can_manage_release(
            &publisher, None, &publisher, false,
        ));
        assert!(governance_actor_can_manage_release(
            &publisher,
            Some(&owner),
            &owner,
            false,
        ));
        assert!(!governance_actor_can_manage_release(
            &publisher,
            Some(&owner),
            &unrelated,
            false,
        ));
        assert!(governance_actor_can_transfer_owner(&owner, &owner, false));
        assert!(!governance_actor_can_transfer_owner(
            &owner, &unrelated, false
        ));
        assert!(governance_actor_can_transfer_owner(
            &owner, &unrelated, true
        ));
    }


    #[tokio::test]
    async fn owner_rejects_unprivileged_publish_request_creation_before_writing() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE registry_module_owners (\
                    slug TEXT PRIMARY KEY, owner_principal TEXT NOT NULL, \
                    bound_by_principal TEXT NOT NULL, bound_at TEXT NOT NULL, updated_at TEXT NOT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("owner schema");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_module_owners \
                 (slug, owner_principal, bound_by_principal, bound_at, updated_at) VALUES \
                 ('sample_module', '{\"kind\":\"user\",\"id\":\"owner\"}', \
                  '{\"kind\":\"user\",\"id\":\"operator\"}', datetime('now'), datetime('now'))"
                    .to_string(),
            ))
            .await
            .expect("owner fixture");
        let service = SeaOrmModuleGovernanceService::new(database);
        assert_eq!(
            service
                .create_publish_request(publish_request_create_command())
                .await,
            Err(ModuleGovernanceError::PublishRequestCreationUnauthorized)
        );
    }


    #[tokio::test]
    async fn request_follow_up_projection_is_request_scoped_and_actor_filtered() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let owner_id = Uuid::new_v4();
        let owner = serde_json::json!({
            "kind": "user",
            "user_id": owner_id,
            "subject": format!("user:{owner_id}"),
            "display_label": format!("user:{owner_id}")
        })
        .to_string();
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL, artifact_origin TEXT NOT NULL,\
                requested_by_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                approved_by_principal TEXT NULL, rejected_by_principal TEXT NULL, rejection_reason TEXT NULL,\
                changes_requested_by_principal TEXT NULL, changes_requested_reason TEXT NULL,\
                changes_requested_reason_code TEXT NULL, changes_requested_at TEXT NULL,\
                held_by_principal TEXT NULL, held_reason TEXT NULL, held_reason_code TEXT NULL,\
                held_at TEXT NULL, held_from_status TEXT NULL, validation_warnings TEXT NOT NULL,\
                validation_errors TEXT NOT NULL, validated_at TEXT NULL, approved_at TEXT NULL,\
                artifact_storage_key TEXT NULL, artifact_checksum_sha256 TEXT NULL,\
                artifact_size INTEGER NULL, artifact_content_type TEXT NULL,\
                submitted_at TEXT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, published_at TEXT NULL\
             )",
            "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY, owner_principal TEXT NOT NULL, bound_by_principal TEXT NOT NULL,\
                bound_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_validation_stages (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, stage_key TEXT NOT NULL, status TEXT NOT NULL,\
                detail TEXT NOT NULL, attempt_number INTEGER NOT NULL, updated_at TEXT NOT NULL,\
                started_at TEXT NULL, finished_at TEXT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_artifact_operations (\
                operation_id TEXT PRIMARY KEY, request_id TEXT NOT NULL, idempotency_key TEXT NOT NULL,\
                expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL, trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL, actor_principal TEXT NOT NULL, actor_can_manage_modules INTEGER NOT NULL,\
                checksum_sha256 TEXT NOT NULL, artifact_size INTEGER NOT NULL, content_type TEXT NOT NULL,\
                artifact_storage_key TEXT NOT NULL, previous_storage_key TEXT NULL,\
                reuploaded_after_changes_requested INTEGER NOT NULL, committed_at TEXT NOT NULL,\
                UNIQUE (request_id, idempotency_key)\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema");
        }
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests (\
                    id, slug, version, status, artifact_origin, requested_by_principal, publisher_principal,\
                    validation_warnings, validation_errors, validated_at, created_at, updated_at\
                 ) VALUES (?, ?, '1.0.0', 'approved', 'external_prebuilt', ?, ?, '[]', '[]', ?, ?, ?)"
                    .to_string(),
                vec![
                    "request-1".into(),
                    "sample_module".into(),
                    owner.clone().into(),
                    owner.clone().into(),
                    "2026-08-08T00:00:00Z".into(),
                    "2026-08-08T00:00:00Z".into(),
                    "2026-08-08T00:00:00Z".into(),
                ],
            ))
            .await
            .expect("request fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_module_owners (\
                    slug, owner_principal, bound_by_principal, bound_at, updated_at\
                 ) VALUES (?, ?, ?, ?, ?)"
                    .to_string(),
                vec![
                    "sample_module".into(),
                    owner.clone().into(),
                    owner.clone().into(),
                    "2026-08-08T00:00:00Z".into(),
                    "2026-08-08T00:00:00Z".into(),
                ],
            ))
            .await
            .expect("owner fixture");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_validation_stages (\
                    id, request_id, stage_key, status, detail, attempt_number, updated_at, created_at\
                 ) VALUES (?, ?, 'security_policy_review', 'failed', ?, 1, ?, ?)"
                    .to_string(),
                vec![
                    "stage-1".into(),
                    "request-1".into(),
                    "Manual review found a policy gap.".into(),
                    "2026-08-08T00:00:00Z".into(),
                    "2026-08-08T00:00:00Z".into(),
                ],
            ))
            .await
            .expect("stage fixture");

        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let owner_actor = ModuleGovernanceActorContext {
            principal: serde_json::from_str(&owner).expect("owner principal"),
            can_manage_modules: false,
        };
        let status = service
            .publish_request_status_snapshot("request-1", Some(&owner_actor))
            .await
            .expect("owner status projection")
            .expect("request status projection");
        assert_eq!(status.request.slug, "sample_module");
        assert_eq!(status.request.version, "1.0.0");
        assert!(status.accepted);
        assert!(status.authorization.can_manage);
        assert!(status.authorization.can_review);
        assert!(status.approval_override_required);
        assert_eq!(
            status.next_action,
            Some(ModuleGovernancePublishRequestNextAction::StageExternalPrebuilt)
        );
        assert_eq!(
            status.approval_override_reason_codes,
            REGISTRY_APPROVE_OVERRIDE_REASON_CODES
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        );
        assert!(
            status
                .approval_override_warning
                .as_deref()
                .is_some_and(|warning| warning.contains("security_policy_review (failed)"))
        );
        assert_eq!(status.validation_stages.len(), 1);
        assert_eq!(status.validation_stages[0].key, "security_policy_review");
        assert_eq!(status.validation_stages[0].status, "failed");
        assert_eq!(
            status
                .governance_actions
                .iter()
                .map(|action| action.key.as_str())
                .collect::<Vec<_>>(),
            vec!["approve", "request_changes", "hold", "reject"]
        );

        let anonymous = service
            .publish_request_status_snapshot("request-1", None)
            .await
            .expect("anonymous status projection")
            .expect("request status projection");
        assert!(!anonymous.authorization.can_manage);
        assert!(!anonymous.authorization.can_review);
        assert!(anonymous.governance_actions.is_empty());

        let unrelated_actor = ModuleGovernanceActorContext {
            principal: serde_json::json!({
                "kind": "user",
                "user_id": Uuid::new_v4(),
                "subject": "user:other",
                "display_label": "user:other"
            }),
            can_manage_modules: false,
        };
        let denied = service
            .publish_request_status_snapshot("request-1", Some(&unrelated_actor))
            .await
            .expect("denied projection")
            .expect("request projection");
        assert!(!denied.authorization.can_manage);
        assert!(!denied.authorization.can_review);
        assert!(denied.governance_actions.is_empty());

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_requests SET status = 'draft', artifact_storage_key = NULL, \
                 artifact_checksum_sha256 = NULL, artifact_size = NULL, artifact_content_type = NULL \
                 WHERE id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("draft fixture");
        let checksum = "a".repeat(64);
        let upload = ModulePublishArtifactAttachCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id: owner_id,
                tenant_id: None,
                trace_id: "test:publish-artifact-upload".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            actor_principal: owner_actor.principal.clone(),
            actor_can_manage_modules: false,
            checksum_sha256: checksum.clone(),
            artifact_size: 7,
            content_type: "application/wasm".to_string(),
        };
        let mut stale_upload = upload.clone();
        stale_upload.expected_revision = 2;
        assert_eq!(
            service.prepare_publish_artifact_upload(&stale_upload).await,
            Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: 2,
                current: 1,
            })
        );
        let slot = service
            .prepare_publish_artifact_upload(&upload)
            .await
            .expect("owner upload slot");
        assert!(!slot.artifact_already_attached);
        assert_eq!(
            slot.artifact_storage_key,
            format!("registry-publish-artifact/objects/platform/sha256/aa/aa/{checksum}")
        );
        let attached = service
            .attach_publish_artifact(upload.clone())
            .await
            .expect("attach publish artifact");
        assert_eq!(attached.artifact_storage_key, slot.artifact_storage_key);
        assert_eq!(attached.previous_storage_key, None);
        assert!(!attached.reuploaded_after_changes_requested);
        assert_eq!(
            service
                .attach_publish_artifact(upload.clone())
                .await
                .expect("exact attach replay"),
            attached
        );
        let mut changed_attach_replay = upload.clone();
        changed_attach_replay.context.trace_id = "test:changed-artifact-trace".to_string();
        assert_eq!(
            service.attach_publish_artifact(changed_attach_replay).await,
            Err(ModuleGovernanceError::PublishRequestArtifactIdempotencyConflict)
        );

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE registry_publish_requests SET status = 'submitted', artifact_storage_key = ?, \
                 artifact_checksum_sha256 = ?, artifact_size = ?, artifact_content_type = ? \
                 WHERE id = 'request-1'"
                    .to_string(),
                vec![
                    slot.artifact_storage_key.clone().into(),
                    checksum.clone().into(),
                    7_i64.into(),
                    "application/wasm".into(),
                ],
            ))
            .await
            .expect("attached fixture");
        let replay = service
            .prepare_publish_artifact_upload(&upload)
            .await
            .expect("idempotent upload slot");
        assert!(replay.artifact_already_attached);
        assert_eq!(replay.artifact_storage_key, slot.artifact_storage_key);

        let mut mismatch = upload.clone();
        mismatch.checksum_sha256 = "b".repeat(64);
        assert_eq!(
            service.prepare_publish_artifact_upload(&mismatch).await,
            Err(ModuleGovernanceError::PublishRequestArtifactReplayConflict)
        );
        let mut unauthorized = upload;
        unauthorized.actor_principal = unrelated_actor.principal;
        unauthorized.context.actor_id = governance_principal_user_id(&unauthorized.actor_principal)
            .expect("unrelated actor id")
            .parse()
            .expect("unrelated actor UUID");
        assert_eq!(
            service.prepare_publish_artifact_upload(&unauthorized).await,
            Err(ModuleGovernanceError::PublishRequestArtifactUploadUnauthorized)
        );
    }


    #[tokio::test]
    async fn artifact_download_snapshot_exposes_only_attached_delivery_facts() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "CREATE TABLE registry_publish_requests (\
                    id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, artifact_storage_key TEXT NULL, artifact_content_type TEXT NULL\
                 )"
                .to_string(),
            ))
            .await
            .expect("schema");
        for statement in [
            "INSERT INTO registry_publish_requests (id, artifact_storage_key, artifact_content_type) \
             VALUES ('attached', 'registry/publish/attached.wasm', 'application/wasm')",
            "INSERT INTO registry_publish_requests (id, artifact_storage_key, artifact_content_type) \
             VALUES ('draft', NULL, NULL)",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("fixture");
        }

        let service = SeaOrmModuleGovernanceService::new(database);
        let attached = service
            .publish_artifact_download_snapshot("attached")
            .await
            .expect("download projection")
            .expect("attached artifact");
        assert_eq!(attached.storage_key, "registry/publish/attached.wasm");
        assert_eq!(attached.content_type, "application/wasm");
        assert!(
            service
                .publish_artifact_download_snapshot("draft")
                .await
                .expect("draft projection")
                .is_none()
        );
        assert!(
            service
                .publish_artifact_download_snapshot("missing")
                .await
                .expect("missing projection")
                .is_none()
        );
    }


    #[tokio::test]
    async fn marketplace_projection_reads_yanked_release_versions_in_the_owner() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, publisher_principal TEXT NOT NULL,\
                checksum_sha256 TEXT NULL, default_locale TEXT NOT NULL,\
                published_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_release_artifacts (\
                release_id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE,\
                artifact JSON NOT NULL, descriptor JSON NOT NULL, lineage JSON NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, media_type TEXT NOT NULL\
             )",
            "INSERT INTO registry_module_releases (\
                id, slug, version, status, publisher_principal, checksum_sha256, default_locale, published_at\
             ) VALUES (\
                'release-1', 'sample_module', '1.0.0', 'yanked',\
                '{\"subject\":\"publisher\"}',\
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',\
                'en', datetime('now')\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema or fixture");
        }

        let entry = serde_json::from_value::<ModuleMarketplaceEntry>(serde_json::json!({
            "slug": "sample_module",
            "name": "Static name",
            "latest_version": "0.9.0",
            "description": "Static description retained without an active release.",
            "source": "local",
            "kind": "optional",
            "category": "tools",
            "tags": [],
            "icon_url": null,
            "banner_url": null,
            "screenshots": [],
            "crate_name": "sample-module",
            "dependencies": [],
            "ownership": "third_party",
            "trust_level": "unverified",
            "rustok_min_version": null,
            "rustok_max_version": null,
            "publisher": null,
            "checksum_sha256": null,
            "signature_present": false,
            "versions": [],
            "has_admin_ui": false,
            "has_storefront_ui": false,
            "ui_classification": "no_ui",
            "registry_lifecycle": null,
            "compatible": true,
            "recommended_admin_surfaces": [],
            "showcase_admin_surfaces": [],
            "settings_schema": {},
            "installed": false,
            "installed_version": null,
            "update_available": false
        }))
        .expect("marketplace entry");

        let projected = SeaOrmModuleGovernanceService::new(database)
            .apply_marketplace_projection(vec![entry], Some("en-US"), Some("en"))
            .await
            .expect("owner projection");
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].latest_version, "0.9.0");
        assert_eq!(projected[0].versions.len(), 1);
        assert_eq!(projected[0].versions[0].version, "1.0.0");
        assert!(projected[0].versions[0].yanked);
    }


    #[tokio::test]
    async fn published_rhai_workspace_requires_an_exact_active_contract_and_canonical_cas_bytes() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL,\
                status TEXT NOT NULL, published_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_release_artifacts (\
                release_id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE,\
                artifact JSON NOT NULL, descriptor JSON NOT NULL, lineage JSON NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, media_type TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("schema");
        }

        let workspace = rustok_sandbox::RhaiWorkspace::single_source("40 + 2");
        let workspace_bytes = workspace.canonical_bytes().expect("workspace bytes");
        let digest = workspace.digest().expect("workspace digest");
        let descriptor = ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "published_rhai".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: digest.clone(),
            entrypoint: "main".to_string(),
            capabilities: Vec::new(),
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: Vec::new(),
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        };
        descriptor.validate().expect("descriptor");
        let evidence = [
            ModuleMarketplaceEvidenceKind::AuthorSignature,
            ModuleMarketplaceEvidenceKind::Sbom,
            ModuleMarketplaceEvidenceKind::Provenance,
            ModuleMarketplaceEvidenceKind::PlatformAdmission,
            ModuleMarketplaceEvidenceKind::MarketplaceApproval,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            let digit = char::from(b'a' + index as u8);
            ModuleMarketplaceEvidenceReference {
                kind,
                reference: format!("evidence://published-rhai/{index}"),
                digest: format!("sha256:{}", digit.to_string().repeat(64)),
            }
        })
        .collect::<Vec<_>>();
        let artifact = ModuleMarketplaceArtifactRelease {
            registry_id: "local".to_string(),
            repository: "modules/published_rhai".to_string(),
            origin: ModuleMarketplaceArtifactOrigin::AlloyAuthored,
            runtime_kind: crate::ModuleMarketplaceRuntimeKind::Rhai,
            oci_manifest_digest: format!("sha256:{}", "f".repeat(64)),
            payload_digest: digest.clone(),
            descriptor_digest: crate::canonical_artifact_descriptor_digest(&descriptor),
            source_reference: "alloy://tenant/script/1".to_string(),
            source_digest: digest.clone(),
            evidence,
        };
        artifact.validate().expect("artifact contract");
        let lineage = crate::ArtifactSourceLineage {
            origin: crate::ArtifactOrigin::Marketplace,
            source_digest: digest.clone(),
            parent_release: Some(ArtifactReleaseRef {
                slug: "published_rhai".to_string(),
                version: "0.9.0".to_string(),
                digest: format!("sha256:{}", "e".repeat(64)),
            }),
        };
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_module_releases \
                 (id, slug, version, status, published_at) \
                 VALUES ('release-1', 'published_rhai', '1.0.0', 'active', datetime('now'))"
                    .to_string(),
                Vec::new(),
            ))
            .await
            .expect("release");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_module_release_artifacts \
                 (release_id, request_id, artifact, descriptor, lineage) VALUES ('release-1', 'request-1', ?, ?, ?)"
                    .to_string(),
                vec![
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&artifact).expect("artifact JSON"),
                    ))),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&lineage).expect("lineage JSON"),
                    ))),
                ],
            ))
            .await
            .expect("artifact projection");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                format!(
                    "INSERT INTO registry_publish_platform_admissions (request_id, media_type) \
                     VALUES ('request-1', '{}')",
                    rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
                ),
            ))
            .await
            .expect("admission projection");

        let blobs = InMemoryArtifactBlobStore::default();
        blobs
            .put_verified(&digest, &workspace_bytes)
            .await
            .expect("CAS payload");
        let release = ArtifactReleaseRef {
            slug: "published_rhai".to_string(),
            version: "1.0.0".to_string(),
            digest: digest.clone(),
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let source = service
            .published_rhai_workspace(&release, &blobs)
            .await
            .expect("published workspace");
        assert_eq!(source.release.descriptor.release_ref(), release);
        assert_eq!(source.release.lineage, lineage);
        assert_eq!(source.workspace, workspace);

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_platform_admissions \
                 SET media_type = 'application/json' WHERE request_id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("wrong admission media type");
        assert_eq!(
            service.published_rhai_workspace(&release, &blobs).await,
            Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)
        );

        let missing = ArtifactReleaseRef {
            version: "1.0.1".to_string(),
            ..source.release.descriptor.release_ref()
        };
        assert_eq!(
            service.published_rhai_workspace(&missing, &blobs).await,
            Err(ModuleGovernanceError::ReleaseNotFound)
        );
    }

