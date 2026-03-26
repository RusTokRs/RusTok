# План реализации `rustok-channel` v0

Статус: experimental core capability.

## Состояние на 2026-03-26

`rustok-channel` уже доведён до рабочего v0 baseline:

- storage, service layer и миграции подняты;
- модуль зарегистрирован как `Core` и подключён в `modules.toml`;
- server умеет резолвить `ChannelContext` на запросе;
- есть тонкий REST surface для admin flow;
- есть module-owned Leptos admin UI;
- есть первый живой consumer в `rustok-pages`, уже расширенный до publication-level proof point;
- есть второй живой consumer в `rustok-blog`, уже тоже расширенный до publication-level proof point;
- документация и ADR приведены в актуальное состояние.

Это значит, что следующую сессию можно начинать не с инфраструктурного scaffolding, а уже с первого продуктового/интеграционного шага поверх существующего baseline.

## Цели v0

Собрать лёгкий рабочий прототип, который:

- вводит platform-level сущность `Channel`;
- позволяет привязать к каналу один или несколько target'ов;
- позволяет связать канал с существующим `oauth_app`;
- позволяет явно пометить, какие platform modules участвуют в канале.

## Что входит в v0

### Storage

- [x] `channels`
- [x] `channel_targets`
- [x] `channel_module_bindings`
- [x] `channel_oauth_apps`

### Domain/service layer

- [x] создание канала;
- [x] получение канала;
- [x] список каналов;
- [x] получение расширенного channel detail;
- [x] добавление target'а;
- [x] привязка module binding;
- [x] привязка OAuth-приложения.

### Host wiring

- [x] регистрация `rustok-channel` как `Core` module;
- [x] добавление модуля в `modules.toml`;
- [x] включение миграций в server migrator;
- [x] обновление центральной документации и ADR.

### Runtime/context

- [x] общий `ChannelContext` в `rustok-api`;
- [x] server middleware для channel resolution;
- [x] прокидывание channel-aware данных в `RequestContext`.

### Transport/admin

- [x] тонкий REST bootstrap/write surface в `apps/server`;
- [x] module-owned Leptos admin UI package `rustok-channel-admin`;
- [x] manifest-driven подключение UI в `apps/admin`;
- [x] учёт `Core`-модулей с UI в admin host wiring.

### Проверка и фиксация baseline

- [x] `cargo check -p rustok-channel`;
- [x] `cargo check -p rustok-admin`;
- [x] `cargo check -p rustok-server`;
- [x] `cargo test -p rustok-api --lib`;
- [x] `cargo test -p rustok-pages graphql::query::tests --lib`;
- [x] `cargo test -p rustok-blog graphql::query::tests --lib`;
- [x] `cargo test -p rustok-server registry_dependencies_match_runtime_contract --lib`;
- [x] `cargo test -p rustok-server registry_module_readmes_define_interactions_section --lib`.

## Что не входит в v0

- финальная taxonomy `channel/site/market/touchpoint`;
- channel-owned access tokens;
- storefront UI;
- GraphQL adapter'ы;
- publishable keys;
- сложный rule engine для per-module/per-market resolution;
- typed credential taxonomy поверх existing OAuth subsystem.

## Что уже реализовано в коде

### Модуль и storage

- сущности `Channel`, `ChannelTarget`, `ChannelModuleBinding`, `ChannelOauthApp`;
- миграции для четырёх базовых таблиц;
- `ChannelService` с create/get/list/detail/bind flows.

### Server/runtime wiring

- регистрация модуля в `apps/server`;
- подключение миграций в server migrator;
- middleware для разрешения канала по header/query/host/default fallback;
- thin REST endpoints `/api/channels/*`.

### Shared host contracts

- `ChannelContext` и optional extractor в `rustok-api`;
- прокидывание `channel_id` и `channel_slug` в `RequestContext`;
- расширение `TenantContextExt` для server middleware chain.

### Первый domain consumer

- `rustok-pages` использует `RequestContext.channel_id` на public GraphQL read-path;
- `channel_module_bindings` уже участвуют в реальном runtime gating для `pages`;
- authenticated/admin flow пока intentionally bypass-ит этот gate, чтобы pilot не ломал operator UX.

### Admin UI

- `rustok-channel-admin` как module-owned Leptos package;
- bootstrap page с просмотром runtime context;
- создание channel;
- добавление target;
- binding модулей;
- binding OAuth apps;
- host support для `Core`-модулей с UI в `apps/admin`.

## Стартовая точка на следующую сессию

Завтра начинать отсюда:

1. Расширить уже существующий pilot в `pages` или перенести паттерн во второй модуль.
2. Зафиксировать, что текущий `channel_module_bindings + metadata-based allowlist` хватает для v0 без новых domain-таблиц.
3. Отложить отдельную relation/table до появления требований, которые не покрываются request-time filtering.
4. Только после этого расширять taxonomy и правила resolution.

Текущие proof point-ы:

- `pages`: public read-path уже channel-aware через module binding и metadata-based `channelSlugs` allowlist.
- `blog`: public read-path уже channel-aware через module binding и metadata-based `channelSlugs` allowlist.

## Ближайший план после v0

### Приоритет 1. Первый domain consumer

- [x] выбрать pilot-модуль (`pages`);
- [x] определить минимальный channel-aware use case;
- [x] привязать public read-path этого модуля к `ChannelContext`;
- [x] перенести тот же паттерн в `blog` для сравнения поведения на втором домене;
- [x] расширить `pages` до первого publication-level proof point через metadata-based `channelSlugs` allowlist.
- [x] проверить на сравнении `pages` vs `blog`, достаточно ли текущих `channel_module_bindings` и metadata-подхода без новой relation/table.
- [x] решить, переносим ли publication-level semantics в `blog` или сразу проектируем отдельную entity-to-channel model.

### Приоритет 2. Усиление runtime resolution

- [ ] решить, нужен ли явный policy order для `header -> query -> host -> default`;
- [ ] определить, нужны ли tenant-level default rules для target resolution;
- [ ] добавить более явную диагностику, почему был выбран конкретный канал.

### Приоритет 3. Уточнение channel semantics

- [ ] проверить, хватает ли текущего `target_type + value`;
- [ ] решить, нужен ли переход к более typed target payload;
- [ ] отделить то, что является target, от того, что является connector/integration.

### Приоритет 4. Credential story

- [ ] проверить, достаточно ли связки `channel -> oauth_app`;
- [ ] решить, нужны ли publishable keys;
- [ ] не допустить появления второй параллельной token subsystem рядом с OAuth.

### Приоритет 5. Admin UX polish

- [ ] показать более явный resolution source в UI;
- [ ] добавить редактирование существующих target/module/app bindings;
- [ ] добавить удаление/revoke flows;
- [ ] добавить пустые состояния и более понятные ошибки для операторов.

## Эволюция после v0

После эксплуатации можно отдельно решить:

- нужен ли split `channel` / `site` / `connector`;
- нужны ли publishable keys;
- как выглядит channel-aware request context;
- какие модульные области получают channel-specific settings.
