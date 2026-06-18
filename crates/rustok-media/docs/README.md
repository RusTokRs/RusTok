# Документация `rustok-media`

`rustok-media` — доменный модуль управления медиаактивами платформы. Он
держит метаданные загрузок и хранения, переводы и модульную административную поверхность,
опираясь на `rustok-storage` как физический слой хранения.

## Назначение

- публиковать канонический runtime-контракт медиа для сценариев загрузки, списка, удаления и переводов;
- держать метаданные медиа, валидацию и транспортные поверхности внутри модуля;
- предоставлять платформенную media-возможность без размывания доменной логики по host-слою.

## Зона ответственности

- `MediaService`, media entities/DTOs и контракт обновления переводов;
- типизированный межмодульный image-контракт `MediaImageDescriptor` (`url/alt/size/mime` + derived helpers);
- GraphQL- и REST-адаптеры модуля;
- валидацию загрузок по size/MIME policy и tenant isolation;
- модульный admin UI package `rustok-media-admin` с FFA-разделением `core`/`transport`/`ui/leptos`;
- observability-сигналы для здоровья загрузки, удаления и хранения;
- нормализацию переводов: `locale` trim/lowercase, пустые `title`/`alt_text`/`caption` сохраняются как `None`, списки переводов возвращаются в стабильном порядке по locale.

## Интеграция

- использует `rustok-storage` как контракт backend-хранилища;
- `apps/server` остаётся composition root и wiring-слоем для media routes/graphql;
- runtime guard опирается на tenant-scoped module enablement для публичных поверхностей;
- загрузка остаётся REST-first path, GraphQL сохраняется для read/mutation flows без multipart-расширения, а Leptos admin adapter вызывает transport facade вместо raw API module; transport facade внутри admin package разделяет native server functions, GraphQL fallback и REST upload adapters, а upload/detail presentation state остаётся в Leptos-free `admin/src/core.rs`;
- `rustok-seo` и owner SEO providers потребляют `MediaImageDescriptor` как единственную image boundary для OG/Twitter/schema fallback; descriptor normalization покрывает explicit MIME, invalid dimensions и query/fragment cleanup.

## Проверка

- `cargo xtask module validate media`
- `cargo xtask module test media`
- targeted tests для валидации загрузок, нормализации/обновления переводов, классификации cleanup-probe хранилища и admin-facing read/write contracts

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
