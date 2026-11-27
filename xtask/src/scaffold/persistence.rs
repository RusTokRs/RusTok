use std::path::PathBuf;

use super::ScaffoldOptions;

pub(super) const PAGE_SIZE_KEY: &str = "admin_page_size";
pub(super) const DEFAULT_PAGE_SIZE: u16 = 25;
pub(super) const MAX_PAGE_SIZE: u16 = 100;
pub(super) const PUBLIC_DISCOVERY_KEY: &str = "public_discovery_enabled";

pub(super) fn record_settings_manifest() -> String {
    format!(
        "\n[settings]\n{PAGE_SIZE_KEY} = {{ type = \"integer\", default = {DEFAULT_PAGE_SIZE}, min = 1, max = {MAX_PAGE_SIZE}, description = \"Default number of records per page in the admin table\" }}\n{PUBLIC_DISCOVERY_KEY} = {{ type = \"boolean\", default = false, description = \"Expose published records to public reads\" }}\n"
    )
}

pub(super) fn render(
    options: &ScaffoldOptions,
    entity: &str,
    module: &str,
) -> Vec<(PathBuf, String)> {
    let entity_type = crate::manifest_validation::to_pascal_case(entity);
    let table = format!("{}_records", options.slug);
    let translations = format!("{}_record_translations", options.slug);
    let migration = format!("m20261126_000001_create_{}_records", options.slug);
    let crate_path = options.crate_name().replace('-', "_");
    let module_stem = module
        .strip_suffix("Module")
        .expect("generated module name");
    let module_field = super::options::graphql_field_prefix(module_stem);
    let render = |source: &str| {
        source
            .replace("__ENTITY__", &entity_type)
            .replace("__TABLE__", &table)
            .replace("__TRANSLATIONS__", &translations)
            .replace("__MIGRATION__", &migration)
            .replace("__OWNER__", &crate_path)
            .replace("__MODULE__", module)
            .replace("__MODULE_STEM__", module_stem)
            .replace("__MODULE_FIELD__", &module_field)
            .replace("// __GRAPHQL_TEST_MODULE__", if options.admin_resource.is_some() {
                "#[cfg(feature = \"graphql\")]\nmod graphql;"
            } else {
                ""
            })
            .replace("        // __GRAPHQL_POSTGRES_CASE__", if options.admin_resource.is_some() {
                "        #[cfg(feature = \"graphql\")]\n        {\n            let (tenant, foreign) = seed_tenants(&db).await;\n            graphql::assert_graphql_contract(db.clone(), tenant, foreign).await;\n        }"
            } else {
                ""
            })
            .replace("__SLUG__", &options.slug)
            .replace("__ENTITY_SLUG__", entity)
            .replace("__PAGE_SIZE_KEY__", PAGE_SIZE_KEY)
            .replace("__PUBLIC_DISCOVERY_KEY__", PUBLIC_DISCOVERY_KEY)
            .replace("__DEFAULT_PAGE_SIZE__", &DEFAULT_PAGE_SIZE.to_string())
            .replace("__MAX_PAGE_SIZE__", &MAX_PAGE_SIZE.to_string())
    };
    let migration_path = format!("src/migrations/{migration}.rs");
    [
        ("src/domain/mod.rs", include_str!("persistence/domain.rs.in")),
        ("src/dto/mod.rs", include_str!("persistence/dto.rs.in")),
        ("src/entities/mod.rs", "pub(crate) mod record;\npub(crate) mod translation;\n"),
        ("src/entities/record.rs", include_str!("persistence/record.rs.in")),
        ("src/entities/translation.rs", include_str!("persistence/translation.rs.in")),
        ("src/error/mod.rs", include_str!("persistence/error.rs.in")),
        ("src/integrations/mod.rs", "pub(crate) mod cache;\npub(crate) mod index;\npub(crate) mod search;\n"),
        ("src/integrations/cache.rs", include_str!("persistence/cache.rs.in")),
        ("src/integrations/index.rs", include_str!("persistence/index.rs.in")),
        ("src/integrations/index/publication.rs", include_str!("persistence/index_publication.rs.in")),
        ("src/integrations/search.rs", include_str!("persistence/search.rs.in")),
        ("src/integrations/search/visibility.rs", include_str!("persistence/search_visibility.rs.in")),
        ("src/migrations/mod.rs", "mod __MIGRATION__;\n\npub(crate) use __MIGRATION__::Migration;\npub(crate) const ID: &str = stringify!(__MIGRATION__);\n"),
        (&migration_path, include_str!("persistence/migration.rs.in")),
        ("src/services/mod.rs", "mod record;\n\npub use record::__ENTITY__Service;\npub(crate) use record::resolve_public_discovery;\n// __ADMIN_EXPORTS__\n"),
        ("src/services/record/mod.rs", include_str!("persistence/service.rs.in")),
        ("src/services/record/commands.rs", include_str!("persistence/commands.rs.in")),
        ("src/services/record/list.rs", include_str!("persistence/list.rs.in")),
        ("src/services/record/publication.rs", include_str!("persistence/publication.rs.in")),
        ("src/services/record/queries.rs", include_str!("persistence/queries.rs.in")),
        ("src/services/record/settings.rs", include_str!("persistence/settings.rs.in")),
        ("src/services/record/repository/mod.rs", include_str!("persistence/repository.rs.in")),
        ("src/services/record/repository/list.rs", include_str!("persistence/list_repository.rs.in")),
        ("tests/record/main.rs", include_str!("persistence/contract.rs.in")),
        ("tests/record/list.rs", include_str!("persistence/list_contract.rs.in")),
        ("tests/record/discovery.rs", include_str!("persistence/discovery_contract.rs.in")),
        ("tests/record/settings.rs", include_str!("persistence/settings_contract.rs.in")),
        ("tests/record/search.rs", include_str!("persistence/search_fixture.rs.in")),
    ]
    .into_iter()
    .chain(options.admin_resource.as_ref().into_iter().flat_map(|_| [
        ("src/services/record/admin.rs", include_str!("persistence/admin.rs.in")),
        ("src/graphql/mod.rs", include_str!("persistence/graphql/mod.rs.in")),
        ("src/graphql/runtime.rs", include_str!("persistence/graphql/runtime.rs.in")),
        ("src/graphql/query.rs", include_str!("persistence/graphql/query.rs.in")),
        ("src/graphql/mutation.rs", include_str!("persistence/graphql/mutation.rs.in")),
        ("src/graphql/types.rs", include_str!("persistence/graphql/types.rs.in")),
        ("contracts/graphql/admin_record_fields.graphql", include_str!("persistence/graphql/admin_record_fields.graphql.in")),
        ("contracts/graphql/admin_records.graphql", include_str!("persistence/graphql/admin_records.graphql.in")),
        ("contracts/graphql/admin_record.graphql", include_str!("persistence/graphql/admin_record.graphql.in")),
        ("contracts/graphql/create_admin_record.graphql", include_str!("persistence/graphql/create_admin_record.graphql.in")),
        ("contracts/graphql/save_admin_record_title.graphql", include_str!("persistence/graphql/save_admin_record_title.graphql.in")),
        ("contracts/graphql/set_admin_record_publication.graphql", include_str!("persistence/graphql/set_admin_record_publication.graphql.in")),
        ("tests/record/graphql.rs", include_str!("persistence/graphql/contract.rs.in")),
    ]))
    .map(|(path, source)| {
        let source = render(source);
        let source = if let Some(resource) = &options.admin_resource {
            source
                .replace("__RESOURCE__", resource)
                .replace("// __ADMIN_ADAPTER__", "#[cfg(feature = \"admin\")]\nmod admin;\n#[cfg(feature = \"admin\")]\npub use admin::{admin_record_revision, admin_record_security};")
                .replace("// __ADMIN_EXPORTS__", "#[cfg(feature = \"admin\")]\npub use record::{admin_record_revision, admin_record_security};")
        } else {
            source
                .replace("// __ADMIN_ADAPTER__\n", "")
                .replace("// __ADMIN_EXPORTS__\n", "")
        };
        (PathBuf::from(path), source)
    })
    .collect()
}

pub(super) fn documentation(
    options: &ScaffoldOptions,
    entity: &str,
    module: &str,
    ui: &str,
) -> (String, String, String, String) {
    let name = crate::manifest_validation::to_pascal_case(entity);
    let package = options.crate_name();
    let slug = &options.slug;
    let ffa_status = if options.has_record_admin() {
        "in_progress"
    } else {
        "not_started"
    };
    let graphql_doc = options.admin_resource.as_ref().map_or_else(String::new, |resource| {
        format!(
            "\n## Authenticated admin GraphQL\n\nThe server's module feature selects the `graphql` contribution. Its namespaced list, detail, draft, title edit and publication operations use `Resource::{resource}` with the required action. Tenant and actor come from authenticated request context. The host provides the module settings reader. Use the [operation documents](./contracts/graphql/) with their shared `admin_record_fields.graphql` fragment for client bindings. Edit revisions are decimal strings.\n"
        )
    });
    let readme = format!(
        r#"# {package}

{}

## Purpose
Own localized {entity} titles and record revisions.

## Entry points
- `{module}` registers the owner and its migrations.
- `{name}Service` exposes embedded draft creation, localized reads, paged admin listing, title edits, publication and withdrawal.

## Documentation
- [Owner contract](./CRATE_API.md)
- [Architecture](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
"#,
        options.description
    );
    let docs = format!(
        r#"# {name} owner architecture

## Purpose

Store tenant-owned records with localized titles and explicit edit revisions.

## Responsibility Zone

- `src/domain/` validates the owner's locale and title.
- `src/entities/` and `src/migrations/` own the schema and tenant relations.
- `src/services/record/` owns commands, queries and the private repository.
- `src/integrations/search.rs` maps published owner copy to the Search source contract.
- `src/integrations/index.rs` supplies exact locale copies and owner revisions to relational Index replay and live refresh.
- `src/integrations/index/publication.rs` supplies the exact published-locale and revision predicate used by Index pagination and counts.
- `src/integrations/cache.rs` binds the host-selected Cache backend to a revision-scoped published view.
- `tests/record/` applies platform identity, Outbox and owner migrations and verifies the PostgreSQL Search path when selected.

## Integration

{ui}

Compose the service with the owner permission resource. A public transport derives tenant and authorization from platform context and checks the resolved channel. Storefront Search admission is configured by its host. Distribution composes the Index source, listener and query admission from the owner declaration. Index applies current publication, revision and the tenant discovery setting before pagination and counting.
An embedding host can call `with_public_cache` with its shared service and a bounded backend. Each hit is gated by the current database publication and owner revision. Keys include requested and fallback locales.
Use `with_settings_reader` with the platform static settings reader to apply the tenant's `admin_page_size` to lists without an explicit page size.
Public record reads also use that reader. The snapshot must explicitly enable this optional module and set `public_discovery_enabled` to `true` before a published record can be read or returned from cache.

## Verification

Run `cargo test -p {package} --test record`.

## Related Documentation

See the [platform integration guide](../../../../docs/backend/module-backend-implementation.md#connecting-platform-capabilities) and [owner plan](./implementation-plan.md).
"#
    );
    let plan = format!(
        r#"# {package} implementation plan

## Current state
Localized {entity} persistence, publication, Outbox and sources for Search and relational Index are implemented. Run the generated integration tests against the actual module and platform migrations.

## FFA/FBA status

- FFA status: `{ffa_status}`
- FBA status: `not_started`
- Evidence: {ui}
- Last verified at (UTC): not verified
- Owner: {slug} module owner.

## Release and Data Rollback Readiness

- Runtime kind: Optional
- Rollback unit: Component
- Data boundary owner: `{slug}`
- Native migrations: owner record and translation tables
- Supported migration policy: additive baseline, production execution unverified
- Predecessor standby strategy: none
- Rollback eligibility: `DeniedMaintenanceOnly`
- Recovery invariants: one owner transaction covers revision, localized copy and required Outbox events.

## Verification

- `cargo xtask module lint {slug}`
- `cargo xtask module validate {slug}`
- `cargo test -p {package} --test record`

Record PostgreSQL migration, Search and Index delivery evidence before production use. Compose trusted tenant and principal context before attaching a published transport.
"#
    );
    let api = format!(
        r#"# {package} / CRATE_API

## Embedded owner service

`{name}Service` accepts a trusted tenant, an explicit write locale and title, and a predecessor revision on edits. Its constructor requires the permission resource governing the record. Privileged commands and reads require tenant-wide authority. The service returns the applied or resolved copy with requested, effective and available locales.

## Platform Capability Bindings

The module uses platform locale policy, RBAC, SeaORM migrations, transactional Outbox, an optional host-bound public view cache and module-owned sources for Search and relational Index.

## Domain Collaborations

Platform identity owns tenant records. The generated owner owns record identity and localized titles.

## Minimum Contract Set

### Input DTOs/Commands

`Save{name}Title` contains a required locale and title. Use `create` for a draft, `save_title` for explicit-locale edits, `set_publication` for lifecycle changes, and `get` or `get_published` for reads.

`List{name}Records` accepts a one-based page, optional page size, publication filter, order by update time, and literal title search. The service searches the stored titles in every locale, then returns the requested or fallback title for each result. The total covers the full filtered tenant scope. Count and rows share one read snapshot. An explicit page size overrides the tenant setting.

`get_published` checks `public_discovery_allowed` before reading records or cache entries. The check requires an explicit enablement intent and a true `public_discovery_enabled` setting from the same static settings snapshot. Missing configuration denies public access. A read error or malformed enabled policy is reported to the caller.

### Domain Invariants

The base identity is locale-neutral. Localized writes never infer a locale from a fallback read. A stale revision or foreign tenant cannot mutate the owner.

### Events / Outbox Side Effects

Publication, withdrawal and edits to published copy append the Search and sealed Index refresh requests in the owner transaction. Search loads published locale copies and removes the entity from its corpus on withdrawal. Index reads all exact locale copies from one owner snapshot and persists revisioned tombstones for withdrawals. Live refresh and replay share mutation identities. Storefront corpus admission belongs to the host.

### Errors / Failure Codes

`RecordError` distinguishes invalid input, denial, missing records, revision conflicts, database and Outbox failures. Future transports map owner errors into their typed public envelope.
{graphql_doc}
"#
    );
    (readme, docs, plan, api)
}
