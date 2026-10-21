use super::options::ScaffoldOptions;
use super::write_file_content;
use anyhow::Result;
use std::path::Path;

pub(crate) fn generate_ui(
    opts: &ScaffoldOptions,
    module_dir: &Path,
    pascal: &str,
) -> Result<()> {
    if opts.ui.has_admin() {
        generate_admin_ui(opts, module_dir, pascal)?;
    }
    if opts.ui.has_storefront() {
        generate_storefront_ui(opts, module_dir, pascal)?;
    }
    Ok(())
}

fn generate_admin_ui(
    opts: &ScaffoldOptions,
    module_dir: &Path,
    pascal: &str,
) -> Result<()> {
    let slug = &opts.slug;
    let title = &opts.title;
    let desc = &opts.description;
    let crate_name = format!("rustok-{slug}");
    let admin_crate_name = format!("{crate_name}-admin");
    let admin_dir = module_dir.join("admin");

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{admin_crate_name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Admin FFA package for {title}"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
crate-type = ["rlib"]

[features]
default = []
hydrate = ["leptos/hydrate"]
ssr = [
  "leptos/ssr",
  "dep:leptos_axum",
  "dep:rustok-core",
  "dep:{crate_name}",
  "rustok-api/server",
]

[dependencies]
rustok-ui-core.workspace = true
rustok-ui-transport.workspace = true
rustok-ui-i18n.workspace = true
leptos-ui.workspace = true
leptos.workspace = true
leptos_axum = {{ workspace = true, optional = true }}
leptos-auth.workspace = true
rustok-graphql.workspace = true
leptos-ui-routing.workspace = true
leptos_router.workspace = true
rustok-api = {{ workspace = true, default-features = false }}

rustok-core = {{ workspace = true, optional = true }}
{crate_name} = {{ path = "..", default-features = false, optional = true }}
chrono = {{ workspace = true, features = ["serde"] }}
thiserror = {{ workspace = true }}
serde = {{ workspace = true, features = ["derive"] }}
serde_json.workspace = true
uuid = {{ workspace = true, features = ["serde"] }}
tracing.workspace = true

[package.metadata.cargo-udeps.ignore]
normal = ["leptos-ui-routing", "leptos_axum", "{crate_name}", "rustok-core"]
"#
    );
    write_file_content(&admin_dir.join("Cargo.toml"), &cargo_toml, opts.dry_run)?;

    // README.md
    let readme = format!(
        r#"# {admin_crate_name}

Admin UI package for `{slug}`, following Fluid Frontend Architecture (FFA).

## Responsibilities
- Pure Rust view-models and validation in `core`
- Decoupled backend communication via `transport`
- Leptos UI components and templates in `ui`
- Localization catalog in `locales/`
"#
    );
    write_file_content(&admin_dir.join("README.md"), &readme, opts.dry_run)?;

    // locales/en.ftl
    let en_ftl = format!(
        r#"{slug}-title = {title}
{slug}-description = Manage {title} configuration and operational workflows
{slug}-empty = No records found
{slug}-save = Save
{slug}-cancel = Cancel
{slug}-success = Operation completed successfully
{slug}-error = An error occurred
"#
    );
    write_file_content(
        &admin_dir.join("locales").join("en.ftl"),
        &en_ftl,
        opts.dry_run,
    )?;

    // locales/ru.ftl
    let ru_ftl = format!(
        r#"{slug}-title = {title}
{slug}-description = Управление настройками и рабочими процессами {title}
{slug}-empty = Записи не найдены
{slug}-save = Сохранить
{slug}-cancel = Отмена
{slug}-success = Операция успешно выполнена
{slug}-error = Произошла ошибка
"#
    );
    write_file_content(
        &admin_dir.join("locales").join("ru.ftl"),
        &ru_ftl,
        opts.dry_run,
    )?;

    // src/i18n.rs
    let i18n_rs = "rustok_ui_i18n::declare_module_i18n!();\n";
    write_file_content(
        &admin_dir.join("src").join("i18n.rs"),
        i18n_rs,
        opts.dry_run,
    )?;

    // src/core.rs
    let core_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Rust view-models and input validation for {title} admin UI.
//!
//! INVARIANT (FFA): This module MUST NOT import any UI framework types.
//! All validation and state transition logic must be testable with pure #[test].

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct {pascal}AdminViewModel {{
    pub title: String,
    pub is_valid: bool,
}}

impl {pascal}AdminViewModel {{
    pub fn new(title: String) -> Self {{
        let is_valid = !title.trim().is_empty();
        Self {{ title, is_valid }}
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_admin_view_model_validation() {{
        let valid = {pascal}AdminViewModel::new("Test".to_string());
        assert!(valid.is_valid);

        let invalid = {pascal}AdminViewModel::new("  ".to_string());
        assert!(!invalid.is_valid);
    }}
}}
"#
    );
    write_file_content(
        &admin_dir.join("src").join("core.rs"),
        &core_rs,
        opts.dry_run,
    )?;

    // src/transport.rs
    let transport_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transport facade connecting UI to backend capabilities.
//! UI components must NEVER call raw endpoints directly.

#[derive(Clone, Debug, Default)]
pub struct {pascal}AdminTransport;

impl {pascal}AdminTransport {{
    pub fn new() -> Self {{
        Self
    }}
}}
"#
    );
    write_file_content(
        &admin_dir.join("src").join("transport.rs"),
        &transport_rs,
        opts.dry_run,
    )?;

    // src/ui.rs
    let ui_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! UI components for {title} admin.

use leptos::*;

#[component]
pub fn {pascal}AdminPage() -> impl IntoView {{
    view! {{
        <div class="p-6 space-y-4">
            <h1 class="text-2xl font-bold tracking-tight">"{title}"</h1>
            <p class="text-muted-foreground">"{desc}"</p>
        </div>
    }}
}}
"#
    );
    write_file_content(&admin_dir.join("src").join("ui.rs"), &ui_rs, opts.dry_run)?;

    // src/lib.rs
    let lib_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::*;
pub use transport::*;
pub use ui::{pascal}AdminPage;
"#
    );
    write_file_content(
        &admin_dir.join("src").join("lib.rs"),
        &lib_rs,
        opts.dry_run,
    )?;

    Ok(())
}

fn generate_storefront_ui(
    opts: &ScaffoldOptions,
    module_dir: &Path,
    pascal: &str,
) -> Result<()> {
    let slug = &opts.slug;
    let title = &opts.title;
    let desc = &opts.description;
    let crate_name = format!("rustok-{slug}");
    let sf_crate_name = format!("{crate_name}-storefront");
    let sf_dir = module_dir.join("storefront");

    // Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{sf_crate_name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Storefront FFA package for {title}"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
crate-type = ["rlib"]

[features]
default = []
hydrate = ["leptos/hydrate"]
ssr = [
  "leptos/ssr",
  "dep:leptos_axum",
  "dep:rustok-core",
  "dep:{crate_name}",
  "rustok-api/server",
]

[dependencies]
rustok-ui-core.workspace = true
rustok-ui-transport.workspace = true
rustok-ui-i18n.workspace = true
leptos-ui.workspace = true
leptos.workspace = true
leptos_axum = {{ workspace = true, optional = true }}
leptos-auth.workspace = true
rustok-graphql.workspace = true
leptos-ui-routing.workspace = true
leptos_router.workspace = true
rustok-api = {{ workspace = true, default-features = false }}

rustok-core = {{ workspace = true, optional = true }}
{crate_name} = {{ path = "..", default-features = false, optional = true }}
chrono = {{ workspace = true, features = ["serde"] }}
thiserror = {{ workspace = true }}
serde = {{ workspace = true, features = ["derive"] }}
serde_json.workspace = true
uuid = {{ workspace = true, features = ["serde"] }}
tracing.workspace = true

[package.metadata.cargo-udeps.ignore]
normal = ["leptos-ui-routing", "leptos_axum", "{crate_name}", "rustok-core"]
"#
    );
    write_file_content(&sf_dir.join("Cargo.toml"), &cargo_toml, opts.dry_run)?;

    // README.md
    let readme = format!(
        r#"# {sf_crate_name}

Storefront UI package for `{slug}`, following Fluid Frontend Architecture (FFA).

## Responsibilities
- Pure Rust view-models in `core`
- Decoupled backend communication via `transport`
- Leptos UI components and templates in `ui`
- Localization catalog in `locales/`
"#
    );
    write_file_content(&sf_dir.join("README.md"), &readme, opts.dry_run)?;

    // locales/en.ftl
    let en_ftl = format!(
        r#"{slug}-storefront-title = {title}
{slug}-storefront-description = {desc}
{slug}-storefront-empty = No items available
"#
    );
    write_file_content(
        &sf_dir.join("locales").join("en.ftl"),
        &en_ftl,
        opts.dry_run,
    )?;

    // locales/ru.ftl
    let ru_ftl = format!(
        r#"{slug}-storefront-title = {title}
{slug}-storefront-description = {desc}
{slug}-storefront-empty = Нет доступных элементов
"#
    );
    write_file_content(
        &sf_dir.join("locales").join("ru.ftl"),
        &ru_ftl,
        opts.dry_run,
    )?;

    // src/i18n.rs
    let i18n_rs = "rustok_ui_i18n::declare_module_i18n!();\n";
    write_file_content(&sf_dir.join("src").join("i18n.rs"), i18n_rs, opts.dry_run)?;

    // src/core.rs
    let core_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Rust view-models and input validation for {title} storefront UI.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct {pascal}StorefrontViewModel {{
    pub title: String,
}}

impl {pascal}StorefrontViewModel {{
    pub fn new(title: String) -> Self {{
        Self {{ title }}
    }}
}}
"#
    );
    write_file_content(&sf_dir.join("src").join("core.rs"), &core_rs, opts.dry_run)?;

    // src/transport.rs
    let transport_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transport facade for {title} storefront.

#[derive(Clone, Debug, Default)]
pub struct {pascal}StorefrontTransport;

impl {pascal}StorefrontTransport {{
    pub fn new() -> Self {{
        Self
    }}
}}
"#
    );
    write_file_content(
        &sf_dir.join("src").join("transport.rs"),
        &transport_rs,
        opts.dry_run,
    )?;

    // src/ui.rs
    let ui_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use leptos::*;

#[component]
pub fn {pascal}StorefrontView() -> impl IntoView {{
    view! {{
        <div class="rustok-{slug}-storefront-widget p-4">
            <h2 class="text-xl font-semibold">"{title}"</h2>
            <p class="text-muted-foreground">"{desc}"</p>
        </div>
    }}
}}
"#
    );
    write_file_content(&sf_dir.join("src").join("ui.rs"), &ui_rs, opts.dry_run)?;

    // src/lib.rs
    let lib_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

pub mod core;
pub mod i18n;
pub mod transport;
pub mod ui;

pub use core::*;
pub use transport::*;
pub use ui::{pascal}StorefrontView;
"#
    );
    write_file_content(&sf_dir.join("src").join("lib.rs"), &lib_rs, opts.dry_run)?;

    Ok(())
}
