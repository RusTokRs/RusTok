---
id: doc://docs/standards/coding.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# RusToK Code Quality Standards

## Governance: language, naming, ownership-review

This section defines the mandatory governance baseline for code and documentation changes (DOC-10) to reduce drift between runtime contracts and docs.

- **Documentation language:**
  - all documentation is written in English;
  - `README.ru.md` is the only file allowed in Russian (localized translation of the main README);
  - one file = one language, mixed language within a single file is not allowed.
- **Naming:**
  - names must optimize for stable ownership, quick recognition, refactorability and search, not for minimum length or for encoding every detail;
  - query-keys and URL state in module-owned admin UI must use typed `snake_case` keys;
  - new module/crate/document names must match `modules.toml` and `docs/modules/registry.md`;
  - new names must follow the naming contract below.
- **Ownership-review path (mandatory for cross-cutting changes):**
  1. first update the component's local docs (`apps/*/docs` or `crates/*/docs`);
  2. then synchronize central documents in `docs/`;
  3. when changing the module map — update `docs/modules/registry.md`;
  4. get review from the affected module's owner (or platform team for cross-cutting changes).

Violation of these rules is considered a documentation/contract quality defect and must block merge until fixed.

### Naming Contract

Use the most useful name for day-to-day work: clear at the call site, stable
under refactoring, searchable, and no longer than needed. Minimum-length names
are not the goal. Names that repeat obvious path context or encode a full
implementation story are a design smell; put the explanation in the local
`README.md`, `docs/README.md`, manifest or registry instead of the filename or
identifier.

Mandatory formats:

| Surface | Format | Examples |
|---|---|---|
| Module slug | short `snake_case` | `product`, `order`, `page_builder` |
| Cargo crate/package directory | `kebab-case` | `rustok-product`, `rustok-page-builder` |
| Rust source file/module | `snake_case.rs` | `service.rs`, `graphql_adapter.rs` |
| Rust type/trait/enum | `PascalCase` | `ProductModule`, `CatalogService` |
| Rust function/variable/field | `snake_case` | `load_products`, `tenant_id` |
| JSON/TOML keys, URL query keys | `snake_case` | `tenant_id`, `page_builder` |
| Next/React component file | local frontend convention | `ProductTable.tsx` or existing app pattern |
| Documentation file | `kebab-case.md` | `module-authoring.md`, `runtime-contract.md` |
| ADR file | `YYYY-MM-DD-kebab-case.md` | `2026-07-01-runtime-boundary.md` |

Rules:

1. Do not repeat the domain name when the parent path already provides it.
   Use `crates/rustok-product/src/service.rs`, not
   `crates/rustok-product/src/product_service.rs`, unless the crate contains
   multiple peer services and the qualifier removes real ambiguity.
2. Prefer role names for internal files: `service.rs`, `ports.rs`, `dto.rs`,
   `entities.rs`, `graphql.rs`, `controllers.rs`, `permissions.rs`,
   `migrations/`, `transport/`, `ui/`.
3. Use underscores only where the language or data format expects them:
   Rust files/items, JSON/TOML/query keys, database names and generated
   migration identifiers. Use hyphens for package, crate directory and
   documentation filenames.
4. Treat names with more than three semantic parts as requiring justification.
   They are allowed when they improve call-site clarity or public API search,
   but they should not be caused by repeating path context. If a name needs four
   or more parts because unrelated concepts share one scope, split by directory
   or type boundary.
   Prefer `transport/graphql_adapter.rs` over
   `product_admin_transport_graphql_adapter.rs`.
5. Do not encode implementation status, architecture wave, temporary state or
   history in names. Use docs and plans for that. Avoid suffixes such as
   `new`, `old`, `legacy`, `v2`, `final`, `temp`, `experimental` unless the
   suffix is part of a public protocol version.
6. Do not introduce compatibility aliases with shorter or longer names just to
   preserve an old path. Rename atomically when the target architecture changes.
7. Keep module slugs domain-level and singular unless the business domain is
   inherently plural in platform vocabulary. Prefer `product`, `order`,
   `payment`, `page_builder`; avoid `product_catalog_management`.
8. Support crates may use an extra qualifier only for a real boundary:
   `rustok-commerce-foundation`, `rustok-seo-admin-support`,
   `rustok-graphql-leptos`.
9. A longer name is acceptable when it is the optimal working name: it improves
   call-site clarity, distinguishes peer concepts, supports public API search,
   or preserves necessary protocol meaning. Generated migration files, evidence
   packet fixtures and public API compatibility identifiers commonly need this.

#### Code Identifier Rules

Rust `snake_case` and `PascalCase` are mandatory style rules, but they do not
justify long or repetitive names. Code identifiers must follow the
optimal-working-name rule: expressive enough to read without guessing, compact
enough to scan, and free of context already supplied by the path, module or
receiver type.

Case style is not a semantic difference. The same concept should keep the same
stem across identifier kinds:

| Concept | Type name | Builder/function | Variable |
|---|---|---|---|
| publish command | `PublishCommand` | `build_publish_command` | `publish_command` |
| pricing preview request | `PricingPreviewRequest` | `pricing_preview_request_from_product` | `pricing_preview_request` |
| route segment | `RouteSegment` | `resolve_route_segment` | `route_segment` |

Do not use different stems for the same concept in the same scope, such as
`PublishCommand`, `publish_status_command` and `status_publish_payload` for one
prepared publish command.

1. Do not repeat path context in internal code identifiers.
   In `crates/rustok-product/admin/src/core.rs`, prefer
   `StatusResultViewModel`, `build_status_result_view_model` and
   `parse_inventory_quantity` over
   `ProductAdminStatusMutationResultViewModel`,
   `build_product_admin_status_mutation_result_view_model` and
   `parse_product_admin_inventory_quantity_input`.
2. Repeat the domain in public API only when the caller cannot infer the owner
   from the type, trait or module path. `ProductModule`, `ProductReadPort` and
   `create_product` are acceptable at crate boundaries. Repeating `product` in
   every private helper inside `rustok-product` is not.
3. Repeat a surface qualifier only at the boundary between surfaces. `AdminView`
   and `StorefrontView` are acceptable in a shared host or shared support crate.
   Inside `admin/src/*`, the `admin` qualifier is normally redundant.
4. Prefer role-first helper names inside a scoped file, but keep enough
   information to distinguish peer operations:
   `build_editor_view_model`, `selected_query_state`, `build_list_controls`,
   `build_save_command`, `build_delete_result`, `shipping_profiles_load_view`.
   Do not collapse to vague names such as `build_view`, `handle`, `run` or
   `process`.
5. Use standard function verbs consistently:
   - `build_*` creates a new view model, command, copy bundle or prepared value
     from inputs without side effects;
   - `parse_*` converts text or untrusted input into typed local state;
   - `resolve_*` chooses one value from precedence/fallback rules;
   - `format_*` returns display text;
   - `map_*` converts between transport/domain shapes;
   - `load_*` / `fetch_*` perform IO or call transport.
   Do not alternate `make_*`, `create_*`, `prepare_*`, `compose_*` and `build_*`
   for the same kind of pure view-model construction in one module.
6. Public DTO/type names may be longer than helper names, but should still avoid
   duplicated concepts. `StatusMutationResultViewModel` can be optimal if the
   same scope also contains non-mutation status result types. If mutation is the
   only status operation in that scope, prefer `StatusResultViewModel`.
7. Adapter functions should name the transport only when multiple transports are
   visible from the same scope. Inside `transport/native_server_adapter.rs`,
   prefer `clear_detached_values` over
   `product_admin_clear_detached_attribute_values_native`. The filename already
   carries `native`.
8. Test names may be descriptive, but should assert one behavior and avoid
   repeating the module/surface prefix. Prefer
   `delete_result_formats_failures` over
   `product_admin_delete_result_view_model_formats_failures`.
9. Avoid names ending in generic filler when a shorter role is enough:
   `data`, `info`, `manager`, `handler`, `helper`, `util`, `processor`.
   Use these only when the role is genuinely generic at that boundary.
10. File splitting is not a substitute for naming discipline. If a name grows
   only because one file contains too many peer concepts, split the file. If a
   name remains long after the split, keep the longer name only when it improves
   call-site clarity or distinguishes real peer concepts.
11. Before adding a new long identifier, ask which part is already expressed by
    the path, module, type or trait. Remove that part from the identifier unless
    it is required for public API clarity.

## 1. Architectural Principles

### 1.1 SOLID in Rust

```rust
// S - Single Responsibility
// ✅ Correct: One module = one responsibility
pub mod order_service {
    pub async fn create_order() -> Result<Order> { }
    pub async fn cancel_order() -> Result<()> { }
}

pub mod order_repository {
    pub async fn save(order: &Order) -> Result<()> { }
    pub async fn find_by_id(id: Uuid) -> Result<Option<Order>> { }
}

// O - Open/Closed
// ✅ Correct: Extend via trait, not modification
pub trait PricingStrategy {
    fn calculate_price(&self, product: &Product, quantity: u32) -> Decimal;
}

pub struct StandardPricing;
pub struct VolumeDiscountPricing { threshold: u32, discount: Decimal };
pub struct SeasonalPricing { season: Season };

impl PricingStrategy for StandardPricing { }
impl PricingStrategy for VolumeDiscountPricing { }

// L - Liskov Substitution
// ✅ Correct: Implementations are interchangeable
pub trait CacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;
}

// InMemoryCacheBackend and RedisCacheBackend are interchangeable
fn configure_cache<B: CacheBackend>(backend: B) { }

// I - Interface Segregation
// ✅ Correct: Fine-grained traits
#[async_trait]
pub trait Readable {
    async fn read(&self, id: Uuid) -> Result<Option<Entity>>;
}

#[async_trait]
pub trait Writable {
    async fn write(&self, entity: &Entity) -> Result<()>;
}

#[async_trait]
pub trait Deletable {
    async fn delete(&self, id: Uuid) -> Result<()>;
}

// Repository can implement only what's needed
#[async_trait]
pub trait Repository: Readable + Writable { }

// D - Dependency Inversion
// ✅ Correct: Depend on abstractions
pub struct OrderService {
    repository: Arc<dyn OrderRepository>, // trait object
    event_bus: Arc<dyn EventBus>,         // trait object
}

// ❌ Incorrect: Depend on concretions
pub struct BadOrderService {
    repository: PgOrderRepository,  // concrete type
    event_bus: KafkaEventBus,       // concrete type
}
```

### 1.2 Type Safety First

```rust
// ✅ Correct: Newtype pattern for type safety
pub struct TenantId(Uuid);
pub struct UserId(Uuid);
pub struct OrderId(Uuid);

// Cannot accidentally pass UserId instead of TenantId
fn get_tenant(id: TenantId) -> Tenant { }

// ✅ Correct: Phantom types for states
pub struct Order<S> {
    id: OrderId,
    state: S,
    _marker: PhantomData<S>,
}

pub struct Pending;
pub struct Confirmed;
pub struct Shipped;

// Only Pending can be confirmed
impl Order<Pending> {
    pub fn confirm(self) -> Order<Confirmed> { }
}

// ❌ Incorrect: Stringly-typed
fn process_order(id: String, status: String) { }  // Easy to mix up
```

### 1.3 Zero-Cost Abstractions

```rust
// ✅ Correct: Generic = zero-cost
pub struct Repository<T> {
    _phantom: PhantomData<T>,
}

impl<T: Entity> Repository<T> {
    pub async fn find(&self, id: T::Id) -> Result<Option<T>> { }
}

// Monomorphization creates optimal code for each type

// ✅ Correct: Inline for hot paths
#[inline(always)]
pub fn calculate_hash(bytes: &[u8]) -> u64 {
    // ...
}

// ✅ Correct: Const for compile-time computations
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

// ❌ Incorrect: Runtime computation of something that can be done at compile time
pub fn get_max_retries() -> u32 { 3 }  // Better to make const
```

## 2. Error Handling

### 2.1 Error Hierarchy

```rust
// ✅ Correct: Hierarchy from general to specific
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    
    #[error("External service error: {0}")]
    External(#[from] ExternalError),
    
    #[error("Internal error: {0}")]
    Internal(#[from] InternalError),
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    Connection(String),
    
    #[error("Query failed: {0}")]
    Query(String),
    
    #[error("Constraint violation: {0}")]
    Constraint(String),
}

// ✅ Correct: Error context
type Result<T> = std::result::Result<T, AppError>;

pub trait Context<T> {
    fn context(self, msg: impl Into<String>) -> Result<T>;
}

impl<T, E: Into<AppError>> Context<T> for std::result::Result<T, E> {
    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let error = e.into();
            tracing::error!(error = %error, context = %msg.into(), "Operation failed");
            error
        })
    }
}

// Usage
let user = repository
    .find_by_id(id)
    .context("Failed to find user")?;
```

### 2.2 Recoverable vs Unrecoverable

```rust
// ✅ Correct: Panic only for programming errors
pub fn parse_config(contents: &str) -> Config {
    // This is a code bug - should always be Some
    let value = some_option.expect("config always has defaults");
}

// ✅ Correct: Result for expected errors
pub async fn fetch_user(id: Uuid) -> Result<User> {
    match repository.find(id).await {
        Some(user) => Ok(user),
        None => Err(Error::NotFound),
    }
}

// ✅ Correct: Option for nullable values
pub fn find_admin(admins: &[User]) -> Option<&User> {
    admins.iter().find(|u| u.is_admin)
}
```

## 3. Async/Await Patterns

### 3.1 Cancellation Safety

```rust
// ✅ Correct: Cancellation-safe operations
use tokio::select;

pub async fn process_with_timeout<T>(
    operation: impl Future<Output = T>,
    timeout: Duration,
) -> Result<T, TimeoutError> {
    tokio::time::timeout(timeout, operation).await
        .map_err(|_| TimeoutError::Elapsed)
}

// ✅ Correct: Graceful shutdown
pub async fn run_service(mut rx: mpsc::Receiver<Command>) {
    loop {
        tokio::select! {
            Some(cmd) = rx.recv() => {
                self.handle_command(cmd).await;
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, finishing pending work...");
                self.graceful_shutdown().await;
                break;
            }
        }
    }
}

// ❌ Incorrect: Forget about cancellation
pub async fn critical_operation() {
    let file = File::create("important.dat").await.unwrap();
    // If future is cancelled here, the file remains open/broken
    file.write_all(b"data").await.unwrap();
}

// ✅ Correct: Scope guard for cleanup
pub async fn critical_operation() -> Result<()> {
    let temp_path = tempfile::NamedTempFile::new()?.into_temp_path();
    
    {
        let file = File::create(&temp_path).await?;
        file.write_all(b"data").await?;
    } // file closed
    
    // Atomic rename
    tokio::fs::rename(&temp_path, "important.dat").await?;
    // temp_path is automatically deleted when leaving scope
    
    Ok(())
}
```

### 3.2 Spawn and Task Management

```rust
// ✅ Correct: Named tasks for debugging
let handle = tokio::task::Builder::new()
    .name("order-processor")
    .spawn(async move {
        process_orders(rx).await
    });

// ✅ Correct: JoinSet for managing multiple tasks
use tokio::task::JoinSet;

async fn process_batch(orders: Vec<Order>) -> Vec<Result<Receipt>> {
    let mut set = JoinSet::new();
    
    for order in orders {
        set.spawn(async move {
            process_order(order).await
        });
    }
    
    let mut results = vec![];
    while let Some(result) = set.join_next().await {
        results.push(result.unwrap_or_else(|e| Err(e.into())));
    }
    
    results
}

// ❌ Incorrect: Unlimited spawn
for order in orders {
    // Dangerous: may create thousands of tasks
    tokio::spawn(async move { process(order).await });
}

// ✅ Correct: Semaphore for limiting concurrency
use tokio::sync::Semaphore;

async fn process_limited(orders: Vec<Order>, limit: usize) {
    let semaphore = Arc::new(Semaphore::new(limit));
    
    let handles: Vec<_> = orders
        .into_iter()
        .map(|order| {
            let sem = Arc::clone(&semaphore);
            tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                process(order).await
            })
        })
        .collect();
    
    for handle in handles {
        handle.await.unwrap();
    }
}
```

## 4. Memory Management

### 4.1 Zero-Copy When Possible

```rust
// ✅ Correct: Borrowed data
pub fn parse_header(data: &[u8]) -> Result<Header<'_>> {
    // No copying, only parsing
    Ok(Header { raw: data })
}

// ✅ Correct: Cow for flexibility
use std::borrow::Cow;

pub fn normalize_name(name: &str) -> Cow<'_, str> {
    if name.chars().all(|c| c.is_ascii_lowercase()) {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(name.to_lowercase())
    }
}

// ✅ Correct: Bytes for network data
use bytes::Bytes;

pub fn process_chunk(data: Bytes) {
    // Arc under the hood - cheap clone
    let data2 = data.clone();
}
```

### 4.2 Arena Allocation

```rust
// ✅ Correct: Bump allocator for short-lived data
use bumpalo::Bump;

fn parse_large_file(contents: &str) -> Vec<Node> {
    let arena = Bump::new();
    let mut nodes = Vec::new();
    
    for line in contents.lines() {
        let node = arena.alloc(parse_node(line));
        nodes.push(node);
    }
    
    // All data is cleared in O(1)
    nodes
}
```

## 5. Performance Patterns

### 5.1 Lazy Evaluation

```rust
// ✅ Correct: once_cell for lazy static
use once_cell::sync::Lazy;
use regex::Regex;

static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
});

// ✅ Correct: tokio::sync::OnceCell for async
static DB_POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn get_pool() -> &'static PgPool {
    DB_POOL.get_or_init(|| async {
        PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap()
    }).await
}

// ✅ Correct: itertools for lazy operations
use itertools::Itertools;

let sum: i32 = (0..1_000_000)
    .filter(|n| n % 2 == 0)
    .map(|n| n * n)
    .take(100)
    .sum();
```

### 5.2 SIMD (where appropriate)

```rust
// ✅ Correct: auto-vectorization
pub fn sum_array(arr: &[i32]) -> i32 {
    arr.iter().sum()  // Compiler uses SIMD
}

// ✅ Correct: explicit SIMD for hot paths
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub unsafe fn sum_simd(arr: &[i32]) -> i32 {
    // AVX2 implementation
}
```

## 6. Testing Standards

### 6.1 Test Organization

```rust
// ✅ Correct: Unit tests in the same file
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_total() {
        // Arrange
        let items = vec![
            Item { price: 100, qty: 2 },
            Item { price: 50, qty: 1 },
        ];
        
        // Act
        let total = calculate_total(&items);
        
        // Assert
        assert_eq!(total, 250);
    }
    
    #[test]
    #[should_panic(expected = "overflow")]
    fn test_calculate_total_overflow() {
        let items = vec![Item { price: u64::MAX, qty: 2 }];
        calculate_total(&items);
    }
}

// ✅ Correct: Integration tests in tests/
// tests/order_integration.rs

#[tokio::test]
async fn test_create_order_flow() {
    let app = TestApp::new().await;
    
    let response = app
        .post("/orders")
        .json(&json!({
            "product_id": app.test_product.id,
            "quantity": 2
        }))
        .send()
        .await;
    
    assert_eq!(response.status(), 201);
    
    let order: Order = response.json().await;
    assert_eq!(order.quantity, 2);
}
```

### 6.2 Property-Based Testing

```rust
// ✅ Correct: Proptest for invariants
use proptest::prelude::*;

proptest! {
    #[test]
    fn total_always_non_negative(
        items in prop::collection::vec(
            (0u32..1000, 0u32..100),
            0..100
        )
    ) {
        let items: Vec<Item> = items
            .into_iter()
            .map(|(p, q)| Item { price: p, qty: q })
            .collect();
        
        let total = calculate_total(&items);
        prop_assert!(total >= 0);
    }
    
    #[test]
    fn idempotent_operation(
        input in any::<String>()
    ) {
        // f(f(x)) == f(x)
        let once = normalize(&input);
        let twice = normalize(&once);
        prop_assert_eq!(once, twice);
    }
}
```

## 7. Documentation Standards

### 7.1 Doc Comments

```rust
/// Creates a new order in the system.
///
/// # Type Parameters
///
/// * `T` - Product type, must implement `Product`
///
/// # Arguments
///
/// * `input` - Data for creating the order
/// * `ctx` - Execution context with tenant_id and user_id
///
/// # Returns
///
/// * `Ok(Order)` - Successfully created order
/// * `Err(OrderError::ProductNotFound)` - Product does not exist
/// * `Err(OrderError::InsufficientInventory)` - Insufficient stock
///
/// # Examples
///
/// ```rust
/// use rust_decimal::Decimal;
/// use rustok_commerce::OrderService;
/// use rustok_order::dto::{CreateOrderInput, CreateOrderLineItemInput};
/// use std::str::FromStr;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let service = OrderService::new(db, event_bus);
/// let order = service
///     .create_order(
///         CreateOrderInput {
///             customer_id: None,
///             currency_code: "USD".to_string(),
///             line_items: vec![CreateOrderLineItemInput {
///                 product_id: Some(product.id),
///                 variant_id: Some(variant.id),
///                 shipping_profile_slug: "default".to_string(),
///                 seller_id: None,
///                 sku: Some("SKU-1".to_string()),
///                 title: "Example item".to_string(),
///                 quantity: 2,
///                 unit_price: Decimal::from_str("19.99")?,
///                 metadata: serde_json::json!({ "source": "docs" }),
///             }],
///             adjustments: Vec::new(),
///             tax_lines: Vec::new(),
///             metadata: serde_json::json!({ "source": "docs" }),
///         },
///         &context,
///     )
///     .await?;
///
/// assert_eq!(order.line_items.len(), 1);
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// This function will return an error if:
/// - The product is not found in the database
/// - There is insufficient inventory for the order
/// - The user does not have the `order:create` permission
///
/// # Performance
///
/// - O(1) for permission check
/// - O(n) for inventory reservation, where n = quantity
/// - Typical execution time < 50ms for quantity < 1000
///
/// # Safety
///
/// This function is safe and does not use unsafe code.
///
/// # Panics
///
/// The function should not panic with correct input data.
/// Panics are only possible if database invariants are violated.
#[instrument(skip(self, input), fields(order.product_id = %input.product_id))]
pub async fn create_order<T: Product>(
    &self,
    input: CreateOrderInput,
    ctx: &ExecutionContext,
) -> Result<Order, OrderError> {
    // ...
}
```

### 7.2 Architecture Decision Records

```markdown
# ADR-001: Using Type-State Pattern for Order

## Status
Accepted

## Context
Order can be in different states (Pending, Confirmed, Shipped, etc.).
Need to guarantee valid state transitions at the type level.

## Decision
Use Type-State pattern with PhantomData:

```rust
pub struct Order<S> {
    id: OrderId,
    state: S,
    _marker: PhantomData<S>,
}
```

## Consequences

### Positive
- Transition errors are caught at compile time
- No runtime overhead
- Self-documenting code

### Negative
- More boilerplate
- More complex serialization
```

## 8. Security Guidelines

### 8.1 Input Validation

```rust
// ✅ Correct: Defense in depth
pub async fn create_order(&self, input: CreateOrderInput) -> Result<Order> {
    // 1. Syntactic validation
    input.validate()?;  // validator crate
    
    // 2. Semantic validation
    if input.quantity == 0 {
        return Err(Error::InvalidQuantity);
    }
    
    // 3. Business validation
    let product = self.get_product(input.product_id).await?;
    if input.quantity > product.max_order_quantity {
        return Err(Error::QuantityExceeded);
    }
    
    // 4. Authorization
    self.authz.check_permission(&ctx.user, "order:create").await?;
    
    // ...
}
```

### 8.2 Secrets Management

```rust
// ✅ Correct: Do not store secrets in code
pub struct Config {
    #[serde(skip_serializing)]
    pub database_password: SecretString,
}

// ✅ Correct: Zeroize for sensitive data
use zeroize::Zeroize;

pub struct ApiKey([u8; 32]);

impl Drop for ApiKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
```

## 9. Metrics

### 9.1 Code Metrics

| Metric | Good | Needs Attention | Bad |
|---------|--------|------------------|-------|
| Function | <20 lines | 20-40 lines | >40 lines |
| Module | <500 lines | 500-1000 lines | >1000 lines |
| Function arguments | <4 | 4-6 | >6 |
| Cyclomatic complexity | <10 | 10-20 | >20 |
| Public items | <20 | 20-40 | >40 |

### 9.2 Test Metrics

| Metric | Minimum | Target |
|---------|---------|------|
| Line coverage | 80% | 90% |
| Branch coverage | 70% | 85% |
| Mutation score | 60% | 80% |
| Test execution time | <5 min | <2 min |

---

## Code Review Checklist

- [ ] All public APIs are documented
- [ ] Error handling is correct
- [ ] No unwrap/expect in production code
- [ ] All unsafe blocks are justified and documented
- [ ] Tests cover new functionality
- [ ] Cloning is minimized
- [ ] No blocking operations in async code
- [ ] Logging added for important operations
- [ ] Metrics added for observability
- [ ] Secrets are not hardcoded
