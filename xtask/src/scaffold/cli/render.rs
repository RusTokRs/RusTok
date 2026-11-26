use super::options::Options;
use anyhow::{ensure, Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use syn::visit::Visit;
use syn::{Item, Visibility};

#[derive(Default)]
struct PlaceholderCheck(bool);

impl<'ast> Visit<'ast> for PlaceholderCheck {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        self.0 |= invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "todo" || segment.ident == "unimplemented");
    }
}

fn read_source(path: &Path) -> Result<(String, syn::File)> {
    let metadata = fs::metadata(path).with_context(|| format!("Cannot read {}", path.display()))?;
    ensure!(
        metadata.is_file() && metadata.len() <= 32 * 1024,
        "Expected a regular CLI source file up to 32 KiB at {}",
        path.display()
    );
    let source = fs::read_to_string(path)?;
    let syntax =
        syn::parse_file(&source).with_context(|| format!("Invalid Rust in {}", path.display()))?;
    let mut check = PlaceholderCheck::default();
    check.visit_file(&syntax);
    ensure!(!check.0, "Unimplemented CLI source at {}", path.display());
    Ok((source, syntax))
}

pub(super) fn files(
    options: &Options,
    owner: &str,
    catalog: &toml::Value,
) -> Result<Vec<(String, String)>> {
    let (adapter, syntax) = read_source(&options.adapter)?;
    ensure!(
        syntax
            .items
            .iter()
            .any(|item| matches!(item, Item::Fn(operation)
            if operation.sig.ident == "execute"
                && operation.sig.asyncness.is_some()
                && operation.sig.inputs.len() == 2
                && !operation.block.stmts.is_empty()
                && !matches!(operation.vis, Visibility::Inherited))),
        "The adapter must expose async fn execute(runtime, request)"
    );
    let (tests, syntax) = read_source(&options.tests)?;
    ensure!(
        syntax.items.iter().any(
            |item| matches!(item, Item::Fn(test) if !test.block.stmts.is_empty() && test.attrs
            .iter().any(|attr| attr.path().is_ident("test")
                || (attr.path().segments.len() == 2
                    && attr.path().segments[0].ident == "tokio"
                    && attr.path().segments[1].ident == "test")))
        ),
        "The supplied contract must contain a test function"
    );

    let mut dependencies: BTreeSet<_> = ["async-trait", "rustok-cli-core", "rustok-runtime", owner]
        .into_iter()
        .collect();
    dependencies.extend(options.dependencies.iter().map(String::as_str));
    for key in dependencies.iter().copied().chain(["tokio"]) {
        ensure!(
            catalog.get(key).is_some(),
            "Unknown workspace dependency '{key}'"
        );
    }
    let dependencies = dependencies
        .into_iter()
        .map(|name| format!("{name}.workspace = true\n"))
        .collect::<String>();
    let package = format!("{owner}-cli");
    let manifest = format!(
        r#"[package]
name = "{package}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
readme = "README.md"
repository.workspace = true
description = "Operational command adapters for {owner}"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lints]
workspace = true

[dependencies]
{dependencies}
[dev-dependencies]
tokio.workspace = true
"#
    );
    let (slug, name, operation, summary) = (
        &options.slug,
        &options.name,
        &options.operation,
        &options.summary,
    );
    let readme = format!(
        r#"# {package}

The `{slug} {name}` command invokes an operation owned by `{owner}`.

- `src/provider.rs` owns discovery and dispatch.
- `src/commands/{operation}.rs` adapts the owner operation.
- `src/tests/contract.rs` exercises the command contract.

See the [CLI adapter contract](../../../../docs/backend/module-backend-implementation.md#cli-adapters)
and the [crate API](CRATE_API.md).
"#
    );
    let api = format!(
        r#"# {package} API

`command_provider(&RuntimeComposition)` exposes `{slug} {name}` through the platform CLI registry.

{summary}

The command returns `CommandOutcome` or a typed `CliCoreError`.
"#
    );
    Ok(vec![
        ("Cargo.toml".into(), manifest),
        ("README.md".into(), readme),
        ("CRATE_API.md".into(), api),
        (
            "src/lib.rs".into(),
            format!(
                "//! Operational CLI adapter for `{owner}`.\n\nmod commands;\nmod provider;\n\npub use provider::command_provider;\n\n#[cfg(test)]\nmod tests;\n"
            ),
        ),
        ("src/provider.rs".into(), provider(options)),
        ("src/commands/mod.rs".into(), format!("pub(super) mod {operation};\n")),
        (format!("src/commands/{operation}.rs"), adapter),
        ("src/tests/mod.rs".into(), "mod contract;\n".into()),
        ("src/tests/contract.rs".into(), tests),
    ])
}

fn provider(options: &Options) -> String {
    let Options {
        slug,
        name,
        operation,
        summary,
        supports_dry_run,
        ..
    } = options;
    let dry_run_descriptor = if *supports_dry_run {
        ".with_dry_run()"
    } else {
        ""
    };
    let reject_dry_run = if *supports_dry_run {
        ""
    } else {
        r#"
        if request.dry_run {
            return Err(CliCoreError::DryRunNotSupported {
                namespace: request.namespace,
                name: request.name,
            });
        }
"#
    };
    format!(
        r#"use rustok_cli_core::{{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
}};
use rustok_runtime::RuntimeComposition;

use crate::commands::{operation};

struct Provider {{
    runtime: RuntimeComposition,
}}

#[async_trait::async_trait]
impl CommandProvider for Provider {{
    fn commands(&self) -> Vec<CommandDescriptor> {{
        vec![CommandDescriptor::new(
            {slug:?},
            {name:?},
            {summary:?},
        ){dry_run_descriptor}]
    }}

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {{
        if request.namespace != {slug:?} || request.name != {name:?} {{
            return Err(CliCoreError::UnknownCommand {{
                namespace: request.namespace,
                name: request.name,
            }});
        }}
{reject_dry_run}
        {operation}::execute(&self.runtime, request).await
    }}
}}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {{
    Box::new(Provider {{ runtime: runtime.clone() }})
}}
"#
    )
}
