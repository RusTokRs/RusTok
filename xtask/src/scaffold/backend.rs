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
    let crate_name = opts.crate_name();
    let kebab_name = opts.kebab_name();
    let pascal_upper = snake.to_uppercase();

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
rustok-api = {{ workspace = true }}
rustok-core.workspace = true
rustok-outbox.workspace = true
sea-orm-migration = {{ workspace = true }}
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
    let recommended_surfaces = if opts.ui.has_admin() {
        "recommended_admin_surfaces = [\"leptos-admin\"]"
    } else {
        "recommended_admin_surfaces = []"
    };
    let mut module_toml = format!(
        r#"[module]
slug = "{slug}"
name = "{title}"
version = "0.1.0"
description = "{desc}"
ownership = "first_party"
trust_level = "verified"
ui_classification = "{ui_classification}"
{recommended_surfaces}

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
route_segment = "{kebab_name}"
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
route_segment = "{kebab_name}"
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
- Emits transactional domain events via `rustok-outbox`
- Authorized via `rustok-api` permissions

## Entry points
- `{pascal}Module` implementing `RusToKModule`
- Domain services in `services`
- Transactional Outbox events in `events`
- Capability integration seams in `integrations`

## Interactions
- Emits transactional domain events via `rustok-outbox`
- Search index projections via `rustok-core` search integration
- Multi-language translation targets via `rustok-translation-targets`

## Docs
- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md)
- [Module Architecture Contract](./docs/README.md)
- [Implementation Plan](./docs/implementation-plan.md)
"#
    );
    write_file_content(&module_dir.join("README.md"), &readme, opts.dry_run)?;

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
- Event bus subscription and emission via `rustok-outbox`
- Full-text search discovery projection via `rustok-core`
- Client surfaces in `admin/` and `storefront/`

## Verification
- Unit tests: `cargo test -p {crate_name} --lib`
- Module contract verification: `cargo xtask module validate {slug}`

## Related Documents
- [Implementation Plan](./implementation-plan.md)
- [Module Platform Overview](../overview.md)
- [Backend Implementation Guide](../../../backend/module-backend-implementation.md)

## Related Documentation
- Documentation maps and registry contracts are centralized in `docs/modules/`.
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

## Scope
Sovereign domain capability and client surfaces for {title}.

## Current state
- Status: `in_progress`
- Architecture: FBA / FFA
- Backend crate: `{crate_name}`
- Admin UI crate: `{crate_name}-admin`
- Storefront UI crate: `{crate_name}-storefront`

## Local work phases
- [x] Phase 1: Canonical FBA scaffolding and contract registration
- [ ] Phase 2: Domain entities and persistence migrations
- [ ] Phase 3: Application services and transactional use cases
- [ ] Phase 4: UI components, i18n and transport facade
- [ ] Phase 5: Verification and test suite lock

## Verification
- `cargo check -p {crate_name}`
- `cargo test -p {crate_name} --lib`
- `cargo xtask module validate {slug}`

## Change rules
- Architecture changes must update this plan and relevant ADRs.
- Keep module contract synchronized with `rustok-module.toml`.
"#
    );
    write_file_content(
        &module_dir.join("docs").join("implementation-plan.md"),
        &plan,
        opts.dry_run,
    )?;

    // CRATE_API.md
    let crate_api = format!(
        r#"# {crate_name} / CRATE_API

## Role

`{crate_name}` owns the sovereign domain capability for {title} in RusToK.

Persistence entities are **crate-private**. Consumers interact exclusively through owner DTOs,
services, or explicit ports. Direct cross-crate SeaORM table access is forbidden.

## Public surface

Deliberate public modules and exports:

- `domain` — `{pascal}Status` lifecycle state;
- `dto` — owner command, input, and response data contracts;
- `error` — `{pascal}Error`, `{pascal}Result`;
- `events` — domain event descriptors;
- `integrations` — capability bridges for search, cache, media, translation;
- `module` — `{pascal}Module` lifecycle and migration provider;
- `services` — `{pascal}Service` and transaction orchestration.

`entities` and `migrations` are crate-private.

## Primary entry points

- [`{pascal}Module`](src/module.rs): Platform module registration and migration descriptor.
- [`{pascal}Service`](src/services/mod.rs): Domain application service.

## Invariants

- Tenant isolation: All queries and commands strictly enforce `tenant_id` scoping.
- Concurrency: Write operations execute within explicit transaction boundaries.

## Platform Capability Bindings

- **rustok-outbox**: Transactional outbox events and durable idempotency receipts
- **rustok-tenant**: Multi-tenant data isolation (`TenantId`)

## Domain Collaborations

- None (isolated domain service)

## Minimum Contract Set

### Input DTOs/Commands
- `{pascal}CreateInput` and `{pascal}UpdateInput` data transfer objects.

### Domain Invariants
- Tenant isolation: All queries and commands strictly enforce `tenant_id` scoping.
- Concurrency: Write operations execute within explicit transaction boundaries.

### Events / Outbox Side Effects
- Domain mutations record transactional outbox events via `rustok-outbox`.

### Errors / Failure Codes
- Strongly-typed `{pascal}Error` with structured failure categories.
"#
    );
    write_file_content(
        &module_dir.join("CRATE_API.md"),
        &crate_api,
        opts.dry_run,
    )?;

    // src/contract_tests.rs
    let contract_tests = r####"#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}
"####;
    write_file_content(
        &module_dir.join("src").join("contract_tests.rs"),
        contract_tests,
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
pub mod events;
pub mod integrations;
pub mod module;
pub mod services;

#[cfg(test)]
mod contract_tests;
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
pub use domain::{pascal}Status;
pub use error::{{{pascal}Error, {pascal}Result}};
pub use events::{{
    {pascal}CreatedEvent, {pascal}DeletedEvent, {pascal}DomainEvent, {pascal}UpdatedEvent,
    {pascal_upper}_EVENT_TOPIC,
}};
pub use module::{pascal}Module;
pub use services::{pascal}Service;
"#
    ));

    write_file_content(
        &module_dir.join("src").join("lib.rs"),
        &lib_rs,
        opts.dry_run,
    )?;

    // src/error/mod.rs (directory compliance for enrolled layout)
    let error_mod = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Domain and transactional error catalog for {slug}.

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
        &module_dir.join("src").join("error").join("mod.rs"),
        &error_mod,
        opts.dry_run,
    )?;

    // src/events/mod.rs (Outbox domain events)
    let events_mod = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Transactional Outbox domain events for {slug}.
//!
//! All state mutations emit typed events through `rustok-outbox` for
//! asynchronous distribution, cache invalidation, and CQRS projection updates.

use chrono::{{DateTime, Utc}};
use serde::{{Deserialize, Serialize}};
use uuid::Uuid;

/// Canonical event topic for all {slug} domain events.
pub const {pascal_upper}_EVENT_TOPIC: &str = "{slug}.events";

pub const {pascal_upper}_EVENT_CREATED: &str = "{slug}.created";
pub const {pascal_upper}_EVENT_UPDATED: &str = "{slug}.updated";
pub const {pascal_upper}_EVENT_DELETED: &str = "{slug}.deleted";

/// Published when a new {slug} record is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct {pascal}CreatedEvent {{
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}}

/// Published when an existing {slug} record is updated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct {pascal}UpdatedEvent {{
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub name: Option<String>,
    pub updated_at: DateTime<Utc>,
}}

/// Published when a {slug} record is deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct {pascal}DeletedEvent {{
    pub tenant_id: Uuid,
    pub entity_id: Uuid,
    pub deleted_at: DateTime<Utc>,
}}

/// Enumeration of all domain events emitted by {slug}.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum {pascal}DomainEvent {{
    Created({pascal}CreatedEvent),
    Updated({pascal}UpdatedEvent),
    Deleted({pascal}DeletedEvent),
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_event_serde_roundtrip() {{
        let now = Utc::now();
        let tenant_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        let event = {pascal}CreatedEvent {{
            tenant_id,
            entity_id,
            name: "Test entity".into(),
            created_at: now,
        }};

        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: {pascal}CreatedEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(event, decoded);

        let domain_event = {pascal}DomainEvent::Created(event);
        let domain_json = serde_json::to_string(&domain_event).expect("serialize domain event");
        let domain_decoded: {pascal}DomainEvent =
            serde_json::from_str(&domain_json).expect("deserialize domain event");
        assert_eq!(domain_event, domain_decoded);
    }}
}}
"#
    );
    write_file_content(
        &module_dir.join("src").join("events").join("mod.rs"),
        &events_mod,
        opts.dry_run,
    )?;

    // src/integrations/mod.rs (Platform capability bridges)
    let search_mod = if opts.with_db {
        format!(
            r#"pub mod search {{
    //! Search & discovery projection seam.
    //!
    //! All discoverable domain entities emit search projection documents into `rustok-search`.

    use std::sync::Arc;
    use async_trait::async_trait;
    use rustok_core::search_projection::{{
        SearchProjectionDocument, SearchProjectionPage,
        SearchProjectionSource, SearchProjectionSourceFactory,
    }};
    use rustok_core::{{ModuleRuntimeExtensions, Result}};
    use sea_orm::DatabaseConnection;
    use uuid::Uuid;

    pub const {pascal_upper}_SOURCE_MODULE: &str = "{slug}";

    #[derive(Clone, Default)]
    pub struct {pascal}SearchProjectionSourceFactory;

    impl SearchProjectionSourceFactory for {pascal}SearchProjectionSourceFactory {{
        fn source_module(&self) -> &'static str {{
            {pascal_upper}_SOURCE_MODULE
        }}

        fn build(&self, db: DatabaseConnection) -> Arc<dyn SearchProjectionSource> {{
            Arc::new({pascal}SearchProjectionSource {{ _db: db }})
        }}

        fn build_with_extensions(
            &self,
            db: DatabaseConnection,
            _extensions: &ModuleRuntimeExtensions,
        ) -> Arc<dyn SearchProjectionSource> {{
            Arc::new({pascal}SearchProjectionSource {{ _db: db }})
        }}
    }}

    #[derive(Clone)]
    pub struct {pascal}SearchProjectionSource {{
        _db: DatabaseConnection,
    }}

    #[async_trait]
    impl SearchProjectionSource for {pascal}SearchProjectionSource {{
        fn source_module(&self) -> &'static str {{
            {pascal_upper}_SOURCE_MODULE
        }}

        async fn list_public_documents(
            &self,
            _tenant_id: Uuid,
            _after: Option<String>,
            _limit: usize,
        ) -> Result<SearchProjectionPage> {{
            // Domain query mapping for search indexation
            Ok(SearchProjectionPage {{
                documents: Vec::new(),
                next_cursor: None,
            }})
        }}

        async fn load_public_entity(
            &self,
            _tenant_id: Uuid,
            _entity_type: &str,
            _entity_id: Uuid,
        ) -> Result<Vec<SearchProjectionDocument>> {{
            Ok(Vec::new())
        }}
    }}
}}
"#
        )
    } else {
        r#"pub mod search {
    //! Search & discovery projection seam.
    //!
    //! When persistence is enabled, implement `SearchProjectionSourceFactory`
    //! from `rustok_core::search_projection` to publish discoverable documents.
}
"#
        .to_string()
    };

    let integrations_mod = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical integration seams and capability bridges for {slug}.
//!
//! This module provides uniform conventions for interacting with cross-cutting
//! platform capabilities: Caching, Search indexing, Media attachments, and Translation.

pub mod cache {{
    //! Cache key helpers and invalidation conventions.
    //!
    //! Standard Redis / in-memory cache key format:
    //! `tenant:{{tenant_id}}:{slug}:{{entity_name}}:{{entity_id}}`

    use uuid::Uuid;

    /// Formats a canonical tenant-scoped cache key for a record.
    pub fn record_cache_key(tenant_id: Uuid, id: Uuid) -> String {{
        format!("tenant:{{tenant_id}}:{slug}:record:{{id}}")
    }}

    /// Formats a tenant-scoped cache prefix for bulk invalidation sweeps.
    pub fn tenant_cache_prefix(tenant_id: Uuid) -> String {{
        format!("tenant:{{tenant_id}}:{slug}:")
    }}
}}

{search_mod}
pub mod media {{
    //! Media asset attachment conventions.
    //!
    //! Assets (images, documents, video) are sovereignly owned by `rustok-media`.
    //! When {slug} entities associate with media:
    //! - Store referenced media asset IDs as `Option<uuid::Uuid>` or `Vec<uuid::Uuid>`.
    //! - Invariant: Verify tenant ownership of the media asset before linking.
    //! - Side-effect: When deleting an entity, emit a deletion event so media references can be reclaimed.
}}

pub mod translation {{
    //! Multi-language translation target seam.
    //!
    //! When {slug} domain entities contain localized strings (e.g. title, description, content),
    //! implement `TranslationTargetProvider` from `rustok_translation_targets` and register
    //! the provider inside `{pascal}Module::register_runtime_extensions`.
    //!
    //! See `crates/modules/rustok-taxonomy/src/translation_target.rs` as canonical reference.
}}
"#
    );
    write_file_content(
        &module_dir.join("src").join("integrations").join("mod.rs"),
        &integrations_mod,
        opts.dry_run,
    )?;

    // src/module.rs
    let search_registration = if opts.with_db {
        format!(
            r#"        rustok_core::search_projection::register_search_projection_source(
            extensions,
            crate::integrations::search::{pascal}SearchProjectionSourceFactory,
        )?;
"#
        )
    } else {
        String::new()
    };

    let mut module_rs = format!(
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
use rustok_api::Permission;
use rustok_core::module::{{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleKind, ModuleRuntimeExtensions, RusToKModule,
}};
use sea_orm_migration::MigrationTrait;

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
        ModuleKind::Optional
    }}

    fn dependencies(&self) -> &[&'static str] {{
        &["outbox"]
    }}

    fn permissions(&self) -> Vec<Permission> {{
        // Standard RBAC permissions for {slug}.
        // Declare registered permissions here once corresponding Resource
        // variants are added to `rustok_api::permissions::Resource`.
        Vec::new()
    }}

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {{
{search_registration}        Ok(())
    }}

    fn register_event_listeners(
        &self,
        _registry: &mut ModuleEventListenerRegistry,
        _ctx: &ModuleEventListenerContext<'_>,
    ) {{
        // Register event listeners for cross-module integration or outbox event handling:
    }}
}}
"#
    );

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
    } else {
        module_rs.push_str(&format!(
            r#"
impl MigrationSource for {pascal}Module {{
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {{
        Vec::new()
    }}
}}
"#
        ));
    }

    module_rs.push_str(&format!(
        r#"
#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_module_metadata() {{
        let module = {pascal}Module;
        assert_eq!(module.slug(), "{slug}");
        assert_eq!(module.name(), "{title}");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.kind(), ModuleKind::Optional);
    }}

    #[test]
    fn test_module_permissions_contract() {{
        let module = {pascal}Module;
        let _perms = module.permissions();
    }}
}}
"#
    ));

    write_file_content(
        &module_dir.join("src").join("module.rs"),
        &module_rs,
        opts.dry_run,
    )?;

    // src/domain/mod.rs
    let domain_mod = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure domain model, state machines, invariants, and value objects for {title}.
//!
//! In accordance with FBA, this module MUST NOT depend on database or transport frameworks.

use serde::{{Deserialize, Serialize}};
use crate::error::{{{pascal}Error, {pascal}Result}};

/// Domain lifecycle state for {title}.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum {pascal}Status {{
    #[default]
    Draft,
    Active,
    Archived,
}}

impl {pascal}Status {{
    /// Validates whether a state transition is permitted according to domain invariants.
    pub fn can_transition_to(&self, target: Self) -> bool {{
        match (self, target) {{
            (Self::Draft, Self::Active) => true,
            (Self::Active, Self::Archived) => true,
            (Self::Archived, Self::Draft) => true,
            (current, target) if *current == target => true,
            _ => false,
        }}
    }}

    /// Attempts state transition with typed domain invariant guard.
    pub fn transition_to(&mut self, target: Self) -> {pascal}Result<()> {{
        if !self.can_transition_to(target) {{
            return Err({pascal}Error::Validation(format!(
                "Invalid state transition from {{:?}} to {{:?}}",
                self, target
            )));
        }}
        *self = target;
        Ok(())
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_status_lifecycle_transitions() {{
        let mut status = {pascal}Status::Draft;
        assert!(status.transition_to({pascal}Status::Active).is_ok());
        assert_eq!(status, {pascal}Status::Active);

        // Invalid transition: Active -> Draft
        assert!(status.transition_to({pascal}Status::Draft).is_err());

        // Valid transition: Active -> Archived
        assert!(status.transition_to({pascal}Status::Archived).is_ok());
        assert_eq!(status, {pascal}Status::Archived);
    }}
}}
"#
    );
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

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_summary_serde_roundtrip() {{
        let summary = {pascal}Summary {{
            slug: "{slug}".into(),
            title: "{title}".into(),
        }};
        let json = serde_json::to_string(&summary).expect("serialize summary");
        let decoded: {pascal}Summary = serde_json::from_str(&json).expect("deserialize summary");
        assert_eq!(summary.slug, decoded.slug);
        assert_eq!(summary.title, decoded.title);
    }}
}}
"#
    );
    write_file_content(
        &module_dir.join("src").join("dto").join("mod.rs"),
        &dto_mod,
        opts.dry_run,
    )?;

    // src/services/mod.rs
    let services_mod = format!(
        r#"/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Application and domain services orchestrating business use cases for {title}.

use crate::error::{pascal}Result;

#[derive(Clone, Debug, Default)]
pub struct {pascal}Service;

impl {pascal}Service {{
    pub fn new() -> Self {{
        Self
    }}

    pub fn ping(&self) -> {pascal}Result<&'static str> {{
        Ok("pong")
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn test_service_ping() {{
        let service = {pascal}Service::new();
        assert_eq!(service.ping().unwrap(), "pong");
    }}
}}
"#
    );
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
