# rustok-marketplace-listing / CRATE_API

## Role

`rustok-marketplace-listing` is the canonical owner for seller listing identity, versioned commercial terms, approval workflows, operational lifecycle states, and deterministic eligibility projections for the Marketplace Family.
It links seller organizations to master product variants while enforcing commercial references and immutable term revisions.

## Public surface

### Primary Public Types

- `MarketplaceListingModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplaceListingService`: Core domain service managing listings, terms, and approval workflows.
- `MarketplaceListingRuntime`: Dependency injection carrier providing read and command ports.
- `MarketplaceListingReadPort`, `MarketplaceListingCommandPort`: Typed interface contracts for read queries and mutations.
- `MarketplaceListingPorts`: Aggregate trait bundling listing ports.
- `MarketplaceListingStatus`: Operational lifecycle state (`Draft`, `Active`, `Suspended`, `Archived`).
- `MarketplaceListingApprovalStatus`: Moderation review status (`Draft`, `Submitted`, `InReview`, `Approved`, `Rejected`).
- `MarketplaceListingResponse`, `MarketplaceListingTermsResponse`, `MarketplaceListingEventResponse`: DTO responses.
- `MarketplaceListingEligibilityProjection`: Evaluation projection indicating whether a listing is eligible for customer purchase.
- `CreateMarketplaceListingInput`, `UpdateMarketplaceListingTermsInput`: Listing mutation commands.
- `ReviewMarketplaceListingInput`, `SuspendMarketplaceListingInput`: Moderation and lifecycle management commands.
- `ListMarketplaceListingsInput`, `ReadMarketplaceListingRequest`, `MarketplaceListingEligibilityRequest`: Query requests.
- `MarketplaceListingError`, `MarketplaceListingResult`: Domain error types and result aliases.

## Platform Capability Bindings

- **RusToK Module Registration**: Implements `RusToKModule` with slug `"marketplace_listing"`, declaring dependencies on `"marketplace_seller"` and `"product"`, and registering permissions for listing creation, reads, updates, listing, management, publishing, and moderation.
- **Migration Source**: Implements `MigrationSource` registering database schema migrations for listing headers, versioned terms, command receipts, and audit events.
- **Transactional Outbox Integration**: Publishes listing lifecycle events (`MarketplaceListingCreated`, `MarketplaceListingTermsUpdated`, `MarketplaceListingSubmitted`, `MarketplaceListingReviewed`, `MarketplaceListingSuspended`, `MarketplaceListingReactivated`, `MarketplaceListingArchived`).

## Domain Collaborations

- **`rustok-marketplace-seller`**: Consumes `MarketplaceSellerReadPort` to ensure sellers exist and are active prior to listing creation or activation.
- **`rustok-product`**: Consumes `ProductCatalogReadPort` to validate product and variant references.
- **`rustok-marketplace`**: Provides eligibility projections and listing details for multi-seller order workflows.
- **`rustok-outbox`**: Emits listing domain events atomically with persistence mutations.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateMarketplaceListingInput`: `seller_id`, `product_id`, `variant_id`, `seller_sku`, `market_slug`, `channel_slug`, `currency`, `price_amount`, `compare_at_amount`, `metadata`.
- `UpdateMarketplaceListingTermsInput`: `listing_id`, `seller_sku`, `currency`, `price_amount`, `compare_at_amount`, `metadata`.
- `ReviewMarketplaceListingInput`: `listing_id`, `approved`, `review_note`.
- `SuspendMarketplaceListingInput`: `listing_id`, `reason`.
- `ListMarketplaceListingsInput`: `seller_id`, `product_id`, `status`, `approval_status`, `page`, `per_page`.
- `MarketplaceListingEligibilityRequest`: `listing_ids`, `market_slug`, `channel_slug`.

### Domain Invariants

- **Listing Uniqueness**: One listing identity per tenant, seller, master variant, market, and channel scope.
- **Seller SKU Uniqueness**: Seller SKU is unique within the owning seller organization.
- **Immutable Term Versions**: Price and commercial references are recorded in append-only terms versions; updates create new revisions rather than mutating historical terms in place.
- **Deterministic Eligibility**: Eligibility projections evaluate seller operational status, listing approval, and lifecycle status with explicit reason codes.
- **Idempotent Command Receipts**: Mutating commands persist SHA-256 idempotency receipts with saved responses in the same transaction.

### Events / Outbox Side Effects

- `MarketplaceListingCreated`: Emitted upon new listing registration.
- `MarketplaceListingTermsUpdated`: Emitted when a new version of commercial terms is committed.
- `MarketplaceListingSubmitted`: Emitted when submitted for administrative moderation.
- `MarketplaceListingReviewed`: Emitted upon approval or rejection.
- `MarketplaceListingSuspended` / `MarketplaceListingReactivated`: Emitted on lifecycle status shifts.
- `MarketplaceListingArchived`: Emitted upon permanent retirement of a listing.

### Errors / Failure Codes

- `MarketplaceListingError::NotFound`: Listing or referenced entity does not exist.
- `MarketplaceListingError::Conflict`: Duplicate listing for variant or duplicate seller SKU.
- `MarketplaceListingError::InvalidState`: State transition not permitted from current lifecycle state.
- `MarketplaceListingError::Validation`: Input payload failed schema or business validation constraints.
- `MarketplaceListingError::SellerInactive`: Owning seller is suspended or not in active state.
- `MarketplaceListingError::Database`: Underlying persistence error wrapped cleanly.

