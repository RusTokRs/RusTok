# forum-20ah-native-storefront-adapter

Status: source-ready / unvalidated.

This owner note records the Forum/Notifications source boundary only; runtime execution and compiled verification remain maintainer-owned.

Tests, Cargo, formatting, and CI were not run by the implementation agent.

## Source contract

- # FORUM-20AH native notification storefront adapter
- OAuth service principals are rejected
- authentication and tenant admission occur before notification
- Arc<NotificationSourceRegistry>
- NotificationRecipientPolicyRuntime
