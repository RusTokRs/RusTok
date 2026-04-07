# RusToK Database Schema

> Current-state schema map for the main platform tables and major module-owned schemas.  
> Updated: 2026-04-05

This document is a high-level guide, not the canonical migration source. Source of truth remains:

- SeaORM entities under `apps/server/src/models/_entities` and module crates;
- migrations in `apps/server/migration` and module-owned migration sources;
- module docs for storage/index/workflow-specific schemas.

---

## Foundation Tables

## Multilingual Storage Contract

RusToK now treats multilingual storage as an explicit architecture contract, not a module-by-module convention.

The target pattern is:

- base business tables store only language-agnostic state;
- localized short text lives in parallel `*_translations` records;
- heavy localized content may live in dedicated `*_bodies` records keyed by owner + locale;
- tenant locale policy (`tenants.default_locale`, `tenant_locales`) controls effective locale selection and fallback, not physical ownership of localized fields;
- locale storage follows the same normalized BCP47-like contract as runtime and should use `VARCHAR(32)` as the platform-safe width.

This is already the live direction for platform foundation and content-family storage. Some older module slices still contain legacy mixed patterns and should be treated as migration targets, not as the desired end-state.

### `tenants`

Platform tenant registry.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `name` | TEXT/VARCHAR | Display name |
| `slug` | VARCHAR | Stable tenant slug |
| `domain` | VARCHAR nullable | Optional host/domain binding |
| `settings` | JSONB | Tenant-scoped opaque settings |
| `default_locale` | VARCHAR(32) | Default locale used by request fallback chain and tenant locale policy |
| `is_active` | BOOL | Tenant activity flag |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `updated_at` | TIMESTAMPTZ | Last update timestamp |

### `users`

Tenant-scoped user identity table.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `tenant_id` | UUID | FK to tenant |
| `email` | VARCHAR | Login/identity email |
| `password_hash` | VARCHAR | Password hash |
| `name` | VARCHAR nullable | Display name |
| `status` | ENUM/text | Account status |
| `email_verified_at` | TIMESTAMPTZ nullable | Email verification timestamp |
| `last_login_at` | TIMESTAMPTZ nullable | Last login timestamp |
| `metadata` | JSONB | Additional profile metadata |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `updated_at` | TIMESTAMPTZ | Last update timestamp |

### `sessions`

Auth/session lifecycle table.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `tenant_id` | UUID | FK to tenant |
| `user_id` | UUID | FK to user |
| `token_hash` | VARCHAR | Refresh/session token hash |
| `ip_address` | VARCHAR nullable | Source IP |
| `user_agent` | TEXT/VARCHAR nullable | User agent |
| `last_used_at` | TIMESTAMPTZ nullable | Last use timestamp |
| `expires_at` | TIMESTAMPTZ | Expiration timestamp |
| `revoked_at` | TIMESTAMPTZ nullable | Soft-revoke marker |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `updated_at` | TIMESTAMPTZ | Last update timestamp |

### `platform_settings`

Per-tenant platform configuration overrides.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `tenant_id` | UUID | FK to tenant |
| `category` | VARCHAR | Category such as `email`, `rate_limit`, `events`, `oauth` |
| `settings` | JSONB | Stored category payload |
| `schema_version` | INT | Schema version for validation/migration |
| `updated_by` | UUID nullable | User who last updated the record |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `updated_at` | TIMESTAMPTZ | Last update timestamp |

### `tenant_modules`

Per-tenant module toggle and module-scoped settings.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `tenant_id` | UUID | FK to tenant |
| `module_slug` | VARCHAR | Module identifier |
| `enabled` | BOOL | Runtime enablement flag |
| `settings` | JSONB | Module-owned opaque settings |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `updated_at` | TIMESTAMPTZ | Last update timestamp |

### `tenant_locales`

Tenant-scoped locale availability and fallback policy.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `tenant_id` | UUID | FK to tenant |
| `locale` | VARCHAR(32) | Canonical locale supported by the tenant |
| `name` | VARCHAR | Human-readable locale name |
| `native_name` | VARCHAR | Native language name |
| `is_default` | BOOL | Whether this locale is the tenant default |
| `is_enabled` | BOOL | Whether the locale can be selected at runtime |
| `fallback_locale` | VARCHAR(32) nullable | Optional tenant-local fallback |
| `created_at` | TIMESTAMPTZ | Creation timestamp |

### `oauth_apps`

Tenant-scoped OAuth application registry for provider/client management.

This table exists in the live schema and is used by GraphQL OAuth admin flows.

### `sys_events`

Transactional outbox and delivery state.

| Column | Type | Description |
|--------|------|-------------|
| `id` | UUID | Primary key |
| `payload` | JSONB | Serialized event envelope |
| `status` | VARCHAR | Delivery state (`pending`, `dispatched`, `failed`, etc.) |
| `created_at` | TIMESTAMPTZ | Creation timestamp |
| `dispatched_at` | TIMESTAMPTZ nullable | Dispatch timestamp |

---

## RBAC Tables

RBAC source of truth remains relation tables:

- `roles`
- `permissions`
- `user_roles`
- `role_permissions`

These tables back the Casbin-only runtime through resolver/adapters; they remain authoritative for permission data.

---

## Content and Commerce Tables

### Content

Core content schema remains centered around:

- `nodes`
- `node_translations`
- `bodies`

This is the current canonical storage direction for multilingual content:

- `nodes` owns language-agnostic state;
- `node_translations` owns localized short text and metadata;
- `bodies` owns localized heavy content keyed by `(node_id, locale)`.

### Commerce

Core commerce schema remains centered around:

- `products`
- `product_translations`
- `product_variants`
- `variant_translations`
- `prices`
- `product_images`
- `product_options`

Commerce is still mid-migration in parts of the repo, but the platform target is the same: base entities stay language-agnostic, localized short text lives in `*_translations`, and rich localized content should not be privileged in base rows.

### Flex

`flex` is a capability / ghost-module slice, but it is not exempt from the multilingual storage contract.

Current live server-owned standalone storage is centered around:

- `flex_schemas`
- `flex_schema_translations`
- `flex_entries`
- `flex_entry_localized_values`
- `flex_attached_localized_values`

Current direction:

- `flex_schemas` owns language-agnostic schema state such as `slug`, `fields_config`, `settings`, and activation flags;
- `flex_schema_translations` owns localized schema copy such as `name` and `description`;
- `flex_entries` now owns only language-agnostic standalone entry state plus shared/non-localized entry payload fields;
- `flex_entry_localized_values` owns standalone locale-aware entry payloads by `(entry_id, locale)` and is backfilled from older inline `flex_entries.data` rows;
- `flex_attached_localized_values` owns attached-mode locale-aware field values by `(tenant_id, entity_type, entity_id, field_key, locale)` and its shared entity/helpers now live in `crates/flex`;
- read/write paths now resolve standalone entry payloads by merging `flex_entries.data` with the selected row from `flex_entry_localized_values`, while any residual inline localized keys in `flex_entries.data` are treated only as transitional fallback during cleanup/backfill.

Attached-mode `flex` field-definition localization currently still uses JSON locale maps in field-definition rows; this remains transitional and should converge toward parallel localized records in later slices.
Field-definition rows now also carry explicit `is_localized` semantics, so attached-mode custom fields no longer depend on an implicit convention when deciding whether future values belong to locale-aware parallel storage.
Live attached-value read/write paths now use `flex_attached_localized_values` for `user`, `product`, `order`, and `topic` locale-aware custom values. `topic` now uses `forum_topics.metadata` as the donor payload surface, so it follows the same attached multilingual contract instead of remaining a schema-only consumer. Legacy localized keys that still live inline in donor `metadata` or pre-split standalone `flex_entries.data` rows are treated as transitional fallback data during read/update until cleanup slices finish the migration.

---

## Index Tables

`rustok-index` owns the denormalized read models used by the CQRS read path.

### `index_content`

Implemented and maintained by content indexers.

Representative columns:

- `tenant_id`
- `node_id`
- `locale`
- `kind`
- `slug`
- `title`
- `excerpt`
- `search_vector`
- `indexed_at`

### `index_products`

Implemented and maintained by product indexers.

Representative columns:

- `tenant_id`
- `product_id`
- `locale`
- `handle`
- `title`
- `description`
- `price_min`
- `price_max`
- `attributes`
- `search_vector`
- `indexed_at`

---

## Workflow Tables

`rustok-workflow` owns its own module tables and they are implemented in the live schema:

- `workflows`
- `workflow_steps`
- `workflow_executions`
- `workflow_step_executions`
- `workflow_versions`

Notable runtime fields include:

- workflow trigger config (`trigger_config`)
- failure tracking (`failure_count`, `auto_disabled_at`)
- webhook trigger support (`webhook_slug`, `webhook_secret`)
- execution context and step I/O payloads (`context`, `input`, `output`)

---

## Media and Storage

Media metadata is module-owned while file bytes are handled through the shared storage runtime.

Key media tables:

- `media`
- `media_translations`

Storage backend configuration is not modeled as per-file SQL schema; it is runtime-configured through typed settings and `platform_settings`.

---

## Notes

- `tenant_id` remains the primary isolation boundary for platform and module data.
- JSONB is used intentionally for module/platform settings, workflow configuration, and flexible metadata.
- JSONB is not the canonical long-term owner for multilingual business text when a module is under the parallel-localized-record migration path.
- Read-model tables are denormalized on purpose and should not be treated as authoritative write-side state.
- For exact column/index/constraint details, prefer module migrations and generated entities over this summary doc.

---

## See Also

- [i18n.md](./i18n.md)
- [rbac.md](./rbac.md)
- [events.md](./events.md)
- [workflow.md](./workflow.md)
- [modules.md](./modules.md)
