# RusToK Next Admin

## Purpose

`apps/next-admin` owns the Next.js-based admin application for RusToK.

## Responsibilities

- Provide the React/Next admin host for teams working in the Next ecosystem.
- Mount module-owned Next admin packages from `packages/*`.
- Stay aligned with the Leptos admin contract without becoming the primary auto-deploy admin stack.

## Entry points

- `src/app/*`
- `src/shared/*`
- `packages/*`
- Next.js App Router entrypoints and layouts

## Interactions

- Uses `apps/server` as the backend/API provider.
- Works in parallel with `apps/admin` for UI parity and contract validation.
- Mounts package-owned module UI such as `@rustok/*-admin` instead of owning module business UI inline.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
