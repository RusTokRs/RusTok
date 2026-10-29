# rustok-marketplace-commission / CRATE_API

## Role

`rustok-marketplace-commission` is the canonical owner for versioned marketplace commission rules and order line commission assessments.
It calculates platform fee splits and seller proceeds based on deterministic priority matching and integer minor unit math.

## Public surface

### Primary Public Types

- `MarketplaceCommissionModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplaceCommissionService`: Core domain service managing commission rules, assessments, and receipts.
- `MarketplaceCommissionReadPort`, `MarketplaceCommissionCommandPort`: Typed interface contracts for rule queries and assessment commands.
- `MarketplaceCommissionRuleStatus`: Rule lifecycle status (`Active`, `Archived`).
- `MarketplaceCommissionAssessmentStatus`: Assessment status (`Assessed`, `Reversed`).
- `CreateMarketplaceCommissionRuleVersionInput`: Payload creating a versioned commission rule.
- `AssessMarketplaceOrderCommissionsInput`, `AssessMarketplaceOrderCommissionsResponse`: Batch assessment command and response.
- `MarketplaceCommissionRuleResponse`, `MarketplaceCommissionAssessmentResponse`: Detailed record projections.
- `MarketplaceCommissionRuleListResponse`, `MarketplaceCommissionAssessmentListResponse`: Paginated query responses.
- `MAX_COMMISSION_RULES_PER_PAGE`, `MAX_COMMISSION_ASSESSMENTS_PER_PAGE`: Bounded limit constants.
- `MarketplaceCommissionError`, `MarketplaceCommissionResult`: Domain error and result types.

## Platform Capability Bindings

- **RusToK Module Interface**: Implements `RusToKModule` with slug `"marketplace_commission"`, declaring dependencies on `"marketplace_allocation"`, registering permissions for order reads and listing/seller lookups.
- **Migration Source**: Implements `MigrationSource` registering database schema migrations for commission rule versions, assessment rows, and receipts.
- **In-Process Port Provider**: Exposes `in_process_marketplace_commission_command_port` for low-overhead dispatch from order orchestration.

## Domain Collaborations

- **`rustok-marketplace-allocation`**: Consumes `MarketplaceAllocationReadPort` to retrieve order line allocations for assessment.
- **`rustok-marketplace-balance`**: Provides assessed commission splits to drive sub-ledger postings.
- **`rustok-marketplace`**: Drives financial orchestration across marketplace checkouts.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateMarketplaceCommissionRuleVersionInput`: `rule_key`, `name`, `seller_id`, `listing_id`, `rate_bps`, `fixed_amount_minor`, `currency`, `priority`, `effective_from`.
- `AssessMarketplaceOrderCommissionsInput`: `order_id`, `idempotency_key`.
- `ReadMarketplaceCommissionAssessmentRequest`: `assessment_id`.
- `ListMarketplaceCommissionAssessmentsByOrderRequest`: `order_id`.
- `ListMarketplaceCommissionAssessmentsBySellerRequest`: `seller_id`, `page`, `per_page`.
- `ListMarketplaceCommissionRulesRequest`: `search`, `seller_id`, `listing_id`, `status`, `page`, `per_page`.

### Domain Invariants

- **Append-Only Rule Versions**: Updating a commission policy produces an incremental version under the same `rule_key`; existing rule versions remain immutable.
- **Single Assessment per Allocation**: Each order line allocation receives at most one commission assessment.
- **Deterministic Rule Precedence**: Active rules resolve by scope (listing > seller > global), priority, version, effective date, and UUID tie-breaker.
- **Integer Minor Unit Precision**: Formulas strictly use integer basis points and minor units: `floor(allocation_total * rate_bps / 10000) + fixed_amount`; floats are forbidden (`#![deny(clippy::float_arithmetic)]`).
- **Proceeds Non-Negativity**: Commission amount cannot exceed allocation total; assessments producing negative seller proceeds are rejected.

### Events / Outbox Side Effects

- Atomically commits commission assessments and command receipts in the same database transaction.

### Errors / Failure Codes

- `MarketplaceCommissionError::NotFound`: Rule, allocation, or assessment does not exist.
- `MarketplaceCommissionError::Conflict`: Duplicate assessment attempt or conflicting idempotency key.
- `MarketplaceCommissionError::Validation`: Rate exceeds 10,000 bps (100%) or currency mismatch on fixed component.
- `MarketplaceCommissionError::Database`: Underlying persistence error wrapped cleanly.

