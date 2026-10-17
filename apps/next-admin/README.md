# RusToK Next Admin

## Purpose

`apps/next-admin` owns the Next.js-based admin application for RusToK.

## Responsibilities

- Provide the React/Next admin host for teams working in the Next ecosystem.
- Mount module-owned Next admin packages from `packages/*`.
- Keep module UX out of core navigation: each module registers its own Next admin entrypoint from `packages/*`/`@rustok/*-admin`, and the shell filters those entries by enabled module slug.
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
- Package-owned module UI lives under `packages/*`; app routes and navigation registration import package entrypoints, not host-local module feature folders.
- Starter-only dashboard routes that are not part of the RusTok admin contract should return `notFound()` instead of exposing placeholder UI. Current blocked starter routes include `billing`, `exclusive`, `workspaces`, and nested `workspaces/team`.
- Implements the same typed snake_case route-selection contract as the Leptos admin host, but through local Next helpers instead of shared Rust code.
- Shares SEO control-plane API adapters in `src/shared/api/seo.ts` with REST primary (rollout-gated) + GraphQL secondary path reads for targets, diagnostics, sitemap jobs, and bulk jobs.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)

## Dependency verification — 2026-10-17

Use the checked-in npm lockfile: `npm ci`, `npm run typecheck`, `npm run lint`.
Build/verify `packages/richtext` with its own npm lock before using its shared
frame assets. Next is pinned to 16.3.8; React/React DOM and their types share
19.3.0, ESLint is 9.39.4 and TypeScript is 6.0.3. Tiptap's shared package uses
one 3.30.5 cohort. No `--force` or legacy peer install flags are required, and
Next build-time type errors are no longer ignored. Full host typecheck/lint and
scoped Forum tests pass on the installed graph; browser/runtime acceptance is
separate. See the central richtext plan for dependency evidence and limits.

Docker uses the repository root as its build context:
`docker build -f apps/next-admin/Dockerfile --target production .`.
The npm-only stages retain local file dependency paths, build richtext assets,
and copy the standalone app plus those assets into the image. The old Bun
lockfile is removed. Docker image build/runtime still require external verification.
