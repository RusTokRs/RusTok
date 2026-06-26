---
id: doc://docs/references/loco/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Loco Reference-пакет (RusToK)

Дата последней актуализации: **2026-02-19**.

> Этот пакет фиксирует «как правильно» использовать Loco в текущем RusToK и защищает от ложных переносов привычек из чистого Axum.

## 1) Минимальный рабочий пример: контроллер + маршруты

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

Почему это «минимум» для RusToK:
- берём `AppContext` через `State<AppContext>`;
- сервис создаётся от `ctx.db` + platform event bus;
- ошибки доменного слоя приводятся к `loco_rs::Error`.

## 2) Минимальный рабочий пример: hooks приложения

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

## 3) Актуальные сигнатуры API (в репозитории)

- `pub async fn metrics(State(ctx): State<AppContext>) -> Result<Response>`
- `pub fn routes() -> Routes`
- `async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()>` (Task)
- `fn routes(_ctx: &AppContext) -> AppRoutes` (Hooks)
- `async fn after_routes(router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter>` (Hooks)
- `async fn connect_workers(ctx: &AppContext, _queue: &Queue) -> Result<()>` (Hooks)

## 4) Чего делать нельзя (типичные ложные паттерны из Axum)

1. **Нельзя собирать «чистый Axum-only» pipeline мимо Loco hooks** для основных серверных маршрутов.
   - Антипаттерн: отдельный `axum::Router::new()` без интеграции в `Hooks::routes/after_routes`.
   - Почему плохо: теряется общая инициализация, shared-store, middleware-предпосылки.

2. **Нельзя тащить глобальные singleton-сервисы вместо `AppContext`**.
   - Антипаттерн: `lazy_static`/`OnceCell` с БД/transport, когда они уже живут в `ctx.shared_store`.
   - Почему плохо: рассинхронизация runtime-состояния и lifecycle Loco.

3. **Нельзя возвращать «сырой» axum error contract без выравнивания в `loco_rs::Result`**.
   - Антипаттерн: ручные нестандартные `IntoResponse` в каждом handler вместо единообразного `Result<...>` + map_err.

## 5) Синхронизация с кодом (регламент)

- При изменениях `apps/server/src/app.rs`, `apps/server/src/controllers/**`, `apps/server/src/tasks/**`:
  1) проверить этот reference-пакет;
  2) обновить сигнатуры/примеры;
  3) проставить новую дату в шапке (`Дата последней актуализации`).
- Если пример больше не совпадает с рабочим кодом, он не должен оставаться без пометки.
