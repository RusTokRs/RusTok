# rustok-revisions-monitoring

Monitoring and health check server for rustok-revisions.

## Features

- ✅ Health check endpoint with system metrics
- ✅ Prometheus metrics exporter
- ✅ Web dashboard
- ✅ System resource monitoring (CPU, memory)
- ✅ Database connection monitoring
- ✅ Revision statistics

## Installation

```bash
cargo build --release
./target/release/revisions-monitor
```

Or with Docker:

```bash
docker build -t revisions-monitor -f Dockerfile.monitor .
docker run -p 8080:8080 -e DATABASE_URL="postgres://..." revisions-monitor
```

## Usage

### Environment Variables

- `DATABASE_URL` - PostgreSQL connection string (required)
- `RUST_LOG` - Log level (default: info)

### Running

```bash
export DATABASE_URL="postgres://user:pass@localhost/rustok_revisions"
cargo run
```

The server will start on `http://localhost:8080`.

## Endpoints

### `/health` - Health Check

Returns JSON with system and database health status.

**Example:**
```bash
curl http://localhost:8080/health
```

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "database": {
    "connected": true,
    "revision_count": 12345,
    "oldest_revision": "2024-01-01T00:00:00Z",
    "newest_revision": "2024-01-15T14:30:00Z"
  },
  "system": {
    "cpu_usage": 15.5,
    "memory_used_mb": 256,
    "memory_total_mb": 8192
  }
}
```

### `/metrics` - Prometheus Metrics

Returns metrics in Prometheus format.

**Example:**
```bash
curl http://localhost:8080/metrics
```

**Metrics:**
- `revisions_total` - Total number of revisions by content type and event
- `revisions_size_bytes` - Total size of revisions in bytes
- `revisions_count` - Total number of revisions
- `database_connections_active` - Active database connections
- `request_duration_seconds` - Request duration histogram

### `/dashboard` - Web Dashboard

HTML dashboard with real-time statistics.

**Access:** Open `http://localhost:8080/dashboard` in your browser.

## Prometheus Integration

### Prometheus Configuration

Add to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'rustok-revisions'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### Grafana Dashboard

Import the provided Grafana dashboard JSON or create your own panels:

**Key Panels:**
- Total revisions over time
- Revisions by content type
- Request duration percentiles
- System resource usage
- Database connection pool

## Docker Compose

```yaml
version: '3.8'

services:
  revisions-monitor:
    build:
      context: .
      dockerfile: Dockerfile.monitor
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: postgres://rustok:rustok_password@postgres:5432/rustok_revisions
    depends_on:
      - postgres

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    depends_on:
      - revisions-monitor

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin
    volumes:
      - grafana_data:/var/lib/grafana

volumes:
  grafana_data:
```

## Health Check Integration

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: rustok-revisions
spec:
  template:
    spec:
      containers:
      - name: revisions
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 10
```

### Docker

```yaml
services:
  app:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

## Alerting

### Prometheus Alert Rules

```yaml
groups:
  - name: rustok-revisions
    rules:
      - alert: HighRevisionCount
        expr: revisions_count > 1000000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High revision count"
          description: "Total revisions: {{ $value }}"

      - alert: DatabaseDisconnected
        expr: up{job="rustok-revisions"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Database disconnected"

      - alert: HighMemoryUsage
        expr: system_memory_used_mb / system_memory_total_mb > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
```

## API Documentation

### Health Check Response Schema

```typescript
interface HealthResponse {
  status: "healthy" | "degraded" | "unhealthy";
  version: string;
  uptime_seconds: number;
  database: {
    connected: boolean;
    revision_count: number;
    oldest_revision?: string; // ISO 8601
    newest_revision?: string; // ISO 8601
  };
  system: {
    cpu_usage: number; // 0-100
    memory_used_mb: number;
    memory_total_mb: number;
  };
}
```

## Troubleshooting

### Server won't start

**Problem:** Database connection error

**Solution:** Check `DATABASE_URL` environment variable
```bash
export DATABASE_URL="postgres://user:pass@localhost/rustok_revisions"
```

### Metrics not updating

**Problem:** Prometheus not scraping

**Solution:** Check Prometheus configuration and ensure `/metrics` endpoint is accessible
```bash
curl http://localhost:8080/metrics
```

### High memory usage

**Problem:** Too many revisions in memory

**Solution:** Implement pagination and limit queries in health checks

## License

MIT OR Apache-2.0
