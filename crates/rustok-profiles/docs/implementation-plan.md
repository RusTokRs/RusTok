# План реализации `rustok-profiles`

## Назначение

Этот документ — канонический roadmap для `rustok-profiles`.

Он фиксирует, как довести модуль от текущего scaffold-состояния до полноценного
platform profile layer, который смогут использовать forum, blog, groups, social,
account и commerce-adjacent surfaces без смешивания с auth identity или customer
доменом.

## Цель

Собрать модуль универсального публичного профиля пользователя, который:

- живёт поверх platform `users` как отдельная domain boundary;
- становится каноническим источником author/member summary;
- поддерживает public profile page, `me/profile` и reusable summary lookup;
- не схлопывает `profiles`, `customers`, staff/admin identity и будущие
  seller/merchant accounts в одну сущность.

## Текущее состояние

На момент этого плана уже есть:

- `ProfilesModule` с metadata и permission surface `profiles:*`;
- первичные DTO и enum-контракты: `ProfileSummary`, `UpsertProfileInput`,
  `ProfileVisibility`, `ProfileStatus`;
- SeaORM entity-модели `profiles` и `profile_translations`;
- module-local migrations для storage boundary;
- DB-backed `ProfileService` с `upsert/get-by-user/get-by-handle/get-summary`
  path, handle policy, locale fallback helper и batched summary lookup;
- explicit backfill path для missing profiles c генерацией display_name/handle из
  user/customer seed-данных;
- явный `ProfilesReader` trait для downstream Rust-кода;
- GraphQL transport boundary с `ProfilesQuery` и `ProfilesMutation`;
- targeted GraphQL mutations для update handle/content/locale/visibility/media;
- host-level `ProfileSummaryLoader` для request-scoped batching/cache в GraphQL;
- `ProfileRecord` как read-модель resolved profile state;
- `rustok-blog` и `rustok-forum` уже читают author presentation через
  `ProfilesReader`;
- минимальный outbox/event contract `profile.updated` для downstream sync;
- отсутствие REST adapters и module-owned UI.

Практический вывод: storage/service/GraphQL foundation уже подняты, а следующий
рабочий шаг — довести rollout/runtime policy вокруг `profile.updated`,
backfill существующих пользователей и возможного projection/read-model поверх
уже работающего batched integration contract.

## Архитектурная граница

### Что остаётся вне `rustok-profiles`

- `users` остаётся identity/security boundary: login, password, sessions, auth
  policies, role membership, lifecycle статусы.
- `rustok-customer` остаётся отдельным commerce-поддоменом для storefront
  customer profile, addresses, order-facing data и optional linkage на `user_id`.
- будущий seller/merchant domain не должен жить внутри `profiles`.
- staff/admin-specific attributes не должны попадать в public profile contract.

### Что входит в `rustok-profiles`

- public handle;
- display name;
- bio и другие публичные локализуемые поля;
- avatar/banner references через media boundary;
- preferred locale и visibility policy;
- summary/read contract для author/member rendering в других модулях;
- публичная страница профиля и self-service edit path.

### Интеграционные ожидания

- forum/blog/groups/social не должны читать author/member presentation прямо из
  `users`;
- storefront/account surfaces должны использовать `profiles` как public-facing
  слой, а не переиспользовать `customer` как универсальный профиль платформы;
- `profiles` должен давать batched read API для карточек автора/участника, чтобы
  не плодить cross-module joins на UI-слое.

## Целевая модель данных

### Базовая таблица `profiles`

MVP-модель рекомендуется зафиксировать как `1:1` над `users`:

- `id` или `user_id` как canonical linkage;
- `tenant_id`;
- `user_id`;
- `handle`;
- `display_name` или `default_display_name` как fallback, если локализация имени
  не войдёт в MVP;
- `avatar_media_id`;
- `banner_media_id`;
- `preferred_locale`;
- `visibility`;
- `status`;
- `created_at`, `updated_at`.

### Локализуемые данные

Локализуемые public-поля должны жить отдельно в `profile_translations`:

- `profile_id`;
- `locale`;
- `display_name`;
- `bio`;
- optional future localized tagline/about fields.

### Отложенные сущности

Не тащить в MVP без явной необходимости:

- social links / external links;
- follower graph;
- reputation, counters и engagement analytics;
- seller/store metadata;
- moderation history как публичную часть профиля.

Если для них потребуется storage, лучше вводить отдельные таблицы после фиксации
MVP boundary, а не раздувать `profiles` на старте.

## Delivery phases

### Phase 0 — Module scaffold

- [x] Создать `crates/rustok-profiles`.
- [x] Добавить `ProfilesModule`, `rustok-module.toml` и permission surface
  `profiles:*`.
- [x] Зафиксировать README и локальную документацию модуля.
- [x] Определить базовые DTO и первичную entity/service-заготовку.

### Phase 1 — Storage model и миграции

- [x] Зафиксировать `profiles` как основной aggregate storage.
- [x] В MVP оставить `user_id` как canonical primary key вместо surrogate `id`.
- [x] Вынести локализуемые поля в `profile_translations`.
- [x] Ввести enum/contract для `visibility` и `status` на DB-слое.
- [x] Добавить уникальные ограничения и индексы:
  - [x] `tenant_id + normalized_handle` или alternative uniqueness strategy;
  - [x] lookup index для public handle read path;
  - [x] lookup index для translation lookup path `profile_user_id + locale`.
- [x] Подготовить migration scaffold и smoke-check на применение схемы.
- [x] Зафиксировать `display_name` fallback-полем в `profiles`, а локализуемые
  `display_name`/`bio` держать в `profile_translations`.
- [ ] Зафиксировать, нужен ли дополнительный secondary index для mixed
  `tenant_id + user_id` read path поверх существующего primary key.

Критерий завершения phase:

- модуль поднимает собственные миграции;
- storage shape зафиксирован документально и кодом;
- есть явное решение по handle uniqueness boundary.

### Phase 2 — Domain services и write/read contracts

- [x] Реализовать DB-backed `ProfileService`.
- [x] Разделить write-path и public read-path, чтобы summary lookup не тянул
  лишние поля.
- [x] Оформить `upsert_profile(...)` policy для self-service профиля.
- [x] Зафиксировать normalization/validation rules для handle:
  - [x] lowercase + trim уже есть;
  - [x] добавить reserved handles policy;
  - [x] добавить min/max length;
  - [x] на текущем этапе явно запретить non-ascii в MVP.
- [x] Добавить locale fallback chain:
  `requested -> profile preferred -> tenant default`.
- [x] Подготовить первичный summary read path через `get_profile_summary(...)`.
- [x] Вынести явный `ProfilesReader` / `ProfileSummaryReader` trait/contract для
  других модулей.
- [x] Зафиксировать MVP lifecycle как lazy create on first self-service write
  через `upsert_profile(...)`; read path остаётся nullable/not-found до явного
  создания профиля.
- [x] Расширить service layer batched read path для нескольких `user_id`.
- [x] Добавить explicit backfill/service path для существующих пользователей без
  auto-create в обычном read path.

Критерий завершения phase:

- сервис работает поверх БД, а не только на helper-уровне;
- summary contract можно использовать без прямой зависимости на `users`;
- validation policy зафиксирована тестами.

### Phase 3 — Transport adapters

- [x] Добавить GraphQL read path для public profile lookup по `handle`.
- [x] Добавить GraphQL read/write path для `me/profile`.
- [x] Добавить GraphQL summary path для downstream/UI lookup.
- [x] Определить mutation surface:
  - [x] update handle;
  - [x] update public fields;
  - [x] update locale/visibility;
  - [x] attach avatar и banner references;
  - [x] выделить отдельные targeted mutations вместо одного `upsert_my_profile`.
- [x] Зафиксировать error mapping для типовых кейсов:
  - [x] handle taken;
  - [x] profile not found;
  - [ ] forbidden visibility transition;
  - [x] invalid locale.
  - [ ] invalid media reference.
- [ ] REST surface добавлять только если появится реальный внешний consumer.

Критерий завершения phase:

- есть стабильный API contract для self-service и public rendering;
- downstream UI может жить только поверх module-owned transport layer.

### Phase 4 — Public read model, batching и performance

- [x] Подготовить тонкий summary/read model для author/member cards.
- [x] Поддержать batched lookup по `user_id` и/или `handle`.
- [x] Исключить N+1 path для forum/blog rendering.
- [ ] Решить, нужен ли отдельный projection/read-model или достаточно прямого
  чтения из `profiles` + `profile_translations` в MVP.
- [x] Добавить hooks для DataLoader/host-level caching там, где это уже принято
  в server runtime.

Критерий завершения phase:

- forum/blog могут массово читать author/member summary без прямого запроса к
  `users`;
- latency профиля не зависит от UI-side fan-out.

### Phase 5 — UI surfaces

- [ ] Добавить module-owned account/admin editor для профиля.
- [ ] Добавить module-owned public/storefront profile surface.
- [ ] Вынести reusable UI blocks:
  - [ ] profile header;
  - [ ] author card;
  - [ ] member card;
  - [ ] compact avatar/name badge.
- [ ] Синхронизировать Leptos и Next host contracts вокруг одного transport API,
  если модуль будет использоваться в обоих UI-стэках.

Критерий завершения phase:

- профиль редактируется без host-specific ad-hoc кода;
- author/member presentation больше не дублируется по модулям.

### Phase 6 — Интеграция с другими доменами

- [x] Перевести forum/blog на `ProfilesReader`.
- [x] Подключить optional `customer -> user -> profile` bridge без схлопывания
  доменов.
- [ ] Подготовить integration contract для будущих social/groups surfaces.
- [x] Зафиксировать минимальное событие `profile.updated` для downstream
  re-render и search/index синхронизации.
- [x] Подготовить tenant-scoped backfill path для существующих пользователей
  через server task с `dry_run` и optional `profile.updated` publishing.

Критерий завершения phase:

- ключевые user-facing вертикали blog/forum читают profile summary через модуль;
- больше нет прямой presentation-зависимости blog/forum на `users`, а для
  будущего `groups` уже есть готовый integration contract.

### Phase 7 — Hardening, observability и rollout

- [ ] Добавить audit trail для изменений публичного профиля.
- [ ] Зафиксировать visibility/privacy policy и ограничения редактирования.
- [ ] Подготовить метрики и logging:
  - [ ] create/update success/failure;
  - [ ] handle conflict rate;
  - [ ] public read latency;
  - [ ] profile-not-found rate.
- [ ] Добавить unit/integration tests на storage, service, transport и fallback
  locale policy.
- [ ] Описать operator/runbook-поведение для миграций и initial backfill.

Критерий завершения phase:

- модуль можно безопасно включать в shared platform runtime;
- есть диагностика конфликтов, деградаций и rollout-side эффектов.

## Открытые решения

- [ ] Handle uniqueness global или tenant-scoped?
- [x] `display_name` локализуемый в MVP, но `profiles.display_name` остаётся
  canonical fallback/default display name для прямого read path и отсутствующей
  translation.
- [ ] Нужен ли `banner_media_id` уже в первой итерации, или он должен остаться
  post-MVP полем?
- [x] Профиль в MVP создаётся lazy при первом self-service edit (`upsert_*`);
  public/admin read path не создаёт его автоматически.
- [ ] Должен ли модуль сразу поддерживать staff-hidden / internal-only visibility
  mode, или достаточно `public | authenticated | followers_only | private`?

## Приоритетный backlog на следующую итерацию

1. Решить, нужен ли отдельный projection/read-model помимо прямого чтения из `profiles` + `profile_translations`.
2. Подготовить integration contract для будущих social/groups surfaces.
3. Закрыть оставшиеся storage-решения по handle uniqueness boundary и optional secondary index.
4. Уточнить visibility/media policy для следующего UI-среза.
5. Решить, где кроме server task потребуется event replay/backfill для новых downstream consumers `profile.updated`.

## Definition of Done для MVP

`rustok-profiles` можно считать доведённым до MVP, когда:

- модуль поднимает свои миграции и хранит profile data в БД;
- есть DB-backed service layer и тесты на handle policy;
- есть GraphQL path для self-service edit и public read;
- forum/blog могут читать author/member summary через `ProfilesReader`, а для
  будущего `groups` уже готов module-level reader contract;
- public profile данные отделены от auth identity и customer domain не только
  концептуально, но и на уровне runtime/API contracts.
