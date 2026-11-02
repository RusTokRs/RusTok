# Product axis reconfiguration and SKU recovery

Status (2026-10-29): source implementation, **not runtime-certified**. This is ordinary feature
work, not advancement of the Product reference-hardening or audit cursor. The governing decision
is [unified variant axes, §10a–10b](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10a-existing-axis-preview-and-application-contract).

## Owner and transport contract

`ProductCatalogCommandPort::preview_variant_axes` is a read-policy planning capability of the
**selected** Product command provider. The embedded implementation is
`CatalogService::preview_variant_axes`; neither Commerce nor native admin constructs a different
provider on failure. An external provider must implement this capability or report unavailable.
The preview rolls back its read transaction and creates no receipt, catalog rows or events.
Both admin ingress paths require authenticated tenant/actor scope, enabled Product and
`products:update` permission even though planning is read-only.

The existing `set_variant_axes` command is the sole application path. `SetVariantAxesInput`
contains `axes`, optional `expected_state`, and optional `reassignments`. Each reassignment is an
existing Variant UUID with its **full target tuple**, not a partial patch. Unmentioned SKUs use
their active EAV assignments for the proposed axes. Unexpected assignments in an explicit tuple
are conflicts, not fields that the owner silently prunes.

Preview returns the Product UUID/revision, a proposal-specific SHA-256 `state_token`, complete
bounded SKU rows with current/proposed tuples, `can_apply`, `has_changes`, and bounded conflicts.
The fingerprint binds tenant/Product, document revision, existing axis/allowed-value row IDs and
ordering, complete Variant membership/SKU/combination identity, relevant EAV row/link/detachment
state, and the exact ordered proposal. `expected_state` is excluded from its own fingerprint.
Independent Inventory/Pricing state is not a tuple predecessor. Schema eligibility is revalidated
under the schema fence rather than accepted from the client or inferred from this token.

Apply recomputes the plan under schema → Product locks and compares the expected token before
mutation. It retains Variant UUIDs and retained configuration row UUIDs. Unique combination
identities are staged before tuple swaps; configuration positions use bounded, disjoint,
**nonnegative** temporary holes so immediate unique indexes and historical CHECK constraints
remain satisfied, including authored `i32::MAX` positions. Unchanged position rows are not
rewritten/staged, and tuple-only changes do not rewrite configuration or unaffected Variant
identities; this avoids unnecessary deferred-trigger work. Final invariants hold at commit.
Removed-axis EAV is retained; the operation does not delete/consolidate SKUs, stock, prices or
cross-owner links. Only explicitly reassigned target values are overwritten.

Changed configuration/assignments advance the Product revision, index invalidation and
`ProductUpdated` event atomically with the existing durable receipt. No-ops still complete their
receipt without an extra revision/event. The response remains `Vec<VariantAxisConfigResponse>`:
it is **not** a new Product/revision acknowledgement. Completed old receipts replay before the
new predecessor check; absent/empty new fields retain historical request serialization. An old
uncompleted request without a token is refused, not upgraded/rebased.

## Bounds and actionable conflicts

- At most 256 existing SKUs, 32 axes and 256 allowed values per axis. Over-limit snapshots or
  proposals refuse; no first-N plan or Cartesian generation is used.
- At most 16 conflicts, each with up to eight Variant UUID examples and explicit more flags.
  Missing, duplicate and unexpected assignments, disallowed options, duplicate combinations and
  multiple no-axis defaults require explicit correction/reassignment.
- Stable application codes: `product.variant_axes_preview_required`,
  `product.variant_axes_stale`, `product.variant_axes_conflict`,
  `product.variant_axes_limit_exceeded`. Commerce exposes their uppercase GraphQL equivalents.
- Auth, tenant/context admission, idempotency-key conflicts, unavailable providers, transport
  failures and unknown commit results are **not** proof of a terminal business refusal.

## Next and Leptos operator workflow

Both clients start existing-axis drafts from **saved selections**, never new schema defaults.
Metadata refresh updates labels/choices without reordering, pruning or reseeding the proposal.
Unavailable selected IDs remain visible. Dictionaries above 256 entries expose only retained
selections with a localized limitation notice; new-axis authoring from such dictionaries is
unavailable until bounded dictionary paging exists. There is no arbitrary first-256 selector.
SKU review is paged in blocks of 12 over the complete bounded preview. Tuple/axis edits invalidate
approval; applying requires a matching preview and explicit confirmation. Unsaved category
changes block existing-axis writes.

Before POST, recovery persists and reads back an immutable scope/locale/proposal/token/caller-key
snapshot. Next uses its existing scoped browser journal/Web Lock and URL editor intent; Leptos
uses scoped sessionStorage, a stable `axis_recovery` document marker and generation/predecessor
checks. Unknown outcomes retry that exact snapshot and key. Pending payloads are visible without
credentials. Missing/corrupt storage, changed scope/marker or failed persistence block new writes.
An arbitrary 4xx, auth failure or unrecognized rejection cannot settle a Pending attempt.

Leptos selects native for SSR/hydrate and GraphQL otherwise **before** I/O; axes never fall back
between providers/transports after failure. Its native endpoints use JSON input, avoiding URL
encoding expansion for nested bounded tuples. Next forwards frozen locale through its fifth
GraphQL-executor options argument; custom executors must honor an explicit `locale` override.
The missing-route-locale default is the platform `en`, not a UI en/ru normalization of an explicit
locale.

Acknowledged projection refresh merges axes/variants only, never dirty document controls or a
new document revision. A confirmed axes attempt does not authorize reusing the old Product draft:
operators explicitly open a fresh editor document. Leptos also permits explicit correction after
a durable rejection; this retains the original document revision, still subject to owner CAS. Leptos recovery controls remain outside the
locked document fieldset and are usable even when Product loading fails. This durable recovery
contract also covers standalone SKU creation below; other Leptos retained-key lifecycle helpers
remain memory-only and are not certified durable snapshot recovery.

## Standalone SKU creation and recovery

Standalone creation still invokes `ProductCatalogCommandPort::create_variant`. Its deployed
caller-key framing is **`create_product_variant`**, distinct from the internal port operation;
no receipt schema, response framing or database migration changes. Native and GraphQL select
the same command runtime before IO and never switch provider after an ambiguous result.

Fresh admission prepares one complete input: nullable SKU/barcode/shipping profile, full explicit
axis assignments, every supplied price/compare-at, and explicit inventory quantity/policy.
Prices use exact nonnegative decimal strings; blank price means absent, not a zero-priced row.
The client bounds are 100 characters per optional text field, 32 distinct axis assignments,
64 prices, canonical decimal strings up to 64 characters, nonnegative `i32` stock, and
`deny`/`continue` policy. Fresh creation also refuses capacity above 256 saved SKUs or another
no-axis default. The owner remains responsible for current schema/tuple/uniqueness admission.

Both clients persist and read back the full request, original locale, caller key and authenticated
tenant/actor/Product identity **before** owner IO. Unknown outcomes freeze all these fields;
replay does not normalize the request again or depend on a successful Product/effective-form
GET. A tuple appearing in a later GET is not commit evidence. Auth/context admission, missing
providers, receipt input mismatches, arbitrary 4xx and unrecognized/retryable refusals stay
unknown. Settlement requires the owner acknowledgement or a closed, explicit durable refusal.

### Leptos

`core/write_journal.rs` and `transport/write_journal.rs` are the single owner-local state/storage
mechanism shared with axes. The existing axes key/marker and serialized envelope remain intact;
a regression source pins the pre-extraction pending JSON. Standalone creation specializes it
in `core/variant_create.rs` with `Ready -> Pending -> Confirmed|Rejected`. Full input remains in
both terminal variants. Its scoped key prefix is `rustok.product.variant_create_attempt` and
its document marker is `variant_recovery`; the envelope bound remains 2 MiB.

`ui/variant_create.rs::ProductVariantCreateSection` is mounted outside the document fieldset
alongside the axes controller in both the routed editor and its reference composition. It owns
creation; `ProductVariantsPanel` only edits existing rows. Both reference entry paths delegate
a selected `product_id` to this same keyed editor, not the old projection-dependent saved form.
Changing metadata does not remount recovery, and a missing Product GET cannot remove it. Current authentication is checked
before browser initialization and submission; stale bootstrap/owner responses cannot publish
into a disposed or changed recovery context. Native JSON ingress authenticates the expected
scope, invokes the selected port and validates a nonnil Variant UUID plus matching Product ID.
GraphQL uses its existing `{ id }` response and structured rejection metadata; no nonexistent
Product-ID field was added to that schema.

The root write fence verifies both persisted journals at fresh admission. Pending and terminal
requests fence other Product writes but are not reported as corrupt storage merely for being
unresolved. Missing/changed storage does refuse IO. Explicit terminal correction retains the
complete saved draft, including unavailable assignments/currencies. A confirmed attempt needs
an explicit fresh document: its Variant UUID does not supply a new Product revision or authorize
rebasing dirty document fields. Other lifecycle writers were not cut over by this work.

### Next

The existing Product editor journal adds operation `variant`, not a parallel recovery mechanism.
The request is `{ scope, id, locale, input }`; success is
`{ id: ProductId, revision: null, variantId: VariantUuid }`. Only this operation accepts
`variantId`. Its terminal record retains the exact request and caller key for inspection and
explicit correction. `addVariantAction(request, key)` authenticates scope, validates without
rebuilding the saved input and forwards the frozen locale through the SDK executor options.
Postcommit cache invalidation failure cannot turn owner success into a new creation attempt.

The normal detail route and missing/unavailable-Product recovery-only route mount the same
action. The editor can inspect/resume without Product GET; fresh creation still needs valid
current metadata and explicit operator admission. Terminal correction restores shipping,
compare-at, stock, tuple and retained currencies/IDs rather than silently clearing them.

The existing browser journal now persists/readbacks an explicit empty baseline before attaching
its URL intent. Marked-but-missing/corrupt/foreign storage cannot initialize another request.
Rotation requires a persisted terminal record, not a lost response. Writes use predecessor CAS:
only empty -> pending or exact pending -> terminal is allowed. A late acknowledgement cannot
overwrite a missing/changed slot, and a same-key terminal record with another full request is
not forward restoration. A Web Lock also rechecks the URL intent after acquisition. Unknown
and terminal records are not silently deleted during explicit new-document navigation.

## Verification and release gates

No tests were executed (maintainer request); Cargo/rustc and PostgreSQL CLIs are unavailable.
Rust formatting/parsing, strict TS core checking, JavaScript syntax/module linking and Fluent
syntax/type generation are not compilation, database or mounted-client evidence.

Added source scenarios cover planner collisions/swaps/stale fingerprints/bounds, journal
transitions/full tuples/locale replay/closed refusals, and an ignored real-PostgreSQL contract in
`apps/server/tests/product_variant_axis_policy_postgres.rs`. That contract includes position and
tuple swaps, stable configuration/EAV/Inventory links, read-only preview, exact Vec receipt
replay, stale refusal replay, explicit collision resolution on axis removal and no-op
revision/event preservation. These assertions
have **not run**. Standalone source scenarios additionally cover immutable body/key/locale,
closed settlement, missing/changed storage, terminal correction/CAS and actual SDK/action
bindings. The PostgreSQL fixture includes full stored-response replay after a later stock
update, stable Variant/value/Inventory links, and mismatched body/locale refusal. These are
unexecuted assertions, not evidence for every Pricing/Channel/Index/Cart/Order/Media/Fulfillment
consumer or authoritative Inventory quantity/reservation behavior.

Before rollout: compile owner, Commerce, shared GraphQL and admin SSR/hydrate/GraphQL profiles;
run focused suites and the disposable PostgreSQL contract; exercise lost-response/storage/
History/scope transitions in both mounted clients; measure maximum-bound transaction duration,
lock contention and host/proxy body limits; verify external selected providers and cross-owner
links/projection replay. Full UI typing currently remains blocked by missing dependency/types
and errors outside the checked Product surfaces. No storefront selector cutover, attribute/option
retirement or destructive legacy cleanup is part of this slice; no historical migration changed.
