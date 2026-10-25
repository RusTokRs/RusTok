# rustok-billing / CRATE_API

## Role

`rustok-billing` owns the sovereign domain capability for B2B Billing & Settlements in RusToK.

Persistence entities are **crate-private**. Consumers interact exclusively through owner DTOs,
services, or explicit ports. Direct cross-crate SeaORM table access is forbidden.

## Public surface

Deliberate public modules and exports:

- `domain` — `BillingStatus` lifecycle state;
- `dto` — owner command, input, and response data contracts;
- `error` — `BillingError`, `BillingResult`;
- `events` — domain event descriptors;
- `integrations` — capability bridges for search, cache, media, translation;
- `module` — `BillingModule` lifecycle and migration provider;
- `services` — `BillingService` and transaction orchestration.

`entities` and `migrations` are crate-private.

## Primary entry points

- [`BillingModule`](src/module.rs): Platform module registration and migration descriptor.
- [`BillingService`](src/services/mod.rs): Domain application service.

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
- `BillingCreateInput` and `BillingUpdateInput` data transfer objects.

### Domain Invariants
- Tenant isolation: All queries and commands strictly enforce `tenant_id` scoping.
- Concurrency: Write operations execute within explicit transaction boundaries.

### Events / Outbox Side Effects
- Domain mutations record transactional outbox events via `rustok-outbox`.

### Errors / Failure Codes
- Strongly-typed `BillingError` with structured failure categories.
