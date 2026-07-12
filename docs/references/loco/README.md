---
id: doc://docs/references/loco/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: archived
---

# Archived Loco Reference Package (RusToK)

Last updated: **2026-02-19**.

> Archived historical material. RusToK now uses a pure Axum server runtime and
> separate platform CLI. Do not use these examples, API signatures, or
> procedures for implementation. Use the [Loco RS Exit Plan](../../architecture/loco-exit-plan.md)
> and [backend guides](../../backend/README.md) instead.

## 1) Minimal working example: controller + routes

```rust
use axum::{extract::State, Json};
use loco_rs::prelude::*;

pub async fn list_posts(State(ctx): State<AppContext>) -> Result<Json<Vec<PostListItem>>> {
    let service = PostService::new(
        ctx.db.clone(),
        transactional_event_bus_from_context(&ctx),
    );
    let (items, _) = service
        .list_posts(tenant.id, user.security_context(), filter)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(Json(items))
}

pub fn routes() -> Routes {
    Routes::new().prefix("blog").add("/posts", get(list_posts))
}
```

Why this is "minimum" for RusToK:
- we take `AppContext` via `State<AppContext>`;
- the service is created from `ctx.db` + platform event bus;
- domain layer errors are converted to `loco_rs::Error`.

## 2) Minimal working example: application hooks

```rust
impl Hooks for App {
    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
    }

    async fn connect_workers(ctx: &AppContext, _queue: &Queue) -> Result<()> {
        let event_runtime = ctx
            .shared_store
            .get::<Arc<EventRuntime>>()
            .ok_or_else(|| loco_rs::Error::Message("EventRuntime not initialized".to_string()))?;

        if let Some(relay_config) = event_runtime.relay_config.clone() {
            let handle = spawn_outbox_relay_worker(relay_config);
            ctx.shared_store.insert(Arc::new(handle));
        }

        Ok(())
    }
}
```

## 3) Current API signatures (in repository)

- `pub async fn metrics(State(ctx): State<AppContext>) -> Result<Response>`
- `pub fn routes() -> Routes`
- `async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()>` (Task)
- `fn routes(_ctx: &AppContext) -> AppRoutes` (Hooks)
- `async fn after_routes(router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter>` (Hooks)
- `async fn connect_workers(ctx: &AppContext, _queue: &Queue) -> Result<()>` (Hooks)

## 4) What not to do (typical incorrect patterns from Axum)

1. **Do not build a "pure Axum-only" pipeline bypassing Loco hooks** for main server routes.
   - Anti-pattern: a separate `axum::Router::new()` without integration into `Hooks::routes/after_routes`.
   - Why it is bad: common initialization, shared-store, and middleware prerequisites are lost.

2. **Do not use global singleton services instead of `AppContext`**.
   - Anti-pattern: `lazy_static`/`OnceCell` with DB/transport when they already live in `ctx.shared_store`.
   - Why it is bad: desynchronization of runtime state and Loco lifecycle.

3. **Do not return "raw" axum error contract without aligning to `loco_rs::Result`**.
   - Anti-pattern: manual non-standard `IntoResponse` in each handler instead of uniform `Result<...>` + map_err.

## 5) Synchronization with code (procedure)

- When changes are made to `apps/server/src/app.rs`, `apps/server/src/controllers/**`, `apps/server/src/tasks/**`:
  1) check this reference package;
  2) update signatures/examples;
  3) set a new date in the header (`Last updated`).
- If an example no longer matches the working code, it must not remain without annotation.
