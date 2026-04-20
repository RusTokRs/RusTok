# RusToK Next Storefront

## Purpose

`apps/next-frontend` owns the Next.js storefront application for RusToK.

## Responsibilities

- Provide the React/Next storefront host for customer-facing experiences.
- Keep shared frontend transport/auth/i18n contracts aligned with the Leptos storefront.
- Organize storefront composition through `src/app`, `src/modules`, and shared integration layers.

## Entry points

- `src/app/*`
- `src/modules/*`
- `src/shared/lib/*`
- Next.js App Router entrypoints and layouts

## Interactions

- Uses `apps/server` as the backend/API provider.
- Works in parallel with `apps/storefront` for storefront parity at the contract level.
- Reuses shared frontend contracts instead of duplicating auth and transport logic per page.
- Consumes the canonical SEO contract from `rustok-seo` through a Next Metadata adapter while Rust hosts use `rustok-seo-render`.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
