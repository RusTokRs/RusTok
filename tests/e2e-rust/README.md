# Rust E2E Browser Tests

This crate contains Rust-owned browser smoke tests for Leptos/Trunk surfaces.

It is a Cargo workspace member and intentionally uses `playwright-rs` instead of
the Node Playwright runner. Next.js apps keep their own `@playwright/test`
configuration inside the app boundary.

Run:

```powershell
trunk serve --address 127.0.0.1 --port 8080
$env:RUSTOK_LEPTOS_ADMIN_E2E_URL = "http://127.0.0.1:8080"
cargo test -p rustok-e2e-rust --test leptos_admin_smoke -- --nocapture
```

If local Playwright browser installation is incomplete, set
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` for Node and Rust Playwright runs.
