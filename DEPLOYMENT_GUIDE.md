# Deployment Guide

Полное руководство по развертыванию системы revision history в production.

## Содержание

1. [Требования](#требования)
2. [Подготовка](#подготовка)
3. [Установка](#установка)
4. [Конфигурация](#конфигурация)
5. [Миграции](#миграции)
6. [Запуск](#запуск)
7. [Мониторинг](#мониторинг)
8. [Backup & Recovery](#backup--recovery)
9. [Scaling](#scaling)
10. [Troubleshooting](#troubleshooting)

## Требования

### Системные требования

**Минимальные:**
- CPU: 2 cores
- RAM: 4 GB
- Storage: 20 GB
- PostgreSQL: 12+

**Рекомендуемые:**
- CPU: 4+ cores
- RAM: 8+ GB
- Storage: 100+ GB SSD
- PostgreSQL: 14+

### Зависимости

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# PostgreSQL
sudo apt-get install postgresql postgresql-contrib

# Дополнительные инструменты
sudo apt-get install build-essential pkg-config libssl-dev
```

## Подготовка

### 1. Создание базы данных

```bash
# Подключиться к PostgreSQL
sudo -u postgres psql

# Создать базу данных
CREATE DATABASE rustok_revisions;

# Создать пользователя
CREATE USER rustok WITH ENCRYPTED PASSWORD 'your_secure_password';

# Дать права
GRANT ALL PRIVILEGES ON DATABASE rustok_revisions TO rustok;

# Выйти
\q
```

### 2. Настройка окружения

Создайте файл `.env`:

```bash
# Database
DATABASE_URL=postgres://rustok:your_secure_password@localhost:5432/rustok_revisions

# Application
RUST_ENV=production
RUST_LOG=info

# Performance
MAX_CONNECTIONS=20
MIN_CONNECTIONS=5
CONNECT_TIMEOUT=30

# Retention
DEFAULT_RETENTION_DAYS=90
MAX_REVISIONS_PER_ITEM=100

# Security
JWT_SECRET=your_jwt_secret_here
ENCRYPTION_KEY=your_encryption_key_here
```

### 3. Настройка PostgreSQL

Отредактируйте `postgresql.conf`:

```ini
# Connection Settings
max_connections = 200
shared_buffers = 2GB
effective_cache_size = 6GB
work_mem = 16MB
maintenance_work_mem = 512MB

# WAL Settings
wal_level = replica
max_wal_size = 2GB
min_wal_size = 1GB

# Checkpoint Settings
checkpoint_completion_target = 0.9
checkpoint_timeout = 15min

# Query Tuning
random_page_cost = 1.1
effective_io_concurrency = 200

# Logging
log_min_duration_statement = 1000
log_checkpoints = on
log_connections = on
log_disconnections = on
log_lock_waits = on
log_temp_files = 0
```

Перезапустите PostgreSQL:

```bash
sudo systemctl restart postgresql
```

## Установка

### 1. Клонирование репозитория

```bash
git clone https://github.com/rustok/rustok-revisions.git
cd rustok-revisions
```

### 2. Сборка проекта

```bash
# Development build
cargo build

# Production build (оптимизированный)
cargo build --release

# С всеми features
cargo build --release --all-features
```

### 3. Установка binary

```bash
# Копировать binary
sudo cp target/release/rustok-revisions /usr/local/bin/

# Сделать исполняемым
sudo chmod +x /usr/local/bin/rustok-revisions

# Проверить установку
rustok-revisions --version
```

## Конфигурация

### 1. Application Config

Создайте `config/production.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 4

[database]
url = "postgres://rustok:password@localhost:5432/rustok_revisions"
max_connections = 20
min_connections = 5
connect_timeout = 30

[retention]
default_policy = "keep_last"
default_keep_last = 100
default_keep_days = 90
cleanup_interval_hours = 24

[logging]
level = "info"
format = "json"
output = "stdout"

[metrics]
enabled = true
endpoint = "/metrics"

[security]
jwt_secret = "${JWT_SECRET}"
encryption_key = "${ENCRYPTION_KEY}"
rate_limit = 1000
```

### 2. Environment Variables

```bash
export DATABASE_URL="postgres://rustok:password@localhost:5432/rustok_revisions"
export RUST_ENV="production"
export RUST_LOG="info"
export JWT_SECRET="your_jwt_secret"
export ENCRYPTION_KEY="your_encryption_key"
```

### 3. Systemd Service

Создайте `/etc/systemd/system/rustok-revisions.service`:

```ini
[Unit]
Description=RusTok Revisions Service
After=network.target postgresql.service

[Service]
Type=simple
User=rustok
Group=rustok
WorkingDirectory=/opt/rustok-revisions
EnvironmentFile=/opt/rustok-revisions/.env
ExecStart=/usr/local/bin/rustok-revisions --config config/production.toml
Restart=always
RestartSec=10

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

Активируйте сервис:

```bash
sudo systemctl daemon-reload
sudo systemctl enable rustok-revisions
sudo systemctl start rustok-revisions
```

## Миграции

### 1. Запуск миграций

```bash
# Применить все миграции
rustok-revisions migrate up

# Применить конкретную миграцию
rustok-revisions migrate up --to 20240101000000

# Откатить последнюю миграцию
rustok-revisions migrate down

# Показать статус миграций
rustok-revisions migrate status
```

### 2. Проверка схемы

```bash
# Подключиться к базе
psql $DATABASE_URL

# Проверить таблицы
\dt

# Проверить индексы
\di

# Проверить constraints
\d content_revisions
```

### 3. Создание дополнительных индексов

```sql
-- Индекс для быстрого поиска по tenant
CREATE INDEX idx_revisions_tenant ON content_revisions(tenant_id);

-- Индекс для быстрого поиска по дате
CREATE INDEX idx_revisions_created_at ON content_revisions(created_at DESC);

-- Составной индекс для частых запросов
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);

-- Индекс для named versions
CREATE INDEX idx_revisions_version_name 
  ON content_revisions(version_name) 
  WHERE version_name IS NOT NULL;
```

## Запуск

### 1. Запуск сервиса

```bash
# Запустить через systemd
sudo systemctl start rustok-revisions

# Проверить статус
sudo systemctl status rustok-revisions

# Просмотреть логи
sudo journalctl -u rustok-revisions -f
```

### 2. Проверка работоспособности

```bash
# Health check
curl http://localhost:8080/health

# Metrics
curl http://localhost:8080/metrics

# GraphQL playground
open http://localhost:8080/graphql
```

### 3. Тестирование

```bash
# Создать тестовую ревизию
curl -X POST http://localhost:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "mutation { createRevision(input: { contentId: \"test\", contentType: \"test\", locale: \"en\" }) { id revisionNumber } }"
  }'

# Получить список ревизий
curl -X POST http://localhost:8080/graphql \
  -H "Content-Type: application/json" \
  -d '{
    "query": "query { revisions(contentId: \"test\", contentType: \"test\", locale: \"en\") { edges { node { revisionNumber } } } }"
  }'
```

## Мониторинг

### 1. Prometheus Metrics

Добавьте в `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'rustok-revisions'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

**Доступные метрики:**

```
# Request metrics
http_requests_total{method, path, status}
http_request_duration_seconds{method, path}

# Database metrics
database_connections_active
database_connections_idle
database_query_duration_seconds{query_type}

# Revision metrics
revisions_created_total{content_type}
revisions_restored_total{content_type}
revisions_deleted_total{content_type}

# Performance metrics
revision_creation_duration_seconds
revision_list_duration_seconds
revision_diff_duration_seconds
```

### 2. Grafana Dashboard

Создайте dashboard с панелями:

**Request Rate:**
```promql
rate(http_requests_total[5m])
```

**Error Rate:**
```promql
rate(http_requests_total{status=~"5.."}[5m])
```

**Latency:**
```promql
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))
```

**Active Connections:**
```promql
database_connections_active
```

**Revisions Created:**
```promql
rate(revisions_created_total[1h])
```

### 3. Alerting Rules

Создайте `alerts.yml`:

```yaml
groups:
  - name: rustok-revisions
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors per second"

      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High latency detected"
          description: "95th percentile latency is {{ $value }}s"

      - alert: DatabaseConnectionPoolExhausted
        expr: database_connections_active / database_connections_max > 0.9
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Database connection pool nearly exhausted"
          description: "{{ $value | humanizePercentage }} of connections are in use"
```

### 4. Logging

Настройте структурированное логирование:

```rust
use tracing_subscriber::{fmt, EnvFilter};

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .json()
    .init();
```

**Пример лога:**

```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "level": "INFO",
  "message": "Revision created",
  "revision_id": "550e8400-e29b-41d4-a716-446655440000",
  "content_type": "blog_post",
  "content_id": "123",
  "revision_number": 5,
  "duration_ms": 45,
  "user_id": "456"
}
```

## Backup & Recovery

### 1. Автоматический backup

Создайте скрипт `/opt/scripts/backup-revisions.sh`:

```bash
#!/bin/bash

BACKUP_DIR="/backups/revisions"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/revisions_$DATE.sql"

# Создать директорию
mkdir -p $BACKUP_DIR

# Создать backup
pg_dump -U rustok -d rustok_revisions -F c -f $BACKUP_FILE

# Сжать
gzip $BACKUP_FILE

# Удалить старые backups (старше 30 дней)
find $BACKUP_DIR -name "revisions_*.sql.gz" -mtime +30 -delete

# Логировать
echo "Backup created: $BACKUP_FILE.gz" >> /var/log/revisions-backup.log
```

Добавьте в crontab:

```bash
# Backup каждый день в 2:00
0 2 * * * /opt/scripts/backup-revisions.sh
```

### 2. Восстановление из backup

```bash
# Остановить сервис
sudo systemctl stop rustok-revisions

# Восстановить базу
pg_restore -U rustok -d rustok_revisions -c /backups/revisions/revisions_20240115_020000.sql.gz

# Запустить сервис
sudo systemctl start rustok-revisions
```

### 3. Point-in-Time Recovery

Настройте WAL archiving:

```ini
# postgresql.conf
wal_level = replica
archive_mode = on
archive_command = 'cp %p /archive/wal/%f'
```

Восстановление:

```bash
# Остановить PostgreSQL
sudo systemctl stop postgresql

# Восстановить base backup
pg_restore -U rustok -d rustok_revisions /backups/base/base_20240115.tar

# Создать recovery.conf
cat > /var/lib/postgresql/14/main/recovery.conf <<EOF
restore_command = 'cp /archive/wal/%f %p'
recovery_target_time = '2024-01-15 10:30:00'
EOF

# Запустить PostgreSQL
sudo systemctl start postgresql
```

## Scaling

### 1. Horizontal Scaling

**Load Balancer (nginx):**

```nginx
upstream rustok_revisions {
    least_conn;
    server revisions-1:8080;
    server revisions-2:8080;
    server revisions-3:8080;
}

server {
    listen 80;
    server_name revisions.example.com;

    location / {
        proxy_pass http://rustok_revisions;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

**Database Replication:**

```bash
# Настроить streaming replication
# Master: postgresql.conf
wal_level = replica
max_wal_senders = 3

# Replica: recovery.conf
primary_conninfo = 'host=master port=5432 user=replicator password=password'
```

### 2. Vertical Scaling

**Оптимизация PostgreSQL:**

```ini
# Увеличить shared_buffers
shared_buffers = 8GB

# Увеличить effective_cache_size
effective_cache_size = 24GB

# Увеличить work_mem
work_mem = 64MB

# Увеличить max_connections
max_connections = 500
```

**Оптимизация приложения:**

```toml
[server]
workers = 8

[database]
max_connections = 50
min_connections = 10
```

### 3. Caching

**Redis для кэширования:**

```rust
use redis::Client;

let client = Client::open("redis://localhost:6379")?;
let mut conn = client.get_connection()?;

// Кэшировать список ревизий
let cache_key = format!("revisions:{}:{}:{}", content_type, content_id, locale);
let cached: Option<String> = conn.get(&cache_key)?;

if let Some(cached) = cached {
    return Ok(serde_json::from_str(&cached)?);
}

// Получить из базы
let revisions = db.list_revisions(...).await?;

// Сохранить в кэш
conn.set_ex(&cache_key, serde_json::to_string(&revisions)?, 300)?;

Ok(revisions)
```

## Troubleshooting

### 1. Проблема: Медленные запросы

**Диагностика:**

```sql
-- Найти медленные запросы
SELECT query, calls, total_time, mean_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;

-- Проверить индексы
EXPLAIN ANALYZE SELECT * FROM content_revisions 
WHERE tenant_id = '...' AND content_type = '...' AND content_id = '...';
```

**Решение:**

```sql
-- Добавить недостающие индексы
CREATE INDEX idx_revisions_lookup 
  ON content_revisions(tenant_id, content_type, content_id, locale, revision_number DESC);

-- Обновить статистику
ANALYZE content_revisions;
```

### 2. Проблема: Высокое использование памяти

**Диагностика:**

```bash
# Проверить использование памяти
ps aux | grep rustok-revisions

# Проверить утечки
valgrind --leak-check=full ./rustok-revisions
```

**Решение:**

```toml
# Уменьшить размер connection pool
[database]
max_connections = 10

# Включить garbage collection
[server]
gc_interval_seconds = 300
```

### 3. Проблема: Ошибки подключения к базе

**Диагностика:**

```bash
# Проверить подключение
psql $DATABASE_URL

# Проверить логи PostgreSQL
sudo journalctl -u postgresql -f

# Проверить лимит подключений
SELECT count(*) FROM pg_stat_activity;
```

**Решение:**

```ini
# postgresql.conf
max_connections = 500

# Application config
[database]
max_connections = 20
connect_timeout = 30
```

### 4. Проблема: Миграции не применяются

**Диагностика:**

```bash
# Проверить статус миграций
rustok-revisions migrate status

# Проверить таблицу миграций
psql $DATABASE_URL -c "SELECT * FROM seaql_migrations;"
```

**Решение:**

```bash
# Откатить последнюю миграцию
rustok-revisions migrate down

# Применить заново
rustok-revisions migrate up

# Или принудительно применить
rustok-revisions migrate up --force
```

### 5. Проблема: Retention policy не работает

**Диагностика:**

```bash
# Проверить логи cleanup
journalctl -u rustok-revisions | grep cleanup

# Проверить количество ревизий
psql $DATABASE_URL -c "SELECT content_type, count(*) FROM content_revisions GROUP BY content_type;"
```

**Решение:**

```bash
# Запустить cleanup вручную
rustok-revisions cleanup --dry-run

# Применить cleanup
rustok-revisions cleanup

# Проверить cron job
crontab -l
```

## Performance Tuning

### 1. Database Optimization

```sql
-- Vacuum и analyze
VACUUM ANALYZE content_revisions;

-- Reindex
REINDEX TABLE content_revisions;

-- Обновить статистику
ANALYZE content_revisions;
```

### 2. Application Optimization

```toml
[performance]
# Включить batch processing
batch_size = 100

# Включить connection pooling
pool_mode = "transaction"

# Включить query caching
cache_enabled = true
cache_ttl_seconds = 300
```

### 3. OS Optimization

```bash
# Увеличить file descriptors
ulimit -n 65536

# Оптимизировать TCP
sysctl -w net.core.somaxconn=65535
sysctl -w net.ipv4.tcp_max_syn_backlog=65535

# Оптимизировать память
sysctl -w vm.swappiness=10
sysctl -w vm.dirty_ratio=15
```

## Security

### 1. Firewall Rules

```bash
# Разрешить только необходимые порты
ufw allow 22/tcp    # SSH
ufw allow 80/tcp    # HTTP
ufw allow 443/tcp   # HTTPS
ufw allow 5432/tcp  # PostgreSQL (только для internal network)

ufw enable
```

### 2. SSL/TLS

```nginx
server {
    listen 443 ssl http2;
    server_name revisions.example.com;

    ssl_certificate /etc/letsencrypt/live/revisions.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/revisions.example.com/privkey.pem;

    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    location / {
        proxy_pass http://localhost:8080;
    }
}
```

### 3. Rate Limiting

```rust
use tower::limit::RateLimitLayer;

let app = Router::new()
    .route("/graphql", post(graphql_handler))
    .layer(RateLimitLayer::new(100, Duration::from_secs(60)));
```

## Заключение

Этот deployment guide покрывает:

✅ **Установка** — пошаговая инструкция  
✅ **Конфигурация** — все настройки  
✅ **Мониторинг** — metrics и alerting  
✅ **Backup** — автоматический backup и recovery  
✅ **Scaling** — horizontal и vertical scaling  
✅ **Troubleshooting** — решение типичных проблем  
✅ **Security** — firewall, SSL, rate limiting  

Следуя этому guide, вы сможете развернуть production-ready систему revision history!
