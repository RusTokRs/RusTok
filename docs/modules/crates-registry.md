# Реестр crate-модулей `crates/rustok-*`

Документ фиксирует единый обзор по библиотекам `crates/rustok-*`:

- зона ответственности;
- публичные entry points (основные re-export API, сервисы, трейты);
- недопустимые прямые обходы модульного слоя.

Источник данных для строк таблицы: `README.md` и `docs/implementation-plan.md` внутри каждого crate.

## Границы модульного подхода (важно)

RusToK использует **смешанную архитектуру**, а не «всё в модули»:

- существенная часть платформенного функционала живёт в `apps/server` и core-crate’ах (`rustok-core`, `rustok-outbox`, `rustok-telemetry`, `rustok-events`);
- `crates/rustok-*` закрывают доменные и инфраструктурные контракты повторного использования;
- если функциональность по природе является server/runtime-оркестрацией, её можно оставлять в серверном слое (без искусственного выделения в отдельный модуль).

## Обязательное правило расширения доменов

> Новый функционал в домене добавляется **сначала в соответствующую библиотеку `crates/rustok-*`** (модели, сервисы, события, валидация, контракты), и только после этого потребляется приложениями (`apps/server`, `apps/*frontend`).

Это правило применяется к доменным сценариям и не отменяет ответственность платформенного ядра (`apps/server` + core crates) за runtime-оркестрацию.

## Единый реестр

| Crate | Ответственность | Публичные entry points | Запрещённые прямые обходы |
|---|---|---|---|
| `rustok-core` | Базовые платформенные контракты: модульная модель, события, безопасность, RBAC-примитивы, инфраструктурные типы. | `RusToKModule`, `ModuleRegistry`, `SecurityContext`, `DomainEvent`, `EventEnvelope`, `ModuleContext`, `ModuleKind`, `Permission`, `Resource`, `Action`. | Нельзя дублировать core-контракты в приложениях/модулях (свои альтернативные `SecurityContext`, event-envelope, module lifecycle). |
| `rustok-events` | Стабильная точка импорта событийной модели (re-export из core). | `DomainEvent`, `EventEnvelope` (и root re-export). | Нельзя определять «локальные копии» доменных событий в `apps/*` и обходить единый event-контракт. |
| `rustok-content` | Контентное ядро: узлы, переводы, workflow draft/published/archived, базовые content-сервисы. | `ContentService`, `NodeService`, `TaxonomyService`, `TranslationService`, `ContentModule`, DTO/re-exports (`Node`, `NodeTranslation`, `Body`). | Нельзя писать прямой SQL к контент-таблицам из `apps/server`, если сценарий закрывается сервисами `rustok-content`. |
| `rustok-commerce` | Каталог, цены, инвентарь и жизненный цикл заказов. | `CatalogService`, `PricingService`, `InventoryService`, `CommerceModule`, типы order state machine. | Нельзя обходить сервисы commerce и обновлять заказы/остатки напрямую из `apps/server` или других модулей. |
| `rustok-blog` | Блоговый домен поверх content (wrapper): посты, комментарии, SEO/i18n, state machine, события блога. | `PostService`, `CommentService`, `BlogModule`, DTO (`CreatePostInput`, `UpdatePostInput`, `PostListQuery`), статусы `BlogPostStatus`. | Нельзя реализовывать blog-правила напрямую через `rustok-content` (или SQL) в приложениях, минуя `PostService`/`CommentService`. |
| `rustok-forum` | Форумный домен: темы, ответы, категории, модерация, i18n-логика. | `TopicService`, `ReplyService`, `CategoryService`, `ModerationService`, `ForumModule`, DTO и константы форума. | Нельзя в `apps/server` напрямую модифицировать forum-сущности в обход сервисов форума и их правил модерации. |
| `rustok-pages` | CMS-страницы, блоки и меню, включая модульные сервисы управления страницами. | `PageService`, `BlockService`, `MenuService`, `PagesModule`, DTO/re-exports (`Page`, `Block`, `Menu`). | Нельзя менять страницы/блоки/меню через прямой доступ к БД из приложений при наличии API `rustok-pages`. |
| `rustok-index` | Индексация и поисковая интеграция: контракты индексаторов и модуль индекса. | `Indexer`, `LocaleIndexer`, `IndexerContext`, `IndexModule`, ошибки `IndexError`. | Нельзя строить ad-hoc индексацию в приложениях, минуя контракт `Indexer` и модуль `rustok-index`. |
| `rustok-rbac` | RBAC/authorization-слой платформы, резолвинг разрешений, dual-read/shadow-механики. | `RbacModule`, `PermissionResolver`, `RuntimePermissionResolver`, `PermissionAuthorizer`, `PermissionEvaluator`, `RbacAuthzMode`, `AuthzEngine`. | Нельзя внедрять собственные проверки ролей в `apps/server` в обход `rustok-rbac` (hardcoded role checks как substitute для authorizer). |
| `rustok-tenant` | Multi-tenant управление: арендаторы, включение модулей, tenant-метаданные. | `TenantService`, `TenantModule`, DTO (`CreateTenantInput`, `ToggleModuleInput`, `UpdateTenantInput`). | Нельзя менять tenant-конфигурацию напрямую в приложениях/SQL, минуя `TenantService`. |
| `rustok-outbox` | Transactional outbox и relay-доставка событий в transport-слой. | `TransactionalEventBus`, `OutboxRelay`, `RelayConfig`, `OutboxTransport`, `SysEventsMigration`. | Нельзя публиковать межмодульные события «мимо outbox» в критичных потоках, где требуется transactional delivery. |
| `rustok-iggy` | Транспортный runtime для event streaming (producer/consumer, DLQ, replay, partitioning). | `IggyTransport`, `ConsumerGroupManager`, `DlqManager`, `ReplayManager`, `TopologyManager`, сериализаторы. | Нельзя в сервисах приложений реализовывать самостоятельный transport-runtime параллельно `rustok-iggy` для тех же потоков. |
| `rustok-iggy-connector` | Подключение к Iggy (embedded/remote), pub/sub интерфейсы и конфигурация коннектора. | `IggyConnector` (trait), `MessageSubscriber` (trait), `RemoteConnector`, `EmbeddedConnector`, `ConnectorConfig`, `PublishRequest`. | Нельзя обходить connector-абстракцию, вшивая прямые ad-hoc подключения к брокеру в доменные сервисы. |
| `rustok-telemetry` | Инициализация observability: tracing + OpenTelemetry + Prometheus-метрики. | `init`, `TelemetryConfig`, `TelemetryHandles`, `MetricsHandle`, `render_metrics`, `current_trace_id`. | Нельзя настраивать разрозненные независимые telemetry pipelines в отдельных модулях, обходя единый bootstrap `rustok-telemetry`. |
| `rustok-mcp` | MCP-адаптер RusToK: сервер, инструменты и alloy-интеграция через MCP SDK. | `RusToKMcpServer`, `McpServerConfig`, `serve_stdio`, tool/alloy re-exports (`ListModulesTool`, `GetProductTool` и др.). | Нельзя реализовывать отдельные MCP entrypoints в приложениях, если сценарий уже покрывается `rustok-mcp` и его tool-слоем. |
| `rustok-test-utils` | Общий тестовый toolkit платформы: db setup, mock event bus, fixtures, helpers. | `setup_test_db`, `MockEventBus`, `MockEventTransport`, `mock_transactional_event_bus`, `fixtures::*`, `helpers::*`. | Нельзя тащить `rustok-test-utils` в production runtime и нельзя дублировать базовые фикстуры/моки без необходимости. |

## Связь с внутренними frontend-библиотеками

Самописные библиотеки в репозитории в основном ориентированы на фронтенды (`crates/leptos-*`, `tailwind-*` и др.).
Они не входят в этот реестр `crates/rustok-*`, но должны рассматриваться как first-party слой UI/UX и контрактов клиентских приложений.

## Примечание по применению ограничений

- Правило «без прямых обходов» не запрещает специализированные low-level сценарии, если они формально вынесены в модульный API соответствующего crate.
- Для кросс-модульных изменений сначала расширяйте контракт crate, затем адаптируйте потребителей в `apps/*`.
