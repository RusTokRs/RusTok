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
  group from the host or change a nested landing. Scroll actions use instant scrolling when the
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
reports. This serializes one adapter instance only; cross-tab conflicts and unconfirmed transport
outcomes remain under the server's revision/failure handling and require host-level evidence.
