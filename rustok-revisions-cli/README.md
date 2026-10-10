# revctl - CLI for Content Revisions

Command-line tool for managing content revisions in the RusTok platform.

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/revctl
```

## Usage

### Global Options

```bash
revctl [OPTIONS] <COMMAND>

Options:
  -d, --database-url <DATABASE_URL>  Database URL [env: DATABASE_URL]
  -h, --help                         Print help
  -V, --version                      Print version
```

### Commands

#### `list` - List revisions for content

```bash
revctl list --tenant <TENANT_ID> --content <CONTENT_ID> [--locale <LOCALE>] [--limit <LIMIT>]
```

**Example:**
```bash
revctl list \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8 \
  --locale en \
  --limit 10
```

#### `show` - Show revision details

```bash
revctl show <REVISION_ID>
```

**Example:**
```bash
revctl show 550e8400-e29b-41d4-a716-446655440000
```

#### `diff` - Compare two revisions

```bash
revctl diff <FROM_ID> <TO_ID>
```

**Example:**
```bash
revctl diff \
  550e8400-e29b-41d4-a716-446655440000 \
  6ba7b810-9dad-11d1-80b4-00c04fd430c8
```

#### `restore` - Restore content to a revision

```bash
revctl restore \
  --tenant <TENANT_ID> \
  --content <CONTENT_ID> \
  --revision-id <REVISION_ID> \
  --user <USER_ID> \
  [--locale <LOCALE>] \
  [--yes]
```

**Example:**
```bash
revctl restore \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8 \
  --revision-id 7c9e6679-7425-40de-944b-e07fc1f90ae7 \
  --user 8f14e45f-ceea-367f-a17b-00c04fd430c8 \
  --yes
```

#### `tag` - Create named version

```bash
revctl tag \
  --tenant <TENANT_ID> \
  --content <CONTENT_ID> \
  --revision-id <REVISION_ID> \
  --name <VERSION_NAME> \
  [--locale <LOCALE>]
```

**Example:**
```bash
revctl tag \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8 \
  --revision-id 7c9e6679-7425-40de-944b-e07fc1f90ae7 \
  --name "v1.0-published"
```

#### `tags` - List named versions

```bash
revctl tags --tenant <TENANT_ID> --content <CONTENT_ID> [--locale <LOCALE>]
```

**Example:**
```bash
revctl tags \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8
```

#### `count` - Count revisions

```bash
revctl count --tenant <TENANT_ID> --content <CONTENT_ID> [--locale <LOCALE>]
```

**Example:**
```bash
revctl count \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8
```

#### `cleanup` - Remove old revisions

```bash
revctl cleanup \
  --tenant <TENANT_ID> \
  --content <CONTENT_ID> \
  --keep <COUNT> \
  [--locale <LOCALE>] \
  [--yes]
```

**Example:**
```bash
revctl cleanup \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8 \
  --keep 100 \
  --yes
```

#### `export` - Export revisions to JSON

```bash
revctl export \
  --tenant <TENANT_ID> \
  --content <CONTENT_ID> \
  --output <FILE> \
  [--locale <LOCALE>]
```

**Example:**
```bash
revctl export \
  --tenant 550e8400-e29b-41d4-a716-446655440000 \
  --content 6ba7b810-9dad-11d1-80b4-00c04fd430c8 \
  --output revisions.json
```

## Examples

### Complete Workflow

```bash
# Set database URL
export DATABASE_URL="postgres://user:pass@localhost/rustok"

# List all revisions for a post
revctl list --tenant $TENANT --content $POST_ID --limit 20

# Show details of a specific revision
revctl show $REVISION_ID

# Compare two revisions
revctl diff $OLD_REVISION $NEW_REVISION

# Create a named version
revctl tag --tenant $TENANT --content $POST_ID \
  --revision-id $REVISION_ID --name "v1.0"

# List all named versions
revctl tags --tenant $TENANT --content $POST_ID

# Restore to a previous version
revctl restore --tenant $TENANT --content $POST_ID \
  --revision-id $REVISION_ID --user $USER_ID --yes

# Cleanup old revisions (keep last 50)
revctl cleanup --tenant $TENANT --content $POST_ID --keep 50 --yes

# Export all revisions to JSON
revctl export --tenant $TENANT --content $POST_ID \
  --output post-revisions.json
```

### Scripting

```bash
#!/bin/bash
# backup-all-revisions.sh

export DATABASE_URL="postgres://user:pass@localhost/rustok"

# Get all content IDs (example query)
CONTENT_IDS=$(psql -t -c "SELECT DISTINCT content_id FROM content_revisions")

for CONTENT_ID in $CONTENT_IDS; do
  echo "Exporting revisions for $CONTENT_ID..."
  revctl export \
    --tenant $TENANT \
    --content $CONTENT_ID \
    --output "backup-${CONTENT_ID}.json"
done

echo "Backup complete!"
```

## Environment Variables

- `DATABASE_URL` - PostgreSQL connection string (required)

## Output Formats

### Table Output (list command)

```
Listing revisions for content 6ba7b810-9dad-11d1-80b4-00c04fd430c8 (locale: en)

 # | ID       | Event  | Created             | User     | Tag
---|----------|--------|---------------------|----------|-------------
 5 | 7c9e6679 | Update | 2024-01-15 14:30:00 | 8f14e45f | v1.0
 4 | 6ba7b810 | Update | 2024-01-15 14:25:00 | 8f14e45f | -
 3 | 550e8400 | Update | 2024-01-15 14:20:00 | 8f14e45f | -
 2 | 4d1e7c3a | Update | 2024-01-15 14:15:00 | 8f14e45f | -
 1 | 3b0d6e2f | Create | 2024-01-15 14:10:00 | 8f14e45f | -

Total: 5 revisions
```

### Diff Output

```
Comparing revisions 550e8400... → 7c9e6679...

Added Fields
  + new_field: "new value"

Removed Fields
  - old_field: "old value"

Changed Fields
  ~ title:
    from: "Old Title"
    to: "New Title"
  ~ content:
    from: "Old content"
    to: "New content"
```

### JSON Export Format

```json
{
  "tenant_id": "550e8400-e29b-41d4-a716-446655440000",
  "content_id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
  "locale": "en",
  "exported_at": "2024-01-15T14:30:00Z",
  "revision_count": 5,
  "revisions": [
    {
      "id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
      "revision_number": 5,
      "content_type": "blog_post",
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

## Exit Codes

- `0` - Success
- `1` - General error
- `2` - Invalid arguments
- `3` - Database error
- `4` - Revision not found

## Tips

### Using with jq

```bash
# Extract all revision IDs
revctl export --tenant $TENANT --content $POST_ID --output - | \
  jq '.revisions[].id'

# Count revisions by event type
revctl export --tenant $TENANT --content $POST_ID --output - | \
  jq '.revisions | group_by(.event) | map({event: .[0].event, count: length})'
```

### Using with scripts

```bash
# Restore and tag in one command
REVISION_ID="7c9e6679-7425-40de-944b-e07fc1f90ae7"
revctl restore --tenant $TENANT --content $POST_ID \
  --revision-id $REVISION_ID --user $USER_ID --yes && \
revctl tag --tenant $TENANT --content $POST_ID \
  --revision-id $REVISION_ID --name "restored-$(date +%Y%m%d)"
```

## License

MIT OR Apache-2.0
