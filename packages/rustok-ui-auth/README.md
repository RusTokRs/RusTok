# RusToK UI Auth

`@rustok/ui-auth` owns shared TypeScript authentication DTOs, error mapping,
and browser-session metadata helpers for RusToK frontend hosts.

## Boundary

- `@rustok/ui-auth/browser` is the browser adapter used by the Next storefront.
- Rust framework-neutral auth contracts live in `crates/ui/rustok-ui-auth`.
- Leptos signals, contexts, guards, and native server functions remain in
  `crates/ui/leptos-auth`.
- Server authentication, authorization, token issuance, and tenant authority
  remain owned by the Auth and RBAC modules.

Do not add React, Next.js, Leptos, Dioxus, GraphQL execution, or domain policy
to this package. A host may adapt these values to its framework lifecycle but
must not fork the auth/session vocabulary.

## Entry point

```ts
import { getClientAuth, type AuthSession } from "@rustok/ui-auth/browser";
```
