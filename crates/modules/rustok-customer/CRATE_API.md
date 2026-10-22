# rustok-customer / CRATE_API

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
- Optional profile enrichment via `rustok-profiles` integration.
- Leptos admin package in `admin/` (`rustok-customer-admin`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreateCustomerInput`: email (validated, 3..255 chars), optional user_id, optional first/last name, phone, locale, metadata JSON.
- `UpdateCustomerInput`: optional email, first/last name, phone, locale, metadata JSON.
- `ListCustomersInput`: search query, page, per_page pagination parameters.
- `CustomerProjectionRequest`: customer_id query for read projection.
- `CustomerUserProjectionRequest`: user_id query for customer projection.
- `CustomerListProjectionRequest`: search and page query.
- `CustomerProfileEnrichmentRequest`: user_ids list for profile summary enrichment.

### Domain Invariants
- Email is normalized (trimmed, lowercased) before uniqueness checks and persistence.
- Unique constraint on `(tenant_id, email)` prevents duplicate customer records within a tenant.
- Optional `user_id` linkage: at most one customer record per `(tenant_id, user_id)` pair.
- Cross-tenant customer isolation is strictly maintained across all reads and mutations.
- Customer identity remains distinct from platform user records.

### Events / Outbox Side Effects
- Emits customer lifecycle domain events upon creation, update, and deletion.
- In-process `CustomerReadPort` projections are read-only with zero mutation side effects.

### Errors / Failure Codes
- `CustomerError::Validation`: email format, string lengths, or invalid input data.
- `CustomerError::CustomerNotFound`: customer with given UUID does not exist in tenant.
- `CustomerError::CustomerByUserNotFound`: customer linked to given user UUID not found.
- `CustomerError::DuplicateEmail`: customer with email already exists in tenant.
- `CustomerError::DuplicateUserLink`: user already linked to an existing customer in tenant.
- `CustomerError::Profile`: errors propagated from profile enrichment.
- `CustomerError::Database`: underlying storage failures.
