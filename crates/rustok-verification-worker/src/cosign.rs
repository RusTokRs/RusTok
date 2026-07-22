use std::time::Duration;

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use rustok_modules::{TrustVerificationDecision, TrustVerificationRequest, TrustVerifier};
use serde_json::Value;
use tokio::process::Command;

use crate::{VerificationPolicy, VerificationTrustRoot, policy::vulnerability_severity_rank};

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
        let output = tokio::time::timeout(
            self.timeout,
            Command::new(&self.program).args(arguments).output(),
        )
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
    ) -> Result<(), String> {
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
        self.run(signature).await?;

        let mut attestations = Vec::new();
        for predicate in ["slsaprovenance", "cyclonedx"] {
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
        let expected_digest = expected_sha256(request)?;
        validate_slsa(&attestations[0], expected_digest, &self.policy)?;
        validate_cyclonedx(&attestations[1], expected_digest, &self.policy)
    }

    async fn verify_trust_root(
        &self,
        request: &TrustVerificationRequest,
        trust_root: &VerificationTrustRoot,
    ) -> Result<String, String> {
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
                        if self
                            .verify_with_flags(request, trust_root, flags)
                            .await
                            .is_ok()
                        {
                            return Ok(identity.clone());
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
                self.verify_with_flags(
                    request,
                    trust_root,
                    vec!["--key".to_string(), key_reference.clone()],
                )
                .await?;
                Ok(signer_identity.clone())
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

fn expected_sha256(request: &TrustVerificationRequest) -> Result<&str, String> {
    request
        .descriptor
        .artifact_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| "artifact descriptor digest must be sha256".to_string())
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

fn validate_slsa(
    output: &[u8],
    expected_digest: &str,
    policy: &VerificationPolicy,
) -> Result<(), String> {
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
        });
    accepted.then_some(()).ok_or_else(|| {
        "SLSA provenance does not satisfy subject, builder, build-type, or source policy"
            .to_string()
    })
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
        let mut signer_identity = None;
        for trust_root in self
            .policy
            .trust_roots_at(VerificationPolicy::current_unix_seconds())
        {
            if let Ok(identity) = self.verify_trust_root(&request, trust_root).await {
                signer_identity = Some(identity);
                break;
            }
        }
        let signer_identity = signer_identity.ok_or_else(|| {
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
            evidence_references: vec![
                format!("{}#cosign-signature", request.reference.canonical()),
                format!("{}#slsa-provenance", request.reference.canonical()),
                format!("{}#cyclonedx-sbom", request.reference.canonical()),
            ],
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

    use super::{attestation_statements, validate_cyclonedx, validate_slsa};
    use crate::{VerificationPolicy, VerificationTrustRoot, VerificationTrustRoots};

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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
        assert!(validate_slsa(&output, DIGEST, &policy()).is_ok());
        assert!(
            validate_slsa(
                &output,
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                &policy()
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
                validate_slsa(&cosign_output_value(&substituted), DIGEST, &policy()).is_err(),
                "substituted SLSA field {pointer} must fail closed"
            );
        }
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
