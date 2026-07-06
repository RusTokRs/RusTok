# Error Handling Module

> **Status:** ✅ Production-ready (Sprint 2)  
> **Version:** 1.0.0  
> **Tests:** 12 unit tests

The error module provides standardized error handling with rich context and user-friendly messages.

## Concept

**Rich Error Context:**
- Structured metadata (fields, tenant_id, trace_id)
- Error chaining with context
- User-friendly messages
- RFC 7807 compatible API responses
- Automatic HTTP status mapping

## Components

### 1. RichError - Core Error Type

**File:** `mod.rs` (219 lines)

**Structure:**
```rust
pub struct RichError {
    pub kind: ErrorKind,
    pub message: String,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
    pub fields: HashMap<String, String>,
    pub tenant_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub trace_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}
```

**ErrorKind - 11 categories:**
```rust
pub enum ErrorKind {
    Validation,      // 400 Bad Request
    NotFound,        // 404 Not Found
    Unauthorized,    // 401 Unauthorized
    Forbidden,       // 403 Forbidden
    Conflict,        // 409 Conflict
    Internal,        // 500 Internal Server Error
    ServiceUnavailable, // 503 Service Unavailable
    Database,        // 500 Internal Server Error
    Network,         // 502 Bad Gateway
    Timeout,         // 504 Gateway Timeout
    External,        // 502 Bad Gateway
}
```

**Example:**
```rust
use rustok_core::error::{RichError, ErrorKind};

let error = RichError {
    kind: ErrorKind::NotFound,
    message: "User not found".to_string(),
    fields: [("user_id".to_string(), user_id.to_string())].into(),
    tenant_id: Some(tenant_id),
    trace_id: Some("trace-123".to_string()),
    ..Default::default()
};
```

### 2. ErrorContext - Trait for chaining

**File:** `context.rs` (283 lines)

**Trait:**
```rust
pub trait ErrorContext<T> {
    fn context(self, msg: impl Into<String>) -> Result<T, RichError>;
    fn with_context<F>(self, f: F) -> Result<T, RichError>
    where
        F: FnOnce() -> String;
    fn with_field(self, key: impl Into<String>, value: impl Into<String>) -> Result<T, RichError>;
    fn with_tenant(self, tenant_id: Uuid) -> Result<T, RichError>;
    fn with_user(self, user_id: Uuid) -> Result<T, RichError>;
    fn with_trace(self, trace_id: impl Into<String>) -> Result<T, RichError>;
}
```

**Example:**
```rust
use rustok_core::error::ErrorContext;

fn fetch_user(user_id: Uuid) -> Result<User, RichError> {
    let user = database
        .query_one("SELECT * FROM users WHERE id = $1", &[&user_id])
        .await
        .context("Failed to fetch user from database")?
        .with_field("user_id", user_id.to_string())?
        .with_tenant(tenant_id)?
        .with_trace(request_id)?;
    
    Ok(user)
}
```

**Automatic conversion:**
```rust
// From any std::error::Error
impl<T, E: std::error::Error + Send + Sync + 'static> ErrorContext<T> for Result<T, E>

// From Box<dyn Error>
impl<T> ErrorContext<T> for Result<T, Box<dyn std::error::Error + Send + Sync>>

// From String
impl<T> ErrorContext<T> for Result<T, String>
```

### 3. ErrorResponse - API Responses

**File:** `response.rs` (292 lines)

**RFC 7807 format:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub error_type: String,
    pub title: String,
    pub status: u16,
    pub detail: Option<String>,
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub fields: HashMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}
```

**Quick constructors:**
```rust
// 404 Not Found
ErrorResponse::not_found("User")

// 403 Forbidden
ErrorResponse::forbidden()

// 401 Unauthorized
ErrorResponse::unauthorized()

// 400 Validation
ErrorResponse::validation()
    .with_field_error("email", "invalid email format")
    .with_field_error("age", "must be greater than 0")

// 500 Internal
ErrorResponse::internal("Database connection failed")

// 503 Service Unavailable
ErrorResponse::service_unavailable()
```

**JSON Output:**
```json
{
  "type": "https://api.rustok.dev/errors/validation",
  "title": "Validation Failed",
  "status": 400,
  "detail": "One or more fields failed validation",
  "fields": {
    "email": ["invalid email format", "email already exists"],
    "age": ["must be greater than 0"]
  },
  "trace_id": "trace-abc-123",
  "timestamp": "2026-02-12T10:30:00Z"
}
```

### 4. ValidationErrorBuilder

**File:** `response.rs` (part)

**Builder pattern:**
```rust
use rustok_core::error::ValidationErrorBuilder;

let error = ValidationErrorBuilder::new()
    .field("email", "invalid email format")
    .field("email", "email already exists")
    .field("password", "must be at least 8 characters")
    .field("age", "must be greater than 0")
    .trace_id("trace-123")
    .build();
```

**Supports multiple errors per field:**
```rust
// Email has 2 errors
.field("email", "invalid email format")
.field("email", "email already exists")

// Result:
{
  "fields": {
    "email": [
      "invalid email format",
      "email already exists"
    ]
  }
}
```

## Module integration

### Content Module

**File:** `crates/rustok-content/src/error.rs` (130 lines)

```rust
use rustok_core::error::{ErrorContext, ErrorKind, RichError};

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("Content node not found: {0}")]
    NotFound(String),
    
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Unauthorized to access content")]
    Unauthorized,
    
    #[error("Forbidden to modify content")]
    Forbidden,
    
    #[error("Content already exists: {0}")]
    AlreadyExists(String),
    
    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

// Conversion to RichError
impl From<ContentError> for RichError {
    fn from(error: ContentError) -> Self {
        let kind = match &error {
            ContentError::NotFound(_) => ErrorKind::NotFound,
            ContentError::ValidationFailed(_) => ErrorKind::Validation,
            ContentError::Unauthorized => ErrorKind::Unauthorized,
            ContentError::Forbidden => ErrorKind::Forbidden,
            ContentError::AlreadyExists(_) => ErrorKind::Conflict,
            ContentError::InvalidStateTransition(_) => ErrorKind::Validation,
            ContentError::Database(_) => ErrorKind::Database,
            ContentError::Internal(_) => ErrorKind::Internal,
        };
        
        RichError::new(kind, error.to_string())
    }
}

// ErrorContext implementation
impl<T> ErrorContext<T> for Result<T, ContentError> {
    // Delegate to RichError
}
```

**Usage:**
```rust
use rustok_content::error::{ContentError, ContentResult};
use rustok_core::error::ErrorContext;

pub async fn get_node(id: Uuid, tenant_id: Uuid) -> ContentResult<Node> {
    let node = database
        .query_one("SELECT * FROM nodes WHERE id = $1 AND tenant_id = $2", &[&id, &tenant_id])
        .await
        .map_err(|e| ContentError::Database(e.to_string()))?
        .context("Failed to fetch node")?
        .with_field("node_id", id.to_string())?
        .with_tenant(tenant_id)?;
    
    Ok(node)
}
```

### Commerce Module

**File:** `crates/rustok-commerce/src/error.rs` (190 lines)

```rust
#[derive(Debug, thiserror::Error)]
pub enum CommerceError {
    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),
    
    #[error("Insufficient stock for product {product_id}: available={available}, requested={requested}")]
    InsufficientStock {
        product_id: Uuid,
        available: i32,
        requested: i32,
    },
    
    #[error("Payment failed: {0}")]
    PaymentFailed(String),
    
    #[error("Invalid price: {0}")]
    InvalidPrice(String),
    
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),
    
    #[error("Invalid order state: {0}")]
    InvalidOrderState(String),
    
    // ... + 5 other categories
}
```

**Structured errors:**
```rust
// With metadata in the error itself
CommerceError::InsufficientStock {
    product_id: product.id,
    available: product.stock,
    requested: quantity,
}

// Conversion to RichError automatically adds fields
impl From<CommerceError> for RichError {
    fn from(error: CommerceError) -> Self {
        match error {
            CommerceError::InsufficientStock { product_id, available, requested } => {
                RichError::new(ErrorKind::Validation, error.to_string())
                    .with_field("product_id", product_id.to_string())
                    .with_field("available", available.to_string())
                    .with_field("requested", requested.to_string())
            },
            // ...
        }
    }
}
```

## Axum Integration

**Automatic HTTP response:**
```rust
use axum::{
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
};
use rustok_core::error::{RichError, ErrorResponse};

impl IntoResponse for RichError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let response = ErrorResponse::from(self);
        
        (status, Json(response)).into_response()
    }
}
```

**In handler:**
```rust
async fn get_user(
    Path(user_id): Path<Uuid>,
    Extension(tenant_id): Extension<Uuid>,
) -> Result<Json<User>, RichError> {
    let user = user_service
        .get(user_id)
        .await
        .context("Failed to get user")?
        .with_field("user_id", user_id.to_string())?
        .with_tenant(tenant_id)?;
    
    Ok(Json(user))
}

// Automatically returns JSON error response with the correct status code
```

## Tests

**Total:** 12 unit tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_rich_error_creation() { /* ... */ }
    
    #[test]
    fn test_error_kind_to_status_code() { /* ... */ }
    
    #[test]
    fn test_error_context_chaining() { /* ... */ }
    
    #[test]
    fn test_error_with_fields() { /* ... */ }
    
    #[test]
    fn test_error_response_json() { /* ... */ }
    
    #[test]
    fn test_validation_error_builder() { /* ... */ }
    
    #[test]
    fn test_validation_multiple_fields() { /* ... */ }
    
    #[test]
    fn test_error_response_not_found() { /* ... */ }
    
    #[test]
    fn test_error_response_forbidden() { /* ... */ }
    
    #[test]
    fn test_content_error_conversion() { /* ... */ }
    
    #[test]
    fn test_commerce_error_conversion() { /* ... */ }
    
    #[test]
    fn test_error_trace_propagation() { /* ... */ }
}
```

## Best Practices

### 1. Use ErrorContext to add context

```rust
// ✅ Good
database.query(&sql)
    .await
    .context("Failed to execute query")?
    .with_field("table", "users")?
    .with_tenant(tenant_id)?

// ❌ Bad - losing context
database.query(&sql).await?
```

### 2. Add relevant fields

```rust
// ✅ Good - specific fields
.with_field("user_id", user_id.to_string())
.with_field("email", email)
.with_field("operation", "password_reset")

// ❌ Bad - generic fields
.with_field("data", format!("{:?}", data))
```

### 3. Use ValidationErrorBuilder for multiple errors

```rust
// ✅ Good
let mut errors = ValidationErrorBuilder::new();

if email.is_empty() {
    errors = errors.field("email", "required");
}
if !email.contains('@') {
    errors = errors.field("email", "invalid format");
}
if age < 18 {
    errors = errors.field("age", "must be at least 18");
}

if errors.has_errors() {
    return Err(errors.build());
}

// ❌ Bad - stopping at the first error
if email.is_empty() {
    return Err(RichError::validation("Email required"));
}
```

### 4. Categorize errors correctly

```rust
// ✅ Good
match error {
    sqlx::Error::RowNotFound => ContentError::NotFound(id),
    sqlx::Error::Database(db_err) if is_unique_violation(&db_err) =>
        ContentError::AlreadyExists(id),
    _ => ContentError::Database(error.to_string()),
}

// ❌ Bad - all in Internal
sqlx::Error => ContentError::Internal(error.to_string())
```

## HTTP Status Code Mapping

| ErrorKind | Status Code | Description |
|-----------|-------------|-------------|
| Validation | 400 | Bad Request |
| Unauthorized | 401 | Unauthorized |
| Forbidden | 403 | Forbidden |
| NotFound | 404 | Not Found |
| Conflict | 409 | Conflict |
| Internal | 500 | Internal Server Error |
| Database | 500 | Internal Server Error |
| ServiceUnavailable | 503 | Service Unavailable |
| Timeout | 504 | Gateway Timeout |
| Network | 502 | Bad Gateway |
| External | 502 | Bad Gateway |

## Documentation

Full guide: [docs/ERROR_HANDLING_GUIDE.md](../../../../docs/ERROR_HANDLING_GUIDE.md)

**Sections:**
1. Concepts and architecture
2. RichError API
3. ErrorContext trait
4. ErrorResponse and RFC 7807
5. ValidationErrorBuilder
6. Module integration
7. Axum integration
8. Best practices
9. Testing strategies

## Roadmap

**v1.0.0 (Sprint 2):** ✅ DONE
- RichError with 11 categories
- ErrorContext trait
- ErrorResponse RFC 7807
- ValidationErrorBuilder
- Content/Commerce integration
- 12 comprehensive tests

**v1.1.0 (Future):**
- [ ] Error monitoring integration (Sentry)
- [ ] Structured logging
- [ ] Error analytics
- [ ] More helper constructors

**v2.0.0 (Future):**
- [ ] I18n error messages
- [ ] Error recovery strategies
- [ ] Custom error codes
- [ ] GraphQL error integration

## References

- [RFC 7807: Problem Details](https://tools.ietf.org/html/rfc7807)
- [Rust Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [thiserror crate](https://docs.rs/thiserror/)
- [anyhow vs thiserror](https://nick.groenen.me/posts/rust-error-handling/)
