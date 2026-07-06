---
id: doc://docs/security/rustls-webpki-rustsec-2026-0099-0104.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# RUSTSEC-2026-0098 / RUSTSEC-2026-0099 / RUSTSEC-2026-0104: remediation

## Status

Closed: 2026-06-25.

Workspace no longer contains `rustls-webpki 0.101.7` and `rustls 0.21.12`. The AWS SDK/S3 branch has been migrated to the modern TLS path via `aws-smithy-http-client` with `rustls 0.23.40` and `rustls-webpki 0.103.13`.

## What Changed

- In `crates/rustok-storage/Cargo.toml`, default features are disabled for `aws-sdk-s3` and `aws-config`.
- For the S3 backend, only necessary runtime/TLS features are explicitly enabled:
  - `behavior-version-latest`;
  - `default-https-client`;
  - `rt-tokio`;
  - `sigv4a` for `aws-sdk-s3`.
- The legacy `rustls` feature, which pulled in `rustls 0.21.x`, is no longer enabled.
- Temporary ignores have been removed from `deny.toml`:
  - `RUSTSEC-2026-0098`;
  - `RUSTSEC-2026-0099`;
  - `RUSTSEC-2026-0104`.

## Verification

Local checks performed:

```bash
cargo check -p rustok-storage --features s3
cargo tree -i rustls-webpki@0.101.7 --workspace
cargo tree -i rustls@0.21.12 --workspace
cargo tree -i rustls-webpki --workspace
```

Expected result:

- `rustls-webpki@0.101.7` not found;
- `rustls@0.21.12` not found;
- `rustls-webpki` resolves to `0.103.13`;
- S3 backend compiles with the updated feature set.

Final gate for advisory policy:

```bash
cargo deny check advisories
```
