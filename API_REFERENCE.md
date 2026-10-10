# API Reference

Полный справочник по API библиотеки rustok-revisions.

## Содержание

1. [Основные типы](#основные-типы)
2. [Traits](#traits)
3. [RevisionService](#revisionservice)
4. [RevisionBackend](#revisionbackend)
5. [RevisionTracker](#revisiontracker)
6. [Error Types](#error-types)
7. [Enums](#enums)
8. [Functions](#functions)
9. [Macros](#macros)

## Основные типы

### Revision

Основная структура, представляющая ревизию контента.

```rust
pub struct Revision {
    pub id: Uuid,
    pub tenant_id: String,
    pub content_type: String,
    pub content_id: String,
    pub locale: Option<String>,
    pub revision_number: i32,
    pub parent_revision_id: Option<Uuid>,
    pub delta: serde_json::Value,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub change_source: Option<String>,
    pub change_summary: Option<String>,
    pub version_name: Option<String>,
}
```

**Поля:**

| Поле | Тип | Описание |
|------|-----|----------|
| `id` | `Uuid` | Уникальный идентификатор ревизии |
| `tenant_id` | `String` | Идентификатор tenant (для multi-tenancy) |
| `content_type` | `String` | Тип контента (например, "blog_post") |
| `content_id` | `String` | Идентификатор контента |
| `locale` | `Option<String>` | Локаль (например, "en", "ru") |
| `revision_number` | `i32` | Номер ревизии (последовательный) |
| `parent_revision_id` | `Option<Uuid>` | ID родительской ревизии |
| `delta` | `serde_json::Value` | Изменения в JSON формате |
| `created_by` | `Option<String>` | ID пользователя, создавшего ревизию |
| `created_at` | `DateTime<Utc>` | Время создания ревизии |
| `change_source` | `Option<String>` | Источник изменений ("web", "api", "import") |
| `change_summary` | `Option<String>` | Описание изменений |
| `version_name` | `Option<String>` | Имя версии (для named versions) |

**Пример:**

```rust
let revision = Revision {
    id: Uuid::new_v4(),
    tenant_id: "tenant-1".to_string(),
    content_type: "blog_post".to_string(),
    content_id: "post-123".to_string(),
    locale: Some("en".to_string()),
    revision_number: 5,
    parent_revision_id: Some(Uuid::parse_str("...").unwrap()),
    delta: json!({
        "title": {"old": "Old Title", "new": "New Title"}
    }),
    created_by: Some("user-456".to_string()),
    created_at: Utc::now(),
    change_source: Some("web".to_string()),
    change_summary: Some("Updated title".to_string()),
    version_name: None,
};
```

---

### RevisionDiff

Структура, представляющая различия между двумя ревизиями.

```rust
pub struct RevisionDiff {
    pub from_revision: i32,
    pub to_revision: i32,
    pub changes: Vec<FieldChange>,
}
```

**Поля:**

| Поле | Тип | Описание |
|------|-----|----------|
| `from_revision` | `i32` | Номер начальной ревизии |
| `to_revision` | `i32` | Номер конечной ревизии |
| `changes` | `Vec<FieldChange>` | Список изменений полей |

**Пример:**

```rust
let diff = RevisionDiff {
    from_revision: 3,
    to_revision: 5,
    changes: vec![
        FieldChange {
            field: "title".to_string(),
            old_value: Some(json!("Old Title")),
            new_value: Some(json!("New Title")),
        },
        FieldChange {
            field: "content".to_string(),
            old_value: Some(json!("Old content")),
            new_value: Some(json!("New content")),
        },
    ],
};
```

---

### FieldChange

Структура, представляющая изменение одного поля.

```rust
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}
```

**Поля:**

| Поле | Тип | Описание |
|------|-----|----------|
| `field` | `String` | Имя поля |
| `old_value` | `Option<Value>` | Старое значение (None для новых полей) |
| `new_value` | `Option<Value>` | Новое значение (None для удаленных полей) |

**Пример:**

```rust
let change = FieldChange {
    field: "title".to_string(),
    old_value: Some(json!("Old Title")),
    new_value: Some(json!("New Title")),
};
```

---

### RevisionFilter

Структура для фильтрации ревизий.

```rust
pub struct RevisionFilter {
    pub tenant_id: Option<String>,
    pub content_type: Option<String>,
    pub content_id: Option<String>,
    pub locale: Option<String>,
    pub created_by: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub has_version_name: Option<bool>,
}
```

**Поля:**

| Поле | Тип | Описание |
|------|-----|----------|
| `tenant_id` | `Option<String>` | Фильтр по tenant |
| `content_type` | `Option<String>` | Фильтр по типу контента |
| `content_id` | `Option<String>` | Фильтр по ID контента |
| `locale` | `Option<String>` | Фильтр по локали |
| `created_by` | `Option<String>` | Фильтр по пользователю |
| `created_after` | `Option<DateTime<Utc>>` | Создано после даты |
| `created_before` | `Option<DateTime<Utc>>` | Создано до даты |
| `has_version_name` | `Option<bool>` | Имеет ли имя версии |

**Пример:**

```rust
let filter = RevisionFilter {
    tenant_id: Some("tenant-1".to_string()),
    content_type: Some("blog_post".to_string()),
    content_id: Some("post-123".to_string()),
    locale: Some("en".to_string()),
    created_after: Some(Utc::now() - Duration::days(30)),
    ..Default::default()
};
```

---

### RetentionPolicy

Enum для политик хранения ревизий.

```rust
pub enum RetentionPolicy {
    KeepLast { count: usize },
    KeepDays { days: u32 },
    KeepNamedOnly,
    Custom(Box<dyn RetentionStrategy>),
}
```

**Варианты:**

| Вариант | Описание |
|---------|----------|
| `KeepLast { count }` | Хранить последние N ревизий |
| `KeepDays { days }` | Хранить ревизии за последние N дней |
| `KeepNamedOnly` | Хранить только named versions |
| `Custom(strategy)` | Пользовательская стратегия |

**Пример:**

```rust
// Хранить последние 100 ревизий
let policy = RetentionPolicy::KeepLast { count: 100 };

// Хранить ревизии за последние 90 дней
let policy = RetentionPolicy::KeepDays { days: 90 };

// Хранить только named versions
let policy = RetentionPolicy::KeepNamedOnly;
```

## Traits

### Revisionable

Trait для типов, которые могут быть версионированы.

```rust
pub trait Revisionable: Send + Sync {
    fn to_revision_value(&self) -> Result<serde_json::Value, RevisionError>;
}
```

**Методы:**

#### to_revision_value

Конвертирует объект в JSON Value для хранения в ревизии.

```rust
fn to_revision_value(&self) -> Result<serde_json::Value, RevisionError>;
```

**Возвращает:**
- `Ok(Value)` — JSON представление объекта
- `Err(RevisionError)` — ошибка конвертации

**Пример:**

```rust
impl Revisionable for Post {
    fn to_revision_value(&self) -> Result<serde_json::Value, RevisionError> {
        let mut map = serde_json::Map::new();
        map.insert("title".to_string(), json!(self.title));
        map.insert("content".to_string(), json!(self.content));
        Ok(serde_json::Value::Object(map))
    }
}
```

**Использование с derive macro:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    content: String,
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}
```

---

### ContentRevisionConfig

Trait для конфигурации версионирования типа.

```rust
pub trait ContentRevisionConfig {
    fn content_type() -> &'static str;
    fn tracked_fields() -> &'static [&'static str];
    fn ignored_fields() -> &'static [&'static str];
    fn default_retention_policy() -> Option<RetentionPolicy>;
}
```

**Методы:**

#### content_type

Возвращает тип контента.

```rust
fn content_type() -> &'static str;
```

**Пример:**

```rust
impl ContentRevisionConfig for Post {
    fn content_type() -> &'static str {
        "blog_post"
    }
}
```

#### tracked_fields

Возвращает список отслеживаемых полей.

```rust
fn tracked_fields() -> &'static [&'static str];
```

**Пример:**

```rust
impl ContentRevisionConfig for Post {
    fn tracked_fields() -> &'static [&'static str] {
        &["title", "content", "status"]
    }
}
```

#### ignored_fields

Возвращает список игнорируемых полей.

```rust
fn ignored_fields() -> &'static [&'static str];
```

**Пример:**

```rust
impl ContentRevisionConfig for Post {
    fn ignored_fields() -> &'static [&'static str] {
        &["updated_at", "view_count"]
    }
}
```

#### default_retention_policy

Возвращает политику хранения по умолчанию.

```rust
fn default_retention_policy() -> Option<RetentionPolicy>;
```

**Пример:**

```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

---

### RevisionBackend

Trait для backend реализаций.

```rust
#[async_trait]
pub trait RevisionBackend: Send + Sync {
    async fn create_revision(&self, revision: Revision) -> Result<Uuid, RevisionError>;
    async fn get_revision(&self, id: Uuid) -> Result<Option<Revision>, RevisionError>;
    async fn list_revisions(&self, filter: RevisionFilter) -> Result<Vec<Revision>, RevisionError>;
    async fn delete_revision(&self, id: Uuid) -> Result<(), RevisionError>;
    async fn count_revisions(&self, filter: RevisionFilter) -> Result<usize, RevisionError>;
}
```

**Методы:**

#### create_revision

Создает новую ревизию.

```rust
async fn create_revision(&self, revision: Revision) -> Result<Uuid, RevisionError>;
```

**Параметры:**
- `revision` — ревизия для создания

**Возвращает:**
- `Ok(Uuid)` — ID созданной ревизии
- `Err(RevisionError)` — ошибка создания

#### get_revision

Получает ревизию по ID.

```rust
async fn get_revision(&self, id: Uuid) -> Result<Option<Revision>, RevisionError>;
```

**Параметры:**
- `id` — ID ревизии

**Возвращает:**
- `Ok(Some(Revision))` — найденная ревизия
- `Ok(None)` — ревизия не найдена
- `Err(RevisionError)` — ошибка получения

#### list_revisions

Получает список ревизий по фильтру.

```rust
async fn list_revisions(&self, filter: RevisionFilter) -> Result<Vec<Revision>, RevisionError>;
```

**Параметры:**
- `filter` — фильтр для выборки

**Возвращает:**
- `Ok(Vec<Revision>)` — список ревизий
- `Err(RevisionError)` — ошибка получения

#### delete_revision

Удаляет ревизию по ID.

```rust
async fn delete_revision(&self, id: Uuid) -> Result<(), RevisionError>;
```

**Параметры:**
- `id` — ID ревизии

**Возвращает:**
- `Ok(())` — успешно удалено
- `Err(RevisionError)` — ошибка удаления

#### count_revisions

Подсчитывает количество ревизий по фильтру.

```rust
async fn count_revisions(&self, filter: RevisionFilter) -> Result<usize, RevisionError>;
```

**Параметры:**
- `filter` — фильтр для подсчета

**Возвращает:**
- `Ok(usize)` — количество ревизий
- `Err(RevisionError)` — ошибка подсчета

## RevisionService

Основной сервис для работы с ревизиями.

### new

Создает новый экземпляр сервиса.

```rust
pub fn new(backend: impl RevisionBackend + 'static) -> Self
```

**Параметры:**
- `backend` — backend для хранения ревизий

**Пример:**

```rust
let backend = SeaORMBackend::connect(&database_url).await?;
let service = RevisionService::new(backend);
```

---

### create_revision

Создает новую ревизию для объекта.

```rust
pub async fn create_revision<T: Revisionable>(
    &self,
    item: &T,
    content_id: &str,
    locale: Option<&str>,
    created_by: Option<&str>,
    change_source: Option<&str>,
    change_summary: Option<&str>,
) -> Result<Uuid, RevisionError>
```

**Параметры:**
- `item` — объект для версионирования
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `created_by` — ID пользователя (опционально)
- `change_source` — источник изменений (опционально)
- `change_summary` — описание изменений (опционально)

**Возвращает:**
- `Ok(Uuid)` — ID созданной ревизии
- `Err(RevisionError)` — ошибка создания

**Пример:**

```rust
let revision_id = service.create_revision::<Post>(
    &post,
    "post-123",
    Some("en"),
    Some("user-456"),
    Some("web"),
    Some("Updated title"),
).await?;
```

---

### create_revision_with_tracker

Создает ревизию с использованием tracker.

```rust
pub async fn create_revision_with_tracker<T: Revisionable>(
    &self,
    item: &T,
    content_id: &str,
    locale: Option<&str>,
    tracker: &RevisionTracker,
) -> Result<Uuid, RevisionError>
```

**Параметры:**
- `item` — объект для версионирования
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `tracker` — tracker с метаданными

**Возвращает:**
- `Ok(Uuid)` — ID созданной ревизии
- `Err(RevisionError)` — ошибка создания

**Пример:**

```rust
let tracker = RevisionTracker::new("user-456")
    .with_source("web")
    .with_summary("Updated title");

let revision_id = service.create_revision_with_tracker::<Post>(
    &post,
    "post-123",
    Some("en"),
    &tracker,
).await?;
```

---

### get_revision

Получает ревизию по номеру.

```rust
pub async fn get_revision<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    revision_number: i32,
) -> Result<Option<Revision>, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `revision_number` — номер ревизии

**Возвращает:**
- `Ok(Some(Revision))` — найденная ревизия
- `Ok(None)` — ревизия не найдена
- `Err(RevisionError)` — ошибка получения

**Пример:**

```rust
let revision = service.get_revision::<Post>(
    "post-123",
    Some("en"),
    5,
).await?;

if let Some(rev) = revision {
    println!("Title: {}", rev.delta["title"]["new"]);
}
```

---

### list_revisions

Получает список ревизий с пагинацией.

```rust
pub async fn list_revisions<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    limit: Option<usize>,
    before: Option<&str>,
    after: Option<&str>,
    sort_order: Option<SortOrder>,
) -> Result<Vec<Revision>, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `limit` — максимальное количество (опционально)
- `before` — cursor для пагинации назад (опционально)
- `after` — cursor для пагинации вперед (опционально)
- `sort_order` — порядок сортировки (опционально)

**Возвращает:**
- `Ok(Vec<Revision>)` — список ревизий
- `Err(RevisionError)` — ошибка получения

**Пример:**

```rust
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),  // limit
    None,       // before
    None,       // after
    Some(SortOrder::Descending),
).await?;

for rev in revisions {
    println!("Revision #{} at {}", rev.revision_number, rev.created_at);
}
```

---

### compare_revisions

Сравнивает две ревизии и возвращает diff.

```rust
pub async fn compare_revisions<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    from_revision: i32,
    to_revision: i32,
) -> Result<RevisionDiff, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `from_revision` — номер начальной ревизии
- `to_revision` — номер конечной ревизии

**Возвращает:**
- `Ok(RevisionDiff)` — различия между ревизиями
- `Err(RevisionError)` — ошибка сравнения

**Пример:**

```rust
let diff = service.compare_revisions::<Post>(
    "post-123",
    Some("en"),
    3,  // from
    5,  // to
).await?;

for change in diff.changes {
    println!("{}: {:?} -> {:?}", change.field, change.old_value, change.new_value);
}
```

---

### restore_revision

Восстанавливает контент к указанной ревизии.

```rust
pub async fn restore_revision<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    target_revision: i32,
    restored_by: Option<&str>,
    change_summary: Option<&str>,
    create_snapshot: bool,
) -> Result<Uuid, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `target_revision` — номер целевой ревизии
- `restored_by` — ID пользователя (опционально)
- `change_summary` — описание (опционально)
- `create_snapshot` — создать snapshot перед восстановлением

**Возвращает:**
- `Ok(Uuid)` — ID новой ревизии (snapshot или restored)
- `Err(RevisionError)` — ошибка восстановления

**Пример:**

```rust
let restored_id = service.restore_revision::<Post>(
    "post-123",
    Some("en"),
    3,              // target revision
    Some("user-456"),
    Some("Restored to v3"),
    true,           // create snapshot
).await?;
```

---

### create_named_version

Создает именованную версию (snapshot).

```rust
pub async fn create_named_version<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    name: &str,
    description: Option<&str>,
    created_by: Option<&str>,
) -> Result<Uuid, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `name` — имя версии
- `description` — описание (опционально)
- `created_by` — ID пользователя (опционально)

**Возвращает:**
- `Ok(Uuid)` — ID созданной версии
- `Err(RevisionError)` — ошибка создания

**Пример:**

```rust
let version_id = service.create_named_version::<Post>(
    "post-123",
    Some("en"),
    "v1.0-published",
    Some("First published version"),
    Some("user-456"),
).await?;
```

---

### list_named_versions

Получает список именованных версий.

```rust
pub async fn list_named_versions<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
) -> Result<Vec<Revision>, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)

**Возвращает:**
- `Ok(Vec<Revision>)` — список named versions
- `Err(RevisionError)` — ошибка получения

**Пример:**

```rust
let versions = service.list_named_versions::<Post>(
    "post-123",
    Some("en"),
).await?;

for version in versions {
    println!("{}: Revision #{}", version.version_name.unwrap(), version.revision_number);
}
```

---

### apply_retention_policy_for_type

Применяет retention policy для типа контента.

```rust
pub async fn apply_retention_policy_for_type<T: ContentRevisionConfig>(
    &self,
) -> Result<u64, RevisionError>
```

**Возвращает:**
- `Ok(u64)` — количество удаленных ревизий
- `Err(RevisionError)` — ошибка применения

**Пример:**

```rust
let deleted = service.apply_retention_policy_for_type::<Post>().await?;
println!("Deleted {} old revisions", deleted);
```

---

### get_revision_at_time

Получает ревизию, актуальную на указанное время.

```rust
pub async fn get_revision_at_time<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    timestamp: DateTime<Utc>,
) -> Result<Option<Revision>, RevisionError>
```

**Параметры:**
- `content_id` — идентификатор контента
- `locale` — локаль (опционально)
- `timestamp` — временная метка

**Возвращает:**
- `Ok(Some(Revision))` — ревизия на указанное время
- `Ok(None)` — ревизия не найдена
- `Err(RevisionError)` — ошибка получения

**Пример:**

```rust
let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

let revision = service.get_revision_at_time::<Post>(
    "post-123",
    Some("en"),
    timestamp,
).await?;
```

## RevisionBackend

### SeaORMBackend

Backend для PostgreSQL на основе SeaORM.

#### connect

Подключается к базе данных.

```rust
pub async fn connect(database_url: &str) -> Result<Self, RevisionError>
```

**Параметры:**
- `database_url` — URL подключения к базе

**Возвращает:**
- `Ok(SeaORMBackend)` — подключенный backend
- `Err(RevisionError)` — ошибка подключения

**Пример:**

```rust
let backend = SeaORMBackend::connect(
    "postgres://rustok:password@localhost:5432/rustok_revisions"
).await?;
```

#### from_pool

Создает backend из существующего connection pool.

```rust
pub fn from_pool(pool: DatabaseConnection) -> Self
```

**Параметры:**
- `pool` — connection pool

**Пример:**

```rust
let pool = PgPoolOptions::new()
    .max_connections(20)
    .connect(&database_url)
    .await?;

let backend = SeaORMBackend::from_pool(pool);
```

---

### InMemoryBackend

Backend для тестов (хранение в памяти).

#### new

Создает новый in-memory backend.

```rust
pub fn new() -> Self
```

**Пример:**

```rust
let backend = InMemoryBackend::new();
let service = RevisionService::new(backend);
```

## RevisionTracker

Tracker для группировки связанных изменений.

### new

Создает новый tracker.

```rust
pub fn new(user_id: &str) -> Self
```

**Параметры:**
- `user_id` — ID пользователя

**Пример:**

```rust
let tracker = RevisionTracker::new("user-456");
```

---

### with_source

Устанавливает источник изменений.

```rust
pub fn with_source(mut self, source: &str) -> Self
```

**Параметры:**
- `source` — источник изменений

**Пример:**

```rust
let tracker = RevisionTracker::new("user-456")
    .with_source("web");
```

---

### with_summary

Устанавливает описание изменений.

```rust
pub fn with_summary(mut self, summary: &str) -> Self
```

**Параметры:**
- `summary` — описание изменений

**Пример:**

```rust
let tracker = RevisionTracker::new("user-456")
    .with_source("web")
    .with_summary("Updated title and content");
```

---

### clear

Очищает tracker.

```rust
pub fn clear(&mut self)
```

**Пример:**

```rust
let mut tracker = RevisionTracker::new("user-456");
tracker.clear();
```

## Error Types

### RevisionError

Enum ошибок библиотеки.

```rust
pub enum RevisionError {
    DatabaseError(String),
    SerializationError(String),
    NotFound,
    ValidationError(String),
    Forbidden,
    Unauthorized,
    RateLimitExceeded,
    EncryptionError(String),
    SchemaError(String),
    VaultError(String),
    ConfigError(String),
    AuthenticationError(String),
    InternalError(String),
}
```

**Варианты:**

| Вариант | Описание |
|---------|----------|
| `DatabaseError` | Ошибка базы данных |
| `SerializationError` | Ошибка сериализации |
| `NotFound` | Ревизия не найдена |
| `ValidationError` | Ошибка валидации |
| `Forbidden` | Доступ запрещен |
| `Unauthorized` | Не авторизован |
| `RateLimitExceeded` | Превышен rate limit |
| `EncryptionError` | Ошибка шифрования |
| `SchemaError` | Ошибка схемы |
| `VaultError` | Ошибка Vault |
| `ConfigError` | Ошибка конфигурации |
| `AuthenticationError` | Ошибка аутентификации |
| `InternalError` | Внутренняя ошибка |

**Пример:**

```rust
match service.create_revision::<Post>(&post, "post-123", ...).await {
    Ok(id) => println!("Created: {}", id),
    Err(RevisionError::NotFound) => println!("Not found"),
    Err(RevisionError::ValidationError(msg)) => println!("Validation: {}", msg),
    Err(e) => println!("Error: {:?}", e),
}
```

## Enums

### SortOrder

Порядок сортировки.

```rust
pub enum SortOrder {
    Ascending,
    Descending,
}
```

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

## Functions

### calculate_delta

Вычисляет delta между двумя JSON значениями.

```rust
pub fn calculate_delta(
    old_value: &serde_json::Value,
    new_value: &serde_json::Value,
) -> Result<serde_json::Value, RevisionError>
```

**Параметры:**
- `old_value` — старое значение
- `new_value` — новое значение

**Возвращает:**
- `Ok(Value)` — delta в JSON формате
- `Err(RevisionError)` — ошибка вычисления

**Пример:**

```rust
let old = json!({"title": "Old", "content": "Text"});
let new = json!({"title": "New", "content": "Text"});

let delta = calculate_delta(&old, &new)?;
// Result: {"title": {"old": "Old", "new": "New"}}
```

---

### apply_delta

Применяет delta к JSON значению.

```rust
pub fn apply_delta(
    base: &serde_json::Value,
    delta: &serde_json::Value,
) -> Result<serde_json::Value, RevisionError>
```

**Параметры:**
- `base` — базовое значение
- `delta` — delta для применения

**Возвращает:**
- `Ok(Value)` — результат применения delta
- `Err(RevisionError)` — ошибка применения

**Пример:**

```rust
let base = json!({"title": "Old", "content": "Text"});
let delta = json!({"title": {"old": "Old", "new": "New"}});

let result = apply_delta(&base, &delta)?;
// Result: {"title": "New", "content": "Text"}
```

## Macros

### #[derive(Revisionable)]

Derive macro для автоматической реализации trait `Revisionable`.

**Атрибуты:**

- `#[revision(tracked)]` — поле отслеживается
- `#[revision(ignored)]` — поле игнорируется

**Пример:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    content: String,
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
    
    #[revision(ignored)]
    view_count: i32,
}
```

**Генерируемый код:**

```rust
impl Revisionable for Post {
    fn to_revision_value(&self) -> Result<serde_json::Value, RevisionError> {
        let mut map = serde_json::Map::new();
        map.insert("title".to_string(), json!(self.title));
        map.insert("content".to_string(), json!(self.content));
        Ok(serde_json::Value::Object(map))
    }
}
```

## Заключение

Этот API reference покрывает:

✅ **Основные типы** — Revision, RevisionDiff, FieldChange, RevisionFilter, RetentionPolicy  
✅ **Traits** — Revisionable, ContentRevisionConfig, RevisionBackend  
✅ **RevisionService** — все методы сервиса  
✅ **RevisionBackend** — SeaORMBackend, InMemoryBackend  
✅ **RevisionTracker** — tracker для группировки изменений  
✅ **Error Types** — RevisionError и его варианты  
✅ **Enums** — SortOrder  
✅ **Functions** — calculate_delta, apply_delta  
✅ **Macros** — derive(Revisionable)  

Используйте этот справочник для:

- Быстрого поиска нужных методов
- Понимания сигнатур функций
- Изучения примеров использования
- Понимания типов данных

**Удачи в использовании API!** 📚
