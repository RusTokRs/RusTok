# Стандарты Качества Кода RusToK

## Governance: language, naming, ownership-review

Этот раздел фиксирует обязательный governance baseline для изменений кода и
документации (DOC-10), чтобы снизить drift между runtime-контрактами и docs.

- **Язык документации:**
  - центральные документы в `docs/` ведутся на русском;
  - `README.md`, `AGENTS.md`, `CONTRIBUTING.md` и публичные контрактные файлы
    ведутся на английском;
  - один файл = один язык, смешение языков в одном файле не допускается.
- **Именование:**
  - query-keys и URL state в module-owned admin UI должны использовать typed
    `snake_case` ключи;
  - новые имена модулей/crate-ов/документов должны совпадать с `modules.toml`
    и `docs/modules/registry.md`.
- **Ownership-review path (обязательно для cross-cutting правок):**
  1. сначала обновить локальные docs компонента (`apps/*/docs` или
     `crates/*/docs`);
  2. затем синхронизировать центральные документы в `docs/`;
  3. при изменении модульной карты — обновить `docs/modules/registry.md`;
  4. получить review от owner затронутого модуля (или platform team для
     cross-cutting изменений).

Нарушение этих правил считается дефектом качества документации/контрактов и
должно блокировать merge до исправления.

## 1. Архитектурные Принципы

### 1.1 SOLID в Rust

```rust
// S - Single Responsibility
// ✅ Правильно: Один модуль = одна ответственность
pub mod order_service {
    pub async fn create_order() -> Result<Order> { }
    pub async fn cancel_order() -> Result<()> { }
}

pub mod order_repository {
    pub async fn save(order: &Order) -> Result<()> { }
    pub async fn find_by_id(id: Uuid) -> Result<Option<Order>> { }
}

// O - Open/Closed
// ✅ Правильно: Расширяем через trait, не модифицируя
pub trait PricingStrategy {
    fn calculate_price(&self, product: &Product, quantity: u32) -> Decimal;
}

pub struct StandardPricing;
pub struct VolumeDiscountPricing { threshold: u32, discount: Decimal };
pub struct SeasonalPricing { season: Season };

impl PricingStrategy for StandardPricing { }
impl PricingStrategy for VolumeDiscountPricing { }

// L - Liskov Substitution
// ✅ Правильно: Реализации взаимозаменяемы
pub trait CacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<()>;
}

// InMemoryCacheBackend и RedisCacheBackend взаимозаменяемы
fn configure_cache<B: CacheBackend>(backend: B) { }

// I - Interface Segregation
// ✅ Правильно: Мелкозернистые трейты
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

// Repository может имплементировать только нужное
#[async_trait]
pub trait Repository: Readable + Writable { }

// D - Dependency Inversion
// ✅ Правильно: Зависимость от абстракций
pub struct OrderService {
    repository: Arc<dyn OrderRepository>, // trait object
    event_bus: Arc<dyn EventBus>,         // trait object
}

// ❌ Неправильно: Зависимость от конкретики
pub struct BadOrderService {
    repository: PgOrderRepository,  // конкретный тип
    event_bus: KafkaEventBus,       // конкретный тип
}
```

### 1.2 Type Safety First

```rust
// ✅ Правильно: Newtype pattern для type safety
pub struct TenantId(Uuid);
pub struct UserId(Uuid);
pub struct OrderId(Uuid);

// Нельзя случайно передать UserId вместо TenantId
fn get_tenant(id: TenantId) -> Tenant { }

// ✅ Правильно: Phantom types для состояний
pub struct Order<S> {
    id: OrderId,
    state: S,
    _marker: PhantomData<S>,
}

pub struct Pending;
pub struct Confirmed;
pub struct Shipped;

// Только Pending можно подтвердить
impl Order<Pending> {
    pub fn confirm(self) -> Order<Confirmed> { }
}

// ❌ Неправильно: Stringly-typed
fn process_order(id: String, status: String) { }  // Легко перепутать
```

### 1.3 Zero-Cost Abstractions

```rust
// ✅ Правильно: Generic = zero-cost
pub struct Repository<T> {
    _phantom: PhantomData<T>,
}

impl<T: Entity> Repository<T> {
    pub async fn find(&self, id: T::Id) -> Result<Option<T>> { }
}

// Мономорфизация создаст оптимальный код для каждого типа

// ✅ Правильно: Inline для горячих путей
#[inline(always)]
pub fn calculate_hash(bytes: &[u8]) -> u64 {
    // ...
}

// ✅ Правильно: Const для compile-time вычислений
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;

// ❌ Неправильно: Runtime вычисление того, что можно сделать на этапе компиляции
pub fn get_max_retries() -> u32 { 3 }  // Лучше сделать const
```

## 2. Error Handling

### 2.1 Иерархия Ошибок

```rust
// ✅ Правильно: Иерархия от общего к специфичному
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

// ✅ Правильно: Контекст ошибки
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

// Использование
let user = repository
    .find_by_id(id)
    .context("Failed to find user")?;
```

### 2.2 Recoverable vs Unrecoverable

```rust
// ✅ Правильно: Panic только для программных ошибок
pub fn parse_config(contents: &str) -> Config {
    // Это баг в коде - должен быть Some
    let value = some_option.expect("config always has defaults");
}

// ✅ Правильно: Result для ожидаемых ошибок
pub async fn fetch_user(id: Uuid) -> Result<User> {
    match repository.find(id).await {
        Some(user) => Ok(user),
        None => Err(Error::NotFound),
    }
}

// ✅ Правильно: Option для nullable значений
pub fn find_admin(admins: &[User]) -> Option<&User> {
    admins.iter().find(|u| u.is_admin)
}
```

## 3. Async/Await Patterns

### 3.1 Cancellation Safety

```rust
// ✅ Правильно: Cancellation-safe операции
use tokio::select;

pub async fn process_with_timeout<T>(
    operation: impl Future<Output = T>,
    timeout: Duration,
) -> Result<T, TimeoutError> {
    tokio::time::timeout(timeout, operation).await
        .map_err(|_| TimeoutError::Elapsed)
}

// ✅ Правильно: Graceful shutdown
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

// ❌ Неправильно: Забыть про cancellation
pub async fn critical_operation() {
    let file = File::create("important.dat").await.unwrap();
    // Если future отменена здесь, файл останется открытым/битым
    file.write_all(b"data").await.unwrap();
}

// ✅ Правильно: Scope guard для cleanup
pub async fn critical_operation() -> Result<()> {
    let temp_path = tempfile::NamedTempFile::new()?.into_temp_path();
    
    {
        let file = File::create(&temp_path).await?;
        file.write_all(b"data").await?;
    } // файл закрыт
    
    // Атомарное переименование
    tokio::fs::rename(&temp_path, "important.dat").await?;
    // temp_path автоматически удалится при выходе из scope
    
    Ok(())
}
```

### 3.2 Spawn и Task Management

```rust
// ✅ Правильно: Именованные таски для отладки
let handle = tokio::task::Builder::new()
    .name("order-processor")
    .spawn(async move {
        process_orders(rx).await
    });

// ✅ Правильно: JoinSet для управления множеством тасков
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

// ❌ Неправильно: Неограниченный spawn
for order in orders {
    // Опасно: может создать тысячи тасков
    tokio::spawn(async move { process(order).await });
}

// ✅ Правильно: Semaphore для ограничения concurrency
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

### 4.1 Zero-Copy когда возможно

```rust
// ✅ Правильно: Borrowed data
pub fn parse_header(data: &[u8]) -> Result<Header<'_>> {
    // Нет копирования, только парсинг
    Ok(Header { raw: data })
}

// ✅ Правильно: Cow для гибкости
use std::borrow::Cow;

pub fn normalize_name(name: &str) -> Cow<'_, str> {
    if name.chars().all(|c| c.is_ascii_lowercase()) {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(name.to_lowercase())
    }
}

// ✅ Правильно: Bytes для сетевых данных
use bytes::Bytes;

pub fn process_chunk(data: Bytes) {
    // Arc под капотом - cheap clone
    let data2 = data.clone();
}
```

### 4.2 Arena Allocation

```rust
// ✅ Правильно: Bump allocator для краткосрочных данных
use bumpalo::Bump;

fn parse_large_file(contents: &str) -> Vec<Node> {
    let arena = Bump::new();
    let mut nodes = Vec::new();
    
    for line in contents.lines() {
        let node = arena.alloc(parse_node(line));
        nodes.push(node);
    }
    
    // Все данные очищаются за O(1)
    nodes
}
```

## 5. Performance Patterns

### 5.1 Lazy Evaluation

```rust
// ✅ Правильно: once_cell для lazy static
use once_cell::sync::Lazy;
use regex::Regex;

static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap()
});

// ✅ Правильно: tokio::sync::OnceCell для async
static DB_POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn get_pool() -> &'static PgPool {
    DB_POOL.get_or_init(|| async {
        PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap()
    }).await
}

// ✅ Правильно: itertools для ленивых операций
use itertools::Itertools;

let sum: i32 = (0..1_000_000)
    .filter(|n| n % 2 == 0)
    .map(|n| n * n)
    .take(100)
    .sum();
```

### 5.2 SIMD (where appropriate)

```rust
// ✅ Правильно: auto-vectorization
pub fn sum_array(arr: &[i32]) -> i32 {
    arr.iter().sum()  // Компилятор использует SIMD
}

// ✅ Правильно: explicit SIMD для hot paths
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub unsafe fn sum_simd(arr: &[i32]) -> i32 {
    // Реализация с AVX2
}
```

## 6. Testing Standards

### 6.1 Test Organization

```rust
// ✅ Правильно: Модульные тесты в том же файле
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

// ✅ Правильно: Интеграционные тесты в tests/
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
// ✅ Правильно: Proptest для инвариантов
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
/// Создает новый заказ в системе.
///
/// # Type Parameters
///
/// * `T` - Тип продукта, должен реализовывать `Product`
///
/// # Arguments
///
/// * `input` - Данные для создания заказа
/// * `ctx` - Контекст выполнения с tenant_id и user_id
///
/// # Returns
///
/// * `Ok(Order)` - Успешно созданный заказ
/// * `Err(OrderError::ProductNotFound)` - Продукт не существует
/// * `Err(OrderError::InsufficientInventory)` - Недостаточно товара
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
/// Эта функция вернет ошибку если:
/// - Продукт не найден в базе данных
/// - Недостаточно инвентаря для заказа
/// - Пользователь не имеет права `order:create`
///
/// # Performance
///
/// - O(1) для проверки прав доступа
/// - O(n) для резервирования инвентаря, где n = quantity
/// - Время выполнения обычно < 50ms для quantity < 1000
///
/// # Safety
///
/// Эта функция является безопасной и не использует unsafe код.
///
/// # Panics
///
/// Функция не должна паниковать при корректных входных данных.
/// Паника возможна только при нарушении инвариантов базы данных.
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
# ADR-001: Использование Type-State Pattern для Order

## Status
Accepted

## Context
Order может находиться в разных состояниях (Pending, Confirmed, Shipped, etc.).
Нужно гарантировать валидные переходы между состояниями на уровне типов.

## Decision
Использовать Type-State pattern с PhantomData:

```rust
pub struct Order<S> {
    id: OrderId,
    state: S,
    _marker: PhantomData<S>,
}
```

## Consequences

### Positive
- Ошибки переходов ловятся на этапе компиляции
- Нет runtime overhead
- Self-documenting code

### Negative
- Больше шаблонного кода
- Сложнее сериализация
```

## 8. Security Guidelines

### 8.1 Input Validation

```rust
// ✅ Правильно: Defense in depth
pub async fn create_order(&self, input: CreateOrderInput) -> Result<Order> {
    // 1. Синтаксическая валидация
    input.validate()?;  // validator crate
    
    // 2. Семантическая валидация
    if input.quantity == 0 {
        return Err(Error::InvalidQuantity);
    }
    
    // 3. Бизнес-валидация
    let product = self.get_product(input.product_id).await?;
    if input.quantity > product.max_order_quantity {
        return Err(Error::QuantityExceeded);
    }
    
    // 4. Авторизация
    self.authz.check_permission(&ctx.user, "order:create").await?;
    
    // ...
}
```

### 8.2 Secrets Management

```rust
// ✅ Правильно: Не хранить секреты в коде
pub struct Config {
    #[serde(skip_serializing)]
    pub database_password: SecretString,
}

// ✅ Правильно: Zeroize для чувствительных данных
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

| Метрика | Хорошо | Требует внимания | Плохо |
|---------|--------|------------------|-------|
| Функция | <20 строк | 20-40 строк | >40 строк |
| Модуль | <500 строк | 500-1000 строк | >1000 строк |
| Аргументы функции | <4 | 4-6 | >6 |
| Цикломатическая сложность | <10 | 10-20 | >20 |
| Публичные элементы | <20 | 20-40 | >40 |

### 9.2 Test Metrics

| Метрика | Минимум | Цель |
|---------|---------|------|
| Line coverage | 80% | 90% |
| Branch coverage | 70% | 85% |
| Mutation score | 60% | 80% |
| Test execution time | <5 мин | <2 мин |

---

## Чеклист Code Review

- [ ] Все публичные API задокументированы
- [ ] Обработка ошибок корректна
- [ ] Нет unwrap/expect в production коде
- [ ] Все unsafe блоки обоснованы и документированы
- [ ] Тесты покрывают новый функционал
- [ ] Клонирование минимизировано
- [ ] Нет блокирующих операций в async коде
- [ ] Логирование добавлено для важных операций
- [ ] Метрики добавлены для observability
- [ ] Секреты не захардкожены
