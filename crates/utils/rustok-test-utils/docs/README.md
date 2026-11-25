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

The opt-in `jwt` feature provides `jwt::JwtKeyringFixture::generate()` for
four independent in-memory RSA-2048 key pairs and
`jwt::RsaKeyPairFixture::generate()` for additional rotation keys. Serialize
a complete generation-bearing document with `fixture.document(generation)`
and pass it to the Auth-owned document builder. Generate a new fixture per
isolated security store; retain the fixture only when testing restart,
replication, or rotation within the same store. The helper does not provision
the MAC root, call OpenSSL as an external process, or write private keys.
It uses the same locked AWS-LC backend and PEM codec already present in the
workspace. Source regressions are authored; execution remains pending.
