# rustok-social-graph

Tenant-scoped owner for social relationships and privacy-relevant relation state.

The initial executable surface owns `block` and `mute` state. Friendship, follow,
lists, recommendations, and public transports remain deferred until matching
owner contracts are implemented.

## Interactions

- depends on platform tenant/user identity only through migration ordering and
  tenant-composite foreign keys;
- exposes neutral command and privacy read ports from this crate;
- does not depend on Notifications or read notification persistence;
- the server may adapt `SocialGraphPrivacyReadPort` into consumer-specific
  runtime ports;
- block is strict and symmetric for privacy evaluation when either direction is
  active;
- mute is directional from the muting user to the hidden user;
- missing/error owner state must not be converted into implicit allow.

## Verification

```bash
cargo check -p rustok-social-graph --all-targets
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
node scripts/verify/verify-social-graph-notification-policy.mjs
```
