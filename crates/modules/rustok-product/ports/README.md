# rustok-product-ports

Canonical FBA contract crate for `rustok-product`.

Publishes:
- `ProductCatalogReadPort`: Storefront, administrative, and checkout catalog projection reads.
- `ProductStorefrontHttpReadPort`: Published catalog list used by Commerce REST.
- `ProductVariantIdentityReadPort`: Tenant-scoped variant identity lookup and enumeration.
- `ProductVariantReplayPort`: Versioned live variants and deletion identities for Index.
- `ProductLocaleReplayPort`: Versioned localized Product cards and deletion identities for Index.
- `ProductCatalogCommandPort`: Mutating lifecycle and catalog commands.
- `ProductAttachedValuesReadPort`: Locale-aware Product attached fields in the
  embedded and authenticated gRPC profiles.
- `ProductAttachedValuesWritePort`: Update and delete Product attached fields
  with a deadline and a stable key. Embedded and gRPC commands share the owner receipt.
- Canonical DTOs, requests, and domain models.
- Optional `grpc-client` and `grpc-server` tonic adapters.

## Deployment Configuration

- `RUSTOK_PRODUCT_DEPLOYMENT=embedded` (default): Product runs within the modular monolith.
- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc`: Product catalog reads are delegated to a remote gRPC service.
- `RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT`: gRPC endpoint URL (e.g. `https://product.internal:50051`).
- `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`: Shared secret bearer token for authentication.
- `RUSTOK_PRODUCT_ATTACHED_VALUES_READ_GRPC_BEARER_TOKEN`: Optional credential
  for `ReadAttachedValues`. Configure the same value on the consuming host and
  Product service. Use a credential distinct from both catalog credentials.
- `RUSTOK_PRODUCT_ATTACHED_VALUES_WRITE_GRPC_BEARER_TOKEN`: Optional credential
  for `UpdateAttachedValues` and `DeleteAttachedValues`. Set it on the Product
  service and the host selecting gRPC commands. It must differ from the catalog
  credentials and the attached-value read credential.
- `RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true`: Allow plaintext HTTP only on loopback interfaces (`127.0.0.1`, `::1`).

Invalid configuration or connection failure stops startup immediately.
Connection success establishes transport reachability. The server checks the
credential, tenant context and operation grant when it handles an RPC; a
successful connection alone does not establish command-service availability.
See the [owner's service configuration](../README.md#standalone-service-configuration).

When the Product read provider is `grpc`, Flex GraphQL uses the selected remote
attached-value port if its credential is configured. Without that binding the
operation returns unavailable. The resolver checks the caller's Flex read
permission before delegating to Product, which enforces tenant scope and resolves
the requested locale with its usual fallback policy.

Flex GraphQL accepts `idempotencyKey` on `updateAttachedValues` and
`deleteAttachedValues`. The remote Product command profile requires it. Reuse
the same key and payload to retry a logical command. The host scopes the key to
the authenticated principal, tenant, operation and product. A successful retry
returns the committed result, including the original update snapshot. A key
reused with a different payload returns `CONFLICT`.
Embedded callers may omit the key. Such a call receives a fresh key, so client
retries need an explicit key for deduplication in embedded mode as well.

The catalog read credential covers both storefront list RPCs.
`ListLegacyStorefrontHttpProducts` serves `GET /store/products` with a limit of
100 rows. `ListLegacyStorefrontProducts` serves the GraphQL `storefrontProducts`
field with a limit of 48 rows. Each RPC preserves its existing owner projection.
A standalone Product instance without a Taxonomy provider returns a retryable
unavailable error if listing the requested page requires resolving tag names.
The gRPC client preserves structured owner errors. Bare `DeadlineExceeded` and
`Cancelled` statuses are retryable timeouts because Tonic can report an expired
transport deadline as `Cancelled`. A command whose response timed out may have
committed. Reuse the original idempotency key and payload to recover its result.

## Authentication

- Client authentication: `ProductCatalogGrpcBearerToken` attaches `Authorization: Bearer ...` and `x-rustok-tenant-id`.
- Server authentication: Servers must use `ProductCatalogGrpcBearerInterceptor`, which compares the complete authorization value in constant time.
- Configured via `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`.
- The ordinary catalog read credential has its existing operation grants. The
  attached-value read credential grants only `ReadAttachedValues` on the same endpoint.
- The attached-value write credential grants only the two donor mutations.
  The Product service authenticates them as its configured command service actor.
  The Product event includes the human actor on embedded user requests. Service
  requests carry no human attribution in that event.
- Note: TLS and authentication solve separate problems; TLS protects data in transit while bearer tokens authenticate callers.


## ProductVariant replay

`ProductVariantReplayPort` loads exact variant IDs and scans by increasing
variant ID. The owner returns a source revision with each live projection or
retained deletion identity. An omitted exact-load row represents absence of
owner evidence. Consumers carry the tenant in `PortContext` and submit a
bounded deadline. Scan pages accept up to 1,000 entries and exact loads accept
up to 256 IDs.

The optional gRPC methods `ScanVariantReplay` and `LoadVariantReplay` require a
dedicated service credential. The Product service assigns the authenticated
actor and replay grant. The embedded adapter applies the same owner policy.

## Localized Product replay

`ScanLocaleReplay` and `LoadLocaleReplay` use a dedicated locale replay token.
Each entry carries the graph projection epoch as `source_version` and the exact live or
retained locale revision as `product_source_version`. Index uses the locale revision to
fence refresh events and persists mutations at the graph epoch. Deploy the Product host that
supplies both fields before upgrading a remote consumer to this contract.
Product checks the exact locale, graph epoch and Channel witness. Cursor order
and mutation identity are shared between embedded and remote deployment.
The standalone service uses its Channel identity provider to confirm the live
witness before handing rows to Index.

`ScanLocaleReplay` uses unary responses bounded by the default 4 MiB gRPC receive limit.
The adapter can return a shorter prefix than the requested entry limit, with `next` naming
the last returned Product/locale key. Continue until `next` is absent. The Index source
accepts these shorter pages. Entries keep their full contents and owner revisions.
An individual entry that cannot fit returns the nonretryable
`product.locale_replay_entry_too_large` error.

The transport tests cover JSON escaping, continuation preservation, terminal empty pages and
oversized individual entries. `LoadLocaleReplay` remains an exact-key operation with its
existing unary message-size constraint.

On 2026-11-22 the separate-process PostgreSQL fixture in
[`channel_convergence_service_postgres.rs`](../tests/channel_convergence_service_postgres.rs)
passed with eleven localized entries carrying 512 KiB descriptions, verifying pagination
across the gRPC size boundary and two locales of the same Product without missing or duplicate keys.

