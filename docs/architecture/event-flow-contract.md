# Контракт потока доменных событий (Event Flow Contract)

Документ фиксирует канонический путь `DomainEvent` в RusToK: от доменной операции до обновления read-model/index.

## Канонический путь события

1. **Доменная операция**
   - Модуль (например, `rustok-content` или `rustok-commerce`) выполняет бизнес-операцию в сервисе.
   - После успешной проверки инвариантов формируется `DomainEvent`.

2. **Запись данных + outbox в одной транзакции**
   - Изменения доменных таблиц и запись события в `sys_events` выполняются через `TransactionalEventBus::publish_in_tx(...)` в рамках одного `txn`.
   - Событие считается опубликованным только после `txn.commit()`.

3. **Доставка (relay)**
   - `OutboxRelay` выбирает события `Pending` из `sys_events`, публикует их в целевой `EventTransport` и переводит статус в `Dispatched`.
   - При ошибке событие получает backoff и повтор (retry), после превышения `max_attempts` переводится в `Failed` (DLQ-семантика).

4. **Обработчик (consumer)**
   - `EventDispatcher` выбирает подходящие обработчики по `can_handle(...)`.
   - Каждый обработчик получает retries на уровне dispatcher (`retry_count + 1` попыток).
   - Для long-lived consumers платформенный runtime-contract одинаковый: `Lagged` не замалчивается (`warn` + `rustok_event_consumer_lagged_total`), `Closed` фиксируется явно и завершает loop контролируемо, а bootstrap/restart отражается через `rustok_event_consumer_restarted_total`.

5. **Обновление read-model/index**
   - Индексаторы (`rustok-index`) пересчитывают денормализованные read-модели (content/product indexes).
   - Операции обновления индекса должны быть идемпотентными: повторная обработка того же `event_id` не должна ломать состояние.

---

## Ключевые DomainEvent: контракты публикации/обработки

Ниже перечислены ключевые события, задействованные в боевом потоке publish → outbox → handlers → index.

### Контентные события

| DomainEvent | Кто публикует | Кто обрабатывает | Обязательные поля | Идемпотентность и retry |
|---|---|---|---|---|
| `NodeCreated` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `kind` | Повтор должен приводить к upsert индекса. Relay retry + dispatcher retry обязательны. |
| `NodeUpdated` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `kind` | Повтор допустим, индекс пересобирается по `node_id`. |
| `NodeTranslationUpdated` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `locale` | Повтор допустим, обновляется локализованная часть read-model. |
| `NodePublished` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `kind` | Повтор не должен дублировать записи, только фиксировать publish-state. |
| `NodeUnpublished` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `kind` | Повтор должен оставлять read-model в том же состоянии (unpublished). |
| `NodeDeleted` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `kind` | Повтор должен быть безопасным (delete-if-exists). |
| `BodyUpdated` | `rustok-content::NodeService` | `rustok-index::ContentIndexer` | `node_id`, `locale` | Повтор допустим, переиндексация body идемпотентна. |
| `TagAttached` (`target_type = "node"`) | контентные/связанные сервисы | `rustok-index::ContentIndexer` | `tag_id`, `target_type`, `target_id` | Повтор должен быть no-op, если связь уже учтена в индексе. |
| `TagDetached` (`target_type = "node"`) | контентные/связанные сервисы | `rustok-index::ContentIndexer` | `tag_id`, `target_type`, `target_id` | Повтор должен быть безопасным (detach-if-exists). |
| `CategoryUpdated` | сервисы категорий контента | `rustok-index::ContentIndexer` | `category_id` | Повтор инициирует повторный пересчет контентного индекса без побочных эффектов. |
| `CanonicalUrlChanged` | content orchestration / routing policy сервисы | `rustok-index::ContentIndexer` | `target_id`, `target_kind`, `locale`, `new_canonical_url`, `old_urls[]` | Повтор должен переиндексировать canonical-документ без дублирования; источник истины — актуальный canonical mapping. |
| `UrlAliasPurged` | content orchestration / routing policy сервисы | `rustok-index::ContentIndexer` + cache/search invalidation listeners | `target_id`, `locale`, `urls[]` | Повтор должен быть безопасным (purge-if-exists) и не возвращать alias URL в индекс. |

### Коммерческие события

| DomainEvent | Кто публикует | Кто обрабатывает | Обязательные поля | Идемпотентность и retry |
|---|---|---|---|---|
| `ProductCreated` | `rustok-commerce::CatalogService` | `rustok-index::ProductIndexer` | `product_id` | Повтор = upsert продукта в index-read-model. |
| `ProductUpdated` | `rustok-commerce::CatalogService` | `rustok-index::ProductIndexer` | `product_id` | Повтор безопасен, индекс перестраивается по `product_id`. |
| `ProductPublished` | `rustok-commerce::CatalogService` | `rustok-index::ProductIndexer` | `product_id` | Повтор не должен менять семантику состояния публикации. |
| `ProductDeleted` | `rustok-commerce::CatalogService` | `rustok-index::ProductIndexer` | `product_id` | Повтор безопасен (delete-if-exists в индексе). |
| `VariantCreated` | server commerce controller/services | `rustok-index::ProductIndexer` | `variant_id`, `product_id` | Повтор должен быть идемпотентным upsert варианта. |
| `VariantUpdated` | server commerce controller/services | `rustok-index::ProductIndexer` | `variant_id`, `product_id` | Повтор = детерминированный перерасчет продуктового индекса. |
| `VariantDeleted` | server commerce controller/services | `rustok-index::ProductIndexer` | `variant_id`, `product_id` | Повтор = безопасное удаление варианта, если он существует. |
| `InventoryUpdated` | `rustok-commerce::InventoryService` | `rustok-index::ProductIndexer` | `variant_id`, `product_id`, `location_id`, `old_quantity`, `new_quantity` | Повтор не должен ломать остатки: пересчет от source-of-truth, не инкремент. |
| `PriceUpdated` | `rustok-commerce::PricingService` | `rustok-index::ProductIndexer` | `variant_id`, `product_id`, `currency`, `new_amount` | Повтор допустим, цена в read-model выставляется как абсолютное значение. |

### Системные события индексации

| DomainEvent | Кто публикует | Кто обрабатывает | Обязательные поля | Идемпотентность и retry |
|---|---|---|---|---|
| `ReindexRequested` (`target_type = "content"`) | сервисы/админ-операции реиндекса | `rustok-index::ContentIndexer` | `target_type` | Повтор должен инициировать одинаковый reindex pipeline без дублирования состояния. |
| `ReindexRequested` (`target_type = "product"`) | сервисы/админ-операции реиндекса | `rustok-index::ProductIndexer` | `target_type` | Повтор допустим; массовая/точечная перестройка должна быть идемпотентной. |
| `IndexUpdated` | индексаторы/системные сервисы (по необходимости) | наблюдатели/метрики/аудит (если подключены) | `index_name`, `target_id` | Повтор не должен создавать конфликтующие audit-следы; допускается dedup по `event_id`. |

## Поведение retry и отказоустойчивость

- **Outbox relay retry**: экспоненциальный backoff до `max_attempts`, затем статус `Failed`.
- **Dispatcher retry**: `retry_count + 1` попыток на каждый handler.
- **Consumer runtime telemetry**: деградация delivery-path отслеживается минимум тремя сигналами: `rustok_event_consumer_lagged_total`, `rustok_event_consumer_restarted_total`, `rustok_event_dispatch_latency_ms`.
- **External consumer policy**: loops поверх внешнего runtime/pubsub не должны завершаться после первой ошибки подключения; для них обязателен supervised resubscribe с явным backoff и наблюдаемым restart signal.
- **Readiness policy**: для внешних consumers должен существовать хотя бы один текущий state signal (`healthy/degraded/...`) в `/health/ready` или эквивалентном health endpoint, а не только исторические counters.
- **Требование к consumer-логике**: использовать идемпотентные операции (`upsert`, `delete-if-exists`, пересчет по source-of-truth), чтобы безопасно переживать at-least-once delivery.

- Для URL-policy сценариев (`topic ↔ post` conversion, slug collision resolution, locale-specific slug migration) смена canonical URL обязана сопровождаться парой событий: `CanonicalUrlChanged` (reindex новой canonical-цели) и `UrlAliasPurged` (deindex/purge устаревших URL alias).
- Redirect/canonical-tag policy и event-flow должны ссылаться на один canonical mapping; при рассинхронизации routing считается деградировавшим до устранения инцидента.


## Какие модули обязаны ссылаться на этот контракт

Минимально — все модули, которые публикуют/обрабатывают `DomainEvent` или обеспечивают доставку:

- publishers: `rustok-content`, `rustok-commerce`, `rustok-pages`, `rustok-forum`, `rustok-blog`, `apps/server` (для `BuildRequested` и orchestration-сценариев);
- consumers/read-model: `rustok-index`;
- transport/runtime: `rustok-outbox`, `rustok-core` (контракты `DomainEvent`/dispatcher).

Если модуль не публикует и не потребляет события, ссылка опциональна.

Если создаётся **новый модуль**, это требование должно быть проверено в PR: если модуль публикует/потребляет `DomainEvent`, то в его `docs/README.md` обязательно добавляется секция `Event contracts` со ссылкой на этот документ.

## PR-чеклист для изменений в событиях

Если в PR добавлен новый `DomainEvent` или изменен контракт существующего:

- [ ] Добавлена **consumer-ветка** (handler/indexer/реакция) для нового события.
- [ ] Добавлен тест цепочки **event created → handler executed → projection/index updated** (или эквивалент `publish → outbox → delivery → consumer/read-model` для transactional flow).
- [ ] Для каждого нового `DomainEvent` добавлены минимум два интеграционных теста: happy-path и repeat/idempotency.
- [ ] Имена интеграционных тестов отражают цепочку события (например, `test_product_created_event_updates_index_projection`).
- [ ] Применено переходное правило legacy event-flow: исторические цепочки покрываются поэтапно, но при любом изменении producer/consumer, routing, outbox/delivery или projection/index в legacy-сценарии интеграционные тесты обновляются в этом же PR.
- [ ] Если меняется long-lived consumer loop, обновлены telemetry/runbook-артефакты (`rustok_event_consumer_*`, `rustok_event_dispatch_latency_ms`, incident runbook для reindex).
- [ ] Обновлены документы: этот файл + docs/модуля-публикатора + docs/модуля-consumer.
