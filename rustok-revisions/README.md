# rustok-revisions

A powerful and flexible library for tracking content revisions in Rust applications.

[![Crates.io](https://img.shields.io/crates/v/rustok-revisions.svg)](https://crates.io/crates/rustok-revisions)
[![Documentation](https://docs.rs/rustok-revisions/badge.svg)](https://docs.rs/rustok-revisions)
[![License](https://img.shields.io/crates/l/rustok-revisions.svg)](LICENSE)

## Features

- 🔄 **Revision Tracking**: Automatically track changes to your content
- 🗄️ **Multiple Backends**: Support for PostgreSQL via SeaORM (extensible to others)
- 🔍 **Diff Computation**: Compare revisions and see exactly what changed
- 🏷️ **Named Versions**: Create named snapshots of important revisions
- 🗑️ **Retention Policies**: Automatically clean up old revisions
- ⏪ **Restore**: Restore content to any previous revision
- 🌍 **Multi-tenant**: Built-in support for multi-tenant applications
- 🌐 **Multilingual**: Track revisions per locale
- 🎯 **Type-safe**: Full type safety with Rust's type system
- ⚡ **Async/Await**: Built on tokio for high performance

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rustok-revisions = { version = "0.1.0", features = ["derive", "seaorm"] }
sea-orm = { version = "2.0", features = ["runtime-tokio-rustls", "sqlx-postgres"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

## Quick Start

### 1. Define Your Content Type

```rust
use rustok_revisions::Revisionable;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct Post {
    id: uuid::Uuid,
    title: String,
    content: String,
    author: String,
}
```

### 2. Set Up the Database

```bash
# Install sea-orm-cli
cargo install sea-orm-cli

# Run migrations
sea-orm-cli migrate up -d migrations
```

### 3. Create and Track Revisions

```rust
use rustok_revisions::{
    RevisionService, SeaOrmBackend, RevisionTracker, 
    ChangeSource, RevisionEvent
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to database
    let db = sea_orm::Database::connect("postgres://localhost/mydb").await?;
    
    // Create service
    let backend = SeaOrmBackend::new(db);
    let service = RevisionService::new(Box::new(backend));
    
    // Create a post
    let post = Post {
        id: uuid::Uuid::new_v4(),
        title: "My First Post".to_string(),
        content: "Hello, world!".to_string(),
        author: "Alice".to_string(),
    };
    
    // Track creation
    let revision = service.create_revision_for_create(
        tenant_id,
        post.id,
        "en",
        &post,
        user_id,
        ChangeSource::Web,
    ).await?;
    
    // Update the post
    let mut updated_post = post.clone();
    updated_post.title = "My Updated Post".to_string();
    
    let tracker = RevisionTracker::builder()
        .enabled(true)
        .source(ChangeSource::Web)
        .build();
    
    service.create_revision_with_tracker(
        tenant_id,
        post.id,
        "en",
        &post,
        &updated_post,
        user_id,
        &tracker,
        RevisionEvent::Update,
    ).await?;
    
    Ok(())
}
```

## Examples

### Comparing Revisions

```rust
// Get diff between two revisions
let diff = service.compare_revisions(old_revision_id, new_revision_id).await?;

println!("Added: {:?}", diff.added);
println!("Removed: {:?}", diff.removed);
println!("Changed: {:?}", diff.changed);
```

### Restoring to a Previous Revision

```rust
// Restore content to a previous state
let (restored_post, new_revision) = service.restore_revision::<Post>(
    tenant_id,
    post_id,
    "en",
    target_revision_id,
    user_id,
    ChangeSource::Admin,
).await?;

println!("Restored to: {:?}", restored_post);
```

### Named Versions

```rust
// Create a named version (e.g., for releases)
service.create_named_version(
    tenant_id,
    post_id,
    "en",
    revision_id,
    "v1.0-published",
).await?;

// List all named versions
let versions = service.get_named_versions(tenant_id, post_id, "en").await?;
for version in versions {
    println!("{}: revision #{}", 
        version.version_name.unwrap(),
        version.revision_number
    );
}
```

### Retention Policies

```rust
use rustok_revisions::{RevisionConfig, RetentionPolicy};

impl RevisionConfig for Post {
    fn default_retention_policy() -> Option<RetentionPolicy> {
        // Keep only the last 100 revisions
        Some(RetentionPolicy::KeepLast(100))
    }
}

// Apply retention policy
let deleted = service.apply_retention_policy_for_type::<Post>(
    tenant_id,
    post_id,
    "en",
    &RetentionPolicy::KeepLast(100),
).await?;

println!("Deleted {} old revisions", deleted);
```

## Feature Flags

- `derive`: Enable derive macros for `Revisionable` trait
- `seaorm`: Enable SeaORM backend for PostgreSQL

## Documentation

- [API Reference](https://docs.rs/rustok-revisions)
- [Examples](examples/)
- [Migration Guide](../MIGRATION_GUIDE.md)
- [Best Practices](../BEST_PRACTICES.md)

## Testing

```bash
# Set up test database
export TEST_DATABASE_URL="postgres://localhost/rustok_revisions_test"

# Run tests
cargo test --all-features
```

## Performance

The library is designed for high performance:

- **Delta-based storage**: Only stores changes, not full copies
- **Efficient indexing**: Optimized database indexes
- **Batch operations**: Support for bulk operations
- **Connection pooling**: Built-in connection pool management

See [Performance Optimization Guide](../PERFORMANCE_OPTIMIZATION.md) for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Comparison

See how rustok-revisions compares to other solutions:

| Feature | rustok-revisions | PaperTrail | Django Simple History |
|---------|------------------|------------|----------------------|
| Language | Rust | Ruby | Python |
| Storage | Delta-based | Full snapshots | Full snapshots |
| Async | ✅ Native | ❌ No | ⚠️ Limited |
| Multi-tenant | ✅ Built-in | ❌ No | ❌ No |
| Named versions | ✅ Yes | ❌ No | ❌ No |
| Performance | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |

See [Comparison Matrix](../COMPARISON_MATRIX.md) for details.

## Support

- 📚 [Documentation](https://docs.rs/rustok-revisions)
- 💬 [Discord](https://discord.gg/rustok)
- 🐛 [Issue Tracker](https://github.com/RusTokRs/RusTok/issues)

## Roadmap

See [ROADMAP.md](../ROADMAP.md) for planned features and future development.
