# rustok-revisions-derive

Derive macros for [rustok-revisions](https://crates.io/crates/rustok-revisions).

[![Crates.io](https://img.shields.io/crates/v/rustok-revisions-derive.svg)](https://crates.io/crates/rustok-revisions-derive)
[![Documentation](https://docs.rs/rustok-revisions-derive/badge.svg)](https://docs.rs/rustok-revisions-derive)

## Overview

This crate provides the `#[derive(Revisionable)]` macro that automatically implements the `Revisionable` trait for your types, making it easy to track revisions of your content.

## Installation

This crate is typically used as a dependency of `rustok-revisions` with the `derive` feature enabled:

```toml
[dependencies]
rustok-revisions = { version = "0.1.0", features = ["derive"] }
```

Or you can use it directly:

```toml
[dependencies]
rustok-revisions-derive = "0.1.0"
```

## Usage

### Basic Example

```rust
use rustok_revisions_derive::Revisionable;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Revisionable)]
struct Post {
    title: String,
    content: String,
    author: String,
}
```

This generates:

```rust
impl rustok_revisions::Revisionable for Post {
    fn content_type() -> &'static str {
        "post"  // lowercase struct name
    }

    fn to_revision_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}
```

### Custom Content Type

Use the `#[revision(content_type = "...")]` attribute to specify a custom content type:

```rust
#[derive(Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct Post {
    title: String,
    content: String,
}
```

This generates:

```rust
impl rustok_revisions::Revisionable for Post {
    fn content_type() -> &'static str {
        "blog_post"  // custom content type
    }

    fn to_revision_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}
```

## Attributes

### `#[revision(content_type = "...")]`

Specifies the content type identifier for the revision system.

**Default**: Lowercase version of the struct name.

**Example**:
```rust
#[derive(Revisionable)]
#[revision(content_type = "product")]
struct Product {
    name: String,
    price: f64,
}
```

## Requirements

Your type must:

1. Implement `Clone`
2. Implement `Serialize` (from serde)
3. Implement `Deserialize` (from serde)

## Examples

### Blog Post

```rust
use rustok_revisions_derive::Revisionable;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct BlogPost {
    id: Uuid,
    title: String,
    content: String,
    author: String,
    published: bool,
    tags: Vec<String>,
}
```

### Product

```rust
#[derive(Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "product")]
struct Product {
    id: Uuid,
    name: String,
    description: String,
    price: f64,
    stock: i32,
    categories: Vec<String>,
}
```

### User Profile

```rust
#[derive(Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "user_profile")]
struct UserProfile {
    user_id: Uuid,
    username: String,
    email: String,
    bio: String,
    avatar_url: Option<String>,
}
```

## How It Works

The derive macro:

1. Parses the struct definition
2. Extracts the `content_type` attribute (or uses the struct name)
3. Generates an implementation of `Revisionable` that:
   - Returns the content type as a static string
   - Serializes the struct to JSON using serde

## Limitations

- Only works with structs (not enums or unions)
- Requires serde's `Serialize` and `Deserialize` traits
- Content type must be a string literal

## Troubleshooting

### Error: "the trait bound `T: Serialize` is not satisfied"

Make sure your type implements `Serialize` and `Deserialize`:

```rust
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Revisionable)]
struct MyType {
    // ...
}
```

### Error: "cannot find attribute `revision` in this scope"

Make sure you have the derive feature enabled:

```toml
[dependencies]
rustok-revisions = { version = "0.1.0", features = ["derive"] }
```

## See Also

- [rustok-revisions](https://crates.io/crates/rustok-revisions) - Main library
- [API Documentation](https://docs.rs/rustok-revisions-derive)
- [Examples](https://github.com/RusTokRs/RusTok/tree/main/rustok-revisions/examples)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
