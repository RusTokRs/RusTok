# План устранения недостатков `rustok-product`

**Репозиторий:** `RusTokRs/RusTok`  
**Область:** `crates/rustok-product` и связанные product GraphQL/migrations  
**Проверено на:** `main`, commit `a1032998adfbccacc1679e11a67edcd9fc04c109`

## Обозначения

- **Приоритет:** Critical / High / Medium / Low
- **Трудоёмкость:** S / M / L / XL
- **⚠ Требуется перепроверка:** вывод нельзя считать окончательно подтверждённым без проверки runtime, данных или внешних потребителей.

---

## 1. Архитектура

- [ ] Перенести product DTO, entities и ошибки из `rustok-commerce-foundation` в `rustok-product` либо отдельный `rustok-product-contracts`. В foundation временно оставить совместимые re-export.  
  **Приоритет:** High · **Трудоёмкость:** L

- [ ] Разделить `CatalogService` на отдельные компоненты:
  - product commands;
  - product queries;
  - inventory integration;
  - tags;
  - projection builder.  
  **Приоритет:** High · **Трудоёмкость:** XL

- [ ] Разделить `ProductCatalogSchemaService` на сервисы:
  - attributes;
  - schemas;
  - categories;
  - values;
  - virtual categories.  
  **Приоритет:** High · **Трудоёмкость:** XL

- [ ] Оставить одного владельца product-миграций. Удалить или отключить копии миграций из `rustok-commerce`.  
  **Приоритет:** High · **Трудоёмкость:** M  
  **⚠ Требуется перепроверка:** зарегистрированы ли одновременно оба migration source.

- [ ] Зафиксировать поддержку только PostgreSQL либо реализовать product-миграции для остальных backend. При PostgreSQL-only завершать запуск ошибкой, а не успешно пропускать миграции.  
  **Приоритет:** High · **Трудоёмкость:** M  
  **⚠ Требуется перепроверка:** заявлена ли поддержка SQLite/MySQL.

- [ ] Перенести product GraphQL surface владельцу `rustok-product` либо заменить проверку `commerce` module slug на `product` module slug.  
  **Приоритет:** High · **Трудоёмкость:** M

---

## 2. База данных и таблицы

- [ ] Перенести создание `product_status_enum` и изменение `products.status` из миграций приложения в миграции `rustok-product`. Модуль должен разворачиваться независимо от `apps/server`.  
  **Приоритет:** Critical · **Трудоёмкость:** M

- [ ] Добавить `tenant_id` в `product_translations`, выполнить backfill и создать уникальное ограничение `(tenant_id, locale, handle)`. Заменить глобальную проверку handle на tenant-scoped.  
  **Приоритет:** Critical · **Трудоёмкость:** L

- [ ] Создать частичный уникальный индекс:
  ```sql
  CREATE UNIQUE INDEX ... ON product_variants (tenant_id, sku)
  WHERE sku IS NOT NULL;
  ```
  Ошибку `unique_violation` преобразовывать в `DuplicateSku`.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Исправить уникальность корневых категорий:
  - отдельный unique index `(tenant_id, slug) WHERE parent_id IS NULL`;
  - для дочерних оставить `(tenant_id, parent_id, slug)`.  
  **Приоритет:** High · **Трудоёмкость:** S

- [ ] Добавить ограничения для EAV-значений:
  - запрет нескольких одновременно заполненных `value_*`;
  - проверку допустимого типа значения;
  - правила для `detached_at`;
  - валидацию option values для `select/multiselect`.  
  **Приоритет:** High · **Трудоёмкость:** L

- [ ] Устранить два источника primary category:
  - `products.primary_category_id`;
  - `product_categories.assignment_kind = 'primary'`.  
  Оставить один источник либо обеспечить транзакционную синхронизацию и partial unique index на одну primary-категорию продукта.  
  **Приоритет:** High · **Трудоёмкость:** M  
  **⚠ Требуется перепроверка:** какой источник считается каноническим.

- [ ] Добавить индекс для storefront-выборки:
  ```sql
  (tenant_id, status, published_at DESC, created_at DESC)
  ```
  **Приоритет:** High · **Трудоёмкость:** S

- [ ] Провести миграцию очистки переходных колонок в:
  - `products`;
  - `product_options`;
  - `product_images`;
  - translations;
  - variants.  
  Перед удалением проверить внешних потребителей.  
  **Приоритет:** Medium · **Трудоёмкость:** L  
  **⚠ Требуется перепроверка.**

- [ ] Мигрировать и удалить устаревшие поля `product_variants`:
  - `manage_inventory`;
  - `allow_backorder`;
  - `variant_rank`.  
  Предлагаемое соответствие:
  - `manage_inventory` → `inventory_management`;
  - `allow_backorder` → `inventory_policy`;
  - `variant_rank` → `position`.  
  **Приоритет:** High · **Трудоёмкость:** M  
  **⚠ Требуется перепроверка:** точная семантика преобразования данных и наличие внешних потребителей.

- [ ] Обеспечить tenant-целостность `product_tags`:
  - проверить и backfill `tenant_id`;
  - добавить `UNIQUE (tenant_id, id)` для `taxonomy_terms`;
  - добавить composite FK `(tenant_id, product_id) → products(tenant_id, id)`;
  - добавить composite FK `(tenant_id, term_id) → taxonomy_terms(tenant_id, id)`;
  - добавить индекс `(tenant_id, product_id)`.  
  **Приоритет:** Critical · **Трудоёмкость:** M

- [ ] Добавить автоматический schema-тест, проверяющий composite FK для всех таблиц с `tenant_id`.  
  **Приоритет:** High · **Трудоёмкость:** M

---

## 3. Код и ORM

- [ ] Добавить в `product_tag::Relation` связь `Term` с `rustok_taxonomy::entities::taxonomy_term::Entity` и реализацию `Related`.  
  **Приоритет:** Low · **Трудоёмкость:** S

- [ ] Удалить проверки уникальности по схеме `SELECT → INSERT` после добавления DB constraints. Обрабатывать конфликт вставки, исключив race condition.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Пакетно вставлять:
  - translations;
  - options;
  - option values;
  - variants;
  - prices.  
  Убрать последовательные `INSERT` в циклах там, где допустим bulk insert.  
  **Приоритет:** Medium · **Трудоёмкость:** M

- [ ] Выделить единый transaction helper для записи сущности и outbox-события.  
  **Приоритет:** Medium · **Трудоёмкость:** M

- [ ] Заменить `expect` при регистрации SEO provider на контролируемую ошибку инициализации модуля.  
  **Приоритет:** Medium · **Трудоёмкость:** S

---

## 4. API и доступ

- [ ] Получать `tenant_id` и actor/user ID только из доверенных `AuthContext` / `TenantContext`.  
  Не принимать `userId` как доверенный аргумент GraphQL mutation.  
  Проверять, что запрошенный tenant совпадает с tenant из auth context.  
  **Приоритет:** Critical · **Трудоёмкость:** M

- [ ] Сохранить существующие RBAC-проверки для product mutations и дополнить их tenant-binding проверкой.  
  **Приоритет:** Critical · **Трудоёмкость:** S

- [ ] Не возвращать текст внутренних ошибок БД через `PortError` и GraphQL `err.to_string()`.  
  Ввести единый mapper:
  - стабильный публичный error code;
  - безопасное клиентское сообщение;
  - полная внутренняя ошибка только в логах;
  - correlation ID.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Добавить явное отображение всех `CommerceError` в стабильные API-коды вместо общего `invariant_violation`.  
  **Приоритет:** Medium · **Трудоёмкость:** S

- [ ] Использовать одинаковую валидацию пагинации во всех точках входа. Убрать молчаливые `max/clamp` внутри сервиса.  
  **Приоритет:** Medium · **Трудоёмкость:** S

---

## 5. Производительность

- [ ] Перенести фильтрацию channel visibility, `COUNT`, `OFFSET/LIMIT` в SQL. Не загружать все опубликованные товары tenant в память перед пагинацией.  
  **Приоритет:** Critical · **Трудоёмкость:** L

- [ ] Нормализовать channel visibility в отдельную таблицу либо добавить поддерживаемый JSONB-предикат и соответствующий индекс.  
  **Приоритет:** High · **Трудоёмкость:** L

- [ ] Сократить количество последовательных запросов в `get_product_with_locale_fallback`:
  - объединить загрузку projections;
  - либо выполнять независимые запросы параллельно;
  - использовать согласованную read-транзакцию при необходимости snapshot consistency.  
  **Приоритет:** High · **Трудоёмкость:** L

- [ ] Проверить новые запросы через:
  ```sql
  EXPLAIN (ANALYZE, BUFFERS)
  ```
  на каталогах 10k, 100k и 1M товаров.  
  **Приоритет:** High · **Трудоёмкость:** M

---

## 6. Тестирование

- [ ] Добавить PostgreSQL integration tests для полного цикла `up/down/up` всех product-миграций.  
  **Приоритет:** Critical · **Трудоёмкость:** L

- [ ] Выполнить реальные persistence-backed тесты:
  - `read_product_projection`;
  - `list_published_products`.  
  **Приоритет:** Critical · **Трудоёмкость:** L

- [ ] Добавить конкурентные тесты создания одинаковых handle и SKU.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Добавить tenant-isolation тесты для:
  - products;
  - categories;
  - schemas;
  - attributes;
  - values;
  - options;
  - variants;
  - translations;
  - tags.  
  **Приоритет:** Critical · **Трудоёмкость:** L

- [ ] Добавить отдельный тест, запрещающий создание `product_tags` с несовпадающими tenant товара, taxonomy term и самой записи.  
  **Приоритет:** Critical · **Трудоёмкость:** S

- [ ] Добавить тесты:
  - повреждённых EAV-значений;
  - циклов категорий;
  - рассинхронизации closure/path;
  - нескольких primary category;
  - duplicate root slug.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Добавить migration tests для существующих дубликатов перед установкой unique constraints.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Добавить migration test для переноса старых inventory-флагов и проверки их удаления.  
  **Приоритет:** High · **Трудоёмкость:** M

- [ ] Проверять parity native/GraphQL для admin и storefront.  
  **Приоритет:** Medium · **Трудоёмкость:** L

---

## 7. Безопасность

- [ ] Закрыть возможность подмены `tenant_id` и `user_id` через GraphQL variables.  
  **Приоритет:** Critical · **Трудоёмкость:** M

- [ ] Закрыть утечку внутренних сообщений БД через API.  
  **Приоритет:** High · **Трудоёмкость:** S

- [ ] Ограничить размер:
  - `metadata`;
  - `validation`;
  - `rule_config`;
  - snapshots;
  - других JSONB-входов.  
  **Приоритет:** Medium · **Трудоёмкость:** M  
  **⚠ Требуется перепроверка:** лимиты transport/body могут применяться выше.

- [ ] Добавить негативные тесты подмены tenant во всех write/read сценариях.  
  **Приоритет:** Critical · **Трудоёмкость:** M

---

## 8. Документация

- [ ] Создать ER-диаграмму product-схемы с:
  - PK;
  - FK;
  - composite FK;
  - unique constraints;
  - partial indexes;
  - обычными индексами.  
  **Приоритет:** Medium · **Трудоёмкость:** M

- [ ] Описать владельца каждой таблицы и канонические источники данных:
  - category;
  - tags;
  - inventory;
  - prices;
  - media;
  - shipping profile.  
  **Приоритет:** Medium · **Трудоёмкость:** M

- [ ] Добавить ADR:
  - PostgreSQL-only;
  - tenant isolation;
  - EAV-модель;
  - closure table;
  - ownership product/commerce.  
  **Приоритет:** Medium · **Трудоёмкость:** S

- [ ] После появления live-тестов обновить статус `boundary_ready/transport_verified`.  
  **Приоритет:** Low · **Трудоёмкость:** S

---

## Рекомендуемый порядок выполнения

1. Сделать snapshot БД и аудит существующих данных:
   - duplicate handles;
   - duplicate SKU;
   - duplicate root slug;
   - cross-tenant tags;
   - несколько primary category;
   - конфликт старых и новых inventory-полей.

2. Закрыть возможность подмены `tenant_id` и `user_id` в API.

3. Перенести enum-миграцию в `rustok-product`.

4. Исправить tenant-целостность `product_tags`.

5. Выполнить backfill `tenant_id` и tenant-ограничений для translations и остальных product-таблиц.

6. Добавить unique/check/composite FK constraints и обработку DB conflicts.

7. Мигрировать старые inventory-поля и удалить legacy-колонки.

8. Перенести storefront-фильтрацию и пагинацию в SQL, добавить индексы.

9. Ввести безопасный единый mapper ошибок API.

10. Добавить PostgreSQL integration, concurrency и tenant-isolation tests.

11. Разделить крупные сервисы и перенести product-owned типы.

12. Удалить дублирующиеся миграции и завершить перенос ownership из `commerce`.

13. Зафиксировать архитектуру и схему БД в документации.

---

## Проверенные и исключённые пункты

Следующие замечания из исходного резюме **не являются актуальными проблемами**:

- У `product_translations` уже есть unique index `(product_id, locale)`.
- У `product_images.media_id` уже есть FK на `media.id` с `ON DELETE SET NULL`.
- Для `product_variants` поздняя миграция добавляет composite FK `(tenant_id, product_id) → products(tenant_id, id)`. Отдельный FK `tenant_id → tenants(id)` не обязателен и будет избыточным при корректном composite FK.

---

## Основные файлы для работы

- `crates/rustok-product/src/services/catalog.rs`
- `crates/rustok-product/src/services/catalog_schema_service.rs`
- `crates/rustok-product/src/ports.rs`
- `crates/rustok-product/src/entities/product_tag.rs`
- `crates/rustok-product/src/migrations/`
- `crates/rustok-commerce/src/graphql/mutations/catalog.rs`
- `crates/rustok-commerce/src/graphql/query.rs`
- `crates/rustok-commerce-foundation/src/entities/`
- `apps/server/migration/src/m20250201_000001_alter_status_to_enums.rs`
