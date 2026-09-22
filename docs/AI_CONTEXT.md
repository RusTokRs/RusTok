---
id: doc://docs/AI_CONTEXT.md
doc_type: current_contract
status: current
owner: platform-architecture
canonical_for:
  - ai-bootstrap-context
language: markdown
---

# AI Context for RusToK

This document is the mandatory bootstrap navigation and architectural invariant brief for AI sessions working in the RusToK repository.

---

## 1. Authority Hierarchy & Mandatory Reading Order

Before proposing, modifying, or deleting code or documentation, follow this order:

1. [`AGENTS.md`](../AGENTS.md) — Canonical contributor & AI-agent governance contract (source of truth, zero-legacy, concurrency, verification).
2. [`ARCHITECTURE.md`](../ARCHITECTURE.md) — Current-state architectural quick reference & platform shape.
3. [`docs/index.md`](./index.md) — Canonical platform documentation map.
4. Target component's `README.md` and local `docs/README.md` / `docs/implementation-plan.md`.
5. Active ADRs in [`DECISIONS/`](../DECISIONS/README.md) governing the affected boundary.

---

## 2. Composition Dimensions & Module Model

Do not collapse these independent architectural axes into a single label:

| Axis | Meaning | Allowed States / Forms |
| :--- | :--- | :--- |
| **Architectural role** | Functional classification in the platform | Domain module, capability extension, shared library, host application, worker |
| **Tenant lifecycle** | How a tenant interacts with the module | `Core` (required, unconditionally active for all tenants) or `Optional` (tenant-managed, enabled/disabled via `rustok-tenant`) |
| **Runtime composition** | How the host composition root links and runs the component | `runtime = "module"` (participates in tenant `ModuleRegistry`) or `runtime = "extension"` (deployment-scoped capability contribution) |
| **Packaging** | Physical packaging in source and distribution | Crate, package, binary, generated artifact |

### Critical Rules on Composition

- **`runtime = "extension"`** (e.g., `rustok-ai`, `rustok-iggy-connector`) is a deployment-scoped capability contribution composed into the host runtime globally when compiled. It is **not** a third tenant `ModuleKind` and is never tenant-toggled.
- **Composition Source of Truth:** [`modules.toml`](../modules.toml) is the sole source of truth for platform composition. Do not hardcode static module lists in documentation or prompts. Verify composition with `cargo xtask validate-manifest`.
- **Not every crate is a module:** Crates in `crates/libs/*`, `crates/ui/*`, `crates/utils/*`, and `crates/workers/*` are shared libraries, tooling, or worker runtimes, not platform modules.

---

## 3. Core Platform Invariants

1. **One Canonical Owner & Source of Truth:**
   Every domain concept has exactly one canonical owner. Other representations are projections, adapters, overlays, caches, indexes, or transport DTOs. Embedded execution does not permit consumers to query another module's private tables or bypass port contracts.
2. **Atomic Write-Side & Transactional Outbox:**
   Any domain write that produces domain events MUST commit atomically with the outbox table (`sys_events`) using `publish_in_tx`. Asynchronous consumers or indexers update read models and projections from the outbox stream.
3. **Typed Context Propagation:**
   Tenant (`tenant_id`), actor identity, trace identifiers (`correlation_id`, `causation_id`), and execution deadlines are resolved by their canonical owner and passed via typed `PortContext`. Caller-supplied identity headers (e.g. `X-User-ID`) are untrusted and must be verified.
4. **UI Ownership & Framework Boundaries (FFA):**
   - Leptos UI surfaces are owned by their respective modules (`admin/` and `storefront/` sub-crates) and compile into Leptos host applications.
   - Internal SSR/hydrate UI default data layer uses native `#[server]` functions.
   - Headless GraphQL and REST endpoints are parallel public contracts for external consumers, Next.js, and mobile clients.
   - Do not implement new Leptos UI as GraphQL-only when a native `#[server]` function is applicable.
5. **Zero-Legacy Policy Before Release:**
   Internal repository code has no legacy consumers that justify compatibility layers. Changes must atomically update all callers, transports, schemas, tests, and documentation, and delete superseded code in the same change.

---

## 4. Operational Cautions for Agents

### False Compilation Errors with `AdminAssets`

When running `cargo check -p rustok-server` without a pre-built admin frontend, you may encounter:

```text
error: #[derive(RustEmbed)] folder 'apps/admin/dist' does not exist
error[E0599]: no function or associated item named `get` found for struct `AdminAssets`
```

**This is not a bug in server code.** It is the expected behavior of the `embed-admin-assets` build feature when frontend artifacts have not been built (`trunk build` or `npm run build`). In development/CI checks where frontend artifacts are absent, this feature is disabled by default. Inspect only errors in your changed files.

### Asynchronous Lifecycle & Logging

- Long-running or background tasks must use explicit Tokio lifecycle handles or outbox workers.
- Production code must use the structured platform telemetry facilities (`tracing::info!`, `tracing::error!`). Commit no ad-hoc `println!` or `dbg!` diagnostics in runtime code.

### Canonical Verification Commands

Before declaring completion, run the canonical verification checks for the affected boundary:

- **Manifest & Module Contracts:** `cargo xtask validate-manifest` and `cargo xtask module validate <slug>`
- **Documentation System:** `npm run verify:docs`
- **ADR Governance:** `npm run verify:adrs`
- **Architecture Boundaries:** `python scripts/architecture_dependency_guard.py`
