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
- поднят DB-backed `ProfileService` с `upsert/get-by-user/get-by-handle/get-summary` path и locale fallback helper;
- добавлен явный `ProfilesReader` contract для downstream Rust-модулей;
- поднят GraphQL transport boundary: `ProfilesQuery` и `ProfilesMutation` для `profile_by_handle`, `me_profile`, `profile_summary` и `upsert_my_profile`;
- module-owned UI пока ещё не реализован.

## Архитектурная граница

- `users` остаётся identity/security слоем: логин, пароль, сессии, роли, статус.
- `profiles` — отдельная доменная надстройка над `user_id`.
- `customer` остаётся отдельным commerce-подмодулем с optional linkage на `user_id`, а не становится каноническим профилем платформы.
- будущие seller/merchant surfaces должны жить в отдельном домене, а не внутри `profiles`.

## Первичный domain scope

- public handle;
- display name;
- avatar/banner references через `rustok-media`;
- bio и локализуемые public-поля;
- preferred locale и visibility policy для публичной страницы.

## Следующий шаг

- расширить `ProfilesReader` от текущего explicit contract до действительно batched read path без N+1;
- решить финальную MVP-политику для `display_name`/`bio` localization и lazy vs eager profile creation;
- подключить `forum/blog/groups` к `ProfilesReader` вместо прямой завязки на `users`;
- подготовить module-owned UI packages для admin/storefront после фиксации доменного контракта.

## Связанные документы

- [План реализации](./implementation-plan.md)
- [README crate](../README.md)
- [Карта документации](../../../docs/index.md)
