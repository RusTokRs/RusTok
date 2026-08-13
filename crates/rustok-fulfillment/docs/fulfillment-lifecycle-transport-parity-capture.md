# Fulfillment lifecycle transport parity capture

Status: capture contract published, execution pending.

## Purpose

This capture proves mounted **projection parity** for fulfillment lifecycle reads
that already consume `FulfillmentReadPort` through the application-host-composed
runtime. It compares real GraphQL HTTP and admin REST responses without retaining
credentials, raw bodies, or fulfillment metadata.

The locked execution contract is:

`crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution-contract.json`

The runner is:

`scripts/evidence/capture-fulfillment-lifecycle-transport-parity.mjs`

The source guard is:

`scripts/verify/verify-fulfillment-lifecycle-transport-parity-capture.mjs`

A successful maintainer-owned run writes one immutable packet to:

`crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution.json`

That output file is intentionally absent until the mounted capture succeeds.

## Mounted inputs

The runner requires full mounted URLs instead of assuming a server prefix:

- `RUSTOK_FULFILLMENT_PARITY_GRAPHQL_URL`, ending in the canonical
  `/api/graphql` path;
- `RUSTOK_FULFILLMENT_PARITY_REST_BASE_URL`, the prefix immediately before
  `/admin/fulfillments`.

Remote mounted endpoints must use HTTPS. Plain HTTP is accepted only for
`localhost`, `127.0.0.1`, or IPv6 loopback so the bearer token is not sent over an
unencrypted remote connection. Redirect responses are rejected.

The bearer token must carry both `fulfillments:read` and `orders:read`, because the
latest-by-order scenario reads `order.fulfillment` as well as fulfillment roots.
Tenant resolution uses the configured header, defaulting to `X-Tenant-ID`.
Reserved transport headers cannot be selected as the tenant header.

Required fixture identities:

- one known fulfillment for detail parity;
- its order id and current status for filtered-list parity;
- the expected latest fulfillment id for that order;
- one canonical UUID that does not exist for optional-not-found behavior.

The mounted binary revision, selected adapter profile, and runtime-instance label
are operator claims retained for review. The runner does not independently prove
source revision, external-adapter identity, or process identity.

## Scenarios

1. GraphQL `fulfillment` and REST `GET /admin/fulfillments/{id}` must return
   identical normalized owner projections.
2. GraphQL `fulfillments` and REST `GET /admin/fulfillments` must preserve the same
   ordered projections, total, page, per-page, and `has_next` result for the same
   order/status filter.
3. GraphQL `order.fulfillment` must return the configured latest fulfillment and
   match its REST detail projection.
4. A missing fulfillment must return GraphQL `null` without errors and REST
   `404 commerce_admin_not_found`.

Fulfillment items are sorted by id before hashing. Top-level list order remains
transport-visible and must match. Equivalent RFC3339 timestamps are normalized to UTC millisecond form before comparison so `Z` and `+00:00` formatting do not create false mismatches. Metadata is excluded from the retained projection boundary.

## Capture command

```bash
export RUSTOK_FULFILLMENT_PARITY_GRAPHQL_URL='http://127.0.0.1:3000/api/graphql'
export RUSTOK_FULFILLMENT_PARITY_REST_BASE_URL='http://127.0.0.1:3000/api'
export RUSTOK_FULFILLMENT_PARITY_TENANT_ID='<tenant-uuid>'
export RUSTOK_FULFILLMENT_PARITY_AUTH_TOKEN='<bearer-token>'
export RUSTOK_FULFILLMENT_PARITY_DETAIL_ID='<known-fulfillment-uuid>'
export RUSTOK_FULFILLMENT_PARITY_ORDER_ID='<known-order-uuid>'
export RUSTOK_FULFILLMENT_PARITY_STATUS='<current-status>'
export RUSTOK_FULFILLMENT_PARITY_LATEST_ID='<latest-fulfillment-uuid>'
export RUSTOK_FULFILLMENT_PARITY_MISSING_ID='<nonexistent-uuid>'
export RUSTOK_FULFILLMENT_PARITY_SOURCE_REVISION='<mounted-40-char-git-sha>'
export RUSTOK_FULFILLMENT_PARITY_ADAPTER_PROFILE='in_process'
node scripts/evidence/capture-fulfillment-lifecycle-transport-parity.mjs
```

Optional inputs control tenant header, locale, page, per-page, client timeout, and
an operator-provided runtime-instance label. URLs containing credentials, query,
or fragments are rejected. Existing evidence is never overwritten implicitly.

## Retained packet

The packet retains:

- contract and source hashes;
- operator-claimed mounted revision, adapter profile, and runtime instance;
- sanitized endpoint paths;
- fixture identifiers;
- response status and duration facts;
- normalized projection hashes and bounded pagination facts;
- explicit scenario pass results.

It does not retain the bearer token, authorization header, raw response bodies, or
fulfillment metadata.

A successful packet may set `transport_projection_parity_proven` to `true`.
`runtime_parity_proven` remains `false` because this capture does not prove owner
deadline/failure injection, process restart, external-adapter identity, or remote
adapter behavior.

## Remaining evidence

After projection parity is retained, separate execution must prove:

- owner deadline and typed failure behavior with a controllable adapter;
- runtime selection across process restart;
- external-adapter identity rather than an operator label;
- remote-adapter projection and error parity.

No FFA/FBA or production status is promoted by publishing this capture contract.

## Intended source check

```bash
node scripts/verify/verify-fulfillment-lifecycle-transport-parity-capture.mjs
```

The implementation agent did not run the source guard, capture runner, tests,
Cargo commands, formatting, workflows, or CI.
