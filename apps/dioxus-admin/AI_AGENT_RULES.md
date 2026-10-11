# AI Agent Rules for `apps/dioxus-admin`

> [!CAUTION]
> **DORMANT CONCEPT / PROTOTYPE SCAFFOLDING (PHASE E)**
> 
> This package is **DORMANT** until domain modules implement their `ui/dioxus.rs` render adapters.

## Mandatory Rules for AI Agents

1. **DO NOT Run Workspace Test Sweeps or Audits Here:**
   - Do NOT run `cargo test`, `cargo clippy`, or audit scripts on this package during repository audits or full-stack review sweeps.
   - This package is deliberately excluded from `Cargo.toml` `[workspace.members]`.
   
2. **DO NOT Add to Workspace Members:**
   - Under NO circumstances should `apps/dioxus-admin` be added to the root `Cargo.toml` `[workspace.members]` without an explicit ADR and maintainer sign-off.

3. **DO NOT Implement Domain Logic or Components Here:**
   - All domain UI views (catalog, orders, pages, settings) belong in `crates/modules/*/admin/src/ui/dioxus.rs`.
   - All shared UI controls (buttons, inputs, grids) belong in `crates/ui/rustok-ui/dioxus` and `crates/ui/rustok-grid/dioxus`.
   - Host applications are strictly mounting/wiring shells, never domain logic owners.

4. **DO NOT Refactor or Create Parallel Abstractions:**
   - Leave this package as a dormant runner until Phase E pilot tasks are explicitly assigned by the maintainer.
