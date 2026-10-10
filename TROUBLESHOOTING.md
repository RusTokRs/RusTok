# Troubleshooting Guide

Полное руководство по решению проблем и диагностике системы revision history.

## Содержание

1. [Общие проблемы](#общие-проблемы)
2. [Проблемы с базой данных](#проблемы-с-базой-данных)
3. [Проблемы производительности](#проблемы-производительности)
4. [Проблемы с API](#проблемы-с-api)
5. [Проблемы с данными](#проблемы-с-данными)
6. [Проблемы с миграциями](#проблемы-с-миграциями)
7. [Проблемы с конфигурацией](#проблемы-с-конфигурацией)
8. [Диагностические инструменты](#диагностические-инструменты)
9. [Часто задаваемые вопросы](#часто-задаваемые-вопросы)

## Общие проблемы

### Проблема: Ревизии не создаются

**Симптомы:**
- `create_revision()` возвращает ошибку
- Ревизии не появляются в базе данных
- Логи показывают ошибки

**Диагностика:**

```rust
// Проверяем подключение к базе
let backend = SeaORMBackend::connect(&database_url).await?;
println!("Database connected: {:?}", backend.is_healthy().await);

// Проверяем что поля помечены как tracked
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]  // ✅ Должно быть
    title: String,
}

// Проверяем что объект валидный
let post = Post {
    id: "post-1".to_string(),
    title: "Test".to_string(),
};

match service.create_revision::<Post>(&post, &post.id, None, None, None, None).await {
    Ok(revision_id) => println!("Created: {}", revision_id),
    Err(e) => eprintln!("Error: {:?}", e),
}
```

**Решения:**

1. **Проверьте подключение к базе данных:**
```bash
psql $DATABASE_URL -c "SELECT 1"
```

2. **Проверьте что миграции применены:**
```bash
rustok-revisions migrate status
```

3. **Проверьте что поля помечены как `#[revision(tracked)]`:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]  // Обязательно!
    title: String,
}
```

4. **Проверьте логи:**
```bash
journalctl -u rustok-revisions -f
```

---

### Проблема: Восстановление не работает

**Симптомы:**
- `restore_revision()` возвращает ошибку
- Восстановленные данные некорректны
- Revision number не увеличивается

**Диагностика:**

```rust
// Проверяем что ревизия существует
let revision = service.get_revision::<Post>(
    "post-123",
    Some("en"),
    3,
).await?;

match revision {
    Some(rev) => println!("Revision exists: {:?}", rev),
    None => println!("Revision not found!"),
}

// Проверяем что используем правильный тип
let restored = service.restore_revision::<Post>(  // ✅ Post, не &Post
    "post-123",
    Some("en"),
    3,
    Some("user-123"),
    Some("Restored"),
    true,
).await?;
```

**Решения:**

1. **Проверьте что ревизия существует:**
```rust
let revision = service.get_revision::<Post>(content_id, locale, revision_number).await?;
assert!(revision.is_some());
```

2. **Проверьте что используете правильный тип:**
```rust
// ✅ Правильно
let restored = service.restore_revision::<Post>(...).await?;

// ❌ Неправильно - другой тип
let restored = service.restore_revision::<Comment>(...).await?;
```

3. **Проверьте права доступа:**
```rust
if !user.can_restore(content_id) {
    return Err(RevisionError::Forbidden);
}
```

---

### Проблема: Сравнение показывает неправильные изменения

**Симптомы:**
- `compare_revisions()` показывает неверные diff
- Изменения дублируются
- Некоторые изменения отсутствуют

**Диагностика:**

```rust
// Проверяем что обе ревизии существуют
let rev3 = service.get_revision::<Post>("post-123", Some("en"), 3).await?;
let rev5 = service.get_revision::<Post>("post-123", Some("en"), 5).await?;

println!("Rev 3: {:?}", rev3);
println!("Rev 5: {:?}", rev5);

// Сравниваем
let diff = service.compare_revisions::<Post>(
    "post-123",
    Some("en"),
    3,
    5,
).await?;

for change in diff.changes {
    println!("{}: {:?} -> {:?}", change.field, change.old_value, change.new_value);
}
```

**Решения:**

1. **Проверьте что обе ревизии существуют:**
```rust
let from = service.get_revision::<Post>(content_id, locale, from_revision).await?;
let to = service.get_revision::<Post>(content_id, locale, to_revision).await?;

assert!(from.is_some() && to.is_some());
```

2. **Проверьте порядок ревизий:**
```rust
// ✅ Правильно: from < to
let diff = service.compare_revisions::<Post>(content_id, locale, 3, 5).await?;

// ❌ Неправильно: from > to
let diff = service.compare_revisions::<Post>(content_id, locale, 5, 3).await?;
```

3. **Проверьте что используете правильный locale:**
```rust
let diff = service.compare_revisions::<Post>(
    "post-123",
    Some("en"),  // ✅ Должен совпадать
    3,
    5,
).await?;
```

---

### Проблема: Retention policy не работает

**Симптомы:**
- Старые ревизии не удаляются
- `apply_retention_policy()` возвращает 0
- База данных растет

**Диагностика:**

```rust
// Проверяем что policy настроена
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })  // ✅ Должно быть
    }
}

// Проверяем количество ревизий
let count = service.count_revisions::<Post>("post-123", Some("en")).await?;
println!("Current count: {}", count);

// Запускаем cleanup
let deleted = service.apply_retention_policy_for_type::<Post>().await?;
println!("Deleted: {}", deleted);
```

**Решения:**

1. **Проверьте что policy настроена:**
```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

2. **Запустите cleanup вручную:**
```bash
rustok-revisions cleanup --dry-run
rustok-revisions cleanup
```

3. **Проверьте cron job:**
```bash
crontab -l
```

4. **Проверьте что named versions не удаляются:**
```rust
// Named versions защищены от удаления
let named = service.list_named_versions::<Post>("post-123", Some("en")).await?;
println!("Named versions: {}", named.len());
```

## Проблемы с базой данных

### Проблема: Медленные запросы

**Симптомы:**
- Запросы занимают > 1 секунды
- Высокая нагрузка на CPU
- Таймауты

**Диагностика:**

```sql
-- Найти медленные запросы
SELECT query, calls, total_time, mean_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;

-- Проверить использование индексов
EXPLAIN ANALYZE 
SELECT * FROM content_revisions 
WHERE tenant_id = 'tenant-1' 
  AND content_type = 'blog_post' 
  AND content_id = 'post-123'
ORDER BY revision_number DESC
LIMIT 100;

-- Проверить размер таблицы
SELECT 
    pg_size_pretty(pg_total_relation_size('content_revisions')) AS total_size,
    pg_size_pretty(pg_relation_size('content_revisions')) AS table_size,
    pg_size_pretty(pg_indexes_size('content_revisions')) AS index_size;
```

**Решения:**

1. **Создайте недостающие индексы:**
```sql
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);

CREATE INDEX idx_revisions_created_at 
  ON content_revisions(created_at DESC);
```

2. **Обновите статистику:**
```sql
ANALYZE content_revisions;
VACUUM ANALYZE content_revisions;
```

3. **Оптимизируйте запросы:**
```rust
// ✅ Хорошо: использует индекс
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),  // Limit!
    None,
    None,
    Some(SortOrder::Descending),
).await?;

// ❌ Плохо: нет limit
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    None,  // No limit!
    None,
    None,
    Some(SortOrder::Descending),
).await?;
```

4. **Используйте connection pooling:**
```rust
let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect(&database_url)
    .await?;
```

---

### Проблема: Ошибки подключения к базе

**Симптомы:**
- `Connection refused`
- `Too many connections`
- `Connection timeout`

**Диагностика:**

```bash
# Проверить подключение
psql $DATABASE_URL -c "SELECT 1"

# Проверить количество подключений
psql $DATABASE_URL -c "SELECT count(*) FROM pg_stat_activity;"

# Проверить лимит подключений
psql $DATABASE_URL -c "SHOW max_connections;"

# Проверить логи PostgreSQL
sudo journalctl -u postgresql -f
```

**Решения:**

1. **Увеличьте max_connections:**
```ini
# postgresql.conf
max_connections = 500
```

2. **Настройте connection pooling:**
```rust
let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(300))
    .connect(&database_url)
    .await?;
```

3. **Проверьте firewall:**
```bash
# Разрешить подключения к PostgreSQL
sudo ufw allow 5432/tcp
```

4. **Проверьте credentials:**
```bash
# Пересоздайте пользователя
sudo -u postgres psql
CREATE USER rustok WITH ENCRYPTED PASSWORD 'new_password';
GRANT ALL PRIVILEGES ON DATABASE rustok_revisions TO rustok;
```

---

### Проблема: База данных растет слишком быстро

**Симптомы:**
- Disk usage > 80%
- Backup занимает много времени
- Высокая стоимость storage

**Диагностика:**

```sql
-- Проверить размер таблицы
SELECT 
    pg_size_pretty(pg_total_relation_size('content_revisions')) AS total_size;

-- Проверить количество ревизий по типам
SELECT content_type, count(*) 
FROM content_revisions 
GROUP BY content_type 
ORDER BY count(*) DESC;

-- Проверить самые старые ревизии
SELECT min(created_at), max(created_at) 
FROM content_revisions;
```

**Решения:**

1. **Настройте retention policy:**
```rust
impl ContentRevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        Some(RetentionPolicy::KeepLast { count: 100 })
    }
}
```

2. **Запустите cleanup:**
```bash
rustok-revisions cleanup
```

3. **Используйте partitioning:**
```sql
CREATE TABLE content_revisions (
    -- columns
) PARTITION BY RANGE (created_at);

CREATE TABLE content_revisions_2024_q1 
  PARTITION OF content_revisions
  FOR VALUES FROM ('2024-01-01') TO ('2024-04-01');
```

4. **Архивируйте старые данные:**
```sql
-- Переместить старые ревизии в archive
CREATE TABLE content_revisions_archive AS
SELECT * FROM content_revisions 
WHERE created_at < NOW() - INTERVAL '1 year';

DELETE FROM content_revisions 
WHERE created_at < NOW() - INTERVAL '1 year';
```

## Проблемы производительности

### Проблема: Высокое использование памяти

**Симптомы:**
- Memory usage > 80%
- OOM killer terminates process
- Swap usage high

**Диагностика:**

```bash
# Проверить использование памяти
ps aux | grep rustok-revisions

# Проверить memory leaks
valgrind --leak-check=full ./rustok-revisions

# Проверить heap profile
heaptrack ./rustok-revisions
```

**Решения:**

1. **Уменьшите connection pool size:**
```rust
let pool = PgPoolOptions::new()
    .max_connections(10)  // Вместо 20
    .min_connections(2)   // Вместо 5
    .connect(&database_url)
    .await?;
```

2. **Ограничьте размер выборки:**
```rust
let revisions = service.list_revisions::<Post>(
    "post-123",
    Some("en"),
    Some(100),  // Limit!
    None,
    None,
    Some(SortOrder::Descending),
).await?;
```

3. **Используйте streaming для больших данных:**
```rust
use futures::stream::StreamExt;

let mut stream = service.stream_revisions::<Post>("post-123", Some("en")).await?;

while let Some(revision) = stream.next().await {
    process_revision(revision?);
}
```

4. **Включите garbage collection:**
```rust
// В config
[server]
gc_interval_seconds = 300
```

---

### Проблема: Высокая задержка (latency)

**Симптомы:**
- P95 latency > 500ms
- Таймауты на клиенте
- Пользователи жалуются на медленную работу

**Диагностика:**

```bash
# Проверить latency
curl -w "@curl-format.txt" -o /dev/null -s http://localhost:8080/graphql

# Проверить метрики
curl http://localhost:8080/metrics | grep http_request_duration

# Проверить медленные запросы
psql $DATABASE_URL -c "
SELECT query, mean_time 
FROM pg_stat_statements 
ORDER BY mean_time DESC 
LIMIT 10;
"
```

**Решения:**

1. **Добавьте индексы:**
```sql
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);
```

2. **Используйте кэширование:**
```rust
let cache = RevisionCache::new(Duration::from_secs(300));

let revisions = cache.get_or_fetch("post-123:en", || async {
    service.list_revisions::<Post>("post-123", Some("en"), Some(100), ...).await
}).await?;
```

3. **Оптимизируйте queries:**
```rust
// ✅ Хорошо: batch query
let revisions = service.batch_get_revisions::<Post>(
    vec![
        ("post-1".to_string(), Some("en".to_string()), 5),
        ("post-2".to_string(), Some("en".to_string()), 3),
    ]
).await?;

// ❌ Плохо: N+1 queries
let rev1 = service.get_revision::<Post>("post-1", Some("en"), 5).await?;
let rev2 = service.get_revision::<Post>("post-2", Some("en"), 3).await?;
```

4. **Масштабируйте горизонтально:**
```nginx
upstream rustok_revisions {
    least_conn;
    server revisions-1:8080;
    server revisions-2:8080;
    server revisions-3:8080;
}
```

## Проблемы с API

### Проблема: GraphQL queries возвращают ошибки

**Симптомы:**
- GraphQL errors в response
- `null` значения в данных
- Validation errors

**Диагностика:**

```graphql
# Проверить query в GraphiQL
query {
  revisions(contentId: "post-123", contentType: "blog_post") {
    edges {
      node {
        revisionNumber
        delta
        createdAt
      }
    }
  }
}
```

**Решения:**

1. **Проверьте schema:**
```graphql
type Query {
  revisions(
    contentId: String!
    contentType: String!
    locale: String
  ): RevisionConnection!
}
```

2. **Проверьте input validation:**
```rust
#[derive(Validate)]
struct RevisionsInput {
    #[validate(length(min = 1, max = 255))]
    content_id: String,
    
    #[validate(length(min = 1, max = 255))]
    content_type: String,
}
```

3. **Проверьте error handling:**
```rust
async fn revisions_resolver(
    input: RevisionsInput,
) -> Result<RevisionConnection, FieldError> {
    service.list_revisions::<Post>(&input.content_id, ...).await
        .map_err(|e| FieldError::new(e.to_string(), Value::null()))
}
```

---

### Проблема: Subscriptions не работают

**Симптомы:**
- WebSocket connection fails
- Нет обновлений в реальном времени
- Timeout errors

**Диагностика:**

```javascript
// Проверить WebSocket connection
const ws = new WebSocket('ws://localhost:8080/graphql');

ws.onopen = () => {
  console.log('Connected');
  ws.send(JSON.stringify({
    type: 'start',
    payload: {
      query: 'subscription { revisionCreated(contentId: "post-123") { revisionNumber } }'
    }
  }));
};

ws.onmessage = (event) => {
  console.log('Received:', event.data);
};

ws.onerror = (error) => {
  console.error('Error:', error);
};
```

**Решения:**

1. **Проверьте WebSocket support:**
```rust
use async_graphql_warp::{graphql_subscription};

let routes = graphql_subscription(schema)
    .or(graphql_post(schema));
```

2. **Проверьте CORS:**
```rust
use tower_http::cors::{CorsLayer, Any};

let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);

app.layer(cors)
```

3. **Проверьте authentication:**
```rust
let ws = new WebSocket('ws://localhost:8080/graphql', {
  headers: {
    'Authorization': 'Bearer ' + token
  }
});
```

## Проблемы с данными

### Проблема: Delta содержит неверные данные

**Симптомы:**
- `delta` field содержит wrong values
- `old` и `new` перепутаны
- Missing fields в delta

**Диагностика:**

```rust
let revision = service.get_revision::<Post>("post-123", Some("en"), 5).await?;

println!("Delta: {:?}", revision.delta);

// Проверяем структуру
assert!(revision.delta.is_object());
for (field, change) in revision.delta.as_object().unwrap() {
    println!("{}: old={:?}, new={:?}", 
        field, 
        change["old"], 
        change["new"]
    );
}
```

**Решения:**

1. **Проверьте Revisionable implementation:**
```rust
#[derive(Revisionable)]
struct Post {
    #[revision(tracked)]
    title: String,  // ✅ Будет в delta
}

impl Revisionable for Post {
    fn to_revision_value(&self) -> Result<serde_json::Value, RevisionError> {
        let mut map = serde_json::Map::new();
        map.insert("title".to_string(), json!(self.title));
        Ok(serde_json::Value::Object(map))
    }
}
```

2. **Проверьте serialization:**
```rust
let post = Post { title: "Test".to_string() };
let value = post.to_revision_value()?;
println!("Serialized: {:?}", value);
```

3. **Проверьте delta calculation:**
```rust
let old_value = json!({"title": "Old"});
let new_value = json!({"title": "New"});

let delta = calculate_delta(&old_value, &new_value)?;
println!("Delta: {:?}", delta);
// Expected: {"title": {"old": "Old", "new": "New"}}
```

---

### Проблема: Revision numbers не последовательны

**Симптомы:**
- Gaps в revision numbers (1, 2, 5, 7)
- Duplicate revision numbers
- Negative revision numbers

**Диагностика:**

```sql
-- Проверить gaps
SELECT revision_number 
FROM content_revisions 
WHERE content_id = 'post-123' 
ORDER BY revision_number;

-- Проверить duplicates
SELECT revision_number, count(*) 
FROM content_revisions 
WHERE content_id = 'post-123' 
GROUP BY revision_number 
HAVING count(*) > 1;
```

**Решения:**

1. **Используйте transactions:**
```rust
let mut txn = pool.begin().await?;

// Get next revision number
let next_number = sqlx::query_scalar::<_, i32>(
    "SELECT COALESCE(MAX(revision_number), 0) + 1 
     FROM content_revisions 
     WHERE content_id = $1"
)
.bind(content_id)
.fetch_one(&mut txn)
.await?;

// Create revision
sqlx::query("INSERT INTO content_revisions (revision_number, ...) VALUES ($1, ...)")
    .bind(next_number)
    .execute(&mut txn)
    .await?;

txn.commit().await?;
```

2. **Пересчитайте revision numbers:**
```sql
-- Пересчитать revision numbers
WITH numbered AS (
    SELECT 
        id,
        ROW_NUMBER() OVER (
            PARTITION BY content_type, content_id, locale 
            ORDER BY created_at
        ) as new_number
    FROM content_revisions
)
UPDATE content_revisions cr
SET revision_number = numbered.new_number
FROM numbered
WHERE cr.id = numbered.id;
```

## Проблемы с миграциями

### Проблема: Миграции не применяются

**Симптомы:**
- `rustok-revisions migrate up` не работает
- Таблицы не создаются
- Ошибки в логах

**Диагностика:**

```bash
# Проверить статус миграций
rustok-revisions migrate status

# Проверить таблицу миграций
psql $DATABASE_URL -c "SELECT * FROM seaql_migrations;"

# Проверить логи
rustok-revisions migrate up --verbose
```

**Решения:**

1. **Примените миграции принудительно:**
```bash
rustok-revisions migrate up --force
```

2. **Откатите и примените заново:**
```bash
rustok-revisions migrate down
rustok-revisions migrate up
```

3. **Проверьте права доступа:**
```sql
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO rustok;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO rustok;
```

4. **Примените миграции вручную:**
```sql
-- Создайте таблицу вручную
CREATE TABLE IF NOT EXISTS content_revisions (
    id UUID PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    content_type VARCHAR(255) NOT NULL,
    content_id VARCHAR(255) NOT NULL,
    locale VARCHAR(10),
    revision_number INTEGER NOT NULL,
    delta JSONB NOT NULL,
    created_by VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    change_source VARCHAR(255),
    change_summary TEXT,
    version_name VARCHAR(255)
);
```

## Проблемы с конфигурацией

### Проблема: Переменные окружения не загружаются

**Симптомы:**
- `DATABASE_URL not set`
- `JWT_SECRET not set`
- Приложение использует default значения

**Диагностика:**

```bash
# Проверить переменные окружения
env | grep DATABASE_URL
env | grep JWT_SECRET

# Проверить .env файл
cat .env

# Проверить загрузку в приложении
RUST_LOG=debug ./rustok-revisions
```

**Решения:**

1. **Экспортируйте переменные:**
```bash
export DATABASE_URL="postgres://rustok:password@localhost:5432/rustok_revisions"
export JWT_SECRET="your_secret_here"
```

2. **Используйте .env файл:**
```bash
# .env
DATABASE_URL=postgres://rustok:password@localhost:5432/rustok_revisions
JWT_SECRET=your_secret_here
```

```rust
use dotenv::dotenv;

fn main() {
    dotenv().ok();  // Загружает .env файл
    
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
}
```

3. **Проверьте systemd service:**
```ini
[Service]
EnvironmentFile=/opt/rustok-revisions/.env
ExecStart=/usr/local/bin/rustok-revisions
```

## Диагностические инструменты

### 1. Health Check

```rust
pub async fn health_check(
    Extension(state): Extension<AppState>,
) -> Json<HealthStatus> {
    let db_healthy = sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    
    let cache_healthy = state.cache.ping().await.is_ok();
    
    Json(HealthStatus {
        status: if db_healthy && cache_healthy { "healthy" } else { "unhealthy" },
        database: db_healthy,
        cache: cache_healthy,
        uptime: state.start_time.elapsed().as_secs(),
        version: env!("CARGO_PKG_VERSION"),
    })
}
```

### 2. Debug Endpoint

```rust
pub async fn debug_info(
    Extension(state): Extension<AppState>,
) -> Json<DebugInfo> {
    let revision_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM content_revisions"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    
    let connection_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM pg_stat_activity"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);
    
    Json(DebugInfo {
        revision_count,
        connection_count,
        memory_usage: get_memory_usage(),
        cpu_usage: get_cpu_usage(),
    })
}
```

### 3. Metrics Endpoint

```rust
pub async fn metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
```

## Часто задаваемые вопросы

### Q: Как debug-ить revision creation?

**A:** Включите debug logging:

```rust
use tracing_subscriber::{fmt, EnvFilter};

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

```bash
RUST_LOG=debug ./rustok-revisions
```

### Q: Как восстановить удаленную ревизию?

**A:** Если ревизия была удалена:

1. Восстановите из backup:
```bash
pg_restore -U rustok -d rustok_revisions backup.sql
```

2. Если backup нет, ревизия потеряна навсегда.

### Q: Как экспортировать всю историю?

**A:** Используйте SQL:

```sql
COPY (
    SELECT * FROM content_revisions 
    ORDER BY created_at
) TO '/tmp/revisions_export.csv' WITH CSV HEADER;
```

### Q: Как импортировать данные из другой системы?

**A:** Используйте migration guide:

1. Экспортируйте данные в CSV
2. Трансформируйте в delta format
3. Импортируйте с помощью migration tool

См. `MIGRATION_GUIDE.md` для деталей.

### Q: Как оптимизировать производительность?

**A:** Следуйте best practices:

1. Добавьте индексы
2. Используйте connection pooling
3. Включите кэширование
4. Ограничьте размер выборки

См. `PERFORMANCE_OPTIMIZATION.md` для деталей.

### Q: Как обеспечить безопасность?

**A:** Следуйте security guide:

1. Используйте JWT authentication
2. Проверяйте tenant isolation
3. Валидируйте все inputs
4. Шифруйте чувствительные данные

См. `SECURITY_AUDIT.md` для деталей.

## Заключение

Этот troubleshooting guide покрывает:

✅ **Общие проблемы** — ревизии не создаются, восстановление не работает  
✅ **Проблемы с базой данных** — медленные запросы, ошибки подключения  
✅ **Проблемы производительности** — высокое использование памяти, latency  
✅ **Проблемы с API** — GraphQL errors, subscriptions  
✅ **Проблемы с данными** — неверные deltas, revision numbers  
✅ **Проблемы с миграциями** — миграции не применяются  
✅ **Проблемы с конфигурацией** — переменные окружения  
✅ **Диагностические инструменты** — health check, debug, metrics  
✅ **FAQ** — часто задаваемые вопросы  

**Общий подход к troubleshooting:**

1. **Идентифицируйте проблему** — соберите симптомы
2. **Диагностируйте** — используйте логи, метрики, queries
3. **Найдите причину** — анализируйте данные
4. **Примените решение** — следуйте рекомендациям
5. **Верифицируйте** — убедитесь что проблема решена

**Полезные команды:**

```bash
# Проверить логи
journalctl -u rustok-revisions -f

# Проверить метрики
curl http://localhost:8080/metrics

# Проверить health
curl http://localhost:8080/health

# Проверить базу данных
psql $DATABASE_URL -c "SELECT count(*) FROM content_revisions;"

# Проверить медленные запросы
psql $DATABASE_URL -c "SELECT query, mean_time FROM pg_stat_statements ORDER BY mean_time DESC LIMIT 10;"
```

**Удачи в troubleshooting!** 🔧
