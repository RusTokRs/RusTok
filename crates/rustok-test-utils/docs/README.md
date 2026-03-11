# rustok-test-utils documentation

## Purpose

`rustok-test-utils` provides reusable testing helpers for RusToK crates and applications.
It standardizes test setup patterns and reduces duplicated boilerplate in unit, integration,
and contract tests.

## Responsibilities

- Provide database test setup helpers.
- Provide event testing utilities (mock event bus/transport).
- Provide fixtures/builders for common domain entities.
- Provide helper functions and test context shortcuts for frequent scenarios.

## Interactions

- **Primary usage scope:** `dev-dependencies` in RusToK crates and app test targets.
- **Integration points:** interacts with `rustok-core` contracts and event abstractions in tests.
- **Quality role:** supports platform-level testing strategy and validation gates.

## Entry points

- `rustok_test_utils::setup_test_db`
- `rustok_test_utils::MockEventBus`
- `rustok_test_utils::MockEventTransport`
- `rustok_test_utils::mock_transactional_event_bus`
- `rustok_test_utils::fixtures::*`
- `rustok_test_utils::helpers::*`

## Related docs

- [Implementation plan](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Testing guide](../../../docs/guides/testing.md)
