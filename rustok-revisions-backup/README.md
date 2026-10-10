# rustok-revisions-backup

Backup and restore utility for rustok-revisions.

## Features

- ✅ Export revisions to JSON/GZIP files
- ✅ Import revisions from backup files
- ✅ List backup contents
- ✅ Verify backup integrity
- ✅ Skip existing revisions on import
- ✅ Dry run mode
- ✅ Progress bars and colored output

## Installation

```bash
cargo build --release
./target/release/revbackup
```

## Usage

### Environment Variables

- `DATABASE_URL` - PostgreSQL connection string (required)

### Commands

#### `export` - Export revisions to backup file

```bash
revbackup export --output backup.json.gz
```

**Options:**
- `-o, --output <FILE>` - Output file (gzipped if ends with .gz)
- `-t, --tenant <UUID>` - Filter by tenant ID (optional)
- `-c, --content-type <TYPE>` - Filter by content type (optional)
- `--include-content` - Include revision content (default: true)

**Examples:**
```bash
# Export all revisions
revbackup export -o all-revisions.json.gz

# Export for specific tenant
revbackup export -o tenant-backup.json.gz -t 550e8400-e29b-41d4-a716-446655440000

# Export specific content type
revbackup export -o posts.json.gz -c blog_post
```

#### `import` - Import revisions from backup file

```bash
revbackup import --input backup.json.gz
```

**Options:**
- `-i, --input <FILE>` - Input file
- `--skip-existing` - Skip revisions that already exist
- `--dry-run` - Don't actually import (preview only)

**Examples:**
```bash
# Import all revisions
revbackup import -i backup.json.gz

# Import, skipping existing
revbackup import -i backup.json.gz --skip-existing

# Dry run (preview)
revbackup import -i backup.json.gz --dry-run
```

#### `list` - List backup file contents

```bash
revbackup list --input backup.json.gz
```

**Example output:**
```
Listing backup file: backup.json.gz

Backup Metadata:
  Version: 0.1.0
  Created: 2024-01-15 14:30:00 UTC
  Revisions: 1234
  Tenant: 550e8400-e29b-41d4-a716-446655440000

Revisions:
  1. 7c9e6679 - blog_post - 5 - 2024-01-15 14:30:00
  2. 6ba7b810 - blog_post - 4 - 2024-01-15 14:25:00
  3. 550e8400 - blog_post - 3 - 2024-01-15 14:20:00
  ...
```

#### `verify` - Verify backup file integrity

```bash
revbackup verify --input backup.json.gz
```

**Example output:**
```
Verifying backup file: backup.json.gz
✓ Backup file is valid
  Version: 0.1.0
  Revisions: 1234
  Created: 2024-01-15 14:30:00 UTC
```

## Backup File Format

Backup files are JSON (optionally gzipped) with the following structure:

```json
{
  "metadata": {
    "version": "0.1.0",
    "created_at": "2024-01-15T14:30:00Z",
    "revision_count": 1234,
    "tenant_filter": null,
    "content_type_filter": null
  },
  "revisions": [
    {
      "id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
      "tenant_id": "550e8400-e29b-41d4-a716-446655440000",
      "content_id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
      "content_type": "blog_post",
      "locale": "en",
      "revision_number": 5,
      "event": "Update",
      "content": { "title": "...", "body": "..." },
      "user_id": "8f14e45f-ceea-367f-a17b-00c04fd430c8",
      "source": "Web",
      "summary": "Updated title",
      "created_at": "2024-01-15T14:30:00Z",
      "version_name": "v1.0"
    }
  ]
}
```

## Examples

### Complete Backup Workflow

```bash
# Set database URL
export DATABASE_URL="postgres://user:pass@localhost/rustok_revisions"

# Create backup
revbackup export -o backup-$(date +%Y%m%d).json.gz

# Verify backup
revbackup verify -i backup-20240115.json.gz

# List contents
revbackup list -i backup-20240115.json.gz

# Restore to new database
export DATABASE_URL="postgres://user:pass@localhost/rustok_revisions_new"
revbackup import -i backup-20240115.json.gz
```

### Scheduled Backups

```bash
#!/bin/bash
# backup-revisions.sh

export DATABASE_URL="postgres://user:pass@localhost/rustok_revisions"
BACKUP_DIR="/backups/revisions"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

echo "Creating backup..."
revbackup export -o $BACKUP_DIR/backup-$DATE.json.gz

echo "Verifying backup..."
revbackup verify -i $BACKUP_DIR/backup-$DATE.json.gz

# Keep only last 30 days
find $BACKUP_DIR -name "backup-*.json.gz" -mtime +30 -delete

echo "Backup complete: backup-$DATE.json.gz"
```

Add to crontab:
```bash
# Daily backup at 2 AM
0 2 * * * /path/to/backup-revisions.sh
```

### Migration Between Environments

```bash
# Export from production
export DATABASE_URL="postgres://prod-db/rustok_revisions"
revbackup export -o prod-backup.json.gz

# Import to staging
export DATABASE_URL="postgres://staging-db/rustok_revisions"
revbackup import -i prod-backup.json.gz --skip-existing
```

## Compression

Backup files can be compressed with gzip:

```bash
# Compressed (recommended)
revbackup export -o backup.json.gz

# Uncompressed
revbackup export -o backup.json
```

**Size comparison:**
- Uncompressed: ~100 MB for 100,000 revisions
- Compressed: ~10 MB for 100,000 revisions (90% reduction)

## Performance

**Export speed:**
- 10,000 revisions: ~5 seconds
- 100,000 revisions: ~30 seconds
- 1,000,000 revisions: ~5 minutes

**Import speed:**
- 10,000 revisions: ~10 seconds
- 100,000 revisions: ~60 seconds
- 1,000,000 revisions: ~10 minutes

## Troubleshooting

### Out of memory on large exports

**Problem:** Export fails with OOM error

**Solution:** Use pagination (not yet implemented) or export by tenant/content type

### Import conflicts

**Problem:** Import fails due to duplicate revision IDs

**Solution:** Use `--skip-existing` flag
```bash
revbackup import -i backup.json.gz --skip-existing
```

### Corrupted backup file

**Problem:** Import fails with JSON parse error

**Solution:** Verify backup first
```bash
revbackup verify -i backup.json.gz
```

## Exit Codes

- `0` - Success
- `1` - General error
- `2` - Invalid arguments
- `3` - Database error
- `4` - File I/O error
- `5` - Backup verification failed

## License

MIT OR Apache-2.0
