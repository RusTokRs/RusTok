use std::time::Duration;

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use rustok_modules::{
    TrustAlloyWorkspaceProvenance, TrustEvidenceKind, TrustEvidenceReference,
    TrustVerificationDecision, TrustVerificationRequest, TrustVerifier,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::{VerificationPolicy, VerificationTrustRoot, policy::vulnerability_severity_rank};

const REQUIRED_ATTESTATION_TYPES: [&str; 2] = [
    "https://slsa.dev/provenance/v1",
    "https://cyclonedx.org/bom",
];

/// Concrete worker-only Cosign adapter. Values are passed as process arguments,
/// never through a shell; trust-root flags derive exclusively from mounted
/// policy.
pub struct CosignTrustVerifier {
    program: String,
    policy: VerificationPolicy,
    timeout: Duration,
}

impl CosignTrustVerifier {
    pub fn new(policy: VerificationPolicy) -> Self {
        Self {
            program: std::env::var("RUSTOK_COSIGN_PROGRAM").unwrap_or_else(|_| "cosign".into()),
            policy,
            timeout: Duration::from_secs(30),
        }
    }

    async fn run(&self, arguments: Vec<String>) -> Result<Vec<u8>, String> {
        let mut command = Command::new(&self.program);
        command.args(arguments).kill_on_drop(true);
        let output = tokio::time::timeout(self.timeout, command.output())
            .await
            .map_err(|_| "cosign verification timed out".to_string())?
            .map_err(|error| format!("could not start cosign: {error}"))?;
        if !output.status.success() {
            return Err("cosign verification rejected the artifact".to_string());
        }
        Ok(output.stdout)
    }

    async fn verify_with_flags(
        &self,
        request: &TrustVerificationRequest,
        trust_root: &VerificationTrustRoot,
        mut flags: Vec<String>,
    ) -> Result<[Vec<u8>; 3], String> {
        let reference = request.reference.canonical();
        if requires_transparency_bundle(trust_root) {
            flags.push("--offline".to_string());
        }

        let mut signature = vec![
            "verify".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];
        signature.extend(flags.clone());
        signature.push(reference.clone());
        let signature = self.run(signature).await?;

        let mut attestations = Vec::new();
        for predicate in REQUIRED_ATTESTATION_TYPES {
            let mut command = vec![
                "verify-attestation".to_string(),
                "--type".to_string(),
                predicate.to_string(),
                "--output".to_string(),
                "json".to_string(),
            ];
            command.extend(flags.clone());
            command.push(reference.clone());
            attestations.push(self.run(command).await?);
        }
        let expected_digest = expected_manifest_sha256(&request.reference)?;
        validate_slsa_with_alloy(
            &attestations[0],
            expected_digest,
            &self.policy,
            request.expected_alloy_workspace_provenance.as_ref(),
        )?;
        validate_cyclonedx(&attestations[1], expected_digest, &self.policy)?;
        let [provenance, sbom] = attestations
            .try_into()
            .map_err(|_| "cosign verification returned an invalid evidence set".to_string())?;
        Ok([signature, provenance, sbom])
    }

    async fn verify_trust_root(
        &self,
        request: &TrustVerificationRequest,
        trust_root: &VerificationTrustRoot,
    ) -> Result<(String, [Vec<u8>; 3]), String> {
        match trust_root {
            VerificationTrustRoot::KeylessSigstore {
                allowed_signer_identities,
                allowed_oidc_issuers,
                ..
            } => {
                for identity in allowed_signer_identities {
                    for issuer in allowed_oidc_issuers {
                        let flags = vec![
                            "--certificate-identity".to_string(),
                            identity.clone(),
                            "--certificate-oidc-issuer".to_string(),
                            issuer.clone(),
                        ];
                        if let Ok(evidence) =
                            self.verify_with_flags(request, trust_root, flags).await
                        {
                            return Ok((identity.clone(), evidence));
                        }
                    }
                }
                Err(
                    "no configured Cosign signer identity and OIDC issuer verified the artifact"
                        .to_string(),
                )
            }
            VerificationTrustRoot::KmsKey {
                key_reference,
                signer_identity,
                ..
            } => {
                let evidence = self
                    .verify_with_flags(
                        request,
                        trust_root,
                        vec!["--key".to_string(), key_reference.clone()],
                    )
                    .await?;
                Ok((signer_identity.clone(), evidence))
            }
        }
    }
}

fn attestation_statements(output: &[u8]) -> Result<Vec<Value>, String> {
    let records = serde_json::Deserializer::from_slice(output)
        .into_iter::<Value>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "cosign attestation output is not JSON".to_string())?;
    let records = records.into_iter().flat_map(|record| match record {
        Value::Array(records) => records,
        record => vec![record],
    });
    let mut statements = Vec::new();
    for record in records {
        let payload = record
            .get("payload")
            .or_else(|| record.get("Payload"))
            .and_then(Value::as_str)
            .ok_or_else(|| "cosign attestation payload is missing".to_string())?;
        let bytes = STANDARD
            .decode(payload)
            .map_err(|_| "cosign attestation payload is not base64".to_string())?;
        let statement = serde_json::from_slice(&bytes)
            .map_err(|_| "in-toto statement is not JSON".to_string())?;
        statements.push(statement);
    }
    if statements.is_empty() {
        return Err("cosign returned no verified attestations".to_string());
    }
    Ok(statements)
}

/// Cosign's `attest` command creates the in-toto statement itself and binds its
/// subject to the resolved OCI manifest. The descriptor payload digest is
/// separately re-fetched and revalidated by the owner before this verifier is
/// invoked; it must not be substituted for the attestation subject.
fn expected_manifest_sha256(
    reference: &rustok_modules::OciArtifactReference,
) -> Result<&str, String> {
    reference
        .digest
        .strip_prefix("sha256:")
        .filter(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| "OCI manifest digest must be sha256".to_string())
}

fn verified_evidence_reference(
    subject: &str,
    kind: TrustEvidenceKind,
    fragment: &str,
    bytes: &[u8],
) -> TrustEvidenceReference {
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
    TrustEvidenceReference {
        kind,
        reference: format!("oci://{subject}#{fragment}?verified-evidence={digest}"),
        digest,
    }
}

fn subject_matches(statement: &Value, expected_digest: &str) -> bool {
    statement
        .get("subject")
        .and_then(Value::as_array)
        .is_some_and(|subjects| {
            subjects.iter().any(|subject| {
                subject.pointer("/digest/sha256").and_then(Value::as_str) == Some(expected_digest)
            })
        })
}

fn allowed(values: &[String], actual: &str) -> bool {
    values.iter().any(|value| value == actual)
}

fn validate_slsa_with_alloy(
    output: &[u8],
    expected_digest: &str,
    policy: &VerificationPolicy,
    expected_alloy_workspace_provenance: Option<&TrustAlloyWorkspaceProvenance>,
) -> Result<(), String> {
    if expected_alloy_workspace_provenance.is_some_and(|binding| !binding.validate()) {
        return Err("Alloy workspace provenance binding is invalid".to_string());
    }
    let accepted = attestation_statements(output)?
        .into_iter()
        .any(|statement| {
            let builder = statement
                .pointer("/predicate/runDetails/builder/id")
                .and_then(Value::as_str);
            let build_type = statement
                .pointer("/predicate/buildDefinition/buildType")
                .and_then(Value::as_str);
            let source = statement
                .pointer("/predicate/buildDefinition/externalParameters/source/uri")
                .and_then(Value::as_str);
            let source_ref = statement
                .pointer("/predicate/buildDefinition/externalParameters/source/ref")
                .and_then(Value::as_str);
            statement.get("predicateType").and_then(Value::as_str)
                == Some("https://slsa.dev/provenance/v1")
                && subject_matches(&statement, expected_digest)
                && builder.is_some_and(|value| allowed(&policy.allowed_builders, value))
                && build_type.is_some_and(|value| allowed(&policy.allowed_build_types, value))
                && source.is_some_and(|value| allowed(&policy.allowed_source_repositories, value))
                && source_ref.is_some_and(|value| allowed(&policy.allowed_source_refs, value))
                && expected_alloy_workspace_provenance
                    .is_none_or(|binding| alloy_workspace_provenance_matches(&statement, binding))
        });
    accepted.then_some(()).ok_or_else(|| {
        "SLSA provenance does not satisfy subject, builder, build-type, or source policy"
            .to_string()
    })
}

fn alloy_workspace_provenance_matches(
    statement: &Value,
    expected: &TrustAlloyWorkspaceProvenance,
) -> bool {
    let Some(rustok) = statement
        .pointer("/predicate/buildDefinition/externalParameters/rustok")
        .and_then(Value::as_object)
    else {
        return false;
    };
    let tenant_id = expected.alloy_tenant_id.to_string();
    let script_id = expected.alloy_script_id.to_string();
    rustok.get("artifactOrigin").and_then(Value::as_str) == Some("alloy_authored")
        && rustok.get("requestId").and_then(Value::as_str) == Some(expected.request_id.as_str())
        && rustok.get("alloyTenantId").and_then(Value::as_str) == Some(tenant_id.as_str())
        && rustok.get("alloyScriptId").and_then(Value::as_str) == Some(script_id.as_str())
        && rustok.get("sourceRevision").and_then(Value::as_u64)
            == Some(u64::from(expected.source_revision))
        && rustok.get("sourceDigest").and_then(Value::as_str)
            == Some(expected.source_digest.as_str())
        && rustok.get("reviewDigest").and_then(Value::as_str)
            == Some(expected.review_digest.as_str())
        && rustok.get("descriptorDigest").and_then(Value::as_str)
            == Some(expected.descriptor_digest.as_str())
        && rustok.get("workspaceEntrypoint").and_then(Value::as_str)
            == Some(expected.workspace_entrypoint.as_str())
}

fn component_licenses_are_allowed(bom: &Value, policy: &VerificationPolicy) -> bool {
    let mut has_component = false;
    bom.get("metadata")
        .and_then(|metadata| metadata.get("component"))
        .into_iter()
        .chain(
            bom.get("components")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .all(|component| {
            has_component = true;
            component
                .get("licenses")
                .and_then(Value::as_array)
                .is_some_and(|licenses| {
                    !licenses.is_empty()
                        && licenses.iter().all(|entry| {
                            entry
                                .get("license")
                                .and_then(|license| {
                                    license.get("id").or_else(|| license.get("name"))
                                })
                                .and_then(Value::as_str)
                                .is_some_and(|license| allowed(&policy.allowed_licenses, license))
                        })
                })
        })
        && has_component
}

fn vulnerabilities_are_within_policy(bom: &Value, maximum: &str) -> bool {
    let Some(maximum) = vulnerability_severity_rank(maximum) else {
        return false;
    };
    bom.get("vulnerabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .all(|vulnerability| {
            let ratings = vulnerability.get("ratings").and_then(Value::as_array);
            ratings.is_some_and(|ratings| {
                !ratings.is_empty()
                    && ratings.iter().all(|rating| {
                        rating
                            .get("severity")
                            .and_then(Value::as_str)
                            .and_then(vulnerability_severity_rank)
                            .is_some_and(|severity| severity <= maximum)
                    })
            })
        })
}

fn validate_cyclonedx(
    output: &[u8],
    expected_digest: &str,
    policy: &VerificationPolicy,
) -> Result<(), String> {
    let accepted = attestation_statements(output)?
        .into_iter()
        .any(|statement| {
            let bom = statement
                .pointer("/predicate/bom")
                .unwrap_or_else(|| statement.get("predicate").unwrap_or(&Value::Null));
            statement.get("predicateType").and_then(Value::as_str)
                == Some("https://cyclonedx.org/bom")
                && subject_matches(&statement, expected_digest)
                && bom.get("bomFormat").and_then(Value::as_str) == Some("CycloneDX")
                && bom
                    .get("specVersion")
                    .and_then(Value::as_str)
                    .is_some_and(|version| {
                        allowed(&policy.allowed_cyclonedx_spec_versions, version)
                    })
                && component_licenses_are_allowed(bom, policy)
                && vulnerabilities_are_within_policy(bom, &policy.maximum_vulnerability_severity)
        });
    accepted.then_some(()).ok_or_else(|| {
        "CycloneDX SBOM does not satisfy subject, schema, license, or vulnerability policy"
            .to_string()
    })
}

#[async_trait]
impl TrustVerifier for CosignTrustVerifier {
    async fn verify(
        &self,
        request: TrustVerificationRequest,
    ) -> Result<TrustVerificationDecision, String> {
        self.policy.validate()?;
        let mut verified = None;
        for trust_root in self
            .policy
            .trust_roots_at(VerificationPolicy::current_unix_seconds())
        {
            if let Ok(result) = self.verify_trust_root(&request, trust_root).await {
                verified = Some(result);
                break;
            }
        }
        let (signer_identity, verified_evidence) = verified.ok_or_else(|| {
            "no active or unexpired retiring Cosign trust root verified the artifact".to_string()
        })?;
        Ok(TrustVerificationDecision {
            signer_identity,
            trust_policy_revision: request.trust_policy_revision,
            capability_policy_revision: request.capability_policy_revision,
            signature_verified: true,
            provenance_verified: true,
            sbom_verified: true,
            license_policy_verified: true,
            vulnerability_policy_verified: true,
            evidence: [
                (
                    TrustEvidenceKind::Signature,
                    "cosign-signature",
                    &verified_evidence[0],
                ),
                (
                    TrustEvidenceKind::Provenance,
                    "slsa-provenance",
                    &verified_evidence[1],
                ),
                (
                    TrustEvidenceKind::Sbom,
                    "cyclonedx-sbom",
                    &verified_evidence[2],
                ),
            ]
            .into_iter()
            .map(|(kind, fragment, bytes)| {
                verified_evidence_reference(&request.reference.canonical(), kind, fragment, bytes)
            })
            .collect(),
        })
    }
}

fn requires_transparency_bundle(trust_root: &VerificationTrustRoot) -> bool {
    match trust_root {
        VerificationTrustRoot::KeylessSigstore {
            require_transparency_bundle,
            ..
        }
        | VerificationTrustRoot::KmsKey {
            require_transparency_bundle,
            ..
        } => *require_transparency_bundle,
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde_json::json;

    use super::{
        REQUIRED_ATTESTATION_TYPES, attestation_statements, expected_manifest_sha256,
        validate_cyclonedx, validate_slsa_with_alloy, verified_evidence_reference,
    };
    use crate::{VerificationPolicy, VerificationTrustRoot, VerificationTrustRoots};
    use rustok_modules::{
        ALLOY_WORKSPACE_PUBLICATION_BUILD_TYPE, ALLOY_WORKSPACE_PUBLICATION_BUILDER_ID,
        ALLOY_WORKSPACE_PUBLICATION_SOURCE_REF, ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI,
        OciArtifactReference, TrustAlloyWorkspaceProvenance, TrustEvidenceKind,
    };
    use uuid::Uuid;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn verifier_requires_current_slsa_and_cyclonedx_predicate_types() {
        assert_eq!(
            REQUIRED_ATTESTATION_TYPES,
            [
                "https://slsa.dev/provenance/v1",
                "https://cyclonedx.org/bom",
            ]
        );
    }

    #[test]
    fn attestation_subject_uses_the_digest_pinned_oci_manifest() {
        let reference = OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/example".to_string(),
            digest: format!("sha256:{DIGEST}"),
        };

        assert_eq!(
            expected_manifest_sha256(&reference).expect("valid manifest subject"),
            DIGEST
        );
    }

    #[test]
    fn verified_outputs_have_distinct_typed_evidence_identities() {
        let subject = format!("registry.example/module@sha256:{DIGEST}");
        let signature = verified_evidence_reference(
            &subject,
            TrustEvidenceKind::Signature,
            "cosign-signature",
            b"signature",
        );
        let provenance = verified_evidence_reference(
            &subject,
            TrustEvidenceKind::Provenance,
            "slsa-provenance",
            b"provenance",
        );
        let sbom = verified_evidence_reference(
            &subject,
            TrustEvidenceKind::Sbom,
            "cyclonedx-sbom",
            b"sbom",
        );

        assert_ne!(signature.digest, provenance.digest);
        assert_ne!(signature.digest, sbom.digest);
        assert_ne!(provenance.digest, sbom.digest);
        assert!(signature.validate());
        assert!(provenance.validate());
        assert!(sbom.validate());
    }

    fn policy() -> VerificationPolicy {
        VerificationPolicy {
            trust_policy_revision: 1,
            capability_policy_revision: 1,
            trust_root: VerificationTrustRoots {
                active: VerificationTrustRoot::KeylessSigstore {
                    allowed_signer_identities: vec!["builder@rustok.dev".into()],
                    allowed_oidc_issuers: vec!["https://issuer.rustok.dev".into()],
                    require_transparency_bundle: true,
                },
                retiring: None,
            },
            allowed_builders: vec!["https://build.rustok.dev/worker".into()],
            allowed_source_repositories: vec!["https://github.com/rustok/module".into()],
            allowed_source_refs: vec!["refs/heads/main".into()],
            allowed_build_types: vec!["https://rustok.dev/build/wasm/v1".into()],
            allowed_licenses: vec!["MIT".into(), "Apache-2.0".into()],
            allowed_cyclonedx_spec_versions: vec!["1.6".into()],
            maximum_vulnerability_severity: "medium".into(),
        }
    }

    fn cosign_output(statement: &str) -> Vec<u8> {
        serde_json::to_vec(&json!([{ "payload": STANDARD.encode(statement) }]))
            .expect("fixture output")
    }

    fn statement(source: &str) -> serde_json::Value {
        serde_json::from_str(source).expect("statement fixture")
    }

    fn cosign_output_value(statement: &serde_json::Value) -> Vec<u8> {
        cosign_output(&serde_json::to_string(statement).expect("statement JSON"))
    }

    #[test]
    fn slsa_fixture_requires_exact_subject_digest() {
        let output = cosign_output(include_str!("../fixtures/slsa-statement.json"));
        assert!(validate_slsa_with_alloy(&output, DIGEST, &policy(), None).is_ok());
        assert!(
            validate_slsa_with_alloy(
                &output,
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                &policy(),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn slsa_fixture_requires_exact_builder_build_type_source_and_ref() {
        let fixture = statement(include_str!("../fixtures/slsa-statement.json"));
        for (pointer, replacement) in [
            (
                "/predicate/runDetails/builder/id",
                "https://attacker.invalid/worker",
            ),
            (
                "/predicate/buildDefinition/buildType",
                "https://attacker.invalid/build",
            ),
            (
                "/predicate/buildDefinition/externalParameters/source/uri",
                "https://attacker.invalid/module",
            ),
            (
                "/predicate/buildDefinition/externalParameters/source/ref",
                "refs/heads/unreviewed",
            ),
        ] {
            let mut substituted = fixture.clone();
            *substituted
                .pointer_mut(pointer)
                .expect("fixture policy field") = json!(replacement);
            assert!(
                validate_slsa_with_alloy(
                    &cosign_output_value(&substituted),
                    DIGEST,
                    &policy(),
                    None,
                )
                .is_err(),
                "substituted SLSA field {pointer} must fail closed"
            );
        }
    }

    #[test]
    fn alloy_workspace_slsa_requires_the_exact_owner_receipt_binding() {
        let tenant_id =
            Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("tenant UUID");
        let script_id =
            Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("script UUID");
        let binding = TrustAlloyWorkspaceProvenance {
            request_id: "request-1".to_string(),
            alloy_tenant_id: tenant_id,
            alloy_script_id: script_id,
            source_revision: 7,
            source_digest: format!("sha256:{DIGEST}"),
            review_digest: format!("sha256:{}", "b".repeat(64)),
            descriptor_digest: format!("sha256:{}", "c".repeat(64)),
            workspace_entrypoint: "src/main.rhai".to_string(),
        };
        let mut alloy_policy = policy();
        alloy_policy.allowed_builders = vec![ALLOY_WORKSPACE_PUBLICATION_BUILDER_ID.to_string()];
        alloy_policy.allowed_build_types = vec![ALLOY_WORKSPACE_PUBLICATION_BUILD_TYPE.to_string()];
        alloy_policy.allowed_source_repositories =
            vec![ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI.to_string()];
        alloy_policy.allowed_source_refs = vec![ALLOY_WORKSPACE_PUBLICATION_SOURCE_REF.to_string()];
        let mut statement = statement(include_str!("../fixtures/slsa-statement.json"));
        *statement
            .pointer_mut("/predicate/runDetails/builder/id")
            .expect("builder") = json!(ALLOY_WORKSPACE_PUBLICATION_BUILDER_ID);
        *statement
            .pointer_mut("/predicate/buildDefinition/buildType")
            .expect("build type") = json!(ALLOY_WORKSPACE_PUBLICATION_BUILD_TYPE);
        *statement
            .pointer_mut("/predicate/buildDefinition/externalParameters/source/uri")
            .expect("source URI") = json!(ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI);
        *statement
            .pointer_mut("/predicate/buildDefinition/externalParameters/source/ref")
            .expect("source ref") = json!(ALLOY_WORKSPACE_PUBLICATION_SOURCE_REF);
        statement["predicate"]["buildDefinition"]["externalParameters"]["rustok"] = json!({
            "artifactOrigin": "alloy_authored",
            "requestId": binding.request_id.as_str(),
            "alloyTenantId": binding.alloy_tenant_id,
            "alloyScriptId": binding.alloy_script_id,
            "sourceRevision": binding.source_revision,
            "sourceDigest": binding.source_digest.as_str(),
            "reviewDigest": binding.review_digest.as_str(),
            "descriptorDigest": binding.descriptor_digest.as_str(),
            "workspaceEntrypoint": binding.workspace_entrypoint.as_str(),
        });
        let output = cosign_output_value(&statement);
        assert!(validate_slsa_with_alloy(&output, DIGEST, &alloy_policy, Some(&binding)).is_ok());

        statement["predicate"]["buildDefinition"]["externalParameters"]["rustok"]["sourceRevision"] =
            json!(8);
        assert!(
            validate_slsa_with_alloy(
                &cosign_output_value(&statement),
                DIGEST,
                &alloy_policy,
                Some(&binding),
            )
            .is_err()
        );
    }

    #[test]
    fn cyclonedx_fixture_enforces_license_and_vulnerability_policy() {
        let output = cosign_output(include_str!("../fixtures/cyclonedx-statement.json"));
        assert!(validate_cyclonedx(&output, DIGEST, &policy()).is_ok());

        let mut denied_license = policy();
        denied_license.allowed_licenses = vec!["MIT".into()];
        assert!(validate_cyclonedx(&output, DIGEST, &denied_license).is_err());

        let mut denied_severity = policy();
        denied_severity.maximum_vulnerability_severity = "low".into();
        assert!(validate_cyclonedx(&output, DIGEST, &denied_severity).is_err());
    }

    #[test]
    fn cyclonedx_fixture_requires_subject_schema_component_licenses_and_ratings() {
        let fixture = statement(include_str!("../fixtures/cyclonedx-statement.json"));
        let cases = [
            "/subject/0/digest/sha256",
            "/predicate/specVersion",
            "/predicate/metadata/component/licenses",
            "/predicate/components/0/licenses",
            "/predicate/vulnerabilities/0/ratings",
        ];
        for pointer in cases {
            let mut invalid = fixture.clone();
            *invalid.pointer_mut(pointer).expect("fixture policy field") = match pointer {
                "/subject/0/digest/sha256" => json!("b".repeat(64)),
                "/predicate/specVersion" => json!("0.1"),
                _ => json!([]),
            };
            assert!(
                validate_cyclonedx(&cosign_output_value(&invalid), DIGEST, &policy()).is_err(),
                "invalid CycloneDX field {pointer} must fail closed"
            );
        }
    }

    #[test]
    fn cyclonedx_fixture_rejects_unknown_or_over_policy_severity() {
        let fixture = statement(include_str!("../fixtures/cyclonedx-statement.json"));
        for severity in ["unknown", "high", "critical"] {
            let mut invalid = fixture.clone();
            *invalid
                .pointer_mut("/predicate/vulnerabilities/0/ratings/0/severity")
                .expect("fixture severity") = json!(severity);
            assert!(
                validate_cyclonedx(&cosign_output_value(&invalid), DIGEST, &policy()).is_err(),
                "severity {severity} must fail medium policy"
            );
        }
    }

    #[test]
    fn cosign_envelope_rejects_missing_malformed_or_empty_attestations() {
        assert!(attestation_statements(br#"[{"payload":"%%%"}]"#).is_err());
        assert!(attestation_statements(br#"[{"not_payload":"value"}]"#).is_err());
        assert!(attestation_statements(b"[]").is_err());
    }
}
