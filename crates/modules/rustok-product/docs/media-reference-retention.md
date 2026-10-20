# Product image / Media retention workflow

Current as of 2026-10-18. Decision: **Accepted**. Implementation: **in progress**;
source implementation is present, but Rust build/database/live transport gates have
not been executed in this workspace. This is not deployment certification.

## Ownership and ordering

Product owns `product_images.id`; Media owns the asset and durable hold. The exact
hold identity is `(tenant, owner_module = "product", reference_id = image.id, media_id)`.
Repeated use of one asset has distinct image IDs/holds. Reordering and alt-text edits
never change that identity. Browser inputs still cannot choose image IDs.

`ProductMediaReferencePort` is an explicitly Product-owned consumer projection.
Product does not import Media, its tables, or its service. The host adapts the public
Media ports. `MediaAssetPorts::new(Arc<Owner>)` publishes read and write capabilities
from one owner instance, including `GrpcMediaProvider`. The server `media-grpc` feature exposes the control client without depending on Profiles;
`HostRuntimeContext::with_shared_value(MediaAssetPorts::new(Arc::new(provider)))`
selects it programmatically. No new unauthenticated environment-based remote endpoint
or whole-module extraction certification is implied. Separate unpaired trait
objects are insufficient and must not trigger an embedded fallback.

1. Authenticate tenant/actor, enforce module/permission policy, admit the Product receipt.
2. Completed receipts replay without Media IO. First/reclaimed execution gets its image
   UUID from the stable receipt operation ID. Trusted direct internal service calls
   allocate a fresh UUID; foreign callers must still use the owner command port.
3. Validate input/target and legacy provisioning coverage; retain through Media, then
   confirm the exact current hold. Historical Media receipt success alone is not proof.
4. Only then begin the Product transaction, lock/recheck the parent, insert image and
   provisioning marker, project the response, and commit rows + event + Product receipt.
5. Local failure/unknown commit never issues compensating release. It may leave an
   orphan hold, deliberately favoring protection over premature asset deletion.

For image/Product deletion, the Product transaction enumerates exact image references,
inserts `product_media_releases`, removes relations, and completes event/receipt atomically.
No Media call runs inside this transaction. Committed deletion success is not overturned
by a later release failure. The role-gated singleton worker delivers the committed queue,
with bounded pages, due-time backoff, deterministic child keys, current-absence
confirmation, and application shutdown cancellation. Process restart or cancellation
before acknowledgement leaves work replayable. Media retain/release IO is capped at
2 seconds (or the caller's shorter deadline). Audit/backfill port calls also have a
whole-call deadline; reduce page size for slow remote owners.

The host worker drains through one `ProductCatalogCommandRuntime` snapshot, which
uses its embedded CatalogService's database and Media capability. It does not join a
host-default database to a separately published reference port. Its lifecycle is
reserved atomically before spawn and is compiled with `mod-product`, not `mod-media`:
a host may supply a neutral external Media port without the embedded Media crate.
External Product owners never fall back to local queue delivery. Changing the selected
owner while work is pending still requires an explicit drain/cutover; a runtime snapshot
is not a fleet-wide handover fence.

Deferred delivery receives a fresh bounded 2-second attempt budget, rather than
reusing an expired/too-short user request budget; actor, logical key and tracing
provenance remain stable. The queue stores the original authorized operation context for stable delivery/audit,
not a bearer token. Operator responses never expose stored actor/claims/context. Local queue status remains
available even when Media capability is absent; audit/backfill then fail closed. Remote
Media interceptors must establish trusted tenant/principal authority and authorize the
reference read/retain/release operations; principal identity must remain stable across
retries. Network payload claims are not a replacement for `TrustedMediaAuthority`.

## Failures and replay

- Missing provider, incomplete legacy backfill, timeout, unknown outcome, protocol
  mismatch, and remote authorization/configuration failure are unavailable/retryable.
  They must not finalize a caller's valid Product request as rejected.
- Authoritative Media not-found is deterministic Product validation. Nil asset IDs
  and invalid local input are rejected before Media writes.
- Receipt replay is not a new admission to Media. Updating/reordering images requires
  no mutable Media preflight; metadata changes do not create/remove hold identities.
- Only a committed Product removal queue row authorizes automatic release. A worker
  additionally refuses live image IDs and mismatched stored tenant provenance.
- Diagnostic orphan observations never authorize release. In-flight retain, unknown
  commit, and a racing backfill can all legitimately produce conservative orphans.

## Operator HTTP contract

Paths below are relative to the mounted Commerce HTTP root. All require trusted
matching tenant/auth/request context; no tenant/actor comes from the JSON body.
The paths and typed responses are registered in Commerce OpenAPI.

Missing effective maintenance permissions are `PortErrorKind::Forbidden` at the native
port boundary and HTTP 403, not input validation or provider unavailability. Protected
OpenAPI paths advertise 400/401/403/503. Invalid context/pagination remains invalid input;
timeouts and missing capabilities remain unavailable. The hold-page audit batches its
local image resolution in one tenant-scoped query instead of one query per hold.

| Route | Permission | Parameters / result |
|---|---|---|
| `GET /admin/products/media-references/audit` | effective `products:read` | `after_image_id`, `after_hold_id`, `limit` (1..100); missing/mismatch/orphan issues and independent owner cursors |
| `POST /admin/products/media-references/backfill` | `products:manage` | mandatory `Idempotency-Key`, JSON `{ "after_image_id": null, "limit": 100 }`; examined/retained/failed image IDs, cursor, `has_more`, `legacy_images_remaining` |
| `GET /admin/products/media-references/releases` | effective `products:read` | `after_image_id`, `limit` (1..100); exact queued image/asset IDs, attempts, next due time, sanitized error code and pagination |

A privileged native operator can use the same `ProductMediaMaintenancePort` from the
selected `ProductCatalogCommandRuntime`; REST is an adapter, not another persistence
implementation. External Product command owners must provide their own maintenance
and release-worker deployment; the host must not decorate an external command proxy
or inspect its database. No remote Product command deployment is newly certified here.

Audit uses bounded exact lookup for Product rows plus a bounded Media-owner hold page.
The two owners do not provide a distributed snapshot. Keep each cursor independently
and stop a scan only when both `*_have_more` flags are false. Duplicate/out-of-scope or
non-progressing owner pages are refused. A pending queued removal can also appear as
an orphan hold: consult `releases` before classifying it as unaccounted work.

## Safe legacy cutover / rollback

1. Enter a maintenance window: stop legacy Product image writers and Media destructive
   cleanup/delete activity while unprotected legacy rows are being provisioned. A
   Product-only barrier cannot retroactively protect assets before their first retain.
2. Back up Product/Media owner data; apply additive migration
   `m20261018_000037_create_product_media_releases`. It runs transactionally, including
   SQLite, and adds no FK to foreign owner storage.
3. Publish one Media owner pair, then compose the embedded Product owner. A catalog-only
   deployment may omit Media, but new image writes then fail closed. Media remains an
   optional deployment capability, not a Product crate/persistence dependency.
4. Backfill each tenant in bounded pages using existing image IDs. Keep the same run
   key on uncertain retry. Advance only from a successful response, retaining failed
   page/image IDs for review; do not skip failures merely because `has_more` is false.
   An explicitly new repair attempt after a finalized/refused Media receipt needs a
   deliberate new run key, not an automatic timeout-based rotation.
5. Confirm `legacy_images_remaining = false`, rerun the full audit, resolve missing or
   mismatched holds, and inspect the release queue. Product coverage markers record
   provisioning history, **not** current Media truth; audit reads the owner itself. A sealed
   per-tenant readiness row makes subsequent admission an indexed point lookup rather
   than rescanning all tenant images; only a fully covered legacy set or an already
   admitted first image may seal it. Zero-legacy writer cutover is mandatory.
6. Resume image admission and Media cleanup only after the verification gates below.
   Keep a background-worker role active. Drain committed release work before disabling
   the owner/provider, changing authority, or retiring the deployment.

The migration down path refuses pending releases or protected live relations. Drained
rollback takes a PostgreSQL table lock and removes only the empty owner-local structures;
it never silently forgets a release obligation. Generic migration lifecycle fixtures use
an empty/drained retention state, not a production rollback approval.

There is intentionally no automatic orphan-release or mismatch-reassignment endpoint.
Manual repair needs proof about owner receipts/in-flight writes beyond a single absence
snapshot. Never release an unexplained hold just to make the audit green.

## Verification and remaining gates

Executed evidence is recorded in the implementation plan. Source checks are regression
locks only. Added Rust tests cover Product receipt replay with unavailable Media,
duplicate asset use, committed image/Product deletion, release failure, insertion/deletion
rollback, cross-tenant refusal, legacy backfill/admission, bounded diagnostics, context,
permissions, deadline enforcement, and host paired composition. A shared real-database
protocol fixture targets SQLite and PostgreSQL, including coverage FK cascade and
pending-work rollback refusal; its Media port is a fake, explicitly not GC proof.
Deterministic queue races now cover concurrent acknowledgement/deletion and a newer
retry schedule arriving before a stale failure. Failure metadata updates use a CAS
on image ID and original attempt count: zero updated rows do not abort the batch.
The PostgreSQL fixture also invokes both race scenarios.

Additional server test sources use real Media persistence/local storage and separate
Product persistence, both natively and through a live loopback gRPC server. They cover
retention/delete fences, cross-tenant refusal, historical retain/release receipts,
racing repair, refusal to release live images, and queue draining before final deletion.
Product rows and removal intents are fixture-seeded: these are not mounted Product
command/receipt transaction tests or crash/restart proof. All added Rust tests remain
**unrun** here; source presence is not evidence that the scenarios pass.

Required maintainer execution (Rust toolchain **1.96** and repository prerequisites):

```sh
cargo check --locked -p rustok-product -p rustok-commerce
cargo test --locked -p rustok-product --lib
cargo test --locked -p rustok-product --test media_reference_protocol
cargo test --locked -p rustok-commerce --test catalog_service_test
cargo test --locked -p rustok-media --test lifecycle
cargo test --locked -p rustok-media-transport --test port_conformance
# With the configured PostgreSQL admin URL:
cargo test --locked -p rustok-product --test media_reference_protocol -- --ignored
cargo test --locked -p rustok-product --test postgres_migrations -- --ignored
cargo test --locked -p rustok-server --features mod-product,mod-media product_media_reference::
cargo test --locked -p rustok-server --features mod-product,mod-media,media-grpc product_media_reference::
# Also compile/run every supported native/CSR host feature combination.
```

Before implementation status becomes Completed: compile all supported host/native/CSR
feature combinations; execute real native and remote Product→Media integration, Media
GC fences, ambiguous commit/lease reclaim, process crash/restart and multi-replica release
races; exercise authenticated mounted routes and both admin image-command paths. The
current workspace has no cargo/rustc or PostgreSQL service, so these remain open. The
available WASM rustfmt wrapper formats source; it is not a compiler or cargo-fmt check.
