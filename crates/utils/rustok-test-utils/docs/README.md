# rustok-test-utils documentation

`rustok-test-utils` provides shared fixtures, mocks, database setup, and
helpers for RusToK unit, integration, and contract tests. It is test-time
support only and must not contain production runtime or domain behavior.

Use it from test/dev dependencies for database, event/outbox, tenancy/RBAC, and
common fixture setup. The active migration and contract work is recorded in the
[implementation plan](./implementation-plan.md).

`db` also owns bounded PostgreSQL connections plus validated isolated-database
name, URL, create, force-drop, and information-schema assertion helpers for
module migration tests.
