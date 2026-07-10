# rustok-seo-render documentation

`rustok-seo-render` converts canonical `SeoPageContext` into deterministic Rust
SSR head HTML. It owns rendering only: SEO resolution, tenant policy, storage,
and Next Metadata mapping remain outside this crate.

`apps/storefront` is the Rust-host consumer. Cross-host semantic parity and
renderer safety work are recorded in the
[implementation plan](./implementation-plan.md).
