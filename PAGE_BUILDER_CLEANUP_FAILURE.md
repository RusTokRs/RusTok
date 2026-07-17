# Page Builder current-only cleanup failure

```text
error: let chains are only allowed in Rust 2024 or later
   --> /home/runner/work/RusTok/RusTok/apps/server/src/services/channel_cache_invalidation.rs:203:12
    |
203 |         if let Some(previous) = previous
    |            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Error writing files: failed to resolve mod `channel_cache_invalidation`: cannot parse /home/runner/work/RusTok/RusTok/apps/server/src/services/channel_cache_invalidation.rs
error: let chains are only allowed in Rust 2024 or later
   --> /home/runner/work/RusTok/RusTok/apps/server/tests/channel_cache_resolved_value.rs:191:16
    |
191 |             if let Ok(client) = redis::Client::open(url)
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
   --> /home/runner/work/RusTok/RusTok/apps/server/tests/channel_cache_resolved_value.rs:192:20
    |
192 |                 && let Ok(mut connection) = client.get_multiplexed_async_connection().await
    |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
   --> /home/runner/work/RusTok/RusTok/apps/server/tests/channel_cache_resolved_value.rs:235:16
    |
235 |             if let Ok(client) = redis::Client::open(url)
    |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
   --> /home/runner/work/RusTok/RusTok/apps/server/tests/channel_cache_resolved_value.rs:236:20
    |
236 |                 && let Ok(mut connection) = client.get_multiplexed_async_connection().await
    |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
  --> /home/runner/work/RusTok/RusTok/crates/rustok-cache/src/startup_recovery_tests.rs:36:16
   |
36 |             if let Ok(client) = redis::Client::open(url.as_str())
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
  --> /home/runner/work/RusTok/RusTok/crates/rustok-cache/src/startup_recovery_tests.rs:37:20
   |
37 |                 && let Ok(mut connection) = client.get_multiplexed_async_connection().await
   |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

Error writing files: failed to resolve mod `startup_recovery_tests`: cannot parse /home/runner/work/RusTok/RusTok/crates/rustok-cache/src/startup_recovery_tests.rs
error: let chains are only allowed in Rust 2024 or later
  --> /home/runner/work/RusTok/RusTok/crates/rustok-cache/tests/fallback_cas_live.rs:20:16
   |
20 |             if let Ok(client) = redis::Client::open(url)
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
  --> /home/runner/work/RusTok/RusTok/crates/rustok-cache/tests/fallback_cas_live.rs:21:20
   |
21 |                 && let Ok(mut connection) = client.get_multiplexed_async_connection().await
   |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
  --> /home/runner/work/RusTok/RusTok/crates/rustok-cache/tests/real_redis_hardening.rs:45:16
   |
45 |             if let Ok(client) = redis::Client::open(url)
   |                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: let chains are only allowed in Rust 2024 or later
  --> /home/runner/work/RusTok/RusTok/crates/rustok-cache/tests/real_redis_hardening.rs:46:20
   |
46 |                 && let Ok(mut connection) = client.get_multiplexed_async_connection().await
   |                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

```
