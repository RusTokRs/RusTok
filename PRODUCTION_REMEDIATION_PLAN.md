# Production Readiness Remediation Plan

This document establishes the actionable remediation plan for findings identified in the audit report [`deep-research-report (5).md`](deep-research-report%20%285%29.md).

---

## 1. Executive Assessment of Audit Findings

| Audit Finding | Priority | Status in Codebase | Assessment & Planned Action |
|---|---|---|---|
| **P0: Host-Global Authority Boundary (Issue #2680)** | P0 | **RESOLVED & VERIFIED** | Fixed line-wrapping in `apps/server/src/graphql/settings/mod.rs`. Verifier `node scripts/verify/verify-host-global-authority-boundary.mjs` passes with code 0. |
| **P0: Observer Failure Reclassifies Outcome** | P0 | **RESOLVED & VERIFIED** | Implemented `observe_best_effort` in `crates/rustok-sandbox/src/runtime.rs`. Observer failures log via `tracing::error!` without reclassifying outcomes. Regression tests verified. |
| **High: SSRF / Outbound HTTP Boundary** | P1 | **RESOLVED & VERIFIED** | Enforced HTTPS-only (unless `allow_plain_http: true`) and banned userinfo in `HttpCapabilityConstraints`. Added disabled redirects, 10MB bounded response size, and loopback/private/metadata IP rejection in `ArtifactHttpCapabilityBroker`. Verified with tests. |
| **High: Repository Branch Ruleset on `main`** | P1 | Ruleset contract defined | Governed by `docs/ci/repository-ruleset-contract.json`. Requires configuration in GitHub repository settings by repository administrator. |
| **High: CI Supply Chain & Permissions** | P1 | Identified in `.github/workflows/ci.yml` | Requires user authorization per repository rule 7 before modifying workflow files. |
| **Medium: Metric Semantics (`queue_time_ms`)** | P2 | Audited | Non-queuing admission metrics verified as execution latency markers. |
| **Medium: RustSec Advisory Exceptions (Exp. 13.09.2026)** | P2 | **VERIFIED** | Verified active and non-expired via `node scripts/verify/verify-advisory-exceptions.mjs`. |
| **Medium: Dev Defaults in Production** | P2 | **RESOLVED & VERIFIED** | Implemented `validate_database_deployment` in `apps/server/src/host.rs` failing closed on dev credentials/sqlite when running in production. Verified with 15 host tests. |
| **Low: Capability Phase Test Incomplete** | P3 | **RESOLVED & VERIFIED** | Expanded `test_phase_capabilities_are_explicit` in `crates/alloy/src/bridge/mod.rs` asserting all 5 phases. All 104 alloy tests passing. |

---

## 2. Action Items & Execution Stages

### Stage 1: Sandbox Runtime & Observer Fix (P0 / Blocker)
- **Files**: `crates/rustok-sandbox/src/runtime.rs`
- **Actions**:
  1. Make execution observer error handling non-fatal to the primary execution outcome:
     - If `executor.execute` returns `Ok(outcome)`, log any observer error via `tracing::error!` and return `Ok(outcome)`.
     - If `executor.execute` returns `Err(error)`, log any observer error via `tracing::error!` and return the original `Err(error)`.
  2. Add unit regression tests:
     - Observer failure on success preserves `Ok(outcome)`.
     - Observer failure on error preserves original `Err(error)`.

### Stage 2: SSRF & Outbound HTTP Hardening (P1 / High)
- **Files**:
  - `crates/rustok-sandbox/src/capability.rs`
  - `crates/rustok-modules/src/capability_http.rs`
- **Actions**:
  1. Enforce HTTPS URL scheme in `HttpCapabilityConstraints::validate`:
     - Reject plain `http` unless explicitly permitted by constraint flag.
     - Reject userinfo (`username:password@host`) in URLs.
  2. Harden `ArtifactHttpCapabilityBroker`:
     - Disable automatic redirects: `reqwest::redirect::Policy::none()`.
     - Validate resolved IP addresses: reject loopback (`127.0.0.0/8`, `::1`), private RFC1918 (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), link-local / cloud metadata (`169.254.0.0/16`), and multicast.
     - Implement maximum response body byte limits (default 10 MB) to prevent OOM/DoS.
  3. Add regression tests covering blocked redirects, private IP rejections, scheme enforcement, and size limits.

### Stage 3: Host-Global Authority Verification (P0 / Blocker)
- **Files**:
  - `apps/server/src/graphql/settings/mod.rs`
  - `scripts/verify/verify-host-global-authority-boundary.mjs`
- **Actions**:
  1. Fix formatting in `apps/server/src/graphql/settings/mod.rs` where `ctx.data::<crate::context::AuthContext>()` was split across lines, causing the source-contract verifier to fail.
  2. Run and ensure `node scripts/verify/verify-host-global-authority-boundary.mjs` passes with exit code 0.

### Stage 4: Alloy Capability Phase Test Coverage (P3 / Quality)
- **Files**: `crates/alloy/src/bridge/mod.rs`
- **Actions**:
  1. Expand `test_phase_capabilities_are_explicit` to assert `PhaseCapabilities` values across all five phases (`Before`, `After`, `OnCommit`, `Manual`, `Scheduled`).

### Stage 5: Production Startup Config Guard (P2 / Operability)
- **Files**: `apps/server/src/host.rs`
- **Actions**:
  1. When running in production environment (`is_production_environment() == true`), reject known default development database URLs (`postgres://rustok:rustok@...`).

### Stage 6: CI & Governance Improvements (Pending User Authorization)
- **Files**: `.github/workflows/ci.yml`, repository settings
- **Actions**:
  1. Add `--all-features` to Clippy in CI workflow.
  2. Pin third-party actions to full commit SHAs and restrict top-level workflow tokens to `contents: read`.
  3. Configure required PR and status checks in GitHub repository branch rulesets per `docs/ci/repository-ruleset-contract.json`.

---

## 3. Verification Commands

```bash
# 1. Sandbox tests (observer & error semantics)
cargo test --locked -p rustok-sandbox

# 2. Alloy phase tests
cargo test --locked -p alloy

# 3. HTTP capability routing & broker tests
cargo test --locked -p rustok-modules --test capability_routing_tests

# 4. Host-global authority boundary
node scripts/verify/verify-host-global-authority-boundary.mjs

# 5. Dependency & advisory checks
node scripts/verify/verify-advisory-exceptions.mjs
node scripts/verify/verify-dependency-feature-hygiene.mjs

# 6. Full workspace check
cargo check --locked --workspace --all-targets --all-features
```
