use super::*;
use crate::manifest_validation::module_validate_command;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleScaffoldUi {
    None,
    Admin,
    Storefront,
    Dual,
}

impl ModuleScaffoldUi {
    pub fn has_admin(&self) -> bool {
        matches!(self, Self::Admin | Self::Dual)
    }

    pub fn has_storefront(&self) -> bool {
        matches!(self, Self::Storefront | Self::Dual)
    }

    pub fn classification_str(&self) -> &'static str {
        match self {
            Self::None => "no_ui",
            Self::Admin => "admin_only",
            Self::Storefront => "storefront_only",
            Self::Dual => "dual_surface",
        }
    }
}

pub struct ModuleScaffoldOptions {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub with_db: bool,
    pub ui: ModuleScaffoldUi,
    pub dry_run: bool,
    pub skip_server_wire: bool,
    pub skip_validate: bool,
}

pub(crate) fn module_scaffold_command(args: &[String]) -> Result<()> {
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_module_scaffold_usage();
        if args.is_empty() {
            anyhow::bail!("Missing module slug for scaffolding");
        }
        return Ok(());
    }

    let slug = args[0].trim().to_ascii_lowercase();
    if !is_valid_module_slug(&slug) {
        anyhow::bail!(
            "Invalid module slug '{slug}'. Must contain only lowercase letters, digits, and hyphens (e.g. 'calls', 'party', 'digital-wallet')."
        );
    }

    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let skip_server_wire = args.iter().any(|arg| arg == "--skip-server-wire");
    let skip_validate = args.iter().any(|arg| arg == "--skip-validate");

    let title = cli_option_value(args, "--title")
        .or_else(|| cli_option_value(args, "--name"))
        .unwrap_or_else(|| default_title_from_slug(&slug));

    let description = cli_option_value(args, "--description")
        .unwrap_or_else(|| format!("{title} capability module for RusToK"));

    let category = cli_option_value(args, "--category")
        .unwrap_or_else(|| "collaboration".to_string());

    let no_db = args.iter().any(|arg| arg == "--no-db");
    let with_db = !no_db;

    let ui = if args.iter().any(|arg| arg == "--no-ui") {
        ModuleScaffoldUi::None
    } else if let Some(ui_val) = cli_option_value(args, "--ui") {
        match ui_val.trim().to_ascii_lowercase().as_str() {
            "none" | "no-ui" => ModuleScaffoldUi::None,
            "admin" | "admin-only" => ModuleScaffoldUi::Admin,
            "storefront" | "storefront-only" => ModuleScaffoldUi::Storefront,
            "dual" | "dual-surface" | "both" => ModuleScaffoldUi::Dual,
            other => anyhow::bail!(
                "Invalid --ui option '{other}'. Expected: 'dual', 'admin', 'storefront', or 'none'"
            ),
        }
    } else if args.iter().any(|arg| arg == "--with-ui") {
        ModuleScaffoldUi::Dual
    } else {
        // Default to dual surface for SuperApp / modern modules
        ModuleScaffoldUi::Dual
    };

    let options = ModuleScaffoldOptions {
        slug,
        title,
        description,
        category,
        with_db,
        ui,
        dry_run,
        skip_server_wire,
        skip_validate,
    };

    scaffold_module(&options)?;

    Ok(())
}

pub(crate) fn print_module_scaffold_usage() {
    println!("Usage:");
    println!(
        "  cargo xtask module new <slug> [OPTIONS]\n  cargo xtask module scaffold <slug> [OPTIONS]"
    );
    println!();
    println!("Options:");
    println!("  --title <title>          Human-readable title (defaults to Title Case of slug)");
    println!("  --description <desc>     Module description for documentation and manifests");
    println!("  --category <cat>         Marketplace category (default: 'collaboration')");
    println!("  --no-db                  Omit SeaORM persistence entities and migrations");
    println!("  --ui <mode>              UI classification: dual, admin, storefront, or none (default: dual)");
    println!("  --with-ui                Alias for --ui dual");
    println!("  --no-ui                  Alias for --ui none (headless module)");
    println!("  --skip-server-wire       Do not register feature flags in apps/server/Cargo.toml");
    println!("  --skip-validate          Skip post-scaffold contract validation");
    println!("  --dry-run                Preview generated files and updates without writing to disk");
    println!();
    println!("Examples:");
    println!("  cargo xtask module new calls --title \"WebRTC Calls & Conferencing\" --category collaboration");
    println!("  cargo xtask module new party --title \"Counterparty & Legal Entities\" --category crm");
    println!("  cargo xtask module new billing --title \"B2B Invoices & Settlements\" --category erp");
}

fn is_valid_module_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-')
        && !slug.ends_with('-')
}

fn default_title_from_slug(slug: &str) -> String {
    slug.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn scaffold_module(options: &ModuleScaffoldOptions) -> Result<()> {
    let root = workspace_root();
    let slug = &options.slug;
    let pascal_name = to_pascal_case(slug);
    let snake_name = slug.replace('-', "_");
    let crate_name = format!("rustok-{slug}");
    let module_dir = root.join("crates").join("modules").join(&crate_name);

    if module_dir.exists() && !options.dry_run {
        anyhow::bail!(
            "Module crate directory already exists at {}",
            module_dir.display()
        );
    }

    println!("Scaffolding module '{}' ({})...", slug, crate_name);
    if options.dry_run {
        println!("  [DRY-RUN] Target directory: {}", module_dir.display());
    }

    // 1. Generate core backend files
    generate_backend_crate(options, &module_dir, &pascal_name, &snake_name)?;

    // 2. Generate UI packages (FFA: core, transport, ui, i18n)
    if options.ui.has_admin() {
        generate_admin_ui_crate(options, &module_dir, &pascal_name)?;
    }
    if options.ui.has_storefront() {
        generate_storefront_ui_crate(options, &module_dir, &pascal_name)?;
    }

    // 3. Update manifests and global registration
    if !options.dry_run {
        register_in_workspace_cargo_toml(&root, &crate_name, options.ui)?;
        register_in_modules_toml(&root, slug, &crate_name)?;
        register_in_registry_md(&root, slug, &crate_name, &options.description)?;
        register_in_docs_index_md(&root, &crate_name, options.ui.has_admin())?;

        if options.ui.has_admin() || options.ui.has_storefront() {
            register_in_ui_packages_index_md(
                &root,
                &crate_name,
                options.ui.has_admin(),
                options.ui.has_storefront(),
            )?;
        }

        if !options.skip_server_wire {
            register_in_server_cargo_toml(&root, slug, &crate_name)?;
            register_in_distribution_cargo_toml(&root, slug, &crate_name)?;
            register_in_distribution_lib_rs(&root, slug, &pascal_name, &snake_name)?;
        }

        println!("  Registration complete across workspace manifests and docs.");

        // 4. Run post-scaffold validation if not skipped
        if !options.skip_validate {
            println!("\nVerifying module publish-readiness contracts for '{slug}'...");
            match module_validate_command(std::slice::from_ref(slug)) {
                Ok(_) => {
                    println!("\n  SUCCESS: Module '{slug}' created and verified!");
                    println!("  Next steps:");
                    println!("    cargo check -p {crate_name}");
                    if options.ui.has_admin() {
                        println!("    cargo check -p {crate_name}-admin");
                    }
                    if options.ui.has_storefront() {
                        println!("    cargo check -p {crate_name}-storefront");
                    }
                }
                Err(err) => {
                    println!("\n  WARNING: Module files created, but verification reported:");
                    println!("    {err:#}");
                }
            }
        }
    } else {
        println!("\n  [DRY-RUN] Completed preview without disk modifications.");
    }

    Ok(())
}

fn write_file_content(path: &Path, content: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("  [CREATE] {}", path.display());
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    fs::write(path, content)
        .with_context(|| format!("Failed to write file {}", path.display()))?;
    println!("  Created {}", path.display());
    Ok(())
}

fn generate_backend_crate(
    opts: &ModuleScaffoldOptions,
    module_dir: &Path,
    pascal: &str,
    snake: &str,
) -> Result<()> {
    let slug = &opts.slug;
    let title = &opts.title;
    let desc = &opts.description;
    let crate_name = format!("rustok-{slug}");

    // Cargo.toml
    let mut cargo_toml = format!(
        r#"[package]
name = "{crate_name}"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "{desc}"
repository.workspace = true
readme = "README.md"

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[dependencies]
async-trait.workspace = true
chrono = {{ workspace = true, features = ["serde"] }}
rustok-core.workspace = true
rustok-outbox.workspace = true
serde = {{ workspace = true, features = ["derive"] }}
serde_json.workspace = true
thiserror.workspace = true
tokio = {{ workspace = true, features = ["sync"] }}
tracing.workspace = true
uuid = {{ workspace = true, features = ["serde", "v4"] }}
"#
    );

    if opts.with_db {
        cargo_toml.push_str(
            r#"sea-orm = { workspace = true }
sea-orm-migration = { workspace = true }
"#,
        );
    }

    cargo_toml.push_str(
        r#"
[dev-dependencies]
tokio = { workspace = true, features = ["full", "macros"] }

[features]
default = []
"#,
    );

    write_file_content(&module_dir.join("Cargo.toml"), &cargo_toml, opts.dry_run)?;

    // rustok-module.toml
    let ui_classification = opts.ui.classification_str();
    let mut module_toml = format!(
        r#"[module]
slug = "{slug}"
name = "{title}"
version = "0.1.0"
description = "{desc}"
ownership = "first_party"
trust_level = "verified"
ui_classification = "{ui_classification}"
recommended_admin_surfaces = ["leptos-admin"]

[marketplace]
category = "{}"
tags = ["{slug}"]

[dependencies]
outbox = {{ version_req = ">=0.1.0" }}

[crate]
entry_type = "{pascal}Module"
"#,
        opts.category
    );

    if opts.ui.has_admin() {
        module_toml.push_str(&format!(
            r#"
[provides.admin_ui]
leptos_crate = "{crate_name}-admin"
route_segment = "{slug}"
nav_label = "{title}"

[provides.admin_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "admin/locales"
"#
        ));
    }

    if opts.ui.has_storefront() {
        module_toml.push_str(&format!(
            r#"
[provides.storefront_ui]
leptos_crate = "{crate_name}-storefront"
slot = "home_after_catalog"
route_segment = "{slug}"
page_title = "{title}"

[provides.storefront_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "storefront/locales"
"#
        ));
    }

    write_file_content(
        &module_dir.join("rustok-module.toml"),
        &module_toml,
        opts.dry_run,
    )?;

    // README.md
    let readme = format!(
        r#"# {crate_name}

{desc}

## Purpose
Provides sovereign {title} domain capability for RusToK.

## Responsibility Zone
- Domain entities, state invariants and business rules for {slug}
- Storage and transactional boundaries for {slug} operations

## Entry points
- `{pascal}Module` implementing `RusToKModule`
- Domain services in `services`

## Interactions
- Emits transactional domain events via `rustok-outbox`
- Authorized via `rustok-rbac` permissions
"#
    );
    write_file_content(&module_dir.join("README.md"), &readme, opts.dry_run)?;

    // CRATE_API.md
    let crate_api = format!(
        r#"# {crate_name} API Specification

## Public Types and Re-exports
- `{pascal}Module`: platform lifecycle descriptor implementing `RusToKModule`.
- `{pascal}Error`: domain error catalog for {slug}.
- `{pascal}Result<T>`: standard result type alias.

## Services
- Domain and application use cases exported from `services`.
"#
    );
    write_file_content(&module_dir.join("CRATE_API.md"), &crate_api, opts.dry_run)?;

    // docs/README.md
    let docs_readme = format!(
        r#"# {title} Module Architecture Contract

## Purpose
The `{slug}` module provides canonical {title} domain functionality for RusToK.

## Responsibility Zone
- Sovereign domain model and persistence for {slug}
- Typed events published to the transactional outbox
- RBAC permissions guard: `{slug}:read`, `{slug}:write`, `{slug}:admin`

## Integration
- Core runtime registration in `apps/server` via `RusToKModule`
- Event bus subscription via `rustok-outbox`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p {crate_name} --lib`
- Module contract verification: `cargo xtask module validate {slug}`

## Related Documentation
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)
"#
    );
    write_file_content(
        &module_dir.join("docs").join("README.md"),
        &docs_readme,
        opts.dry_run,
    )?;

    // docs/implementation-plan.md
    let plan = format!(
        r#"# {title} Module Implementation Plan

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `{crate_name}`
- Admin UI crate: `{crate_name}-admin`
- Storefront UI crate: `{crate_name}-storefront`

## Implementation Phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p {crate_name}`
- `cargo test -p {crate_name} --lib`
- `cargo xtask module validate {slug}`
"#
    );
    write_file_content(
        &module_dir.join("docs").join("implementation-plan.md"),
        &plan,
        opts.dry_run,
    )?;

    // src/lib.rs
    let mut lib_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of {title} capabilities in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

#![deny(clippy::float_arithmetic)]

pub mod domain;
pub mod dto;
pub mod error;
pub mod module;
pub mod services;
"#
    );

    if opts.with_db {
        lib_rs.push_str(
            r#"
pub mod entities;
pub mod migrations;
"#,
        );
    }

    lib_rs.push_str(&format!(
        r#"
pub use error::{{{pascal}Error, {pascal}Result}};
pub use module::{pascal}Module;
"#
    ));

    write_file_content(
        &module_dir.join("src").join("lib.rs"),
        &lib_rs,
        opts.dry_run,
    )?;

    // src/error.rs
    let error_rs = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use thiserror::Error;

/// Result alias for the {slug} module.
pub type {pascal}Result<T> = Result<T, {pascal}Error>;

/// Domain and transactional error conditions for {slug}.
#[derive(Debug, Error)]
pub enum {pascal}Error {{
    #[error("Entity not found: {{0}}")]
    NotFound(String),

    #[error("Validation failed: {{0}}")]
    Validation(String),

    #[error("Conflict: {{0}}")]
    Conflict(String),

    #[error("Database error: {{0}}")]
    Database(String),

    #[error("Internal error: {{0}}")]
    Internal(String),
}}
"#
    );
    write_file_content(
        &module_dir.join("src").join("error.rs"),
        &error_rs,
        opts.dry_run,
    )?;

    // src/module.rs
    let mut module_rs = r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_core::{ModuleKind, Permission, RusToKModule};
"#
    .to_string();

    if opts.with_db {
        module_rs.push_str(
            r#"use rustok_core::MigrationSource;
use sea_orm_migration::MigrationTrait;
"#,
        );
    }

    module_rs.push_str(&format!(
        r#"
pub struct {pascal}Module;

#[async_trait]
impl RusToKModule for {pascal}Module {{
    fn slug(&self) -> &'static str {{
        "{slug}"
    }}

    fn name(&self) -> &'static str {{
        "{title}"
    }}

    fn version(&self) -> &'static str {{
        env!("CARGO_PKG_VERSION")
    }}

    fn description(&self) -> &'static str {{
        "{desc}"
    }}

    fn kind(&self) -> ModuleKind {{
        ModuleKind::Domain
    }}
}}
"#
    ));

    if opts.with_db {
        module_rs.push_str(&format!(
            r#"
impl MigrationSource for {pascal}Module {{
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {{
        crate::migrations::migrations()
    }}
}}
"#
        ));
    }

    write_file_content(
        &module_dir.join("src").join("module.rs"),
        &module_rs,
        opts.dry_run,
    )?;

    // src/domain/mod.rs
    let domain_mod = r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure domain model, state machines, invariants, and value objects.
//!
//! In accordance with FBA, this module MUST NOT depend on database or transport frameworks.
"#
    .to_string();
    write_file_content(
        &module_dir.join("src").join("domain").join("mod.rs"),
        &domain_mod,
        opts.dry_run,
    )?;

    // src/dto/mod.rs
    let dto_mod = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Request/response, command/query, and data transfer objects.

use serde::{{Deserialize, Serialize}};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct {pascal}Summary {{
    pub slug: String,
    pub title: String,
}}
"#
    );
    write_file_content(
        &module_dir.join("src").join("dto").join("mod.rs"),
        &dto_mod,
        opts.dry_run,
    )?;

    // src/services/mod.rs
    let services_mod = r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Application and domain services orchestrating business use cases.
"#
    .to_string();
    write_file_content(
        &module_dir.join("src").join("services").join("mod.rs"),
        &services_mod,
        opts.dry_run,
    )?;

    // If with_db: generate entities and migrations
    if opts.with_db {
        let entities_mod = format!(
            r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! SeaORM persistence mappings owned by {slug}.
"#
        );
        write_file_content(
            &module_dir.join("src").join("entities").join("mod.rs"),
            &entities_mod,
            opts.dry_run,
        )?;

        let migrations_mod = format!(
            r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Database migrations owned by {slug}.

mod m20260101_000001_initial;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {{
    vec![Box::new(m20260101_000001_initial::Migration)]
}}
"#
        );
        write_file_content(
            &module_dir.join("src").join("migrations").join("mod.rs"),
            &migrations_mod,
            opts.dry_run,
        )?;

        let migration_initial = format!(
            r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait]
impl MigrationTrait for Migration {{
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {{
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("{snake}_records"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("name")).string_len(255).not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }}

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {{
        manager
            .drop_table(Table::drop().table(Alias::new("{snake}_records")).to_owned())
            .await
    }}
}}
"#
        );
        write_file_content(
            &module_dir
                .join("src")
                .join("migrations")
                .join("m20260101_000001_initial.rs"),
            &migration_initial,
            opts.dry_run,
        )?;
    }

    Ok(())
}

fn generate_admin_ui_crate(
    opts: &ModuleScaffoldOptions,
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

fn generate_storefront_ui_crate(
    opts: &ModuleScaffoldOptions,
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

fn register_in_workspace_cargo_toml(
    root: &Path,
    crate_name: &str,
    ui: ModuleScaffoldUi,
) -> Result<()> {
    let cargo_path = root.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_path)
        .with_context(|| format!("Failed to read {}", cargo_path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    // 1. Add admin/storefront UI to members array if present
    if ui.has_admin() {
        let admin_entry = format!("    \"crates/modules/{crate_name}/admin\",");
        if !content.contains(admin_entry.trim()) {
            if let Some(pos) = lines.iter().position(|l| l.trim() == "tests/e2e-rust\",") {
                lines.insert(pos, admin_entry);
                modified = true;
            } else if let Some(pos) = lines.iter().position(|l| l.trim() == "members = [") {
                lines.insert(pos + 1, admin_entry);
                modified = true;
            }
        }
    }

    if ui.has_storefront() {
        let sf_entry = format!("    \"crates/modules/{crate_name}/storefront\",");
        if !content.contains(sf_entry.trim()) {
            if let Some(pos) = lines.iter().position(|l| l.trim() == "tests/e2e-rust\",") {
                lines.insert(pos, sf_entry);
                modified = true;
            } else if let Some(pos) = lines.iter().position(|l| l.trim() == "members = [") {
                lines.insert(pos + 1, sf_entry);
                modified = true;
            }
        }
    }

    // 2. Add to [workspace.dependencies]
    let dep_needle = format!("{crate_name} = ");
    if !content.contains(&dep_needle) {
        let dep_entry = format!("{crate_name} = {{ path = \"crates/modules/{crate_name}\" }}");
        if let Some(pos) = lines
            .iter()
            .position(|l| l.trim() == "[workspace.dependencies]")
        {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    if modified {
        fs::write(&cargo_path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", cargo_path.display()))?;
        println!("  Updated root Cargo.toml (members & workspace.dependencies)");
    }

    Ok(())
}

fn register_in_modules_toml(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("modules.toml");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let module_needle = format!("{slug} = ");
    if content.contains(&module_needle) {
        return Ok(());
    }

    let entry = format!(
        "{slug} = {{ crate = \"{crate_name}\", source = \"path\", path = \"crates/modules/{crate_name}\", depends_on = [\"outbox\"] }}"
    );

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let insert_pos = lines
        .iter()
        .position(|l| l.starts_with("# Capability extensions") || l.starts_with("[settings]"))
        .unwrap_or(lines.len());

    lines.insert(insert_pos, entry);

    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    println!("  Updated modules.toml with module entry");
    Ok(())
}

fn register_in_registry_md(
    root: &Path,
    slug: &str,
    crate_name: &str,
    desc: &str,
) -> Result<()> {
    let path = root.join("docs").join("modules").join("registry.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let slug_needle = format!("| `{slug}` |");
    if content.contains(&slug_needle) {
        return Ok(());
    }

    let row = format!("| `{slug}` | `{crate_name}` | `outbox` | {desc} |");

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut insert_pos = None;

    let mut in_optional = false;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("### Optional") {
            in_optional = true;
            continue;
        }
        if in_optional && (line.starts_with("## ") || line.starts_with("The dependency column above")) {
            insert_pos = Some(i);
            break;
        }
    }

    let pos = insert_pos.unwrap_or(lines.len());
    lines.insert(pos, row);

    fs::write(&path, lines.join("\n") + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))?;
    println!("  Updated docs/modules/registry.md table");
    Ok(())
}

fn register_in_docs_index_md(root: &Path, crate_name: &str, has_admin: bool) -> Result<()> {
    let path = root.join("docs").join("modules").join("_index.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let crate_needle = format!("| `{crate_name}` |");
    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if !content.contains(&crate_needle) {
        let row = format!(
            "| `{crate_name}` | [docs](../../crates/modules/{crate_name}/docs/README.md) | [plan](../../crates/modules/{crate_name}/docs/implementation-plan.md) |"
        );

        let mut insert_pos = None;
        let mut in_domain = false;
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with("## Domain Modules") {
                in_domain = true;
                continue;
            }
            if in_domain && line.starts_with("## ") {
                insert_pos = Some(i);
                break;
            }
        }

        let pos = insert_pos.unwrap_or(lines.len());
        lines.insert(pos, row);
        modified = true;
    }

    if has_admin {
        let admin_needle = format!("- `{crate_name}`: [README](../../crates/modules/{crate_name}/admin/README.md)");
        if !content.contains(&admin_needle) {
            let mut insert_pos = None;
            let mut in_admin = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Admin Surfaces") {
                    in_admin = true;
                    continue;
                }
                if in_admin && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, admin_needle);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated docs/modules/_index.md navigation links");
    }

    Ok(())
}

fn register_in_ui_packages_index_md(
    root: &Path,
    crate_name: &str,
    has_admin: bool,
    has_storefront: bool,
) -> Result<()> {
    let path = root.join("docs").join("modules").join("UI_PACKAGES_INDEX.md");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    if has_admin {
        let admin_needle = format!("- `{crate_name}` admin UI: [README](../../crates/modules/{crate_name}/admin/README.md)");
        if !content.contains(&admin_needle) {
            let mut insert_pos = None;
            let mut in_admin = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Admin Surfaces") {
                    in_admin = true;
                    continue;
                }
                if in_admin && (line.starts_with("### ") || line.starts_with("## ")) {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, admin_needle);
            modified = true;
        }
    }

    if has_storefront {
        let sf_needle = format!("- `{crate_name}` storefront UI: [README](../../crates/modules/{crate_name}/storefront/README.md)");
        if !content.contains(&sf_needle) {
            let mut insert_pos = None;
            let mut in_sf = false;
            for (i, line) in lines.iter().enumerate() {
                if line.starts_with("### Optional/Storefront Surfaces") || line.starts_with("### Module UI Examples") {
                    in_sf = true;
                    continue;
                }
                if in_sf && (line.starts_with("### Large Capability") || line.starts_with("## What Not")) {
                    insert_pos = Some(i);
                    break;
                }
            }

            let pos = insert_pos.unwrap_or(lines.len());
            lines.insert(pos, sf_needle);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated docs/modules/UI_PACKAGES_INDEX.md links");
    }

    Ok(())
}

fn register_in_server_cargo_toml(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("apps").join("server").join("Cargo.toml");
    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    // 1. Feature entry: mod-<slug> = ["dep:rustok-<slug>", "rustok-distribution/mod-<slug>"]
    let feature_needle = format!("mod-{slug} ");
    let feature_needle_eq = format!("mod-{slug} =");
    if !content.contains(&feature_needle) && !content.contains(&feature_needle_eq) {
        let feature_entry = format!("mod-{slug} = [\"dep:{crate_name}\", \"rustok-distribution/mod-{slug}\"]");
        if let Some(pos) = lines.iter().position(|l| l.trim().starts_with("mod-workflow =") || l.trim().starts_with("mod-messenger =") || l.trim().starts_with("mod-docs =")) {
            lines.insert(pos + 1, feature_entry);
            modified = true;
        }
    }

    // 2. Dependency entry: rustok-<slug> = { workspace = true, optional = true }
    let dep_needle = format!("{crate_name} = ");
    let dep_needle_sp = format!("{crate_name} ");
    if !content.contains(&dep_needle) && !content.contains(&dep_needle_sp) {
        let dep_entry = format!("{crate_name} = {{ workspace = true, optional = true }}");
        if let Some(pos) = lines.iter().position(|l| l.trim().starts_with("rustok-messenger =") || l.trim().starts_with("rustok-docs =") || l.trim().starts_with("rustok-blog =")) {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    // 3. udeps ignore
    if !content.contains(&format!("\"{crate_name}\"")) {
        if let Some(pos) = lines.iter().position(|l| l.contains("normal = [\"rustok-rbac\"")) {
            lines[pos] = lines[pos].replace("]", &format!(", \"{crate_name}\"]"));
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated apps/server/Cargo.toml with feature & dependency");
    }

    Ok(())
}

fn register_in_distribution_cargo_toml(root: &Path, slug: &str, crate_name: &str) -> Result<()> {
    let path = root.join("crates").join("modules").join("rustok-distribution").join("Cargo.toml");
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    let mut modified = false;

    let feature_needle = format!("mod-{slug} =");
    if !content.contains(&feature_needle) {
        let feature_entry = format!("mod-{slug} = [\"dep:{crate_name}\"]");
        if let Some(pos) = lines.iter().position(|l| l.trim().starts_with("mod-messenger =") || l.trim().starts_with("mod-docs =") || l.trim().starts_with("mod-blog =")) {
            lines.insert(pos + 1, feature_entry);
            modified = true;
        }
    }

    let dep_needle = format!("{crate_name} =");
    if !content.contains(&dep_needle) {
        let dep_entry = format!("{crate_name} = {{ workspace = true, optional = true }}");
        if let Some(pos) = lines.iter().position(|l| l.trim() == "[dependencies]") {
            lines.insert(pos + 1, dep_entry);
            modified = true;
        }
    }

    if modified {
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated rustok-distribution/Cargo.toml with feature & dependency");
    }

    Ok(())
}

fn register_in_distribution_lib_rs(root: &Path, slug: &str, pascal: &str, snake: &str) -> Result<()> {
    let path = root.join("crates").join("modules").join("rustok-distribution").join("src").join("lib.rs");
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;

    let feature_needle = format!("feature = \"mod-{slug}\"");
    if content.contains(&feature_needle) {
        return Ok(());
    }

    let registration_block = format!(
        "    #[cfg(feature = \"mod-{slug}\")]\n    let registry = registry.register(rustok_{snake}::{pascal}Module);"
    );

    let mut lines = content.lines().map(String::from).collect::<Vec<_>>();
    if let Some(pos) = lines.iter().position(|l| l.contains("generated_promotions::register_promoted_modules(registry)")) {
        lines.insert(pos, registration_block);
        fs::write(&path, lines.join("\n") + "\n")
            .with_context(|| format!("Failed to write {}", path.display()))?;
        println!("  Updated rustok-distribution/src/lib.rs with build_registry() registration");
    }

    Ok(())
}
