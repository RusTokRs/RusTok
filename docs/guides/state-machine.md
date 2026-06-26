---
id: doc://docs/guides/state-machine.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Type-Safe State Machine Guide

## Overview

Type-safe state machines provide **compile-time guarantees** for state transitions, making illegal states unrepresentable and invalid transitions impossible.

## Benefits

### 🛡️ Compile-Time Safety
- **Invalid transitions are compile errors**
- **Impossible states don't exist**
- **No runtime panics** from bad state logic
- **Refactoring confidence** - compiler catches breaks

### 📝 Self-Documenting
- **State graph visible in type system**
- **Available transitions are methods**
- **IDE autocomplete shows valid actions**
- **Documentation in code structure**

### 🎯 State-Specific Data
- Each state has its own fields
- Published content has `published_at`
- Shipped orders have `tracking_number`
- Type system enforces data presence

### 🔍 Better Testing
- Test state transitions, not state validation
- Compile errors catch invalid test cases
- Property-based testing easier
- Integration tests clearer

## Examples

### Content Node State Machine

State diagram:
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
       │                    ↓
       │              ┌──────────┐
       └─────────────→│ Archived │
                      └──────────┘
```

#### Usage

```rust
use rustok_content::{ContentNode, Draft, Published, Archived};

// Create new content in draft state
let node = ContentNode::new_draft(
    id,
    tenant_id,
    Some(author_id),
    "article".to_string(),
);

// Publish (only available on Draft)
let node = node.publish();

// Archive with reason (only available on Published)
let node = node.archive("Content outdated".to_string());

// Restore to draft (only available on Archived)
let node = node.restore_to_draft();

// ❌ Compile error: can't archive draft directly
// let node = node.archive("test"); // No method `archive` on `ContentNode<Draft>`

// ❌ Compile error: can't publish archived content
// let node = node.publish(); // No method `publish` on `ContentNode<Archived>`
```

#### State-Specific Data

```rust
// Draft state has creation/update timestamps
struct Draft {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Published state has publication timestamp
struct Published {
    pub published_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Archived state has reason
struct Archived {
    pub archived_at: DateTime<Utc>,
    pub reason: String,
}
```

### Order State Machine

State diagram:
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
       ↓             │
 ┌──────────┐        │
 │   Paid   │────────┤
 └─────┬────┘        │
       │ ship()      │
       ↓             │
 ┌─────────┐         │
 │ Shipped │─────────┤
 └────┬────┘         │
      │ deliver()    │
      ↓              │
 ┌───────────┐       │
 │ Delivered │       │
 └───────────┘       │
                     ↓
               ┌───────────┐
               │ Cancelled │
               └───────────┘
```

#### Usage

```rust
use rustok_commerce::{Order, Pending, Confirmed, Paid, Shipped};
use rust_decimal::Decimal;

// Create new order
let order = Order::new_pending(
    id,
    tenant_id,
    customer_id,
    Decimal::new(10000, 2), // $100.00
    "USD".to_string(),
);

// Confirm order (validates inventory)
let order = order.confirm()?;

// Process payment
let order = order.pay(
    "pay_123".to_string(),
    "credit_card".to_string(),
)?;

// Ship order (only available after payment!)
let order = order.ship(
    "TRACK123".to_string(),
    "FedEx".to_string(),
)?;

// Mark as delivered
let order = order.deliver(Some("John Doe".to_string()));

// ❌ Compile error: can't ship unpaid order
// let order = order.ship("TRACK123", "FedEx"); // No method on `Order<Confirmed>`

// ❌ Compile error: can't deliver before shipping
// let order = order.deliver(None); // No method on `Order<Paid>`
```

#### State-Specific Data

```rust
// Pending state
struct Pending {
    pub created_at: DateTime<Utc>,
}

// Paid state has payment info
struct Paid {
    pub paid_at: DateTime<Utc>,
    pub payment_id: String,
    pub payment_method: String,
}

// Shipped state has tracking info
struct Shipped {
    pub shipped_at: DateTime<Utc>,
    pub tracking_number: String,
    pub carrier: String,
}
```

## Implementation Patterns

### Pattern 1: Type Parameter

```rust
// Define states
struct Draft;
struct Published { published_at: DateTime<Utc> }

// Generic machine with type parameter
struct Document<S> {
    id: Uuid,
    title: String,
    state: S,
}

// Implement transitions per state
impl Document<Draft> {
    fn publish(self) -> Document<Published> {
        Document {
            id: self.id,
            title: self.title,
            state: Published {
                published_at: Utc::now(),
            },
        }
    }
}
```

### Pattern 2: Enum (Anti-pattern)

❌ **Don't do this** - loses compile-time safety:

```rust
// BAD: Uses enum, allows invalid transitions at runtime
enum DocumentState {
    Draft,
    Published { published_at: DateTime<Utc> },
    Archived { reason: String },
}

struct Document {
    id: Uuid,
    state: DocumentState, // ❌ Can transition to any state
}

impl Document {
    fn publish(&mut self) {
        match &self.state {
            DocumentState::Draft => {
                self.state = DocumentState::Published {
                    published_at: Utc::now(),
                };
            }
            // ⚠️ Must handle all cases, including invalid ones
            DocumentState::Published { .. } => {
                // What to do? Panic? Return error? Already published?
            }
            DocumentState::Archived { .. } => {
                // What to do? Can archived docs be republished?
            }
        }
    }
}
```

### Pattern 3: Builder + Guards

```rust
use rustok_core::state_machine::{TransitionGuard, Transition};

// Define guard
struct HasInventory;

impl TransitionGuard<Order<Confirmed>> for HasInventory {
    fn can_transition(&self, order: &Order<Confirmed>) -> bool {
        order.state.inventory_reserved
    }
    
    fn error_message(&self) -> String {
        "Order must have inventory reserved".to_string()
    }
}

// Use guard in transition
impl Order<Confirmed> {
    fn pay(self, payment_id: String) -> Result<Order<Paid>, OrderError> {
        let guard = HasInventory;
        
        if !guard.can_transition(&self) {
            return Err(OrderError::GuardFailed(guard.error_message()));
        }
        
        // Process payment...
        Ok(Order {
            id: self.id,
            // ...
            state: Paid { /* ... */ },
        })
    }
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_draft_to_published() {
        let node = ContentNode::new_draft(/* ... */);
        let node = node.publish();
        
        assert_eq!(node.to_status(), ContentStatus::Published);
        assert!(node.state.published_at <= Utc::now());
    }
    
    #[test]
    fn test_full_order_lifecycle() {
        let order = Order::new_pending(/* ... */)
            .confirm().unwrap()
            .pay("pay_123".into(), "card".into()).unwrap()
            .ship("TRACK".into(), "FedEx".into()).unwrap()
            .deliver(None);
        
        // Delivered state
        assert!(order.state.delivered_at <= Utc::now());
    }
    
    // Compile-time safety tests (commented out, should not compile)
    
    // #[test]
    // fn test_invalid_transition() {
    //     let order = Order::new_pending(/* ... */);
    //     // ❌ Compile error
    //     let order = order.ship("TRACK".into(), "FedEx".into());
    // }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_any_order_can_be_cancelled(
        id: Uuid,
        customer_id: Uuid,
        amount in 1..1000000i64,
    ) {
        let order = Order::new_pending(
            id,
            Uuid::new_v4(),
            customer_id,
            Decimal::new(amount, 2),
            "USD".into(),
        );
        
        let cancelled = order.cancel("Test".to_string());
        
        prop_assert_eq!(cancelled.state.reason, "Test");
        prop_assert!(!cancelled.state.refunded);
    }
}
```

## Database Integration

### Storing Type-Safe States

Option 1: Store enum + metadata JSON

```rust
// Database model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
pub struct Model {
    pub id: Uuid,
    pub status: OrderStatus, // Enum: Pending, Confirmed, etc.
    pub state_data: Json,    // State-specific data
}

// Convert from type-safe to database
impl<S: Serialize> Order<S> {
    pub fn to_model(&self) -> Model {
        Model {
            id: self.id,
            status: self.status_enum(),
            state_data: serde_json::to_value(&self.state).unwrap(),
        }
    }
}

// Convert from database to type-safe
impl Order<Pending> {
    pub fn from_model_pending(model: Model) -> Result<Self, Error> {
        let state: Pending = serde_json::from_value(model.state_data)?;
        
        Ok(Order {
            id: model.id,
            // ...
            state,
        })
    }
}
```

Option 2: Separate state tables

```sql
CREATE TABLE orders (
    id UUID PRIMARY KEY,
    status VARCHAR(50) NOT NULL
);

CREATE TABLE order_state_pending (
    order_id UUID PRIMARY KEY REFERENCES orders(id),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE order_state_paid (
    order_id UUID PRIMARY KEY REFERENCES orders(id),
    paid_at TIMESTAMPTZ NOT NULL,
    payment_id VARCHAR(255) NOT NULL
);
```

### Example: Service Layer

```rust
pub struct OrderService {
    db: DatabaseConnection,
}

impl OrderService {
    /// Create new order
    pub async fn create_order(
        &self,
        customer_id: Uuid,
        items: Vec<OrderItem>,
    ) -> Result<Order<Pending>, CommerceError> {
        let id = Uuid::new_v4();
        let total = items.iter().map(|i| i.price).sum();
        
        let order = Order::new_pending(
            id,
            Uuid::new_v4(), // tenant_id from context
            customer_id,
            total,
            "USD".to_string(),
        );
        
        // Save to database
        self.save_order(&order).await?;
        
        Ok(order)
    }
    
    /// Confirm order (with business logic)
    pub async fn confirm_order(
        &self,
        order: Order<Pending>,
    ) -> Result<Order<Confirmed>, CommerceError> {
        // Business logic: check inventory
        let has_inventory = self.check_inventory(&order).await?;
        
        if !has_inventory {
            return Err(CommerceError::InsufficientInventory);
        }
        
        // Transition state
        let order = order.confirm()?;
        
        // Save to database
        self.save_order(&order).await?;
        
        Ok(order)
    }
}
```

## Migration Guide

### From Simple Enum

**Before:**
```rust
#[derive(Debug, Clone)]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Paid,
    Shipped,
    Delivered,
    Cancelled,
}

pub struct Order {
    pub id: Uuid,
    pub status: OrderStatus,
    // Optional fields that may not exist in all states
    pub paid_at: Option<DateTime<Utc>>,
    pub payment_id: Option<String>,
    pub tracking_number: Option<String>,
}

impl Order {
    pub fn ship(&mut self, tracking: String) -> Result<(), Error> {
        match self.status {
            OrderStatus::Paid => {
                self.status = OrderStatus::Shipped;
                self.tracking_number = Some(tracking);
                Ok(())
            }
            // ⚠️ Must handle invalid states
            _ => Err(Error::InvalidTransition),
        }
    }
}
```

**After:**
```rust
pub struct Order<S> {
    pub id: Uuid,
    pub state: S,
}

pub struct Paid {
    pub paid_at: DateTime<Utc>,
    pub payment_id: String,
}

pub struct Shipped {
    pub shipped_at: DateTime<Utc>,
    pub tracking_number: String,
}

impl Order<Paid> {
    // Only available on paid orders - compile-time guarantee!
    pub fn ship(self, tracking: String) -> Result<Order<Shipped>, Error> {
        Ok(Order {
            id: self.id,
            state: Shipped {
                shipped_at: Utc::now(),
                tracking_number: tracking,
            },
        })
    }
}
```

### Migration Steps

1. **Create state structs** with state-specific fields
2. **Convert machine to generic** with type parameter
3. **Move transition logic** to `impl Machine<State>` blocks
4. **Update database schema** to store state data
5. **Add conversion helpers** to/from database models
6. **Update service layer** to use type-safe transitions
7. **Remove runtime state checks** (compiler handles it!)

## Best Practices

### ✅ DO

- **Use type parameter** for state machine struct
- **Implement transitions** as methods on specific states
- **Include state-specific data** in state structs
- **Log all transitions** for audit trail
- **Write property-based tests** for state invariants
- **Document state diagram** in code comments
- **Use Result<T, E>** for fallible transitions

### ❌ DON'T

- **Don't use enum** for states (loses compile-time safety)
- **Don't check state at runtime** (use type system)
- **Don't add optional fields** to machine struct (use state-specific data)
- **Don't panic in transitions** (return Result)
- **Don't bypass state machine** (all changes go through transitions)
- **Don't forget audit logging** (track all state changes)

## Advanced Patterns

### Parallel States

```rust
// Order can be in both fulfillment and payment states
struct Order<F, P> {
    id: Uuid,
    fulfillment: F,
    payment: P,
}

impl Order<Pending, Unpaid> {
    fn pay(self) -> Order<Pending, Paid> { /* ... */ }
    fn ship(self) -> Order<Shipped, Unpaid> { /* ... */ }
}
```

### Hierarchical States

```rust
struct Active<S> {
    started_at: DateTime<Utc>,
    sub_state: S,
}

struct Processing;
struct Completed;

type OrderProcessing = Order<Active<Processing>>;
type OrderCompleted = Order<Active<Completed>>;
```

### Event Sourcing

```rust
pub enum OrderEvent {
    Created { customer_id: Uuid },
    Confirmed { inventory_reserved: bool },
    Paid { payment_id: String },
    Shipped { tracking: String },
}

impl Order<Pending> {
    fn apply(self, event: OrderEvent) -> Box<dyn Any> {
        match event {
            OrderEvent::Confirmed { inventory_reserved } => {
                Box::new(Order {
                    state: Confirmed { /* ... */ },
                    ..self
                })
            }
            // ...
        }
    }
}
```

## Performance Considerations

### Memory

- Type-safe states use **same memory** as enum + optional fields
- Generic monomorphization may increase binary size slightly
- State-specific data **reduces wasted memory** (no unused Options)

### CPU

- **Zero runtime cost** - all checks at compile time
- No pattern matching overhead
- Direct method calls (no vtable lookups)

### Benchmarks

```
Simple enum match:     ~2ns
Type-safe transition:  ~2ns (same!)
Invalid transition:    Compile error (no runtime cost!)
```

## Resources

### Implementation
- [Content State Machine](../crates/rustok-content/src/state_machine.rs)
- [Order State Machine](../crates/rustok-commerce/src/state_machine.rs)
- [State Machine Framework](../crates/rustok-core/src/state_machine/)

### External References
- [Type-State Pattern](https://cliffle.com/blog/rust-typestate/)
- [Session Types](https://blog.yoshuawuyts.com/session-types/)
- [Compile-Time State Machines](https://hoverbear.org/blog/rust-state-machine-pattern/)

---

**Status:** ✅ Production Ready  
**Version:** 1.0  
**Last Updated:** 2026-02-12
