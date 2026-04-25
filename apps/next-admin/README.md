# RusToK Next Admin

## Purpose

`apps/next-admin` owns the Next.js-based admin application for RusToK.

## Responsibilities

- Provide the React/Next admin host for teams working in the Next ecosystem.
- Mount module-owned Next admin packages from `packages/*`.
- Keep module UX out of core navigation: each module registers its own Next admin entrypoint from `src/features/<module>` or a mounted `@rustok/*-admin` package, and the shell filters those entries by enabled module slug.
- Stay aligned with the Leptos admin contract without becoming the primary auto-deploy admin stack.
- Keep URL-owned typed route-selection parity with `apps/admin`.

## Entry points

- `src/app/*`
- `src/shared/*`
- `packages/*`
- Next.js App Router entrypoints and layouts

## Local Debug

Run the local debug server against `apps/server` on `http://localhost:5150`:

```powershell
npm.cmd run dev -- --hostname localhost --port 3000 --webpack
```

Use `localhost`, not `127.0.0.1`, in this Windows debug environment. The local loopback path through `127.0.0.1` can accept TCP connections while HTTP responses never reach the client; `localhost` resolves to the working IPv6 loopback.

`--webpack` is intentional for local debug because Next.js 16 Turbopack currently hangs while compiling `/auth/sign-in` in this workspace. This does not change the public backend contract: `NEXT_PUBLIC_API_URL=http://localhost:5150`, GraphQL remains `/api/graphql`, and auth remains `/api/auth`.

## Interactions

- Uses `apps/server` as the backend/API provider.
- Works in parallel with `apps/admin` for UI parity and contract validation.
- Mounts package-owned module UI such as `@rustok/*-admin` instead of owning module business UI inline.
- Core shell routes are limited to platform host surfaces. Product, blog, workflow, search, AI and similar module/capability UI must be registered by their module package, so a tenant that only enables `blog` does not see ecommerce-only navigation.
- Implements the same typed snake_case route-selection contract as the Leptos admin host, but through local Next helpers instead of shared Rust code.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
