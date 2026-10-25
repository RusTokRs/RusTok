# Comments delegation — preserved-owner snapshot (retired live pins)

Status: `historical_record`
Date recorded: 2026-10-26

## What this is

Six Blog/Comments guards used to require that a list of files they did *not* own stayed
byte-identical to a recorded git blob SHA. The pinned set included `crates/libs/rustok-events/src/lib.rs`,
`apps/server/Cargo.toml`, `Cargo.lock` and the slice plans of other owners. The rule said:
"this slice did not modify these files".

That is a true statement about one commit and a false statement about every commit after it: any
legitimate change to those files — a dependency bump in `Cargo.toml`, a new event type, a reworded
plan — makes the digest differ, and the guard reports drift for work it was never meant to judge. Six
guards were red for exactly that reason while every behavioural invariant they also check was intact.
Refreshing the hashes would have kept the mechanism and destroyed its meaning, so the byte-level
assertion is retired as a *live rule* and preserved here as a record. Git history remains the source of
truth for what those commits contained.

## What still guards the same ground

* every behavioural marker in the six guards is unchanged and still enforced;
* workflow actions stay commit-pinned through `npm run verify:workflow-action-pins`;
* owner manifests and the lockfile have their own gates (`verify-dependency*`, `verify-release-*`).

## Recorded snapshot

### `scripts/verify/verify-blog-comments-audited-worker-response-disconnect.mjs`

audited worker-response disconnect (slice 85)

| file | git blob SHA at slice close |
|---|---|
| ``storePath`` | `4094539df6158d0a899e641322ae84bdbb6a129f` |
| `apps/server/Cargo.toml` | `f72bc1ce6f615150c86afa55ae0bc5b3478b366b` |
| `apps/server/tests/blog_comments_schedule_audit_postgres.rs` | `7b726fe69de5d678021bb376ea4ca9a0cb2b89e6` |
| `crates/modules/rustok-blog/docs/implementation-plan-slice-85.md` | `f584014c2693cd43ce42eca4b93b1a54eae3f986` |

### `scripts/verify/verify-blog-comments-audit-canonical-handoff-contract.mjs`

canonical audit handoff admission (slice 86)

| file | git blob SHA at slice close |
|---|---|
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs` | `4094539df6158d0a899e641322ae84bdbb6a129f` |
| `crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs` | `2f6473d7889a242ef86621b8615834e8bffe1013` |
| `crates/modules/rustok-blog/docs/implementation-plan-slice-86.md` | `c969aed84350f2ec7af4c60846f429debc47dfbe` |
| ``relayPath`` | `47f98da024744f5d61c9faeca7492a4a6456c75d` |
| `crates/modules/rustok-outbox/docs/implementation-plan.md` | `b198630b31bff569503c45428c639ec4507259f7` |
| `crates/libs/rustok-events/src/lib.rs` | `d2593f110102800bd1e0e77f2d4aeaaf7791ae7c` |
| `crates/libs/rustok-events/src/contract.rs` | `caee66898db718514c6695b12cba1df3e6b491b5` |
| `crates/libs/rustok-events/contracts/event-contract-digests.json` | `5bac36a17bbfc4c6f485cfb92e81ab50488b7e1f` |
| `apps/server/Cargo.toml` | `f72bc1ce6f615150c86afa55ae0bc5b3478b366b` |

### `scripts/verify/verify-blog-comments-tcp-delegation-keyring-host.mjs`

TCP delegation keyring host wrapper

| file | git blob SHA at slice close |
|---|---|
| ``basePath`` | `3f7a04899f8a31717f365d6dd69f7b4785d10203` |

### `scripts/verify/verify-blog-comments-tcp-delegation-schedule-audit-postgres-faults.mjs`

audited PostgreSQL commit-fault harness

| file | git blob SHA at slice close |
|---|---|
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_base.rs` | `ffcb6586e3dc3f1b2d6f21f4d1ab405fb5657fcd` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs` | `4a65f209a7fc23d769160166b3b8bbe462916115` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs` | `f805a7c0c733f4a17ed79c4d019f32b90a7e606d` |
| `crates/modules/rustok-comments/src/tcp_delegation_schedule.rs` | `2cd2a8c2bba97bff61ea5a37fd21405c2c482717` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence.rs` | `5834d1021dfad4fed23e8ade98aff816f004e865` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persisted_trigger.rs` | `17a3be2732cd7fbd48981ff417265055ce86e4b2` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_bridge.rs` | `9ec8190ff19ad1603e60a5d60c566bb8f417ef6c` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres.rs` | `8f3a5d8b988ad278ebbe33d00f7c663448f6f62f` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs` | `4094539df6158d0a899e641322ae84bdbb6a129f` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs` | `8660fd1b96997bcdbaf52718847ce7c8b7eeb7e0` |
| `crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs` | `2f6473d7889a242ef86621b8615834e8bffe1013` |
| `apps/server/tests/blog_comments_schedule_audit_postgres.rs` | `7b726fe69de5d678021bb376ea4ca9a0cb2b89e6` |
| `apps/server/Cargo.toml` | `f72bc1ce6f615150c86afa55ae0bc5b3478b366b` |

### `scripts/verify/verify-blog-comments-tcp-delegation-schedule-audit-postgres-harness.mjs`

audited PostgreSQL harness

| file | git blob SHA at slice close |
|---|---|
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_base.rs` | `ffcb6586e3dc3f1b2d6f21f4d1ab405fb5657fcd` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_guard.rs` | `4a65f209a7fc23d769160166b3b8bbe462916115` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_trigger.rs` | `f805a7c0c733f4a17ed79c4d019f32b90a7e606d` |
| `crates/modules/rustok-comments/src/tcp_delegation_schedule.rs` | `2cd2a8c2bba97bff61ea5a37fd21405c2c482717` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence.rs` | `5834d1021dfad4fed23e8ade98aff816f004e865` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persisted_trigger.rs` | `17a3be2732cd7fbd48981ff417265055ce86e4b2` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_bridge.rs` | `9ec8190ff19ad1603e60a5d60c566bb8f417ef6c` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres.rs` | `8f3a5d8b988ad278ebbe33d00f7c663448f6f62f` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs` | `4094539df6158d0a899e641322ae84bdbb6a129f` |
| `apps/server/src/services/comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs` | `8660fd1b96997bcdbaf52718847ce7c8b7eeb7e0` |
| `crates/modules/rustok-blog/src/migrations/m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox.rs` | `2f6473d7889a242ef86621b8615834e8bffe1013` |
| `apps/server/Cargo.toml` | `f72bc1ce6f615150c86afa55ae0bc5b3478b366b` |

### `scripts/verify/verify-blog-comments-tcp-delegation-schedule-postgres.mjs`

TCP delegation schedule PostgreSQL persistence

| file | git blob SHA at slice close |
|---|---|
| ``scheduleBasePath`` | `ffcb6586e3dc3f1b2d6f21f4d1ab405fb5657fcd` |
| ``scheduleGuardPath`` | `4a65f209a7fc23d769160166b3b8bbe462916115` |
| ``processTriggerPath`` | `f805a7c0c733f4a17ed79c4d019f32b90a7e606d` |
| ``commentsSchedulePath`` | `2cd2a8c2bba97bff61ea5a37fd21405c2c482717` |
| ``persistencePath`` | `5834d1021dfad4fed23e8ade98aff816f004e865` |
| ``persistedTriggerPath`` | `17a3be2732cd7fbd48981ff417265055ce86e4b2` |
| ``bridgePath`` | `9ec8190ff19ad1603e60a5d60c566bb8f417ef6c` |
| ``serverManifestPath`` | `f72bc1ce6f615150c86afa55ae0bc5b3478b366b` |
| ``blogManifestPath`` | `6ddf884489822b893a6ee6e449e9287e56b28f3f` |
| ``lockPath`` | `2241697f53e2a264bce6fa529c07fcb6d2048d83` |

## How to read this table

These digests are frozen evidence of the slices named above; they are not re-verified and they are
not expected to match the current tree. A guard that needs a fact about the present must state that
fact as a marker over the current source — which is what the six guards now do.
