---
id: doc://docs/architecture/database.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Data Schema

This document provides a top-level map of RusToK write-side and read-side schemas.
It does not replace migrations, entities and local docs of modules.

## Sources of Truth

Canonical source of truth for DB structure:

- migrations in `rustok-migrations` and module-owned migration sources
- SeaORM entities in `apps/server/src/models/_entities` and module crates
- local docs of modules for module-owned storage contracts

This document is only needed as a central summary layer.

## Common Invariants

- `tenant_id` remains the main isolation boundary for platform and module data
- write-side tables are considered source of truth for domain state
- denormalized index/read tables are not considered source of truth
- JSONB is allowed for settings, config and flexible payloads, but not as the final
  canonical form for localized business text

## Multilingual Storage Contract

The current target platform pattern:

- base business tables store language-agnostic state
- localized short texts live in `*_translations`
- heavy localized content may live in `*_bodies`
- tenant locale policy manages effective locale and fallback, but not ownership
  of localized fields
- locale storage follows a single normalized contract with a safe width of
  `VARCHAR(32)`
- widening locale columns to `VARCHAR(32)` is considered a safe forward migration:
  rollback must not narrow such columns back and risk losing valid BCP47-like tags

If an old module uses a mixed storage pattern, this is considered a
migration target, not a desired baseline.

## Foundation Tables

Foundation storage includes:

- `tenants`
- `users`
- `sessions`
- `install_sessions`
- `install_step_receipts`
- `platform_settings`
- `tenant_modules`
- `tenant_locales`
- `oauth_apps`
- `sys_events`

### What Matters Here

- `tenants` and `tenant_locales` define the tenant and locale policy layer
- `sessions` and auth-related tables support the auth/session lifecycle
- `install_sessions` and `install_step_receipts` capture resumable installer
  state, input checksums, outcomes and diagnostics; secrets are not stored there
- `platform_settings` and `tenant_modules` store platform/module settings
- `sys_events` remains a transactional outbox table, not a generic audit dump

## Installer Storage

The hybrid installer uses `rustok-installer` as a support crate for
typed plan/state/receipt contracts. Its persistence in `apps/server` must
follow these rules:

- `install_sessions` stores session-level state, profile, environment,
  redacted plan snapshot and current status;
- `install_step_receipts` stores step outcome, input checksum, installer
  version, timestamp and diagnostic payload;
- final installed/adopted marker and deployment metadata can be reflected in
  `platform_settings` under the `installer` category;
- plaintext secrets are not written to installer tables; only redacted
  values or references to a secret backend are allowed;
- production installs require explicit PostgreSQL engine, while SQLite remains
  a local/demo/test mode.

## RBAC Tables

RBAC source of truth lives in relation tables:

- `roles`
- `permissions`
- `user_roles`
- `role_permissions`

They support the permission/runtime contract and must not be duplicated in
alternative ownership tables without a clear architectural reason.

`rustok-rbac::RbacModule` owns the migrations that enforce tenant integrity
across these relations and persist the durable permission-invalidation
generation. The platform migrator composes those module-owned migrations.

## Auth and OAuth Storage

`rustok-auth` owns OAuth applications, authorization codes, refresh tokens,
consents, and invite-consumption audit links. Relations from codes, tokens, and
consents to applications and users are tenant-composite database constraints;
invite-consumption user links enforce the same tenant equality while preserving
their delete-to-null audit behavior.

OAuth protocol tables are an explicit RLS exception because an unauthenticated
request must resolve a globally unique `client_id` before a tenant context exists.
After resolution, database relations and server queries remain tenant-composite,
and app state, grant, scope, consent, user, RBAC, and session checks fail closed.

The current inline `oauth_apps.name` and `oauth_apps.description` columns remain
an open multilingual-contract gap. The auth implementation plan owns their atomic
cutover to a translation table across storage and all transports; no JSON or
transport-local translation fallback is allowed.

## Module Artifact Control-Plane Storage

`rustok-modules` owns tenant-scoped artifact installation, lifecycle, data,
secret, scheduling, delivery, and binding-operation persistence. Tenant-bound
tables use explicit tenant predicates plus PostgreSQL RLS. In particular,
`module_artifact_binding_operations` contains durable idempotency request and
response data, so claim, completion, abandonment, replay, and recovery must run
inside a transaction with `rustok.tenant_id` configured. A caller-supplied
`tenant_id` predicate alone is not considered a sufficient isolation boundary.

## Content-family Storage

The current content baseline is built around:

- `nodes`
- `node_translations`
- `bodies`

Principle:

- `nodes` owns language-agnostic state
- `node_translations` owns localized short fields
- `bodies` owns heavy localized content

## Commerce-family Storage

Commerce storage remains a split-domain family, but the top-level base consists of:

- `products`
- `product_translations`
- `product_variants`
- `variant_translations`
- `prices`
- `product_images`
- `product_options`
- `cart_line_item_translations`
- `order_line_item_translations`

Native product catalog attributes extend this baseline through
product-owned tables:

- `product_attributes`, `product_attribute_translations`, `product_attribute_options`
- `catalog_categories`, `catalog_category_translations`, `catalog_category_closure`
- `product_attribute_schemas` and schema/group/binding tables
- `category_attribute_schema_assignments`, `category_attributes`, `category_attribute_groups`
- `product_categories`, `virtual_category_product_assignments`
- `product_attribute_values`, `product_variant_attribute_values` and related localized value/option tables

And the same principle applies: base rows are language-agnostic, localized
fields are moved to parallel records.

## Registry / Marketplace

Registry storage for publish/governance is also aligned with the same multilingual contract:

- `registry_publish_requests`
- `registry_publish_request_translations`
- `registry_module_releases`
- `registry_module_release_translations`
- `registry_module_owners`
- `registry_governance_events`

Principle:

- base rows publish/release hold language-agnostic state, typed principals, status,
  artifact storage keys and `default_locale`
- module display metadata (`name`, `description`) lives in dedicated translations tables,
  not in base rows
- ownership and governance audit trail remain language-agnostic persistence
- `registry_governance_events.details` is considered an internal audit payload, not a canonical
  multilingual business copy

## Flex

`flex` is a capability slice, but it follows the same storage contract.

The current live baseline includes:

- `flex_schemas`
- `flex_schema_translations`
- `flex_entries`
- `flex_attached_localized_values`

Current-state conclusion:

- schema-level language-agnostic state lives in base tables
- localized schema copy lives in translations tables
- attached localized values are moved to dedicated locale-aware storage
- cleanup/backfill of legacy inline localized payloads must happen through migrations,
  not through constant runtime fallback to base-row JSON

## Index/Read-side Tables

`rustok-index` owns denormalized read models, for example:

- `index_content`
- `index_products`
- `index_product_categories`
- `index_product_attribute_values`

They exist for query/index/search paths and must not be used as
authoritative write-side storage.

The product indexer updates category and attribute projections by `tenant_id`,
`product_id` and explicit locale. Attribute projection takes the effective schema from
the read-only resolver of `rustok-product`, splits multiselect into individual
option rows and does not index detached values. Localized labels do not use a
package-local fallback chain.

Facet/search/sort rows have channel scope. For active channels, the indexer creates
a separate set of rows and computes flags with priority `attribute defaults <
schema/category overrides < channel settings`; explicit `false` is not lost during
inheritance. The row also stores effective storefront/comparison/admin-grid
visibility. If no active channel exists, a single global scope is used with `channel_id = null`. Read model indexes include `channel_id`, so
search does not mix facet buckets from different channels.

Before updating category projection, the indexer recalculates
`virtual_category_product_assignments` according to bounded V1 rules. Materialization
is idempotent at the `(tenant_id, product_id)` level: old product rows are deleted,
then matched virtual categories are written in a single transaction. Rules
use only write-side product facts and effective locale-neutral attributes;
the read model does not become the source of truth for rule evaluation.

## Workflow Storage

`rustok-workflow` owns its own module storage:

- `workflows`
- `workflow_steps`
- `workflow_executions`
- `workflow_step_executions`
- `workflow_versions`

This is a module-owned schema, not a generic platform queue layer.

## Media and Storage Layer

Media metadata remains module-owned, while file bytes are handled through a shared
storage runtime.

Base media tables:

- `media`
- `media_translations`

Storage backend configuration lives not in per-file SQL contract, but in
typed runtime settings.

## What Not To Do

- do not consider a summary document as a replacement for migrations
- do not use read-side tables as write-side authority
- do not store localized business text in base rows if the module already follows the
  parallel-localized-record path
- do not blur ownership of module-owned tables between host and module crate

## Related Documents

- [Module Architecture](./modules.md)
- [Domain Event Flow Contract](./event-flow-contract.md)
- [i18n Architecture](./i18n.md)
- [Module and Application Registry](../modules/registry.md)
