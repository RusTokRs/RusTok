use chrono::{DateTime, Utc};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rustok_sandbox::{CapabilityName, SandboxExecutorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPayloadKind {
    Rhai,
    WasmComponent,
    StaticPromoted,
    Sidecar,
}

impl ArtifactPayloadKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rhai => "rhai",
            Self::WasmComponent => "wasm_component",
            Self::StaticPromoted => "static_promoted",
            Self::Sidecar => "sidecar",
        }
    }

    /// Static promotion is intentionally outside the isolated executor process.
    pub fn sandbox_executor(self) -> Option<SandboxExecutorKind> {
        match self {
            Self::Rhai => Some(SandboxExecutorKind::Rhai),
            Self::WasmComponent => Some(SandboxExecutorKind::WasmComponent),
            Self::Sidecar => Some(SandboxExecutorKind::Sidecar),
            Self::StaticPromoted => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactOrigin {
    AlloyDraft,
    Marketplace,
    FirstParty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReleaseRef {
    pub slug: String,
    pub version: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSourceLineage {
    pub origin: ArtifactOrigin,
    pub source_digest: String,
    pub parent_release: Option<ArtifactReleaseRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleArtifactDescriptor {
    pub slug: String,
    pub version: String,
    pub payload_kind: ArtifactPayloadKind,
    pub runtime_abi: String,
    pub artifact_digest: String,
    pub entrypoint: String,
    #[serde(default)]
    pub capabilities: Vec<CapabilityName>,
    #[serde(default)]
    pub bindings: Vec<ModuleRuntimeBinding>,
    #[serde(default)]
    pub dependencies: Vec<ModuleDependencyConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleDependencyConstraint {
    pub slug: String,
    pub version_requirement: String,
}

/// Declarative runtime binding admitted with an immutable artifact descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRuntimeBinding {
    pub id: String,
    pub kind: ModuleRuntimeBindingKind,
    pub entrypoint: String,
    pub input_schema_digest: String,
    pub output_schema_digest: String,
    pub permission: Option<String>,
    pub idempotency: ModuleBindingIdempotency,
    pub limit_profile: String,
    #[serde(default)]
    pub capabilities: Vec<CapabilityName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleRuntimeBindingKind {
    PreEnable,
    PostEnable,
    PreDisable,
    PostDisable,
    Command,
    Http,
    Event,
    Schedule,
    Health,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBindingIdempotency {
    Required,
    BestEffort,
    None,
}

impl ModuleArtifactDescriptor {
    pub fn validate(&self) -> Result<(), ModuleArtifactError> {
        if !valid_slug(&self.slug) {
            return Err(ModuleArtifactError::InvalidSlug(self.slug.clone()));
        }
        Version::parse(&self.version)
            .map_err(|error| ModuleArtifactError::InvalidVersion(error.to_string()))?;
        if self.runtime_abi.trim().is_empty() {
            return Err(ModuleArtifactError::MissingRuntimeAbi);
        }
        if !valid_digest(&self.artifact_digest) {
            return Err(ModuleArtifactError::InvalidDigest(
                self.artifact_digest.clone(),
            ));
        }
        if self.entrypoint.trim().is_empty() {
            return Err(ModuleArtifactError::MissingEntrypoint);
        }
        for (index, capability) in self.capabilities.iter().enumerate() {
            if self.capabilities[..index]
                .iter()
                .any(|previous| previous == capability)
            {
                return Err(ModuleArtifactError::DuplicateCapability(
                    capability.as_str().to_string(),
                ));
            }
        }
        for (index, binding) in self.bindings.iter().enumerate() {
            if binding.id.trim().is_empty() || binding.entrypoint.trim().is_empty() {
                return Err(ModuleArtifactError::InvalidBinding(binding.id.clone()));
            }
            if !valid_digest(&binding.input_schema_digest)
                || !valid_digest(&binding.output_schema_digest)
            {
                return Err(ModuleArtifactError::InvalidBindingSchemaDigest(
                    binding.id.clone(),
                ));
            }
            if self.bindings[..index]
                .iter()
                .any(|previous| previous.id == binding.id)
            {
                return Err(ModuleArtifactError::DuplicateBinding(binding.id.clone()));
            }
            if binding
                .capabilities
                .iter()
                .any(|capability| !self.capabilities.contains(capability))
            {
                return Err(ModuleArtifactError::UndeclaredBindingCapability(
                    binding.id.clone(),
                ));
            }
        }
        for (index, dependency) in self.dependencies.iter().enumerate() {
            if !valid_slug(&dependency.slug) || dependency.slug == self.slug {
                return Err(ModuleArtifactError::InvalidDependency(dependency.slug.clone()));
            }
            VersionReq::parse(&dependency.version_requirement).map_err(|_| {
                ModuleArtifactError::InvalidDependencyVersionRequirement {
                    slug: dependency.slug.clone(),
                    requirement: dependency.version_requirement.clone(),
                }
            })?;
            if self.dependencies[..index]
                .iter()
                .any(|previous| previous.slug == dependency.slug)
            {
                return Err(ModuleArtifactError::DuplicateDependency(dependency.slug.clone()));
            }
        }
        Ok(())
    }

    pub fn release_ref(&self) -> ArtifactReleaseRef {
        ArtifactReleaseRef {
            slug: self.slug.clone(),
            version: self.version.clone(),
            digest: self.artifact_digest.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRelease {
    pub descriptor: ModuleArtifactDescriptor,
    pub lineage: ArtifactSourceLineage,
    pub published_at: DateTime<Utc>,
}

impl ArtifactRelease {
    /// Creates the next immutable release in this artifact's source lineage.
    ///
    /// A revision cannot silently change ownership to another module slug or
    /// overwrite the published version it was derived from.
    pub fn fork(
        &self,
        descriptor: ModuleArtifactDescriptor,
        source_digest: String,
    ) -> Result<ArtifactReleaseDraft, ModuleArtifactError> {
        descriptor.validate()?;
        if descriptor.slug != self.descriptor.slug {
            return Err(ModuleArtifactError::ForkSlugMismatch {
                expected: self.descriptor.slug.clone(),
                received: descriptor.slug,
            });
        }

        let parent_version = Version::parse(&self.descriptor.version)
            .expect("published artifact version must have been validated");
        let next_version =
            Version::parse(&descriptor.version).expect("validated artifact version must parse");
        if next_version <= parent_version {
            return Err(ModuleArtifactError::ForkVersionNotIncremented {
                parent: self.descriptor.version.clone(),
                received: descriptor.version,
            });
        }

        Ok(ArtifactReleaseDraft {
            descriptor,
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::AlloyDraft,
                source_digest,
                parent_release: Some(self.descriptor.release_ref()),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactReleaseDraft {
    pub descriptor: ModuleArtifactDescriptor,
    pub lineage: ArtifactSourceLineage,
}

impl ArtifactReleaseDraft {
    pub fn publish(
        self,
        published_at: DateTime<Utc>,
    ) -> Result<ArtifactRelease, ModuleArtifactError> {
        self.descriptor.validate()?;
        if !valid_digest(&self.lineage.source_digest) {
            return Err(ModuleArtifactError::InvalidSourceDigest(
                self.lineage.source_digest,
            ));
        }
        Ok(ArtifactRelease {
            descriptor: self.descriptor,
            lineage: self.lineage,
            published_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModuleArtifactError {
    #[error("artifact slug `{0}` must be a short snake_case identifier")]
    InvalidSlug(String),
    #[error("artifact version is not valid semantic versioning: {0}")]
    InvalidVersion(String),
    #[error("artifact runtime ABI must be declared")]
    MissingRuntimeAbi,
    #[error("artifact digest `{0}` must be a sha256 digest")]
    InvalidDigest(String),
    #[error("artifact source digest `{0}` must be a sha256 digest")]
    InvalidSourceDigest(String),
    #[error("artifact entrypoint must be declared")]
    MissingEntrypoint,
    #[error("artifact capability `{0}` is declared more than once")]
    DuplicateCapability(String),
    #[error("artifact binding `{0}` is invalid")]
    InvalidBinding(String),
    #[error("artifact binding `{0}` must declare sha256 input/output schemas")]
    InvalidBindingSchemaDigest(String),
    #[error("artifact binding `{0}` is declared more than once")]
    DuplicateBinding(String),
    #[error("artifact binding `{0}` declares a capability absent from the descriptor")]
    UndeclaredBindingCapability(String),
    #[error("artifact dependency `{0}` is invalid")]
    InvalidDependency(String),
    #[error("artifact dependency `{slug}` has invalid semantic-version requirement `{requirement}")]
    InvalidDependencyVersionRequirement { slug: String, requirement: String },
    #[error("artifact dependency `{0}` is declared more than once")]
    DuplicateDependency(String),
    #[error("forked artifact slug must remain `{expected}`, received `{received}`")]
    ForkSlugMismatch { expected: String, received: String },
    #[error("forked artifact version must be newer than `{parent}`, received `{received}`")]
    ForkVersionNotIncremented { parent: String, received: String },
}

fn valid_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && !value.starts_with('_')
        && !value.ends_with('_')
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(character: char) -> String {
        format!("sha256:{}", character.to_string().repeat(64))
    }

    fn descriptor(
        kind: ArtifactPayloadKind,
        version: &str,
        marker: char,
    ) -> ModuleArtifactDescriptor {
        ModuleArtifactDescriptor {
            slug: "sample_module".to_string(),
            version: version.to_string(),
            payload_kind: kind,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            artifact_digest: digest(marker),
            entrypoint: "main".to_string(),
            capabilities: vec![CapabilityName::new("platform.events").expect("capability")],
            bindings: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn sandboxed_payloads_select_the_common_executor_registry() {
        assert_eq!(
            descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a')
                .payload_kind
                .sandbox_executor(),
            Some(SandboxExecutorKind::Rhai)
        );
        assert_eq!(
            descriptor(ArtifactPayloadKind::WasmComponent, "1.0.0", 'b')
                .payload_kind
                .sandbox_executor(),
            Some(SandboxExecutorKind::WasmComponent)
        );
        assert_eq!(
            descriptor(ArtifactPayloadKind::StaticPromoted, "1.0.0", 'c')
                .payload_kind
                .sandbox_executor(),
            None
        );
    }

    #[test]
    fn alloy_fork_creates_a_new_immutable_release_lineage() {
        let original = ArtifactReleaseDraft {
            descriptor: descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a'),
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::AlloyDraft,
                source_digest: digest('1'),
                parent_release: None,
            },
        }
        .publish(Utc::now())
        .expect("publish original");

        let fork = original
            .fork(
                descriptor(ArtifactPayloadKind::Rhai, "1.1.0", 'b'),
                digest('2'),
            )
            .expect("fork")
            .publish(Utc::now())
            .expect("publish fork");

        assert_eq!(
            fork.lineage.parent_release,
            Some(original.descriptor.release_ref())
        );
        assert_eq!(original.descriptor.version, "1.0.0");
        assert_eq!(fork.descriptor.version, "1.1.0");
    }

    #[test]
    fn duplicate_capabilities_are_rejected_before_publish() {
        let capability = CapabilityName::new("platform.events").expect("capability");
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.capabilities.push(capability);

        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::DuplicateCapability(_))
        ));
    }

    #[test]
    fn binding_cannot_expand_descriptor_capabilities() {
        let mut descriptor = descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a');
        descriptor.bindings.push(ModuleRuntimeBinding {
            id: "pre_enable".to_string(),
            kind: ModuleRuntimeBindingKind::PreEnable,
            entrypoint: "pre_enable".to_string(),
            input_schema_digest: digest('b'),
            output_schema_digest: digest('c'),
            permission: None,
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "lifecycle".to_string(),
            capabilities: vec![CapabilityName::new("platform.http").expect("capability")],
        });

        assert!(matches!(
            descriptor.validate(),
            Err(ModuleArtifactError::UndeclaredBindingCapability(_))
        ));
    }

    #[test]
    fn fork_must_keep_slug_and_increment_version() {
        let original = ArtifactReleaseDraft {
            descriptor: descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'a'),
            lineage: ArtifactSourceLineage {
                origin: ArtifactOrigin::AlloyDraft,
                source_digest: digest('1'),
                parent_release: None,
            },
        }
        .publish(Utc::now())
        .expect("publish original");

        let mut renamed = descriptor(ArtifactPayloadKind::Rhai, "1.1.0", 'b');
        renamed.slug = "another_module".to_string();
        assert!(matches!(
            original.fork(renamed, digest('2')),
            Err(ModuleArtifactError::ForkSlugMismatch { .. })
        ));

        assert!(matches!(
            original.fork(
                descriptor(ArtifactPayloadKind::Rhai, "1.0.0", 'c'),
                digest('3')
            ),
            Err(ModuleArtifactError::ForkVersionNotIncremented { .. })
        ));
    }
}
