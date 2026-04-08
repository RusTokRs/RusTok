# Документация `rustok-media`

`rustok-media` — доменный модуль управления медиаактивами платформы. Он
держит upload/storage metadata, translations и module-owned admin surface,
опираясь на `rustok-storage` как физический storage backend.

## Назначение

- публиковать канонический media runtime contract для upload/list/delete/translation flows;
- держать media metadata, validation и transport surfaces внутри модуля;
- предоставлять platform-wide media capability без размывания domain логики по host-слою.

## Зона ответственности

- `MediaService`, media entities/DTOs и translation upsert contract;
- GraphQL и REST adapters модуля;
- upload validation по size/MIME policy и tenant isolation;
- module-owned admin UI package `rustok-media-admin`;
- observability signals для upload/delete/storage health.

## Интеграция

- использует `rustok-storage` как storage backend contract;
- `apps/server` остаётся composition root и wiring-слоем для media routes/graphql;
- runtime guard опирается на tenant-scoped module enablement для public surfaces;
- upload остаётся REST-first path, а GraphQL сохраняется для read/mutation flows без multipart expansion.

## Проверка

- `cargo xtask module validate media`
- `cargo xtask module test media`
- targeted tests для upload validation, translation upsert, storage cleanup и admin-facing read/write contracts

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
