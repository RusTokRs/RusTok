# Pages / Page Builder Inline Edit Admin Launch Packet

Date: 2026-08-06  
Status: `source-ready / admin-asset-build-and-browser-execution-pending`

## Scope

This packet adds the admin-owned entry point for the authenticated Pages inline authoring route. It does not change the route, grant, Fly command, Pages persistence, asset-delivery or public storefront owners.

The source slice adds:

- `rustok-pages-admin/inline-edit-launch` as a non-default feature;
- `rustok-admin/pages-inline-edit-launch` as the app-level pass-through feature;
- a Pages admin launch control when an exact unpublished page is selected;
- an explicit compile-time same-origin deployment acknowledgement;
- bounded, encoded route construction without credentials or proof material;
- unvalidated evidence and a static source guard.

## Same-origin session boundary

The authoring route requires a direct authenticated browser session. A standalone admin can be configured to call a remote API with a bearer token, but navigating a browser to that remote authoring route must not move the bearer token into a URL, DOM attribute or referrer.

The launch control therefore uses only the fixed relative route:

```text
/modules/pages-authoring?page_id=<pages UUID>&lang=<encoded exact document locale>
```

It is rendered only when both conditions are true:

```text
Cargo feature: rustok-pages-admin/inline-edit-launch
compile-time flag: RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true
```

The flag defaults to absent/disabled. Enabling only the Cargo feature is insufficient. The future embedded-admin build integration must set the flag deliberately after composing the admin and authoring route on the same origin.

No arbitrary external origin, API base URL or redirect target is accepted by this component.

## Exact draft identity

The component reuses the existing Pages admin transport to reload the selected page before exposing a link. It does not trust the admin UI locale as document identity.

The launch identity is derived from:

- the canonical non-nil UUID returned by the selected Pages detail;
- translation locale when present;
- otherwise the current body locale.

A page whose status is `published` does not receive a launch control. Missing page, missing exact locale, invalid identity or transport failure also render no link. The authoring route independently repeats unpublished-state, exact-locale, tenant-feature and owner-aware permission checks.

This read is a presentation prerequisite only; it does not create another mutation or persistence path.

## Visibility and authorization

The control is hidden unless:

- the compile-time same-origin acknowledgement is exactly `true` ignoring case and surrounding whitespace;
- the selected page identity parses as a non-nil UUID;
- the existing Pages admin transport reloads an unpublished page;
- the selected translation/body provides the exact document locale;
- the current canonical admin role has effective Pages edit capability;
- the locale is non-empty, bounded to 64 bytes and contains no control characters.

This role check is presentation-only. The authenticated authoring middleware still requires a direct user, non-nil session and effective `pages:update`. Pages still rechecks owner-aware permission, tenant feature enablement, unpublished state, exact locale, revision and Fly project identity.

A visible link does not grant access and does not create a second authorization path.

## URL and DOM contract

Query construction uses `url::form_urlencoded::Serializer`.

The href contains only:

- canonical hyphenated Pages UUID;
- percent/form-encoded exact document locale.

It never contains:

- bearer token;
- session identifier;
- inline-edit grant;
- authorization proof;
- signing key;
- tenant secret;
- caller-provided origin.

The link opens in a new tab with:

```text
target="_blank"
rel="noopener noreferrer"
```

The launch section exposes only `data-pages-inline-edit-launch="same-origin"`; it does not write page identity, locale, token or proof into a custom data attribute.

## Feature graph

```text
rustok-pages-admin/inline-edit-launch
└── optional url + uuid dependencies

rustok-admin/pages-inline-edit-launch
└── rustok-pages-admin/inline-edit-launch
```

Neither feature is part of the retained default, CSR, hydrate or SSR feature definitions. Deployment/build tooling must opt in explicitly.

## Deliberate limits

This slice does not:

- update `apps/admin` Leptos/Trunk metadata to build the feature;
- set `RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true` in any build script;
- integrate the launch feature into release workflows or Docker builders;
- enable the server asset profile by default;
- change the anonymous/public Pages route;
- add credentials to navigation;
- change database, GraphQL, REST, event, publish or rollback schemas;
- change `PageService::save_document` ownership;
- claim an admin WASM artifact, rendered control, navigation, HTTP response or browser edit;
- promote FFA or FBA.

## Source evidence

- `apps/admin/Cargo.toml`
- `crates/rustok-pages/admin/Cargo.toml`
- `crates/rustok-pages/admin/src/lib.rs`
- `crates/rustok-pages/admin/src/inline_edit_launch.rs`
- `crates/rustok-pages/contracts/evidence/pages-inline-edit-admin-launch-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-inline-edit-admin-launch.mjs`

## Next cursor

1. update the deterministic embedded-admin build to enable `pages-inline-edit-launch` and set the exact same-origin flag;
2. compose that admin build with `pages-inline-edit-assets` in release and production Docker builders;
3. update protected release-infrastructure guards and reproducibility contracts;
4. retain generated admin JS/WASM and server binary hashes/sizes;
5. observe role-visible/hidden, published-hidden and exact-locale launch states plus same-origin navigation;
6. execute authenticated edit/save, stale revision, replay and expiry browser cases.

## Validation status

No tests, static verifiers, formatting, Cargo checks, admin WASM builds, embedded-admin builds, server builds, HTTP requests, browser navigation, workflows or CI were run by the implementation agent.

Suggested commands, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-admin-launch.mjs
cargo test -p rustok-pages-admin --features inline-edit-launch --all-targets -- --nocapture
cargo check -p rustok-admin --no-default-features \
  --features hydrate,pages-inline-edit-launch \
  --target wasm32-unknown-unknown
RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true \
  cargo check -p rustok-admin --no-default-features \
  --features hydrate,pages-inline-edit-launch \
  --target wasm32-unknown-unknown
```

Execution evidence remains pending.
