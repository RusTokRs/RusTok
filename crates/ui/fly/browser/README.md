# Fly Browser

For full public interface and protocol definitions, see [CRATE_API.md](CRATE_API.md).

SSR-first browser adapter assets and protocol contracts for Fly editors.

## Responsibilities

- Define the typed browser intent protocol.
- Classify revision-protected mutating intents.
- Provide the JavaScript adapter bundle and safe default limits.
- Bind the server-rendered editor controls (`bindSsrControls`): palette insert and drag, layer
  selection, toolbar intents, and route view state such as the previewed viewport. The
  attributes are a two-sided contract with the views that render them; see
  [the decision](../../../../DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md).
- Provide `FLY_ACTIONS_JS` for published actions. Target resolution and tab, accordion and slider
  group traversal are confined to the nearest landing body (or standalone document body), with
  renderer-owned hooks preferred to authored IDs. Tab panels may sit outside a nested tablist
  header; discovery stops at landing, explicit group, enclosing panel or competing-group
  boundaries, and a missing target leaves selection unchanged. Clicks and swipes cannot inherit a
  group from the host or change a nested landing. Accordion targets must belong to the trigger's
  nearest accordion and may not contain the trigger itself. Sliders prioritize the explicit active
  marker over visibility and reject malformed or ambiguous selectors. Swipe state is document-local
  and completes only for the initiating touch. Animation lookup accepts only own allow-listed names.
  Scroll actions use instant scrolling when the
  target document requests reduced motion, even if its CSS specifies smooth scrolling. It emits
  host events but does not implement Cart, Product or Pricing rules.

## Verification

- `node --test crates/ui/fly/browser/tests/action_runtime.test.mjs` exercises action dispatch with a minimal DOM boundary.
- `apps/next-frontend/tests/e2e/fly-actions.spec.ts` covers the shipped asset in Chromium, including actual scrolling and media emulation; it requires the Playwright dependencies and browser.
- `cargo test -p fly-browser` covers the Rust protocol and asset contracts.

## Interactions

Fly editor hosts embed the exported adapter and dispatch validated intents to their owner transport. This crate contains no domain persistence.

## Entry points

- `src/lib.rs` — protocol types, configuration, and embedded adapter assets.

See the [Page Builder documentation](../../modules/rustok-page-builder/docs/README.md) and the [module UI guide](../../../docs/UI/module-package-implementation.md).

## Request lifecycle and failure-state guarantees

Once stopped, an adapter cannot emit or post another intent, and a fetch that resolves after
unmount cannot publish a successful response into the disposed root. Intent POSTs use a verified
same-origin endpoint with `redirect: "error"`, rather than following any login or cross-origin
redirect while carrying a CSRF header. Network failures (including rejected redirects) remain
observable as browser errors. A successful non-JSON/empty response cannot clear an earlier
parked save failure: only an explicit clean document state can do that.

A read-only selection, hover, navigation or server preview no longer eclipses an earlier
in-flight editing response: authoritative commands have their own response generation.
Read requests started while a write is pending are not permitted to overwrite the later
write's revision/hash or clear its save warning. Concurrent *writes* still need a serialized
transport/request-identity contract; this is not an end-to-end ordering guarantee.

Authoritative writes from the same mounted browser adapter are now sent in sequence, with a
bounded queue under `maxPendingIntentRequests`; the next request takes its revision, hash and
draft generation from the preceding response while retaining the original clicked target.
Stopping the adapter resolves queued promises without sending them and suppresses late error
reports. This serializes one adapter instance only. With `BroadcastChannel` available,
another confirmed same-origin write for the same page marks the mounted editor stale,
emits `fly:browser-peer-write` and a visible `FLY_PEER_WRITE_CONFLICT` problem, rejects
subsequent mutations before DOM intent emission or POST and discards unsent queued writes with the existing
`fly:browser-intents-discarded` notice. The warning survives read-only responses
until remount/reload; the adapter never reloads automatically or silently merges
unsaved changes. The structured channel message carries only page ID, confirmed revision,
hash and editor-scoped sender ID. Both adapters require nonblank metadata, at most 256 UTF-8
bytes per field and 1024 UTF-8 bytes for the serialized message; malformed notifications are
ignored without advancing the baseline or fencing the editor.
Browsers without cross-tab messaging still rely on server-side CAS. This is a
conservative page-level guard (including editors on different locales), not
cross-tab ordering, server-recognized idempotency or recovery for an aborted
request: those remain unimplemented.

The serialized writer queue fails closed on HTTP errors, timeouts/aborts, redirected
responses, application-level save failures, navigation responses, or HTTP successes without a
confirmed nonempty revision and project hash. Pending mutations are resolved as unsent instead
of being replayed with an unverified base. An abort signal that expired while queued skips that
intent without posting. If a dequeued request fails admission before taking the in-flight guard,
the remaining queue is explicitly discarded and its promises settled; no request is left waiting
for a nonexistent transport completion. Synchronous response or network-status listeners may
unmount the adapter; the stopped continuation cannot rebase the root or initiate navigation.
This is intentionally a safety gate, not automatic retry or reconciliation.

Timeout notifications use the same authoritative/read-only request ordering rule as response
and error notifications. A newer selection request cannot hide the timeout of a pending write.

Expected iframe origins use the same explicit HTTP(S) origin grammar as Fly Web:
canonical host and default port normalization are supported, while paths (including
dot segments), credentials, malformed authority delimiters, queries and fragments
are rejected before URL parsing. The literal sandboxed origin `"null"` remains valid.

Same-origin navigation/intent-endpoint admission rejects raw backslashes and ASCII control
or whitespace characters before URL resolution, including the `about:blank`/srcdoc fallback
that previously treated ambiguous root-relative paths as safe.

When an unconfirmed write causes the adapter to abandon queued mutations, it emits a
`fly:browser-intents-discarded` event with the unsent intent names and count, exposes
`data-fly-queued-intents-discarded`, and adds an accessible status message without hiding the
original revision/transport error. The queue is never quietly treated as saved. The marker is
cleared by a confirmed later write or adapter stop.

## Strict SSR operation recovery

When the host advertises the rendered draft contract, the adapter persists each
mutating envelope and its operation ID in tab-scoped `sessionStorage` before
sending it. A reload or ambiguous response triggers a read-only receipt lookup.
The adapter blocks new mutations while the outcome is unknown and exposes
explicit status-check and same-request retry controls; it never automatically
retries the mutation. A restored pending envelope immediately shows its recovery
notice and starts a receipt lookup. Lookups and retries of the same operation are
mutually exclusive. A tab/page pending slot cannot be overwritten by a second mounted root, and clearing it requires the exact original envelope. A committed receipt must match the operation ID and draft token, advance the original generation, and include a usable editor snapshot. Unconfirmed success is not returned as a usable result.
Selection and hover cannot supersede recovery. The editable Pages server also commits
draft generation for view-state intents, so strict transport serializes and receipts
all such requests, not only document commands. A receipt lookup is the only pure
lookup and cannot advance the draft base until a matching committed receipt arrives.
Both DOM dispatch and POST apply this strict classification, so a pending operation
or peer conflict blocks view-state intents as well as document edits. A successful
HTTP response without a matching committed receipt is non-current: it cannot clear
a parked save warning, update revision/hash/draft metadata, or navigate the editor.
Recovery refuses a different draft token, and the parked conflict retains the expected
revision across reloads. Confirmed clean writes notify peers
even when the SSR response requires reload; dirty draft-only changes do not.
HTTP revision-conflict responses fence the editor, and late responses cannot replace
a peer-write warning. The host must render an owner-bound draft token before
enabling edits and bind that token into the route. This protocol is currently
implemented by the Pages SSR host; other hosts must not opt into the strict
contract without matching server-side atomic receipts.
