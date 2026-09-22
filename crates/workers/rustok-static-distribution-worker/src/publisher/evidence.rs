use super::digest_bytes;
use crate::{StaticDistributionPublisherError, StaticDistributionPublisherRequest};

pub fn build_cyclonedx_sbom(
    request: &StaticDistributionPublisherRequest,
    lock_bytes: &[u8],
    maximum_bytes: u64,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let lock_text = std::str::from_utf8(lock_bytes).map_err(|_| {
        StaticDistributionPublisherError::InvalidInput(
            "resolved Cargo.lock is not UTF-8".to_string(),
        )
    })?;
    let lock = lock_text.parse::<toml::Table>().map_err(|error| {
        StaticDistributionPublisherError::InvalidInput(format!(
            "resolved Cargo.lock is invalid: {error}"
        ))
    })?;
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| {
            StaticDistributionPublisherError::InvalidInput(
                "resolved Cargo.lock has no packages".to_string(),
            )
        })?;
    let mut components = Vec::with_capacity(packages.len());
    for package in packages {
        components.push(parse_cargo_package(package)?);
    }
    components.sort_by(|left, right| left["bom-ref"].as_str().cmp(&right["bom-ref"].as_str()));
    let document = format_cyclonedx_document(request, components);
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))?;
    if bytes.len() as u64 > maximum_bytes {
        return Err(StaticDistributionPublisherError::InvalidInput(
            "generated SBOM exceeds the evidence bound".to_string(),
        ));
    }
    Ok(bytes)
}

fn parse_cargo_package(
    package: &toml::Value,
) -> Result<serde_json::Value, StaticDistributionPublisherError> {
    let package = package.as_table().ok_or_else(|| {
        StaticDistributionPublisherError::InvalidInput(
            "resolved Cargo.lock package is invalid".to_string(),
        )
    })?;
    let (name, version, source) = parse_package_identity(package)?;
    let bom_ref = digest_bytes(format!("{name}\0{version}\0{source}").as_bytes());
    let mut component = serde_json::json!({
        "type": "library",
        "bom-ref": bom_ref,
        "name": name,
        "version": version,
        "properties": [{ "name": "rustok:cargo:source", "value": source }]
    });
    if let Some(checksum) = package.get("checksum").and_then(toml::Value::as_str)
        && checksum.len() == 64
        && checksum
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        component["hashes"] = serde_json::json!([{ "alg": "SHA-256", "content": checksum }]);
    }
    Ok(component)
}

fn parse_package_identity(
    package: &toml::Table,
) -> Result<(&str, &str, &str), StaticDistributionPublisherError> {
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            StaticDistributionPublisherError::InvalidInput(
                "resolved Cargo.lock package name is missing".to_string(),
            )
        })?;
    let version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            StaticDistributionPublisherError::InvalidInput(
                "resolved Cargo.lock package version is missing".to_string(),
            )
        })?;
    let source = package
        .get("source")
        .and_then(toml::Value::as_str)
        .unwrap_or("workspace");
    Ok((name, version, source))
}

fn format_cyclonedx_document(
    request: &StaticDistributionPublisherRequest,
    components: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "serialNumber": format!("urn:uuid:{}", request.distribution_build_id),
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": request.composition_digest,
                "name": "rustok-static-distribution",
                "version": request.distribution_build_id.to_string()
            },
            "properties": [
                { "name": "rustok:composition_digest", "value": request.composition_digest },
                { "name": "rustok:resolved_lock_digest", "value": request.resolved_lock_digest }
            ]
        },
        "components": components
    })
}

pub fn build_slsa_provenance(
    request: &StaticDistributionPublisherRequest,
    artifact: &rustok_modules::OciArtifactReference,
    publisher_request_digest: &str,
) -> Result<Vec<u8>, StaticDistributionPublisherError> {
    let subject_digest = artifact.digest.strip_prefix("sha256:").ok_or_else(|| {
        StaticDistributionPublisherError::InvalidInput(
            "published artifact digest is invalid".to_string(),
        )
    })?;
    let document =
        format_slsa_predicate(request, artifact, subject_digest, publisher_request_digest);
    serde_json::to_vec_pretty(&document)
        .map_err(|error| StaticDistributionPublisherError::Io(error.to_string()))
}

fn format_slsa_predicate(
    request: &StaticDistributionPublisherRequest,
    artifact: &rustok_modules::OciArtifactReference,
    subject_digest: &str,
    publisher_request_digest: &str,
) -> serde_json::Value {
    serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": artifact.canonical(),
            "digest": { "sha256": subject_digest }
        }],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://rustok.dev/build-types/static-distribution",
                "externalParameters": {
                    "distribution_build_id": request.distribution_build_id,
                    "composition_digest": request.composition_digest,
                    "generated_output_digest": request.generated_output_digest
                },
                "internalParameters": {
                    "job_request_digest": request.job_request_digest,
                    "publisher_request_digest": publisher_request_digest,
                    "toolchain_digest": request.toolchain_digest,
                    "build_target": request.build_target,
                    "resolved_lock_digest": request.resolved_lock_digest
                },
                "resolvedDependencies": []
            },
            "runDetails": {
                "builder": { "id": "https://rustok.dev/builders/static-distribution" },
                "metadata": { "invocationId": request.claim_id }
            }
        }
    })
}
