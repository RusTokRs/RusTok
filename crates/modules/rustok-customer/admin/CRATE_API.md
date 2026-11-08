# Crate API: rustok-customer-admin

## Public Surface

### UI Components
- `CustomerAdmin`: Root Leptos administration component for customer operations.

### Core
- `customer_grid_columns`: Column definitions for the customer directory table.
- `filter_customers`: In-memory filtering helper for customer records.

### Models
- `CurrentTenant`: Tenant identity model (`id`, `slug`, `name`).
- `CustomerAdminBootstrap`: Bootstrap state for customer admin (`current_tenant`).
- `CustomerList`: Paginated customer list (`items`, `total`, `page`, `per_page`, `has_next`).
- `CustomerListItem`: Summary representation of a customer record.
- `CustomerDetail`: Customer record plus optional profile bridge summary.
- `CustomerRecord`: Detailed customer entity record.
- `CustomerProfileRecord`: Bridged profile information (`handle`, `display_name`, `preferred_locale`, `visibility`, `tags`).
- `CustomerDraft`: Customer input payload (`user_id`, `email`, `first_name`, `last_name`, `phone`, `locale`).

### Transport
- `fetch_bootstrap()`: Fetches current tenant bootstrap context.
- `fetch_customers(search, page, per_page)`: Fetches paginated customer list.
- `fetch_customer_detail(customer_id)`: Fetches customer record and linked profile.
- `create_customer(payload)`: Creates new customer in tenant.
- `update_customer(customer_id, payload)`: Updates customer record in tenant.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-customer`.
- Grid integration via `rustok-grid` and `rustok-grid-leptos`.
- Internationalization via `rustok-ui-i18n` with package-owned translations (`en`, `ru`).
- Native server functions via `HostRuntimeContext`.

## Domain Collaborations
- Collaborates with `rustok-customer` backend service for customer lifecycle operations.
- Interacts with `rustok-profiles` for extended profile attribute enrichment.
- Consumed by `apps/admin` through manifest-driven composition (`rustok-module.toml`).

## Minimum Contract Set

### Input DTOs/Commands
- `CustomerAdmin`: Entrypoint workspace component for customer administration.
- `CustomerAdminDraftInput`: Form draft input for customer editor.
- `CustomerAdminSubmitCommand`: Validated submission command with effective locale.
- `CustomerListRequest`: Customer list pagination and search request parameters.

### Domain Invariants
- Customer email addresses and identifiers must preserve tenant boundary constraints.
- Zero business logic inside UI view templates; filtering, view-models, and validation live in `core`.
- Mutations execute through native server functions with proper authorization context.
- All model structs and `ApiError` derive `Clone`, `Debug`, `PartialEq`, and `Eq`.

### Events / Outbox Side Effects
- Emits customer profile updates and lifecycle state modifications.
- Read operations are side-effect free.

### Errors / Failure Codes
- `CustomerAdminSubmitCommandError::EmailRequired`: Email cannot be empty.
- `CustomerAdminSubmitCommandError::LocaleUnavailable`: Effective host locale must be present.
- `ApiError`: Transport failure wrapper for server function calls.
