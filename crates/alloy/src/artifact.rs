use std::collections::BTreeSet;

use thiserror::Error;

use rustok_modules::{
    ArtifactAdmissionLimits, ArtifactOrigin, ArtifactPayloadKind, ArtifactRelease,
    ArtifactReleaseDraft, ArtifactSourceLineage, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
    ModuleArtifactDescriptor, ModuleArtifactError, ModuleArtifactPackage, OciArtifactReference,
};
use rustok_sandbox::CapabilityName;

use crate::{RhaiWorkspace, RhaiWorkspaceFileKind, Script};

#[derive(Debug, Error)]
pub enum AlloyArtifactError {
    #[error(transparent)]
    Module(#[from] ModuleArtifactError),
    #[error("Alloy source cannot be released as module `{slug}`: {message}")]
    InvalidRelease { slug: String, message: String },
    #[error("release `{slug}@{version}` is not a Rhai module artifact")]
    NotRhaiRelease { slug: String, version: String },
    #[error("Rhai module package is invalid: {0}")]
    InvalidPackage(String),
    #[error(
        "Rhai source uses a dynamic capability name in `{path}`; publication requires a literal declared name"
    )]
    DynamicCapabilityCall { path: String },
    #[error("Rhai source redefines reserved capability helper `{helper}` in `{path}")]
    ReservedCapabilityHelper { path: String, helper: String },
    #[error(
        "Rhai capability declarations do not match source tool use; missing declarations: {missing:?}; unused declarations: {unused:?}"
    )]
    CapabilityDeclarationMismatch {
        missing: Vec<String>,
        unused: Vec<String>,
    },
    #[error("Rhai source contains an invalid literal capability in `{path}")]
    InvalidLiteralCapability { path: String },
}

/// Returns the exact capability names reachable from executable Rhai source
/// files through the neutral sandbox helper surface. Publication accepts only
/// literal generic capability names: a dynamically chosen capability cannot be
/// proven against an immutable descriptor and is therefore rejected.
pub fn observed_rhai_capabilities(
    workspace: &RhaiWorkspace,
) -> Result<Vec<CapabilityName>, AlloyArtifactError> {
    workspace
        .validate()
        .map_err(|error| AlloyArtifactError::InvalidPackage(error.to_string()))?;

    let mut observed = BTreeSet::new();
    for file in workspace
        .files
        .iter()
        .filter(|file| file.kind == RhaiWorkspaceFileKind::Source)
    {
        observe_rhai_source_capabilities(&file.path, &file.contents, &mut observed)?;
    }

    observed
        .into_iter()
        .map(|name| {
            CapabilityName::new(name).map_err(|_| AlloyArtifactError::InvalidLiteralCapability {
                path: workspace.entrypoint.clone(),
            })
        })
        .collect()
}

/// Requires the descriptor capability set to be exactly the set of capability
/// helpers observed in the immutable Rhai source. This prevents both an
/// undeclared tool call and a broader unused declaration from reaching module
/// admission, where tenant policy could otherwise grant it later.
pub fn validate_rhai_capabilities(
    workspace: &RhaiWorkspace,
    declared_capabilities: &[CapabilityName],
) -> Result<(), AlloyArtifactError> {
    let observed = observed_rhai_capabilities(workspace)?
        .into_iter()
        .map(|capability| capability.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let declared = declared_capabilities
        .iter()
        .map(|capability| capability.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let missing = observed.difference(&declared).cloned().collect::<Vec<_>>();
    let unused = declared.difference(&observed).cloned().collect::<Vec<_>>();
    if missing.is_empty() && unused.is_empty() {
        Ok(())
    } else {
        Err(AlloyArtifactError::CapabilityDeclarationMismatch { missing, unused })
    }
}

/// Stages an immutable Rhai module artifact from an Alloy source revision.
///
/// Capability grants are supplied by the review/policy layer rather than inferred
/// from the script's application permissions. A source-backed Rhai artifact uses
/// its canonical source digest as the payload digest until OCI packaging adds the
/// equivalent source layer to a manifest.
pub fn stage_rhai_module_release(
    module_slug: impl Into<String>,
    version: impl Into<String>,
    script: &Script,
    capabilities: Vec<CapabilityName>,
) -> Result<ArtifactReleaseDraft, AlloyArtifactError> {
    let module_slug = module_slug.into();
    validate_rhai_capabilities(&script.workspace, &capabilities)?;
    let source_digest =
        script
            .workspace
            .digest()
            .map_err(|error| AlloyArtifactError::InvalidRelease {
                slug: module_slug.clone(),
                message: error.to_string(),
            })?;
    let descriptor = ModuleArtifactDescriptor {
        schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: module_slug.clone(),
        version: version.into(),
        payload_kind: ArtifactPayloadKind::Rhai,
        module_kind: rustok_modules::ArtifactModuleKind::Optional,
        runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        artifact_digest: source_digest.clone(),
        entrypoint: script.workspace.entrypoint.clone(),
        capabilities,
        bindings: Vec::new(),
        dependencies: Vec::new(),
        permissions: Vec::new(),
        schema_documents: Vec::new(),
        settings_schema_digest: None,
        data_schema_digest: None,
        ui_contributions: Vec::new(),
        persistence_contract: None,
    };
    descriptor
        .validate()
        .map_err(|error| AlloyArtifactError::InvalidRelease {
            slug: module_slug,
            message: error.to_string(),
        })?;

    Ok(ArtifactReleaseDraft {
        descriptor,
        lineage: ArtifactSourceLineage {
            origin: ArtifactOrigin::AlloyDraft,
            source_digest,
            parent_release: None,
        },
    })
}

/// Starts the next immutable Rhai release from a marketplace release lineage.
pub fn fork_rhai_module_release(
    parent: &ArtifactRelease,
    version: impl Into<String>,
    script: &Script,
    capabilities: Vec<CapabilityName>,
) -> Result<ArtifactReleaseDraft, AlloyArtifactError> {
    if parent.descriptor.payload_kind != ArtifactPayloadKind::Rhai {
        return Err(AlloyArtifactError::NotRhaiRelease {
            slug: parent.descriptor.slug.clone(),
            version: parent.descriptor.version.clone(),
        });
    }

    let draft = stage_rhai_module_release(
        parent.descriptor.slug.clone(),
        version,
        script,
        capabilities,
    )?;
    parent
        .fork(draft.descriptor, draft.lineage.source_digest)
        .map_err(AlloyArtifactError::from)
}

/// Packages reviewed Alloy source as the immutable OCI payload selected by an
/// already-staged module release. The caller supplies a digest-pinned OCI
/// manifest location; the descriptor separately pins the source payload layer.
pub async fn package_rhai_module_release(
    reference: OciArtifactReference,
    draft: &ArtifactReleaseDraft,
    script: &Script,
) -> Result<ModuleArtifactPackage, AlloyArtifactError> {
    if draft.descriptor.payload_kind != ArtifactPayloadKind::Rhai {
        return Err(AlloyArtifactError::InvalidRelease {
            slug: draft.descriptor.slug.clone(),
            message: "only Rhai release drafts can be packaged from Alloy source".to_string(),
        });
    }
    validate_rhai_capabilities(&script.workspace, &draft.descriptor.capabilities)?;
    let package = ModuleArtifactPackage {
        reference,
        descriptor: draft.descriptor.clone(),
        media_type: rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
        payload: rustok_modules::ArtifactPayloadSource::Bytes(
            script
                .workspace
                .canonical_bytes()
                .map_err(|error| AlloyArtifactError::InvalidPackage(error.to_string()))?,
        ),
    };
    package
        .verify(ArtifactAdmissionLimits::default())
        .await
        .map_err(|error| AlloyArtifactError::InvalidPackage(error.to_string()))?;
    Ok(package)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RhaiToken {
    Identifier(String),
    StringLiteral { value: String, escaped: bool },
    Symbol(char),
}

fn observe_rhai_source_capabilities(
    path: &str,
    source: &str,
    observed: &mut BTreeSet<String>,
) -> Result<(), AlloyArtifactError> {
    let tokens = tokenize_rhai(source);
    for (index, token) in tokens.iter().enumerate() {
        let RhaiToken::Identifier(name) = token else {
            continue;
        };
        if name == "fn" {
            if let Some(RhaiToken::Identifier(helper)) = tokens.get(index + 1) {
                if is_capability_helper(helper) {
                    return Err(AlloyArtifactError::ReservedCapabilityHelper {
                        path: path.to_string(),
                        helper: helper.clone(),
                    });
                }
            }
        }
        if !matches!(tokens.get(index + 1), Some(RhaiToken::Symbol('('))) {
            continue;
        }
        match name.as_str() {
            "http_get" | "http_post" | "http_request" => {
                observed.insert("platform.http".to_string());
            }
            "capability_call" => {
                let Some(RhaiToken::StringLiteral { value, escaped }) = tokens.get(index + 2)
                else {
                    return Err(AlloyArtifactError::DynamicCapabilityCall {
                        path: path.to_string(),
                    });
                };
                if *escaped || CapabilityName::new(value.clone()).is_err() {
                    return Err(AlloyArtifactError::InvalidLiteralCapability {
                        path: path.to_string(),
                    });
                }
                observed.insert(value.clone());
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_capability_helper(name: &str) -> bool {
    matches!(
        name,
        "capability_call" | "http_get" | "http_post" | "http_request"
    )
}

fn tokenize_rhai(source: &str) -> Vec<RhaiToken> {
    let characters = source.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() {
            index += 1;
        } else if character == '/' && characters.get(index + 1) == Some(&'/') {
            index += 2;
            while index < characters.len() && characters[index] != '\n' {
                index += 1;
            }
        } else if character == '/' && characters.get(index + 1) == Some(&'*') {
            index += 2;
            let mut depth = 1_u32;
            while index < characters.len() && depth > 0 {
                if characters[index] == '/' && characters.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if characters[index] == '*' && characters.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        } else if character == '"' {
            index += 1;
            let mut value = String::new();
            let mut escaped = false;
            while index < characters.len() {
                match characters[index] {
                    '"' => {
                        index += 1;
                        break;
                    }
                    '\\' => {
                        escaped = true;
                        index += 1;
                        if index < characters.len() {
                            value.push(characters[index]);
                            index += 1;
                        }
                    }
                    value_character => {
                        value.push(value_character);
                        index += 1;
                    }
                }
            }
            tokens.push(RhaiToken::StringLiteral { value, escaped });
        } else if character.is_ascii_alphabetic() || character == '_' {
            let start = index;
            index += 1;
            while index < characters.len()
                && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            tokens.push(RhaiToken::Identifier(
                characters[start..index].iter().collect(),
            ));
        } else {
            tokens.push(RhaiToken::Symbol(character));
            index += 1;
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rustok_modules::ArtifactPayloadKind;
    use rustok_sandbox::CapabilityName;

    use super::{
        AlloyArtifactError, fork_rhai_module_release, observed_rhai_capabilities,
        package_rhai_module_release, stage_rhai_module_release, validate_rhai_capabilities,
    };
    use crate::{RhaiWorkspace, Script, ScriptTrigger};

    fn script(code: &str) -> Script {
        Script::new(
            "tax_adjustment",
            RhaiWorkspace::single_source(code),
            ScriptTrigger::Manual,
        )
    }

    #[test]
    fn reviewed_rhai_source_stages_as_a_module_artifact() {
        let draft = stage_rhai_module_release(
            "tax_adjustment",
            "1.0.0",
            &script("capability_call(\"platform.events\", \"emit\", #{})"),
            vec![CapabilityName::new("platform.events").expect("capability")],
        )
        .expect("stage release");

        assert_eq!(draft.descriptor.payload_kind, ArtifactPayloadKind::Rhai);
        assert_eq!(
            draft.descriptor.artifact_digest,
            draft.lineage.source_digest
        );
        assert_eq!(draft.descriptor.runtime_abi, "rustok:module/runtime@1");
    }

    #[test]
    fn editing_a_marketplace_rhai_release_creates_new_lineage() {
        let original = stage_rhai_module_release(
            "tax_adjustment",
            "1.0.0",
            &script("input.total * 0.2"),
            Vec::new(),
        )
        .expect("stage original")
        .publish(Utc::now())
        .expect("publish original");

        let revision = fork_rhai_module_release(
            &original,
            "1.1.0",
            &script("input.total * 0.21"),
            Vec::new(),
        )
        .expect("fork release")
        .publish(Utc::now())
        .expect("publish revision");

        assert_eq!(
            revision.lineage.parent_release,
            Some(original.descriptor.release_ref())
        );
        assert_ne!(
            revision.descriptor.artifact_digest,
            original.descriptor.artifact_digest
        );
    }

    #[test]
    fn release_capability_declarations_match_literal_source_tool_use() {
        let source = script(
            r#"
                let documentation = "http_get is only documentation";
                // capability_call("platform.secrets", "resolve", #{});
                /* http_post("https://example.test/ignored", #{}); */
                http_get("https://example.test/health");
                capability_call("platform.events", "emit", #{});
            "#,
        );
        let observed = observed_rhai_capabilities(&source.workspace)
            .expect("observe source capabilities")
            .into_iter()
            .map(|capability| capability.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(observed, vec!["platform.events", "platform.http"]);

        let exact = vec![
            CapabilityName::new("platform.events").expect("event capability"),
            CapabilityName::new("platform.http").expect("HTTP capability"),
        ];
        assert!(validate_rhai_capabilities(&source.workspace, &exact).is_ok());

        let error = validate_rhai_capabilities(
            &source.workspace,
            &[CapabilityName::new("platform.events").expect("event capability")],
        )
        .expect_err("missing HTTP declaration");
        assert!(matches!(
            error,
            AlloyArtifactError::CapabilityDeclarationMismatch { missing, unused }
                if missing == vec!["platform.http"] && unused.is_empty()
        ));

        let error = validate_rhai_capabilities(
            &source.workspace,
            &[
                CapabilityName::new("platform.events").expect("event capability"),
                CapabilityName::new("platform.http").expect("HTTP capability"),
                CapabilityName::new("platform.secrets").expect("secret capability"),
            ],
        )
        .expect_err("unused secret declaration");
        assert!(matches!(
            error,
            AlloyArtifactError::CapabilityDeclarationMismatch { missing, unused }
                if missing.is_empty() && unused == vec!["platform.secrets"]
        ));
    }

    #[test]
    fn release_capability_validation_rejects_dynamic_and_shadowed_helpers() {
        let dynamic = script(
            r#"
                let capability = "platform.events";
                capability_call(capability, "emit", #{});
            "#,
        );
        assert!(matches!(
            validate_rhai_capabilities(&dynamic.workspace, &[]),
            Err(AlloyArtifactError::DynamicCapabilityCall { .. })
        ));

        let shadowed = script("fn http_get(url) { url }");
        assert!(matches!(
            validate_rhai_capabilities(&shadowed.workspace, &[]),
            Err(AlloyArtifactError::ReservedCapabilityHelper { .. })
        ));
    }

    #[tokio::test]
    async fn reviewed_rhai_source_packages_at_a_digest_pinned_oci_reference() {
        let source = script("input.total * 0.2");
        let draft = stage_rhai_module_release("tax_adjustment", "1.0.0", &source, Vec::new())
            .expect("stage release");
        let package = package_rhai_module_release(
            rustok_modules::OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/tax_adjustment".to_string(),
                digest: format!("sha256:{}", "c".repeat(64)),
            },
            &draft,
            &source,
        )
        .await
        .expect("package release");

        assert_ne!(package.reference.digest, draft.descriptor.artifact_digest);
        assert_eq!(
            package.media_type,
            rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
        );
        assert!(matches!(
            package.payload,
            rustok_modules::ArtifactPayloadSource::Bytes(payload)
                if payload == source.workspace.canonical_bytes().expect("workspace bytes")
        ));
    }
}
