# Shipping-option GraphQL/REST transport parity capture

Status: capture contract published, execution pending.

This capture compares the mounted Commerce GraphQL shipping-option projections with the
mounted Commerce REST projections for the same tenant and authorization context.

Contract:

- crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-execution-contract.json
- scripts/evidence/capture-shipping-option-read-transport-parity.mjs
- scripts/verify/verify-shipping-option-read-transport-parity-capture.mjs
- output: crates/modules/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-execution.json

## Scenarios

1. Storefront active list: GraphQL storefrontShippingOptions vs GET /store/shipping-options.
2. Admin lookup: GraphQL shippingOption vs GET /admin/shipping-options/{id}.
3. Admin list: GraphQL shippingOptions vs GET /admin/shipping-options, including filters and pagination.
4. Optional not-found: GraphQL lookup returns null without errors while REST returns 404 commerce_admin_not_found.

## Required environment

RUSTOK_SHIPPING_PARITY_GRAPHQL_URL
RUSTOK_SHIPPING_PARITY_REST_BASE_URL
RUSTOK_SHIPPING_PARITY_TENANT_ID
RUSTOK_SHIPPING_PARITY_AUTH_TOKEN
RUSTOK_SHIPPING_PARITY_DETAIL_ID
RUSTOK_SHIPPING_PARITY_MISSING_ID

Useful optional inputs include locale, storefront currency/region/country, admin active/provider/search
filters, page, per-page, a tenant header override, and the client timeout. Remote mounted endpoints must use HTTPS.
Loopback HTTP is permitted only for local capture.

## Capture

Run from the repository root:

node scripts/evidence/capture-shipping-option-read-transport-parity.mjs

The runner rejects endpoint credentials/query/fragment data, redirects, oversized responses, invalid UUIDs,
unsafe tenant headers, and an existing evidence packet. It retains normalized projection hashes, source hashes,
sanitized endpoints, bounded request facts, and scenario timings; it does not retain the bearer token, raw response
bodies, or shipping-option metadata.

The source revision and adapter profile are claims supplied by the operator. The runner records them as claims only;
claims_verified_by_runner remains false.

## Verification

Run:

node scripts/verify/verify-shipping-option-read-transport-parity-capture.mjs
node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs

No capture result may promote runtime_parity_proven. Deadline/failure injection, process restart, external adapter
identity, and remote adapter behavior remain separate evidence gates.

Only a successful immutable capture packet can set transport_projection_parity_proven to true. The packet still keeps
runtime_parity_proven false until the wider runtime evidence is independently executed and reviewed.
