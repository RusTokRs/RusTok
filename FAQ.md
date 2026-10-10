# Frequently Asked Questions (FAQ)

Ответы на часто задаваемые вопросы о системе revision history.

## Содержание

1. [Общие вопросы](#общие-вопросы)
2. [Установка и настройка](#установка-и-настройка)
3. [Использование](#использование)
4. [Производительность](#производительность)
5. [Безопасность](#безопасность)
6. [Troubleshooting](#troubleshooting)
7. [Миграция с других решений](#миграция-с-других-решений)

## Общие вопросы

### Что такое rustok-revisions?

**rustok-revisions** — это standalone библиотека для управления историей изменений контента в Rust приложениях. Она предоставляет:

- Delta-based хранение изменений
- Multilingual поддержку
- Named versions (snapshots)
- Restoration capabilities
- Async/await API
- Backend agnostic архитектуру

### Чем это отличается от Git?

**Git** — это система контроля версий для файлов и кода.

**rustok-revisions** — это система контроля версий для структурированных данных (JSON объектов) в базе данных.

**Ключевые отличия:**
- Git работает с файлами, rustok-revisions с JSON объектами
- Git хранит полные snapshots, rustok-revisions хранит deltas
- Git для разработчиков, rustok-revisions для бизнес-логики
- Git работает локально, rustok-revisions работает с базой данных

### Можно ли использовать это для файлов?

**Нет**, rustok-revisions предназначен для структурированных данных (JSON).

Для файлов используйте:
- **Git** — для текстовых файлов и кода
- **Oxen** — для больших данных и ML datasets
- **S3 Versioning** — для файлов в облаке

### Поддерживается ли только PostgreSQL?

**Нет**, библиотека backend agnostic.

**Встроенные backends:**
- `InMemoryBackend` — для тестов
- `SeaORMBackend` — для PostgreSQL

**Можно создать свой backend для:**
- MySQL
- SQLite
- MongoDB
- DynamoDB
- И любой другой базы данных

### Какие типы данных поддерживаются?

Поддерживаются все JSON-совместимые типы:

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,              // ✅ String
    
    #[revision(tracked)]
    content: String,            // ✅ String
    
    #[revision(tracked)]
    views: i32,                 // ✅ Integer
    
    #[revision(tracked)]
    rating: f64,                // ✅ Float
    
    #[revision(tracked)]
    published: bool,            // ✅ Boolean
    
    #[revision(tracked)]
    tags: Vec<String>,          // ✅ Array
    
    #[revision(tracked)]
    metadata: serde_json::Value, // ✅ JSON object
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,  // ❌ Ignored
}
```

### Можно ли отслеживать вложенные объекты?

**Да**, с помощью `serde_json::Value`:

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    metadata: serde_json::Value, // Отслеживает весь объект
}

// Пример
let metadata = json!({
    "seo": {
        "title": "SEO Title",
        "description": "SEO Description"
    },
    "social": {
        "twitter": "@username"
    }
});
```

## Установка и настройка

### Как установить библиотеку?

```toml
[dependencies]
rustok-revisions = "0.1.0"
rustok-revisions-derive = "0.1.0"
```

### Нужен ли derive macro?

**Не обязательно**, но **настоятельно рекомендуется**.

**С derive macro:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>,
}
```

**Без derive macro:**
```rust
impl Revisionable for Post {
    fn to_revision_value(&self) -> Result<serde_json::Value, RevisionError> {
        let mut map = serde_json::Map::new();
        map.insert("title".to_string(), json!(self.title));
        // Не включаем updated_at
        Ok(serde_json::Value::Object(map))
    }
}

impl ContentRevisionConfig for Post {
    fn content_type() -> &'static str {
        "post"
    }
    
    fn tracked_fields() -> &'static [&'static str] {
        &["title"]
    }
    
    fn ignored_fields() -> &'static [&'static str] {
        &["updated_at"]
    }
    
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

### Как настроить базу данных?

1. **Создайте базу данных:**

```sql
CREATE DATABASE rustok_revisions;
CREATE USER rustok WITH ENCRYPTED PASSWORD 'password';
GRANT ALL PRIVILEGES ON DATABASE rustok_revisions TO rustok;
```

2. **Запустите миграции:**

```bash
rustok-revisions migrate up
```

3. **Настройте подключение:**

```rust
let db = SeaORMBackend::connect(
    "postgres://rustok:password@localhost:5432/rustok_revisions"
).await?;
```

### Как настроить retention policy?

**Для всех объектов:**

```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

**Для конкретного объекта:**

```rust
let config = ContentRevisionConfig {
    tenant_id: "tenant-1".to_string(),
    retention_policy: Some(RetentionPolicy::KeepDays { days: 90 }),
};
```

**Применить ко всем:**

```rust
service.apply_retention_policy_for_type::<Post>().await?;
```

### Как настроить multilingual поддержку?

**Просто используйте поле `locale`:**

```rust
// Создать ревизию для English
service.create_revision::<Post>(
    &post,
    "post-123",
    Some("en"),
    None,
    None,
    None,
).await?;

// Создать ревизию для Russian
service.create_revision::<Post>(
    &post,
    "post-123",
    Some("ru"),
    None,
    None,
    None,
).await?;
```

**Каждая локаль имеет свою историю ревизий.**

## Использование

### Как создать ревизию?

```rust
let revision_id = service.create_revision::<Post>(
    &post,              // Объект
    "post-123",         // content_id
    Some("en"),         // locale
    Some("user-456"),   // created_by
    Some("web"),        // change_source
    Some("Updated title"), // change_summary
).await?;
```

### Как создать ревизию с tracker?

```rust
// Создать tracker
let tracker = RevisionTracker::new("user-456")
    .with_source("web")
    .with_summary("Updated title");

// Использовать tracker
let revision_id = service.create_revision_with_tracker::<Post>(
    &post,
    "post-123",
    Some("en"),
    &tracker,
).await?;

// Очистить tracker
tracker.clear();
```

### Как получить историю?

```rust
let history = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(10),  // limit
    None,      // before
    None,      // after
    Some(SortOrder::Descending),
).await?;

for revision in history {
    println!("Revision #{} by {}", revision.revision_number, revision.created_by);
}
```

### Как получить конкретную ревизию?

```rust
let revision = service.get_revision::<Post>(
    "post-123",
    Some("en"),
    5,  // revision_number
).await?;

if let Some(revision) = revision {
    println!("Title: {}", revision.content["title"]);
}
```

### Как сравнить две версии?

```rust
let diff = service.compare_revisions::<Post>(
    "post-123",
    Some("en"),
    3,  // from_revision
    5,  // to_revision
).await?;

for change in diff.changes {
    println!("{}: {:?} -> {:?}", change.field, change.old_value, change.new_value);
}
```

### Как восстановить к предыдущей версии?

```rust
let restored_revision_id = service.restore_revision::<Post>(
    "post-123",
    Some("en"),
    3,              // target_revision
    Some("user-456"),
    Some("Restored to v3"),
    true,           // create_snapshot
).await?;
```

### Как создать named version?

```rust
let revision_id = service.create_named_version::<Post>(
    "post-123",
    Some("en"),
    "v1.0-published",
    Some("First published version"),
    Some("user-456"),
).await?;
```

### Как получить все named versions?

```rust
let versions = service.list_named_versions::<Post>(
    "post-123",
    Some("en"),
).await?;

for version in versions {
    println!("{}: Revision #{}", version.name, version.revision_number);
}
```

### Как восстановить состояние на определенное время?

```rust
let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

let revision = service.get_revision_at_time::<Post>(
    "post-123",
    Some("en"),
    timestamp,
).await?;
```

### Как отключить автоматические ревизии?

```rust
// Просто не вызывайте create_revision()

// Или используйте условную логику
if should_track_changes {
    service.create_revision::<Post>(&post, "post-123", ...).await?;
}
```

### Как отслеживать только определенные поля?

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,      // ✅ Отслеживается
    
    #[revision(tracked)]
    content: String,    // ✅ Отслеживается
    
    #[revision(ignored)]
    views: i32,         // ❌ Игнорируется
    
    #[revision(ignored)]
    updated_at: DateTime<Utc>, // ❌ Игнорируется
}
```

## Производительность

### Какую производительность можно ожидать?

**Создание ревизии:**
- Delta calculation: ~5-10 ms
- Database insert: ~10-20 ms
- **Total: ~15-30 ms**

**Получение истории (100 ревизий):**
- Database query: ~20-50 ms
- Delta application: ~10-20 ms
- **Total: ~30-70 ms**

**Сравнение версий:**
- Delta retrieval: ~10-20 ms
- Diff calculation: ~5-10 ms
- **Total: ~15-30 ms**

### Как оптимизировать производительность?

1. **Используйте индексы:**

```sql
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);
```

2. **Включите connection pooling:**

```rust
let pool = Pool::builder()
    .max_size(20)
    .min_idle(5)
    .build()?;
```

3. **Используйте batch operations:**

```rust
// Batch create
let revision_ids = service.batch_create_revisions::<Post>(posts).await?;
```

4. **Включите кэширование:**

```rust
// Кэшировать часто запрашиваемые данные
let cached = cache.get_or_insert(key, async {
    service.list_revisions::<Post>(...).await
}).await?;
```

### Как уменьшить размер базы данных?

1. **Используйте delta-based storage:**

Ревизия хранит только изменения, а не полный snapshot.

2. **Настройте retention policy:**

```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

3. **Запускайте cleanup регулярно:**

```rust
// Cron job
service.apply_retention_policy_for_type::<Post>().await?;
```

4. **Используйте сжатие:**

```rust
// Включить сжатие для больших объектов
let compressed = compress_delta(&delta);
```

### Как обрабатывать большие объекты?

1. **Разбейте на меньшие части:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(tracked)]
    content_part1: String,
    
    #[revision(tracked)]
    content_part2: String,
}
```

2. **Игнорируйте большие поля:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
    
    #[revision(ignored)]
    large_content: String,  // Не отслеживается
}
```

3. **Используйте сжатие:**

```rust
let compressed_delta = compress(&delta);
```

### Как масштабировать систему?

**Horizontal scaling:**

```nginx
upstream rustok_revisions {
    least_conn;
    server revisions-1:8080;
    server revisions-2:8080;
    server revisions-3:8080;
}
```

**Database replication:**

```
Master: Write operations
Replicas: Read operations
```

**Caching:**

```rust
// Redis для кэширования
let cached = redis.get(key).await?;
```

## Безопасность

### Как обеспечить безопасность данных?

1. **Используйте шифрование:**

```rust
let encrypted_delta = encrypt(&delta, &encryption_key);
```

2. **Настройте access control:**

```rust
// Проверить права доступа
if !user.can_access(content_id) {
    return Err(RevisionError::Unauthorized);
}
```

3. **Используйте rate limiting:**

```rust
.layer(RateLimitLayer::new(100, Duration::from_secs(60)))
```

4. **Аудит всех операций:**

```rust
info!("Revision created: {} by {}", revision_id, user_id);
```

### Как защитить от несанкционированного доступа?

1. **JWT аутентификация:**

```rust
let claims = validate_jwt(token)?;
let user_id = claims.sub;
```

2. **Role-based access control:**

```rust
if !user.has_role(Role::Admin) {
    return Err(RevisionError::Forbidden);
}
```

3. **Audit log:**

```rust
audit_log.record(AuditEvent::RevisionRestored {
    revision_id,
    user_id,
    timestamp: Utc::now(),
});
```

### Как соответствовать GDPR?

1. **Право на удаление:**

```rust
// Удалить все ревизии пользователя
service.delete_all_revisions_for_user(user_id).await?;
```

2. **Право на экспорт:**

```rust
// Экспортировать историю изменений
let history = service.export_user_history(user_id).await?;
```

3. **Data retention:**

```rust
// Автоматическое удаление старых данных
impl ContentRevisionConfig for UserData {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepDays { days: 365 })
    }
}
```

## Troubleshooting

### Почему ревизии не создаются?

**Проверьте:**

1. **Подключение к базе данных:**

```bash
psql $DATABASE_URL
```

2. **Миграции применены:**

```bash
rustok-revisions migrate status
```

3. **Поля помечены как tracked:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]  // ✅ Должно быть
    title: String,
}
```

### Почему восстановление не работает?

**Проверьте:**

1. **Ревизия существует:**

```rust
let revision = service.get_revision::<Post>("post-123", Some("en"), 3).await?;
assert!(revision.is_some());
```

2. **Правильный тип данных:**

```rust
// Должен быть тот же тип
let restored = service.restore_revision::<Post>("post-123", Some("en"), 3, ...).await?;
```

### Почему сравнение показывает неправильные изменения?

**Проверьте:**

1. **Правильные revision numbers:**

```rust
let diff = service.compare_revisions::<Post>("post-123", Some("en"), 3, 5).await?;
```

2. **Ревизии существуют:**

```rust
let rev3 = service.get_revision::<Post>("post-123", Some("en"), 3).await?;
let rev5 = service.get_revision::<Post>("post-123", Some("en"), 5).await?;
assert!(rev3.is_some() && rev5.is_some());
```

### Почему retention policy не работает?

**Проверьте:**

1. **Policy настроена:**

```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

2. **Cleanup запущен:**

```rust
service.apply_retention_policy_for_type::<Post>().await?;
```

## Миграция с других решений

### Как мигрировать с PaperTrail?

**PaperTrail (Ruby):**

```ruby
class Post < ApplicationRecord
  has_paper_trail
end
```

**rustok-revisions:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
}
```

**Миграция данных:**

```sql
-- Экспортировать из PaperTrail
SELECT item_type, item_id, object, created_at FROM versions;

-- Импортировать в rustok-revisions
INSERT INTO content_revisions (content_type, content_id, delta, created_at)
SELECT item_type, item_id, object, created_at FROM versions;
```

### Как мигрировать с Django Simple History?

**Django Simple History:**

```python
class Post(models.Model):
    title = models.CharField()
    history = HistoricalRecords()
```

**rustok-revisions:**

```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,
}
```

**Миграция данных:**

```python
# Экспортировать из Django
for history in Post.history.all():
    export(history)

# Импортировать в rustok-revisions
# Используйте API или прямые SQL запросы
```

### Как мигрировать с Git-based решений?

**Git-based:**

```bash
git log --oneline file.txt
```

**rustok-revisions:**

```rust
let history = service.list_revisions::<Post>("post-123", ...).await?;
```

**Миграция:**

1. Парсить Git log
2. Извлекать изменения
3. Создавать ревизии через API

```rust
for commit in parse_git_log("file.txt") {
    service.create_revision::<Post>(
        &parse_content(commit.content),
        "post-123",
        Some("en"),
        Some(commit.author),
        Some("git"),
        Some(commit.message),
    ).await?;
}
```

## Заключение

Этот FAQ покрывает:

✅ **Общие вопросы** — что это и зачем  
✅ **Установка** — как установить и настроить  
✅ **Использование** — как использовать основные функции  
✅ **Производительность** — как оптимизировать  
✅ **Безопасность** — как защитить данные  
✅ **Troubleshooting** — как решить проблемы  
✅ **Миграция** — как перейти с других решений  

Если у вас есть вопросы, которых здесь нет:

1. Проверьте [документацию](README.md)
2. Посмотрите [examples](examples/)
3. Откройте [issue на GitHub](https://github.com/rustok/rustok-revisions/issues)
4. Присоединяйтесь к [Discord](https://discord.gg/rustok)

**Удачного использования!** 🚀
