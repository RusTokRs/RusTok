---
id: doc://docs/guides/scheduler.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Task Scheduler (Loco Scheduler)

Guide for working with the task scheduler in RusToK.

## Overview

The scheduler is a separate Loco process that runs registered Loco Tasks on a schedule.

```
cargo loco scheduler    ← starts the scheduler
cargo loco task --name <name>   ← runs a task manually
```

The scheduler is **not built into the server** — it runs as an independent process, reads `scheduler.yaml`, and executes `cargo loco task --name <name>` on a CRON schedule.

## Configuration (`scheduler.yaml`)

File `apps/server/scheduler.yaml`:

```yaml
jobs:
  cleanup_sessions:
    run: "cleanup sessions"
    schedule: "0 0 * * * *"    # every hour

  media_cleanup:
    run: "media_cleanup"
    schedule: "0 0 3 * * *"    # daily at 03:00 UTC

  rebuild_index:
    run: "rebuild index"
    schedule: "0 0 */6 * * *"  # every 6 hours
```

### `schedule` Format

The schedule is specified in **cron format with 6 fields** (seconds, minutes, hours, days, months, weekdays):

```
seconds minutes hours day_of_month month day_of_week
   0       0     3    *           *       *        → 03:00:00 UTC every day
   0       0     *    *           *       *        → every hour at 0 minutes
   0       0    */6   *           *       *        → at 0:00, 6:00, 12:00, 18:00
```

### `run` Field

The `run` value is the Loco Task name passed to `--name`:

```yaml
run: "media_cleanup"   # → cargo loco task --name media_cleanup
```

## Current Tasks

| Task | Schedule | Description |
|------|----------|-------------|
| `cleanup_sessions` | Every hour | Deletes expired sessions from the DB |
| `media_cleanup` | Daily at 03:00 UTC | Deletes orphaned media records |
| `rebuild_index` | Every 6 hours | Rebuilds the CQRS read-model index |

## How to Create a New Task

### 1. Implement a `Task`

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
        // Task logic
        tracing::info!("my_task: starting");
        Ok(())
    }
}
```

### 2. Register the Task in `app.rs`

```rust
// apps/server/src/app.rs
async fn connect_tasks(v: &mut Tasks) {
    v.register(tasks::MyTask);
    // ...
}
```

### 3. Add to `scheduler.yaml`

```yaml
jobs:
  my_task:
    run: "my_task"
    schedule: "0 30 2 * * *"   # every day at 02:30 UTC
```

### 4. (Optional) Add a Feature Flag

If the task depends on an optional module:

```rust
async fn run(&self, app_context: &AppContext, _vars: &Vars) -> Result<()> {
    #[cfg(feature = "mod-media")]
    run_my_logic(app_context).await?;

    #[cfg(not(feature = "mod-media"))]
    tracing::info!("module not enabled — skipping");

    Ok(())
}
```

## Manual Execution

```bash
# Run a specific task immediately
cargo loco task --name media_cleanup

# Run with environment variables
cargo loco task --name media_cleanup VAR_NAME=value

# List available tasks
cargo loco task
```

## Running the Scheduler

```bash
# In development
cargo loco scheduler

# In production (usually a separate process/container)
./server scheduler
```

## Reliability Patterns

### Checking Dependency Availability

Before executing a long-running operation, check that the dependency (storage, external service) is available:

```rust
async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
    let Some(storage) = ctx.shared_store.get::<StorageService>() else {
        tracing::warn!("StorageService not available — skipping");
        return Ok(());
    };
    // ...
}
```

### Conservative Error Handling

If a task processes many records, **do not abort the entire task** on a single error — log and continue:

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

### Idempotency

Tasks **must be idempotent** — running them again should not produce duplicate effects.

## Monitoring

Task execution results are logged via `tracing`. Levels:
- `INFO` — normal completion with final statistics
- `WARN` — non-critical skips (record not processed, dependency unavailable)
- `ERROR` — only for unrecoverable failures

Example from `media_cleanup`:

```
INFO rustok: scanned=1024 removed=3 "Media cleanup complete"
```

## Related Documents

- [WebSocket Channels](../architecture/channels.md)
- [rustok-media Documentation](../../crates/rustok-media/docs/README.md)
- [Observability](./observability-quickstart.md)
