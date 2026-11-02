# Commerce admin Fulfillment owner-command cutover — 2026-08-09

Status: source-complete for mounted admin Fulfillment ship/deliver/reopen/reship/cancel routes; execution evidence pending and unvalidated. The 2026-11-01 shipment continuation below supersedes the original reconstructed-key and replay assumptions.

## Scope

The canonical mounted lifecycle routes call a Fulfillment-owned command capability:

- `POST /admin/fulfillments/{id}/ship`;
- `POST /admin/fulfillments/{id}/deliver`;
- `POST /admin/fulfillments/{id}/reopen`;
- `POST /admin/fulfillments/{id}/reship`;
- `POST /admin/fulfillments/{id}/cancel`.

Manual fulfillment creation is a cross-owner Commerce workflow. Its separate [create cutover](admin-fulfillment-create-owner-port-cutover-2026-08-09.md) also uses typed owner capabilities. It does not move Order, seller or shipping-profile policy into F.

`controllers/admin/mod.rs` mounts only `fulfillments_owner_commands.rs` as `fulfillments`. That module owns list/show/create as well as the five lifecycle handlers. There is no old `fulfillments.rs` controller or wildcard compatibility re-export to retain. Public router/OpenAPI symbol names remain unchanged.

## Fulfillment owner capability

`FulfillmentAdminCommandPort` and `FulfillmentAdminCommandRuntime` own service construction, lifecycle/counters, selected provider resolution, journal execution/checkpoint and local recovery. Mounted Commerce handlers construct neither `FulfillmentService` nor the old orchestration service.

C composes selected saved Order facts with Inventory's native shipment implementation before carrier/F write transaction. F consumes a prepare/apply port without depending on O/I. Locked F snapshot correlation, native physical movement/proof, progress and provider checkpoint use one local transaction. The carrier and saved O reads are not part of that transaction. See the [shipment handoff](implementation-plan.md#shipment-stock-provenance-source-evidence).

## Literal intent and replay identity

Every write requires a caller-owned `Idempotency-Key` header of 1..=191 UTF-8 bytes. Distinct later intents require distinct keys; uncertain retries retain key and payload. The admitted key is forwarded literally to the owner journal/provider. Neither the controller nor F reconstructs an intent from a fulfillment ID or current payload hash.

Historical lifecycle keys used `fulfillment:{fulfillment_id}:{operation}:{fnv128}`. Their exact retained journal value, not a new calculation, remains the only recovery identity. Private `commerce_orchestration` descriptors retain immutable operation/carrier/tracking/item or reason facts; they are not stock history or caller metadata.

Committed provider replay does not reinvoke the carrier or reacquire stock. Deliver/reopen kernel commands bind key, payload and exact response through their owner-local receipt transaction; plain domain methods do not provide that kernel receipt contract. Cancelled status or provider metadata alone does not prove restored stock.

## Canonical request context

HTTP uses `auth.port_actor()` to preserve human versus OAuth service identity, forwards permissions/scopes, locale/channel and actual request correlation, and uses a two-second call budget. AuthContext has no roles field. Native callers retain their own roles, causation and tracing rather than accepting a synthetic reconstructed context.

The generic key is not outcome evidence or authorization. Actual kernel/journal admission and immutable correlation remain the owner contract. A deadline phase check is not a SQL statement timeout.

## Provider outcome and reconciliation

Fresh claims require Pending with absent result/reference/completion/commit markers and no unresolved sibling effect. Legacy `provider_error`, unknown states and malformed Pending/Committed records fence a new identity; the same owner predicate also fences local progress. A private release is permitted only before carrier invocation. Explicit operator confirmation of no effect may requeue the original key, but cannot erase retained reference/result/commit evidence. Neither action infers safety from a legacy error label.

Fulfillment-owned local recovery validates current context and retains the saved literal key. It applies only local progress/stock/checkpoint without calling a carrier. Selected recovery takes precedence; unsupported selected capability never falls back to native data. Other operator list/resolve/create-label remote composition remains a separate topology gate.

Known or ambiguous carrier outcome with failed local persistence requires reconciliation, not automatic key rotation or compensation. The public HTTP 409 family remains `commerce_admin_fulfillment_reconciliation_required`; bounded owner advice is preserved. Storage, validation, state conflict and not-found families retain static messages, not raw provider/database text.

## Runtime composition and remaining work

`CommerceHttpRuntime` prefers the selected F command runtime. Native composition receives the actual selected Order reader and deployment-selected provider registry. A missing stock configuration fails closed; it is never replaced by Noop. Only explicit Noop is a deployment/test opt-out, including unknown header-only fresh Ship. Pending cancel, delivery and undo-delivery need no stock dependency.

The canonical broad Commerce topology P0 remains open. Both new additive migrations must precede the new binary: I's native shipment proof and F's provider-effect admission correction. Applied migrations and historical rows are not rewritten. Full event/outbox/Index, mounted/remote, unpaid-return and deployment convergence remain open.

## Validation status

The current source guards may execute read-only source checks. Syntax parsing and WASM formatting are not Rust typechecking or behavioral proof. No tests, Rust compilation, SQL/carrier execution, runtime HTTP, restart/lost-response, CI/workflow or remote-adapter validation is claimed. Tests remain with the maintainer; see the [source evidence](../contracts/evidence/shipment-stock-provenance-source.json).
