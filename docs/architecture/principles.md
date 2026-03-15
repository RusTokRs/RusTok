# RusToK — Architecture Guide

> **Дата:** 2026-02-06  
> **Статус:** Актуальный. Дополняется при изменениях архитектурных принципов платформы.

---

## 1. Core Tenets

### 1.1 CQRS-lite: Write vs Read

```text
WRITE: API → Service → SeaORM → PostgreSQL → EventBus
READ:  User → Index Tables (denormalized) → Search Results
```

| Write Model | Read Model |
|-------------|------------|
| Строгие 3NF таблицы | Денормализованные index-таблицы |
| Транзакции, валидация | GIN-индексы, быстрый поиск |
| Источник истины | Проекции для storefront |

**Ключ:** "No Heavy JOINs on Storefront" — данные склеиваются в indexer при записи.

### 1.2 Evolvable Event Transport (L0 → L1 → L2)

| Уровень | Транспорт | Когда |
|---------|-----------|-------|
| L0 | In-memory broadcast (mpsc) | Dev/MVP |
| L1 | Outbox Pattern | Надёжность доставки |
| L2 | Iggy Streaming | Highload, replay |

**Принцип:** Единый trait `EventTransport` — меняем уровень без переписывания логики.

### 1.3 Modular Monolith

- Модули изолированы и общаются через события
- Нет прямых импортов доменных зависимостей между модулями
- Интеграция только через `EventBus` и `Service Layer`

---

## 2. Module Taxonomy

| Тип | Описание | Примеры |
|-----|----------|---------|
| **Core Components** | Инфраструктура, базовые кирпичики | `rustok-core`, `rustok-tenant` |
| **Domain Modules** | Полноценные бизнес-вертикали | `rustok-commerce`, `rustok-content` |
| **Wrapper Modules** | Надстройки без своих таблиц | `rustok-blog`, `rustok-forum` |
| **Infrastructure** | Технические модули | `rustok-index`, `rustok-outbox` |

### Wrapper Module Pattern

```text
rustok-blog  →  использует nodes с kind='post'  →  rustok-content
rustok-forum →  использует nodes с kind='topic' →  rustok-content
```

**Специализация без дублирования таблиц.**

---

## 3. Flex Module (Generic Content Builder)

> ⚠️ **Новый концепт из архитектурного обсуждения**

### 3.1 Определение

**Flex** — опциональный вспомогательный модуль-конструктор данных для ситуаций, когда стандартных модулей недостаточно.

Позволяет:

- Определять динамические схемы (runtime) с типами и валидацией
- Хранить записи в JSONB
- Прикреплять данные к сущностям других модулей (attached behavior)
- Интегрироваться через события и индексатор

### 3.2 Правила (HARD LAWS)

| # | Правило |
|---|---------|
| 1 | **Standard modules NEVER depend on Flex** |
| 2 | Flex depends only on `rustok-core` (events/tenant/rbac) |
| 3 | **Removal-safe:** отключение Flex не ломает платформу |
| 4 | Read model через индексатор, не через runtime JOIN |

### 3.3 Guardrails (обязательны)

- ⬜ TODO: Строгая валидация JSONB по schema на запись
- ⬜ TODO: Лимит на число полей (например 50)
- ⬜ TODO: Лимит глубины вложенности/связей
- ⬜ TODO: Обязательная пагинация
- ⬜ TODO: Запрет в критических доменах (orders/payments/inventory)

### 3.4 Когда использовать

✅ **Используем:**

- Кастомные поля/структуры для edge-cases
- Маркетинговые лендинги, формы, баннеры
- Справочники с произвольными полями

❌ **НЕ используем:**

- Если кейс закрывается стандартными модулями
- Как основу для товаров/заказов/оплат
- Для критических финансовых данных

### 3.5 Data Model (предварительная)

```sql
-- Определения схем
CREATE TABLE flex_schemas (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL REFERENCES tenants(id),
    slug            VARCHAR(64) NOT NULL,
    name            VARCHAR(255) NOT NULL,
    fields_config   JSONB NOT NULL,  -- описание полей
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, slug)
);

-- Записи данных (attached behavior)
CREATE TABLE flex_entries (
    id              UUID PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    schema_id       UUID NOT NULL REFERENCES flex_schemas(id),
    entity_type     VARCHAR(64),  -- 'product', 'user', etc.
    entity_id       UUID,         -- ID сущности в другом модуле
    data            JSONB NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_flex_entries_data ON flex_entries USING GIN (data);
```

---

## 4. Content ↔ Commerce Strategy

**Решение:** Commerce владеет своими данными (SEO, rich description), Index собирает композитную картину.

| Подход | Описание | Статус |
|--------|----------|--------|
| ~~Product → content.node_id~~ | Явная связь контекстов | ❌ Отклонён |
| **Commerce owns data** | Автономность bounded contexts | ✅ Выбран |

```text
Commerce: SEO fields + rich description (JSONB)
    ↓ events
Index: Композитный read model для storefront
```

- ⬜ TODO: Зафиксировать как canonical approach в манифесте

---

## 5. RBAC Strategy

### 5.1 Minimum Viable Model

```rust
// Каждый модуль объявляет свои permissions
impl RusToKModule for CommerceModule {
    fn permissions(&self) -> &[Permission] {
        &[
            Permission::new("commerce.products.read"),
            Permission::new("commerce.products.write"),
            Permission::new("commerce.orders.read"),
            Permission::new("commerce.orders.manage"),
        ]
    }
}
```

### 5.2 Naming Convention

```
<module>.<entity>.<action>
```

Примеры: `content.nodes.write`, `blog.posts.publish`, `commerce.orders.refund`

### 5.3 TODO

- ⬜ TODO: Определить модель (RBAC vs ABAC vs гибрид)
- ⬜ TODO: Tenant-изоляция и наследование ролей
- ⬜ TODO: Добавить `fn permissions()` в `RusToKModule` trait
- ⬜ TODO: Единый enforcement layer для API и Admin

---

## 6. Migrations Convention

### 6.1 Naming Format

```
mYYYYMMDD_<module>_<nnn>_<description>.rs
```

Примеры:

- `m20250201_content_001_create_nodes.rs`
- `m20250201_commerce_001_create_products.rs`
- `m20250201_commerce_002_create_variants.rs`

### 6.2 Правила

- ⬜ TODO: Префикс модуля исключает коллизии
- ⬜ TODO: Одна миграция — одна цель
- ⬜ TODO: При параллельной работе — координация через module prefix

---

## 7. Observability & Health

### 7.1 Endpoints

| Endpoint | Назначение |
|----------|------------|
| `/health/live` | Liveness probe (K8s) |
| `/health/ready` | Readiness probe |
| `/health/modules` | Агрегированный health по модулям |

### 7.2 Status / TODO

- ✅ Реализованы `/health/live`, `/health/ready`, `/health/modules`.
- ✅ Реализована агрегация readiness по критичным/некритичным зависимостям и модулям.
- ✅ Readiness-ответ унифицирован: `status`, `checks`, `modules`, `degraded_reasons`, latency и reason per-check.
- ⬜ TODO: Structured logging + correlation IDs
- ⬜ TODO: Prometheus metrics endpoint

---

## 8. API Versioning

| API | Стратегия |
|-----|-----------|
| REST | URL-based: `/api/v1/products` |
| GraphQL | Schema evolution: deprecated fields + новые поля |

- ⬜ TODO: Зафиксировать до первого production deployment

---

## 9. Alloy Scripting (скрытая сложность)

> ⚠️ **Требует особого внимания**

### 9.1 Ограничения

| Аспект | Правило |
|--------|---------|
| Где выполняется | **Только async:** background workers, event handlers |
| Request path | ❌ Запрещено (узкое место) |
| Лимиты | Timeout, memory, CPU |
| Sandbox | Ограниченный доступ к системе |

- ⬜ TODO: Формализовать sandboxing и limits
- ⬜ TODO: Документировать "Alloy is async-only by default"

---

## 10. Testing Strategy

### 10.1 Уровни тестирования

| Уровень | Назначение |
|---------|------------|
| Unit | Изолированная логика модулей |
| Integration | Cross-module flows через events |
| E2E | Полный путь tenant → module → entity → index |

### 10.2 Test Utilities (TODO)

- ⬜ TODO: Mock `EventTransport` в `rustok-core`
- ⬜ TODO: In-memory database setup
- ⬜ TODO: Test tenant factory

---

## См. также

- [ROADMAP.md](../ROADMAP.md) — фазы разработки
- [architecture/flex.md](./flex.md) — спецификация и план реализации Flex
- [MANIFEST_ADDENDUM.md](../MANIFEST_ADDENDUM.md) — дополнения к манифесту

