# Документация модуля `rustok-profiles`

`rustok-profiles` — модуль универсального публичного профиля пользователя для RusToK.

## Назначение

- дать единую profile-boundary поверх platform `users`;
- не смешивать identity/auth, public profile, commerce customer и будущий seller account;
- стать каноническим источником author/member summary для groups, forum, blog, social и commerce surfaces.

## Текущий статус

- поднят базовый module scaffold (`ProfilesModule`, `rustok-module.toml`, permissions, docs);
- зафиксированы DTO/enum-контракты для `ProfileVisibility`, `ProfileStatus`, `ProfileSummary` и `UpsertProfileInput`;
- реализованы SeaORM entity-модели `profiles` и `profile_translations`;
- добавлены module-local миграции для storage boundary и tenant-scoped handle uniqueness;
- поднят DB-backed `ProfileService` с `upsert/get-by-user/get-by-handle/get-summary` path, locale fallback helper и batched summary lookup;
- добавлен явный `ProfilesReader` contract для downstream Rust-модулей, который теперь читает summaries пакетно без внутреннего N+1;
- поднят GraphQL transport boundary: `ProfilesQuery` и `ProfilesMutation` для `profile_by_handle`, `me_profile`, `profile_summary`, `upsert_my_profile` и targeted self-service update mutations;
- в shared server runtime зарегистрирован `ProfileSummaryLoader`, а `blog/forum` используют его как request-scoped cache с fallback на service path;
- `rustok-blog` и `rustok-forum` уже используют `ProfilesReader` для author presentation в GraphQL read-path;
- profile write-path теперь публикует outbox-событие `profile.updated` для downstream sync/re-render/index сценариев;
- profile write/read path теперь поддерживает taxonomy-backed `tags` через module-owned relation `profile_tags`;
- `rustok-profiles` зависит от `rustok-taxonomy`, но словарь терминов остаётся shared, а привязки живут внутри модуля профилей;
- `ProfileService` умеет explicit backfill missing profiles из существующих `users`/`customer`-seed данных с безопасной генерацией handle;
- module-owned UI пока ещё не реализован.

## Архитектурная граница

- `users` остаётся identity/security слоем: логин, пароль, сессии, роли, статус.
- `profiles` — отдельная доменная надстройка над `user_id`.
- `customer` остаётся отдельным commerce-подмодулем с optional linkage на `user_id`, а не становится каноническим профилем платформы.
- будущие seller/merchant surfaces должны жить в отдельном домене, а не внутри `profiles`.

## Первичный domain scope

- public handle;
- display name с canonical fallback в `profiles` и localized overrides в `profile_translations`;
- avatar/banner references через `rustok-media`;
- bio и локализуемые public-поля;
- profile tags / interests через shared taxonomy dictionary и module-owned relation `profile_tags`;
- preferred locale и visibility policy для публичной страницы.

## Зафиксированные MVP-решения

- `display_name` и `bio` локализуются через `profile_translations`, но `profiles.display_name` остаётся обязательным fallback/default значением для read path без translation.
- Профиль создаётся lazy при первом self-service write через `upsert_profile` / `upsert_my_profile`; обычный read path не создаёт профиль автоматически и может возвращать `null` / not found.

## Следующий шаг

- подготовить integration contract для будущих social/groups surfaces;
- решить, нужен ли отдельный projection/read-model помимо прямого чтения `profiles + profile_translations`;
- довести rollout policy для `profile.updated` и решить, где нужен event replay/backfill помимо server task;
- подготовить module-owned UI packages для admin/storefront после фиксации доменного контракта.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [README crate](../README.md)
- [Карта документации](../../../docs/index.md)
