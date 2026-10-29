# fly-browser / CRATE_API

## Public surface

### Protocol Constants & Assets
- `FLY_BROWSER_PROTOCOL`: `"fly_iframe"`.
- `FLY_BROWSER_ADAPTER`: `"fly_browser"`.
- `FLY_CSRF_HEADER_NAME`: `"x-csrf-token"`.
- `FLY_BROWSER_ADAPTER_JS`: embedded JavaScript adapter runtime.
- `FLY_ACTIONS_JS`: standalone action runtime for published pages.

### Types & Protocol Contracts
- `BrowserIntentKind`: typed browser action kind enum (`Select`, `Hover`, `ActivatePage`, `Drop`, `InsertBlock`, `RemoveSelected`, `MoveSelected`, `PatchSelected`, `Undo`, `Redo`, `Save`, `Publish`, etc.).
- `BrowserIntent`: strongly typed intent envelope carrying tenant context, project revision, target component ID, and payload.
- `BrowserConfig`: client-side adapter configuration (timeouts, resource bounds, CSRF tokens, protocol settings).
- `BrowserEnvelope`: framed transport wrapper.

## Invariants

- **Safe Rust**: The entire crate strictly forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Zero Framework Coupling**: No DOM, WebSys, Leptos, or WASM bindings inside Rust; SSR-first architecture with embedded JS distribution.
- **CSRF & Revision Protection**: Mutating intents require valid CSRF headers and expected revision tracking to guard against stale updates and lost-update races.
