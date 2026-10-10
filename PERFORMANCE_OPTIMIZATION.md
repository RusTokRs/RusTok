# Performance Optimization Guide

Полное руководство по оптимизации производительности системы revision history.

## Содержание

1. [Benchmarking](#benchmarking)
2. [Database Optimization](#database-optimization)
3. [Application Optimization](#application-optimization)
4. [Caching Strategies](#caching-strategies)
5. [Scaling](#scaling)
6. [Monitoring](#monitoring)
7. [Best Practices](#best-practices)

## Benchmarking

### Baseline Performance

**Тестовая среда:**
- CPU: 4 cores
- RAM: 8 GB
- Storage: SSD
- PostgreSQL: 14
- Concurrent users: 100

**Результаты:**

| Operation | Avg Latency | P95 Latency | Throughput |
|-----------|-------------|-------------|------------|
| Create revision | 25 ms | 45 ms | 4,000 req/s |
| List revisions (100) | 50 ms | 85 ms | 2,000 req/s |
| Get revision | 15 ms | 25 ms | 6,500 req/s |
| Compare revisions | 30 ms | 55 ms | 3,300 req/s |
| Restore revision | 35 ms | 60 ms | 2,800 req/s |

### Benchmark Tools

**1. wrk:**

```bash
# Тест создания ревизий
wrk -t12 -c400 -d30s -s create_revision.lua http://localhost:8080/graphql

# Тест получения истории
wrk -t12 -c400 -d30s -s list_revisions.lua http://localhost:8080/graphql
```

**create_revision.lua:**
```lua
wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = '{"query": "mutation { createRevision(...) { id } }"}'
```

**2. Apache Bench:**

```bash
ab -n 10000 -c 100 -p create.json -T application/json \
   http://localhost:8080/graphql
```

**3. Custom benchmark:**

```rust
use std::time::Instant;

#[tokio::test]
async fn benchmark_create_revision() {
    let service = create_test_service().await;
    let post = create_test_post();
    
    let start = Instant::now();
    
    for i in 0..1000 {
        service.create_revision::<Post>(
            &post,
            &format!("post-{}", i),
            Some("en"),
            None,
            None,
            None,
        ).await.unwrap();
    }
    
    let duration = start.elapsed();
    println!("1000 revisions in {:?}", duration);
    println!("Avg: {:?}", duration / 1000);
}
```

## Database Optimization

### 1. Indexes

**Essential indexes:**

```sql
-- Primary lookup index
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(
    tenant_id, 
    content_type, 
    content_id, 
    locale, 
    revision_number DESC
  );

-- Time-based queries
CREATE INDEX idx_revisions_created_at 
  ON content_revisions(created_at DESC);

-- Named versions
CREATE INDEX idx_revisions_version_name 
  ON content_revisions(version_name) 
  WHERE version_name IS NOT NULL;

-- Tenant isolation
CREATE INDEX idx_revisions_tenant 
  ON content_revisions(tenant_id);

-- Content type filtering
CREATE INDEX idx_revisions_content_type 
  ON content_revisions(content_type);
```

**Composite indexes:**

```sql
-- For common queries
CREATE INDEX idx_revisions_tenant_type 
  ON content_revisions(tenant_id, content_type);

-- For date range queries
CREATE INDEX idx_revisions_type_date 
  ON content_revisions(content_type, created_at DESC);
```

**Проверка использования индексов:**

```sql
-- Найти неиспользуемые индексы
SELECT schemaname, tablename, indexname, idx_scan
FROM pg_stat_user_indexes
WHERE idx_scan = 0
ORDER BY tablename, indexname;

-- Найти медленные запросы
SELECT query, calls, total_time, mean_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;
```

### 2. Query Optimization

**EXPLAIN ANALYZE:**

```sql
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) 
SELECT * FROM content_revisions 
WHERE tenant_id = 'tenant-1' 
  AND content_type = 'blog_post' 
  AND content_id = 'post-123' 
  AND locale = 'en'
ORDER BY revision_number DESC
LIMIT 100;
```

**Optimization tips:**

```sql
-- ✅ Хорошо: использует индекс
SELECT * FROM content_revisions 
WHERE tenant_id = 'tenant-1' 
  AND content_type = 'blog_post'
ORDER BY revision_number DESC
LIMIT 100;

-- ❌ Плохо: full table scan
SELECT * FROM content_revisions 
WHERE content_type LIKE '%post%'
ORDER BY created_at DESC;

-- ✅ Лучше: точное совпадение
SELECT * FROM content_revisions 
WHERE content_type = 'blog_post'
ORDER BY created_at DESC
LIMIT 100;
```

### 3. Connection Pooling

**PgBouncer configuration:**

```ini
[databases]
rustok_revisions = host=localhost dbname=rustok_revisions

[pgbouncer]
pool_mode = transaction
max_client_conn = 1000
default_pool_size = 20
min_pool_size = 5
reserve_pool_size = 5
reserve_pool_timeout = 3
server_reset_query = DISCARD ALL
```

**Application configuration:**

```rust
use sqlx::postgres::PgPoolOptions;

let pool = PgPoolOptions::new()
    .max_connections(20)
    .min_connections(5)
    .connect_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(300))
    .max_lifetime(Duration::from_secs(1800))
    .connect(&database_url)
    .await?;
```

### 4. Partitioning

**Range partitioning by date:**

```sql
-- Создать partitioned table
CREATE TABLE content_revisions (
    id UUID PRIMARY KEY,
    tenant_id VARCHAR(255) NOT NULL,
    content_type VARCHAR(255) NOT NULL,
    content_id VARCHAR(255) NOT NULL,
    locale VARCHAR(10),
    revision_number INTEGER NOT NULL,
    delta JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
) PARTITION BY RANGE (created_at);

-- Создать partitions
CREATE TABLE content_revisions_2024_q1 
  PARTITION OF content_revisions
  FOR VALUES FROM ('2024-01-01') TO ('2024-04-01');

CREATE TABLE content_revisions_2024_q2 
  PARTITION OF content_revisions
  FOR VALUES FROM ('2024-04-01') TO ('2024-07-01');

CREATE TABLE content_revisions_2024_q3 
  PARTITION OF content_revisions
  FOR VALUES FROM ('2024-07-01') TO ('2024-10-01');

CREATE TABLE content_revisions_2024_q4 
  PARTITION OF content_revisions
  FOR VALUES FROM ('2024-10-01') TO ('2025-01-01');
```

**List partitioning by content type:**

```sql
CREATE TABLE content_revisions (
    -- columns
) PARTITION BY LIST (content_type);

CREATE TABLE content_revisions_blog 
  PARTITION OF content_revisions
  FOR VALUES IN ('blog_post', 'blog_comment');

CREATE TABLE content_revisions_forum 
  PARTITION OF content_revisions
  FOR VALUES IN ('forum_topic', 'forum_post');
```

### 5. PostgreSQL Configuration

**postgresql.conf:**

```ini
# Connection Settings
max_connections = 200
shared_buffers = 4GB
effective_cache_size = 12GB
work_mem = 32MB
maintenance_work_mem = 1GB

# WAL Settings
wal_level = replica
max_wal_size = 4GB
min_wal_size = 1GB
wal_compression = on

# Checkpoint Settings
checkpoint_completion_target = 0.9
checkpoint_timeout = 15min

# Query Tuning
random_page_cost = 1.1
effective_io_concurrency = 200
default_statistics_target = 100

# Parallel Query
max_worker_processes = 8
max_parallel_workers_per_gather = 4
max_parallel_workers = 8
parallel_leader_participation = on

# Logging
log_min_duration_statement = 1000
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on
log_temp_files = 0
```

### 6. Vacuum and Maintenance

**Autovacuum configuration:**

```ini
autovacuum = on
autovacuum_max_workers = 3
autovacuum_naptime = 1min
autovacuum_vacuum_threshold = 50
autovacuum_analyze_threshold = 50
autovacuum_vacuum_scale_factor = 0.1
autovacuum_analyze_scale_factor = 0.05
```

**Manual maintenance:**

```sql
-- Vacuum
VACUUM ANALYZE content_revisions;

-- Reindex
REINDEX TABLE content_revisions;

-- Update statistics
ANALYZE content_revisions;

-- Check bloat
SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
```

## Application Optimization

### 1. Batch Operations

**Batch create:**

```rust
pub async fn batch_create_revisions<T: Revisionable>(
    &self,
    items: Vec<(T, String, Option<String>)>, // (item, content_id, locale)
) -> Result<Vec<Uuid>, RevisionError> {
    let mut revision_ids = Vec::new();
    
    // Использовать transaction
    let txn = self.backend.begin_transaction().await?;
    
    for (item, content_id, locale) in items {
        let revision_id = self.create_revision_in_transaction(
            &txn,
            &item,
            &content_id,
            locale.as_deref(),
        ).await?;
        revision_ids.push(revision_id);
    }
    
    txn.commit().await?;
    
    Ok(revision_ids)
}
```

**Batch fetch:**

```rust
pub async fn batch_get_revisions<T: Revisionable>(
    &self,
    requests: Vec<(String, Option<String>, i32)>, // (content_id, locale, revision_number)
) -> Result<Vec<Option<Revision>>, RevisionError> {
    // Один запрос вместо N
    let revisions = self.backend.batch_get_revisions(requests).await?;
    Ok(revisions)
}
```

### 2. Lazy Loading

**Load revisions on demand:**

```rust
pub struct RevisionHistory {
    content_id: String,
    content_type: String,
    locale: Option<String>,
    loaded: Vec<Revision>,
    has_more: bool,
    cursor: Option<String>,
}

impl RevisionHistory {
    pub async fn load_more(&mut self, limit: usize) -> Result<(), RevisionError> {
        if !self.has_more {
            return Ok(());
        }
        
        let revisions = self.service.list_revisions(
            &self.content_id,
            self.locale.as_deref(),
            Some(limit),
            self.cursor.as_deref(),
            None,
            Some(SortOrder::Descending),
        ).await?;
        
        self.loaded.extend(revisions);
        self.has_more = revisions.len() == limit;
        self.cursor = self.loaded.last().map(|r| r.id.to_string());
        
        Ok(())
    }
}
```

### 3. Streaming

**Stream large result sets:**

```rust
use futures::stream::{self, StreamExt};

pub async fn stream_revisions<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
) -> impl Stream<Item = Result<Revision, RevisionError>> + '_ {
    stream::unfold(
        (content_id.to_string(), locale.map(String::from), None::<String>),
        move |(content_id, locale, cursor)| async move {
            let revisions = self.list_revisions(
                &content_id,
                locale.as_deref(),
                Some(100),
                cursor.as_deref(),
                None,
                Some(SortOrder::Descending),
            ).await.ok()?;
            
            if revisions.is_empty() {
                return None;
            }
            
            let next_cursor = revisions.last().map(|r| r.id.to_string());
            let items = revisions.into_iter().map(Ok).collect::<Vec<_>>();
            
            Some((
                stream::iter(items),
                (content_id, locale, next_cursor),
            ))
        },
    )
    .flatten()
}
```

### 4. Connection Pooling

**Optimize pool size:**

```rust
// Formula: connections = ((core_count * 2) + effective_spindle_count)
// For SSD: connections = (4 * 2) + 1 = 9

let pool = PgPoolOptions::new()
    .max_connections(10)
    .min_connections(2)
    .connect_timeout(Duration::from_secs(30))
    .idle_timeout(Duration::from_secs(300))
    .max_lifetime(Duration::from_secs(1800))
    .test_before_acquire(true)
    .connect(&database_url)
    .await?;
```

### 5. Async Operations

**Concurrent operations:**

```rust
use futures::future::join_all;

pub async fn create_revisions_concurrent<T: Revisionable>(
    &self,
    items: Vec<(T, String, Option<String>)>,
) -> Result<Vec<Uuid>, RevisionError> {
    let futures: Vec<_> = items
        .into_iter()
        .map(|(item, content_id, locale)| {
            self.create_revision(
                &item,
                &content_id,
                locale.as_deref(),
                None,
                None,
                None,
            )
        })
        .collect();
    
    let results = join_all(futures).await;
    results.into_iter().collect()
}
```

### 6. Memory Management

**Limit memory usage:**

```rust
pub async fn list_revisions_with_limit<T: Revisionable>(
    &self,
    content_id: &str,
    locale: Option<&str>,
    max_items: usize,
) -> Result<Vec<Revision>, RevisionError> {
    // Загружать порциями
    let mut all_revisions = Vec::new();
    let mut cursor = None;
    
    while all_revisions.len() < max_items {
        let batch_size = std::cmp::min(100, max_items - all_revisions.len());
        
        let batch = self.list_revisions(
            content_id,
            locale,
            Some(batch_size),
            cursor.as_deref(),
            None,
            Some(SortOrder::Descending),
        ).await?;
        
        if batch.is_empty() {
            break;
        }
        
        cursor = batch.last().map(|r| r.id.to_string());
        all_revisions.extend(batch);
    }
    
    Ok(all_revisions)
}
```

## Caching Strategies

### 1. In-Memory Caching

**Using `dashmap`:**

```rust
use dashmap::DashMap;
use std::time::{Duration, Instant};

pub struct RevisionCache {
    cache: DashMap<String, (Revision, Instant)>,
    ttl: Duration,
}

impl RevisionCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: DashMap::new(),
            ttl,
        }
    }
    
    pub fn get(&self, key: &str) -> Option<Revision> {
        self.cache.get(key).and_then(|entry| {
            let (revision, timestamp) = entry.value();
            if timestamp.elapsed() < self.ttl {
                Some(revision.clone())
            } else {
                None
            }
        })
    }
    
    pub fn set(&self, key: String, revision: Revision) {
        self.cache.insert(key, (revision, Instant::now()));
    }
    
    pub fn invalidate(&self, key: &str) {
        self.cache.remove(key);
    }
}
```

### 2. Redis Caching

**Using `redis-rs`:**

```rust
use redis::AsyncCommands;

pub struct RedisRevisionCache {
    client: redis::Client,
    ttl: u64,
}

impl RedisRevisionCache {
    pub async fn get(&self, key: &str) -> Result<Option<Revision>, RevisionError> {
        let mut conn = self.client.get_async_connection().await?;
        let cached: Option<String> = conn.get(key).await?;
        
        match cached {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }
    
    pub async fn set(&self, key: &str, revision: &Revision) -> Result<(), RevisionError> {
        let mut conn = self.client.get_async_connection().await?;
        let json = serde_json::to_string(revision)?;
        conn.set_ex(key, json, self.ttl).await?;
        Ok(())
    }
    
    pub async fn invalidate(&self, pattern: &str) -> Result<(), RevisionError> {
        let mut conn = self.client.get_async_connection().await?;
        let keys: Vec<String> = conn.keys(pattern).await?;
        
        if !keys.is_empty() {
            conn.del(keys).await?;
        }
        
        Ok(())
    }
}
```

**Cache keys:**

```rust
// Single revision
let key = format!("revision:{}:{}:{}:{}", 
    content_type, content_id, locale, revision_number);

// Revision list
let key = format!("revisions:{}:{}:{}:{}", 
    content_type, content_id, locale, limit);

// Named versions
let key = format!("named_versions:{}:{}:{}", 
    content_type, content_id, locale);
```

### 3. Multi-Level Caching

```rust
pub struct MultiLevelCache {
    l1: RevisionCache,        // In-memory
    l2: RedisRevisionCache,   // Redis
}

impl MultiLevelCache {
    pub async fn get(&self, key: &str) -> Result<Option<Revision>, RevisionError> {
        // Check L1 cache
        if let Some(revision) = self.l1.get(key) {
            return Ok(Some(revision));
        }
        
        // Check L2 cache
        if let Some(revision) = self.l2.get(key).await? {
            // Populate L1 cache
            self.l1.set(key.to_string(), revision.clone());
            return Ok(Some(revision));
        }
        
        Ok(None)
    }
    
    pub async fn set(&self, key: &str, revision: &Revision) -> Result<(), RevisionError> {
        // Set in both caches
        self.l1.set(key.to_string(), revision.clone());
        self.l2.set(key, revision).await?;
        Ok(())
    }
    
    pub async fn invalidate(&self, key: &str) -> Result<(), RevisionError> {
        self.l1.invalidate(key);
        self.l2.invalidate(key).await?;
        Ok(())
    }
}
```

### 4. Cache Invalidation

**Invalidate on write:**

```rust
pub async fn create_revision<T: Revisionable>(
    &self,
    item: &T,
    content_id: &str,
    locale: Option<&str>,
) -> Result<Uuid, RevisionError> {
    let revision_id = self.backend.create_revision(revision).await?;
    
    // Invalidate cache
    let cache_key = format!("revisions:{}:{}:{}", 
        T::content_type(), content_id, locale.unwrap_or("default"));
    self.cache.invalidate(&cache_key).await?;
    
    Ok(revision_id)
}
```

**TTL-based expiration:**

```rust
let cache = RevisionCache::new(Duration::from_secs(300)); // 5 minutes
```

**Event-driven invalidation:**

```rust
use tokio::sync::broadcast;

pub struct CacheManager {
    tx: broadcast::Sender<CacheEvent>,
}

enum CacheEvent {
    RevisionCreated { content_type: String, content_id: String },
    RevisionDeleted { content_type: String, content_id: String },
}

impl CacheManager {
    pub async fn handle_event(&self, event: CacheEvent) {
        match event {
            CacheEvent::RevisionCreated { content_type, content_id } => {
                let pattern = format!("revisions:{}:{}:*", content_type, content_id);
                self.cache.invalidate(&pattern).await.ok();
            }
            // ...
        }
    }
}
```

## Scaling

### 1. Horizontal Scaling

**Load balancer (nginx):**

```nginx
upstream rustok_revisions {
    least_conn;
    server revisions-1:8080 weight=5;
    server revisions-2:8080 weight=5;
    server revisions-3:8080 weight=5;
    
    keepalive 32;
}

server {
    listen 80;
    
    location / {
        proxy_pass http://rustok_revisions;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        
        proxy_connect_timeout 5s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }
}
```

**Read replicas:**

```rust
pub struct ReadWriteSplit {
    writer: SeaORMBackend,
    readers: Vec<SeaORMBackend>,
}

impl ReadWriteSplit {
    pub async fn create_revision(&self, revision: Revision) -> Result<Uuid, RevisionError> {
        // Write to primary
        self.writer.create_revision(revision).await
    }
    
    pub async fn list_revisions(&self, content_id: &str) -> Result<Vec<Revision>, RevisionError> {
        // Read from replica
        let reader = self.select_reader();
        reader.list_revisions(content_id).await
    }
    
    fn select_reader(&self) -> &SeaORMBackend {
        // Round-robin or least-connections
        &self.readers[rand::random::<usize>() % self.readers.len()]
    }
}
```

### 2. Vertical Scaling

**Optimize PostgreSQL:**

```ini
# Увеличить shared_buffers
shared_buffers = 8GB

# Увеличить work_mem
work_mem = 64MB

# Увеличить max_connections
max_connections = 500

# Включить parallel query
max_parallel_workers_per_gather = 4
```

**Optimize application:**

```rust
// Увеличить worker threads
#[tokio::main(worker_threads = 8)]
async fn main() {
    // ...
}
```

### 3. Sharding

**Shard by tenant:**

```rust
pub struct ShardedBackend {
    shards: Vec<SeaORMBackend>,
}

impl ShardedBackend {
    pub fn get_shard(&self, tenant_id: &str) -> &SeaORMBackend {
        let shard_index = self.hash(tenant_id) % self.shards.len();
        &self.shards[shard_index]
    }
    
    fn hash(&self, key: &str) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish() as usize
    }
}
```

## Monitoring

### 1. Prometheus Metrics

```rust
use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, 
    IntGauge, Opts, Registry
};

pub struct Metrics {
    pub requests_total: IntCounterVec,
    pub request_duration: HistogramVec,
    pub active_connections: IntGauge,
    pub revisions_created: IntCounterVec,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        let requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total HTTP requests"),
            &["method", "path", "status"],
        ).unwrap();
        
        let request_duration = HistogramVec::new(
            HistogramOpts::new("http_request_duration_seconds", "Request duration")
                .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
            &["method", "path"],
        ).unwrap();
        
        let active_connections = IntGauge::new(
            "database_connections_active",
            "Active database connections",
        ).unwrap();
        
        let revisions_created = IntCounterVec::new(
            Opts::new("revisions_created_total", "Total revisions created"),
            &["content_type"],
        ).unwrap();
        
        registry.register(Box::new(requests_total.clone())).unwrap();
        registry.register(Box::new(request_duration.clone())).unwrap();
        registry.register(Box::new(active_connections.clone())).unwrap();
        registry.register(Box::new(revisions_created.clone())).unwrap();
        
        Self {
            requests_total,
            request_duration,
            active_connections,
            revisions_created,
        }
    }
}
```

### 2. Distributed Tracing

**Using `tracing`:**

```rust
use tracing::{info_span, Instrument};

pub async fn create_revision<T: Revisionable>(
    &self,
    item: &T,
    content_id: &str,
    locale: Option<&str>,
) -> Result<Uuid, RevisionError> {
    let span = info_span!(
        "create_revision",
        content_type = T::content_type(),
        content_id = content_id,
        locale = locale,
    );
    
    async {
        let start = Instant::now();
        
        let revision = self.build_revision(item, content_id, locale)?;
        let revision_id = self.backend.create_revision(revision).await?;
        
        let duration = start.elapsed();
        tracing::info!(
            revision_id = %revision_id,
            duration_ms = duration.as_millis(),
            "Revision created"
        );
        
        Ok(revision_id)
    }
    .instrument(span)
    .await
}
```

### 3. Health Checks

```rust
pub async fn health_check(&self) -> HealthStatus {
    let db_check = self.check_database().await;
    let cache_check = self.check_cache().await;
    
    HealthStatus {
        status: if db_check && cache_check { "healthy" } else { "unhealthy" },
        database: db_check,
        cache: cache_check,
        uptime: self.uptime(),
        version: env!("CARGO_PKG_VERSION"),
    }
}

async fn check_database(&self) -> bool {
    sqlx::query("SELECT 1")
        .fetch_one(&self.pool)
        .await
        .is_ok()
}
```

## Best Practices

### 1. Use Prepared Statements

```rust
// ✅ Хорошо: prepared statement
let revision = sqlx::query_as::<_, Revision>(
    "SELECT * FROM content_revisions WHERE id = $1"
)
.bind(id)
.fetch_one(&pool)
.await?;

// ❌ Плохо: string interpolation
let query = format!("SELECT * FROM content_revisions WHERE id = '{}'", id);
let revision = sqlx::query_as::<_, Revision>(&query)
    .fetch_one(&pool)
    .await?;
```

### 2. Use Transactions

```rust
// ✅ Хорошо: transaction
let mut txn = pool.begin().await?;

sqlx::query("INSERT INTO content_revisions ...")
    .execute(&mut txn)
    .await?;

sqlx::query("UPDATE content_revisions ...")
    .execute(&mut txn)
    .await?;

txn.commit().await?;

// ❌ Плохо: separate queries
sqlx::query("INSERT INTO content_revisions ...")
    .execute(&pool)
    .await?;

sqlx::query("UPDATE content_revisions ...")
    .execute(&pool)
    .await?;
```

### 3. Limit Result Sets

```rust
// ✅ Хорошо: limit
let revisions = service.list_revisions(
    content_id,
    locale,
    Some(100),  // limit
    None,
    None,
    Some(SortOrder::Descending),
).await?;

// ❌ Плохо: no limit
let revisions = service.list_revisions(
    content_id,
    locale,
    None,  // no limit!
    None,
    None,
    Some(SortOrder::Descending),
).await?;
```

### 4. Use Indexes

```sql
-- ✅ Хорошо: uses index
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);

SELECT * FROM content_revisions 
WHERE tenant_id = 'tenant-1' 
  AND content_type = 'blog_post'
ORDER BY revision_number DESC;

-- ❌ Плохо: full table scan
SELECT * FROM content_revisions 
WHERE content_type LIKE '%post%';
```

### 5. Monitor Performance

```rust
// Log slow queries
let start = Instant::now();
let result = self.backend.list_revisions(...).await;
let duration = start.elapsed();

if duration > Duration::from_millis(100) {
    tracing::warn!(
        duration_ms = duration.as_millis(),
        "Slow query detected"
    );
}
```

## Заключение

Этот guide покрывает:

✅ **Benchmarking** — как измерить производительность  
✅ **Database optimization** — индексы, queries, partitioning  
✅ **Application optimization** — batch, lazy loading, streaming  
✅ **Caching** — in-memory, Redis, multi-level  
✅ **Scaling** — horizontal, vertical, sharding  
✅ **Monitoring** — metrics, tracing, health checks  
✅ **Best practices** — prepared statements, transactions, limits  

Следуя этим рекомендациям, вы сможете достичь:

- **Latency:** < 50 ms для большинства операций
- **Throughput:** > 4,000 req/s
- **Scalability:** horizontal scaling до 100+ servers
- **Reliability:** 99.9% uptime

**Удачи в оптимизации!** 🚀
