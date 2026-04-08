# rustok-test-utils

## Purpose

`rustok-test-utils` owns shared fixtures, mocks, and test helpers for RusToK workspace tests.

## Responsibilities

- Provide reusable test database setup helpers.
- Provide shared fixtures and builder-style test data factories.
- Provide mock event infrastructure and common test helpers.
- Keep cross-module test scaffolding out of production crates.

## Entry points

- `setup_test_db`
- `db::setup_test_db_with_migrations`
- `MockEventBus`
- `fixtures::*`
- `helpers::*`

## Interactions

- Used by domain modules, support crates, and host applications for consistent test setup.
- Depends on foundational runtime crates only for shared contracts needed by tests.
- Exists purely for test-time composition and should not absorb production runtime logic.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
