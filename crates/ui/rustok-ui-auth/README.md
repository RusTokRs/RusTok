# RusToK UI Auth

For full public interface and contract definitions, see [CRATE_API.md](CRATE_API.md).

`rustok-ui-auth` owns framework-neutral client authentication values and expiry
policy shared by RusToK UI adapters.

## Responsibilities

- `AuthUser`, `AuthSession`, and `AuthError` client contracts;
- redacted debug output for session secrets;
- host-supplied expiry evaluation;
- status-to-client-error mapping.

## Boundary

Server authentication, authorization, token issuance, refresh persistence, and
tenant authority remain owned by Auth and RBAC. Leptos contexts, route guards,
signals, browser storage, and native server functions remain in `leptos-auth`.
Other framework adapters consume this crate directly and add only their own
lifecycle bindings.

Do not add Leptos, Dioxus, React, router, cookie, GraphQL, or server-runtime
dependencies to this crate.

## Entry point

- `src/lib.rs` — framework-neutral auth contracts and expiry policy.
