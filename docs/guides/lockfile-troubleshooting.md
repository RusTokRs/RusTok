---
id: doc://docs/guides/lockfile-troubleshooting.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Cargo.lock checksum troubleshooting

If Cargo reports a checksum mismatch such as:

```
checksum for `itertools v0.14.0` changed between lock files
```

it indicates the lockfile no longer matches the registry index. This typically
happens when a lockfile was edited manually or a registry mirror returned a
different checksum.

## Recommended fixes

1. Regenerate the lockfile with network access:

   ```sh
   cargo generate-lockfile
   ```

2. If a single crate is involved, refresh just that crate entry:

   ```sh
   cargo update -p itertools --precise 0.14.0
   ```

3. Re-run workspace checks:

   ```sh
   cargo clippy --workspace --all-targets -- -D warnings
   ```

If you are in an offline or restricted network environment, perform the lockfile
regeneration on a machine with registry access and commit the updated
`Cargo.lock`.

