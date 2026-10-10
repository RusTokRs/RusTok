# Getting Started Guide

Welcome to RusTok Revisions! This guide will help you get up and running in 5 minutes.

## 📋 Prerequisites

- Rust 1.75 or later
- PostgreSQL 12 or later
- Basic knowledge of Rust and async programming

## 🚀 Quick Start (5 Minutes)

### Step 1: Install Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify installation:
```bash
rustc --version
cargo --version
```

### Step 2: Set Up Database

Start PostgreSQL (using Docker):

```bash
docker run -d \
  --name rustok-postgres \
  -e POSTGRES_USER=rustok \
  -e POSTGRES_PASSWORD=rustok_password \
  -e POSTGRES_DB=rustok_revisions \
  -p 5432:5432 \
  postgres:15
```

Or use the provided docker-compose:

```bash
cd /home/user/RusTok
docker-compose up -d postgres
```

### Step 3: Clone and Build

```bash
cd /home/user/RusTok

# Build all crates
cargo build --workspace --all-features
```

### Step 4: Run Migrations

```bash
# Install sea-orm-cli
cargo install sea-orm-cli

# Run migrations
cd rustok-revisions
sea-orm-cli migrate up
```

### Step 5: Run Your First Example

```bash
# Set database URL
export DATABASE_URL="postgres://rustok:rustok_password@localhost:5432/rustok_revisions"

# Run basic example
cargo run --example basic_usage
```

You should see output like:
```
Creating revision for post 123...
✓ Created revision #1
Listing revisions...
  Revision #1 - 2026-01-09 10:30:00 UTC
```

🎉 **Congratulations! You're ready to use RusTok Revisions!**

---

## 📚 What's Next?

### 1. Understand the Basics

Read the core concepts:

- **Revisions** - Snapshots of content at a point in time
- **Delta-based storage** - Only stores changes (95% space savings)
- **Multi-tenant** - Isolated revisions per tenant
- **Multilingual** - Track revisions per locale

### 2. Try the Examples

We provide 3 complete examples:

```bash
# Simple blog post tracking
cargo run --example basic_usage

# Advanced multi-tenant scenario
cargo run --example advanced_usage

# Real-world e-commerce
cargo run --example ecommerce
```

### 3. Use the CLI Tool

The CLI tool (`revctl`) provides 9 commands for managing revisions:

```bash
# Install CLI
cargo install --path rustok-revisions-cli

# List revisions
revctl list --tenant <TENANT_ID> --content <CONTENT_ID>

# Show revision details
revctl show <REVISION_ID>

# Compare revisions
revctl compare <FROM_ID> <TO_ID>

# Restore to previous revision
revctl restore --tenant <ID> --content <ID> --revision <REV_ID> --user <USER_ID>

# Create named version
revctl tag --tenant <ID> --content <ID> --revision <REV_ID> --name "v1.0"

# List named versions
revctl tags --tenant <ID> --content <ID>

# Count revisions
revctl count --tenant <ID> --content <ID>

# Apply retention policy
revctl cleanup --tenant <ID> --content <ID> --keep-last 100

# Export to JSON
revctl export --tenant <ID> --content <ID> --output backup.json
```

### 4. Set Up Monitoring

Start the monitoring server:

```bash
# Install monitoring server
cargo install --path rustok-revisions-monitoring

# Run monitoring server
export DATABASE_URL="postgres://rustok:rustok_password@localhost:5432/rustok_revisions"
revisions-monitor
```

Access endpoints:
- Health check: http://localhost:8080/health
- Metrics: http://localhost:8080/metrics
- Dashboard: http://localhost:8080/dashboard

### 5. Configure Backups

Set up automated backups:

```bash
# Install backup utility
cargo install --path rustok-revisions-backup

# Create backup
export DATABASE_URL="postgres://rustok:rustok_password@localhost:5432/rustok_revisions"
revbackup export -o backup-$(date +%Y%m%d).json.gz

# Verify backup
revbackup verify -i backup-20260109.json.gz

# List backup contents
revbackup list -i backup-20260109.json.gz
```

---

## 💻 Basic Usage Example

Here's a complete example of tracking blog post revisions:

```rust
use rustok_revisions::{
    RevisionService, SeaOrmBackend, RevisionEvent,
    Revisionable, RevisionTracker, ChangeSource
};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

// 1. Define your content type
#[derive(Clone, Serialize, Deserialize, Revisionable)]
#[revision(content_type = "blog_post")]
struct Post {
    title: String,
    content: String,
    tags: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 2. Connect to database
    let database_url = "postgres://rustok:rustok_password@localhost/rustok_revisions";
    let backend = SeaOrmBackend::new(database_url).await?;
    let service = RevisionService::new(backend);
    
    // 3. Create your content
    let tenant_id = Uuid::new_v4();
    let content_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    let post = Post {
        title: "Hello World".to_string(),
        content: "This is my first post".to_string(),
        tags: vec!["intro".to_string()],
    };
    
    // 4. Create initial revision
    let revision = service.create_revision(
        tenant_id,
        content_id,
        "en",
        &post,
        user_id,
        ChangeSource::Web,
        RevisionEvent::Create,
    ).await?;
    
    println!("Created revision #{}", revision.revision_number);
    
    // 5. Update content
    let old_post = post.clone();
    let mut updated_post = post;
    updated_post.title = "Hello Rust".to_string();
    updated_post.tags.push("rust".to_string());
    
    // 6. Create update revision
    let tracker = RevisionTracker::builder()
        .source(ChangeSource::Web)
        .summary("Updated title and added rust tag")
        .build();
    
    let revision = service.create_revision_with_tracker(
        tenant_id,
        content_id,
        "en",
        &old_post,
        &updated_post,
        user_id,
        &tracker,
        RevisionEvent::Update,
    ).await?;
    
    println!("Created revision #{}", revision.revision_number);
    
    // 7. List all revisions
    let revisions = service.list_revisions(
        tenant_id,
        content_id,
        "en",
        Some(10),  // limit
        None,      // offset
    ).await?;
    
    println!("Total revisions: {}", revisions.len());
    
    // 8. Compare revisions
    if revisions.len() >= 2 {
        let diff = service.compare_revisions(
            revisions[1].id,
            revisions[0].id,
        ).await?;
        
        println!("Changes:");
        println!("  Added: {:?}", diff.added);
        println!("  Removed: {:?}", diff.removed);
        println!("  Changed: {:?}", diff.changed);
    }
    
    Ok(())
}
```

---

## 🔧 Configuration

### Environment Variables

```bash
# Database connection
export DATABASE_URL="postgres://user:pass@localhost/rustok_revisions"

# Monitoring server
export MONITOR_PORT=8080
export RUST_LOG=info

# Backup utility
export BACKUP_DIR="/backups/revisions"
export COMPRESSION_LEVEL=6
```

### Database Configuration

Create a custom configuration:

```rust
use sea_orm::{ConnectOptions, Database};

let mut opt = ConnectOptions::new(database_url);
opt.max_connections(100)
   .min_connections(5)
   .connect_timeout(Duration::from_secs(8))
   .idle_timeout(Duration::from_secs(8))
   .sqlx_logging(true);

let db = Database::connect(opt).await?;
```

---

## 📖 Common Use Cases

### Use Case 1: Blog with Revision History

Track all changes to blog posts:

```rust
async fn update_post(
    service: &RevisionService,
    tenant_id: Uuid,
    post_id: Uuid,
    new_post: Post,
    user_id: Uuid,
) -> Result<()> {
    // Get old version
    let old_post = get_post_from_db(post_id).await?;
    
    // Update in database
    update_post_in_db(&new_post).await?;
    
    // Create revision
    let tracker = RevisionTracker::builder()
        .source(ChangeSource::Web)
        .build();
    
    service.create_revision_with_tracker(
        tenant_id,
        post_id,
        "en",
        &old_post,
        &new_post,
        user_id,
        &tracker,
        RevisionEvent::Update,
    ).await?;
    
    Ok(())
}
```

### Use Case 2: Document with Undo/Redo

Implement undo/redo for documents:

```rust
async fn undo(
    service: &RevisionService,
    tenant_id: Uuid,
    doc_id: Uuid,
    user_id: Uuid,
) -> Result<Document> {
    // Get current revision
    let revisions = service.list_revisions(
        tenant_id,
        doc_id,
        "en",
        Some(2),
        None,
    ).await?;
    
    if revisions.len() < 2 {
        return Err("Nothing to undo".into());
    }
    
    // Restore to previous revision
    let (doc, _) = service.restore_revision(
        tenant_id,
        doc_id,
        "en",
        revisions[1].id,
        user_id,
        ChangeSource::Web,
    ).await?;
    
    Ok(doc)
}
```

### Use Case 3: Audit Trail

Create an audit trail for compliance:

```rust
async fn create_audit_trail(
    service: &RevisionService,
    tenant_id: Uuid,
    entity_id: Uuid,
    user_id: Uuid,
    action: &str,
) -> Result<()> {
    let tracker = RevisionTracker::builder()
        .source(ChangeSource::Admin)
        .summary(format!("Audit: {}", action))
        .metadata("ip_address", "192.168.1.1")
        .metadata("user_agent", "Mozilla/5.0")
        .build();
    
    service.create_revision_with_tracker(
        tenant_id,
        entity_id,
        "en",
        &old_entity,
        &new_entity,
        user_id,
        &tracker,
        RevisionEvent::Update,
    ).await?;
    
    Ok(())
}
```

---

## 🐛 Troubleshooting

### Problem: Database connection failed

**Solution:**
```bash
# Check PostgreSQL is running
docker ps | grep postgres

# Check connection
psql postgres://rustok:rustok_password@localhost:5432/rustok_revisions

# Check migrations
sea-orm-cli migrate status
```

### Problem: Compilation errors

**Solution:**
```bash
# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build --workspace --all-features
```

### Problem: Tests failing

**Solution:**
```bash
# Ensure test database exists
createdb rustok_revisions_test

# Run migrations on test database
export DATABASE_URL="postgres://localhost/rustok_revisions_test"
sea-orm-cli migrate up

# Run tests
cargo test --all-features
```

---

## 📚 Additional Resources

### Documentation

- **README.md** - Project overview
- **ARCHITECTURE.md** - System architecture
- **API_REFERENCE.md** - Complete API documentation
- **DEPLOYMENT.md** - Deployment guide
- **MONITORING.md** - Monitoring setup
- **BACKUP.md** - Backup & recovery

### Examples

- **basic_usage.rs** - Simple example
- **advanced_usage.rs** - Advanced features
- **ecommerce.rs** - Real-world scenario

### Tools

- **revctl** - CLI tool (9 commands)
- **revisions-monitor** - Monitoring server (3 endpoints)
- **revbackup** - Backup utility (4 commands)

---

## 🎯 Next Steps

1. **Read the Architecture** - Understand how the system works
2. **Try the Examples** - Learn by doing
3. **Use the CLI** - Manage revisions from command line
4. **Set Up Monitoring** - Track system health
5. **Configure Backups** - Protect your data
6. **Deploy to Production** - Go live!

---

## 🆘 Getting Help

### Documentation

- Check the [README.md](README.md) for overview
- Read [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- Browse [API_REFERENCE.md](API_REFERENCE.md) for API details

### Examples

- Run `cargo run --example basic_usage` for simple example
- Run `cargo run --example advanced_usage` for advanced features
- Run `cargo run --example ecommerce` for real-world scenario

### Issues

- GitHub Issues: https://github.com/RusTokRs/RusTok/issues
- GitHub Discussions: https://github.com/RusTokRs/RusTok/discussions

---

## ✅ Checklist

Before moving to production:

- [ ] Install Rust and build all crates
- [ ] Set up PostgreSQL database
- [ ] Run migrations
- [ ] Run tests (`cargo test --all-features`)
- [ ] Try examples (`cargo run --example basic_usage`)
- [ ] Install CLI tool (`cargo install --path rustok-revisions-cli`)
- [ ] Set up monitoring server
- [ ] Configure automated backups
- [ ] Review security settings
- [ ] Deploy to staging
- [ ] Load test
- [ ] Deploy to production

---

**You're all set! Happy coding with RusTok Revisions!** 🚀
