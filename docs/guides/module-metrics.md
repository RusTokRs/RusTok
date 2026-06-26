---
id: doc://docs/guides/module-metrics.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Метрики модулей

Этот документ фиксирует актуальный базовый набор Prometheus-метрик для модулей
RusToK. Источником истины по именам и labels остаётся
`crates/rustok-telemetry/src/metrics.rs`; этот guide описывает, как ими
пользоваться и что считать минимальным operational baseline.

## Базовый набор

### Входные точки модулей

```
rustok_module_entrypoint_calls_total{module,entry_point,path}
```

- `module` — slug модуля, например `rbac` или `comments`
- `entry_point` — имя публичной операции
- `path` — путь интеграции: `library`, `core_runtime`, `bypass`

Эта метрика нужна, чтобы видеть, через какой контракт реально ходит runtime и
не вернулся ли legacy bypass.

### Ошибки модулей

```
rustok_module_errors_total{module,error_type,severity}
```

- `error_type` — короткий стабильный класс ошибки (`database`, `validation`,
  `forbidden`, `not_found`)
- `severity` — операторская важность (`warning`, `error`)

Используйте только низкокардинальные значения. Нельзя передавать request id,
tenant slug, user id или raw error message.

### Длительность и ошибки операций

```
rustok_span_duration_seconds{operation}
rustok_spans_with_errors_total{operation,error_type}
```

- `operation` — стабильное имя операции, например `comments.create_comment`
- `error_type` — тот же низкокардинальный класс ошибки

Это минимальный latency/error слой для write-path и library entry-point
операций.

### Бюджеты read-path

```
rustok_read_path_requested_limit{surface,path}
rustok_read_path_effective_limit{surface,path}
rustok_read_path_returned_items{surface,path}
rustok_read_path_limit_clamped_total{surface,path}
rustok_read_path_query_duration_seconds{surface,path,query}
rustok_read_path_query_rows{surface,path,query}
```

- `surface` — transport или runtime surface (`rest`, `graphql`, `library`)
- `path` — имя read-path
- `query` — стабильный шаг внутри read-path, например `comments.page`

Этот набор обязателен для bounded list/read surfaces, где есть `page/per_page`,
SSR feed или batch read path.

## Что уже instrumented

- `rustok-forum` — public GraphQL/REST read-path для categories, topics и
  replies пишет read-path budgets и query metrics.
- `rustok-blog` — public post list/read surfaces пишет read-path budgets и
  query metrics.
- `rustok-pages` — public page read-path пишет read-path budgets и query
  metrics.
- `rustok-comments` — service entry-points пишут module entrypoint metrics,
  span duration/error и read-path budget/query metrics для
  `list_comments_for_target`.
- `rustok-content` — orchestration/helper операции пишут span duration/error,
  а canonical/orchestration runbooks опираются на них.

## Минимальный contract для нового модуля

Если модуль добавляет новую публичную surface, минимальный baseline такой:

1. Для каждого service entry-point писать
   `rustok_module_entrypoint_calls_total`.
2. Для ошибок писать `rustok_module_errors_total` с коротким классификатором.
3. Для write-path или orchestration операций писать
   `rustok_span_duration_seconds` и `rustok_spans_with_errors_total`.
4. Для bounded list/read path писать весь read-path budget/query набор.

Без этого модуль считается operationally incomplete.

## Пример

```rust
use std::time::Instant;
use rustok_telemetry::metrics;

fn record_entrypoint() {
    metrics::record_module_entrypoint_call("comments", "create_comment", "library");
}

fn finish(operation: &str, started: Instant, result: &Result<(), MyError>) {
    metrics::record_span_duration(operation, started.elapsed().as_secs_f64());
    if let Err(error) = result {
        metrics::record_span_error(operation, error.kind());
        metrics::record_module_error("comments", error.kind(), error.severity());
    }
}
```

## Операторские вопросы

При проблемах с модулем сначала отвечайте на три вопроса:

1. Через какой integration path идёт трафик:
   `rate(rustok_module_entrypoint_calls_total{module="comments"}[5m])`
2. Какой error-class растёт:
   `sum(rate(rustok_module_errors_total{module="comments"}[5m])) by (error_type,severity)`
3. Где read-path теряет бюджет или упирается в latency:
   `histogram_quantile(0.95, rate(rustok_read_path_query_duration_seconds_bucket{path="comments.list_comments_for_target"}[5m]))`

## Правила

1. Не вводить high-cardinality labels.
2. Не плодить новые метрики, если существующий baseline уже покрывает задачу.
3. Не оставлять новый public read-path без `read_path_*`.
4. Не оставлять новый write/orchestration path без `span_*`.
5. Документация модуля должна перечислять, какие его surfaces уже
   instrumented и чего ещё не хватает.
