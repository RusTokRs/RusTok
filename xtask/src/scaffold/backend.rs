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
    let entity = opts.entity.as_deref();
    let storage_dependencies = if entity.is_some() {
        "chrono.workspace = true\nrustok-api = { workspace = true, features = [\"runtime\"] }\nrustok-cache.workspace = true\nrustok-events.workspace = true\nrustok-outbox-storage.workspace = true\nsea-orm.workspace = true\nserde.workspace = true\nserde_json.workspace = true\nthiserror.workspace = true\ntracing.workspace = true\nuuid.workspace = true\n\n[dev-dependencies]\nrustok-migrations.workspace = true\nrustok-modules.workspace = true\nrustok-search.workspace = true\nrustok-test-utils.workspace = true\ntokio.workspace = true\ntoml.workspace = true\n"
    } else {
        ""
    };
    let (graphql_dependency, graphql_feature) = if opts.admin_resource.is_some() {
        (
            "async-graphql = { workspace = true, optional = true }\nrustok-channel-ports = { workspace = true, optional = true }\nrustok-index = { workspace = true, optional = true }\n",
            "admin = [\"rustok-api/server\"]\ngraphql = [\"dep:async-graphql\", \"dep:rustok-channel-ports\", \"dep:rustok-index\", \"admin\"]\n",
        )
    } else {
        ("", "")
    };
    let ui_requested = opts.ui != super::options::ScaffoldUi::None;
    let ui_classification = if opts.mounts_record_admin() {
        "admin_only"
    } else if ui_requested {
        "future_ui"
    } else {
        "no_ui"
    };
    let recommended_admin_surfaces = if opts.mounts_record_admin() {
        r#"["leptos-admin", "next-admin"]"#
    } else {
        "[]"
    };
    let ui_summary = match (opts.ui, opts.has_record_admin()) {
        (super::options::ScaffoldUi::None, _) => "No UI packages requested.",
        (super::options::ScaffoldUi::Admin, true) => {
            "Leptos and Next Admin register a record table, localized editor and settings. Runtime parity requires operator verification."
        }
        (super::options::ScaffoldUi::Dual, true) => {
            "Leptos admin package with a record table, localized editor and native/GraphQL adapters, plus a draft storefront package. Both remain unmounted."
        }
        (super::options::ScaffoldUi::Admin, false) => {
            "Draft Leptos admin crate created but NOT mounted in the host."
        }
        (super::options::ScaffoldUi::Storefront, _) => {
            "Draft Leptos storefront crate created but NOT mounted in the host."
        }
        (super::options::ScaffoldUi::Dual, false) => {
            "Draft Leptos admin and storefront crates created but NOT mounted in hosts."
        }
    };
    let ui_readiness = if ui_requested {
        "draft FFA packages, not_started; no live host route or owner operations"
    } else {
        "not applicable; no UI packages or live UI routes"
    };
    let ui_work_phases = if ui_requested {
        "- [ ] Implement owner-specific native/GraphQL transports and mount Leptos UI after real operations.\n- [ ] Implement and verify the independent classic Next.js client before publishing a live UI surface."
    } else {
        "- [ ] Preserve the headless contract; add UI only if a real owner API and new product scope justify it."
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
rustok-core.workspace = true
sea-orm-migration.workspace = true
{graphql_dependency}{storage_dependencies}
[features]
default = []
{graphql_feature}"#
    );

    let record_settings = entity.map_or_else(String::new, |_| {
        super::persistence::record_settings_manifest()
    });
    let admin_manifest = if opts.mounts_record_admin() {
        let route = opts.kebab_name();
        format!(
            r#"
[provides.admin_ui]
leptos_crate = "{crate_name}-admin"
next_package = "@rustok/{route}-admin"
route_segment = "{route}"
nav_label = {title_toml}

[provides.admin_ui.i18n]
default_locale = "en"
supported_locales = ["en", "ru"]
leptos_locales_path = "admin/locales"

[[provides.admin_ui.child_pages]]
subpath = "records"
title = "Records"
nav_label = "Records"
nav_label_key = "records-heading"

[[provides.admin_ui.child_pages]]
subpath = "settings"
title = "Settings"
nav_label = "Settings"
nav_label_key = "settings-heading"

[[provides.admin_ui.child_pages]]
subpath = "settings/records"
title = "Record settings"
nav_label = "Record settings"
nav_label_key = "settings-records-nav"
"#
        )
    } else {
        String::new()
    };
    let graphql_manifest = if opts.admin_resource.is_some() {
        format!(
            "\n[provides.graphql]\nquery = \"graphql::{pascal}Query\"\nmutation = \"graphql::{pascal}AdminMutation\"\nruntime_data_factory = \"graphql::attach_schema_data\"\n"
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
recommended_admin_surfaces = {recommended_admin_surfaces}

[marketplace]
category = {category_toml}
tags = ["{slug}"]

[crate]
entry_type = "{module_type}"
{record_settings}{graphql_manifest}{admin_manifest}"#
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
- No services, persistence tables, business GraphQL/HTTP endpoints or event producers are declared.
- {ui_summary} No `[provides.*_ui]` live surface is declared.

## Interactions
- Platform runtime discovery only. No external capability or module dependency is required.

## Known Limitations / Pending Implementation
- There are no domain commands, durable persistence, migrations or business transports yet.
- No live UI route is declared; any requested UI development package is unmounted.
- Compilation, runtime composition and FBA/FFA readiness require separate evidence.
- Track owner operations and verification in [the implementation plan](./docs/implementation-plan.md).

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

### Connecting a capability

Start in the owner command or query. When it needs a platform capability:

1. Use the [platform integration guide](../../../../docs/backend/module-backend-implementation.md#connecting-platform-capabilities)
   to select the canonical library, owner contract and file. It covers cache,
   RBAC, relational Index, Search, Media, outbox, i18n and Page Builder / Fly.
2. Keep pure invariants in `src/domain/` and the use case in `src/services/`.
   Put any required provider adapter in `src/integrations/<provider>.rs` and its
   module-local registration in `src/module.rs`. Update `Cargo.toml`,
   `rustok-module.toml` and the selected runtime composition when required by
   that contract.
3. Document the operation, typed context, authority, transaction boundary and
   failure behavior in [the implementation plan](./implementation-plan.md).
   Keep a short source comment where the binding or ordering needs explanation.
4. Verify the owner operation through the selected provider and exercise the
   relevant denial, rollback, invalidation or retry paths. Retain exact commands
   and results for the applicable reference gates.

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
- UI surfaces: {ui_summary}
- UI readiness: {ui_readiness}
- No business functionality or runtime transport parity is claimed.

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `not_started`
- Evidence: {ui_summary} No owner operations, native/GraphQL parity or live client parity are verified.
- Last verified at (UTC): not verified; record scoped execution evidence before promotion.
- Owner: {slug} module owner.

### FBA owner ports and deployment

Use the [canonical FBA layout](../../../../docs/backend/module-backend-implementation.md#remote-fba-module-layout)
for each published owner contract. Put real ports and their DTOs in
`ports/` (`{crate_name}-ports`). The owner implements the same ports in-process
and through optional gRPC adapters. Put standalone composition in the root
`src/bin/service.rs` binary behind the `service` feature.

Choose providers in the host and keep data ownership and transactions in the
owner. Configuration, schemas and policies stay with their defining owners;
values attached to this owner's records stay in this owner's transactions.
Apply the same layout when converting an existing owner. Run
`cargo xtask module lint {slug} --fba` after publishing the ports package and
verify both execution profiles before promoting readiness.

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `data boundary: none`
- Native migrations: none
- Supported migration policy: none
- Predecessor standby strategy: none
- Rollback eligibility: `DeniedMaintenanceOnly`
- Recovery invariants: no persistent state or mutation path exists; release execution,
  predecessor retention and rollback are unverified. Specify epoch/idempotency
  invariants when owner operations are implemented.

## Local work phases
- [x] Define native module runtime metadata and contract tests.
- [ ] Specify domain rules, owner service/DTO commands and typed context/errors.
- [ ] Design real persistence and migrations if required.
{ui_work_phases}
- [ ] Verify runtime, FBA/FFA evidence, registration and release readiness.

## Reference verification

Use the [integration steps](./README.md#connecting-a-capability) to record
how each implemented owner operation binds to a platform capability.

Use the [file placement rules](../../../../docs/backend/module-backend-implementation.md#file-placement-for-reference-capabilities)
and the [storage and Page Builder reference variants](../../../../docs/backend/module-backend-implementation.md#storage-and-page-builder-reference-variants)
to select paths for each implemented capability.

Use the [applicable reference evidence matrix](../../../../docs/backend/module-backend-verification.md#applicable-reference-evidence)
to record scenario names, exact commands, revision, build/database profile,
prerequisites and results in this plan as owner operations are implemented.
Record an owner-contract reason for each inapplicable family; missing execution
infrastructure leaves applicable evidence pending. Test counts and static guards
alone do not establish behavioral coverage.

If persistence is introduced, verify the same owner invariants on PostgreSQL
and SQLite within their supported profiles through real migrations. PostgreSQL
is the production profile; SQLite is local/demo/test. Database generation is not
provided by this identity scaffold.

Reuse the canonical backend, FFA/UI and i18n libraries for implemented consumers;
do not add unused layers or dependencies to fill the reference checklist.

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

    let (lib_summary, modules, service_exports, migration_body, migration_assertion) = match entity {
        Some(entity) => {
            let name = crate::manifest_validation::to_pascal_case(entity);
            (
                "//! The owner service exposes a tenant-scoped localized record.",
                "mod domain;\nmod dto;\nmod entities;\nmod error;\nmod integrations;\nmod migrations;\nmod services;\n",
                format!("pub use dto::{{List{name}Records, {name}Page, {name}View, RecordOrder, RecordPublication, Save{name}Title}};\npub use error::RecordError;\npub use services::{name}Service;\n"),
                "vec![Box::new(crate::migrations::Migration)]\n    }\n\n    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {\n        vec![rustok_core::MigrationDependencyDescriptor::new(\n            crate::migrations::ID,\n            vec![\"m20250101_000001_create_tenants\", \"m20260211_000002_create_sys_events\"],\n        )]\n",
                "assert_eq!(module.migrations().len(), 1);",
            )
        }
        None => (
            "//! Business API and persistence are intentionally absent until specified.",
            "",
            String::new(),
            "// Export real owner migrations from src/migrations/mod.rs once the\n        // schema and its ordering and safety contracts have been defined.\n        Vec::new()\n",
            "assert!(module.migrations().is_empty());",
        ),
    };
    let graphql_module = if opts.admin_resource.is_some() {
        "#[cfg(feature = \"admin\")]\npub use services::{admin_record_revision, admin_record_security};\n\n#[cfg(feature = \"graphql\")]\npub mod graphql;\n\n"
    } else {
        ""
    };
    let lib = format!(
        r#"//! Native runtime module for {title}.
//!
{lib_summary}

#![doc = include_str!("../CRATE_API.md")]

{modules}mod module;

{service_exports}{graphql_module}pub use module::{module_type};

#[cfg(test)]
mod tests;
"#
    );

    let register_search = if entity.is_some() {
        r#"
    fn register_runtime_extensions(
        &self,
        extensions: &mut rustok_core::ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        crate::integrations::search::register(extensions)?;
        crate::integrations::index::register(extensions);
        Ok(())
    }
"#
    } else {
        ""
    };
    let permissions = opts
        .admin_resource
        .as_ref()
        .map_or_else(String::new, |resource| {
            format!(
                r#"
    fn permissions(&self) -> Vec<rustok_api::Permission> {{
        use rustok_api::{{Action, Permission, Resource}};
        vec![
            Permission::new(Resource::{resource}, Action::List),
            Permission::new(Resource::{resource}, Action::Read),
            Permission::new(Resource::{resource}, Action::Create),
            Permission::new(Resource::{resource}, Action::Update),
            Permission::new(Resource::{resource}, Action::Publish),
        ]
    }}
"#
            )
        });
    let module = format!(
        r#"use async_trait::async_trait;
use rustok_core::module::{{MigrationSource, ModuleKind, RusToKModule}};
use sea_orm_migration::MigrationTrait;

/// Native runtime registration for this owner.
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
        // Declare required runtime modules when an owner operation consumes
        // them. See docs/README.md for provider wiring and verification.
        &[]
    }}
{permissions}{register_search}}}

impl MigrationSource for {module_type} {{
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {{
        {migration_body}    }}
}}
"#
    );

    let tests = format!(
        r####"use crate::{module_type};
use rustok_core::module::{{MigrationSource, ModuleKind, RusToKModule}};

#[test]
fn native_contract_matches_the_declared_module() {{
    let module = {module_type};
    assert_eq!(module.slug(), "{slug}");
    assert_eq!(module.name(), {title_rust});
    assert_eq!(module.description(), {description_rust});
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert!(module.dependencies().is_empty());
    {migration_assertion}
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
    assert!(api.contains({contract_marker:?}));
}}
"####,
        contract_marker = if entity.is_some() {
            "## Embedded owner service"
        } else {
            "No create/update/delete commands are advertised"
        }
    );

    let (readme, docs, plan, api) = entity.map_or((readme, docs, plan, api), |entity| {
        super::persistence::documentation(opts, entity, &module_type, ui_summary)
    });
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
    .chain(
        entity
            .into_iter()
            .flat_map(|entity| super::persistence::render(opts, entity, &module_type)),
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::render_backend;
    use crate::scaffold::options::{ScaffoldOptions, ScaffoldProfile, ScaffoldUi};

    #[test]
    fn localized_record_renders_owner_files_and_registered_migration() {
        let mut options = options();
        options.with_db = true;
        options.entity = Some("knowledge_entry".to_owned());
        let files = render_backend(&options, "Example");
        let file = |expected: &str| {
            &files
                .iter()
                .find(|(path, _)| path == std::path::Path::new(expected))
                .unwrap()
                .1
        };

        assert!(file("src/lib.rs").contains("KnowledgeEntryService"));
        assert!(file("src/module.rs").contains("crate::migrations::Migration"));
        assert!(
            file("src/migrations/m20261126_000001_create_example_records.rs")
                .contains("example_record_translations")
        );
        assert!(file("tests/record/main.rs").contains("rustok_example"));
        assert!(file("tests/record/list.rs").contains("assert_list_contract"));
        assert!(file("tests/record/search.rs").contains("execute_audience_eligible_search"));
        let manifest: toml::Table = toml::from_str(file("rustok-module.toml")).unwrap();
        let page_size = &manifest["settings"]["admin_page_size"];
        assert_eq!(page_size["type"].as_str(), Some("integer"));
        assert_eq!(page_size["default"].as_integer(), Some(25));
        assert_eq!(page_size["min"].as_integer(), Some(1));
        assert_eq!(page_size["max"].as_integer(), Some(100));
        for (path, source) in files {
            if path.extension().is_some_and(|extension| extension == "rs") {
                syn::parse_file(&source)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            }
        }
    }

    #[test]
    fn published_admin_graphql_uses_owner_specific_symbols_and_an_explicit_resource() {
        let mut options = options();
        options.with_db = true;
        options.entity = Some("article".to_owned());
        options.admin_resource = Some("Posts".to_owned());
        let files = render_backend(&options, "Example");
        let file = |expected: &str| {
            &files
                .iter()
                .find(|(path, _)| path == std::path::Path::new(expected))
                .unwrap()
                .1
        };
        assert!(file("src/graphql/query.rs").contains("exampleAdminRecords"));
        assert!(file("src/graphql/types.rs").contains("ExampleAdminRecordTitleInput"));
        assert!(file("src/services/record/admin.rs").contains("Resource::Posts"));
        assert!(file("src/lib.rs").contains("#[cfg(feature = \"graphql\")]"));
        assert!(file("src/module.rs").contains("Permission::new(Resource::Posts, Action::List)"));
        let manifest: toml::Table = toml::from_str(file("rustok-module.toml")).unwrap();
        let graphql = &manifest["provides"]["graphql"];
        assert_eq!(graphql["query"].as_str(), Some("graphql::ExampleQuery"));
        assert_eq!(
            graphql["mutation"].as_str(),
            Some("graphql::ExampleAdminMutation")
        );
        assert_eq!(
            graphql["runtime_data_factory"].as_str(),
            Some("graphql::attach_schema_data")
        );
        for (path, source) in files {
            if path.extension().is_some_and(|extension| extension == "rs") {
                syn::parse_file(&source)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            }
        }
    }

    fn options() -> ScaffoldOptions {
        ScaffoldOptions {
            profile: ScaffoldProfile::NativeModule,
            slug: "example".into(),
            title: "Example".into(),
            description: "A native example module without business logic".into(),
            category: "collaboration".into(),
            with_db: false,
            entity: None,
            admin_resource: None,
            ui: ScaffoldUi::None,
            dry_run: true,
            skip_server_wire: false,
            skip_validate: false,
        }
    }

    #[test]
    fn minimal_backend_has_only_real_responsibilities() {
        let files = render_backend(&options(), "Example");
        let paths: Vec<_> = files
            .iter()
            .map(|(path, _)| path.to_str().unwrap())
            .collect();
        assert_eq!(paths.len(), 10);
        assert!(paths.contains(&"src/tests/contract.rs"));
        assert!(!paths.contains(&"src/contract_tests.rs"));
        assert!(!paths.iter().any(|path| path.contains("entities")
            || path.contains("migrations")
            || path.contains("services")
            || path.contains("integrations")
            || path.contains("domain")));
        let lib = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("src/lib.rs"))
            .unwrap()
            .1
            .as_str();
        assert!(lib.contains("mod module;"));
        assert!(lib.contains("#![doc = include_str!(\"../CRATE_API.md\")]"));
        assert!(!lib.contains("pub mod entities;"));
    }

    #[test]
    fn native_module_manifest_matches_the_golden_snapshot() {
        let files = render_backend(&options(), "Example");
        let manifest = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("rustok-module.toml"))
            .unwrap()
            .1
            .as_str();
        assert_eq!(
            manifest,
            include_str!("goldens/native-module-manifest.golden")
        );
    }

    #[test]
    fn metadata_is_parseable_and_reports_no_ui_or_fake_dependencies() {
        let files = render_backend(&options(), "Example");
        let manifest = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("rustok-module.toml"))
            .unwrap();
        let value: toml::Value = toml::from_str(&manifest.1).unwrap();
        assert_eq!(value["module"]["ui_classification"].as_str(), Some("no_ui"));
        assert_eq!(value["module"]["trust_level"].as_str(), Some("private"));
        assert!(value.get("provides").is_none());
        assert!(value.get("dependencies").is_none());
        let cargo = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("Cargo.toml"))
            .unwrap();
        let parsed: toml::Value = toml::from_str(&cargo.1).unwrap();
        assert!(parsed["dependencies"].get("sea-orm").is_none());
        assert!(parsed["dependencies"].get("rustok-outbox").is_none());
    }

    #[test]
    fn no_ui_plan_does_not_pretend_a_draft_ui_exists() {
        let files = render_backend(&options(), "Example");
        let plan = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("docs/implementation-plan.md"))
            .unwrap()
            .1
            .as_str();
        assert!(plan.contains("no UI packages or live UI routes"));
        assert!(plan.contains("Preserve the headless contract"));
        assert!(!plan.contains("draft skeleton"));
        assert!(!plan.contains("mount Leptos UI"));
        let readme = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("README.md"))
            .unwrap()
            .1
            .as_str();
        assert!(readme.contains("No UI packages requested"));
        assert!(readme.contains("No live UI route is declared"));
    }

    #[test]
    fn requested_ui_creates_only_future_ui_metadata_without_live_provides() {
        for ui in [ScaffoldUi::Admin, ScaffoldUi::Storefront, ScaffoldUi::Dual] {
            let mut opts = options();
            opts.ui = ui;
            let files = render_backend(&opts, "Example");
            let manifest = &files
                .iter()
                .find(|(p, _)| p.to_str() == Some("rustok-module.toml"))
                .unwrap()
                .1;
            let value: toml::Value = toml::from_str(manifest).unwrap();
            assert_eq!(
                value["module"]["ui_classification"].as_str(),
                Some("future_ui")
            );
            assert!(
                value.get("provides").is_none(),
                "draft UI must not mount a live surface"
            );
            let plan = &files
                .iter()
                .find(|(p, _)| p.to_str() == Some("docs/implementation-plan.md"))
                .unwrap()
                .1;
            assert!(plan.contains("not_started"));
            assert!(plan.contains("NOT mounted"));
        }
    }

    #[test]
    fn every_mode_declares_owner_readiness_without_claiming_safe_release() {
        for ui in [
            ScaffoldUi::None,
            ScaffoldUi::Admin,
            ScaffoldUi::Storefront,
            ScaffoldUi::Dual,
        ] {
            let mut request = options();
            request.ui = ui;
            let files = render_backend(&request, "Example");
            let plan = &files
                .iter()
                .find(|(path, _)| path.to_str() == Some("docs/implementation-plan.md"))
                .unwrap()
                .1;
            for required in [
                "## FFA/FBA status",
                "#remote-fba-module-layout",
                "`ports/` (`rustok-example-ports`)",
                "`src/bin/service.rs` binary behind the `service` feature",
                "cargo xtask module lint example --fba",
                "## Release and Data Rollback Readiness",
                "Runtime kind: Optional",
                "Rollback unit: Component",
                "Data boundary owner: `data boundary: none`",
                "Native migrations: none",
                "Supported migration policy: none",
                "Predecessor standby strategy: none",
                "Rollback eligibility: `DeniedMaintenanceOnly`",
                "Recovery invariants:",
            ] {
                assert!(plan.contains(required), "missing {required} for {ui:?}");
            }
        }
    }

    #[test]
    fn caller_strings_are_escaped_for_rust_and_toml() {
        let mut opts = options();
        opts.title = "An \"Example\"".into();
        opts.description = "A \\quoted\\ module with a \"name\" and valid description".into();
        let files = render_backend(&opts, "Example");
        let metadata = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("rustok-module.toml"))
            .unwrap();
        let parsed: toml::Value = toml::from_str(&metadata.1).unwrap();
        assert_eq!(parsed["module"]["name"].as_str(), Some(opts.title.as_str()));
        assert_eq!(
            parsed["module"]["description"].as_str(),
            Some(opts.description.as_str())
        );
        let module_rs = files
            .iter()
            .find(|(path, _)| path.to_str() == Some("src/module.rs"))
            .unwrap()
            .1
            .as_str();
        assert!(module_rs.contains(&format!("{:?}", opts.title)));
    }
}
