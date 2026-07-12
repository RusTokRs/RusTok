---
id: doc://docs/standards/forbidden-actions.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# RusToK — Forbidden Actions (NEVER DO)

This document contains **hard prohibitions** — things that must never be done under any circumstances when working with the RusToK platform. Violation of any of these points leads to critical consequences: data leaks, consistency loss, server crashes, or security vulnerabilities.

> **Rule:** When in doubt — don't do it. Ask. This document takes absolute priority over any other recommendations.

---

## Notation

- **SEVERITY: CRITICAL** — May lead to data leaks, production crashes, or irreversible loss
- **SEVERITY: HIGH** — Serious functional degradation, complex recovery
- **SEVERITY: MEDIUM** — Technical debt, potential bugs, DX degradation

---

## 1. Data and Multi-Tenancy

### 1.1 FORBIDDEN: SQL queries without `WHERE tenant_id = ?`

**SEVERITY: CRITICAL**

```rust
// ❌ FORBIDDEN — data leak between tenants
let products = Product::find().all(&db).await?;

// ✅ REQUIRED
let products = Product::find()
    .filter(product::Column::TenantId.eq(tenant_id))
    .all(&db)
    .await?;
```

**Consequences:** One tenant sees another tenant's data. GDPR violation, customer loss, legal consequences.

**How to check:** `grep -r "find().all" --include="*.rs"` — every such call must have `.filter(...tenant_id...)` above.

---

### 1.2 FORBIDDEN: Tables without `tenant_id` column

**SEVERITY: CRITICAL**

Every domain table **must** have `tenant_id UUID NOT NULL`. The only exceptions are system tables (`tenants` itself, `sys_events`, `seaql_migrations`).

**Consequences:** Impossible to isolate data, impossible to delete a tenant.

---

### 1.3 FORBIDDEN: Hard DELETE for business entities

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN
Product::delete_by_id(product_id).exec(&db).await?;

// ✅ Soft delete via state machine
product.status = Status::Archived;
product.update(&db).await?;
```

**Consequences:** Loss of audit history, broken references from orders/events.

---

## 2. Event System

### 2.1 FORBIDDEN: `publish()` instead of `publish_in_tx()` for business events

**SEVERITY: CRITICAL**

```rust
// ❌ FORBIDDEN — event goes out even if the transaction rolls back
service.create_product(&input).await?;
event_bus.publish(ProductCreated { id }).await?;

// ✅ REQUIRED — atomic in one transaction
let tx = db.begin().await?;
let product = service.create_product_in_tx(&tx, &input).await?;
event_bus.publish_in_tx(&tx, ProductCreated { id: product.id }).await?;
tx.commit().await?;
```

**Consequences:** Phantom events (event sent but no data) or lost events (data exists but event not sent). Index out of sync with write DB.

---

### 2.2 FORBIDDEN: Production without Outbox relay worker

**SEVERITY: CRITICAL**

If `transport = "outbox"` but the relay worker is not running — events will be **permanently** stuck in the `sys_events` table with `pending` status.

**Consequences:** Index not updated, storefront shows stale data, DLQ grows indefinitely.

---

### 2.3 FORBIDDEN: `transport = "memory"` in production

**SEVERITY: HIGH**

Memory transport (`tokio::broadcast`) loses all events on server restart.

**Consequences:** Event loss on deploy, restart, or OOM kill.

---

### 2.4 FORBIDDEN: Events without `tenant_id` in payload

**SEVERITY: HIGH**

Every `DomainEvent` **must** contain `tenant_id`. Index and listeners filter by tenant.

**Consequences:** Index cannot determine which tenant the event belongs to. Cross-tenant data pollution.

---

## 3. Auth and RBAC

### 3.1 FORBIDDEN: Endpoints without RBAC check

**SEVERITY: CRITICAL**

```rust
// ❌ FORBIDDEN — any authenticated user can delete a product
pub async fn delete_product(
    user: CurrentUser,  // auth only, no RBAC
    Path(id): Path<Uuid>,
) -> Result<()> { ... }

// ✅ REQUIRED
pub async fn delete_product(
    RequireProductsDelete(user): RequireProductsDelete,  // auth + RBAC
    Path(id): Path<Uuid>,
) -> Result<()> { ... }
```

**Exceptions (endpoints without RBAC):** `GET /api/health`, `POST /api/auth/login`, `POST /api/auth/register`, public storefront read queries.

**Consequences:** Privilege escalation — Customer can delete products, change settings, manage users.

---

### 3.2 FORBIDDEN: Hardcoded secrets in code

**SEVERITY: CRITICAL**

```rust
// ❌ FORBIDDEN
const JWT_SECRET: &str = "my-super-secret-key-123";
const DB_PASSWORD: &str = "postgres";

// ✅ REQUIRED — via env vars
let jwt_secret = std::env::var("JWT_SECRET")
    .expect("JWT_SECRET must be set");
```

**Consequences:** Compromise of all tokens/passwords if repository is leaked.

---

### 3.3 FORBIDDEN: Duplicating auth logic between REST and GraphQL

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — different logic in REST and GraphQL
// controllers/auth.rs
fn login(input) { /* own logic */ }
// graphql/auth.rs
fn login_mutation(input) { /* different logic */ }

// ✅ REQUIRED — single AuthLifecycleService
// services/auth_lifecycle.rs contains all logic
// REST and GraphQL are thin adapters
```

**Consequences:** Desync — one transport allows, another blocks. Security holes.

---

### 3.4 FORBIDDEN: Business logic in controllers/resolvers

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — business logic in controller
pub async fn create_product(input: CreateProductInput) -> Result<Json<Product>> {
    input.validate()?;
    let product = Product::new(input.name, input.price);
    // 50 lines of business logic right here...
    product.save(&db).await?;
    event_bus.publish(ProductCreated { id: product.id }).await?;
    Ok(Json(product))
}

// ✅ REQUIRED — controller calls service
pub async fn create_product(
    RequireProductsCreate(user): RequireProductsCreate,
    State(runtime): State<ServerRuntimeContext>,
    Json(input): Json<CreateProductInput>,
) -> Result<Json<ProductResponse>, rustok_web::HttpError> {
    let product = ProductService::create(runtime.db(), &input).await?;
    Ok(Json(product.into()))
}
```

**Consequences:** Duplication between REST and GraphQL. Impossible to test business logic without HTTP.

---

### 3.5 FORBIDDEN: GraphQL resolvers without DataLoader for related data

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — N+1 queries
#[Object]
impl ProductQuery {
    async fn variants(&self, ctx: &Context<'_>) -> Result<Vec<Variant>> {
        // Each product makes its own SQL query!
        db.find_variants_by_product(self.id).await
    }
}

// ✅ REQUIRED — DataLoader
#[Object]
impl ProductQuery {
    async fn variants(&self, ctx: &Context<'_>) -> Result<Vec<Variant>> {
        let loader = ctx.data::<DataLoader<VariantLoader>>()?;
        loader.load_one(self.id).await
    }
}
```

**Consequences:** 100 products × 1 query each = 101 SQL queries instead of 2. Latency ×50, DB load.

---

### 3.6 FORBIDDEN: List endpoints without pagination

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — loads the entire table
async fn list_products() -> Result<Json<Vec<Product>>> {
    let all = Product::find().all(&db).await?;
    Ok(Json(all))
}

// ✅ REQUIRED — pagination
async fn list_products(
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ProductResponse>>> {
    let page = ProductService::list(&ctx, params.page, params.per_page).await?;
    Ok(Json(page))
}
```

**Consequences:** 100,000 records in memory. OOM, timeout, DoS.

---

### 3.7 FORBIDDEN: REST endpoints without OpenAPI annotations

**SEVERITY: MEDIUM**

```rust
// ❌ FORBIDDEN — endpoint not visible in Swagger
pub async fn create_product(...) -> Result<Json<Product>> { }

// ✅ REQUIRED
#[utoipa::path(
    post, path = "/api/products",
    request_body = CreateProductInput,
    responses((status = 201, body = ProductResponse)),
    security(("bearer_auth" = []))
)]
pub async fn create_product(...) -> Result<Json<ProductResponse>> { }
```

**Consequences:** Swagger UI doesn't show the endpoint. Frontend developers don't know the API exists.

---

## 4. Code and Runtime

### 4.1 FORBIDDEN: `unwrap()` / `expect()` in production code

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN
let user = db.find_user(id).await.unwrap();
let config = serde_json::from_str(data).expect("valid json");

// ✅ REQUIRED
let user = db.find_user(id).await
    .map_err(|e| Error::Database(e))?;
let config: Config = serde_json::from_str(data)
    .map_err(|e| Error::Validation(e.to_string()))?;
```

**Exceptions:** `expect()` is allowed ONLY for program invariants guaranteed at the type level (and documented).

**Consequences:** Panic crashes the entire tokio runtime = all connected clients lose connection.

---

### 4.2 FORBIDDEN: Blocking operations in async context

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — blocks tokio worker thread
async fn process() {
    std::thread::sleep(Duration::from_secs(1));  // blocking!
    let data = std::fs::read_to_string("file.txt")?;  // blocking!
}

// ✅ REQUIRED
async fn process() {
    tokio::time::sleep(Duration::from_secs(1)).await;
    let data = tokio::fs::read_to_string("file.txt").await?;
}
```

**Consequences:** Blocks all async tasks on this worker thread. Latency spikes, timeouts.

---

### 4.3 FORBIDDEN: Unlimited `tokio::spawn` in a loop

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — may create a million tasks
for item in huge_list {
    tokio::spawn(async move { process(item).await });
}

// ✅ REQUIRED — Semaphore or JoinSet
let semaphore = Arc::new(Semaphore::new(100));
for item in huge_list {
    let permit = semaphore.clone().acquire_owned().await?;
    tokio::spawn(async move {
        process(item).await;
        drop(permit);
    });
}
```

**Consequences:** OOM, CPU starvation, resource exhaustion.

---

### 4.4 FORBIDDEN: Logging PII and secrets

**SEVERITY: CRITICAL**

```rust
// ❌ FORBIDDEN
tracing::info!("User login: email={}, password={}", email, password);
tracing::debug!("JWT token: {}", token);
tracing::info!("DB connection: {}", connection_string_with_password);

// ✅ REQUIRED
tracing::info!(user_id = %user.id, "User logged in");
```

**Consequences:** GDPR violation, credential leak through log aggregators.

---

## 5. Module System

### 5.1 FORBIDDEN: Disabling Core modules

**SEVERITY: CRITICAL**

`rustok-index`, `rustok-tenant`, `rustok-rbac` have `ModuleKind::Core`. Any attempt to toggle via `ModuleLifecycleService` **must** return an error.

**Consequences:** RBAC disabled = no authorization. Tenant disabled = no multi-tenancy. Index disabled = storefront doesn't work.

---

### 5.2 FORBIDDEN: Bypassing ModuleRegistry for lifecycle

**SEVERITY: HIGH**

```rust
// ❌ FORBIDDEN — module connects bypassing registry
fn routes() -> AppRoutes {
    AppRoutes::new()
        .add_route(my_custom_module::routes())  // Bypassing registry!
}

// ✅ REQUIRED — via RusToKModule + build_registry()
```

**Consequences:** Module health not visible, toggle doesn't work, migrations aren't picked up.

---

### 5.3 FORBIDDEN: Enabling a dependent module without its dependency

**SEVERITY: HIGH**

Blog depends on Content. Forum depends on Content.

```rust
// ❌ FORBIDDEN
toggle_module("blog", true);   // Content is disabled!

// ✅ toggle_module checks dependencies automatically
```

**Consequences:** Runtime errors, missing tables, panics.

---

## 6. Axum Runtime

### 6.1 FORBIDDEN: Bypassing the Axum lifecycle

**SEVERITY: HIGH**

Must not create a parallel "pure Axum" lifecycle — `Hooks::routes`, `Hooks::after_routes`, `Hooks::connect_workers` exist for initialization.

**Consequences:** Middleware not applied, dependency injection doesn't work, auth/tenant/RBAC not initialized.

---

### 6.2 FORBIDDEN: Mixing error contracts in controllers

**SEVERITY: MEDIUM**

```rust
// ❌ FORBIDDEN — custom error types in controllers
pub async fn handler() -> Result<Json<Data>, MyCustomError> { }

// ✅ REQUIRED — rustok-web error mapping
pub async fn handler() -> Result<Json<Data>, rustok_web::HttpError> { }
```

**Consequences:** Incompatible error responses, middleware cannot handle the error.

---

### 6.3 Framework deviation criteria (mandatory rejection criteria)

**SEVERITY: HIGH**

Any intentional deviation from the runtime baseline is only permissible with explicit fulfillment of the minimum criteria:

1. **Reliability semantics** — delivery/consistency model is fixed (at-most-once / at-least-once / exactly-once where applicable), idempotency boundaries and assumptions defined.
2. **Backpressure** — load control is described (queue limits, concurrency limits, fail-fast/timeout policy) to prevent unbounded growth of latency and memory.
3. **Replay** — a safe replay strategy is defined (replay window, deduplication/idempotency contract, recovery order).
4. **Multi-tenant guarantees** — shown that tenant isolation, routing and authz checks do not degrade under the new execution path.
5. **Operational runbook impact** — incident/runbook contour is updated: signals, metrics, alerts, triage procedures and on-call steps.

Deviation without covering all five points is considered architecturally unsubstantiated and is not accepted.

---

### 6.4 Framework deviation checklist (mandatory for each new deviation)

Before merge, the following checklist must be completed:

- [ ] **Benchmark evidence**: reproducible benchmark results (baseline vs proposed), input data and methodology attached.
- [ ] **Failure-mode table**: a table of failure modes (symptom, blast radius, detection signal, mitigation, owner).
- [ ] **Rollback strategy**: rollback path is fixed (rollback triggers, steps, expected recovery time, residual risk).
- [ ] **Owner sign-off**: explicit confirmation from the domain/platform owner for the chosen deviation.

The checklist is a mandatory gate and must be reflected:

- in the PR checklist of the contribution process: [`CONTRIBUTING.md`](../../CONTRIBUTING.md#pr-checklist);
- in the server governance documentation: [`apps/server/docs/README.md`](../../apps/server/docs/README.md).

---

## 7. MCP

### 7.1 FORBIDDEN: Business logic in MCP adapter

**SEVERITY: MEDIUM**

MCP layer is a thin adapter over service/registry. All logic belongs in domain services.

**Consequences:** Logic duplication, impossible to use the same rules without MCP.

---

### 7.2 FORBIDDEN: Bypassing typed tools (`McpToolResponse`)

**SEVERITY: MEDIUM**

```rust
// ❌ FORBIDDEN
return serde_json::json!({"result": "ok"});

// ✅ REQUIRED
return McpToolResponse::success(data);
```

**Consequences:** Client cannot parse the response, no error handling.

---

## 8. Telemetry

### 8.1 FORBIDDEN: Multiple telemetry initialization

**SEVERITY: HIGH**

Telemetry runtime (tracing subscriber, OTLP exporter) is initialized **exactly once** at server startup.

**Consequences:** Panic, span duplication, memory leak, incorrect metrics.

---

### 8.2 FORBIDDEN: Fragmented metrics registry

**SEVERITY: MEDIUM**

All Prometheus metrics go through a single registry. Do not create separate registries in modules.

**Consequences:** `/metrics` doesn't show some metrics, Grafana dashboards are empty.

---

## 9. DevOps

### 9.1 FORBIDDEN: Committing without `cargo fmt` and `cargo clippy`

**SEVERITY: MEDIUM**

```bash
# ❌ FORBIDDEN — commit without checking
git add . && git commit -m "changes"

# ✅ REQUIRED
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
# Only then commit
```

**Consequences:** Noisy diffs, hidden bugs, failed CI.

---

### 9.2 FORBIDDEN: Editing CI/CD workflow without explicit request

**SEVERITY: HIGH**

`.github/workflows/*.yml` — only on explicit request and with review.

**Consequences:** Broken CI for the entire team.

---

### 9.3 FORBIDDEN: Committing `.env` files with real credentials

**SEVERITY: CRITICAL**

`.gitignore` excludes `.env`. Only `.env.dev.example` with placeholder values.

**Consequences:** Production credential leak through git history.

---

## Pre-commit Checklist

Before each commit, ensure **none** of the prohibitions in this document are violated:

- [ ] No SQL without `tenant_id` filter
- [ ] No `unwrap()`/`expect()` in new production code
- [ ] No `publish()` instead of `publish_in_tx()` for business events
- [ ] No hardcoded secrets
- [ ] No endpoints without RBAC (except public)
- [ ] No blocking ops in async
- [ ] No logging of PII
- [ ] No business logic in controllers/resolvers (only service calls)
- [ ] No N+1 queries in GraphQL (DataLoader for related data)
- [ ] No list endpoints without pagination
- [ ] REST endpoints have `#[utoipa::path]` annotations
- [ ] REST and GraphQL auth logic are identical (via AuthLifecycleService)
- [ ] `cargo fmt` and `cargo clippy` passed
- [ ] Documentation updated when code changes

---

## Related Documents

- [Patterns vs Antipatterns](./patterns-vs-antipatterns.md) — summary table of correct and incorrect approaches
- [Code Standards](./coding.md) — detailed guide
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md) — traps for AI agents
- [Security Standards](./security.md) — OWASP coverage
- [Verification Plan](../verification/PLATFORM_VERIFICATION_PLAN.md) — main orchestration plan for periodic platform verification
