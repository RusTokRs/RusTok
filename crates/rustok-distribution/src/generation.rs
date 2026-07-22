//! Deterministic build-time output for reviewed static promotions.

use rustok_modules::{ModuleStaticDistributionExecutorMode, ModuleStaticDistributionWorkItem};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const GENERATED_DISTRIBUTION_MANIFEST_PATH: &str = ".rustok/generated/static-distribution.json";
pub const GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH: &str =
    "crates/rustok-distribution/Cargo.toml";
pub const GENERATED_DISTRIBUTION_REGISTRY_PATH: &str =
    "crates/rustok-distribution/src/generated_promotions.rs";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedStaticDistributionSource {
    pub ordinal: u16,
    pub promotion_id: Uuid,
    pub promotion_revision: u64,
    pub release_id: String,
    pub module_slug: String,
    pub module_version: String,
    pub cargo_package: String,
    pub entry_type: String,
    pub dependency_alias: String,
    pub source_reference: String,
    pub source_digest: String,
    pub dependency_lock_digest: String,
    pub executor_mode: ModuleStaticDistributionExecutorMode,
    pub materialization_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedStaticDistributionManifest {
    pub distribution_build_id: Uuid,
    pub claim_id: Uuid,
    pub attempt_number: u32,
    pub runner_id: String,
    pub composition_revision: u64,
    pub composition_digest: String,
    pub platform_source_reference: String,
    pub platform_source_digest: String,
    pub toolchain_digest: String,
    pub build_target: String,
    pub manifest_path: String,
    pub cargo_manifest_path: String,
    pub registry_source_path: String,
    pub sources: Vec<GeneratedStaticDistributionSource>,
    pub output_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedStaticDistributionFiles {
    pub manifest: GeneratedStaticDistributionManifest,
    pub manifest_json: Vec<u8>,
    pub cargo_dependencies_toml: String,
    pub registry_source: String,
}

#[derive(Debug, Error)]
pub enum StaticDistributionGenerationError {
    #[error("static distribution work item is invalid: {0}")]
    InvalidWorkItem(String),
    #[error("static distribution generated manifest could not be serialized: {0}")]
    Serialization(String),
}

#[derive(Serialize)]
struct GeneratedOutputDigestInput<'a> {
    distribution_build_id: Uuid,
    claim_id: Uuid,
    attempt_number: u32,
    runner_id: &'a str,
    composition_revision: u64,
    composition_digest: &'a str,
    platform_source_reference: &'a str,
    platform_source_digest: &'a str,
    toolchain_digest: &'a str,
    build_target: &'a str,
    sources: &'a [GeneratedStaticDistributionSource],
    cargo_dependencies_toml: &'a str,
    registry_source: &'a str,
    manifest_path: &'static str,
    cargo_manifest_path: &'static str,
    registry_source_path: &'static str,
}

/// Produces the only generated Cargo dependency fragment, registry source, and
/// machine-readable composition manifest for one owner-validated work item.
/// The caller applies these files only inside the materialized CI workspace.
pub fn generate_static_distribution(
    work_item: &ModuleStaticDistributionWorkItem,
) -> Result<GeneratedStaticDistributionFiles, StaticDistributionGenerationError> {
    work_item
        .validate()
        .map_err(|error| StaticDistributionGenerationError::InvalidWorkItem(error.to_string()))?;
    let build = &work_item.build;
    let runner_id = build.claimed_by.clone().ok_or_else(|| {
        StaticDistributionGenerationError::InvalidWorkItem("runner is missing".into())
    })?;
    let mut sources = Vec::with_capacity(build.items.len());
    for (ordinal, item) in build.items.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).map_err(|_| {
            StaticDistributionGenerationError::InvalidWorkItem(
                "distribution selection exceeds the generated-output bound".into(),
            )
        })?;
        sources.push(GeneratedStaticDistributionSource {
            ordinal,
            promotion_id: item.promotion_id,
            promotion_revision: item.promotion_revision,
            release_id: item.release_id.clone(),
            module_slug: item.module_slug.clone(),
            module_version: item.module_version.clone(),
            cargo_package: item.cargo_package.clone(),
            entry_type: item.entry_type.clone(),
            dependency_alias: dependency_alias(usize::from(ordinal)),
            source_reference: item.source_reference.clone(),
            source_digest: item.source_digest.clone(),
            dependency_lock_digest: item.dependency_lock_digest.clone(),
            executor_mode: item.executor_mode,
            materialization_path: materialization_path(usize::from(ordinal)),
        });
    }
    let cargo_dependencies_toml = cargo_dependencies(&sources);
    let registry_source = registry_source(&sources);
    let digest_input = GeneratedOutputDigestInput {
        distribution_build_id: build.distribution_build_id,
        claim_id: work_item.claim_id,
        attempt_number: work_item.attempt_number,
        runner_id: &runner_id,
        composition_revision: build.composition_revision,
        composition_digest: &build.composition_digest,
        platform_source_reference: &build.platform_source_reference,
        platform_source_digest: &build.platform_source_digest,
        toolchain_digest: &build.toolchain_digest,
        build_target: &build.build_target,
        sources: &sources,
        cargo_dependencies_toml: &cargo_dependencies_toml,
        registry_source: &registry_source,
        manifest_path: GENERATED_DISTRIBUTION_MANIFEST_PATH,
        cargo_manifest_path: GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH,
        registry_source_path: GENERATED_DISTRIBUTION_REGISTRY_PATH,
    };
    let digest_value = serde_json::to_value(&digest_input)
        .map_err(|error| StaticDistributionGenerationError::Serialization(error.to_string()))?;
    let output_digest = format!(
        "sha256:{}",
        rustok_api::manifest_hash::hash_manifest_snapshot(&digest_value)
    );
    let manifest = GeneratedStaticDistributionManifest {
        distribution_build_id: build.distribution_build_id,
        claim_id: work_item.claim_id,
        attempt_number: work_item.attempt_number,
        runner_id,
        composition_revision: build.composition_revision,
        composition_digest: build.composition_digest.clone(),
        platform_source_reference: build.platform_source_reference.clone(),
        platform_source_digest: build.platform_source_digest.clone(),
        toolchain_digest: build.toolchain_digest.clone(),
        build_target: build.build_target.clone(),
        manifest_path: GENERATED_DISTRIBUTION_MANIFEST_PATH.to_string(),
        cargo_manifest_path: GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH.to_string(),
        registry_source_path: GENERATED_DISTRIBUTION_REGISTRY_PATH.to_string(),
        sources,
        output_digest,
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| StaticDistributionGenerationError::Serialization(error.to_string()))?;
    Ok(GeneratedStaticDistributionFiles {
        manifest,
        manifest_json,
        cargo_dependencies_toml,
        registry_source,
    })
}

fn dependency_alias(ordinal: usize) -> String {
    format!("rustok_static_promotion_{ordinal:03}")
}

fn materialization_path(ordinal: usize) -> String {
    format!(".rustok/static-sources/{ordinal:03}")
}

fn cargo_dependencies(sources: &[GeneratedStaticDistributionSource]) -> String {
    let mut output = String::from("# Generated static-promotion dependencies.\n");
    for source in sources {
        output.push_str(&format!(
            "{} = {{ package = \"{}\", path = \"../../{}\" }}\n",
            source.dependency_alias, source.cargo_package, source.materialization_path,
        ));
    }
    output
}

fn registry_source(sources: &[GeneratedStaticDistributionSource]) -> String {
    let mut output = String::from(
        "// Generated from an immutable static-distribution work item.\n\
use rustok_core::ModuleRegistry;\n\n\
pub(crate) fn register_promoted_modules(mut registry: ModuleRegistry) -> ModuleRegistry {\n",
    );
    for source in sources {
        output.push_str(&format!(
            "    registry = registry.register({}::{});\n",
            source.dependency_alias, source.entry_type,
        ));
    }
    output.push_str("    registry\n}\n");
    output
}
