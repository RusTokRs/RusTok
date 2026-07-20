# rustok-test-utils implementation plan

## Current state

`rustok-test-utils` is the shared test-time composition crate. It provides
SQLite setup and transactions, event-bus/transport doubles, fixtures, identity
helpers, and common assertions. It is used by the server and multiple domain
modules and must remain confined to test dependencies; it owns no production
runtime or domain behavior.

## Boundary

- Owner: platform testing foundation.
- Consumers use it only from test or dev dependencies. Production crates must
  not gain a `rustok-test-utils` dependency.
- `rustok-core`, `rustok-events`, and `rustok-api` contracts remain canonical;
  test utilities provide fixtures and doubles rather than replacement runtime
  semantics.
- The crate depends only on shared foundation/event test contracts. Domain
  modules must not be pulled into its dependency graph.

## Next results

1. **Finish the neutral server-test migration.** Replace remaining
   legacy framework test-context use with reusable `rustok-test-utils` server/runtime
   fixtures. Done when the server test
   suite no longer needs framework test-context imports outside an explicitly
   temporary, named migration bridge.
2. **Lock the public mock and fixture contract.** Add focused regression tests
   for database transaction isolation, fixture defaults, tenant/RBAC context,
   event publication order, tenant filtering, and clear/reset behavior. Done
   when a breaking helper change fails self-tests rather than a downstream
   module suite.
3. **Publish supported testing recipes from real consumers.** Maintain a small
   scenario-to-helper guide for database, event/outbox, tenancy/RBAC, and
   cross-module integration tests; remove duplicated local helpers only after
   equivalent shared coverage exists. Done when new module tests can select a
   documented shared pattern without importing production logic.

## Verification

- `cargo test -p rustok-test-utils`
- `scripts/verify/verify-code-quality.sh` (test-utils remains dev-only for
  consumers)
- Targeted server test migration checks from the server test plan.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Testing guide](../../../docs/guides/testing.md)
- [Axum runtime and operations CLI boundary](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md)
