# RUSTSEC-2026-0045 remediation note

## Summary

`cargo update -p aws-lc-sys` is the intended remediation for `RUSTSEC-2026-0045`, but dependency index access is currently blocked in this environment (`https://index.crates.io/config.json` returned HTTP 403), so the lockfile cannot be refreshed here.

## Reproduction

- `cargo update -p aws-lc-sys` fails due to crates.io index access restriction.
- `cargo update -p aws-lc-sys --offline` reports no newer cached package is available.

## Required follow-up

Run the following in a network-enabled CI or local environment and commit the resulting lockfile update:

```bash
cargo update -p aws-lc-sys
cargo tree -i aws-lc-sys --locked
```

Then verify that `Cargo.lock` no longer contains `aws-lc-sys 0.37.1`.
