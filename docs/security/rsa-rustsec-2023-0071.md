# RUSTSEC-2023-0071 remediation note

## Summary

`cargo deny` reports `RUSTSEC-2023-0071` through `rsa 0.9.10`.

In this workspace, `rsa` is currently pulled transitively and cannot be moved to a fixed release with a plain `cargo update -p rsa --workspace` under the currently resolved dependency constraints.

## Reproduction

```bash
cargo tree -i rsa@0.9.10 --workspace
cargo update -p rsa --workspace
```

The update command currently reports no lockfile changes.

## Temporary policy

`deny.toml` temporarily ignores `RUSTSEC-2023-0071` to keep CI green while upstream dependency updates are prepared.

## Required follow-up

Re-run dependency refresh in a future bump cycle and remove the ignore once `rsa 0.9.10` is no longer present in `Cargo.lock`.
