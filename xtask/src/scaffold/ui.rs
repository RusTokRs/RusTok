use super::options::{ScaffoldOptions, ScaffoldUi};
use super::write_file_content;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// The existing UI scaffold is retained as an extension point, but now emits
/// only an honest, non-operational FFA package until an owner API is designed.
/// The public scaffold command rejects non-none UI modes before any writes.
pub(crate) fn generate_ui(opts: &ScaffoldOptions, module_dir: &Path, pascal: &str) -> Result<()> {
    if opts.ui != ScaffoldUi::None && !opts.dry_run {
        anyhow::bail!("draft FFA packages can be previewed but cannot be registered without owner APIs and Next.js routes");
    }
    for (relative, contents) in render_ui_templates(opts, pascal) {
        write_file_content(&module_dir.join(relative), &contents, opts.dry_run)?;
    }
    // These follow the existing Blog Next.js package paths. Preview them
    // without importing, routing, or registering modules in either Next host.
    if opts.dry_run {
        let root = crate::manifest_io::workspace_root();
        for (relative, contents) in render_next_ui_templates(opts, pascal) {
            write_file_content(&root.join(relative), &contents, true)?;
        }
    }
    Ok(())
}

/// Pure renderer makes it possible to inspect the proposed Leptos packages
/// without registering UI routes or requiring a workspace mutation.
pub(crate) fn render_ui_templates(
    opts: &ScaffoldOptions,
    pascal: &str,
) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    if opts.ui.has_admin() {
        files.extend(render_ui_package(opts, pascal, "admin"));
    }
    if opts.ui.has_storefront() {
        files.extend(render_ui_package(opts, pascal, "storefront"));
    }
    files
}

fn render_ui_package(
    opts: &ScaffoldOptions,
    pascal: &str,
    side: &str,
) -> Vec<(PathBuf, String)> {
    let slug = &opts.slug;
    let crate_name = opts.crate_name();
    let ui_crate = format!("{crate_name}-{side}");
    let page = if side == "admin" {
        format!("{pascal}AdminPage")
    } else {
        format!("{pascal}StorefrontView")
    };
    let side_name = if side == "admin" { "Admin" } else { "Storefront" };
    let side_key = format!("{slug}-{side}-unavailable");
    let title_rust = format!("{:?}", opts.title);
    let description_rust = format!("{:?}", opts.description);
    let cargo_description = toml::Value::String(format!(
        "{side_name} FFA adapter for {crate_name} (owner API pending)"
    )).to_string();

    // No unused graphql, native-server, grid, database or module dependencies:
    // adding any of them requires implementing an actual adapter.
    let cargo = format!(
        r#"[package]
name = "{ui_crate}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = {cargo_description}
readme = "README.md"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[features]
default = []
csr = ["leptos/csr"]
hydrate = ["leptos/hydrate"]
ssr = ["leptos/ssr"]

[dependencies]
leptos.workspace = true
rustok-ui-i18n.workspace = true
"#
    );

    let readme = format!(
        r#"# {ui_crate}

Draft Leptos {side} UI adapter for the {slug} native module.

## Responsibility
- Keep UI rendering and bindings in src/ui/leptos.rs.
- Keep pure, framework-independent presentation state in src/core.rs.
- Keep all future backend calls behind src/transport/mod.rs.
- Do not claim that CRUD, GraphQL, native server transport, or Next.js parity exists.

## Current state
This is an **unavailable UI preview**, not a registered functional route.
There is no owner operation to read or mutate. The transport facade fails
closed until a genuine backend API and its transport-specific adapters exist.
No business data, permissions, tenant context or mutation outcomes are invented.

The normal scaffold command does not currently activate UI generation.
After a real module owner contract exists, implement both native and GraphQL
adapters and the corresponding Next.js package/route before registering UI
surfaces in rustok-module.toml or promoting readiness.

## Reference
- [Blog's production Leptos package](../../rustok-blog/{side}/README.md)
- [FFA implementation guide](../../../../docs/UI/module-package-implementation.md)

## Verification
- cargo check -p {ui_crate}
- cargo test -p {ui_crate}
- cargo xtask module lint {slug}
"#
    );

    let api = format!(
        r#"# {ui_crate} / CRATE_API

## Role
Draft {side} Leptos adapter for {slug}. Not published as a working FFA surface.

## Public surface
- {page} renders an explicit unavailability message.
- Pure readiness model in src/core.rs; render/bind in src/ui/leptos.rs.

## Platform Capability Bindings
- None. Native and GraphQL transports are not implemented.

## Domain Collaborations
- None; an owner API and context invariants must be specified first.

## Minimum Contract Set

### Input DTOs/Commands
- None. No CRUD or owner business operation is available.

### Domain Invariants
- Missing transport is always reported as unavailable.

### Events / Outbox Side Effects
- None. The UI never simulates successful mutations.

### Errors / Failure Codes
- MissingOwnerContract is a typed unavailable result, not an empty success.
"#
    );

    let model = r#"//! Framework- and transport-independent availability contract.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceReadiness {
    AwaitingOwnerContract,
    OwnerContractConnected,
}
"#.to_string();

    let core = r#"//! Pure FFA presentation policy, independent of Leptos and transport.
use crate::model::SurfaceReadiness;

pub const fn classify_owner_contract(available: bool) -> SurfaceReadiness {
    if available {
        SurfaceReadiness::OwnerContractConnected
    } else {
        SurfaceReadiness::AwaitingOwnerContract
    }
}
"#.to_string();

    let transport = r#"//! No backend operation is configured. This boundary refuses all calls
//! until the owner module provides a real native and GraphQL contract.
//! Do not add fake reads, static sample data or log-only write handlers.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingOwnerContract;

impl std::fmt::Display for MissingOwnerContract {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("owner transport contract has not been implemented")
    }
}

impl std::error::Error for MissingOwnerContract {}

pub fn require_owner_adapter() -> Result<(), MissingOwnerContract> {
    Err(MissingOwnerContract)
}
"#.to_string();

    let leptos = format!(
        r#"use leptos::prelude::*;
use crate::{{core, i18n::t}};

/// Shell-only adapter; the module has no owner read/write API yet.
#[component]
pub fn {page}() -> impl IntoView {{
    let state = core::classify_owner_contract(
        crate::transport::require_owner_adapter().is_ok()
    );
    let message = match state {{
        crate::model::SurfaceReadiness::AwaitingOwnerContract =>
            t(None, "{side_key}", "This module's UI is not available until its owner API is implemented."),
        crate::model::SurfaceReadiness::OwnerContractConnected =>
            t(None, "{slug}-{side}-connected", "Owner API is connected."),
    }};
    view! {{
        <section class="p-6 space-y-3" aria-live="polite">
            <h1 class="text-2xl font-semibold">{title_rust}</h1>
            <p class="text-muted-foreground">{description_rust}</p>
            <p role="status">
                {{message}}
            </p>
        </section>
    }}
}}
"#
    );
    let lib = format!(
        r#"//! Draft FFA {side} package for {slug}.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::{page};

#[cfg(test)]
mod tests;
"#
    );

    let tests = format!(
        r#"#[test]
fn owner_contract_must_be_implemented_before_ui_is_ready() {{
    assert_eq!(
        crate::core::classify_owner_contract(false),
        crate::model::SurfaceReadiness::AwaitingOwnerContract
    );
    assert_eq!(
        crate::transport::require_owner_adapter(),
        Err(crate::transport::MissingOwnerContract)
    );
}}

#[test]
fn documented_ui_is_not_a_fake_business_api() {{
    let docs = include_str!("../../CRATE_API.md");
    for section in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {{
        assert!(docs.contains(section), "Missing UI contract: {{section}}");
    }}
    assert!(docs.contains("No CRUD or owner business operation is available"));
}}
"#
    );

    let translations_en = format!(
        "{side_key} = This module's UI is not available until its owner API is implemented.\n{slug}-{side}-connected = Owner API is connected.\n"
    );
    let translations_ru = format!(
        "{side_key} = Интерфейс модуля недоступен до реализации серверного контракта.\n{slug}-{side}-connected = Серверный API подключён.\n"
    );

    [
        ("Cargo.toml", cargo),
        ("README.md", readme),
        ("CRATE_API.md", api),
        ("locales/en.ftl", translations_en),
        ("locales/ru.ftl", translations_ru),
        ("src/lib.rs", lib),
        ("src/core.rs", core),
        ("src/model.rs", model),
        ("src/i18n.rs", "rustok_ui_i18n::declare_module_i18n!();\n".to_string()),
        ("src/transport/mod.rs", transport),
        ("src/ui/mod.rs", format!("mod leptos;\npub use leptos::{page};\n")),
        ("src/ui/leptos.rs", leptos),
        ("src/tests/mod.rs", "mod contract;\n".to_string()),
        ("src/tests/contract.rs", tests),
    ]
    .into_iter()
    .map(|(file, source)| (PathBuf::from(side).join(file), source))
    .collect()
}


/// Preview the matching Next.js package boundary without mounting it in a host.
/// The actual entrypoint/route is intentionally not registered until backend
/// owner operations and a real API contract exist.
pub(crate) fn render_next_ui_templates(
    opts: &ScaffoldOptions,
    pascal: &str,
) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    if opts.ui.has_admin() {
        files.extend(render_next_package(opts, pascal, "admin"));
    }
    if opts.ui.has_storefront() {
        files.extend(render_next_package(opts, pascal, "storefront"));
    }
    files
}

fn render_next_package(
    opts: &ScaffoldOptions,
    pascal: &str,
    side: &str,
) -> Vec<(PathBuf, String)> {
    let kebab = opts.kebab_name();
    let (directory, package_name, component) = if side == "admin" {
        (
            PathBuf::from(format!("apps/next-admin/packages/{kebab}")),
            format!("@rustok/{kebab}-admin"),
            format!("{pascal}AdminPage"),
        )
    } else {
        (
            PathBuf::from(format!("apps/next-frontend/packages/rustok-{kebab}")),
            format!("@rustok/{kebab}-frontend"),
            format!("{pascal}StorefrontView"),
        )
    };

    let package_json = serde_json::to_string_pretty(&serde_json::json!({
        "name": package_name,
        "version": "0.1.0",
        "private": true,
        "main": "./src/index.tsx",
        "types": "./src/index.tsx",
        "dependencies": { "react": "^19.2.0" },
        "license": "BUSL-1.1",
    }))
    .expect("serializing static Next.js package metadata cannot fail");

    let title = serde_json::to_string(&opts.title)
        .expect("serializing a title as JSON string cannot fail");
    let description = serde_json::to_string(&opts.description)
        .expect("serializing a description as JSON string cannot fail");
    let page = format!(
        r#"// Draft Next.js {side} adapter. No route or module registry entry is installed.
// Replace this shell only after implementing the same owner read/write contract
// as the native/GraphQL Leptos adapters.
const title = {title};
const description = {description};
const unavailableCopy = {{
  en: "This module's UI is not available until its owner API is implemented.",
  ru: "Интерфейс модуля недоступен до реализации серверного контракта.",
}} as const;

export function {component}({{ locale = "en" }}: {{ locale?: string }}) {{
  const message = locale.toLowerCase().startsWith("ru")
    ? unavailableCopy.ru
    : unavailableCopy.en;
  return (
    <section className="space-y-3 p-6" aria-live="polite">
      <h1 className="text-2xl font-semibold">{{title}}</h1>
      <p className="text-muted-foreground">{{description}}</p>
      <p role="status">{{message}}</p>
    </section>
  );
}}
"#
    );
    let readme = format!(
        r#"# {package_name}

Draft Next.js {side} module adapter for {kebab}.

This is **not** a registered Next.js route or working GraphQL client.
The existing Blog Next.js packages are the reference for module ownership,
host composition, tenant/auth/locale context, and real service-backed reads.
Do not add the host package import or route until the owner API, GraphQL/REST
operations, and corresponding Leptos native/GraphQL adapters are functional.
"#
    );
    [
        ("package.json", package_json + "\n"),
        ("README.md", readme),
        ("src/index.tsx", page),
    ]
    .into_iter()
    .map(|(file, content)| (directory.join(file), content))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{render_next_ui_templates, render_ui_templates};
    use crate::scaffold::options::{ScaffoldOptions, ScaffoldProfile, ScaffoldUi};
    use std::path::PathBuf;

    fn options(ui: ScaffoldUi) -> ScaffoldOptions {
        ScaffoldOptions {
            profile: ScaffoldProfile::NativeModule,
            slug: "module_example".to_string(),
            title: "Module \"Example\"".to_string(),
            description: "New module UI without a live business owner yet".to_string(),
            category: "collaboration".to_string(),
            with_db: false,
            ui,
            dry_run: true,
            skip_server_wire: false,
            skip_validate: false,
        }
    }

    #[test]
    fn no_ui_generates_no_ui_packages() {
        assert!(render_ui_templates(&options(ScaffoldUi::None), "ModuleExample").is_empty());
    }

    #[test]
    fn admin_and_storefront_modes_preserve_distinct_leptos_adapters() {
        for (ui, expected_root) in [
            (ScaffoldUi::Admin, "admin"),
            (ScaffoldUi::Storefront, "storefront"),
        ] {
            let rendered = render_ui_templates(&options(ui), "ModuleExample");
            assert!(rendered.iter().all(|(p, _)| p.starts_with(expected_root)));
            for relative in [
                "src/core.rs",
                "src/model.rs",
                "src/ui/mod.rs",
                "src/ui/leptos.rs",
                "src/transport/mod.rs",
                "src/tests/contract.rs",
                "CRATE_API.md",
            ] {
                assert!(rendered.iter().any(|(path, _)| path == &PathBuf::from(expected_root).join(relative)),
                    "Missing {expected_root}/{relative}");
            }
            let lib = rendered.iter().find(|(path, _)| path.ends_with("src/lib.rs")).unwrap();
            assert!(!lib.1.contains("#[component]"));
            let leptos = rendered.iter().find(|(path, _)| path.ends_with("src/ui/leptos.rs")).unwrap();
            assert!(leptos.1.contains("#[component]"));
            assert!(leptos.1.contains("Module \\\"Example\\\""));
        }
    }

    #[test]
    fn dual_mode_is_only_a_request_and_does_not_invent_crud_or_transport() {
        let rendered = render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample");
        assert_eq!(rendered.len(), 28);
        let text = rendered.iter().map(|(_, source)| source.as_str()).collect::<Vec<_>>().join("\n");
        for forbidden in [
            "sample_rows", "rec-1", "tracing::info!", "create new module_example record",
            "can_edit: true", "can_delete: true", "DataGrid", "rustok-grid-leptos",
        ] {
            assert!(!text.contains(forbidden), "Generated fake UI pattern: {forbidden}");
        }
        assert!(text.contains("MissingOwnerContract"));
        assert!(text.contains("Err(MissingOwnerContract)"));
        assert!(text.contains("Next.js parity"));
    }

    #[test]
    fn next_drafts_match_blog_package_locations_without_fake_host_wiring() {
        let opts = options(ScaffoldUi::Dual);
        let rendered = render_next_ui_templates(&opts, "ModuleExample");
        assert_eq!(rendered.len(), 6);
        assert!(rendered.iter().any(|(path, _)| path ==
            &PathBuf::from("apps/next-admin/packages/module-example/src/index.tsx")));
        assert!(rendered.iter().any(|(path, _)| path ==
            &PathBuf::from("apps/next-frontend/packages/rustok-module-example/src/index.tsx")));
        for (path, contents) in &rendered {
            if path.ends_with("package.json") {
                let meta: serde_json::Value = serde_json::from_str(contents).unwrap();
                assert!(meta["name"].as_str().unwrap().starts_with("@rustok/"));
            }
            assert!(!contents.contains("registerAdminModule("));
            assert!(!contents.contains("registerStorefrontModule("));
            assert!(!contents.contains("fetch("));
            assert!(!contents.contains("graphql("));
            if path.ends_with("src/index.tsx") {
                assert!(contents.contains("locale?: string"));
                assert!(contents.contains("Интерфейс модуля недоступен"));
            }
        }
        assert!(render_next_ui_templates(&options(ScaffoldUi::None), "ModuleExample").is_empty());
    }

    #[test]
    fn generated_cargo_and_i18n_keys_are_well_formed() {
        let rendered = render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample");
        for (path, source) in rendered {
            if path.ends_with("Cargo.toml") {
                let cargo: toml::Value = toml::from_str(&source).unwrap();
                let deps = &cargo["dependencies"];
                assert!(deps.get("rustok-graphql").is_none());
                assert!(deps.get("leptos").is_some());
                assert!(deps.get("sea-orm").is_none());
            }
            if path.ends_with("locales/en.ftl") {
                assert!(source.contains("not available"));
            }
        }
    }
}
