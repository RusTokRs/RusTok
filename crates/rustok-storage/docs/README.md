# Документация `rustok-storage`

`rustok-storage` — shared storage abstraction layer платформы. Он даёт единый
`StorageBackend` contract для доменных модулей, которым нужно хранить файлы,
независимо от конкретного backend-а.

## Назначение

- публиковать канонический storage backend contract;
- изолировать доменные модули от деталей local/S3-compatible storage implementation;
- держать единый high-level `StorageService` для file-oriented сценариев платформы.

## Зона ответственности

- `StorageBackend`, `UploadedObject`, `StorageService`;
- backend selection/configuration и path generation helpers;
- local storage implementation и future backend seams;
- storage errors, public URL construction и path-safety guarantees;
- отсутствие domain-owned media/business logic.

## Интеграция

- используется `rustok-media` и другими file-oriented модулями как shared storage dependency;
- `apps/server` выступает только wiring-слоем для регистрации `StorageService`;
- storage health и basic observability должны оставаться синхронизированными с host/runtime docs;
- domain modules не должны обходить `rustok-storage` прямым backend-specific кодом без явной причины.

## Проверка

- structural verification: local docs и storage contract должны оставаться синхронизированными;
- targeted compile/tests при изменении `StorageBackend`, path safety или backend configuration;
- integration checks нужны при изменении backend implementations и health semantics.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [Документация `rustok-media`](../../rustok-media/docs/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
