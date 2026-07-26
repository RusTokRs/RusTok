# FORUM-20AG static review checklist

- storefront request DTOs contain no tenant or recipient identity fields;
- tenant and recipient UUIDs derive only from `PortContext`;
- only user actors may access the port;
- all four reads require shared read/deadline policy;
- the group-state write requires shared write/deadline/idempotency policy;
- unread count delegates to `count_unread`;
- grouped summaries, items, open authorization, and state commands delegate to existing owners;
- owner validation and failures map to sanitized `PortError` values;
- source privacy/authorization ordering remains in the existing read owners;
- exact state/timestamp invariants remain in the existing command owner;
- no native server function, GraphQL resolver, route registration, UI state, or delivery attempt is added;
- parallel `main` changes reviewed during implementation touched Cart and Index only.

Tests, Cargo commands, formatting commands, verifier execution, workflows, and CI were not run by the implementation agent.
