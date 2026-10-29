# rustok-billing-admin / CRATE_API

## Public surface

### Entry Point & Admin Pages
- `BillingAdmin`: root administration component for B2B billing and settlements.
- `BillingAdminPage`: page component hosting settlements and invoice grids.

### Core Models & Logic
- `BillingAdminTableRow`: view model representing billing records and action states.
- `filter_rows`: in-memory search and filter predicate for billing table rows.
- `BillingAdminTransport`: transport facade managing backend communication.

## Platform Capability Bindings
- **Admin Control Plane**: Registers `BillingAdmin` within administrator workspaces.
- **Fluid Frontend Architecture**: Enforces clean boundary between Leptos components and pure Rust models in `core`.
- **Settlement & Invoicing Management**: Manages commercial billing accounts, invoices, and settlement statuses.

## Domain Collaborations
- **`rustok-billing`**: Core domain module providing billing accounts, invoices, and ledger calculations.
- **`rustok-grid-leptos`**: Data grid adapter presenting billing records with pagination and sorting.
- **`rustok-ui-core`**: Core UI layout and accessibility foundations.
- **`rustok-ui-i18n`**: Fluent translation catalogs (`en.ftl`, `ru.ftl`) synchronized with host locale context.

## Minimum Contract Set

### Input DTOs/Commands
- `BillingAdminTableRow`: constructor parameters (`id`, `title`, `status`, `created_at`).
- `filter_rows`: filter predicate inputs (`rows`, `query`).

### Domain Invariants
- **Non-Swallowed Errors**: Billing and transport errors must be surfaced to administrators rather than hidden.
- **Financial Precision**: Monetary amounts and line items reflect authoritative figures from `rustok-billing`.
- **Auditable Status Transitions**: Invoices and settlement state changes follow strict accounting invariants.

### Events / Outbox Side Effects
- Dispatches settlement reconciliations and invoice modifications through the backend transport facade.

### Errors / Failure Codes
- `UiError`: presentation and input validation error states.
- `BillingTransportError`: network and RPC failure codes for billing operations.
