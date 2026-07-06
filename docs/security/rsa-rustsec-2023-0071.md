---
id: doc://docs/security/rsa-rustsec-2023-0071.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# RUSTSEC-2023-0071 remediation note

## Status

Resolved on June 25, 2026.

Workspace migrated from backend `jsonwebtoken/rust_crypto` to
`jsonwebtoken/aws_lc_rs`. Support for `HS256` and `RS256` is preserved, and the
transitive dependency on `rsa 0.9.10` has been removed.

## Verification

```bash
cargo tree -i rsa@0.9.10 --workspace
cargo deny check advisories
cargo test -p rustok-auth --lib
```

The first command should not find any reverse dependencies. The
`RUSTSEC-2023-0071` exception has been removed from `deny.toml`.
