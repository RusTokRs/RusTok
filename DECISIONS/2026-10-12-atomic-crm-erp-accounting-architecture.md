# Sovereign CRM, ERP, Finance, and Accounting Domain Architecture for RusToK

- Date: 2026-10-12
- Decision status: Accepted
- Implementation status: Not started
- Owners: platform-architecture
- Extends: [Commerce module split: product, pricing, inventory](./2026-03-25-commerce-module-split-product-pricing-inventory.md), [Checkout operation invariants owned by typed Rust](./2026-10-07-checkout-operation-invariants-owned-by-rust.md), [Canonical money owner](./2026-10-07-canonical-money-owner.md)
- Supersedes: None
- Superseded by: None

## Context

RusToK is evolving toward a high-performance, modular SuperApp platform serving commerce, operational, and back-office workloads across web and Flutter mobile runtimes. A major functional requirement is providing end-to-end CRM (customer relationship management, leads, deals, pipelines, interaction activities), ERP (warehouse topology, inventory movements, procurement, billing, waybills), Operational Finance (bank accounts, cash registers, statement import, cash flows), and Accounting (universal chart of accounts, double-entry bookkeeping, tax accounting, and settlement reconciliation).

These operational workflows are deeply intertwined:
- a CRM deal closes and creates a customer order and commercial billing invoice;
- an order fulfillment triggers warehouse picking, reservation, and shipping waybills;
- shipping waybills generate inventory reduction in warehouse ledgers and revenue/cost-of-goods-sold postings in accounting;
- cash or bank payments settle invoices and create cash-flow ledger postings.

Two extreme implementation models were evaluated and rejected:
1. *A monolithic 1C-like dynamic configurator:* Implementing an interpretation engine over dynamic schemas, EAV tables, and runtime UI generators. This eliminates Rust's compile-time correctness guarantees, degrades PostgreSQL relational integrity and indexing, and creates heavy, non-native mobile user experiences unsuited for responsive Flutter SuperApp clients.
2. *Unbounded micro-crate fragmentation (15+ crates):* Decomposing every single table into an isolated crate (e.g. separating waybills, warehouse topology, movements, costing, and reconciliation into distinct crates). This breaks relational atomicity, forces complex multi-phase distributed sagas for simple operational tasks, and multiplies build overhead without semantic justification.

Furthermore, historical implementations in the codebase (e.g. `rustok-marketplace-ledger`) created localized, private double-entry ledgers inside feature modules due to the lack of a universal accounting core. An authoritative target architecture is required to define how CRM, ERP, Finance, and Accounting functionality decompose into sovereign, ACID-bound domain modules while establishing clean separation between operational money management (`rustok-finance`) and formal double-entry bookkeeping (`rustok-accounting`).

## Decision

RusToK adopts a **Sovereign Domain Modules + Operational Finance + Universal Accounting + Posting Rules Engine + Auxiliary No-Code Automation** architecture:

1. **Sovereign Domain Modules with ACID-Bound Subprocesses:**
   Related subprocesses that must commit together inside single database transactions are housed within coherent module boundaries:
   - **`rustok-party`**: Canonical master data owner for legal entities, individuals, tax IDs (INN/OGRN/KPP/VAT), corporate addresses, bank accounts, contracts, and credit limits.
   - **`rustok-deal`**: CRM leads, deals, pipelines with native typed FSM stage transitions, quote lines, and timeline activities (calls, tasks, meetings, notes).
   - **`rustok-inventory` (v2)**: Warehouse topology (warehouses, zones, bins), primary warehouse documents (goods receipts, sales shipments, internal transfers), append-only stock movement ledger (`stock_movements`), reservations (`stock_reservations`), batch/lot and expiry date tracking, and FIFO/average lot valuation in **one ACID PostgreSQL transaction boundary**.
   - **`rustok-billing`**: Commercial invoices for payment, proforma invoices, supplier bills, payment allocation (`PaymentAllocation`), acts of reconciliation, monotonic document sequence numbering (`document_sequences` with `SELECT ... FOR UPDATE`), and PDF print forms with facsimile stamps.
   - **`rustok-finance`**: Operational bank accounts, cash registers, bank statement import/parsing (1C/ClientBank, ISO 20022), cash inflow/outflow orders (PKO/RKO), daily central bank currency rates (ЦБ РФ), automated exchange difference calculation, and Cash Flow (ДДС) tracking.
   - **`rustok-accounting`**: Universal double-entry accounting engine (chart of accounts, immutable journal transactions, debit/credit postings, configurable Posting Rules Engine, typed relational subconto analytics, balance sheet, trial balance OSV).
   - **Foundation Primitives**: Foundational Units of Measure (UoM) reside in `crates/modules/rustok-commerce-foundation`. Tax rates are calculated via `crates/modules/rustok-tax`. E-commerce checkout orders remain in `crates/modules/rustok-order`.

2. **Decoupling Double-Entry Accounting from Feature Domains:**
   - Double-entry accounting is decoupled from feature modules. `rustok-accounting` is the sole platform owner of the chart of accounts and journal transactions.
   - Feature modules (Orders, Invoices, Warehouse Waybills, Marketplace Commissions) produce operational business facts. In the target state, the private ledgering in `rustok-marketplace-ledger` migrates into standard entries in `rustok-accounting` categorized by `party_id` (vendor subconto).

3. **Dual-Layer Communication: Compiled Core vs Auxiliary No-Code Automation:**
   - **Core Transactional Paths (Compiled Rust):** Mission-critical business workflows (e.g. `WaybillDispatched` $\to$ `Inventory` deduction $\to$ `Accounting` posting) are executed strictly by **statically typed, compiled Rust event handlers** subscribing to the Transactional Outbox (`sys_events`). These handlers provide atomic guarantees, idempotency, and high throughput.
   - **Auxiliary No-Code Automation (`rustok-workflow` — n8n Analogue):** `rustok-workflow` is an auxiliary, user-configurable automation layer for rapid custom integrations, external notifications (Telegram, Slack, Email), third-party webhooks, and cron triggers. Core financial, inventory, and accounting operations **never** depend on `rustok-workflow`, and no-code script failures cannot block or roll back core business transactions.

4. **Configurable Posting Rules Engine (Движок правил проводок):**
   Accounting debits and credits are not hardcoded in Rust. Primary documents pass through a declarative Posting Rules Engine in `rustok-accounting`:
   $$\text{Document Type} + \text{Operation Type} + \text{Item Group / Tax Regime} \longrightarrow \text{Debit Account} + \text{Credit Account} + \text{Analytics}$$
   Rules are stored in relational configuration tables with tenant overrides, enabling seamless adaptation to national accounting standards (RAS, US GAAP, IFRS).

5. **Indexed Relational Subconto Analytics:**
   To guarantee instantaneous calculation of trial balances (Оборотно-сальдовая ведомость) and acts of reconciliation at enterprise scale, `gl_postings` uses typed foreign key columns for high-cardinality standard dimensions:
   - `party_id: Option<Uuid>`,
   - `contract_id: Option<Uuid>`,
   - `item_id: Option<Uuid>`,
   - `warehouse_id: Option<Uuid>`,
   - `additional_analytics: Option<Jsonb>` (for rare, arbitrary user dimensions).

6. **Mobile SuperApp Architecture (Flutter & FFA):**
   - **Offline-First CQRS Queue:** Mobile clients (warehouse barcode scanners, couriers, sales reps) record local **Drafts** in SQLite/Isar. Mutations are flushed to the server via an idempotent sync queue when connectivity is restored.
   - **Server Authoritative Posting:** Final state transitions, stock deductions, and accounting journal entries are executed authoritatively by the server inside PostgreSQL transactions.
   - **Flex Metadata Form Rendering:** Standard screens run at 60/120 FPS via native Flutter widgets, while arbitrary enterprise attributes render dynamically via `FlexDynamicFormWidget` reading metadata from `crates/modules/flex`.

7. **Evolutionary Module Extraction Principle:**
   Premature decomposition into dozens of micro-crates is prohibited. Functionality is developed and stabilized inside cohesive core modules (`party`, `deal`, `inventory`, `billing`, `finance`, `accounting`). Splitting out dedicated sub-modules is considered only after domain invariants have stabilized and an explicit business requirement demonstrates the need for independent lifecycles, separate scaling, or access boundaries.

## Sources of truth and ownership

- **`rustok-party`** is the sole canonical source of truth for counterparty identity, registration details, tax numbers, and contracts. `rustok-customer` is a contextual B2C role projection over `Party`.
- **`rustok-inventory`** owns physical storage layouts, primary warehouse documents (waybills), on-hand balances (via append-only movements), reservations, and FIFO batch valuations.
- **`rustok-deal`** owns CRM pipeline positions, deal values, and timeline activities.
- **`rustok-billing`** owns commercial invoices, payment allocations, reconciliation acts, sequence number generators, and PDF print forms.
- **`rustok-finance`** owns cash and bank balance registers, bank statement imports, currency exchange rates, and cash flow records.
- **`rustok-accounting`** is the sole canonical source of truth for double-entry financial balances and journal lines.
- **`rustok-flex`** owns custom field definitions and attached JSONB entry values.
- **`rustok-workflow`** owns visual automation flow graphs and external execution history.

Consumers must never mutate another module's ledger or tables directly; all cross-boundary effects must occur via typed port methods or transactional outbox events.

## Invariants

- Every accounting transaction in `rustok-accounting` must be balanced: $\sum \text{Debit} = \sum \text{Credit}$.
- Journal postings in `rustok-accounting` and `rustok-inventory` (`stock_movements`) are append-only. Existing entries cannot be updated or deleted; corrections require explicit reversing entries (storno).
- `Party` records must be unique by `(tenant_id, tax_identifier)` when a formal tax identifier is present.
- Primary documents must obtain sequential monotonic numbers without gaps or collisions via `document_sequences` locked with `SELECT ... FOR UPDATE`.
- Inventory quantities in `rustok-inventory` must not drop below zero in strict non-negative accounting modes.
- Stock reservations in `rustok-inventory` cannot exceed available free balance ($\text{On-Hand} - \text{Existing Reserves}$).
- Every monetary amount must use canonical integer minor-unit money primitives (`rustok-commerce-foundation` / `rustok-core`) and specify an explicit ISO-4217 currency.

### Allowed states

- A `Party` can hold zero, one, or multiple operational roles simultaneously (e.g., both Customer and Vendor).
- A primary document (e.g., `Waybill`, `BillingInvoice`) progresses through formal lifecycle states: `Draft`, `Submitted`, `Approved`, `Posted`, `Cancelled`.
- An inventory reservation can be `Active`, `Claimed`, or `Expired`.
- Accounting periods can be `Open`, `Closing`, or `Closed`.

### Forbidden states

- Direct mutation of posted journal lines or posted stock ledger records.
- Creating an unbalanced journal entry where debit does not equal credit.
- Cross-tenant party, document, or inventory references.
- Floating-point arithmetic in monetary valuations or journal balances.
- Bypassing the transactional outbox for cross-aggregate financial or inventory side effects.
- Delegating core system transactional integrity or ledger mutations to `rustok-workflow`.

## Non-goals

- Implementing an in-browser arbitrary table DDL configurator or interpreter-based EAV core.
- Hardcoding statutory country-specific tax declaration forms directly into core crates (statutory reports are produced via reporting plugins and export connectors).
- Replacing high-volume real-time external bank settlement networks; bank integration is handled through standard import/export formats (1CClientBank, ISO 20022 camt.053) and gateway adapters.
- Prematurely fracturing core business entities into dozens of micro-crates prior to domain stabilization.

## Data, transaction, and concurrency boundary

- **PostgreSQL relational integrity:** Foreign keys, tenant-scoped compound primary and unique keys, and check constraints enforce structural validity.
- **Rust domain validation:** Complex business rules (costing algorithms, pipeline stage gate validations, reservation capacity checks) run in typed Rust domain services inside PostgreSQL transactions.
- **Concurrency control:** Optimistic concurrency tokens (`revision` counters) protect mutable document headers and party master data. Keyset-based pagination and idempotency tokens (`idempotency_key` via `PortContext`) protect all mutation commands.

## Context dimensions

- **Tenant (`tenant_id`):** Strict multitenant isolation across all party, inventory, financial, and document tables.
- **Channel / Organization:** Legal entities, operating branches, and sales channels are explicit dimensions in party contracts and general ledger postings.
- **Currency:** Explicit ISO-4217 currency code attached to every monetary field; conversion rates are recorded in transaction headers.
- **Timezone & Locale:** UTC timestamps for all journal and ledger events; display formatting governed by canonical locale contracts.

## Events and projections

- Core lifecycle facts emit typed domain events via `sys_events`:
  - `PartyCreated`, `PartyUpdated`
  - `DealStageChanged`, `DealWon`, `DealLost`
  - `WaybillPosted`, `WaybillCancelled`
  - `StockMoved`, `StockReserved`, `StockReleased`
  - `InvoiceIssued`, `PaymentAllocated`
  - `JournalTransactionPosted`
- Read models, customer aging reports, stock balance summaries, and trial balance projections are rebuilt idempotently from outbox events and indexed via `rustok-index`.

## Failure semantics

- Financial and inventory mutations fail closed: if balance checks, reservation limits, or outbox insertions fail, the entire database transaction rolls back.
- Outbox event consumers process messages with at-least-once delivery; consumer projections and accounting bridges maintain deterministic idempotency receipts.
- Failures in auxiliary `rustok-workflow` automations do not impact core domain transactions.

## Migration and cutover

- Existing modules (`rustok-customer`, `rustok-inventory`, `rustok-order`, `rustok-payment`, `rustok-tax`, `rustok-commerce-foundation`) remain fully operational.
- `rustok-party` is introduced as the foundational counterparty layer; `rustok-customer` is migrated to reference `Party` without breaking existing storefront contracts.
- `rustok-inventory` evolves in-place into v2, introducing multi-tier topology, primary waybills, and append-only stock movement tables while keeping checkout reservations compatible.
- `rustok-accounting` provides universal double-entry accounting, enabling future migration of marketplace commission and clearing entries out of `rustok-marketplace-ledger`.

## Alternatives considered

- *All-in-one ERP/CRM Monolith:* Rejected due to excessive coupling, violation of RusToK module boundaries, and bloated client footprint for specialized apps.
- *Unbounded Micro-Crate Decomposition (17 crates):* Rejected due to breaking ACID transactions across tightly coupled warehouse, billing, and contract operations.
- *Pure EAV/Configurator Engine (1C-style):* Rejected because it sacrifices Rust compile-time invariants, introduces severe database performance bottlenecks, and prevents ergonomic native mobile development.
- *Core Orchestration via `rustok-workflow`:* Rejected because core financial and material guarantees require compiled Rust performance and compile-time type verification, while `rustok-workflow` is an auxiliary no-code user tool.

## Verification

- Scoped crate unit tests verifying ledger debit/credit equality, inventory reservation bounds, and FIFO costing accuracy.
- Database constraint verification for tenant scoping, foreign keys, and immutability guards.
- Manifest and module governance gates (`cargo xtask module validate <slug>`, `npm run verify:adrs`, `npm run verify:docs`).
- Outbox transaction idempotency and replay test suites.

## Consequences

- The platform gains a clean, enterprise-grade architecture for CRM, ERP, Finance, and Accounting workflows without micro-crate sprawl.
- Core domains preserve ACID guarantees within sovereign module boundaries.
- Operational money and bank accounts are cleanly owned by `rustok-finance`, while formal double-entry bookkeeping is unified under `rustok-accounting`.
- Business operators gain no-code automation agility via `rustok-workflow` without endangering core transactional stability.
- Flutter mobile apps can assemble targeted, role-based workflows with offline draft resilience.
