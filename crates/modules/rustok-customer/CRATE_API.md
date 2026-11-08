# rustok-customer / CRATE_API

## Public Modules
- `dto`: Input and response transfer objects for customer operations (`CreateCustomerInput`, `UpdateCustomerInput`, `ListCustomersInput`, `CustomerResponse`, `CustomerWithProfileResponse`).
- `entities`: SeaORM entity models for customer database storage (`Model`, `Entity`, `ActiveModel`).
- `error`: Customer domain errors (`CustomerError`, `CustomerResult`).
- `migrations`: SeaORM migration definitions for customer schemas (initial creation, identity enforcement, locale expansion).
- `module`: Core `CustomerModule` definition and platform registration.
- `ports`: Transport-neutral read projection traits and query requests (`CustomerReadPort`, `CustomerProjectionRequest`, `CustomerUserProjectionRequest`, `CustomerListProjectionRequest`, `CustomerListProjectionResponse`, `CustomerProfileEnrichmentRequest`, `CustomerProfileEnrichment`).
- `services`: Domain business logic and mutation services (`CustomerService`, `InProcessCustomerReadPort`).

## Platform Capability Bindings
- `CustomerModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `customer`.
- Permissions: `CUSTOMERS_CREATE`, `CUSTOMERS_READ`, `CUSTOMERS_UPDATE`, `CUSTOMERS_DELETE`, `CUSTOMERS_LIST`, `CUSTOMERS_MANAGE`.

## Domain Collaborations
- Default storefront customer submodule of the ecommerce family.
- Separates customer identity from admin/runtime users while allowing tenant-scoped optional linkage by `user_id`.
- Provides `CustomerReadPort` as transport-neutral read-projection provider for commerce checkout and order customer snapshots.
- Implements `InProcessCustomerReadPort` with retained local outcome diagnostics and policy admission enforcement.
- Optional profile enrichment via `rustok-profiles` integration.
- Leptos admin package in `admin/` (`rustok-customer-admin`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreateCustomerInput`: email (validated, 3..255 chars), optional user_id, optional first/last name, phone, locale, metadata JSON.
- `UpdateCustomerInput`: optional email, first/last name, phone, locale, metadata JSON.
- `ListCustomersInput`: search query, page, per_page pagination parameters (per_page clamped 1..=100).
- `CustomerProjectionRequest`: customer_id query for read projection.
- `CustomerUserProjectionRequest`: user_id query for customer projection.
- `CustomerListProjectionRequest`: search, page, and per_page (validated 1..=100).
- `CustomerProfileEnrichmentRequest`: user_ids list for profile summary enrichment (bounded to 500).

### Domain Invariants
- Email is normalized (trimmed, canonicalized) before uniqueness checks and persistence.
- Unique constraint on `(tenant_id, email)` prevents duplicate customer records within a tenant.
- Optional `user_id` linkage: at most one customer record per `(tenant_id, user_id)` pair.
- Cross-tenant customer isolation is strictly maintained across all reads and mutations; nil `tenant_id` is rejected.
- Customer identity remains distinct from platform user records.
- Both `CustomerService` and `InProcessCustomerReadPort` implement `Clone` and `std::fmt::Debug`.
- `CustomerResponse`, `CustomerWithProfileResponse`, and `CustomerListProjectionResponse` implement `PartialEq`.

### Events / Outbox Side Effects
- Emits customer lifecycle domain events upon creation, update, and deletion.
- In-process `CustomerReadPort` projections are read-only with zero mutation side effects.

### Errors / Failure Codes
- `CustomerError::Validation`: email format, string lengths, or nil UUID guards (`customer.validation_failed`).
- `CustomerError::CustomerNotFound`: customer with given UUID does not exist in tenant (`customer.not_found`).
- `CustomerError::CustomerByUserNotFound`: customer linked to given user UUID not found (`customer.user_not_found`).
- `CustomerError::DuplicateEmail`: customer with email already exists in tenant (`customer.duplicate_email`).
- `CustomerError::DuplicateUserLink`: user already linked to an existing customer in tenant (`customer.duplicate_user_link`).
- `CustomerError::Profile`: errors propagated from profile enrichment (`customer.profile_error`).
- `CustomerError::Database`: underlying storage failures (`customer.database_error`).
