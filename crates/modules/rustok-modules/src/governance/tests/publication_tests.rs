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


    #[tokio::test]
    async fn publication_persists_release_binding_request_and_audit_facts_together() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, revision INTEGER NOT NULL DEFAULT 1, slug TEXT NOT NULL, version TEXT NOT NULL, crate_name TEXT NOT NULL,\
                default_locale TEXT NOT NULL, ownership TEXT NOT NULL, trust_level TEXT NOT NULL,\
                license TEXT NOT NULL, entry_type TEXT NULL, artifact_origin TEXT NOT NULL, marketplace TEXT NOT NULL, ui_packages TEXT NOT NULL,\
                status TEXT NOT NULL, artifact_storage_key TEXT NULL, artifact_checksum_sha256 TEXT NULL,\
                artifact_size INTEGER NULL, approved_by_principal TEXT NULL, approved_at TEXT NULL,\
                submitted_at TEXT NULL, published_at TEXT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_request_translations (\
                request_id TEXT NOT NULL, locale TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,\
                PRIMARY KEY (request_id, locale)\
             )",
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NULL, slug TEXT NOT NULL, version TEXT NOT NULL,\
                crate_name TEXT NOT NULL, default_locale TEXT NOT NULL, ownership TEXT NOT NULL,\
                trust_level TEXT NOT NULL, license TEXT NOT NULL, entry_type TEXT NULL, artifact_origin TEXT NOT NULL, marketplace TEXT NOT NULL,\
                ui_packages TEXT NOT NULL, status TEXT NOT NULL, publisher_principal TEXT NOT NULL,\
                artifact_storage_key TEXT NULL, checksum_sha256 TEXT NULL, artifact_size INTEGER NULL,\
                yanked_reason TEXT NULL, yanked_by_principal TEXT NULL, yanked_at TEXT NULL,\
                published_at TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,\
                UNIQUE (slug, version)\
             )",
            "CREATE TABLE registry_module_release_translations (\
                release_id TEXT NOT NULL, locale TEXT NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL,\
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY (release_id, locale)\
             )",
            "CREATE TABLE registry_module_release_artifacts (\
                release_id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE,\
                artifact JSON NOT NULL, descriptor JSON NOT NULL, lineage JSON NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publication_operations (\
                operation_id TEXT PRIMARY KEY, request_id TEXT NOT NULL, idempotency_key TEXT NOT NULL,\
                actor_id TEXT NOT NULL, trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL,\
                actor_principal TEXT NOT NULL, publisher_principal TEXT NOT NULL, allow_owner_rebind INTEGER NOT NULL,\
                approval_override TEXT NULL, release_id TEXT NOT NULL, committed_at TEXT NOT NULL,\
                UNIQUE (request_id, idempotency_key)\
             )",
            "CREATE TABLE registry_module_owners (\
                slug TEXT PRIMARY KEY, owner_principal TEXT NOT NULL, bound_by_principal TEXT NOT NULL,\
                bound_at TEXT NOT NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL,\
                event_type TEXT NOT NULL, actor_principal TEXT NOT NULL, publisher_principal TEXT NULL,\
                details TEXT NOT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publication_evidence (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, authority TEXT NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL, evidence_reference TEXT NOT NULL,\
                issuer_identity TEXT NOT NULL, policy_revision TEXT NOT NULL,\
                signature_digest_sha256 TEXT NULL,\
                evidence_digest_sha256 TEXT NOT NULL, recorded_by_principal TEXT NOT NULL,\
                created_at TEXT NOT NULL, UNIQUE (request_id, evidence_digest_sha256)\
             )",
            "CREATE TABLE registry_publish_build_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, source_reference TEXT NULL,\
                source_digest TEXT NOT NULL, parent_release_slug TEXT NULL,\
                parent_release_version TEXT NULL, parent_release_digest TEXT NULL, component_digest TEXT NOT NULL,\
                artifact_manifest_digest TEXT NOT NULL,\
                staged_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, registry_id TEXT NOT NULL, registry TEXT NOT NULL,\
                repository TEXT NOT NULL, manifest_digest TEXT NOT NULL, payload_digest TEXT NOT NULL,\
                descriptor_digest TEXT NOT NULL, descriptor JSON NOT NULL, runtime_kind TEXT NOT NULL,\
                media_type TEXT NOT NULL, signature_reference TEXT NOT NULL, signature_digest TEXT NOT NULL,\
                provenance_reference TEXT NOT NULL, provenance_digest TEXT NOT NULL,\
                sbom_reference TEXT NOT NULL, sbom_digest TEXT NOT NULL,\
                admission_reference TEXT NOT NULL, admission_digest TEXT NOT NULL,\
                recorded_at TEXT NOT NULL\
             )",
            "INSERT INTO registry_publish_requests (\
                id, slug, version, crate_name, default_locale, ownership, trust_level, license, entry_type,\
                artifact_origin, marketplace, ui_packages, status, artifact_storage_key, artifact_checksum_sha256, artifact_size, submitted_at, updated_at\
             ) VALUES (\
                'request-1', 'sample_module', '1.0.0', 'sample_crate', 'en', 'platform', 'verified', 'MIT',\
                NULL, 'platform_built', '{}', '[]', 'approved', 'registry/request-1', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 42, datetime('now'), datetime('now')\
             )",
            "INSERT INTO registry_publish_request_translations (request_id, locale, name, description) VALUES (\
                'request-1', 'en', 'Sample module', 'Sample module description'\
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
            publisher_principal: serde_json::json!({ "kind": "user", "id": "publisher" }),
            allow_owner_rebind: false,
            approval_override: None,
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication without a current platform build stage must fail");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingPlatformBuildStage
        ));
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_build_staging \
                 (id, request_id, source_reference, source_digest, component_digest, \
                  artifact_manifest_digest, staged_at) VALUES (\
                 'stage-1', 'request-1', \
                 'https://source.example/sample-module.tar.gz', \
                 'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', \
                 'sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee', \
                 'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                 datetime('now'))"
                    .to_string(),
            ))
            .await
            .expect("platform build stage fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication without author evidence must fail");
        assert!(
            matches!(
                blocked,
                ModuleGovernanceError::PublishRequestMissingAuthorSignature
            ),
            "{blocked:?}"
        );

        let staged_subject = "a".repeat(64);
        let oci_subject = "b".repeat(64);
        let evidence_fixture = |id: &str, authority: &str, subject: &str, created_at: &str| {
            Statement::from_sql_and_values(
                DbBackend::Sqlite,
                format!(
                    "INSERT INTO registry_publication_evidence \
                 (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                  issuer_identity, policy_revision, signature_digest_sha256, evidence_digest_sha256, \
                  recorded_by_principal, created_at) \
                 VALUES (?, 'request-1', ?, ?, 'evidence://fixture', 'fixture', \
                         'fixture-v1', ?, ?, '{{}}', {created_at})"
                ),
                vec![
                    id.to_string().into(),
                    authority.to_string().into(),
                    subject.to_string().into(),
                    (authority == "author_signature")
                        .then(|| "f".repeat(64))
                        .into(),
                    hex::encode(Sha256::digest(id.as_bytes())).into(),
                ],
            )
        };
        database
            .execute_raw(evidence_fixture(
                "rpe_author",
                "author_signature",
                staged_subject.as_str(),
                "datetime('now')",
            ))
            .await
            .expect("author evidence fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication without matching build and platform evidence must fail");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission
        ));
        for (id, authority) in [
            ("rpe_build", "build_service_attestation"),
            ("rpe_platform", "platform_admission"),
        ] {
            database
                .execute_raw(evidence_fixture(
                    id,
                    authority,
                    oci_subject.as_str(),
                    "datetime('now')",
                ))
                .await
                .expect("publication evidence fixture");
        }

        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_requests \
                 SET submitted_at = datetime('now', '+1 second') WHERE id = 'request-1'"
                    .to_string(),
            ))
            .await
            .expect("reupload stage fixture");
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_build_staging \
                 (id, request_id, source_reference, source_digest, component_digest, \
                  artifact_manifest_digest, staged_at) VALUES (\
                 'stage-2', 'request-1', \
                 'https://source.example/sample-module.tar.gz', \
                 'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc', \
                 'sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee', \
                 'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', \
                 datetime('now', '+2 seconds'))"
                    .to_string(),
            ))
            .await
            .expect("current platform build stage fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("stale publication evidence must not survive a reupload");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingAuthorSignature
        ));
        database
            .execute_raw(evidence_fixture(
                "rpe_author_current",
                "author_signature",
                staged_subject.as_str(),
                "datetime('now', '+2 seconds')",
            ))
            .await
            .expect("current author evidence fixture");
        for (id, authority) in [
            ("rpe_build_current", "build_service_attestation"),
            ("rpe_platform_current", "platform_admission"),
        ] {
            database
                .execute_raw(evidence_fixture(
                    id,
                    authority,
                    oci_subject.as_str(),
                    "datetime('now', '+2 seconds')",
                ))
                .await
                .expect("current publication evidence fixture");
        }
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_build_staging \
                 SET artifact_manifest_digest = \
                 'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc' \
                 WHERE id = 'stage-2'"
                    .to_string(),
            ))
            .await
            .expect("mismatched build manifest fixture");
        let blocked = service
            .publish_request(command.clone())
            .await
            .expect_err("publication evidence must match the staged OCI manifest");
        assert!(matches!(
            blocked,
            ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission
        ));
        database
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE registry_publish_build_staging \
                 SET artifact_manifest_digest = \
                 'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' \
                 WHERE id = 'stage-2'"
                    .to_string(),
            ))
            .await
            .expect("matching build manifest fixture");

        let descriptor = crate::ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: crate::ArtifactPayloadKind::WasmComponent,
            module_kind: crate::ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", "e".repeat(64)),
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
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_platform_admissions \
                 (request_id, registry_id, registry, repository, manifest_digest, payload_digest, \
                  descriptor_digest, descriptor, runtime_kind, media_type, signature_reference, \
                  signature_digest, provenance_reference, provenance_digest, sbom_reference, \
                  sbom_digest, admission_reference, admission_digest, recorded_at) \
                 VALUES ('request-1', 'local', 'registry.example', 'modules/sample', ?, ?, ?, ?, \
                         'wasm_component', 'application/vnd.rustok.wasm.component.v1+wasm', 'evidence://signature', ?, \
                         'evidence://provenance', ?, 'evidence://sbom', ?, \
                         'evidence://platform-admission', ?, datetime('now'))"
                    .to_string(),
                vec![
                    format!("sha256:{}", "b".repeat(64)).into(),
                    format!("sha256:{}", "e".repeat(64)).into(),
                    crate::canonical_artifact_descriptor_digest(&descriptor).into(),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                    format!("sha256:{}", "d".repeat(64)).into(),
                    format!("sha256:{}", "e".repeat(64)).into(),
                    format!("sha256:{}", "f".repeat(64)).into(),
                    "1".repeat(64).into(),
                ],
            ))
            .await
            .expect("platform admission contract fixture");

        service
            .publish_request(command.clone())
            .await
            .expect("publish request");
        service
            .publish_request(command.clone())
            .await
            .expect("published request replay");
        let mut conflicting_replay = command;
        conflicting_replay.context.trace_id = "test:publication:changed-trace".to_string();
        let conflict = service
            .publish_request(conflicting_replay)
            .await
            .expect_err("idempotency key must bind immutable publication context");
        assert!(matches!(
            conflict,
            ModuleGovernanceError::PublicationIdempotencyConflict
        ));

        let release_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_module_releases".to_string(),
            ))
            .await
            .expect("release count query")
            .expect("release count row");
        assert_eq!(
            release_count
                .try_get::<i64>("", "count")
                .expect("release count"),
            1
        );
        let artifact_contract_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_module_release_artifacts".to_string(),
            ))
            .await
            .expect("artifact contract count query")
            .expect("artifact contract count row");
        assert_eq!(
            artifact_contract_count
                .try_get::<i64>("", "count")
                .expect("artifact contract count"),
            1
        );
        let publication_operation_count = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM registry_publication_operations".to_string(),
            ))
            .await
            .expect("publication operation count query")
            .expect("publication operation count row");
        assert_eq!(
            publication_operation_count
                .try_get::<i64>("", "count")
                .expect("publication operation count"),
            1
        );

        let request = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status FROM registry_publish_requests WHERE id = 'request-1'".to_string(),
            ))
            .await
            .expect("request query")
            .expect("request row");
        assert_eq!(
            request
                .try_get::<String>("", "status")
                .expect("request status"),
            "published"
        );
        let release = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT id, status, checksum_sha256 FROM registry_module_releases".to_string(),
            ))
            .await
            .expect("release query")
            .expect("release row");
        assert_eq!(
            release
                .try_get::<String>("", "status")
                .expect("release status"),
            "active"
        );
        assert_eq!(
            release
                .try_get::<String>("", "checksum_sha256")
                .expect("release checksum"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        let release_id: String = release.try_get("", "id").expect("release id");
        let translations = database
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT locale, name FROM registry_module_release_translations".to_string(),
            ))
            .await
            .expect("release translations");
        assert_eq!(translations.len(), 1);
        assert_eq!(
            translations[0]
                .try_get::<String>("", "locale")
                .expect("translation locale"),
            "en"
        );
        let owner = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT owner_principal FROM registry_module_owners WHERE slug = 'sample_module'"
                    .to_string(),
            ))
            .await
            .expect("owner query")
            .expect("owner row");
        let owner: serde_json::Value = serde_json::from_str(
            &owner
                .try_get::<String>("", "owner_principal")
                .expect("owner principal"),
        )
        .expect("owner JSON");
        assert_eq!(owner["id"], "publisher");
        let events = database
            .query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT event_type, release_id FROM registry_governance_events ORDER BY created_at, id"
                    .to_string(),
            ))
            .await
            .expect("events query");
        assert_eq!(events.len(), 3);
        let event_types = events
            .iter()
            .map(|event| {
                event
                    .try_get::<String>("", "event_type")
                    .expect("event type")
            })
            .collect::<Vec<_>>();
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "owner_bound")
        );
        let publication = events
            .iter()
            .find(|event| {
                event
                    .try_get::<String>("", "event_type")
                    .expect("event type")
                    == "release_published"
            })
            .expect("publication event");
        assert_eq!(
            publication
                .try_get::<String>("", "release_id")
                .expect("publication release ID"),
            release_id
        );
        assert!(
            event_types
                .iter()
                .any(|event_type| event_type == "marketplace_approval_recorded")
        );
        let evidence = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT authority, subject_digest_sha256 FROM registry_publication_evidence \
                 WHERE authority = 'marketplace_approval'"
                    .to_string(),
            ))
            .await
            .expect("marketplace approval evidence query")
            .expect("marketplace approval evidence row");
        assert_eq!(
            evidence
                .try_get::<String>("", "authority")
                .expect("marketplace approval authority"),
            "marketplace_approval"
        );
        assert_eq!(
            evidence
                .try_get::<String>("", "subject_digest_sha256")
                .expect("marketplace approval subject"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }

