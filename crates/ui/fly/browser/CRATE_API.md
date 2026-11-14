# Crate: `fly-browser`

SSR-first JavaScript browser adapter assets, event bridging, CSRF security tokens, and protocol contracts for Fly visual editors.

## 1. Public API Surface

### Protocol Constants & Assets
- `FLY_BROWSER_PROTOCOL`: Protocol identifier string (`"fly_iframe"`).
- `FLY_BROWSER_ADAPTER`: Adapter identifier string (`"fly_browser"`).
- `FLY_CSRF_HEADER_NAME`: Standardized CSRF request header (`"x-csrf-token"`).
- `FLY_BROWSER_ADAPTER_JS`: Embedded JavaScript adapter runtime for iframe sandbox isolation.
- `FLY_ACTIONS_JS`: Standalone interactive action runtime for published pages (accordion toggles, tab switching, animations).

### Types & Protocol Contracts
- `BrowserIntentKind`: Typed browser action kind enum (`Select`, `Hover`, `ActivatePage`, `Drop`, `InsertBlock`, `RemoveSelected`, `MoveSelected`, `PatchSelected`, `Undo`, `Redo`, `Save`, `Publish`, etc.).
- `BrowserIntent`: Strongly typed intent envelope carrying tenant context, project revision, target component ID, and payload.
- `BrowserConfig`: Client-side adapter configuration (timeouts, resource bounds, CSRF tokens, protocol settings).
- `BrowserEnvelope`: Framed transport wrapper ensuring typed deserialization and forward compatibility.

## 2. Invariants & Business Rules
- **Safe Rust**: The entire crate strictly forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Zero Framework Coupling**: No DOM, WebSys, Leptos, or WASM bindings inside Rust; SSR-first architecture with embedded JS distribution.
- **Save Failure Reporting**: A successful intent response may carry `save_failure` (`status`, `code`, `error`, `intent`) with `reload: true`. The adapter reports it as a browser problem (fallback code `SAVE_FAILED`) and keeps it in `sessionStorage` across the reload. It stands until a response reports a saved working copy: a response without `save_failure` whose state is still `dirty: true` keeps the message (root attribute `data-fly-save-failed="true"`), a response with `dirty` other than `true` clears it.
- **CSRF & Revision Protection**: Mutating intents require valid CSRF headers and expected revision tracking to guard against stale updates and lost-update races.
- **Protocol Parity**: Cross-protocol requests and invalid envelopes are rejected fail-closed.
- **SSR Control Contract**: `bindSsrControls` in `assets/fly-browser.js` binds the `data-fly-*` attributes that the default admin views render (`data-fly-block-id`, `data-fly-can-drag`, `data-fly-component-id`, `data-fly-route-query`, and the `data-fly-action` values `insert-block`, `begin-block-drag`, `select-component`, `set-viewport`, `intent:<name>`). The set is derived from both sides by `scripts/verify/verify-fly-ssr-control-contract.mjs`; a binding without a producer, or a rendered action without a handler, fails it. See `DECISIONS/2026-11-11-fly-ssr-control-contract-and-route-view-state.md`.
- **Route View State**: A `<select data-fly-action data-fly-route-query="<key>">` changes no project state. On `change` the adapter sets `<key>` in the current URL, keeps the other parameters, and navigates; it posts no intent.
- **Published action scope**: `FLY_ACTIONS_JS` resolves targets, groups and group members inside the nearest `[data-fly-page-body]`/`.fly-page-body`, falling back to `body` for a standalone document. A click or swipe cannot inherit a host group or change a nested landing; canceled or reparented swipes do not act. Renderer-owned component hooks win over authored HTML IDs. Custom actions prevent accidental native navigation, submission or reset even when the action and control are nested; native navigation and form actions are not replaced. Scroll actions honor the target document's reduced-motion preference with instant scrolling, including when CSS requests smooth scrolling. The asset dispatches `emit_event`/`provider_action` to the host but owns no commerce mutations.

## 3. Error Handling & Edge Cases
- Unknown intent kinds preserve structured extension payloads without failing entire batch envelopes.
- Timeout classification cleanly differentiates aborted, network-hung, and canceled user actions.
- Graceful degradation when JavaScript action runtime is executed on published vs preview documents.

## 4. Security & Isolation Invariants
- Embedded JS adapter runs within sandboxed iframe boundaries without top-level origin contamination.
- Cryptographic CSRF token derivation using SHA-256 prevents cross-site request forgery in visual editor commands.
- Mutating intent classification strictly distinguishes read-only vs write actions.

## 5. Contract Tests & Verification
- Unit and integration contract tests cover CSRF derivation, protocol rejection, action runtime assets, auto-mount, abort classification, lifecycle, and order preservation.
- `src/contract_tests.rs`: Direct verification of package metadata, CSRF validation, and asset bundle integrity.
- `apps/next-frontend/tests/e2e/fly-actions.spec.ts`: Chromium exercises the shipped asset's behavior, including nested landing/host groups and form activation boundaries.
