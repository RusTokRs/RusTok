use super::options::ScaffoldOptions;
use super::write_file_content;
use anyhow::Result;
use std::path::Path;

pub(crate) fn generate_backend(
    opts: &ScaffoldOptions,
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
use rustok_core::{ModuleKind, RusToKModule};
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
