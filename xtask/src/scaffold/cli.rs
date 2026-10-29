use super::options::ScaffoldOptions;
use std::path::PathBuf;

/// Render an optional, discoverable CLI adapter. It intentionally starts with
/// an empty command catalog: a slug cannot define safe owner operations.
pub(crate) fn render_cli_templates(
    opts: &ScaffoldOptions,
    pascal: &str,
) -> Vec<(PathBuf, String)> {
    if !opts.with_cli {
        return Vec::new();
    }

    let crate_name = opts.crate_name();
    let crate_ident = opts.kebab_name().replace('-', "_");
    let cli_crate = format!("{crate_name}-cli");
    let provider = format!("{pascal}CommandProvider");
    let title = &opts.title;

    let cargo = format!(
        r#"[package]
name = "{cli_crate}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Optional CLI adapter for {crate_name}"
readme = "README.md"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[dependencies]
{crate_name}.workspace = true
rustok-cli-core.workspace = true
rustok-runtime.workspace = true
"#
    );

    let readme = format!(
        r#"# {cli_crate}

Optional command-provider package for the {title} module.

## Current state

The provider is registered through rustok-module.toml and the platform CLI registry, but it intentionally exposes zero commands. The generator cannot infer safe command names, input schemas, authorization rules, tenant scope, or side effects from a module slug.

Add commands only after the owner service, tenant/context checks, authorization policy, idempotency requirements, dry-run behavior, and failure contract are specified. Keep parsing and dispatch in this crate; business policy and persistence stay in the backend module.

## Reference

- Blog production CLI adapter: ../../rustok-blog/cli/README.md
- Reuse its CommandProvider and owner-service boundary patterns, not its domain-specific commands.

## Verification

- cargo check -p {cli_crate}
- cargo test -p {cli_crate}
- node scripts/generate/generate-cli-registry.mjs --check
"#
    );

    let api = format!(
        r#"# {cli_crate} / CRATE_API

## Role

Optional CLI adapter for {crate_name}. The initial provider is discoverable but defines no operational commands.

## Platform Capability Bindings

- Implements CommandProvider from rustok-cli-core.
- Depends on and re-exports the owner backend crate as owner.
- Receives RuntimeComposition from the platform CLI registry.
- Does not own domain policy or persistence.
- Blog's production CLI adapter is the reference for service-backed commands and tenant-safe execution; its Blog commands are not copied.

## Public surface

- {provider}: empty command catalog until the module owner defines real operations.
- command_provider(runtime): factory consumed by the platform CLI registry.

## Minimum Contract Set

### Input DTOs/Commands

- None generated. A module name is not sufficient to infer safe commands.

### Domain Invariants

- No business operation executes from the generated provider.

### Events / Outbox Side Effects

- None. The empty provider does not mutate state or publish events.

### Errors / Failure Codes

- The shared CommandProvider default reports UnknownCommand for an unimplemented command.
"#
    );

    let lib = format!(
        r#"//! Optional CLI adapter for {crate_name}.
//!
//! Add commands only after owner services and their safety contracts exist.

use rustok_cli_core::{{CommandDescriptor, CommandProvider}};
use rustok_runtime::RuntimeComposition;

pub use rustok_{crate_ident} as owner;

#[cfg(test)]
mod contract_tests;

/// Empty until the module owner adds real operational commands.
pub struct {provider};

impl CommandProvider for {provider} {{
    fn commands(&self) -> Vec<CommandDescriptor> {{
        Vec::new()
    }}
}}

pub fn command_provider(_runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {{
    Box::new({provider})
}}
"#
    );

    let tests = format!(
        r#"use rustok_cli_core::CommandProvider;

use crate::{provider};

#[test]
fn generated_provider_does_not_invent_business_commands() {{
    let provider = {provider};
    assert!(provider.commands().is_empty());
}}

#[test]
fn cli_contract_documents_owner_work_required_before_commands_exist() {{
    let api = include_str!("../CRATE_API.md");
    assert!(api.contains("None generated. A module name is not sufficient to infer safe commands."));
    assert!(api.contains("UnknownCommand"));
}}
"#
    );

    [
        ("Cargo.toml", cargo),
        ("README.md", readme),
        ("CRATE_API.md", api),
        ("src/lib.rs", lib),
        ("src/contract_tests.rs", tests),
    ]
    .into_iter()
    .map(|(file, contents)| (PathBuf::from("cli").join(file), contents))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::render_cli_templates;
    use crate::scaffold::options::{ScaffoldOptions, ScaffoldProfile, ScaffoldUi};

    fn options(with_cli: bool) -> ScaffoldOptions {
        ScaffoldOptions {
            profile: ScaffoldProfile::NativeModule,
            slug: "example".into(),
            title: "Example".into(),
            description: "A native example module without business logic".into(),
            category: "collaboration".into(),
            with_db: false,
            with_fba: false,
            with_cli,
            ui: ScaffoldUi::None,
            dry_run: true,
            skip_server_wire: false,
            skip_validate: false,
        }
    }

    #[test]
    fn cli_package_is_opt_in() {
        assert!(render_cli_templates(&options(false), "Example").is_empty());
        let files = render_cli_templates(&options(true), "Example");
        assert_eq!(files.len(), 5);
        let manifest = files.iter().find(|(path, _)| path.ends_with("Cargo.toml")).unwrap();
        assert!(manifest.1.contains("rustok-cli-core.workspace = true"));
        assert!(manifest.1.contains("rustok-example.workspace = true"));
        let source = files.iter().find(|(path, _)| path.ends_with("src/lib.rs")).unwrap();
        assert!(source.1.contains("pub use rustok_example as owner;"));
        assert!(source.1.contains("Vec::new()"));
        assert!(!source.1.contains("CommandDescriptor::new"));
    }
}
