---
id: doc://docs/standards/errors.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Error Handling Guide

## Overview

Standardized error handling framework providing rich context, user-friendly messages, and consistent API responses.

## Features

### 🎯 Rich Error Context
- Error chains (cause tracking)
- Structured metadata (key-value pairs)
- Request tracing IDs
- User-facing vs internal messages

### 📊 Standardized Responses
- Consistent JSON structure
- HTTP status code mapping
- Field validation errors
- RFC 7807 Problem Details compatible

### 🏷️ Error Categories
- **Validation** (400) - Input validation failures
- **Unauthenticated** (401) - Authentication required
- **Forbidden** (403) - Permission denied
- **Not Found** (404) - Resource not found
- **Conflict** (409) - Duplicate/race condition
- **Rate Limited** (429) - Too many requests
- **Database** (500) - Database errors
- **External Service** (502/503) - Third-party failures
- **Internal** (500) - Unexpected errors
- **Timeout** (504) - Request timeout

## Quick Start

### Basic Error with Context

```rust
use rustok_core::error::{RichError, ErrorKind, ErrorContext};
use uuid::Uuid;

fn process_order(order_id: Uuid) -> Result<Order, RichError> {
    let order = fetch_order(order_id)
        .context("Failed to fetch order")
        .with_field("order_id", order_id.to_string())?;
    
    Ok(order)
}
```

### Validation Errors

```rust
use rustok_core::error::{ValidationErrorBuilder, ErrorResponse};

fn validate_user_input(input: &CreateUserInput) -> Result<(), ErrorResponse> {
    let mut builder = ValidationErrorBuilder::new();
    let mut has_errors = false;
    
    if input.email.is_empty() {
        builder = builder.field("email", "required");
        has_errors = true;
    }
    
    if input.age < 18 {
        builder = builder.field("age", "must be at least 18");
        has_errors = true;
    }
    
    if has_errors {
        return Err(builder.build());
    }
    
    Ok(())
}
```

### User-Friendly Errors

```rust
use rustok_core::error::{RichError, ErrorKind};

fn delete_user(user_id: Uuid) -> Result<(), RichError> {
    let user = find_user(user_id)
        .ok_or_else(|| {
            RichError::new(
                ErrorKind::NotFound,
                format!("User record {} not found in database", user_id),
            )
            .with_user_message("The requested user does not exist")
            .with_field("user_id", user_id.to_string())
            .with_error_code("USER_NOT_FOUND")
        })?;
    
    // Delete user...
    Ok(())
}
```

### API Error Responses

```rust
use rustok_core::error::ErrorResponse;

// Quick responses
let response = ErrorResponse::not_found("User");
let response = ErrorResponse::forbidden();
let response = ErrorResponse::validation()
    .with_field_error("email", "required")
    .with_field_error("password", "too short");

// Custom responses
let response = ErrorResponse::new(400, "CUSTOM_ERROR", "Something went wrong")
    .with_detail("Additional context here")
    .with_trace_id(trace_id)
    .with_metadata("resource", "order");
```

## Detailed Usage

### Error Context Trait

The `ErrorContext` trait adds context to any error type:

```rust
use rustok_core::error::ErrorContext;

// Simple context
let result = some_operation()
    .context("Failed to complete operation")?;

// Context with fields
let result = some_operation()
    .with_context(|| {
        (
            "Failed to process payment".to_string(),
            vec![
                ("order_id".to_string(), order_id.to_string()),
                ("amount".to_string(), amount.to_string()),
            ],
        )
    })?;
```

### RichError Construction

```rust
use rustok_core::error::{RichError, ErrorKind};

// Basic error
let error = RichError::new(ErrorKind::NotFound, "User not found");

// With full context
let error = RichError::new(ErrorKind::NotFound, "User not found")
    .with_user_message("The requested user does not exist")
    .with_error_code("USER_NOT_FOUND")
    .with_field("user_id", user_id.to_string())
    .with_field("requested_by", requester_id.to_string())
    .with_trace_id(trace_id)
    .with_status_code(404);
```

### Error Response for APIs

```rust
use rustok_core::error::ErrorResponse;
use axum::{response::IntoResponse, Json};

// Convert RichError to HTTP response
impl IntoResponse for RichError {
    fn into_response(self) -> axum::response::Response {
        let status = axum::http::StatusCode::from_u16(self.status_code)
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        
        let response: ErrorResponse = self.into();
        
        (status, Json(response)).into_response()
    }
}
```

## Module-Specific Errors

### Content Module

```rust
use rustok_content::error::{ContentError, ContentResult};
use rustok_core::error::RichError;

// Create typed errors
let err = ContentError::node_not_found(node_id);
let err = ContentError::translation_not_found(node_id, "en");
let err = ContentError::validation("Invalid node type");

// Convert to RichError for API response
let rich: RichError = err.into();
```

### Commerce Module

```rust
use rustok_commerce::error::{CommerceError, CommerceResult};
use rustok_core::error::RichError;

// Create typed errors
let err = CommerceError::product_not_found(product_id);
let err = CommerceError::duplicate_sku("SKU-123");
let err = CommerceError::insufficient_inventory(10, 5);

// Convert to RichError for API response
let rich: RichError = err.into();
```

## Error Response Format

### Success Response

```json
{
  "data": {
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "name": "Example"
  }
}
```

### Error Response (Simple)

```json
{
  "status": 404,
  "code": "NOT_FOUND",
  "message": "User not found"
}
```

### Error Response (Detailed)

```json
{
  "status": 404,
  "code": "USER_NOT_FOUND",
  "message": "The requested user does not exist",
  "detail": "User record 123 not found in database",
  "trace_id": "550e8400-e29b-41d4-a716-446655440000",
  "metadata": {
    "user_id": "123",
    "requested_by": "456"
  }
}
```

### Validation Error Response

```json
{
  "status": 400,
  "code": "VALIDATION_ERROR",
  "message": "Validation failed",
  "fields": {
    "email": [
      "invalid email format",
      "email already exists"
    ],
    "age": [
      "must be greater than 0"
    ]
  },
  "trace_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

## Best Practices

### ✅ DO

**Use ErrorContext for external errors**
```rust
let data = fetch_data()
    .context("Failed to fetch data")?;
```

**Include structured metadata**
```rust
RichError::new(ErrorKind::NotFound, "User not found")
    .with_field("user_id", id.to_string())
    .with_field("searched_by", searcher_id.to_string())
```

**Set user-friendly messages**
```rust
RichError::new(ErrorKind::Database, "FK constraint violation")
    .with_user_message("Unable to delete: resource is in use")
```

**Use appropriate ErrorKind**
```rust
// Validation error
RichError::new(ErrorKind::Validation, "Invalid email")

// Business logic error
RichError::new(ErrorKind::BusinessLogic, "Insufficient funds")
```

**Include trace IDs**
```rust
error.with_trace_id(request_trace_id)
```

**Log internal details, return user-friendly messages**
```rust
tracing::error!(
    error = ?internal_error,
    user_id = %user_id,
    "Database connection failed"
);

return Err(RichError::new(ErrorKind::Database, "Database connection failed")
    .with_user_message("Unable to process request. Please try again later."));
```

### ❌ DON'T

**Don't expose internal details**
```rust
// ❌ Bad
"SQL error: foreign key constraint on table users_orders"

// ✅ Good
"Unable to delete user: user has active orders"
```

**Don't use generic errors**
```rust
// ❌ Bad
"Internal Server Error"

// ✅ Good
RichError::new(ErrorKind::Database, "Failed to save order")
    .with_user_message("Unable to complete purchase")
```

**Don't lose context**
```rust
// ❌ Bad
some_operation().map_err(|_| Error::Internal)?;

// ✅ Good
some_operation()
    .context("Failed to process payment")
    .with_field("order_id", order_id.to_string())?;
```

**Don't mix error types**
```rust
// ❌ Bad
RichError::new(ErrorKind::Database, "User not found") // Wrong kind!

// ✅ Good
RichError::new(ErrorKind::NotFound, "User not found")
```

## Migration Guide

### Before (Old Style)

```rust
#[derive(Debug, Error)]
pub enum MyError {
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Database error: {0}")]
    Database(#[from] DbErr),
    
    #[error("Validation error: {0}")]
    Validation(String),
}

pub fn my_function() -> Result<Data, MyError> {
    let data = fetch_data()
        .map_err(|e| MyError::Database(e))?;
    
    if !validate(&data) {
        return Err(MyError::Validation("Invalid data".into()));
    }
    
    Ok(data)
}
```

### After (Rich Errors)

```rust
use rustok_core::error::{RichError, ErrorKind, ErrorContext};

pub fn my_function() -> Result<Data, RichError> {
    let data = fetch_data()
        .context("Failed to fetch data")
        .with_field("resource", "user_data")?;
    
    if !validate(&data) {
        return Err(
            RichError::new(ErrorKind::Validation, "Data validation failed")
                .with_user_message("Invalid input data")
                .with_field("validation_rule", "email_format")
        );
    }
    
    Ok(data)
}
```

### Axum Handler Example

```rust
use axum::{extract::Path, response::IntoResponse, Json};
use rustok_core::error::{ErrorResponse, RichError};
use uuid::Uuid;

async fn get_user(
    Path(user_id): Path<Uuid>,
) -> Result<Json<User>, ErrorResponse> {
    let user = fetch_user(user_id)
        .await
        .context("Failed to fetch user")
        .with_field("user_id", user_id.to_string())
        .map_err(|e: RichError| ErrorResponse::from(e))?;
    
    Ok(Json(user))
}
```

## Error Logging

### Log Levels by Error Kind

```rust
match error.kind {
    ErrorKind::Validation | ErrorKind::NotFound => {
        tracing::info!(error = %error, "Client error");
    }
    ErrorKind::Database | ErrorKind::Internal => {
        tracing::error!(error = ?error, "Server error");
    }
    ErrorKind::ExternalService => {
        tracing::warn!(error = %error, "External service error");
    }
    _ => {
        tracing::warn!(error = %error, "Error occurred");
    }
}
```

### Structured Logging

```rust
tracing::error!(
    error_kind = ?error.kind,
    error_code = ?error.error_code,
    trace_id = ?error.trace_id,
    user_id = %user_id,
    "Failed to process order"
);
```

## Testing

### Testing Error Creation

```rust
#[test]
fn test_user_not_found_error() {
    let user_id = Uuid::new_v4();
    let error = RichError::new(ErrorKind::NotFound, "User not found")
        .with_user_message("User does not exist")
        .with_field("user_id", user_id.to_string());
    
    assert_eq!(error.kind, ErrorKind::NotFound);
    assert_eq!(error.status_code, 404);
    assert_eq!(error.user_message, Some("User does not exist".to_string()));
    assert_eq!(error.fields.get("user_id"), Some(&user_id.to_string()));
}
```

### Testing Error Conversion

```rust
#[test]
fn test_content_error_to_rich_error() {
    let id = Uuid::new_v4();
    let err = ContentError::node_not_found(id);
    let rich: RichError = err.into();
    
    assert_eq!(rich.kind, ErrorKind::NotFound);
    assert_eq!(rich.status_code, 404);
    assert!(rich.fields.contains_key("node_id"));
}
```

### Testing Validation Errors

```rust
#[test]
fn test_validation_error_builder() {
    let error = ValidationErrorBuilder::new()
        .field("email", "required")
        .field("email", "invalid format")
        .field("age", "must be >= 18")
        .build();
    
    assert_eq!(error.status, 400);
    assert_eq!(error.code, "VALIDATION_ERROR");
    
    let fields = error.fields.unwrap();
    assert_eq!(fields.get("email").unwrap().len(), 2);
    assert_eq!(fields.get("age").unwrap().len(), 1);
}
```

## Performance

### Benchmarks

```
Error creation:          ~100ns
Context addition:        ~50ns per field
JSON serialization:      ~1-2μs
Total API error:         ~2-3μs

Zero cost when Result<T, E> is Ok
```

### Memory

```
RichError size:          ~200 bytes
ErrorResponse size:      ~150 bytes
Stack allocated:         Yes (most cases)
```

## Examples

### Complete Example: Order Processing

```rust
use rustok_core::error::{RichError, ErrorKind, ErrorContext};
use uuid::Uuid;

pub async fn process_order(
    order_id: Uuid,
    user_id: Uuid,
) -> Result<Order, RichError> {
    // Fetch order with context
    let order = fetch_order(order_id)
        .await
        .context("Failed to fetch order")
        .with_field("order_id", order_id.to_string())
        .with_field("user_id", user_id.to_string())?;
    
    // Validate ownership
    if order.user_id != user_id {
        return Err(
            RichError::new(ErrorKind::Forbidden, "Order belongs to different user")
                .with_user_message("You do not have permission to access this order")
                .with_field("order_id", order_id.to_string())
                .with_field("order_owner", order.user_id.to_string())
                .with_field("requested_by", user_id.to_string())
        );
    }
    
    // Process payment
    let payment = process_payment(&order)
        .await
        .context("Failed to process payment")
        .with_field("order_id", order_id.to_string())
        .with_field("amount", order.total.to_string())?;
    
    // Update order
    let updated_order = update_order_status(order_id, OrderStatus::Paid)
        .await
        .context("Failed to update order status")
        .with_field("order_id", order_id.to_string())?;
    
    Ok(updated_order)
}
```

## Resources

### Implementation
- [Error Context](../crates/rustok-core/src/error/context.rs)
- [Error Response](../crates/rustok-core/src/error/response.rs)
- [Content Errors](../crates/rustok-content/src/error.rs)
- [Commerce Errors](../crates/rustok-commerce/src/error.rs)

### External References
- [RFC 7807 - Problem Details for HTTP APIs](https://tools.ietf.org/html/rfc7807)
- [Rust Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [thiserror crate](https://docs.rs/thiserror)
- [anyhow crate](https://docs.rs/anyhow)

---

**Status:** ✅ Production Ready  
**Version:** 1.0  
**Last Updated:** 2026-02-12
