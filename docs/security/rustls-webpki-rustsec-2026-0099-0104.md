# RUSTSEC-2026-0098 / RUSTSEC-2026-0099 / RUSTSEC-2026-0104 remediation note

## Summary

`rustls-webpki` advisory fixes require `>=0.103.13`, but the workspace currently reaches `rustls-webpki 0.101.7` through `rustls 0.21.x` pulled by `aws-smithy-http-client 1.1.12` (via `aws-config` / `aws-sdk-s3`).

At the moment, `cargo update` cannot move this chain to a non-vulnerable `rustls-webpki` because upstream semver constraints in the resolved AWS SDK dependency graph still pin the legacy `rustls 0.21` line.

## Reproduction

```bash
cargo tree -i rustls-webpki@0.101.7 --workspace
cargo update -p rustls-webpki@0.101.7 --precise 0.103.13 --workspace
```

The second command fails with:

- `failed to select a version for the requirement rustls-webpki = "^0.101.7"`
- required by `rustls v0.21.12` from `aws-smithy-http-client v1.1.12`

## Temporary policy

`deny.toml` advisory ignores were added for:

- `RUSTSEC-2026-0098`
- `RUSTSEC-2026-0099`
- `RUSTSEC-2026-0104`

This is a temporary unblock until the AWS SDK chain can be updated to a `rustls` line that accepts fixed `rustls-webpki` versions.

## Required follow-up

In a future dependency refresh, run:

```bash
cargo update -p aws-config -p aws-sdk-s3 -p aws-smithy-runtime -p aws-smithy-http-client
cargo tree -i rustls-webpki --workspace
```

Then remove all three ignore entries from `deny.toml` after `rustls-webpki 0.101.7` is no longer present in `Cargo.lock`.
