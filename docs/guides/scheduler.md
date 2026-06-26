---
id: doc://docs/guides/scheduler.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Планировщик задач (Loco Scheduler)

Руководство по работе с планировщиком задач в RusToK.

## Обзор

Планировщик — это отдельный процесс Loco, который по расписанию запускает зарегистрированные Loco Tasks.

```
cargo loco scheduler    ← запускает сам планировщик
cargo loco task --name <name>   ← запускает задачу вручную
```

Планировщик **не встроен в сервер** — он запускается как самостоятельный процесс, читает `scheduler.yaml` и по CRON-расписанию выполняет `cargo loco task --name <name>`.

## Конфигурация (`scheduler.yaml`)

Файл `apps/server/scheduler.yaml`:

```yaml
jobs:
  cleanup_sessions:
    run: "cleanup sessions"
    schedule: "0 0 * * * *"    # каждый час

  media_cleanup:
    run: "media_cleanup"
    schedule: "0 0 3 * * *"    # ежедневно в 03:00 UTC

  rebuild_index:
    run: "rebuild index"
    schedule: "0 0 */6 * * *"  # каждые 6 часов
```

### Формат `schedule`

Расписание задаётся в формате **cron с 6 полями** (секунды, минуты, часы, дни, месяцы, дни недели):

```
секунды минуты часы дни_месяца месяцы дни_недели
  0       0     3    *           *       *        → 03:00:00 UTC каждый день
  0       0     *    *           *       *        → каждый час в 0 минут
  0       0    */6   *           *       *        → в 0:00, 6:00, 12:00, 18:00
```

### Поле `run`

Значение `run` — это имя Loco Task, которое передаётся в `--name`:

```yaml
run: "media_cleanup"   # → cargo loco task --name media_cleanup
```

## Текущие задачи

| Задача | Расписание | Описание |
|--------|-----------|----------|
| `cleanup_sessions` | Каждый час | Удаляет истёкшие сессии из БД |
| `media_cleanup` | Ежедневно в 03:00 UTC | Удаляет осиротевшие записи медиа |
| `rebuild_index` | Каждые 6 часов | Перестраивает CQRS read-model индекс |

## Как создать новую задачу

### 1. Реализуйте `Task`

```rust
// apps/server/src/tasks/my_task.rs
use async_trait::async_trait;
use loco_rs::{app::AppContext, task::{Task, TaskInfo, Vars}, Result};

pub struct MyTask;

#[async_trait]
impl Task for MyTask {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "my_task".to_string(),
            detail: "Short description of what the task does".to_string(),
        }
    }

    async fn run(&self, app_context: &AppContext, _vars: &Vars) -> Result<()> {
        // Логика задачи
        tracing::info!("my_task: starting");
        Ok(())
    }
}
```

### 2. Зарегистрируйте задачу в `app.rs`

```rust
// apps/server/src/app.rs
async fn connect_tasks(v: &mut Tasks) {
    v.register(tasks::MyTask);
    // ...
}
```

### 3. Добавьте в `scheduler.yaml`

```yaml
jobs:
  my_task:
    run: "my_task"
    schedule: "0 30 2 * * *"   # каждый день в 02:30 UTC
```

### 4. (Опционально) Добавьте feature-флаг

Если задача зависит от опционального модуля:

```rust
async fn run(&self, app_context: &AppContext, _vars: &Vars) -> Result<()> {
    #[cfg(feature = "mod-media")]
    run_my_logic(app_context).await?;

    #[cfg(not(feature = "mod-media"))]
    tracing::info!("module not enabled — skipping");

    Ok(())
}
```

## Запуск вручную

```bash
# Запустить конкретную задачу немедленно
cargo loco task --name media_cleanup

# Запустить с переменными окружения
cargo loco task --name media_cleanup VAR_NAME=value

# Просмотреть список задач
cargo loco task
```

## Запуск планировщика

```bash
# В разработке
cargo loco scheduler

# В production (обычно отдельный процесс/контейнер)
./server scheduler
```

## Паттерны надёжности

### Проверка доступности зависимостей

Перед выполнением длинной операции проверьте, что зависимость (хранилище, внешний сервис) доступна:

```rust
async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
    let Some(storage) = ctx.shared_store.get::<StorageService>() else {
        tracing::warn!("StorageService not available — skipping");
        return Ok(());
    };
    // ...
}
```

### Консервативная обработка ошибок

Если задача обрабатывает множество записей, **не прерывайте всю задачу** при единичной ошибке — логируйте и продолжайте:

```rust
for item in items {
    match process_item(&item).await {
        Ok(_) => processed += 1,
        Err(e) => {
            tracing::warn!(id = %item.id, error = %e, "Failed to process item");
        }
    }
}
tracing::info!(processed, total = items.len(), "Task complete");
```

### Идемпотентность

Задачи **должны быть идемпотентными** — повторный запуск не должен приводить к дублированию эффектов.

## Мониторинг

Результат выполнения задачи логируется через `tracing`. Уровни:
- `INFO` — нормальное завершение с итоговой статистикой
- `WARN` — некритичные пропуски (запись не обработана, зависимость недоступна)
- `ERROR` — только при невосстановимых сбоях

Пример из `media_cleanup`:

```
INFO rustok: scanned=1024 removed=3 "Media cleanup complete"
```

## Связанные документы

- [WebSocket-каналы](../architecture/channels.md)
- [Документация rustok-media](../../crates/rustok-media/docs/README.md)
- [Наблюдаемость](./observability-quickstart.md)
