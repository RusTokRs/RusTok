---
id: doc://docs/guides/input-validation.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Input Validation in RusToK

## Overview

RusToK uses the `validator` crate to provide declarative, type-safe input validation for all API inputs. Validation rules are defined directly on DTO structs using attributes, ensuring data integrity at the API boundary.

## Features

- ✅ Declarative validation with derive macros
- ✅ Custom validators for business rules
- ✅ Clear, structured error messages
- ✅ Compile-time safety
- ✅ Automatic validation in services
- ✅ Consistent across REST and GraphQL APIs

## Basic Usage

### Defining Validated DTOs

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateNodeInput {
    #[validate(length(min = 1, max = 64))]
    pub kind: String,
    
    #[validate(length(min = 1))]
    #[validate]  // Nested validation
    pub translations: Vec<NodeTranslationInput>,
    
    #[validate]  // Nested validation
    pub bodies: Vec<BodyInput>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct NodeTranslationInput {
    #[validate(length(min = 2, max = 10))]
    pub locale: String,
    
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
    
    #[validate(length(max = 1000))]
    pub excerpt: Option<String>,
}
```

### Validating Input

```rust
use validator::Validate;
use crate::dto::CreateNodeInput;

pub async fn create_node(input: CreateNodeInput) -> Result<NodeResponse> {
    // Validate input
    input.validate()
        .map_err(|e| ContentError::Validation(format!("Invalid input: {}", e)))?;
    
    // Process valid input
    // ...
}
```

## Built-in Validators

### Length Validation

```rust
#[derive(Validate)]
pub struct Example {
    #[validate(length(min = 1, max = 100))]
    pub title: String,
    
    #[validate(length(min = 1))]  // At least one
    pub tags: Vec<String>,
    
    #[validate(length(equal = 10))]  // Exactly 10
    pub code: String,
}
```

### Range Validation

```rust
#[derive(Validate)]
pub struct Example {
    #[validate(range(min = 0, max = 100))]
    pub position: i32,
    
    #[validate(range(min = 0.0, max = 1.0))]
    pub percentage: f64,
}
```

### Email Validation

```rust
#[derive(Validate)]
pub struct UserInput {
    #[validate(email)]
    pub email: String,
}
```

### URL Validation

```rust
#[derive(Validate)]
pub struct LinkInput {
    #[validate(url)]
    pub homepage: String,
}
```

### Custom Message

```rust
#[derive(Validate)]
pub struct Example {
    #[validate(
        length(min = 8, max = 128),
        message = "Password must be between 8 and 128 characters"
    )]
    pub password: String,
}
```

## Custom Validators

### Defining Custom Validators

```rust
use validator::ValidationError;

/// Validate body format
pub fn validate_body_format(format: &str) -> Result<(), ValidationError> {
    match format {
        "markdown" | "html" | "plain" | "json" => Ok(()),
        _ => Err(ValidationError::new("invalid_format")),
    }
}

/// Validate locale format (e.g., "en", "en-US", "es-419")
pub fn validate_locale(locale: &str) -> Result<(), ValidationError> {
    if locale.len() < 2 || locale.len() > 10 {
        return Err(ValidationError::new("invalid_locale_length"));
    }

    let parts: Vec<&str> = locale.split('-').collect();
    match parts.len() {
        1 => {
            if parts[0].len() != 2 || !parts[0].chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(ValidationError::new("invalid_locale_format"));
            }
        }
        2 => {
            if parts[0].len() != 2 || !parts[0].chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(ValidationError::new("invalid_locale_format"));
            }

            let region = parts[1];
            let is_alpha_region = region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic());
            let is_numeric_region = region.len() == 3 && region.chars().all(|c| c.is_ascii_digit());

            if !(is_alpha_region || is_numeric_region) {
                return Err(ValidationError::new("invalid_locale_format"));
            }
        }
        _ => return Err(ValidationError::new("invalid_locale_format")),
    }

    Ok(())
}

/// Validate slug format
pub fn validate_slug(slug: &str) -> Result<(), ValidationError> {
    if slug.is_empty() || slug.len() > 255 {
        return Err(ValidationError::new("invalid_slug_length"));
    }
    
    // Lowercase letters, numbers, hyphens only
    if !slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(ValidationError::new("slug_invalid_characters"));
    }
    
    // Must not start or end with hyphen
    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(ValidationError::new("slug_hyphen_boundary"));
    }
    
    Ok(())
}
```

### Using Custom Validators

```rust
use validator::Validate;
use super::validation::{validate_body_format, validate_locale, validate_slug};

#[derive(Validate)]
pub struct BodyInput {
    #[validate(custom(function = "validate_locale"))]
    pub locale: String,
    
    #[validate(custom(function = "validate_body_format"))]
    pub format: Option<String>,
}

#[derive(Validate)]
pub struct NodeTranslationInput {
    #[validate(custom(function = "validate_locale"))]
    pub locale: String,
    
    #[validate(custom(function = "validate_slug"))]
    pub slug: Option<String>,
}
```

## Content Module Validation

### Node Creation

```rust
#[derive(Validate)]
pub struct CreateNodeInput {
    // Kind must be one of: post, page, article, custom
    #[validate(length(min = 1, max = 64))]
    #[validate(custom(function = "validate_kind"))]
    pub kind: String,
    
    // At least one translation required
    #[validate(length(min = 1))]
    #[validate]
    pub translations: Vec<NodeTranslationInput>,
    
    // Position must be non-negative and reasonable
    #[validate(custom(function = "validate_position"))]
    pub position: Option<i32>,
    
    // Depth must be 0-100
    #[validate(custom(function = "validate_depth"))]
    pub depth: Option<i32>,
    
    // Reply count must be non-negative
    #[validate(custom(function = "validate_reply_count"))]
    pub reply_count: Option<i32>,
}
```

### Translation Validation

```rust
#[derive(Validate)]
pub struct NodeTranslationInput {
    // Locale: en, en-US, ru-RU, etc.
    #[validate(custom(function = "validate_locale"))]
    pub locale: String,
    
    // Title: 1-255 characters
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
    
    // Slug: lowercase-with-hyphens
    #[validate(custom(function = "validate_slug"))]
    pub slug: Option<String>,
    
    // Excerpt: max 1000 characters
    #[validate(length(max = 1000))]
    pub excerpt: Option<String>,
}
```

### Body Validation

```rust
#[derive(Validate)]
pub struct BodyInput {
    #[validate(custom(function = "validate_locale"))]
    pub locale: String,
    
    // Body: max 1MB
    #[validate(length(max = 1_000_000))]
    pub body: Option<String>,
    
    // Format: markdown, html, plain, json
    #[validate(custom(function = "validate_body_format"))]
    pub format: Option<String>,
}
```

## Error Handling

### Validation Errors

```rust
use validator::Validate;

match input.validate() {
    Ok(_) => {
        // Input is valid
    }
    Err(validation_errors) => {
        // Extract error messages
        for (field, errors) in validation_errors.field_errors() {
            for error in errors {
                eprintln!("Field '{}': {:?}", field, error.code);
                if let Some(message) = &error.message {
                    eprintln!("  Message: {}", message);
                }
            }
        }
    }
}
```

### Converting to API Errors

```rust
use validator::Validate;
use crate::error::ContentError;

pub async fn create_node(input: CreateNodeInput) -> Result<NodeResponse, ContentError> {
    // Validate and convert errors
    input.validate()
        .map_err(|e| ContentError::Validation(format_validation_errors(&e)))?;
    
    // ...
}

fn format_validation_errors(errors: &validator::ValidationErrors) -> String {
    let mut messages = Vec::new();
    
    for (field, field_errors) in errors.field_errors() {
        for error in field_errors {
            let message = error.message
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| format!("{}: {}", field, error.code));
            messages.push(message);
        }
    }
    
    messages.join("; ")
}
```

### GraphQL Error Handling

```rust
use async_graphql::{Error, ErrorExtensions};

pub async fn create_node(ctx: &Context<'_>, input: CreateNodeInput) -> Result<NodeResponse> {
    input.validate()
        .map_err(|e| {
            Error::new("Validation failed")
                .extend_with(|_, ext| {
                    ext.set("validation_errors", format_validation_errors(&e));
                })
        })?;
    
    // ...
}
```

## Testing Validation

### Unit Tests

```rust
use crate::dto::validation::*;

#[test]
fn test_validate_body_format_valid() {
    assert!(validate_body_format("markdown").is_ok());
    assert!(validate_body_format("html").is_ok());
    assert!(validate_body_format("plain").is_ok());
    assert!(validate_body_format("json").is_ok());
}

#[test]
fn test_validate_body_format_invalid() {
    assert!(validate_body_format("xml").is_err());
    assert!(validate_body_format("unknown").is_err());
}

#[test]
fn test_validate_slug_valid() {
    assert!(validate_slug("my-post").is_ok());
    assert!(validate_slug("hello-world-123").is_ok());
}

#[test]
fn test_validate_slug_invalid() {
    assert!(validate_slug("My-Post").is_err()); // Uppercase
    assert!(validate_slug("my_post").is_err()); // Underscore
    assert!(validate_slug("-mypost").is_err()); // Starts with hyphen
    assert!(validate_slug("mypost-").is_err()); // Ends with hyphen
}
```

### Integration Tests

```rust
use crate::dto::CreateNodeInput;
use validator::Validate;

#[test]
fn test_create_node_input_validation() {
    // Valid input
    let valid_input = CreateNodeInput {
        kind: "post".to_string(),
        translations: vec![/* valid translation */],
        // ...
    };
    assert!(valid_input.validate().is_ok());
    
    // Invalid: empty kind
    let invalid_input = CreateNodeInput {
        kind: "".to_string(),
        // ...
    };
    assert!(invalid_input.validate().is_err());
    
    // Invalid: no translations
    let invalid_input = CreateNodeInput {
        kind: "post".to_string(),
        translations: vec![],
        // ...
    };
    assert!(invalid_input.validate().is_err());
}
```

## Best Practices

### 1. Validate Early

Always validate input at the API boundary (controller/resolver level):

```rust
pub async fn create_node_handler(
    Json(input): Json<CreateNodeInput>
) -> Result<Json<NodeResponse>> {
    // Validate immediately
    input.validate()
        .map_err(|e| ApiError::BadRequest(format_validation_errors(&e)))?;
    
    // Pass to service
    let result = service.create_node(input).await?;
    Ok(Json(result))
}
```

### 2. Use Custom Validators for Business Rules

Don't put business logic in services - encode it in validators:

```rust
#[validate(custom(function = "validate_product_price"))]
pub price: Decimal,

fn validate_product_price(price: &Decimal) -> Result<(), ValidationError> {
    if *price < Decimal::ZERO {
        return Err(ValidationError::new("price_negative"));
    }
    if *price > Decimal::from(1_000_000) {
        return Err(ValidationError::new("price_too_high"));
    }
    Ok(())
}
```

### 3. Provide Clear Error Messages

Use custom messages for better UX:

```rust
#[validate(
    length(min = 8, max = 128),
    message = "Password must be between 8 and 128 characters"
)]
pub password: String,

#[validate(
    custom(function = "validate_slug"),
    message = "Slug must be lowercase with hyphens, e.g., 'my-post-title'"
)]
pub slug: String,
```

### 4. Validate Nested Structures

Use `#[validate]` on fields containing other validated structs:

```rust
#[derive(Validate)]
pub struct CreateNodeInput {
    #[validate]  // Validates each translation
    pub translations: Vec<NodeTranslationInput>,
}
```

### 5. Test Validators Thoroughly

Write unit tests for all custom validators:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_all_valid_cases() { /* ... */ }
    
    #[test]
    fn test_all_invalid_cases() { /* ... */ }
    
    #[test]
    fn test_edge_cases() { /* ... */ }
}
```

## Common Validators

### Content Module

- `validate_kind`: post, page, article, custom
- `validate_body_format`: markdown, html, plain, json
- `validate_locale`: en, en-US, ru-RU, es-419, etc.
- `validate_slug`: lowercase-with-hyphens
- `validate_position`: 0-100,000
- `validate_depth`: 0-100
- `validate_reply_count`: >= 0

### Commerce Module (Future)

- `validate_sku`: alphanumeric SKU codes
- `validate_price`: non-negative, reasonable max
- `validate_quantity`: >= 0
- `validate_currency`: ISO currency codes

### User Module (Future)

- `validate_username`: alphanumeric + underscores
- `validate_email`: standard email format
- `validate_password`: strength requirements

## Performance Considerations

### Validation Cost

- Most validators: O(1) or O(n) where n = string length
- Negligible overhead (<1ms for typical inputs)
- Run once at API boundary, not in every service method

### Optimization Tips

1. **Cache compiled regexes** in custom validators
2. **Short-circuit** complex validators for obvious failures
3. **Don't validate twice** - do it once at the boundary

```rust
// Good: validate once
pub async fn create_node(input: CreateNodeInput) -> Result<NodeResponse> {
    input.validate()?;  // Once here
    self.service.create(input).await  // Service assumes valid
}

// Bad: validate multiple times
pub async fn create_node(input: CreateNodeInput) -> Result<NodeResponse> {
    input.validate()?;
    self.service.create(input).await  // Service validates again ❌
}
```

## Migration Guide

### Adding Validation to Existing DTOs

1. Add `validator` dependency
2. Add `#[derive(Validate)]` to struct
3. Add validation attributes to fields
4. Add `.validate()` call in handlers

```rust
// Before
#[derive(Deserialize)]
pub struct CreateNodeInput {
    pub kind: String,
}

// After
#[derive(Deserialize, Validate)]
pub struct CreateNodeInput {
    #[validate(length(min = 1, max = 64))]
    pub kind: String,
}

// In handler
input.validate()
    .map_err(|e| ApiError::BadRequest(format_validation_errors(&e)))?;
```

## Future Enhancements

- [ ] Async validators (e.g., check if slug is unique)
- [ ] Conditional validation (validate field X if field Y is set)
- [ ] Cross-field validation (field A must be greater than field B)
- [ ] Localized error messages
- [ ] Auto-generate OpenAPI validation schemas

---

**See Also:**
- [Error Handling](./error-handling.md)
- [API Documentation](./api.md)
- [Testing Guide](./testing.md)
