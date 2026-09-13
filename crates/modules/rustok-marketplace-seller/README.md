# rustok-marketplace-seller

## Purpose

`rustok-marketplace-seller` owns seller identity, seller lifecycle, onboarding,
and seller-scoped memberships for the RusToK Marketplace Family.

## Responsibilities

- Persist tenant-scoped seller identity and profile data.
- Own seller lifecycle transitions: draft, active, suspended, and closed.
- Own onboarding review state independently from business lifecycle state.
- Create the initial owner membership atomically with a seller.
- Own seller-scoped member roles and status without replacing platform RBAC.
- Publish typed FBA read and command ports.
- Persist tenant-scoped seller command receipts with canonical SHA-256 request
  identity and normalized typed response snapshots.
- Commit a receipt, its seller/member mutation, and the saved response in one
  database transaction so a lost response can be replayed safely.
- Reject reuse of an idempotency key for another actor, command kind, or payload.
- Publish module-owned GraphQL query/mutation roots over the same typed ports.
- Publish a module-owned admin FFA package with explicit native/GraphQL transport
  selection and no implicit fallback.
- Provide an exact-locale CAS mutation boundary for public seller presentation
  copy (`display_name`), serialized on the parent seller row.
- Store only normalized verification facts; provider-specific KYC payloads belong
  behind a future SPI and must not be persisted here.

## Translation target boundary

Marketplace Seller owns the neutral Translation target
`marketplace_seller/seller_presentation`. It exposes exactly one localized field:
`display_name`, classified as public plain text and eligible for AI export under
Translation's normal policy. `legal_name`, handle, metadata, onboarding and
suspension prose, memberships, verification facts, lifecycle state, and all
other operational seller data are intentionally outside this target and must not
be copied into Translation-owned storage.

The target lists and reads exact locale rows only. Resource, source-locale, and
target-locale revisions use the existing semantic SHA-256 owner contract; apply
is guarded by all three revisions and changes only the requested target locale.
A real apply commits the localized row, append-only Seller-owned Translation
change evidence, the canonical `MarketplaceSellerProfileUpdated` outbox event,
and the durable owner operation receipt in one transaction. Idempotent replay or
an unchanged target does not manufacture a second owner event or change entry.

Canonical `create_seller` and `update_seller_profile` commands record the same
Translation-relevant change evidence when presentation copy changes. Seller
commands that change only legal, membership, onboarding, suspension, metadata,
or lifecycle facts are semantically deduplicated from the Translation journal
unless they alter the target lifecycle itself. Aggregate progress is based only
on exact target rows, and the bounded ChangeCursor is frozen to a stable owner
high-water mark so concurrent writes cannot make one page skip evidence.

Production composition registers the provider only when
`mod-marketplace_seller` is compiled. Registration does not imply rollout
readiness: focused PostgreSQL migration, concurrent CAS, idempotent replay,
lifecycle/progress, and ChangeCursor evidence remains a separate readiness gate.

## Entry points

- `MarketplaceSellerModule`
- `MarketplaceSellerService`
- `MarketplaceSellerTranslationService`
- `MarketplaceSellerTranslationTargetProvider`
- `register_marketplace_seller_translation_target_provider`
- `MarketplaceSellerReadPort`
- `MarketplaceSellerCommandPort`
- `graphql::MarketplaceSellerQuery` with the `graphql` feature
- `graphql::MarketplaceSellerMutation` with the `graphql` feature
- `dto::*`
- `entities::*`

## Interactions

The module is consumed by `rustok-marketplace`, module-owned admin transports, and
future listing/allocation modules through typed ports. The Leptos admin package
uses one serializable command envelope for both native server functions and
GraphQL, preserves the original idempotency key for explicit retry, and never
falls back to another transport automatically.

The module does not depend on `rustok-commerce`, copy auth users, own product
catalog content, or own payment, ledger, and payout state.
