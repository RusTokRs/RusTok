# Rust E2E Browser Tests

This crate contains Rust-owned browser smoke tests for Leptos/Trunk surfaces.

It is a Cargo workspace member and intentionally uses `playwright-rs` instead of
the Node Playwright runner. Next.js apps keep their own `@playwright/test`
configuration inside the app boundary.

Run the Leptos admin smoke test:

```powershell
trunk serve --address 127.0.0.1 --port 8080
$env:RUSTOK_LEPTOS_ADMIN_E2E_URL = "http://127.0.0.1:8080"
cargo test -p rustok-e2e-rust --test leptos_admin_smoke -- --nocapture
```

Run the Forum/Reactions storefront evidence against a prepared tenant where the
Forum and Reactions modules are enabled and the supplied topic/reply are visible:

```powershell
$env:RUSTOK_FORUM_TOPIC_REACTIONS_E2E_URL = "<full canonical Forum topic URL>"
$env:RUSTOK_FORUM_REPLY_REACTIONS_E2E_URL = "<same canonical topic URL>?reply=<reply-uuid>"
cargo test -p rustok-e2e-rust --test leptos_storefront_forum_reactions -- --nocapture
```

The Forum/Reactions harness asserts the host composition markers rendered by the
real storefront: the topic URL exposes exactly the topic reaction composition,
while the explicit reply selection exposes exactly the reply reaction
composition. It does not seed fixtures or bypass Forum/Reactions authorization.

If local Playwright browser installation is incomplete, set
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` for Node and Rust Playwright runs.
