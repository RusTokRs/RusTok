# forum-20ag-storefront-port

Status: source-ready / unvalidated.

This owner note records the Forum/Notifications source boundary only; runtime execution and compiled verification remain maintainer-owned.

Tests, Cargo, formatting, and CI were not run by the implementation agent.

## Source contract

- # FORUM-20AG notification inbox storefront port
- PortContext.tenant_id
- PortActorKind::User
- PortCallPolicy::read()
- PortCallPolicy::write()
- tests/inbox_storefront_port_sqlite.rs
- does not add a Leptos server function
