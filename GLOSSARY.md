# Glossary

Глоссарий терминов и понятий, используемых в rustok-revisions.

## A

### Async/Await
**Async/Await** — паттерн асинхронного программирования в Rust, позволяющий писать неблокирующий код, который выглядит как синхронный. Используется в rustok-revisions для эффективной обработки I/O операций.

**Пример:**
```rust
let revision = service.create_revision::<Post>(&post, "post-123", ...).await?;
```

### Audit Log
**Audit Log** — неизменяемый журнал всех операций с ревизиями, используемый для compliance и security. Содержит информацию о том, кто, когда и что сделал с ревизиями.

### Authentication
**Authentication** — процесс проверки подлинности пользователя или системы. В rustok-revisions используется JWT (JSON Web Tokens) для аутентификации.

### Authorization
**Authorization** — процесс проверки прав доступа пользователя к ресурсам. В rustok-revisions используется RBAC (Role-Based Access Control).

## B

### Backend
**Backend** — компонент системы, отвечающий за хранение данных. В rustok-revisions backend абстрагирован через trait `RevisionBackend`, что позволяет использовать разные базы данных.

**Примеры backends:**
- `SeaORMBackend` — для PostgreSQL
- `InMemoryBackend` — для тестов

### Batch Operations
**Batch Operations** — операции, выполняемые над группой объектов за один раз, что улучшает производительность за счет уменьшения количества запросов к базе данных.

**Пример:**
```rust
let revision_ids = service.batch_create_revisions::<Post>(items).await?;
```

## C

### Change Source
**Change Source** — источник изменений, указывающий откуда пришли изменения (например, "web", "api", "import", "migration").

### Change Summary
**Change Summary** — текстовое описание изменений, объясняющее что было изменено и почему.

**Пример:**
```rust
service.create_revision::<Post>(
    &post,
    "post-123",
    None,
    Some("user-456"),
    Some("web"),
    Some("Fixed typo in introduction paragraph"),  // Change summary
).await?;
```

### Content ID
**Content ID** — уникальный идентификатор контента в рамках определенного типа. Например, "post-123" для blog post.

### Content Type
**Content Type** — тип контента, который версионируется (например, "blog_post", "comment", "product").

### Cursor-Based Pagination
**Cursor-Based Pagination** — метод пагинации, использующий курсоры (обычно ID последней записи) вместо offset, что обеспечивает стабильную пагинацию при изменении данных.

**Пример:**
```rust
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),
    None,           // before
    Some("cursor"), // after
    Some(SortOrder::Descending),
).await?;
```

## D

### Delta
**Delta** — различие между двумя версиями объекта. В rustok-revisions delta хранится в JSON формате и содержит только измененные поля.

**Пример:**
```json
{
  "title": {
    "old": "Old Title",
    "new": "New Title"
  },
  "content": {
    "old": "Old content",
    "new": "New content"
  }
}
```

### Delta-Based Storage
**Delta-Based Storage** — подход к хранению истории изменений, при котором хранятся только изменения (deltas) между ревизиями, а не полные копии объектов. Экономит до 95% места по сравнению с full snapshot storage.

### Derive Macro
**Derive Macro** — процедурный макрос Rust, автоматически генерирующий реализацию traits. В rustok-revisions используется `#[derive(Revisionable)]` для автоматической генерации кода.

**Пример:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
}
```

## E

### Encryption at Rest
**Encryption at Rest** — шифрование данных при хранении на диске. Используется для защиты чувствительных данных в ревизиях.

### Encryption in Transit
**Encryption in Transit** — шифрование данных при передаче по сети. В rustok-revisions используется TLS для защиты API запросов.

## F

### Field Change
**Field Change** — изменение одного поля объекта, содержащее старое и новое значения.

**Пример:**
```rust
FieldChange {
    field: "title".to_string(),
    old_value: Some(json!("Old Title")),
    new_value: Some(json!("New Title")),
}
```

### Full Snapshot Storage
**Full Snapshot Storage** — подход к хранению истории изменений, при котором каждая ревизия содержит полную копию объекта. Использует больше места, но проще для чтения.

## G

### GDPR
**GDPR (General Data Protection Regulation)** — европейский регламент о защите персональных данных. rustok-revisions поддерживает GDPR через:
- Право на доступ (export user data)
- Право на удаление (delete user data)
- Data retention policies

## H

### Health Check
**Health Check** — endpoint для проверки работоспособности системы. Возвращает статус базы данных, кэша и других компонентов.

**Пример:**
```rust
GET /health
{
  "status": "healthy",
  "database": true,
  "cache": true,
  "uptime": 3600
}
```

### Horizontal Scaling
**Horizontal Scaling** — масштабирование системы путем добавления дополнительных серверов. В rustok-revisions поддерживается через load balancer и read replicas.

## I

### Idempotent Migration
**Idempotent Migration** — миграция базы данных, которая может быть применена многократно без побочных эффектов. Проверяет существование объектов перед созданием.

### Ignored Fields
**Ignored Fields** — поля объекта, которые не отслеживаются в ревизиях. Обычно это технические поля (updated_at, view_count).

**Пример:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}
```

## J

### JSON
**JSON (JavaScript Object Notation)** — текстовый формат обмена данными. Используется в rustok-revisions для хранения deltas и контента ревизий.

### JWT
**JWT (JSON Web Token)** — стандарт для создания токенов доступа. Используется в rustok-revisions для аутентификации пользователей.

## L

### Locale
**Locale** — локаль контента (например, "en", "ru", "de"). Позволяет версионировать контент на разных языках независимо.

**Пример:**
```rust
service.create_revision::<Post>(&post, "post-123", Some("en"), ...).await?;
service.create_revision::<Post>(&post, "post-123", Some("ru"), ...).await?;
```

## M

### Multi-Tenancy
**Multi-Tenancy** — архитектура, при которой одна инсталляция системы обслуживает несколько независимых клиентов (tenants). В rustok-revisions каждый tenant имеет свою изолированную историю ревизий.

## N

### Named Version
**Named Version** — именованная версия (snapshot) важной ревизии. Используется для отметки значимых моментов (например, "v1.0-published", "before-major-refactor").

**Пример:**
```rust
service.create_named_version::<Post>(
    "post-123",
    Some("en"),
    "v1.0-published",
    Some("First published version"),
    Some("user-456"),
).await?;
```

Named versions защищены от удаления retention policy.

## P

### Partitioning
**Partitioning** — разделение большой таблицы на меньшие части (partitions) для улучшения производительности и управляемости. В PostgreSQL можно partitioning по дате или типу контента.

**Пример:**
```sql
CREATE TABLE content_revisions (
    ...
) PARTITION BY RANGE (created_at);

CREATE TABLE content_revisions_2024_q1 
  PARTITION OF content_revisions
  FOR VALUES FROM ('2024-01-01') TO ('2024-04-01');
```

### Point-in-Time Recovery
**Point-in-Time Recovery** — возможность восстановить состояние системы на определенный момент времени. В rustok-revisions поддерживается через `get_revision_at_time()`.

**Пример:**
```rust
let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
let revision = service.get_revision_at_time::<Post>("post-123", Some("en"), timestamp).await?;
```

### PostgreSQL
**PostgreSQL** — объектно-реляционная система управления базами данных. Основная база данных, поддерживаемая rustok-revisions.

## R

### Rate Limiting
**Rate Limiting** — ограничение количества запросов от пользователя или системы за определенный период времени. Используется для защиты от DDoS атак и злоупотреблений.

### RBAC
**RBAC (Role-Based Access Control)** — модель управления доступом на основе ролей. В rustok-revisions используются роли: Admin, Editor, Viewer.

### Read Replica
**Read Replica** — копия базы данных, используемая только для чтения. Позволяет распределить нагрузку между primary (write) и replicas (read).

### Retention Policy
**Retention Policy** — политика хранения ревизий, определяющая как долго и сколько ревизий хранить.

**Типы:**
- `KeepLast { count }` — хранить последние N ревизий
- `KeepDays { days }` — хранить ревизии за последние N дней
- `KeepNamedOnly` — хранить только named versions

**Пример:**
```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

### Revision
**Revision** — версия контента в определенный момент времени. Содержит delta (изменения), метаданные и информацию о создателе.

### Revision Number
**Revision Number** — последовательный номер ревизии для конкретного контента. Начинается с 1 и увеличивается с каждым изменением.

### Revisionable
**Revisionable** — trait для типов, которые могут быть версионированы. Определяет метод `to_revision_value()` для конвертации объекта в JSON.

**Пример:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
}
```

### RevisionBackend
**RevisionBackend** — trait для backend реализаций, определяющий методы для создания, получения, удаления ревизий.

### RevisionDiff
**Revision Diff** — различия между двумя ревизиями, показывающие какие поля изменились и как.

**Пример:**
```rust
let diff = service.compare_revisions::<Post>("post-123", Some("en"), 3, 5).await?;
// diff.changes содержит список FieldChange
```

### RevisionFilter
**Revision Filter** — структура для фильтрации ревизий по различным критериям (tenant, content_type, date range, etc.).

### RevisionService
**Revision Service** — основной сервис для работы с ревизиями. Предоставляет методы для создания, получения, сравнения и восстановления ревизий.

### RevisionTracker
**Revision Tracker** — helper для группировки связанных изменений. Позволяет установить user_id, source и summary один раз и использовать для нескольких ревизий.

**Пример:**
```rust
let tracker = RevisionTracker::new("user-456")
    .with_source("bulk-update")
    .with_summary("Updated all posts");

for post in posts {
    service.create_revision_with_tracker::<Post>(&post, &post.id, None, &tracker).await?;
}
```

## S

### SeaORM
**SeaORM** — async ORM для Rust, используемый в rustok-revisions для работы с PostgreSQL. Предоставляет type-safe queries и code generation.

### Sharding
**Sharding** — разделение данных между несколькими базами данных по определенному критерию (например, по tenant_id). Используется для горизонтального масштабирования.

### Snapshot
**Snapshot** — полная копия объекта в определенный момент времени. В rustok-revisions snapshots создаются автоматически при использовании delta-based storage для оптимизации производительности.

### SOC 2
**SOC 2 (Service Organization Control 2)** — стандарт аудита для сервисных организаций, фокусирующийся на security, availability, processing integrity, confidentiality и privacy.

### Sort Order
**Sort Order** — порядок сортировки результатов (Ascending или Descending).

**Пример:**
```rust
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),
    None,
    None,
    Some(SortOrder::Descending),  // Новые сначала
).await?;
```

### Streaming
**Streaming** — обработка данных потоком, а не загрузка всех данных в память сразу. Используется для работы с большими объемами данных.

**Пример:**
```rust
let mut stream = service.stream_revisions::<Post>("post-123", Some("en")).await?;
while let Some(revision) = stream.next().await {
    process_revision(revision?);
}
```

## T

### Tenant
**Tenant** — клиент или организация в multi-tenant системе. Каждый tenant имеет свою изолированную историю ревизий.

### Tenant Isolation
**Tenant Isolation** — обеспечение изоляции данных между разными tenants. В rustok-revisions каждый запрос фильтруется по tenant_id.

### TLS
**TLS (Transport Layer Security)** — криптографический протокол для защиты данных при передаче по сети. Используется для HTTPS connections.

### Tracked Fields
**Tracked Fields** — поля объекта, которые отслеживаются в ревизиях. Изменения в этих полях создают новые ревизии.

**Пример:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    content: String,
}
```

## V

### Version Control
**Version Control** — система управления изменениями. В контексте rustok-revisions — управление изменениями структурированных данных (JSON объектов).

### Vertical Scaling
**Vertical Scaling** — масштабирование системы путем увеличения ресурсов одного сервера (CPU, RAM, storage).

## Заключение

Этот глоссарий покрывает:

✅ **Основные понятия** — Revision, Delta, Backend  
✅ **Технические термины** — Async/Await, JWT, TLS  
✅ **Архитектурные концепции** — Multi-Tenancy, Sharding, Partitioning  
✅ **Функции библиотеки** — Revisionable, RevisionService, RevisionTracker  
✅ **Best practices** — Retention Policy, Rate Limiting, RBAC  
✅ **Compliance** — GDPR, SOC 2, Audit Log  

Используйте этот глоссарий для:

- Понимания терминологии
- Изучения концепций
- Быстрого поиска определений
- Обучения новых разработчиков

**Удачи в изучении rustok-revisions!** 📖
