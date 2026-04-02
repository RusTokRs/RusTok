# Модуль Channel

`rustok-channel` — экспериментальный `Core`-модуль, который вводит platform-level контекст канала для внешних точек доступа: сайтов, приложений, API-клиентов, embedded target'ов и других delivery surface.

## Задачи модуля

- хранить каноническую сущность `Channel`;
- хранить targets канала с явным `target_type`;
- хранить простые channel-to-module bindings;
- связывать канал с существующими OAuth-приложениями;
- дать платформе базовый channel context без разрастания логики в `apps/server`.

## Что считается каналом в v0

В первой версии канал трактуется как внешний контекст публикации и доступа, а не как исключительно sales channel.

Это позволяет использовать слой channel management не только для commerce, но и для `blog`, `forum`, `pages` и других модулей.

## Границы v0

Первая версия сознательно минимальна:

- без полноценной omnichannel taxonomy;
- без собственной token-подсистемы;
- без storefront UI;
- без GraphQL transport adapter'ов;
- без финальной taxonomy `channel/site/market/touchpoint`.

Цель v0 — получить рабочий platform baseline и проверить модель на реальных сценариях.

## Что уже есть в runtime

- storage-модель `channels`, `channel_targets`, `channel_module_bindings`, `channel_oauth_apps`;
- post-v0 storage для typed policy layer: `channel_resolution_policy_sets`, `channel_resolution_policy_rules`;
- service layer для создания каналов, target'ов, module bindings и OAuth app bindings; для `v0` target semantics остаются на уровне `target_type + value`, но с explicit allowlist типов и `web_domain`-only host resolution;
- domain-owned resolution layer в `rustok-channel`: `RequestFacts`, `ResolutionDecision`, `ResolutionTraceStep`, `ChannelResolver`;
- persisted typed policy model в том же модуле: versioned `ChannelResolutionRuleDefinition`, typed predicates `HostEquals` / `HostSuffix` / `OAuthAppEquals` / `SurfaceIs` / `LocaleEquals` и typed action `ResolveToChannel`;
- `web_domain` target semantics дополнительно стабилизированы общей canonical normalization/validation: storage и runtime host lookup теперь одинаково режут scheme/path/port, lower-case'ят host и отбрасывают невалидные значения;
- server middleware больше не держит локальную business-логику выбора канала, а только собирает request facts и применяет domain resolver pipeline; текущий runtime order теперь выглядит как `header (X-Channel-ID / X-Channel-Slug) -> query channel -> built-in host target slice -> tenant-scoped typed policies -> explicit default channel`;
- у канала появился explicit default flag, а admin flow умеет назначать tenant default без опоры на порядок создания;
- общий request contract в `rustok-api` для channel-aware transport/adapters, включая `channel_id`, `channel_slug` и `channel_resolution_source`;
- тонкий REST surface в `apps/server` для bootstrap, создания каналов, target'ов и bindings;
- module-owned Leptos admin UI package `rustok-channel-admin`, подключаемый в `apps/admin` через manifest-driven wiring и уже показывающий explicit resolution source в runtime context bootstrap panel.
- первый живой consumer в `rustok-pages`: public read-path уже использует `channel_module_bindings` для runtime gating, а поверх этого появился первый publication-level proof point через `channel_slug` allowlist в metadata страниц.
- второй живой consumer в `rustok-blog`: тот же паттерн теперь тоже расширен до publication-level semantics через metadata-based `channelSlugs` allowlist.
- третий живой consumer в `rustok-commerce`: storefront REST/GraphQL уже используют `channel_module_bindings` для runtime gating, cart/order snapshot'ы сохраняют `channel_id`/`channel_slug`, а catalog/shipping visibility и storefront inventory availability могут ограничиваться metadata-based allowlist по `channel_slug`.

## Что проверено

Текущий baseline уже подтверждён локальной проверкой:

- `cargo test -p rustok-channel --lib`;
- `cargo check -p rustok-channel`;
- `cargo check -p rustok-admin`;
- `cargo check -p rustok-server`;
- `cargo test -p rustok-api --lib`;
- `cargo test -p rustok-server middleware::channel::tests --lib`;
- `cargo test -p rustok-server registry_dependencies_match_runtime_contract --lib`;
- `cargo test -p rustok-server registry_module_readmes_define_interactions_section --lib`.

## Следующий логичный шаг

Следующий этап для модуля — не расширение infrastructure ради infrastructure, а проверка, достаточно ли текущей модели на реальном domain behavior.

Текущие proof point-ы уже выглядят так:

- `pages` использует channel binding на public read-path и уже расширен до metadata-based publication semantics по `channel_slug`;
- `blog` использует тот же паттерн и уже тоже расширен до metadata-based publication semantics по `channel_slug`.

На текущем этапе решение уже зафиксировано:

- для v0 сохраняем `channel_module_bindings + metadata-based allowlist`;
- отдельную relation/table откладываем до появления требований, которые нельзя закрыть request-time filtering;
- дальнейшую taxonomy и richer semantics расширяем только поверх этого зафиксированного baseline;
- следующий архитектурный шаг — не `tenant-level default rules`, а `tenant-scoped typed resolution policies`.

## Что хотим проверить дальше

- достаточно ли текущей модели `channel + target + module binding + oauth app binding`;
- нужен ли split между `target` и `connector`;
- когда именно понадобятся publishable keys;
- какие domain modules должны стать channel-aware в первую очередь;
- насколько текущий admin flow пригоден для реальной операторской работы.

## Взаимодействие с другими частями платформы

- `apps/server` знает модуль и монтирует его как обязательный `Core`;
- `apps/server` остаётся thin composition root: channel domain/service/storage живут в модуле, а server владеет только middleware и transport wiring;
- `rustok-api` хранит общий `ChannelContext` и request-level contracts;
- `rustok-auth` остаётся источником истины для `oauth_apps` и access tokens;
- tenant lifecycle не управляет включением/выключением `channel`, потому что это `Core`;
- admin UI уже module-owned и живёт в `crates/rustok-channel/admin`;
- доменные модули могут постепенно становиться channel-aware через request context или channel bindings;
- `rustok-pages`, `rustok-blog` и `rustok-commerce` уже служат живыми proof point-ами для этого подхода.
