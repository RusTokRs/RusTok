# Fly SSR-first browser runtime

Date: 2026-07-16
Status: Accepted

## Context

RusTok prioritizes classic server-side rendering. Public pages must render from Rust on the server and must not require WebAssembly, hydration, an iframe, or a client-side Leptos runtime. The visual editor still needs DOM geometry, pointer capture, keyboard events, `ResizeObserver`, and validated iframe messaging.

The earlier Fly adapter placed those browser mechanics in `fly-leptos` through `wasm-bindgen` and `web-sys`. That made the browser implementation appear mandatory even though project state, commands, validation, rendering, permissions, and persistence already live in Rust.

## Decision

1. `fly`, `fly-ui`, and the Page Builder render APIs remain platform-neutral Rust.
2. Storefront rendering is Rust SSR and never depends on WASM.
3. `fly-leptos` defaults to the `ssr` feature.
4. The existing Rust browser adapter is retained only behind the explicit `wasm-client` feature.
5. `fly-browser` distributes a standalone JavaScript adapter and transport-neutral intent envelope.
6. `rustok-page-builder-admin` defaults to `ssr + browser-js`.
7. `apps/admin` defaults to `ssr`; `csr` and `hydrate` are explicit profiles.
8. The JavaScript adapter may own only browser mechanics:
   - DOM listeners;
   - iframe source/origin/sequence checks;
   - geometry collection and overlays;
   - pointer and keyboard forwarding;
   - optional POST delivery to a consumer-owned SSR intent endpoint.
9. The JavaScript adapter may not own:
   - canonical project state;
   - Fly commands or history;
   - validation or publish policy;
   - permissions;
   - persistence;
   - HTML/CSS rendering rules.
10. Consumer modules own the SSR intent endpoint and draft lifecycle. The endpoint receives `BrowserIntentEnvelope`, loads the current consumer document, applies the intent through Fly, and persists through the existing consumer facade.

## Runtime profiles

### Storefront

```text
HTTP request
  -> Rust route
  -> load canonical project
  -> Fly runtime materialization
  -> Fly HTML/CSS/head renderer
  -> server response
```

No WASM or hydration is required.

### Admin, classic SSR

```text
HTTP request
  -> Rust SSR admin shell
  -> embedded fly-browser.js
  -> DOM/iframe events
  -> BrowserIntentEnvelope
  -> consumer-owned server endpoint
  -> Fly command/state transition
  -> redirect, fragment response, or refreshed SSR page
```

### Optional WASM client

The `fly-leptos/wasm-client` feature remains available for experiments such as offline editing or embedded standalone editors. It is not the default RusTok runtime and must not become a storefront dependency.

## Consequences

- Native SSR builds no longer compile the Fly `web-sys` browser runtime.
- The public site remains classic HTML-first.
- Page Builder browser mechanics can evolve independently of the Rust engine.
- Full no-WASM authoring requires consumer-owned SSR intent handlers and response replacement; this is an integration task, not a change to the Fly project contract.
- Source guards must fail if unconditional WASM dependencies return to the default Fly or Page Builder paths.
