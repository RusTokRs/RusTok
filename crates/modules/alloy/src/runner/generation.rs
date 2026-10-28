//! Deterministic Rust/WASM source generation for reviewed Alloy rewrites.
//!
//! The AI/provider is responsible for producing the reviewed implementation
//! body. Alloy owns the ABI wrapper: it renders the canonical standalone
//! Component template, binds the exact owner-selected WIT identity, and emits
//! one typed `Guest` implementation. No Rhai AST compilation or filesystem
//! access happens in this layer.

use crate::model::RustComponentSourceFile;
use rustok_module_sdk::{WIT_PACKAGE, WIT_SOURCE, WIT_WORLD};
use rustok_module_template::{ModuleTemplateError, ModuleTemplateInput, render};
use rustok_modules::{
    MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_WIT_VERSION,
    MODULE_BUILD_WIT_WORLD, ModuleBuildWitContract,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Maximum reviewed implementation body accepted by the deterministic
/// generator. The final candidate workspace remains bounded independently.
pub const MAX_RUST_COMPONENT_IMPLEMENTATION_BODY_BYTES: usize = 256 * 1024;

/// Data-only request used after an AI/code-generation stage has produced a
/// proposed Rust implementation body.
///
/// The implementation body is deliberately not interpreted as an AST here. It
/// is wrapped in a typed WIT guest implementation and remains subject to
/// source review and the isolated module build before execution or publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentGenerationRequest {
    pub slug: String,
    pub version: String,
    pub display_name: String,
    pub wit: ModuleBuildWitContract,
    pub implementation_body: String,
}

impl RustComponentGenerationRequest {
    pub fn validate(&self) -> Result<(), RustComponentGenerationError> {
        if self.slug.trim().is_empty()
            || self.version.trim().is_empty()
            || self.display_name.trim().is_empty()
            || self.implementation_body.trim().is_empty()
            || self.implementation_body.len() > MAX_RUST_COMPONENT_IMPLEMENTATION_BODY_BYTES
            || self.implementation_body.contains('\0')
        {
            return Err(RustComponentGenerationError::InvalidInput);
        }

        // The owner build contract uses the package/world form, while the SDK
        // exposes package and world separately. Require both to resolve to the
        // exact same WIT world before any Rust source is emitted.
        let expected_world = format!("rustok:module/{WIT_WORLD}");
        if self.wit.world != expected_world
            || self.wit.version != MODULE_BUILD_WIT_VERSION
            || MODULE_BUILD_WIT_WORLD != expected_world
            || WIT_PACKAGE != "rustok:module@1.0.0"
            || WIT_WORLD != "module-runtime"
            || MODULE_BUILD_WIT_VERSION != "1.0.0"
            || WIT_SOURCE.is_empty()
            || !WIT_SOURCE.contains("import host;")
            || !WIT_SOURCE.contains("export run: func(input: string)")
        {
            return Err(RustComponentGenerationError::WitContractMismatch);
        }

        let forbidden_fragments = [
            "impl rustok_module_sdk::Guest for",
            "rustok_module_sdk::export!",
            "wit_bindgen::generate!",
        ];
        if forbidden_fragments
            .iter()
            .any(|fragment| self.implementation_body.contains(fragment))
        {
            return Err(RustComponentGenerationError::DuplicateAbiSurface);
        }

        Ok(())
    }
}

/// Generated Rust source tree plus the exact ABI/build identities it was
/// generated against.
///
/// This is intentionally not a `RustComponentWorkspace`: the canonical
/// module template does not invent `Cargo.lock`; the owner CLI creates the
/// lockfile with pinned Cargo before a source tree becomes a candidate-ready
/// workspace. Keeping the two types separate prevents Alloy from manufacturing
/// dependency-lock evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentGeneratedSource {
    pub files: Vec<RustComponentSourceFile>,
    pub wit: ModuleBuildWitContract,
    pub runtime_abi: String,
    pub component_target: String,
    pub generated_source_digest: String,
}

impl RustComponentGeneratedSource {
    pub fn files(&self) -> &[RustComponentSourceFile] {
        &self.files
    }
}

pub fn generate_rust_component(
    request: &RustComponentGenerationRequest,
) -> Result<RustComponentGeneratedSource, RustComponentGenerationError> {
    request.validate()?;

    let rendered = render(&ModuleTemplateInput {
        slug: request.slug.clone(),
        version: request.version.clone(),
        display_name: request.display_name.clone(),
    })
    .map_err(RustComponentGenerationError::Template)?;

    let generated_lib = typed_guest_source(&request.implementation_body);
    let files = rendered
        .files()
        .iter()
        .map(|file| {
            let contents = if file.path == "src/lib.rs" {
                generated_lib.clone()
            } else {
                String::from_utf8(file.contents.clone())
                    .map_err(|_| RustComponentGenerationError::TemplateUtf8)?
            };
            Ok(RustComponentSourceFile {
                path: file.path.to_string(),
                contents,
            })
        })
        .collect::<Result<Vec<_>, RustComponentGenerationError>>()?;

    let generated_source_digest = source_digest(&files)?;

    Ok(RustComponentGeneratedSource {
        files,
        wit: request.wit.clone(),
        runtime_abi: MODULE_BUILD_RUNTIME_ABI.to_string(),
        component_target: MODULE_BUILD_COMPONENT_TARGET.to_string(),
        generated_source_digest,
    })
}

fn typed_guest_source(implementation_body: &str) -> String {
    format!(
        r#"//! Generated Alloy Rust/WASM rewrite.
//! WIT package: {WIT_PACKAGE}
//! WIT world: {WIT_WORLD}
//! Bindings are generated by rustok-module-sdk from its canonical WIT source.

const _: &str = rustok_module_sdk::WIT_PACKAGE;
const _: &str = rustok_module_sdk::WIT_WORLD;
const _: &str = rustok_module_sdk::WIT_SOURCE;

struct Module;

impl rustok_module_sdk::Guest for Module {{
    fn run(input: String) -> Result<String, String> {{
{implementation_body}
    }}
}}

rustok_module_sdk::export!(Module);
"#,
        implementation_body = indent_body(implementation_body),
    )
}

fn indent_body(body: &str) -> String {
    body.lines()
        .map(|line| {
            if line.is_empty() {
                "        ".to_string()
            } else {
                format!("        {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn source_digest(
    files: &[RustComponentSourceFile],
) -> Result<String, RustComponentGenerationError> {
    let mut canonical = files.to_vec();
    canonical.sort_by(|left, right| left.path.cmp(&right.path));
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| RustComponentGenerationError::Serialization(error.to_string()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

#[derive(Debug, Error)]
pub enum RustComponentGenerationError {
    #[error("Rust Component generation input is invalid")]
    InvalidInput,
    #[error(
        "Rust Component generation WIT contract does not match the approved SDK/build identity"
    )]
    WitContractMismatch,
    #[error("Rust Component implementation already declares the WIT ABI surface")]
    DuplicateAbiSurface,
    #[error("canonical module template failed: {0}")]
    Template(ModuleTemplateError),
    #[error("canonical module template emitted non-UTF-8 source")]
    TemplateUtf8,
    #[error("generated Rust Component source serialization failed: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RustComponentGenerationRequest {
        RustComponentGenerationRequest {
            slug: "sample_module".to_string(),
            version: "1.1.0".to_string(),
            display_name: "Sample Module".to_string(),
            wit: ModuleBuildWitContract {
                world: MODULE_BUILD_WIT_WORLD.to_string(),
                version: MODULE_BUILD_WIT_VERSION.to_string(),
            },
            implementation_body: "let _ = input;\nOk(\"generated\".to_string())".to_string(),
        }
    }

    #[test]
    fn generation_wraps_reviewed_body_in_the_typed_guest_contract() {
        let generated = generate_rust_component(&request()).expect("generate Component source");
        let source = generated
            .files()
            .iter()
            .find(|file| file.path == "src/lib.rs")
            .expect("generated lib");
        assert!(
            source
                .contents
                .contains("impl rustok_module_sdk::Guest for Module")
        );
        assert!(
            source
                .contents
                .contains("rustok_module_sdk::export!(Module);")
        );
        assert!(source.contents.contains(WIT_PACKAGE));
        assert!(source.contents.contains(WIT_WORLD));
        assert!(source.contents.contains("let _ = input;"));
        assert_eq!(generated.runtime_abi, MODULE_BUILD_RUNTIME_ABI);
        assert_eq!(generated.component_target, MODULE_BUILD_COMPONENT_TARGET);
        assert!(
            generated
                .files()
                .iter()
                .all(|file| !file.path.contains("Cargo.lock"))
        );
        assert_eq!(
            generated.generated_source_digest,
            source_digest(generated.files()).expect("source digest")
        );
    }

    #[test]
    fn generation_requires_the_exact_owner_wit_identity() {
        let mut invalid = request();
        invalid.wit.world = "rustok:module/other-world".to_string();
        assert!(matches!(
            generate_rust_component(&invalid),
            Err(RustComponentGenerationError::WitContractMismatch)
        ));

        invalid = request();
        invalid.wit.version = "2.0.0".to_string();
        assert!(matches!(
            generate_rust_component(&invalid),
            Err(RustComponentGenerationError::WitContractMismatch)
        ));
    }

    #[test]
    fn generation_rejects_duplicate_abi_declarations() {
        let mut invalid = request();
        invalid.implementation_body =
            "impl rustok_module_sdk::Guest for Module { /* duplicate */ }".to_string();
        assert!(matches!(
            generate_rust_component(&invalid),
            Err(RustComponentGenerationError::DuplicateAbiSurface)
        ));

        invalid.implementation_body = "rustok_module_sdk::export!(Module);".to_string();
        assert!(matches!(
            generate_rust_component(&invalid),
            Err(RustComponentGenerationError::DuplicateAbiSurface)
        ));
    }

    #[test]
    fn generation_stays_data_only_and_defers_lockfile_creation_to_owner() {
        let generated = generate_rust_component(&request()).expect("generate Component source");
        let serialized = serde_json::to_string(&request()).expect("request serializes");
        assert!(!serialized.contains("Rhai"));
        assert!(
            !generated
                .files()
                .iter()
                .any(|file| { file.path.contains("..") || file.path.starts_with("/") })
        );
        assert!(
            generated
                .files()
                .iter()
                .any(|file| file.path == "src/lib.rs")
        );
        assert!(
            generated
                .files()
                .iter()
                .any(|file| file.path == "module-build-policy.toml")
        );
    }
}
