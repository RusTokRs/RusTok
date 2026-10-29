use super::options::ScaffoldOptions;
use std::path::PathBuf;

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
    let ui_requested = opts.ui != super::options::ScaffoldUi::None;
    let ui_classification = if ui_requested { "future_ui" } else { "no_ui" };
    let ui_summary = match opts.ui {
        super::options::ScaffoldUi::None => "No UI packages requested.",
        super::options::ScaffoldUi::Admin => "Draft Leptos admin crate created but NOT mounted in the host.",
        super::options::ScaffoldUi::Storefront => "Draft Leptos storefront crate created but NOT mounted in the host.",
        super::options::ScaffoldUi::Dual => "Draft Leptos admin and storefront crates created but NOT mounted in hosts.",
    };

    let fba_dependency = if opts.with_fba {
        "rustok-fba.workspace = true\n"
    } else {
        ""
    };
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
{fba_dependency}rustok-core.workspace = true
sea-orm-migration.workspace = true

[features]
default = []
"#
    );

    let cli_crate_ident = opts.kebab_name().replace('-', "_");
    let cli_namespace = if slug.chars().next().map(|character| character.is_ascii_lowercase()).unwrap_or(false) {
        slug.clone()
    } else {
        format!("module_{slug}")
    };
    let cli_manifest = if opts.with_cli {
        format!(
            r#"
[provides.cli]
namespace = "{cli_namespace}"
factory = "rustok_{cli_crate_ident}_cli::command_provider"
"#
        )
    } else {
        String::new()
    };

    let manifest = format!(
        r#"[module]
slug = "{slug}"
name = {title_toml}
version = "0.1.0"
description = {description_toml}
ownership = "first_party"
trust_level = "private"
ui_classification = "{ui_classification}"
recommended_admin_surfaces = []

[marketplace]
category = {category_toml}
tags = ["{slug}"]

[crate]
entry_type = "{module_type}"
{cli_manifest}
"#
    );

    let fba_summary = if opts.with_fba {
        "The optional integration uses the shared rustok-fba library; no provider or operation is claimed yet."
    } else {
        "No FBA library dependency or provider contract is generated."
    };
    let cli_summary = if opts.with_cli {
        "The optional CLI provider is registered but exposes zero commands until the module owner adds real operations."
    } else {
        "No CLI package is generated unless --with-cli is requested."
    };

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
- No services, persistence tables, business GraphQL/HTTP endpoints or event producers are declared.
- {ui_summary} No [provides.*_ui] live surface is declared.
- {fba_summary}
- {cli_summary}

## Interactions
- Platform runtime discovery only; the optional FBA helper uses the shared library without declaring a cross-module provider or consumer.
- The optional CLI adapter is discoverable but exposes zero commands until owner operations exist.

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
business transport, active UI routes, migrations, outbox producers or external capability adapters.
{ui_summary}
{fba_summary}
{cli_summary}

## Verification
- `cargo xtask module lint {slug}` — static source architecture
- `cargo xtask module validate {slug}` — publish metadata and composition
- `cargo check -p {crate_name}` and `cargo test -p {crate_name} --lib` — Rust evidence

## Related Documentation
- [Owner implementation plan](./implementation-plan.md)
- [Backend contract](../../../../docs/backend/module-backend-implementation.md)
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
- FBA status: not_started
- FBA integration: {fba_summary}
- CLI integration: {cli_summary}
- FFA status: not_started
- UI surfaces: {ui_summary}
- UI readiness: draft skeleton, not_started, no live host mount or owner operations
- No business functionality or runtime transport parity is claimed.

## Local work phases
- [x] Define native module runtime metadata and contract tests.
- [ ] Specify domain rules, owner service/DTO commands and typed context/errors.
- [ ] Design real persistence and migrations if required.
- [ ] If FBA was selected, define real owner ports and a schema-valid provider/consumer registry using the shared rustok-fba library.
- [ ] If CLI was selected, add owner-backed commands only after authorization, tenant/context, idempotency, dry-run and error contracts are specified.
- [ ] Implement owner-specific native and GraphQL transports and mount Leptos UI after real operations.
- [ ] Implement any independent classic Next.js client separately in its host, if needed.
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
- MigrationSource returns no migrations because no owner schema exists.
- {fba_summary}
- {cli_summary}

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

    let fba_module = if opts.with_fba { "pub mod fba;\n" } else { "" };
    let lib = format!(
        r#"//! Native runtime module for {title}.
//!
//! Business API and persistence are intentionally absent until specified.

mod module;
{fba_module}
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

    let test_modules = if opts.with_fba {
        "mod contract;\nmod fba;\n"
    } else {
        "mod contract;\n"
    };

    let mut files = vec![
        ("Cargo.toml", cargo),
        ("rustok-module.toml", manifest),
        ("README.md", readme),
        ("docs/README.md", docs),
        ("docs/implementation-plan.md", plan),
        ("CRATE_API.md", api),
        ("src/lib.rs", lib),
        ("src/module.rs", module),
        ("src/tests/mod.rs", test_modules.to_string()),
        ("src/tests/contract.rs", tests),
    ]
    .into_iter()
    .map(|(file, content)| (PathBuf::from(file), content))
    .collect::<Vec<_>>();

    if opts.with_fba {
        files.push((
            PathBuf::from("src/fba.rs"),
            format!(
                r#"//! Typed integration surface for the shared FBA contract library.
//!
//! No provider capability is advertised until the owner defines real ports.

pub use rustok_fba as shared;
pub use rustok_fba::{{BackendTopology, CapabilityId, FbaCallContext, FbaConsumerDependency, FbaProviderDescriptor, FbaResult, TransportProfile}};

pub const MODULE_SLUG: &str = "{slug}";

pub fn capability_id(capability: impl Into<String>, version: impl Into<String>) -> CapabilityId {{
    CapabilityId::new(MODULE_SLUG, capability, version)
}}
"#
            ),
        ));
        files.push((
            PathBuf::from("contracts/README.md"),
            format!(
                r#"# {crate_name} FBA contracts

This module opted into the shared rustok-fba library and the src/fba.rs boundary helper.

**No provider or consumer registry is generated yet.** The module has no specified owner ports or cross-module operations, so creating a registry now would invent contracts or violate the shared registry schema.

When real ports are defined, add the owner registry and reference it from rustok-module.toml under [fba.provider] or [fba.consumer]. Follow the shared schema at crates/libs/rustok-fba/contracts/provider-registry-v1.schema.json (../../../libs/rustok-fba/contracts/provider-registry-v1.schema.json), and record evidence in docs/implementation-plan.md.
"#
            ),
        ));
        files.push((
            PathBuf::from("src/tests/fba.rs"),
            format!(
                r#"#[test]
fn capability_identity_uses_the_canonical_module_slug() {{
    let id = crate::fba::capability_id("sample_capability", "1.0.0");
    assert_eq!(id.module, "{slug}");
    assert_eq!(id.capability, "sample_capability");
    assert_eq!(id.version, "1.0.0");
}}
"#
            ),
        ));
    }

    files
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
            with_fba: false,
            with_cli: false,
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
        let lib = files.iter().find(|(path, _)| path.to_str() == Some("src/lib.rs")).unwrap().1.as_str();
        assert!(lib.contains("mod module;"));
        assert!(!lib.contains("pub mod entities;"));
    }

    #[test]
    fn metadata_is_parseable_and_reports_no_ui_or_fake_dependencies() {
        let files = render_backend(&options(), "Example");
        let manifest = files.iter().find(|(path, _)| path.to_str() == Some("rustok-module.toml")).unwrap();
        let value: toml::Value = toml::from_str(&manifest.1).unwrap();
        assert_eq!(value["module"]["ui_classification"].as_str(), Some("no_ui"));
        assert_eq!(value["module"]["trust_level"].as_str(), Some("private"));
        assert!(value.get("provides").is_none());
        assert!(value.get("dependencies").is_none());
        let cargo = files.iter().find(|(path, _)| path.to_str() == Some("Cargo.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&cargo.1).unwrap();
        assert!(parsed["dependencies"].get("sea-orm").is_none());
        assert!(parsed["dependencies"].get("rustok-outbox").is_none());
    }

    #[test]
    fn requested_ui_creates_only_future_ui_metadata_without_live_provides() {
        for ui in [ScaffoldUi::Admin, ScaffoldUi::Storefront, ScaffoldUi::Dual] {
            let mut opts = options();
            opts.ui = ui;
            let files = render_backend(&opts, "Example");
            let manifest = &files.iter().find(|(p, _)| p.to_str() == Some("rustok-module.toml")).unwrap().1;
            let value: toml::Value = toml::from_str(manifest).unwrap();
            assert_eq!(value["module"]["ui_classification"].as_str(), Some("future_ui"));
            assert!(value.get("provides").is_none(), "draft UI must not mount a live surface");
            let plan = &files.iter().find(|(p, _)| p.to_str() == Some("docs/implementation-plan.md")).unwrap().1;
            assert!(plan.contains("not_started"));
            assert!(plan.contains("NOT mounted"));
        }
    }

    #[test]
    fn fba_is_opt_in_and_uses_the_shared_library_without_fake_registry() {
        let mut opts = options();
        opts.with_fba = true;
        let files = render_backend(&opts, "Example");
        let cargo = &files.iter().find(|(path, _)| path.to_str() == Some("Cargo.toml")).unwrap().1;
        assert!(cargo.contains("rustok-fba.workspace = true"));
        let manifest = &files.iter().find(|(path, _)| path.to_str() == Some("rustok-module.toml")).unwrap().1;
        assert!(!manifest.contains("[fba.provider]"));
        assert!(files.iter().any(|(path, _)| path.to_str() == Some("src/fba.rs")));
        assert!(files.iter().any(|(path, _)| path.to_str() == Some("contracts/README.md")));
        let test_modules = &files.iter().find(|(path, _)| path.to_str() == Some("src/tests/mod.rs")).unwrap().1;
        assert!(test_modules.contains("mod fba;"));
    }

    #[test]
    fn cli_manifest_binding_is_opt_in_and_uses_canonical_factory_path() {
        let mut opts = options();
        opts.with_cli = true;
        let files = render_backend(&opts, "Example");
        let manifest = &files.iter().find(|(path, _)| path.to_str() == Some("rustok-module.toml")).unwrap().1;
        assert!(manifest.contains("[provides.cli]"));
        assert!(manifest.contains("namespace = \"example\""));
        assert!(manifest.contains("factory = \"rustok_example_cli::command_provider\""));
        assert!(!files.iter().any(|(path, _)| path.to_str() == Some("src/fba.rs")));
    }

    #[test]
    fn caller_strings_are_escaped_for_rust_and_toml() {
        let mut opts = options();
        opts.title = "An \"Example\"".into();
        opts.description = "A \\quoted\\ module with a \"name\" and valid description".into();
        let files = render_backend(&opts, "Example");
        let metadata = files.iter().find(|(path, _)| path.to_str() == Some("rustok-module.toml")).unwrap();
        let parsed: toml::Value = toml::from_str(&metadata.1).unwrap();
        assert_eq!(parsed["module"]["name"].as_str(), Some(opts.title.as_str()));
        assert_eq!(parsed["module"]["description"].as_str(), Some(opts.description.as_str()));
        let module_rs = files.iter().find(|(path, _)| path.to_str() == Some("src/module.rs")).unwrap().1.as_str();
        assert!(module_rs.contains(&format!("{:?}", opts.title)));
    }
}
