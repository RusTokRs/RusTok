---
id: doc://docs/guides/testing-integration.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Integration Tests Guide

> **Status:** Active
> **Scope:** current live server test targets and crate-owned integration suites

## Overview

This guide documents only integration and smoke tests that are actually executed by
the current workspace.

Removed product surfaces and non-discovered test files must not stay in this guide.

## Current `apps/server` test targets

Server-level verification currently lives in top-level test targets under
`apps/server/tests/`:

- `commerce_openapi_contract.rs`
- `library_api_smoke.rs`
- `module_lifecycle.rs`
- `tenant_cache_stampede_test.rs`

`library_api_smoke.rs` also validates a checked-in fixture
`apps/server/tests/fixtures/order_scenario.rs.txt` to ensure host-level scenarios use
library contracts from `rustok-commerce` and `rustok-events`, rather than local
shadow types.

## Where domain integration tests live

Storage-owner domain scenarios are verified inside their owning crates:

- `crates/modules/rustok-blog/tests/*`
- `crates/modules/rustok-forum/tests/*`
- `crates/modules/rustok-pages/tests/*`
- `crates/modules/rustok-comments/tests/*`
- `crates/modules/rustok-content/tests/*`
- `crates/modules/rustok-commerce/tests/*`

This keeps server tests focused on host/runtime wiring and avoids reintroducing
removed product surfaces into `apps/server`.

## Running Tests

### Run a specific live server test target

```bash
cd apps/server
cargo test -p rustok-server --test module_lifecycle
```

### Compile a specific live server test target

```bash
cd apps/server
cargo test -p rustok-server --test module_lifecycle --no-run --config profile.test.debug=0
```

### Run crate-owned integration suites

```bash
cargo test -p rustok-blog --test integration
cargo test -p rustok-forum --test integration
cargo test -p rustok-pages --test integration
```

## Principles

- Keep only discoverable, executable tests in documented server test catalogs.
- Keep product-domain behavior in the owning crate.
- Delete obsolete test files instead of leaving them as pseudo-documentation.
- Prefer fixtures for contract-shape assertions over fake integration targets.

## Related Documents

- [Testing Guide](./testing.md)
- [Platform Verification Plans](../verification/README.md)
- [Server Docs](../../apps/server/docs/README.md)
