# `rustok-test-utils` Documentation

`rustok-test-utils` — shared support crate for the RusToK testing infrastructure.
It holds reusable fixtures, mocks and helpers that should reduce
local duplication in unit/integration/contract tests.

## Purpose

- publish a canonical shared testing helper surface;
- standardize test setup patterns for platform and module tests;
- reduce the number of ad-hoc fixtures and local mock implementations in the workspace.

## Scope

- database setup helpers;
- mock event bus/transport utilities;
- fixtures/builders for common domain entities;
- helper functions and test context shortcuts;
- no production runtime logic or domain-owned behavior.

## Integration

- used as `dev-dependencies` in crates and app test targets;
- relies on `rustok-core`/`rustok-events` contracts for test doubles and fixtures;
- the testing guide and module-level verification docs must remain synchronized with this crate;
- extension of helpers must go through reusable patterns, not through random one-off fixtures.

## Verification

- structural verification for local docs and the public test-utils surface;
- targeted self-tests needed when changing fixtures, mocks and helper contracts;
- consumer-module docs updated when changing recommended testing patterns.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Testing guide](../../../docs/guides/testing.md)
