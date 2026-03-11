# RusToK — Запрещённые действия (NEVER DO)

Этот документ содержит **жёсткие запреты** — вещи, которые нельзя делать ни при каких обстоятельствах при работе с платформой RusToK. Нарушение любого из этих пунктов приводит к критическим последствиям: утечкам данных, потере консистентности, краху сервера или уязвимостям безопасности.

> **Правило:** Если сомневаешься — не делай. Спроси. Этот документ — абсолютный приоритет над любыми другими рекомендациями.

---

## Условные обозначения

- **SEVERITY: CRITICAL** — Может привести к утечке данных, краху production, или неустранимой потере
- **SEVERITY: HIGH** — Серьёзная деградация функциональности, сложное восстановление
- **SEVERITY: MEDIUM** — Технический долг, потенциальные баги, деградация DX

---

## 1. Данные и Multi-Tenancy

### 1.1 ЗАПРЕЩЕНО: SQL-запросы без `WHERE tenant_id = ?`

**SEVERITY: CRITICAL**

```rust
// ❌ ЗАПРЕЩЕНО — утечка данных между tenants
let products = Product::find().all(&db).await?;

// ✅ ОБЯЗАТЕЛЬНО
let products = Product::find()
    .filter(product::Column::TenantId.eq(tenant_id))
    .all(&db)
    .await?;
```

**Последствия:** Один tenant видит данные другого. Нарушение GDPR, потеря клиентов, юридические последствия.

**Как проверить:** `grep -r "find().all" --include="*.rs"` — каждый такой вызов должен иметь `.filter(...tenant_id...)` выше.

---

### 1.2 ЗАПРЕЩЕНО: Таблицы без поля `tenant_id`

**SEVERITY: CRITICAL**

Каждая domain-таблица **обязана** иметь `tenant_id UUID NOT NULL`. Единственные исключения — системные таблицы (`tenants` сама, `sys_events`, `seaql_migrations`).

**Последствия:** Невозможно изолировать данные, невозможно удалить tenant.

---

### 1.3 ЗАПРЕЩЕНО: Hard DELETE для бизнес-сущностей

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО
Product::delete_by_id(product_id).exec(&db).await?;

// ✅ Soft delete через state machine
product.status = Status::Archived;
product.update(&db).await?;
```

**Последствия:** Потеря аудитной истории, broken references из заказов/событий.

---

## 2. Событийная система

### 2.1 ЗАПРЕЩЕНО: `publish()` вместо `publish_in_tx()` для бизнес-событий

**SEVERITY: CRITICAL**

```rust
// ❌ ЗАПРЕЩЕНО — событие уходит даже если транзакция откатится
service.create_product(&input).await?;
event_bus.publish(ProductCreated { id }).await?;

// ✅ ОБЯЗАТЕЛЬНО — атомарно в одной транзакции
let tx = db.begin().await?;
let product = service.create_product_in_tx(&tx, &input).await?;
event_bus.publish_in_tx(&tx, ProductCreated { id: product.id }).await?;
tx.commit().await?;
```

**Последствия:** Phantom events (событие ушло, а данных нет) или lost events (данные есть, а событие не ушло). Index рассинхронизирован с write DB.

---

### 2.2 ЗАПРЕЩЕНО: Production без Outbox relay worker

**SEVERITY: CRITICAL**

Если `transport = "outbox"`, но relay worker не запущен — события **навсегда** застрянут в таблице `sys_events` со статусом `pending`.

**Последствия:** Index не обновляется, storefront показывает устаревшие данные, DLQ растёт бесконечно.

---

### 2.3 ЗАПРЕЩЕНО: `transport = "memory"` в production

**SEVERITY: HIGH**

Memory transport (`tokio::broadcast`) теряет все события при перезапуске сервера.

**Последствия:** Потеря событий при deploy, рестарте, OOM kill.

---

### 2.4 ЗАПРЕЩЕНО: События без `tenant_id` в payload

**SEVERITY: HIGH**

Каждый `DomainEvent` **обязан** содержать `tenant_id`. Index и listeners фильтруют по tenant.

**Последствия:** Index не может определить, к какому tenant относится событие. Cross-tenant data pollution.

---

## 3. Auth и RBAC

### 3.1 ЗАПРЕЩЕНО: Endpoints без RBAC-проверки

**SEVERITY: CRITICAL**

```rust
// ❌ ЗАПРЕЩЕНО — любой authenticated user может удалить product
pub async fn delete_product(
    user: CurrentUser,  // только auth, без RBAC
    Path(id): Path<Uuid>,
) -> Result<()> { ... }

// ✅ ОБЯЗАТЕЛЬНО
pub async fn delete_product(
    RequireProductsDelete(user): RequireProductsDelete,  // auth + RBAC
    Path(id): Path<Uuid>,
) -> Result<()> { ... }
```

**Исключения (endpoints без RBAC):** `GET /api/health`, `POST /api/auth/login`, `POST /api/auth/register`, public storefront read queries.

**Последствия:** Privilege escalation — Customer может удалять products, менять settings, управлять users.

---

### 3.2 ЗАПРЕЩЕНО: Hardcoded secrets в коде

**SEVERITY: CRITICAL**

```rust
// ❌ ЗАПРЕЩЕНО
const JWT_SECRET: &str = "my-super-secret-key-123";
const DB_PASSWORD: &str = "postgres";

// ✅ ОБЯЗАТЕЛЬНО — через env vars
let jwt_secret = std::env::var("JWT_SECRET")
    .expect("JWT_SECRET must be set");
```

**Последствия:** Компрометация всех токенов/паролей при утечке репозитория.

---

### 3.3 ЗАПРЕЩЕНО: Дублирование auth логики между REST и GraphQL

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — разная логика в REST и GraphQL
// controllers/auth.rs
fn login(input) { /* своя логика */ }
// graphql/auth.rs
fn login_mutation(input) { /* другая логика */ }

// ✅ ОБЯЗАТЕЛЬНО — единый AuthLifecycleService
// services/auth_lifecycle.rs содержит всю логику
// REST и GraphQL — тонкие adapters
```

**Последствия:** Рассинхрон — один transport разрешает, другой запрещает. Дыры в безопасности.

---

### 3.4 ЗАПРЕЩЕНО: Бизнес-логика в controllers/resolvers

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — бизнес-логика в controller
pub async fn create_product(input: CreateProductInput) -> Result<Json<Product>> {
    input.validate()?;
    let product = Product::new(input.name, input.price);
    // 50 строк бизнес-логики прямо здесь...
    product.save(&db).await?;
    event_bus.publish(ProductCreated { id: product.id }).await?;
    Ok(Json(product))
}

// ✅ ОБЯЗАТЕЛЬНО — controller вызывает service
pub async fn create_product(
    RequireProductsCreate(user): RequireProductsCreate,
    State(ctx): State<AppContext>,
    Json(input): Json<CreateProductInput>,
) -> loco_rs::Result<Json<ProductResponse>> {
    let product = ProductService::create(&ctx, &input).await?;
    Ok(Json(product.into()))
}
```

**Последствия:** Дублирование между REST и GraphQL. Невозможно тестировать бизнес-логику без HTTP.

---

### 3.5 ЗАПРЕЩЕНО: GraphQL resolvers без DataLoader для связанных данных

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — N+1 запросов
#[Object]
impl ProductQuery {
    async fn variants(&self, ctx: &Context<'_>) -> Result<Vec<Variant>> {
        // Каждый product делает свой SQL запрос!
        db.find_variants_by_product(self.id).await
    }
}

// ✅ ОБЯЗАТЕЛЬНО — DataLoader
#[Object]
impl ProductQuery {
    async fn variants(&self, ctx: &Context<'_>) -> Result<Vec<Variant>> {
        let loader = ctx.data::<DataLoader<VariantLoader>>()?;
        loader.load_one(self.id).await
    }
}
```

**Последствия:** 100 products × 1 query каждый = 101 SQL запрос вместо 2. Latency ×50, нагрузка на DB.

---

### 3.6 ЗАПРЕЩЕНО: List endpoints без пагинации

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — загрузит всю таблицу
async fn list_products() -> Result<Json<Vec<Product>>> {
    let all = Product::find().all(&db).await?;
    Ok(Json(all))
}

// ✅ ОБЯЗАТЕЛЬНО — пагинация
async fn list_products(
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ProductResponse>>> {
    let page = ProductService::list(&ctx, params.page, params.per_page).await?;
    Ok(Json(page))
}
```

**Последствия:** 100 000 записей в памяти. OOM, timeout, DoS.

---

### 3.7 ЗАПРЕЩЕНО: REST endpoints без OpenAPI annotations

**SEVERITY: MEDIUM**

```rust
// ❌ ЗАПРЕЩЕНО — endpoint не виден в Swagger
pub async fn create_product(...) -> Result<Json<Product>> { }

// ✅ ОБЯЗАТЕЛЬНО
#[utoipa::path(
    post, path = "/api/products",
    request_body = CreateProductInput,
    responses((status = 201, body = ProductResponse)),
    security(("bearer_auth" = []))
)]
pub async fn create_product(...) -> Result<Json<ProductResponse>> { }
```

**Последствия:** Swagger UI не показывает endpoint. Фронтенд-разработчики не знают о существовании API.

---

## 4. Код и Runtime

### 4.1 ЗАПРЕЩЕНО: `unwrap()` / `expect()` в production коде

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО
let user = db.find_user(id).await.unwrap();
let config = serde_json::from_str(data).expect("valid json");

// ✅ ОБЯЗАТЕЛЬНО
let user = db.find_user(id).await
    .map_err(|e| Error::Database(e))?;
let config: Config = serde_json::from_str(data)
    .map_err(|e| Error::Validation(e.to_string()))?;
```

**Исключения:** `expect()` допустим ТОЛЬКО для программных инвариантов, которые гарантированы на уровне типов (и задокументированы).

**Последствия:** Паника крашит весь tokio runtime = все подключённые клиенты теряют соединение.

---

### 4.2 ЗАПРЕЩЕНО: Blocking операции в async контексте

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — блокирует tokio worker thread
async fn process() {
    std::thread::sleep(Duration::from_secs(1));  // blocking!
    let data = std::fs::read_to_string("file.txt")?;  // blocking!
}

// ✅ ОБЯЗАТЕЛЬНО
async fn process() {
    tokio::time::sleep(Duration::from_secs(1)).await;
    let data = tokio::fs::read_to_string("file.txt").await?;
}
```

**Последствия:** Блокировка всех async tasks на этом worker thread. Latency spikes, timeouts.

---

### 4.3 ЗАПРЕЩЕНО: Неограниченный `tokio::spawn` в цикле

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — может создать миллион tasks
for item in huge_list {
    tokio::spawn(async move { process(item).await });
}

// ✅ ОБЯЗАТЕЛЬНО — Semaphore или JoinSet
let semaphore = Arc::new(Semaphore::new(100));
for item in huge_list {
    let permit = semaphore.clone().acquire_owned().await?;
    tokio::spawn(async move {
        process(item).await;
        drop(permit);
    });
}
```

**Последствия:** OOM, CPU starvation, resource exhaustion.

---

### 4.4 ЗАПРЕЩЕНО: Логирование PII и secrets

**SEVERITY: CRITICAL**

```rust
// ❌ ЗАПРЕЩЕНО
tracing::info!("User login: email={}, password={}", email, password);
tracing::debug!("JWT token: {}", token);
tracing::info!("DB connection: {}", connection_string_with_password);

// ✅ ОБЯЗАТЕЛЬНО
tracing::info!(user_id = %user.id, "User logged in");
```

**Последствия:** GDPR violation, утечка credentials через лог-агрегаторы.

---

## 5. Модульная система

### 5.1 ЗАПРЕЩЕНО: Отключать Core-модули

**SEVERITY: CRITICAL**

`rustok-index`, `rustok-tenant`, `rustok-rbac` имеют `ModuleKind::Core`. Попытка toggle через `ModuleLifecycleService` **обязана** возвращать ошибку.

**Последствия:** RBAC выключен = нет авторизации. Tenant выключен = нет multi-tenancy. Index выключен = storefront не работает.

---

### 5.2 ЗАПРЕЩЕНО: Обходить ModuleRegistry для lifecycle

**SEVERITY: HIGH**

```rust
// ❌ ЗАПРЕЩЕНО — модуль подключается мимо registry
fn routes() -> AppRoutes {
    AppRoutes::new()
        .add_route(my_custom_module::routes())  // Мимо registry!
}

// ✅ ОБЯЗАТЕЛЬНО — через RusToKModule + build_registry()
```

**Последствия:** Module health не видно, toggle не работает, миграции не подхватываются.

---

### 5.3 ЗАПРЕЩЕНО: Включать зависимый модуль без его dependency

**SEVERITY: HIGH**

Blog зависит от Content. Forum зависит от Content.

```rust
// ❌ ЗАПРЕЩЕНО
toggle_module("blog", true);   // Content отключён!

// ✅ toggle_module проверяет dependencies автоматически
```

**Последствия:** Runtime ошибки, отсутствующие таблицы, паники.

---

## 6. Loco / Framework

### 6.1 ЗАПРЕЩЕНО: Обходить Loco hooks lifecycle

**SEVERITY: HIGH**

Нельзя создавать параллельный lifecycle «чистого Axum» — `Hooks::routes`, `Hooks::after_routes`, `Hooks::connect_workers` существуют для инициализации.

**Последствия:** Middleware не применяется, dependency injection не работает, auth/tenant/RBAC не инициализированы.

---

### 6.2 ЗАПРЕЩЕНО: Смешивать error контракты в controllers

**SEVERITY: MEDIUM**

```rust
// ❌ ЗАПРЕЩЕНО — свои типы ошибок в контроллерах
pub async fn handler() -> Result<Json<Data>, MyCustomError> { }

// ✅ ОБЯЗАТЕЛЬНО — loco_rs::Result
pub async fn handler() -> loco_rs::Result<Json<Data>> { }
```

**Последствия:** Несовместимые error responses, middleware не может обработать ошибку.

---

### 6.3 Framework deviation criteria (обязательные критерии отклонения)

**SEVERITY: HIGH**

Любое осознанное отклонение от framework/runtime baseline (например, отказ от стандартного механизма Loco в пользу custom реализации) допустимо только при явном прохождении минимальных критериев:

1. **Reliability semantics** — зафиксирована модель delivery/consistency (at-most-once / at-least-once / exactly-once where applicable), границы идемпотентности и допущения.
2. **Backpressure** — описан контроль нагрузки (ограничение очередей, concurrency limits, fail-fast/timeout policy), чтобы исключить неограниченный рост latency и memory.
3. **Replay** — определена стратегия безопасного повторного проигрывания (replay window, deduplication/idempotency contract, порядок восстановления).
4. **Multi-tenant guarantees** — показано, что tenant isolation, routing и authz-проверки не деградируют при новом пути исполнения.
5. **Operational runbook impact** — обновлён incident/runbook контур: сигналы, метрики, алерты, triage-процедуры и on-call шаги.

Отклонение без покрытия всех пяти пунктов считается архитектурно неподтверждённым и не принимается.

---

### 6.4 Framework deviation checklist (обязателен для каждого нового отклонения)

Перед merge обязан быть заполнен следующий checklist:

- [ ] **Benchmark evidence**: приложены воспроизводимые benchmark-результаты (baseline vs proposed), входные данные и методика.
- [ ] **Failure-mode table**: есть таблица failure modes (симптом, blast radius, detection signal, mitigation, owner).
- [ ] **Rollback strategy**: зафиксирован rollback path (триггеры отката, шаги, expected recovery time, residual risk).
- [ ] **Owner sign-off**: есть явное подтверждение владельца домена/платформы на выбранное отклонение.

Checklist является обязательным gate и должен быть отражён:

- в PR checklist процесса контрибуции: [`CONTRIBUTING.md`](../../CONTRIBUTING.md#pr-checklist);
- в server governance-документации: [`apps/server/docs/README.md`](../../apps/server/docs/README.md).

---

## 7. MCP

### 7.1 ЗАПРЕЩЕНО: Бизнес-логика в MCP адаптере

**SEVERITY: MEDIUM**

MCP слой — тонкий adapter над service/registry. Вся логика — в domain services.

**Последствия:** Дублирование логики, невозможно использовать те же правила без MCP.

---

### 7.2 ЗАПРЕЩЕНО: Обходить typed tools (`McpToolResponse`)

**SEVERITY: MEDIUM**

```rust
// ❌ ЗАПРЕЩЕНО
return serde_json::json!({"result": "ok"});

// ✅ ОБЯЗАТЕЛЬНО
return McpToolResponse::success(data);
```

**Последствия:** Клиент не может парсить ответ, нет error handling.

---

## 8. Telemetry

### 8.1 ЗАПРЕЩЕНО: Множественная инициализация telemetry

**SEVERITY: HIGH**

Telemetry runtime (tracing subscriber, OTLP exporter) инициализируется **ровно один раз** при старте сервера.

**Последствия:** Паника, дублирование spans, утечка памяти, некорректные метрики.

---

### 8.2 ЗАПРЕЩЕНО: Фрагментация metrics registry

**SEVERITY: MEDIUM**

Все Prometheus метрики — через единый registry. Не создавать отдельные registry в модулях.

**Последствия:** `/metrics` не показывает часть метрик, Grafana dashboards пустые.

---

## 9. DevOps

### 9.1 ЗАПРЕЩЕНО: Коммит без `cargo fmt` и `cargo clippy`

**SEVERITY: MEDIUM**

```bash
# ❌ ЗАПРЕЩЕНО — коммитить без проверки
git add . && git commit -m "changes"

# ✅ ОБЯЗАТЕЛЬНО
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
# Только потом коммит
```

**Последствия:** Шумные diff'ы, скрытые баги, failed CI.

---

### 9.2 ЗАПРЕЩЕНО: Редактировать CI/CD workflow без явного запроса

**SEVERITY: HIGH**

`.github/workflows/*.yml` — только по явному запросу и с ревью.

**Последствия:** Сломанный CI для всей команды.

---

### 9.3 ЗАПРЕЩЕНО: Коммитить `.env` файлы с реальными credentials

**SEVERITY: CRITICAL**

`.gitignore` исключает `.env`. Только `.env.dev.example` с placeholder values.

**Последствия:** Утечка production credentials через git history.

---

## Чеклист перед коммитом

Перед каждым коммитом убедись, что **ни один** запрет из этого документа не нарушен:

- [ ] Нет SQL без `tenant_id` filter
- [ ] Нет `unwrap()`/`expect()` в новом production коде
- [ ] Нет `publish()` вместо `publish_in_tx()` для бизнес-событий
- [ ] Нет hardcoded secrets
- [ ] Нет endpoints без RBAC (кроме public)
- [ ] Нет blocking ops в async
- [ ] Нет логирования PII
- [ ] Нет бизнес-логики в controllers/resolvers (только вызов services)
- [ ] Нет N+1 queries в GraphQL (DataLoader для связанных данных)
- [ ] Нет list endpoints без пагинации
- [ ] REST endpoints имеют `#[utoipa::path]` annotations
- [ ] REST и GraphQL auth logic идентичны (через AuthLifecycleService)
- [ ] `cargo fmt` и `cargo clippy` пройдены
- [ ] Документация обновлена при изменении кода

---

## Связанные документы

- [Паттерны vs Антипаттерны](./patterns-vs-antipatterns.md) — сводная таблица правильного и неправильного
- [Стандарты кода](./coding.md) — детальный гайд
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md) — ловушки для AI-агентов
- [Security Standards](./security.md) — OWASP coverage
- [Plan верификации](../PLATFORM_VERIFICATION_PLAN.md) — глобальный чеклист проверки платформы
