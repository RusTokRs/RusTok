use super::options::{ScaffoldOptions, ScaffoldUi};
use anyhow::Result;
use std::path::PathBuf;

/// Creates Leptos FFA *development skeletons* that compile independently
/// without mounting routes or claiming native/GraphQL business operations.
/// Domain-specific services, transport calls and UI flows are authored later
/// by each module owner; missing operations remain explicit errors.
/// Pure renderer used for both dry-run and writable Leptos skeleton creation.
/// No host mount or live [provides.*_ui] metadata is produced.
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
rustok-ui-transport.workspace = true
"#
    );

    let readme = format!(
        r#"# {ui_crate}

Draft Leptos {side} UI adapter for the {slug} native module.

## Responsibility
- Keep UI rendering and bindings in src/ui/leptos.rs.
- Keep pure, framework-independent presentation state in src/core/mod.rs.
- Keep all future backend calls behind src/transport/mod.rs.
- Do not claim that CRUD, GraphQL, or native server transport exists.

## Current state
This is an **unmounted development package**, not a registered functional route.
There is no owner operation to read or mutate. The transport facade fails
closed until a genuine backend API and its transport-specific adapters exist.
No business data, permissions, tenant context or mutation outcomes are invented.

The scaffold command writes this development package into the Cargo workspace
without mounting it in a Leptos host or advertising a live UI in rustok-module.toml.
After the module owner implements business operations, complete and verify the
native and GraphQL adapters, then add Leptos host composition and live UI
manifest metadata in a separate feature change before promoting readiness.
Next.js applications own separate classic packages in their hosts; they are
not generated as FFA adapters. Before promoting a surface to live, add its
`next_package` under the corresponding Next host, register it in the host
module registry and import it from the declared route. `cargo xtask module lint`
checks that structural package/route wiring.

## Known Limitations / Pending Implementation
- No owner business operation or usable native/GraphQL adapter exists yet.
- This Leptos package has no host route and advertises no live UI capability.
- Client parity and real transport behavior remain pending owner implementation.
- Track subsequent implementation in the module's docs/implementation-plan.md.

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
- Pure readiness model in src/core/mod.rs; render/bind in src/ui/leptos.rs.

## Platform Capability Bindings
- Transport selection uses rustok-ui-transport; both owner-specific adapters explicitly reject calls until implemented.

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
- MissingOwnerContract is a typed unavailable result for each adapter, not an empty success.
"#
    );

    let model = r#"//! Framework- and transport-independent owner-boundary error type.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingOwnerContract;

impl std::fmt::Display for MissingOwnerContract {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("owner API not implemented; FFA transport unavailable")
    }
}

impl std::error::Error for MissingOwnerContract {}
"#.to_string();

    let core = r#"//! Pure presentation policy: reusable by a future Dioxus adapter.
//! Do not put Leptos signals, framework resources or GraphQL requests here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCapabilityState {
    Unavailable,
}

pub const fn owner_capability_state() -> UiCapabilityState {
    UiCapabilityState::Unavailable
}
"#.to_string();

    // This is a real FFA selection facade through the existing platform
    // transport owner. Each adapter explicitly reports missing business logic:
    // no invented GraphQL document, server function or fake successful result.
    let transport = format!(
        r#"//! Leptos/Dioxus-neutral owner transport facade.
//! Neither adapter implements a business operation until the owner API exists.

mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::{{execute_selected_transport, UiTransportError, UiTransportPath}};

pub type ApiError = UiTransportError;

pub fn selected_transport_path() -> UiTransportPath {{
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {{
        UiTransportPath::NativeServer
    }}
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {{
        UiTransportPath::Graphql
    }}
}}

/// Explicit path injection makes the facade usable by future Dioxus bindings
/// without coupling the pure model or core to a Leptos build profile.
pub async fn fetch_owner_capability_for(
    path: UiTransportPath,
) -> Result<(), ApiError> {{
    execute_selected_transport(
        "{slug}/{side}",
        path,
        native_server_adapter::fetch_owner_capability,
        graphql_adapter::fetch_owner_capability,
    )
    .await
}}

pub async fn fetch_owner_capability() -> Result<(), ApiError> {{
    fetch_owner_capability_for(selected_transport_path()).await
}}
"#
    );
    let adapter_error = r#"//! Draft adapter: no owner service or GraphQL schema is declared.
//! Never substitute a simulated record or Ok(()) for missing API behavior.
use crate::model::MissingOwnerContract;

pub(super) async fn fetch_owner_capability() -> Result<(), MissingOwnerContract> {
    Err(MissingOwnerContract)
}
"#.to_string();

    let leptos = format!(
        r#"use leptos::prelude::*;
use crate::{{core, i18n::t}};

/// Render-only adapter. Future Dioxus UI binds the same core/transport.
#[component]
pub fn {page}() -> impl IntoView {{
    let _state = core::owner_capability_state();
    view! {{
        <section class="p-6 space-y-3" aria-live="polite">
            <h1 class="text-2xl font-semibold">{title_rust}</h1>
            <p class="text-muted-foreground">{description_rust}</p>
            <p role="status">
                {{t(None, "{side_key}", "This module's UI is not available until its owner API is implemented.")}}
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

    let tests = r#"#[test]
fn pure_presentation_state_does_not_fake_a_connected_owner() {
    assert_eq!(
        crate::core::owner_capability_state(),
        crate::core::UiCapabilityState::Unavailable
    );
    assert_eq!(
        crate::model::MissingOwnerContract.to_string(),
        "owner API not implemented; FFA transport unavailable"
    );
}

#[test]
fn both_paths_have_an_explicit_selection_contract() {
    use rustok_ui_transport::UiTransportPath;
    assert_eq!(UiTransportPath::NativeServer.as_str(), "native_server");
    assert_eq!(UiTransportPath::Graphql.as_str(), "graphql");
}

#[test]
fn documented_ui_is_not_a_fake_business_api() {
    let docs = include_str!("../../CRATE_API.md");
    for section in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(docs.contains(section), "Missing UI contract: {section}");
    }
    assert!(docs.contains("No CRUD or owner business operation is available"));
}
"#.to_string();

    let translations_en = format!(
        "{side_key} = This module's UI is not available until its owner API is implemented.\n"
    );
    let translations_ru = format!(
        "{side_key} = Интерфейс модуля недоступен до реализации серверного контракта.\n"
    );

    [
        ("Cargo.toml", cargo),
        ("README.md", readme),
        ("CRATE_API.md", api),
        ("locales/en.ftl", translations_en),
        ("locales/ru.ftl", translations_ru),
        ("src/lib.rs", lib),
        ("src/core/mod.rs", core),
        ("src/model.rs", model),
        ("src/i18n.rs", "rustok_ui_i18n::declare_module_i18n!();\n".to_string()),
        ("src/transport/mod.rs", transport),
        ("src/transport/native_server_adapter.rs", adapter_error.clone()),
        ("src/transport/graphql_adapter.rs", adapter_error),
        ("src/ui/mod.rs", format!("mod leptos;\npub use leptos::{page};\n")),
        ("src/ui/leptos.rs", leptos),
        ("src/tests/mod.rs", "mod contract;\n".to_string()),
        ("src/tests/contract.rs", tests),
    ]
    .into_iter()
    .map(|(file, source)| (PathBuf::from(side).join(file), source))
    .collect()
}


#[cfg(test)]
mod tests {
    use super::render_ui_templates;
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

    fn content<'a>(files: &'a [(PathBuf, String)], path: &str) -> &'a str {
        &files.iter().find(|(entry, _)| entry == &PathBuf::from(path))
            .unwrap_or_else(|| panic!("missing {path}")).1
    }

    #[test]
    fn scaffolded_ui_is_documented_as_unmounted_until_owner_wiring_exists() {
        let files = render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample");
        for side in ["admin", "storefront"] {
            let docs = content(&files, &format!("{side}/README.md"));
            assert!(docs.contains("unmounted development package"));
            assert!(docs.contains("Cargo workspace"));
            assert!(docs.contains("native and GraphQL adapters"));
        }
    }

    #[test]
    fn no_ui_emits_no_packages() {
        assert!(render_ui_templates(&options(ScaffoldUi::None), "ModuleExample").is_empty());
    }

    #[test]
    fn each_ui_mode_has_pure_core_and_separate_leptos_adapter() {
        for (ui, side) in [
            (ScaffoldUi::Admin, "admin"),
            (ScaffoldUi::Storefront, "storefront"),
        ] {
            let files = render_ui_templates(&options(ui), "ModuleExample");
            assert!(files.iter().all(|(path, _)| path.starts_with(side)));
            assert_eq!(files.len(), 16);
            let root = format!("{side}/");
            let lib = content(&files, &format!("{root}src/lib.rs"));
            assert!(!lib.contains("#[component]"));
            assert!(!lib.contains("pub mod transport"));
            assert!(lib.contains("mod core;"));
            let core = content(&files, &format!("{root}src/core/mod.rs"));
            assert!(!core.contains("leptos::"));
            assert!(!core.contains("dioxus::"));
            assert!(!core.contains("rustok_graphql::"));
            let leptos = content(&files, &format!("{root}src/ui/leptos.rs"));
            assert!(leptos.contains("#[component]"));
            assert!(leptos.contains("Module \\\"Example\\\""));
            assert!(!leptos.contains("native_server_adapter::"));
            assert!(!leptos.contains("graphql_adapter::"));
        }
    }

    #[test]
    fn dual_path_transport_is_real_selection_but_unimplemented_business_io() {
        let files = render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample");
        assert_eq!(files.len(), 32);
        for side in ["admin", "storefront"] {
            let root = format!("{side}/");
            let facade = content(&files, &format!("{root}src/transport/mod.rs"));
            assert!(facade.contains("execute_selected_transport("));
            assert!(facade.contains("UiTransportPath::NativeServer"));
            assert!(facade.contains("UiTransportPath::Graphql"));
            assert!(facade.contains("pub async fn fetch_owner_capability_for("));
            for path in ["native_server_adapter.rs", "graphql_adapter.rs"] {
                let adapter = content(&files, &format!("{root}src/transport/{path}"));
                assert!(adapter.contains("Err(MissingOwnerContract)"));
                assert!(!adapter.contains("Ok(())"));
            }
            let cargo = content(&files, &format!("{root}Cargo.toml"));
            let parsed: toml::Value = toml::from_str(cargo).unwrap();
            assert!(parsed["dependencies"].get("rustok-ui-transport").is_some());
            assert!(parsed["dependencies"].get("rustok-graphql").is_none());
            assert!(parsed["dependencies"].get("sea-orm").is_none());
        }
    }

    #[test]
    fn no_nextjs_or_fake_crud_are_generated_by_ffa() {
        let files = render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample");
        let all = files.iter().map(|(p, s)| format!("{}:\n{s}", p.display()))
            .collect::<Vec<_>>().join("\n");
        assert!(!all.contains("apps/next-"));
        for forbidden in [
            "sample_rows", "rec-1", "tracing::info!", "create new module_example record",
            "can_edit: true", "can_delete: true", "DataGrid", "rustok-grid-leptos",
            "registerAdminModule(", "registerStorefrontModule(", "GraphqlRequest::new(",
        ] {
            assert!(!all.contains(forbidden), "fake UI pattern: {forbidden}");
        }
    }

    #[test]
    fn future_dioxus_can_share_pure_core_model_and_transport() {
        let files = render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample");
        for (path, source) in &files {
            let pure = path.ends_with("src/model.rs")
                || path.ends_with("src/core/mod.rs")
                || path.to_string_lossy().contains("/src/transport/");
            if pure {
                for framework in ["leptos::", "dioxus::", "leptos_ui::"] {
                    assert!(!source.contains(framework),
                        "{framework} must not leak into {}", path.display());
                }
            }
            assert!(!path.ends_with("src/ui/dioxus.rs"),
                "Dioxus is a future sibling adapter, not a replacement yet");
        }
        assert!(files.iter().any(|(p, _)| p.ends_with("src/ui/leptos.rs")));
        assert!(files.iter().any(|(p, _)| p.ends_with("src/transport/mod.rs")));
    }

    #[test]
    fn result_does_not_claim_transport_or_business_readiness() {
        for (_, source) in render_ui_templates(&options(ScaffoldUi::Dual), "ModuleExample") {
            assert!(!source.contains("Owner API is connected."));
        }
    }
}
