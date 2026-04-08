# rustok-media

## Purpose

`rustok-media` owns media asset uploads, metadata, translations, and transport adapters for RusToK.

## Responsibilities

- Provide the shared media domain service and SeaORM entities for uploads and localized metadata.
- Own media GraphQL and REST transport adapters for module-facing APIs.
- Publish the module-owned Leptos admin UI crate `rustok-media-admin`.
- Integrate storage-backed file lifecycle with tenant-aware media records.

## Interactions

- Depends on `rustok-core` for shared runtime helpers such as `generate_id()`.
- Depends on `rustok-storage` for blob persistence and public URL resolution.
- Depends on `rustok-api` for shared tenant/auth and GraphQL helper contracts.
- Exposes its own GraphQL and REST adapters; `apps/server` now acts only as a composition root
  and re-export shim for media transport entry points.
- REST adapters require authenticated `AuthContext`; GraphQL resolvers keep the existing
  module-enabled guard and tenant-explicit contract.
- `rustok-media-admin` uses native Leptos `#[server]` functions as the default internal data layer,
  keeps GraphQL as the fallback for `list/detail/translations/delete/usage`, and preserves REST-first
  upload via `/api/media`.

## Entry points

- `MediaService`
- `graphql::MediaQuery`
- `graphql::MediaMutation`
- `controllers::routes`
- `rustok-media-admin`
- `MediaItem`
- `MediaTranslationItem`
- `UploadInput`
- `UpsertTranslationInput`

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
