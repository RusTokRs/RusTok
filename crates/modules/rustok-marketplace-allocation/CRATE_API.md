# rustok-marketplace-allocation / CRATE_API

## Role

`rustok-marketplace-allocation` is the canonical owner for immutable marketplace order-line allocations.
It records the binding between checkout order lines, winning sellers, listing IDs, and historical commercial snapshots at the moment of order placement.

## Public surface

### Primary Public Types

- `MarketplaceAllocationModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplaceAllocationService`: Core domain service managing allocation records and idempotency receipts.
- `MarketplaceAllocationReadPort`, `MarketplaceAllocationCommandPort`: Typed interface contracts for read queries and batch allocations.
- `MarketplaceAllocationStatus`: Allocation status enum (`Allocated`, `Cancelled`).
- `AllocateMarketplaceOrderLineInput`, `AllocateMarketplaceOrderLinesInput`: Batch allocation input DTOs.
- `AllocateMarketplaceOrderLinesResponse`, `MarketplaceOrderAllocationResponse`: Allocation output records.
- `ReadMarketplaceAllocationByLineRequest`: Query request by individual order line ID.
- `ListMarketplaceAllocationsByOrderRequest`, `ListMarketplaceAllocationsBySellerRequest`: Query requests by order ID or seller ID.
- `MarketplaceAllocationListResponse`: Paginated allocation list response.
- `MAX_ALLOCATION_LINES_PER_COMMAND`, `MAX_ALLOCATIONS_PER_PAGE`: Bounded limit constants.
- `MarketplaceAllocationError`, `MarketplaceAllocationResult`: Domain error and result types.

## Platform Capability Bindings

- **RusToK Module Interface**: Implements `RusToKModule` with slug `"marketplace_allocation"`, declaring dependencies on `"marketplace_seller"`, `"marketplace_listing"`, and `"order"`, registering permissions for order reads and listing/seller lookups.
- **Migration Source**: Implements `MigrationSource` registering database schema migrations for allocation records and idempotency receipts.
- **In-Process Port Provider**: Exposes `in_process_marketplace_allocation_command_port` for fast, zero-transport dispatch during checkout.

## Domain Collaborations

- **`rustok-order`**: Receives order lines during checkout completion to assign seller allocations.
- **`rustok-marketplace-commission`**: Consumes allocation lines to assess commission rates per seller.
- **`rustok-marketplace-balance`**: Consumes allocation splits to credit seller pending balances.
- **`rustok-marketplace`**: Orchestrates allocation resolution across multi-vendor checkouts.

## Minimum Contract Set

### Input DTOs/Commands

- `AllocateMarketplaceOrderLinesInput`: `order_id`, `idempotency_key`, `lines: Vec<AllocateMarketplaceOrderLineInput>`.
- `AllocateMarketplaceOrderLineInput`: `order_line_id`, `seller_id`, `listing_id`, `product_id`, `variant_id`, `quantity`, `unit_price_minor`, `currency`.
- `ReadMarketplaceAllocationByLineRequest`: `order_line_id`.
- `ListMarketplaceAllocationsByOrderRequest`: `order_id`.
- `ListMarketplaceAllocationsBySellerRequest`: `seller_id`, `page`, `per_page`.

### Domain Invariants

- **Immutable Line Allocation**: An order line can only be allocated once; duplicate allocation attempts are rejected.
- **All-or-Nothing Batch Atomicity**: A batch either allocates every supplied line and commits its receipt in one database transaction, or rolls back entirely.
- **Idempotency with Exact Request Validation**: Replaying an identical request with the same idempotency key returns the cached response; changing parameters with the same key is strictly rejected.
- **Minor Currency Units**: All monetary values are integer amounts represented in currency minor units (e.g. cents).

### Events / Outbox Side Effects

- Commits allocation records and receipts atomically; external event listeners consume allocation completion to trigger commission calculation.

### Errors / Failure Codes

- `MarketplaceAllocationError::NotFound`: Allocation for order line does not exist.
- `MarketplaceAllocationError::Conflict`: Order line is already allocated or idempotency conflict detected.
- `MarketplaceAllocationError::Validation`: Line count exceeded `MAX_ALLOCATION_LINES_PER_COMMAND` or amounts are negative.
- `MarketplaceAllocationError::Database`: Underlying persistence error wrapped cleanly.

