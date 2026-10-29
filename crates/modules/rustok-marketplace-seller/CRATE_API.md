# rustok-marketplace-seller / CRATE_API

## Role

`rustok-marketplace-seller` is the canonical owner for Seller identity, lifecycle states, onboarding workflows, and seller-scoped membership rosters within the Marketplace Family.
It maintains tenant-scoped seller profiles, membership roles, and localized display names with exact-locale CAS mutation boundaries.

## Public surface

### Primary Public Types

- `MarketplaceSellerModule`: Native module struct implementing `RusToKModule` and `MigrationSource`.
- `MarketplaceSellerService`: Core domain service managing seller entities, members, and onboarding reviews.
- `MarketplaceSellerRuntime`: Dependency injection carrier providing ports for FBA composition.
- `MarketplaceSellerReadPort`, `MarketplaceSellerCommandPort`: Typed interface contracts for read queries and mutations.
- `MarketplaceSellerStatus`: Seller operational lifecycle state (`Draft`, `Active`, `Suspended`, `Closed`).
- `MarketplaceSellerOnboardingStatus`: Independent review state (`Draft`, `Submitted`, `InReview`, `Approved`, `Rejected`).
- `MarketplaceSellerMemberRole`: Role within the seller organization (`Owner`, `Admin`, `Member`, `Finance`).
- `MarketplaceSellerMemberStatus`: Membership state (`Active`, `Invited`, `Suspended`).
- `MarketplaceSellerResponse`, `MarketplaceSellerMemberResponse`, `MarketplaceSellerEventResponse`: DTO responses.
- `CreateMarketplaceSellerInput`, `UpdateMarketplaceSellerProfileInput`: Command payloads for seller management.
- `SubmitMarketplaceSellerOnboardingInput`, `ReviewMarketplaceSellerOnboardingInput`: Onboarding transition payloads.
- `SuspendMarketplaceSellerInput`: Suspension command with audit reason.
- `AddMarketplaceSellerMemberInput`, `UpdateMarketplaceSellerMemberInput`: Roster modification payloads.
- `MarketplaceSellerTranslationService`, `MarketplaceSellerTranslationTargetProvider`: Localization target boundary.
- `MarketplaceSellerError`, `MarketplaceSellerResult`: Domain error types and result aliases.

## Platform Capability Bindings

- **RusToK Module Interface**: Implements `RusToKModule` with slug `"marketplace_seller"`, registering permissions for seller creation, reads, updates, deletion, listing, and management.
- **Migration Source**: Implements `MigrationSource` registering database schema migrations for seller tables, member rosters, receipts, events, and translations.
- **Translation Provider Binding**: Registers `MarketplaceSellerTranslationTargetProvider` for the `marketplace_seller/seller_presentation` target.
- **Transactional Outbox Integration**: Emits lifecycle domain events (`MarketplaceSellerCreated`, `MarketplaceSellerProfileUpdated`, `MarketplaceSellerOnboardingSubmitted`, `MarketplaceSellerOnboardingReviewed`, `MarketplaceSellerSuspended`, `MarketplaceSellerReactivated`, `MarketplaceSellerMemberAdded`, `MarketplaceSellerMemberUpdated`).

## Domain Collaborations

- **`rustok-marketplace`**: Orchestrates marketplace flows and queries seller details and membership.
- **`rustok-marketplace-listing`**: Validates seller existence and active state before approving product listings.
- **`rustok-translation`**: Integrates for exact-locale translation export and import via the neutral target provider.
- **`rustok-outbox`**: Publishes seller domain events atomically with persistence mutations.

## Minimum Contract Set

### Input DTOs/Commands

- `CreateMarketplaceSellerInput`: `handle`, `display_name`, `legal_name`, `owner_user_id`, `metadata`.
- `UpdateMarketplaceSellerProfileInput`: `display_name`, `legal_name`, `metadata`.
- `SubmitMarketplaceSellerOnboardingInput`: `note`.
- `ReviewMarketplaceSellerOnboardingInput`: `approved`, `note`.
- `SuspendMarketplaceSellerInput`: `reason`.
- `AddMarketplaceSellerMemberInput`: `user_id`, `role`, `metadata`.
- `UpdateMarketplaceSellerMemberInput`: `role`, `status`, `metadata`.
- `ListMarketplaceSellersInput`: `search`, `status`, `onboarding_status`, `page`, `per_page`.
- `ReadMarketplaceSellerRequest`: `seller_id` or `handle`.

### Domain Invariants

- **Atomic Creation**: A seller record and its initial owner membership must be created atomically in the same database transaction.
- **Lifecycle Decoupling**: Business operational status (`MarketplaceSellerStatus`) and onboarding review status (`MarketplaceSellerOnboardingStatus`) are distinct state machines.
- **Strict Role Model**: Every seller organization must have at least one active `Owner`.
- **Command Receipting & Idempotency**: All mutating operations support idempotency receipts hashed with SHA-256; replaying an identical request returns the cached result without duplicate side effects.
- **Tenant Isolation**: All database queries and mutations are strictly scoped by `tenant_id`.

### Events / Outbox Side Effects

- `MarketplaceSellerCreated`: Emitted upon new seller entity creation and owner membership assignment.
- `MarketplaceSellerProfileUpdated`: Emitted upon profile or localized display name modification.
- `MarketplaceSellerOnboardingSubmitted`: Emitted when onboarding is submitted for platform review.
- `MarketplaceSellerOnboardingReviewed`: Emitted when review is approved or rejected.
- `MarketplaceSellerSuspended` / `MarketplaceSellerReactivated`: Emitted on lifecycle status shifts.
- `MarketplaceSellerMemberAdded` / `MarketplaceSellerMemberUpdated`: Emitted on roster mutations.

### Errors / Failure Codes

- `MarketplaceSellerError::NotFound`: Seller or member does not exist in the tenant.
- `MarketplaceSellerError::AlreadyExists`: Seller handle is already taken within the tenant.
- `MarketplaceSellerError::InvalidState`: State transition is not permitted from current lifecycle state.
- `MarketplaceSellerError::Validation`: Input payload failed schema or business validation constraints.
- `MarketplaceSellerError::Unauthorized`: Actor does not possess necessary seller-level or platform permissions.
- `MarketplaceSellerError::Database`: Underlying persistence error wrapped cleanly.
