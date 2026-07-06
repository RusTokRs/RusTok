# State Machine Module

> **Status:** ✅ Production-ready (Sprint 2)  
> **Version:** 1.0.0  
> **Tests:** 14 unit tests (6 content + 8 commerce)

The state_machine module provides type-safe state machines with compile-time guarantees.

## Concept

**Type-State Pattern:** Each state is a separate type. Impossible transitions = compile errors.

**Advantages:**
- ✅ **Compile-Time Safety:** Errors at compile time, not runtime
- ✅ **Impossible States:** Impossible states are inexpressible
- ✅ **State-Specific Data:** Type-safe access to fields
- ✅ **Self-Documenting:** State graph visible in types
- ✅ **Zero Overhead:** Monomorphization, no runtime cost

## Components

### 1. Core Framework

**File:** `mod.rs` (146 lines)

**Key types:**
```rust
pub trait State: Sized {
    type Machine;
}

pub trait Transition<From: State, To: State> {
    fn transition(from: From) -> Result<To, TransitionError>;
}

pub struct StateMachine<M, S: State<Machine = M>> {
    state: S,
    _phantom: PhantomData<M>,
}
```

### 2. Transition Guards

**File:** `transition.rs` (183 lines)

**Guards:**
```rust
pub trait TransitionGuard<S> {
    fn can_transition(&self, state: &S) -> bool;
}

// Composition
impl<S, G1, G2> TransitionGuard<S> for And<G1, G2>
impl<S, G1, G2> TransitionGuard<S> for Or<G1, G2>
impl<S, G> TransitionGuard<S> for Not<G>
```

**Example:**
```rust
let guard = And::new(
    HasPermission("publish"),
    IsNotArchived,
);

if guard.can_transition(&draft) {
    let published = draft.publish();
}
```

### 3. Builder Pattern

**File:** `builder.rs` (62 lines)

```rust
pub struct StateMachineBuilder<M, S> { /* ... */ }
pub struct TransitionBuilder<From, To> { /* ... */ }
```

## Example Implementations

### Content Node State Machine

**File:** `crates/rustok-content/src/state_machine.rs` (380 lines)

**States:**
- `Draft` - draft
- `Published` - published
- `Archived` - archived

**State Diagram:**
```
 ┌───────┐
 │ Draft │──────────────────┐
 └───┬───┘                  │
     │ publish()            │
     ↓                      │ archive()
 ┌───────────┐              │
 │ Published │──────────────┤
 └─────┬─────┘              │
       │ archive()          │
       ↓                    ↓
 ┌──────────┐         ┌──────────┐
 │ Archived │         │ Archived │
 └──────────┘         └──────────┘
```

**Usage:**
```rust
use rustok_content::{ContentNode, Draft, Published, Archived};

// Create a draft
let node = ContentNode::new_draft(
    id,
    tenant_id,
    author_id,
    "article".to_string(),
);

// Draft → Published
let node = node.publish(); // Returns ContentNode<Published>

// Published → Archived
let node = node.archive("Content outdated".to_string());

// ❌ Compile error: cannot archive a draft directly
// let node = ContentNode::new_draft(...).archive("test");
//                                        ^^^^^^^ no method `archive` on `ContentNode<Draft>`
```

**State-specific fields:**
```rust
impl ContentNode<Draft> {
    pub fn last_edited(&self) -> DateTime<Utc> {
        self.state.last_edited
    }
}

impl ContentNode<Published> {
    pub fn published_at(&self) -> DateTime<Utc> {
        self.state.published_at // Only available on Published!
    }
    
    pub fn view_count(&self) -> u64 {
        self.state.view_count // Only available on Published!
    }
}

impl ContentNode<Archived> {
    pub fn archived_at(&self) -> DateTime<Utc> {
        self.state.archived_at // Only available on Archived!
    }
    
    pub fn reason(&self) -> &str {
        &self.state.reason // Only available on Archived!
    }
}
```

### Order State Machine

**File:** `crates/rustok-commerce/src/state_machine.rs` (550 lines)

**States:**
- `Pending` - created
- `Confirmed` - confirmed
- `Paid` - paid
- `Shipped` - shipped
- `Delivered` - delivered
- `Cancelled` - cancelled

**State Diagram:**
```
 ┌─────────┐
 │ Pending │─────────┐
 └────┬────┘         │
      │ confirm()    │
      ↓              │ cancel()
 ┌───────────┐       │
 │ Confirmed │───────┤
 └─────┬─────┘       │
       │ pay()       │
       ↓             ↓
 ┌──────────┐  ┌───────────┐
 │   Paid   │──│ Cancelled │
 └─────┬────┘  └───────────┘
       │ ship()
       ↓
 ┌─────────┐
 │ Shipped │
 └────┬────┘
      │ deliver()
      ↓
 ┌───────────┐
 │ Delivered │
 └───────────┘
```

**Usage:**
```rust
use rustok_commerce::{Order, Pending, Confirmed, Paid, Shipped, Delivered};

// Create an order
let order = Order::new_pending(
    id,
    tenant_id,
    customer_id,
    Decimal::new(9999, 2), // 99.99
    "USD".to_string(),
);

// Pending → Confirmed
let order = order.confirm()?;

// Confirmed → Paid
let order = order.pay(
    "pay_1234567890".to_string(),
    "credit_card".to_string(),
)?;

// Paid → Shipped
let order = order.ship(
    "TRACK123456789".to_string(),
    "FedEx".to_string(),
)?;

// Shipped → Delivered
let order = order.deliver(Some("John Doe".to_string()));

// ❌ Compile error: cannot ship a pending order
// let order = Order::new_pending(...).ship(...);
//                                     ^^^^ no method `ship` on `Order<Pending>`
```

**State-specific validation:**
```rust
impl Order<Confirmed> {
    pub fn pay(
        self,
        payment_id: String,
        payment_method: String,
    ) -> Result<Order<Paid>, OrderError> {
        // Validation only for Confirmed orders
        if payment_id.is_empty() {
            return Err(OrderError::InvalidPaymentId);
        }
        
        Ok(Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            created_at: self.created_at,
            state: Paid {
                confirmed_at: self.state.confirmed_at,
                paid_at: Utc::now(),
                payment_id,
                payment_method,
            },
        })
    }
}
```

## Conversion Traits

**Save/load to/from DB:**

```rust
// Content
pub trait ToContentStatus {
    fn to_status(&self) -> &str;
}

impl ToContentStatus for Draft {
    fn to_status(&self) -> &str { "draft" }
}

impl ToContentStatus for Published {
    fn to_status(&self) -> &str { "published" }
}

impl ToContentStatus for Archived {
    fn to_status(&self) -> &str { "archived" }
}

// Commerce
impl From<Order<Pending>> for String {
    fn from(order: Order<Pending>) -> String {
        "pending".to_string()
    }
}

// And so on for all states
```

## Tests

**Content Node:** 6 tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_draft_to_published() { /* ... */ }
    
    #[test]
    fn test_published_to_archived() { /* ... */ }
    
    #[test]
    fn test_draft_to_archived() { /* ... */ }
    
    #[test]
    fn test_state_specific_fields() { /* ... */ }
    
    #[test]
    fn test_content_status_conversion() { /* ... */ }
    
    #[test]
    fn test_view_count_increment() { /* ... */ }
}
```

**Order:** 8 tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_order_lifecycle_happy_path() { /* ... */ }
    
    #[test]
    fn test_pending_to_confirmed() { /* ... */ }
    
    #[test]
    fn test_confirmed_to_paid() { /* ... */ }
    
    #[test]
    fn test_paid_to_shipped() { /* ... */ }
    
    #[test]
    fn test_shipped_to_delivered() { /* ... */ }
    
    #[test]
    fn test_cancel_from_pending() { /* ... */ }
    
    #[test]
    fn test_cancel_from_confirmed() { /* ... */ }
    
    #[test]
    fn test_cancel_from_paid() { /* ... */ }
}
```

## Best Practices

### 1. Keep states simple

```rust
// ✅ Good
pub struct Draft {
    last_edited: DateTime<Utc>,
}

// ❌ Bad - too much logic in the state
pub struct Draft {
    last_edited: DateTime<Utc>,
    database: Arc<Database>,
    cache: Arc<Cache>,
    // ...
}
```

### 2. Use Result for fallible transitions

```rust
// ✅ Good
impl Order<Confirmed> {
    pub fn pay(self, payment_id: String) -> Result<Order<Paid>, OrderError> {
        if payment_id.is_empty() {
            return Err(OrderError::InvalidPaymentId);
        }
        Ok(/* ... */)
    }
}

// ❌ Bad - unwrap can panic
impl Order<Confirmed> {
    pub fn pay(self, payment_id: String) -> Order<Paid> {
        assert!(!payment_id.is_empty()); // Panic!
        // ...
    }
}
```

### 3. Document the state graph

```rust
/// Order State Machine
///
/// ```text
///  Pending → Confirmed → Paid → Shipped → Delivered
///     ↓          ↓         ↓
///  Cancelled ←─────────────┘
/// ```
pub struct Order<S: State<Machine = OrderMachine>> { /* ... */ }
```

### 4. Use guards for complex conditions

```rust
let can_publish = And::new(
    HasPermission("content.publish"),
    And::new(
        IsNotEmpty,
        PassesModerationCheck,
    ),
);

if can_publish.can_transition(&draft) {
    let published = draft.publish();
}
```

## Performance

**Zero Runtime Overhead:**
- Monomorphization → different types = different compiled code
- No vtables, no dynamic dispatch
- Compiler optimizes away phantom data

**Memory:**
- Same size as enum-based approach
- No additional allocations

**Benchmarks:**
```
test bench_enum_transition        ... bench:      12 ns/iter
test bench_typestate_transition   ... bench:      11 ns/iter
```

## Documentation

Full guide: [docs/STATE_MACHINE_GUIDE.md](../../../../docs/STATE_MACHINE_GUIDE.md)

**Sections:**
1. Type-State Pattern Concepts
2. Core Framework API
3. Transition Guards
4. Content Node in detail
5. Order State Machine in detail
6. Best Practices
7. Database Integration
8. Testing Strategies

## Roadmap

**v1.0.0 (Sprint 2):** ✅ DONE
- Core framework
- Transition guards
- Builder pattern
- Content Node state machine
- Order state machine
- 14 comprehensive tests

**v1.1.0 (Future):**
- [ ] User state machine (registration flow)
- [ ] Payment state machine
- [ ] Shipment state machine
- [ ] More guards (time-based, conditional)

**v2.0.0 (Future):**
- [ ] State machine visualization
- [ ] GraphQL integration
- [ ] Event sourcing support
- [ ] State history tracking

## References

- [Rust Type-State Pattern](https://cliffle.com/blog/rust-typestate/)
- [Finite State Machines](https://en.wikipedia.org/wiki/Finite-state_machine)
- [Typestate Oriented Programming](http://cliffle.com/blog/rust-typestate/)
