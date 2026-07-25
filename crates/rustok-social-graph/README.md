# rustok-social-graph

Tenant-scoped owner for social relationships and privacy-relevant relation state.

The executable surface owns `block`, `mute`, and directional `follow` state.
Friendship, lists, recommendations, block/mute management transports, and product
UI remain deferred until matching owner contracts are implemented.

## Interactions

- depends on platform tenant/user identity only through migration ordering and
  tenant-composite foreign keys;
- exposes neutral command, privacy read, and revision-bearing follow-state read ports from this crate;
- exposes an optional module-owned GraphQL follow transport through the `graphql`
  feature;
- does not depend on Profiles, Notifications, or read foreign-domain persistence;
- the server and other modules may adapt owner ports into consumer-specific runtime ports;
- block is strict and symmetric for privacy evaluation when either direction is active;
- mute is directional from the muting user to the hidden user;
- follow is directional from follower (`source_user_id`) to profile owner
  (`target_user_id`);
- follow presentation reads support one bounded batch of at most 100 target users,
  deduplicate target ids, and return only active relations;
- `SocialGraphFollowReadPort` returns active state plus the optional current revision for one owned source/target pair, including inactive persisted relations;
- user port actors may read or mutate only relation sources they own, while
  service/system actors remain available for owner composition;
- GraphQL `isFollowing`, `followState`, `followUser`, and `unfollowUser` are human-user-only,
  tenant-bound, and call owner ports with deadline and idempotency semantics;
- GraphQL optimistic revisions are represented as positive 64-bit integer strings;
- missing/error owner state must not be converted into implicit allow.

## Verification

```bash
cargo check -p rustok-social-graph --all-targets
cargo check -p rustok-social-graph --features graphql --all-targets
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_state_sqlite -- --nocapture
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
```
