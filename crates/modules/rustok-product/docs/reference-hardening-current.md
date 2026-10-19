# rustok-product canonical large-reference hardening cursor

Status: `large_reference_hardening_in_progress_after_blog_v1`.

Baseline: `ad04a17e8252ae9d23e99af4b7ad7ffaebdfd10e` (Blog canonical reference v1 source certification, #4073).

This document is the current architecture cursor for turning `rustok-product` into the
large-owner companion reference to `rustok-blog`. It does not replace the historical
implementation plan or Product-specific Index, Translation, Category, FBA, or variant-axis
records. It records only the cross-module canonical-reference gap against the accepted Blog v1
contract.

## Reference target

Product must implement the same responsibility and safety vocabulary as the Blog reference while
retaining Product-specific scale, catalog-schema, pricing/inventory collaboration, Index replay,
Translation targets, and external read-port responsibilities.

A large module is not canonical merely because its individual capabilities are mature. Owner API,
mutation semantics, lifecycle, concurrency, tenant authority, public errors, integrations, UI
adapters, persistence visibility, and machine guards must agree as one architecture.

## Slice 1 — native owner context and public-error boundary

Source-hardening in the current branch closes the first bounded gap:

- admin catalog and schema native server functions resolve `HostRuntimeContext` fallibly;
- storefront catalog-list and detail/search-option native functions resolve host context fallibly;
- missing host runtime/event-bus composition uses the Product-owned safe public-error envelope;
- raw extractor/runtime dependency text is not rendered at public native boundaries;
- schema/admin native context fails closed when `AuthContext.tenant_id != TenantContext.id`;
- the existing Product public-error contract now exposes a generic owner-safe internal descriptor;
- existing admin/storefront/error-safety verifiers guard the fallible context and tenant-authority
  pattern instead of requiring `expect_context`;
- storefront catalog native source evidence records the new fallible-host-context invariant.

This slice does not claim compile, runtime, mounted-parity, or transport evidence. Maintainer
execution remains separate.

## Confirmed open canonical-reference gaps

The following gaps are confirmed from current source and block Product from being called the
canonical large-module reference:

1. **Public persistence API / crate layout**
   - `src/lib.rs` still exports `pub mod entities`.
   - `ProductModule` runtime registration/composition remains in `lib.rs` rather than a
     Blog-style thin facade plus `module.rs`.
   - `ProductStatus` is owned by the persistence entity module and is imported by public DTOs.
   A bounded ownership/public-API migration is required before other modules copy this layout.

2. **Nullable mutation semantics**
   - Product document nullable fields now use `Patch<T>` (`Keep / Set / Clear`).
   - Variant nullable fields now use owner `Patch<T>` through GraphQL, Next and Leptos update
     drafts. Keep is omitted, Clear writes NULL, Set preserves values; the obsolete foundation
     update DTO is removed. See [nullable Variant patches](./variant-nullable-patches.md).
     Source invariants pass; compilation, mounted UI/GraphQL and SQLite/PostgreSQL gates remain open.

3. **Business optimistic concurrency**
   - Product now owns an editorial `revision`; document updates compare the caller predecessor
     after taking the Product lock. External document updates require the revision at ingress.
   - `None` remains the documented owner-internal unconditional mode; lifecycle-only updates may
     omit it. A supplied revision is still checked, including before same-status no-ops.
   - Typed EAV writes remain a distinct receipt-bound surface without an editorial predecessor
     input; the Next and Leptos document+value save workflows are still multi-command and non-atomic.
   - Product/Variant `index_revision` remains an Index watermark, never an editor CAS token.
   This is bounded source hardening, not complete aggregate concurrency certification.

4. **Owner error classification**
   - Flex-to-Product conversion now exhaustively classifies all eight variants: storage stays
     Database, a rejected fixed entity type is Core/Internal, and payload failures use static
     validation copy. The catch-all and command mapper raw error logging are removed.
   - Shared Product terminal receipt policy requires non-retryable Validation/NotFound/Conflict;
     invariant, permission and infrastructure classes remain processing. See the
     [dated error/receipt continuation](./product-public-error-diagnostic-safety.md#flex-and-receipt-failure-continuation-2026-10-18).
   - Taxonomy sync/read adapters and nested Core classification are now exhaustive and static-copy:
     domain conflicts and permission failures retain their kind; corrupt reads are internal, not
     client Validation. All affected Product/schema/read/Translation/HTTP boundaries delegate the
     canonical Core policy. See the [Taxonomy/Core continuation](./product-public-error-diagnostic-safety.md#taxonomy-and-nested-core-continuation-2026-10-18).
   - Seven additional Rust scenario sources are unrun. Compilation, mounted/runtime verification,
     and broader typed Translation/Index/Pricing error provenance remain open.

## Required audits not yet classified as defects

These areas must be audited against Blog v1 before certification; this cursor deliberately does not
prejudge their result:

- multilingual write provenance and missing-translation storage invariants;
- lifecycle transition ownership and whether transports/services share one policy;
- metadata shadow copies versus typed canonical state;
- Product/Commerce GraphQL and HTTP ownership boundaries;
- Category/Taxonomy canonical projection semantics;
- Search/SEO/Index/Translation/Pricing/Inventory integration ownership;
- migration invariants and source/upgrade compatibility;
- admin/storefront responsibility split and file-size/layout profile;
- event/idempotency semantics across Product, Variant, Index refresh, Translation and FBA;
- complete public error semantics beyond the native context slice.

## Accepted fulfillment-requirement cutover

Product currently admits digital products but does not yet provide the complete typed
contract needed to keep them out of shipping flows. Product must own an explicit
digital/physical fulfillment requirement and publish the compatible physical profile
snapshot needed by Cart/Order. A digital product or variant must have no synthetic
default shipping profile and must remain purchasable in a digital-only Commerce
composition with Fulfillment absent. The canonical cross-owner matrix is in
[`docs/architecture/settings.md`](../../../../docs/architecture/settings.md).

## FBA status

`ProductCatalogReadPort / product.catalog_read.v1` remains `boundary_ready`.
The substantial gRPC/provider/consumer source work and maintainer attestations do not by themselves
promote Product to `transport_verified`. The registry's retained separate-process/runtime evidence
requirements remain authoritative.

## Ordered continuation

1. Merge the native owner-context/redaction slice after source review.
2. Re-read fresh `main`.
3. Make Product persistence entities private and split the crate root into a thin facade plus
   runtime/module composition without breaking legitimate owner contracts.
4. Introduce explicit `Patch<T>` semantics for nullable Product edits.
5. Add a dedicated Product business revision and mandatory predecessor CAS through owner service,
   GraphQL/native transports, and admin edit state.
6. Audit and harden lifecycle, multilingual/storage provenance, metadata ownership and events.
7. Audit integrations/FBA and finish Product-specific large-reference machine guards.
8. Only then mark Product as the canonical large-module reference and continue to Pages, Forum,
   Taxonomy, and remaining modules/libs.

## Catalog receipt implementation follow-up (2026-10-17)

All fourteen catalog commands now record transaction-derived public results before commit, and
`ProductCatalogCommandPort` admits/replays/finalizes durable owner receipts. The Product/Variant
projection connection reaches tags, Taxonomy names, metadata, prices, inventory and axes. Eleven
source guard mutation tests cover this wiring; new Commerce/Taxonomy Rust regressions remain
unexecuted without Cargo. The subsequent atomic editorial source composes Product document/category/EAV/publication, but
does not certify database execution, whole-screen Variant atomicity, live reload/child-command Next retry evidence,
Media admission/replay ordering, nullable Variant or PostgreSQL concurrency gaps, and does not
promote Product reference status. See the current [write boundary](./write-boundary.md).


Next save/status/delete now retain immutable attempt identity in a scoped local browser journal,
restore requests across reload/navigation, serialize duplicated intent URLs with Web Locks and
retain terminal tombstones. Missing Product reads can mount recovery-only UI. Receipt-finalized
domain refusals are distinguished from context/admission failures in GraphQL; nonretryability
alone is not conclusive. Node simulations and scoped editor typechecking do not certify real
browser/backend reload/concurrency execution or child-command retries. See the current owner plan.

Leptos Product Save no longer follows its transaction with a hidden Variant write. Variant fields
now identify a separately saved target; action acknowledgements do not refetch over the other
block's unsaved fields, and ordinary edit Save no longer implicitly publishes. This is explicit
transaction separation, not whole-screen atomicity or verified pending-request recovery. Five
Rust tests and mounted/runtime execution remain unrun; source mutation guards cover the
wiring. Shipping Profile preflight before Product receipt admission remains a confirmed open gap.

### 2026-10-18 Media retention follow-up

The Product-image ADR is now Accepted on explicit implementation request. Source implementation
replaces the read-only decorator with receipt-bound retain/current-hold confirmation, Product-owned
stable image IDs, atomic provisioning coverage, a durable committed-removal release queue, bounded
background delivery, and operator audit/backfill/status. Product consumes Media through its own
port and an atomic host-selected Media owner pair; no foreign persistence dependency or opaque
write path remains. Legacy provisioning gates new image admission; orphan scans never release.

Implementation remains In progress. The continuation reran seven workflow source/mutation
suites (**137 PASS**, including **22** retention cases), plus gRPC authentication (**8 PASS**)
and the catalog-schema fixture suite. It fixed stale failure updates across release-worker
replicas and added real native/loopback Media owner integration sources with separate owner
databases. Product rows/intents in those fixtures are seeded, not mounted command/receipt proof.
There are now **24 added Rust tests, all unrun**, including both queue-race scenarios and the
two real-owner scenarios. Historical 86 Admin/four storefront/two scoped TS checks were not
rerun in this continuation. Real database/transport/restart execution still requires cargo/rustc. See the [runbook](./media-reference-retention.md)
and latest owner plan entry. This does not advance module readiness or the Blog-v1 audit cursor,
certify remote Product commands, or resolve the separate Shipping Profile replay-preflight gap.


The subsequent implementation-only continuation binds worker database/Media selection to
one command-runtime owner snapshot, atomically reserves worker startup, removes the
embedded-Media feature requirement from neutral delivery, preserves Forbidden at native/HTTP
maintenance boundaries, and batches local hold-page audit reads. No test suites were rerun;
updated scenario/mutation sources await maintainer execution. Owner handover/fleet fencing
and the existing runtime gates are not certified by these source changes.


Shipping Profile preflight has now moved behind receipt admission in source. Product's
four owner methods validate through ProductShippingProfilePort, including initial Variant
overrides; completed receipts do not call the policy. Missing capability/outages remain
retryable, and host/embedded Commerce fallback composition supplies the provider without
adding a Product-to-Commerce dependency. Two new owner scenario sources are unrun. Earlier
references to this gap above are historical; see the current write-boundary contract and
latest plan entry for execution gates and remaining historical verifier drift.


The following diagnostic continuation supersedes the three historical verifier failures:
source guards now target typed Product read errors, owner-port commands and Order port
errors with bounded mapper assertions. It also fixes static Shipping-option public
messages/log projections, Order Forbidden classification, authenticated scope agreement
before Product/Order IO, and request correlation propagation. Four unit scenario sources
were added but not run; build, mounted/runtime and broader recovery gates remain open.


### 2026-10-18 Pricing consumer continuation

Product storefront native pricing now consumes the Pricing-owned safe public descriptor
instead of classifying Foundation wrappers locally or logging raw errors. Pricing canonical
ports share that classifier, with exhaustive Core11/Rich11 handling. Commerce pricing mutation
presentation is static and query failures retain owner retryability. Ten Pricing owner entry
logs are bounded. See the [owner contract](../../rustok-pricing/docs/pricing-owner-port-error-safety.md#nested-error-and-public-consumer-continuation-2026-10-18).
This does not close complete Pricing-context storefront evidence, the broader diagnostic
inventory, or the Product/Media/Shipping readiness gates. Five added Rust scenarios are unrun;
source guards/formatting are not compilation or mounted runtime evidence.


### 2026-10-18 Native storefront context admission follow-up

All four native storefront owner-read entrypoints now propagate canonical `RequestContext`
rejection instead of falling back to an absent context and potentially widening channel scope.
Valid anonymous and no-channel contexts remain allowed. List/facet tenant-failure events now
contain bounded shape only; required-context rejection uses the existing safe Product descriptor
and a fresh reference. See the [current contract](./product-public-error-diagnostic-safety.md#native-storefront-context-admission-continuation-2026-10-18).
Nine scoped source guard entrypoints and three-file formatting/idempotence checks pass; five
new negative source scenarios are unrun. No compilation, mounted execution, readiness promotion
or full Product audit is claimed. Host trusted-locale composition must be verified by the maintainer.


### 2026-10-18 Admin catalog context admission follow-up

Native admin list/facets no longer swallow canonical RequestContext rejection as an optional
locale context. The existing permission and requested/auth/current tenant checks remain intact;
owner reads only occur after successful context admission. Rejection diagnostics use static
facts and a fresh safe Product reference. See the [current contract](./product-public-error-diagnostic-safety.md#admin-catalog-context-admission-continuation-2026-10-18).
This is bounded source work, not a claim that admin queries lacked tenant isolation. Two new
source scenarios are unrun; compilation, middleware composition and mounted execution remain open.


### 2026-10-18 Schema/tag read diagnostic follow-up

Nine schema/tag read diagnostic events now use the existing Product fact projections;
raw error/message/code/context/attribute identity values are removed without changing
read policy, public classification, copy or retryability. See the [current contract](./product-public-error-diagnostic-safety.md#schema-and-tag-read-diagnostics-continuation-2026-10-18).
Eight scoped source guards and four Rust formatting/idempotence checks pass. Three added
Rust scenarios are unrun; compilation, log capture and mounted execution remain open.
This is not a full diagnostic inventory closure or readiness promotion.


### 2026-10-18 Storefront attribute degradation diagnostics

The two deliberate best-effort skip paths now share one bounded warning helper; stored
attribute identity/code/type/error payload is removed. Omission, JSON exclusion, locale and
valid scalar behavior are preserved, not replaced with whole-request failure. See the
[current contract](./product-public-error-diagnostic-safety.md#storefront-attribute-degradation-diagnostics-continuation-2026-10-18).
Six scoped source guards and one Rust formatting/idempotence check pass; four added Rust
scenarios are unrun. No compile, log-capture or mounted runtime evidence, readiness promotion
or global stored-value integrity certification is claimed.


### 2026-10-18 Storefront HTTP read diagnostic follow-up

Four compatibility HTTP read events now use bounded caller context, owner payload shape and
pagination failure flags. Public mapping, Core/Conflict delegation and checked pagination are
unchanged. See the [current contract](./product-public-error-diagnostic-safety.md#storefront-http-read-diagnostics-continuation-2026-10-18).
Six scoped source guards and one Rust formatting/idempotence check pass; two new Rust scenarios
are unrun. No compilation, tracing capture, mounted HTTP evidence or broader diagnostic closure
is claimed; readiness and full-stack audit status are unchanged.


### 2026-10-19 Catalog command diagnostic follow-up

All seven command-port events share existing bounded context/error projections, including
write-policy rejection, revision refusal and receipt replay/failure. Public classification,
write admission and receipt ordering are unchanged. See the [current contract](./product-public-error-diagnostic-safety.md#catalog-command-diagnostics-continuation-2026-10-19).
Six source guards, one Rust formatting/idempotence and one JS syntax check pass; two Rust
scenarios remain unrun. No compilation, tracing capture or DB/runtime evidence; no readiness
or global audit promotion. Main inspected at `4905561ef0b9e81b3fb187efa9f54384442d036e`.
