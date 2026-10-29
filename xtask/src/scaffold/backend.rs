use super::options::ScaffoldOptions;
use super::write_file_content;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Render an intentionally minimal native runtime module. There is no generic
/// business entity, CRUD contract or persistence schema that can be generated
/// correctly from a slug alone.
pub(crate) fn render_backend(opts: &ScaffoldOptions, pascal: &str) -> Vec<(PathBuf, String)> {
    let slug = &opts.slug;
    let crate_name = opts.crate_name();
    let title = &opts.title;
    let description = &opts.description;
    let title_toml = toml::Value::String(title.clone()).to_string();
    let description_toml = toml::Value::String(description.clone()).to_string();
    let category_toml = toml::Value::String(opts.category.clone()).to_string();
    let title_rust = format!("{title:?}");
    let description_rust = format!("{description:?}");
    let module_type = format!("{pascal}Module");

    let cargo = format!(
        r#"[package]
name = "{crate_name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = {description_toml}
repository.workspace = true
readme = "README.md"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[dependencies]
async-trait.workspace = true
rustok-core.workspace = true
sea-orm-migration.workspace = true

[features]
default = []
"#
    );

    let manifest = format!(
        r#"[module]
slug = "{slug}"
name = {title_toml}
version = "0.1.0"
description = {description_toml}
ownership = "first_party"
trust_level = "private"
ui_classification = "no_ui"
recommended_admin_surfaces = []

[marketplace]
category = {category_toml}
tags = ["{slug}"]

[crate]
entry_type = "{module_type}"
"#
    );

    let readme = format!(
        r#"# {crate_name}

{description}

## Purpose
Native runtime module identity for {title}. No domain capability has been implemented yet.

## Responsibilities
- Own the module identity and its future domain contract.
- Keep domain, persistence and transport implementation absent until an owner
  operation and its invariants have been specified.

## Entry points
- `{module_type}` implements `RusToKModule` and `MigrationSource`.
- No services, persistence tables, GraphQL, HTTP, UI or event producers are declared.

## Interactions
- Platform runtime discovery only. No external capability or module dependency is required.

## Verification
- `cargo xtask module lint {slug}`
- `cargo xtask module validate {slug}`
- `cargo check -p {crate_name}`
- `cargo test -p {crate_name} --lib`

## Documentation
- [Owner contract](./CRATE_API.md)
- [Architecture](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
"#
    );

    let docs = format!(
        r#"# {title} native module

## Purpose
Define the native runtime identity of the {slug} module.

## Responsibility Zone
Only `src/module.rs` composes the module registration metadata.
Domain state, application services and storage have not been defined.

## Integration
The module can be registered by the platform runtime; it contributes no
transport, UI, migrations, outbox producers or external capability adapters.

## Verification
- `cargo xtask module lint {slug}` — static source architecture
- `cargo xtask module validate {slug}` — publish metadata and composition
- `cargo check -p {crate_name}` and `cargo test -p {crate_name} --lib` — Rust evidence

## Related Documentation
- [Owner implementation plan](./implementation-plan.md)
- [Backend contract](../../../../../docs/backend/module-backend-implementation.md)
"#
    );

    let plan = format!(
        r#"# {title} implementation plan

## Scope
Establish native ownership before implementing a business capability.

## Current state
- Runtime identity: scaffolded, not independently verified
- Domain operations: not_started
- Data schema and persistence: not_started
- FBA status: `not_started`
- FFA status: `not_started`
- UI surfaces: none advertised
- No business functionality or runtime transport parity is claimed.

## Local work phases
- [x] Define native module runtime metadata and contract tests.
- [ ] Specify domain rules, owner service/DTO commands and typed context/errors.
- [ ] Design real persistence and migrations if required.
- [ ] Add GraphQL/HTTP/Leptos/Next.js transports only for real operations.
- [ ] Verify runtime, FBA/FFA evidence, registration and release readiness.

## Verification
- `cargo xtask module lint {slug}`
- `cargo xtask module validate {slug}`
- `cargo check -p {crate_name}`
- `cargo test -p {crate_name} --lib`

## Change rules
Synchronize implementation, manifest, local contracts and central registry.
Do not promote FBA/FFA readiness without executable evidence.
"#
    );

    let api = format!(
        r#"# {crate_name} / CRATE_API

## Role
Native runtime owner identity for {title}. No business API is published yet.

## Public surface
- `{module_type}`: module metadata and runtime registration.
- `src/lib.rs` is a thin facade. There are no public persistence entities.

## Primary entry points
- `{module_type}` from `src/module.rs`.

## Platform Capability Bindings
- Runtime module registration through `RusToKModule`.
- `MigrationSource` returns no migrations because no owner schema exists.

## Domain Collaborations
- None. External capabilities are not declared speculatively.

## Minimum Contract Set

### Input DTOs/Commands
- Not implemented. No create/update/delete commands are advertised.

### Domain Invariants
- Not implemented until the business aggregate is specified.

### Events / Outbox Side Effects
- None. There are no state-changing operations.

### Errors / Failure Codes
- No business error contract exists yet. No transport is exposed.
"#
    );

    let lib = format!(
        r#"//! Native runtime module for {title}.
//!
//! Business API and persistence are intentionally absent until specified.

mod module;

pub use module::{module_type};

#[cfg(test)]
mod tests;
"#
    );

    let module = format!(
        r#"use async_trait::async_trait;
use rustok_core::module::{{MigrationSource, ModuleKind, RusToKModule}};
use sea_orm_migration::MigrationTrait;

/// Runtime identity only; does not claim implemented domain operations.
pub struct {module_type};

#[async_trait]
impl RusToKModule for {module_type} {{
    fn slug(&self) -> &'static str {{
        "{slug}"
    }}

    fn name(&self) -> &'static str {{
        {title_rust}
    }}

    fn description(&self) -> &'static str {{
        {description_rust}
    }}

    fn version(&self) -> &'static str {{
        env!("CARGO_PKG_VERSION")
    }}

    fn kind(&self) -> ModuleKind {{
        ModuleKind::Optional
    }}

    fn dependencies(&self) -> &[&'static str] {{
        &[]
    }}
}}

impl MigrationSource for {module_type} {{
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {{
        Vec::new()
    }}
}}
"#
    );

    let tests = format!(
        r#"use crate::{module_type};
use rustok_core::module::{{MigrationSource, ModuleKind, RusToKModule}};

#[test]
fn native_contract_matches_the_declared_module() {{
    let module = {module_type};
    assert_eq!(module.slug(), "{slug}");
    assert_eq!(module.name(), {title_rust});
    assert_eq!(module.description(), {description_rust});
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert!(module.dependencies().is_empty());
    assert!(module.migrations().is_empty());
}}

#[test]
fn public_contract_does_not_claim_unimplemented_capabilities() {{
    let api = include_str!("../../CRATE_API.md");
    for section in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {{
        assert!(api.contains(section), "Missing CRATE_API section: {{section}}");
    }}
    assert!(api.contains("No create/update/delete commands are advertised"));
}}
"#
    );

    [
        ("Cargo.toml", cargo),
        ("rustok-module.toml", manifest),
        ("README.md", readme),
        ("docs/README.md", docs),
        ("docs/implementation-plan.md", plan),
        ("CRATE_API.md", api),
        ("src/lib.rs", lib),
        ("src/module.rs", module),
        ("src/tests/mod.rs", "mod contract;\n".to_string()),
        ("src/tests/contract.rs", tests),
    ]
    .into_iter()
    .map(|(file, content)| (PathBuf::from(file), content))
    .collect()
}

pub(crate) fn generate_backend(
    opts: &ScaffoldOptions,
    module_dir: &Path,
    pascal: &str,
    _snake: &str,
) -> Result<()> {
    for (path, content) in render_backend(opts, pascal) {
        write_file_content(&module_dir.join(path), &content, opts.dry_run)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render_backend;
    use crate::scaffold::options::{ScaffoldOptions, ScaffoldProfile, ScaffoldUi};

    fn options() -> ScaffoldOptions {
        ScaffoldOptions {
            profile: ScaffoldProfile::NativeModule,
            slug: "example".into(),
            title: "Example".into(),
            description: "A native example module without business logic".into(),
            category: "collaboration".into(),
            with_db: false,
            ui: ScaffoldUi::None,
            dry_run: true,
            skip_server_wire: false,
            skip_validate: false,
        }
    }

    #[test]
    fn minimal_backend_has_only_real_responsibilities() {
        let files = render_backend(&options(), "Example");
        let paths: Vec<_> = files.iter().map(|(path, _)| path.to_str().unwrap()).collect();
        assert_eq!(paths.len(), 10);
        assert!(paths.contains(&"src/tests/contract.rs"));
        assert!(!paths.contains(&"src/contract_tests.rs"));
        assert!(!paths.iter().any(|path| path.contains("entities")
            || path.contains("migrations") || path.contains("services")
            || path.contains("integrations") || path.contains("domain")));
        let lib = files.iter().find(|(path, _)| path == "src/lib.rs").unwrap().1.as_str();
        assert!(lib.contains("mod module;"));
        assert!(!lib.contains("pub mod entities;"));
    }

    #[test]
    fn metadata_is_parseable_and_reports_no_ui_or_fake_dependencies() {
        let files = render_backend(&options(), "Example");
        let manifest = files.iter().find(|(path, _)| path == "rustok-module.toml").unwrap();
        let value: toml::Value = toml::from_str(&manifest.1).unwrap();
        assert_eq!(value["module"]["ui_classification"].as_str(), Some("no_ui"));
        assert_eq!(value["module"]["trust_level"].as_str(), Some("private"));
        assert!(value.get("provides").is_none());
        assert!(value.get("dependencies").is_none());
        let cargo = files.iter().find(|(path, _)| path == "Cargo.toml").unwrap();
        let parsed: toml::Value = toml::from_str(&cargo.1).unwrap();
        assert!(parsed["dependencies"].get("sea-orm").is_none());
        assert!(parsed["dependencies"].get("rustok-outbox").is_none());
    }

    #[test]
    fn caller_strings_are_escaped_for_rust_and_toml() {
        let mut opts = options();
        opts.title = "An \"Example\"".into();
        opts.description = "A \\quoted\\ module with a \"name\" and valid description".into();
        let files = render_backend(&opts, "Example");
        let metadata = files.iter().find(|(path, _)| path == "rustok-module.toml").unwrap();
        let parsed: toml::Value = toml::from_str(&metadata.1).unwrap();
        assert_eq!(parsed["module"]["name"].as_str(), Some(opts.title.as_str()));
        assert_eq!(parsed["module"]["description"].as_str(), Some(opts.description.as_str()));
        let module_rs = files.iter().find(|(path, _)| path == "src/module.rs").unwrap().1.as_str();
        assert!(module_rs.contains(&format!("{:?}", opts.title)));
    }
}
