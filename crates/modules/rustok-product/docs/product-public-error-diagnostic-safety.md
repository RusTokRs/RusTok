# Product shared public-error diagnostic safety

Status: **source-ready / unvalidated**

## Initial shared-mapper slice (historical source record)

## Scope

This slice closes the payload-diagnostic gap in Product-owned `map_product_public_error`, the shared owner mapper used by GraphQL and native admin/storefront consumers.

The mapper still accepts `CommerceError`, creates one `ProductPublicError`, and preserves its safe message, stable code, retryability, generated correlation id, and display reference. Consumer adapters, requests, responses, fallback behavior, and transport composition are unchanged.

Coverage in the initial snapshot: all eight current `CommerceError` variants remained mapped
(the live enum and dated continuation below now cover eleven variants):

- database;
- product not found;
- duplicate handle;
- duplicate SKU;
- validation;
- no variants;
- cannot delete published;
- core failure.

## Retained diagnostic shape

The private structured event retains static operation, boundary, public code, retryability, correlation id, and a closed error variant.

Dynamic owner payload is represented only by aggregate shape:

- text field count and total character length;
- UUID field count and non-nil count;
- opaque payload presence for database and core failures.

Database/core errors, product UUIDs, handles, locales, SKUs, validation messages, and the complete `CommerceError` debug/display payload are not recorded.

## Preserved behavior

- every public message is unchanged;
- every public code is unchanged;
- retryability is unchanged;
- `Uuid::new_v4()` still creates the correlation id;
- `ProductPublicError::Display` still renders message, code, and reference;
- the root Product export and all GraphQL/native consumers are unchanged.

## Evidence

- `crates/modules/rustok-product/contracts/evidence/product-public-error-diagnostic-safety-source.json`
- `crates/modules/rustok-product/contracts/evidence/product-public-error-diagnostic-safety-source-review.json`
- `scripts/verify/verify-product-public-error-diagnostic-safety.mjs`

## Remaining gaps

Compile, verifier execution, and mounted GraphQL/native runtime evidence remain open. The broader ecommerce mapper cleanup remains open. This source slice does not promote Product FFA/FBA or transport status.

No test, verifier, formatter, Cargo, workflow, or CI command was executed for this source slice.


## Flex and receipt-failure continuation (2026-10-18)

Current source keeps all eleven Product error variants mapped. The initial evidence files above
retain their original source-only validation disclosure; the executed checks for this continuation
are described separately here. Neither record is runtime certification.

### Flex attached values

Product supplies the fixed `flex::PRODUCT_ENTITY_TYPE`, not a caller-selected entity type.
Its `From<FlexError>` adapter now exhaustively classifies all eight Flex variants:

| Flex error | Product class | Receipt treatment through the owner port |
| --- | --- | --- |
| Database | Database, preserving its internal cause | Unavailable/retryable; no terminal failure |
| UnknownEntityType | Core/Internal with static registration-failure copy | Invariant failure; no terminal failure |
| NotFound | Validation with static missing-definition copy | Domain refusal, not missing Product |
| DuplicateFieldKey | Validation with static duplicate-key copy | Domain refusal |
| InvalidLocale | Validation with static invalid-locale copy | Domain refusal |
| InvalidFieldKey | Validation with static invalid-key copy | Domain refusal |
| TooManyFields | Validation with static field-limit copy | Domain refusal |
| ValidationFailed | Validation with static validation copy | Domain refusal |

The Database branch already preserved the storage class before this continuation; no prior
storage-to-validation bug is claimed for that branch. The removed catch-all allowed configuration
errors to become validation and would silently accept future Flex variants into that class.
No Flex-supplied key, locale, entity name, UUID or validation payload becomes Product error copy.
The database cause remains internal; the command mapper must not render it in diagnostics.
This donor-specific policy does not redefine the generic Flex transport's entity-selection policy.

### Terminal receipts are a positive domain policy

The shared Product failure finalizer requires **both** non-retryability and a known domain kind:
`Validation`, `NotFound` or `Conflict`. Non-retryability alone is not evidence of a terminal
business outcome. `Forbidden`, `Unavailable`, `Timeout` and `InvariantViolation` do not become
failed receipts, even if marked non-retryable. Every retryable failure also remains processing.
Catalog and schema writes share this policy; the existing Product/Variant/Image and schema/EAV
Translation finalizers delegate to the same helper. No success, replay, scope, permission or
rollback ordering is changed. Admission failures remain outside owner failure finalization.
The existing outbox lease expiry/reclaim policy is unchanged; this is not immediate lease release.
Previously persisted terminal receipts are not rewritten or invalidated.

Public invariant classification remains non-retryable with the existing static failure code.
That does not make the logical command definitively rejected: the mounted Product GraphQL
outcome policy already treats this class as unknown. The processing lease is retained for a later
reclaim; clients must not rotate keys merely because `retryable` is false.

### Diagnostic limits

Catalog command mapping retains static kind/code, trusted tenant/correlation context and operation,
without `error_details = %error`. Catalog and schema replay-decode logs do not render serde error
text derived from the stored response. This is a bounded mapper change, not a claim that all
Product, Translation, Taxonomy, Flex or outbox logging has been audited.

### Verification and open scope

- Source entrypoint: `npm run verify:product:error-boundary`.
- Six new Rust regression sources cover Flex classification/redaction, the 14-case kind/retryability
  matrix, skipped non-domain finalization, persisted domain-versus-invariant/permission receipt
  admission, and command-port class/message propagation. **All are unrun.**
- Existing receipt source guards and the affected mutation fixture were migrated to the shared
  terminal predicate. Updating a fixture is not running its suite.
- WASM formatting/idempotence and standalone source checks are not Rust typechecking or execution.
- Compilation, SQLite/PostgreSQL execution, lease expiry/reclaim after configuration repair,
  mounted GraphQL/native/HTTP behavior and log capture remain open. No tests or CI were run.
- The broader ecommerce mapper cleanup remains open, particularly Taxonomy-to-Product catch-all
  adapters, nested Core error classification, Translation diagnostics and full public-boundary parity.
  This continuation does not mark Product as the canonical reference or advance FFA/FBA readiness.


## Taxonomy and nested Core continuation (2026-10-18)

Status: **source implemented; compilation and runtime gates open**. This supersedes the earlier
open Core/Taxonomy classification items, not their execution gates. The live Product error enum
now has twelve variants: a generic owner `Conflict(String)` was added rather than reusing duplicate
handle/SKU errors or encoding Taxonomy conflicts into a Validation message.

### Contextual Taxonomy policies

Both adapters in `catalog_schema_service.rs` exhaustively match all ten Taxonomy errors.
No Taxonomy-supplied slug, alias, key, locale, UUID, error string or validation message becomes
Product public copy.

| Taxonomy class | Category synchronization | Category/projection reads |
| --- | --- | --- |
| Database | Product Database / retryable unavailable | Same |
| Forbidden | Core Forbidden / access denied | Same |
| Internal | Core Internal / invariant | Same |
| TermNotFound | Referenced-resource NotFound, not missing Product | Invariant: expected owner projection is missing |
| Duplicate key/slug/alias, Conflict | Product Conflict | Invariant: not a user mutation |
| TranslationRevisionExhausted | Product Conflict | Invariant |
| Validation | Static Product Validation | Invariant |

Corrupt persisted projections (missing owner row or localized copy, incompatible canonical key/scope,
negative sibling position, missing ancestry or cycle) now produce static Core/Internal failures.
They are not client Validation and cannot become terminal failure receipts. User-input validation
for category slug, position and localized name/description remains in the write admission path.

### Canonical nested Core classification

`error::core_port_error` and `error::core_error_code` share one exhaustive closed policy:

| Core class | Product port class | Retryable |
| --- | --- | --- |
| Database | Unavailable | Yes |
| Cache, External | Unavailable | Yes |
| Auth, Forbidden | Forbidden | No |
| InvalidIdFormat, Validation | Validation | No |
| NotFound | Referenced-resource NotFound | No |
| Serialization, Scripting, Internal | InvariantViolation | No |

Core Auth is a forbidden capability call, not a new authentication challenge; Product request
admission remains responsible for the authenticated actor. Referenced-resource NotFound uses
`product.dependency_not_found`, not a fabricated missing Product UUID. Core internal/invariant
port codes are normalized to `product.invariant_violation`, including schema/Translation adapters;
other owner error cases retain their own existing codes. Conflict uses `product.conflict`.

This policy is wired through catalog/schema writes and reads, Product read diagnostics, storefront
read capabilities, Product/Variant/Image Translation apply/progress and schema/EAV Translation,
Media maintenance, and the shared public mapper used by native and direct GraphQL consumers.
The HTTP Product admin/storefront adapters share the Core policy: forbidden is 403, storage or
external outage is 503, validation is 400, referenced-resource missing is 404, invariant failure
is 500, and generic Product conflict is 409. Existing admin/storefront internal fallback codes
remain distinct. Payloads and technical error causes are not copied into public messages.

The Pricing service's explicit Product error adapter carries the added Conflict case as an existing
RichError Conflict, without changing Pricing ownership or claiming its whole public pipeline is audited.
Two schema/tag read mappers no longer log the complete owner error.

### Receipts and definitive GraphQL outcomes

The prior domain-only receipt finalizer is unchanged. It can finalize the normalized non-retryable
Validation/NotFound/Conflict classes; permission, invariant and infrastructure failures remain
processing. Product GraphQL now recognizes the added known domain codes only with an actual
Validation/NotFound/Conflict kind. A forbidden/invariant error with a misleading domain code is
still unknown. No caller key, lease duration, tenant/module/permission admission, transaction,
success replay or previously stored receipt is rewritten by this slice.

### Source evidence and unrun gates

Seven new Rust scenario sources cover the eleven Core variants with public redaction/retryability,
all ten Taxonomy sync/read variants, command/receipt classification, nested HTTP statuses,
schema-read wildcard masking, and GraphQL domain kind/code outcomes. Existing missing-owner/cycle
scenario sources now assert Core/Internal rather than merely any error. **No tests were run.**

The error-boundary guard now covers exhaustive Core/Taxonomy policies and each affected mapper.
The three historical category create/read/hierarchy guards were stale against the already-existing
Migration 32 same-ID/all-backend donor retirement; they now inspect current canonical writes,
bounded owner reads, locale resolution, hierarchy authority and internal projection failure classes,
and reject the retired donor/binding/closure paths. They do not reinstate compatibility storage.
Sixteen standalone source guards pass; 27 Rust files pass WASM formatting/idempotence. Source
checks and formatter parsing are not Rust typechecking or database/runtime evidence.

Open: Rust/feature compilation; all scenario execution; real SQLite/PostgreSQL failures/rollback;
mounted authenticated HTTP/GraphQL/native and Translation behavior; original-result replay and
lease-reclaim/restart behavior after dependency repair; log-capture/privacy verification. Broader
error provenance in other typed Translation/Index/Pricing owners remains unaudited. No runtime,
CI, FFA/FBA readiness or canonical-reference promotion is claimed.


## Native storefront context admission continuation (2026-10-18)

Status: **source-implemented / execution unvalidated / unintegrated**.
Main snapshot: `6716323bc5630dd87939988a4507eaf0fea53827`; branch:
`arena/8161797f-rustok`. This bounded continuation does not certify the entire Product module.

### Root cause and trust boundary

Catalog list/facets treated every `RequestContext` extractor rejection as an absent optional
context, and detail/pricing used `.await.ok()`. That could continue a catalog read without
its channel scope. The canonical API extractor does not reject merely because an optional
channel or anonymous user is absent: it rejects missing trusted tenant/resolved-locale
extensions and mismatched tenant identities on trusted auth/channel extensions. Swallowing
that rejection bypasses the existing request-admission contract.

All four Product native storefront read entrypoints now call the shared SSR-only
`native_server_adapter::require_request_context` and propagate failure before tenant
extraction and owner service construction/calls:

- catalog list;
- catalog facets;
- detail and Pricing overlay;
- catalog category/attribute search options.

This is intentionally a behavior change for invalid host/request context. A rejected
trusted context can no longer turn into an unscoped or default-locale read. Missing trusted
resolved locale must be fixed in host middleware, not hidden by the storefront adapter.
The helper delegates to the existing `rustok_api::RequestContext` extractor; it does not
reimplement auth/channel/locale validation or introduce a second admission authority.
No login is required for public reads: a valid context may still have `user_id = None` and
no channel. Those successful requests retain the existing locale/channel selection,
explicit Pricing inputs, filters, pagination, owner factories, DTOs and projections.
Tenant extraction remains mandatory. Search-options validation may reject an empty locale
before context admission, but never calls an owner service before admission.

### Diagnostics and public contract

The shared rejection helper discards the extractor payload without a Debug/Display bound.
It logs only static owner, operation, boundary and code, a rejection flag, and the fresh UUID
from Product's existing internal public descriptor. The same safe code/message/reference
is rendered to the client. This reference is not taken from request headers or resource IDs.

List/facet tenant-extraction failures retain their existing static public context messages.
Their diagnostics retain only context-present, correlation character length, channel-ID
presence, channel-slug character length, and locale character length. Raw rejection objects,
request correlation strings, UUIDs, slugs and locale strings are not emitted. The old optional
context warning and error-taking generic helper are removed, not shadowed by a new logger.
Existing Product/Pricing service public mappers and GraphQL selected-path handling remain intact.

### Source verification and remaining execution gates

- Three affected Rust files formatted with Scalar rust-fmt 0.3.0 using Rust 2024 parsing;
  second formatting pass is identical. This is not a compiler/type check.
- Nine standalone source guard entrypoints passed: native context, storefront boundary,
  media/price, storefront GraphQL errors, shared Product diagnostics, Product error boundary,
  Pricing public errors, facet chain, and storefront attributes.
- The canonical native-context guard now verifies all four required-context handoffs/order,
  bounded list/facet diagnostics and absence of rejection-to-optional fallback. It no longer
  requires raw diagnostic fields or optional-context behavior.
- Five negative source-guard scenarios were added to the existing storefront boundary test
  file (list, facets, detail, search-options fallback and raw channel diagnostics) but **not run**.
  The fixture now includes the facet adapter checked by the canonical guard.
- The existing source-evidence JSON is synchronized with required-context admission and
  records only formatter/source-guard execution; tests, Cargo, CI and mounted evidence stay false.

No tests, Cargo check or mounted requests were executed. The maintainer must verify missing
resolved-locale composition, auth/channel tenant mismatch, valid anonymous/no-channel reads,
channel-scoped catalog/facet/detail behavior and all supported render/deployment profiles.
Broader Product/Commerce diagnostics, header admission and other modules' optional-context
consumers are not certified by this slice. No persistence, transaction, receipt, event or schema
change is made; no dependency is added. The full-stack audit cursor/readiness status is unchanged.


## Admin catalog context admission continuation (2026-10-18)

Status: **source-implemented / execution unvalidated / unintegrated**.
Main snapshot `6716323bc5630dd87939988a4507eaf0fea53827`, branch
`arena/8161797f-rustok`. No full-stack audit cursor or readiness promotion.

Both `admin_catalog_native.rs` read entrypoints (list and facets) previously swallowed
`RequestContext` rejection with `.await.ok()` and continued using an explicit or tenant-default
locale. These reads already checked authentication, effective list/read permissions, and
requested/authenticated/current tenant equality; this continuation does not claim those checks
were missing or that a cross-tenant owner query was demonstrated.

A shared SSR-only helper now delegates to the canonical API RequestContext extractor and
propagates rejection before query construction and CatalogService reads, including when the
caller supplied an explicit locale. Missing trusted resolved-locale composition or a rejected
auth/channel tenant context cannot be hidden by an optional-locale fallback. Successful reads
still prefer a nonblank explicit locale, then the trusted resolved request locale; the tenant
locale remains the owner's translation fallback. Authentication/permissions/tenant admission,
filters, pagination, facet counting, successful DTOs and GraphQL selection remain unchanged.
Admin reads are not converted into channel-filtered storefront reads.

The rejection event uses only static owner/operation/boundary/code, a rejection flag, and the
fresh reference from ProductPublicError::internal(). The same safe descriptor is returned to
the caller. Extractor payload and caller correlation/locale/channel values are not logged.
There is no duplicated auth/channel validator or new dependency/schema/persistence contract.
The helper is local to the existing Leptos admin transport, not a new platform abstraction.

The existing admin facet source guard now checks both gates, mandatory tenant/permission
checks, ordering before owner reads, and bounded rejection diagnostics. Two source-contract
scenarios were added to the existing admin facet test file but **not run**. Rust formatting and
idempotence use Scalar rust-fmt 0.3.0 with Rust 2024 parsing; they are not compilation evidence.
Scoped standalone source verification and JS syntax/diff checks are recorded separately from
unrun tests. Cargo, CI, mounted SSR/hydrate behavior and middleware composition remain open.
Other Product/native/GraphQL paths are not certified by this bounded continuation.


Admin continuation verification record: eight standalone source guard entrypoints passed
(admin facets, admin boundary, admin GraphQL read diagnostics, facet chain, Product error
boundary, Product public diagnostics, storefront boundary, Pricing public errors). Six affected
verifier/test-script files passed JS syntax checks; no test file was executed. `git diff --check`
passed. The facet-chain guard/fixture now require the actual fail-closed context handoff rather
than the retired optional-context expression. The admin-boundary guard/fixture now check typed
attribute patches carried in the already-existing atomic owner draft instead of requiring a
separate attribute-save call in submit; this corrects stale verification of earlier atomic-save
work and introduces no new write behavior. Existing unrun fixture tests were synchronized.


## Schema and tag read diagnostics continuation (2026-10-18)

Status: **source-implemented / execution unvalidated / unintegrated**.
Main snapshot `6716323bc5630dd87939988a4507eaf0fea53827`; branch
`arena/8161797f-rustok`. No readiness or full-stack audit cursor promotion.

The nine diagnostic events in `catalog_schema_read_port.rs` and
`storefront_tag_read_port.rs` now use the existing Product port context/owner fact
projections. The fact types and extractors are crate-visible through private-to-crate
re-exports in `ports`; they are not a new public API, classifier or telemetry DTO.
Other diagnostics helpers remain port-internal.

The affected scope includes read-policy refusal, tenant parse refusal, tag request
validation, missing tag-page Product identities, missing effective-form attribute definitions,
and service-error mapping. Events no longer emit raw CommerceError Debug, admission
code/message text, tenant/correlation values or attribute UUIDs. Retained fields are static
operation/code, a closed PortErrorKind when applicable, retryability, aggregate owner payload
shape, tenant/correlation character lengths, counts and attribute-ID non-nil state.
The UUID parser's error is discarded rather than retained in an unused variable.

Existing request validation, deadline requirements, tenant parsing, missing-owner invariant
outcomes, service delegation, public codes/copy and retryability are unchanged. Nested Core
and generic Conflict mappings remain delegated to the existing Product policy. No persistence,
event, receipt, schema or dependency change is made. This does not certify other port loggers'
correlation handling, remaining service diagnostics, or the whole Product module.

Eight standalone source guard entrypoints passed: Product error boundary, public diagnostics,
Taxonomy hierarchy consumers, schema write port, Taxonomy read cutover, Taxonomy create sync,
storefront boundary and admin facets. The error-boundary guard now checks both complete
read adapters for raw diagnostic escapes, canonical fact reuse and context projection for
every event. Closed `error.kind` Debug is deliberately allowed; complete error Debug is not.
Four Rust files formatted/idempotent with Scalar rust-fmt 0.3.0/Rust 2024 parsing; JS syntax and
`git diff --check` passed. These are source checks, not typechecking or log-capture evidence.

Three new Rust scenarios (schema/tag deadline-and-tenant admission and tag Core/Conflict
classification/retryability/privacy) are **unrun**. No tests, Cargo check, CI or mounted requests
were executed. Maintainer verification must cover compilation, actual tracing output with
poisoned payload/context, admission failures and dependency faults. Owner review/integration
and wider diagnostic cleanup remain open.


## Storefront attribute degradation diagnostics continuation (2026-10-18)

Status: **source-implemented / execution unvalidated / unintegrated**.
Main snapshot `6716323bc5630dd87939988a4507eaf0fea53827`; branch
`arena/8161797f-rustok`. No full-stack audit or readiness promotion.

`services/catalog/storefront_attributes.rs` has an explicit best-effort display contract:
unsupported stored types and missing scalar values are omitted rather than failing a product
detail read. JSON remains a deliberate non-display type, and dictionary values continue to
resolve through options. This bounded continuation preserves those semantics rather than
relabeling every projection omission as a client error or a whole-request invariant failure.

Both warning paths now use one private bounded diagnostic helper. It emits static owner,
operation, boundary, reason and expected-category labels, value/attribute ID non-nil flags,
attribute-code/stored-type character lengths, and the localized flag. It does not emit UUIDs,
attribute codes, stored type text, scalar/translated values or the underlying parser error.
Reason and expected-category arguments are static strings; they do not come from storage.
The existing parse decision is isolated in a private helper used by the actual read loop,
not duplicated in a separate classifier. Public DTOs, filtering/visibility, ordering, SQL,
locale fallback, successful display formatting and skip behavior are unchanged.

Four Rust scenarios were added but **not run**: unsupported type omission, missing scalar
omission, false/zero preservation, and localized/base/blank-text precedence. These are pure
helper scenarios, not mounted DB/GraphQL/native evidence or captured tracing assertions.
Six scoped standalone source guards passed (storefront attributes, media/price, storefront
boundary, facet chain, Product error boundary, public diagnostics). The existing attribute
guard now checks shared bounded warning use and rejects raw identity/type/error fields.
One Rust file formatted/idempotent with Scalar rust-fmt 0.3.0/Rust 2024 parsing; JS syntax and
`git diff --check` passed. No tests, Cargo check, CI or runtime execution. Actual log capture,
DB projection behavior, owner review and integration remain open; other Product diagnostics
and broader stored-value integrity are not certified by this slice. No dependency, write,
receipt, event or schema change is made.


## Storefront HTTP read diagnostics continuation (2026-10-18)

Status: **source-implemented / execution unvalidated / unintegrated**.
Main snapshot `6716323bc5630dd87939988a4507eaf0fea53827`; branch
`arena/8161797f-rustok`. No full-stack audit cursor or readiness promotion.

The four diagnostic events in `storefront_http_read_port.rs` now reuse Product's existing
context/error fact projections. Caller correlation and tenant values are represented by
character length only. Pagination refusals retain page-zero, page-size-zero and exceeds-limit
flags rather than exact rejected numbers; the static maximum remains 100. Owner failures add
the canonical closed owner variant, aggregate text/UUID shape and opaque-payload presence.
Existing public codes, including codes generated by the closed Core/Conflict policy, remain
safe diagnostic fields; no caller code/message or complete owner error is rendered.

This continuation does not change request admission, pagination bounds, checked offset
arithmetic/conversion, visibility-before-pagination, filters, translations, successful DTOs,
service calls, public messages/codes/retryability or existing severity. The compatibility HTTP
projection remains separate from other list contracts. No storage, receipt, event, dependency
or schema change is made, and no new error classifier is introduced.

Two Rust scenarios were added but **not run**: page bounds/error codes (including zero and
u64::MAX page size), and nested Core/Conflict/domain classification, retryability and safe copy.
Six standalone source guard entrypoints passed: Product error boundary, Commerce storefront
Product HTTP errors, Product public diagnostics, storefront boundary, admin facets and facet
chain. The error-boundary guard now checks bounded HTTP events. The Commerce HTTP guard now
checks the already-existing `usize::try_from(offset)` and `.skip(offset_usize)` rather than
requiring a retired unchecked cast; pagination implementation was not changed in this slice.
One Rust file formatted/idempotent with Scalar rust-fmt 0.3.0/Rust 2024 parsing; two affected
JavaScript guards passed syntax checks and `git diff --check` passed. None of these checks is
compilation, tracing capture or HTTP runtime evidence. Tests, Cargo, CI, mounted execution,
owner review/integration and wider Product/Commerce diagnostics remain open.


## Catalog command diagnostics continuation (2026-10-19)

Status: **source-implemented / execution unvalidated / unintegrated**.
Main inspected at `4905561ef0b9e81b3fb187efa9f54384442d036e`; work remains on
`arena/8161797f-rustok`. This does not advance readiness or the sequential audit cursor.

All seven events in `catalog_command_port.rs` reuse canonical Product context facts:
revision-required refusal, corrupt receipt replay, receipt failure, tenant parse rejection,
actor parse rejection, write-policy rejection and owner command failure. Correlation and
tenant strings become lengths, Product identity becomes a non-nil flag, and actor context
becomes kind/ID length. Receipt/admission port errors expose only closed kind, string
lengths and retryability, not raw code/message text. The owner mapper adds the existing
canonical Product error variant, aggregate text/UUID shape and opaque-payload flag; its
Product/Core policy-derived code remains a safe static field. No complete context, request,
receipt, idempotency key, response payload, owner error or nested cause is rendered.

This continuation changes diagnostics only. It preserves write policy/UUID parsing,
revision refusal, all 14 command request fingerprints, owner futures, pre-commit receipt
completion, replay/replay-error paths and failure finalization ordering. Public codes,
messages, kinds and retryability are unchanged. In particular, existing receipt mapping
still converts Timeout to Unavailable and NotFound to InvariantViolation; it does not
promise to forward arbitrary upstream retryability. The shared terminal-failure policy
and canonical nested Core/Flex classification are unchanged. No migration/schema/event
or transaction logic is changed.

Two Rust scenarios were added but **not run**: all seven receipt kinds with poisoned
code/message and both retryability values; and valid write scope plus missing key/deadline
and malformed tenant/actor UUID rejection. Existing owner-classification tests remain.
The error-boundary source guard now checks the seven-event fact inventory, actual event
fact consumption and absence of raw caller/port string fields. Six standalone source guards
pass (error boundary, catalog receipts, public diagnostics, atomic save, Variant patches,
Shipping admission), plus one Rust formatter/idempotence check, one JavaScript syntax
check and `git diff --check`. These are not compilation, log capture, DB replay/rollback,
mounted execution or CI evidence. Tests/Cargo, owner review/integration and other command,
schema-write and translation diagnostics remain open.
