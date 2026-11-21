# rustok-modules architecture guide

A map for new developers. `CRATE_API.md` is the formal contract; the crate
`README.md` and `docs/README.md` carry operational detail. This file explains
how the pieces fit together and where to start reading.

## What this crate is

The platform control plane for third-party and first-party modules: artifact
identity, marketplace publication, admission, installation, activation,
tenant enablement policy, static distribution, and the data plane for
module-owned tenant data. It is deliberately transport-neutral: no Axum,
GraphQL, or Leptos dependencies. Hosts (the server, workers) compose one
`ModuleControlPlane` per process/database and derive owner services from it.

Everything durable follows the same command discipline:

- **Tenant-scoped.** Every owner row and command carries `tenant_id`;
  transactions set the tenant scope (`configure_tenant_scope`) before work.
- **Idempotent with a semantic fingerprint.** A retry is identified by
  (scope, idempotency key, hash of the command's semantic fields). Reusing a
  key with a different body is a conflict, never a silent second execution.
  Trace/correlation/reason text is audit evidence, not identity
  (`src/idempotency.rs`; see "Durable replay identity" in `docs/README.md`).
- **Revision-guarded.** Mutations CAS on an expected revision; a lost race is
  a typed conflict, not a partial write.
- **Fail-closed.** Missing owner state, undecodable evidence, or an
  unavailable policy owner denies the operation. Nothing falls back to a
  permissive default.

## Subsystem map

The crate root re-exports ~1000 names for workspace convenience; the
supported surface is `ModuleControlPlane` and the services derived from it
(`src/control_plane.rs`). Grouped by concern:

| Area | Key files | Owns |
| --- | --- | --- |
| Lifecycle & policy | `lifecycle_writer.rs`, `policy.rs`, `policy_cache.rs`, `security_epoch.rs`, `transition_*`, `recovery.rs` | Tenant enable/disable overrides, effective availability decisions, transition checkpoints, rollback/recovery |
| Installation | `installation.rs`, `installation_selection.rs`, `artifact.rs`, `artifact_cas.rs`, `artifact_schema.rs` | Admitted installations, activation/deactivation, CAS byte storage under verified SHA-256 digests |
| Admission & trust | `oci_admission.rs`, `oci.rs`, `external_prebuilt_ingress.rs`, `trust.rs`, `publication_evidence.rs`, `publish_validation.rs` | Digest-pinned OCI admission, signature/SBOM/provenance evidence, trust-policy decisions |
| Governance | `src/governance/` | Marketplace publish pipeline: requests, validation stages, evidence, approvals, holds |
| Distribution | `distribution*.rs`, `build.rs`, `promotion.rs`, `static_package.rs` | Static-promotion builds, worker leasing, rollout with per-(node, role) convergence |
| Execution | `executor.rs`, `dispatcher.rs`, `artifact_capability_router.rs`, `capability_http.rs`, `capability_events.rs`, `mcp.rs` | Sandbox dispatch of admitted bindings; brokered capabilities (`platform.http`, `platform.mcp`, events) |
| Node fleet | `artifact_node_reconciliation.rs`, `executor_pool_registry.rs`, `executor_readiness.rs` | Node-agent claims/heartbeats, topology convergence, readiness gates |
| Settings | `settings*.rs`, `artifact_settings*.rs`, `static_settings_*.rs`, `translation.rs` | Per-tenant module settings with schema validation, CAS, exact-locale reads |
| Secrets | `secrets.rs` | Secret *references* only (resolver alias/key); values are resolved by host adapters and never stored |
| Data plane | `src/data/`, `data_*.rs` | Module-owned tenant data: chunked uploads, snapshots, export, purge, GC |
| Durable work | `operation_store.rs`, `event_delivery.rs`, `schedule_delivery.rs`, `queue_drain.rs`, `delivery_lease.rs`, `claim_heartbeat.rs` | Operation journal, delivery queues with claim tokens, leases, dead-letter budgets |
| Retention | `retention.rs`, `gc.rs` | Retention holds, tombstone/grace/final-recheck garbage collection |

## The four command flows to understand first

1. **Tenant toggles a module** (GraphQL `toggleModule` in the server).
   Host checks `modules:manage` via RBAC, then
   `ModuleLifecycleService::toggle_module` builds a `ModuleControlPlane`
   lifecycle writer. `validate_module_toggle` (in `policy.rs`) rejects core
   modules, missing/incompatible dependencies, and disabling a module with
   enabled dependents. The write is one revision-CAS operation with an
   idempotency receipt; the effective-policy cache for the tenant is
   invalidated by the host afterwards.

2. **A release is published** (registry HTTP surface in the server).
   Authenticated user session required (OAuth service tokens are rejected);
   third-party and external-prebuilt requests additionally need
   `modules.manage`. The governance pipeline records validation stages and
   evidence; marketplace approval independence is re-checked at publication
   time against the final publisher principal, and an author signature must
   exist. Claiming `first_party` ownership in the request confers nothing by
   itself.

3. **An artifact is admitted and activated.**
   OCI references must be digest-pinned (mutable tags are rejected).
   Admission verifies trust evidence via the host-supplied `TrustVerifier`,
   stages bytes into CAS under their verified digest, and journals the
   admission intent. Activation runs inside a fenced transaction under the
   installation scope (platform or tenant), creating retention holds for the
   predecessor so rollback stays possible.

4. **A guest module executes.**
   Dispatch (`dispatcher.rs`) never trusts the caller: it re-checks that the
   admission matches the exact release, binding, installation target,
   tenant, and execution phase. Capabilities are brokered per installation —
   e.g. `platform.http` resolves the grant for the exact
   (installation, tenant, release digest), then applies SSRF rules: private,
   loopback, link-local, CGNAT, and metadata ranges are denied, including
   IPv4 addresses embedded in NAT64/6to4/IPv4-mapped IPv6 literals; DNS is
   resolved once and the validated address is pinned into the HTTP client
   so the connection cannot be rebound; redirects are disabled and
   hop-by-hop headers are rejected.

## Invariants worth knowing before you change anything

- **Secrets are references.** The bindings table stores resolver
  alias/key pairs. If you find yourself persisting a secret value, you are
  in the wrong layer — resolution belongs to host-owned adapters.
- **Policy denial is not a delivery failure.** Event/schedule deliveries
  denied by policy are parked and retried after re-enable; only genuine
  execution failures spend the dead-letter budget (`max_attempts`).
- **Expired lease ≠ stopped work.** Journal and delivery claims remain
  fences after expiry until explicitly reconciled; completions require the
  exact claim token and a live lease by the database clock.
- **GC never deletes on suspicion.** Collection requires tombstone, grace
  period, a final recheck, and zero active holds in the retention ledger.
- **SQLite tenant identity is canonicalized.** Tenant UUIDs exist in native
  and text representations; owner statements resolve the unique parent key
  inside the statement and reject mixed or orphaned identities rather than
  falling back. PostgreSQL uses RLS with the transaction tenant scope.
- **Static settings settle before lifecycle.** Settings writes claim the
  aggregate under their own receipt; a database error mid-claim leaves the
  receipt in `processing` so the idempotency key cannot be reused by a
  different request. See the static settings boundary notes in
  `docs/README.md`.

## Reading order

1. `src/control_plane.rs` — the facade and how services are derived.
2. `src/policy.rs` — effective availability: the decision model everything
   else consumes.
3. `src/lifecycle_writer.rs` — the toggle write path end to end.
4. `src/installation.rs` (skim by operation) — admission → activation →
   rollback against CAS.
5. `src/dispatcher.rs` + `src/capability_http.rs` — how guest execution is
   admitted and brokered.
6. `src/governance/` — only if you touch marketplace publishing.
7. `docs/README.md` — operational caveats (worker upgrade migrations,
   retention worker, delivery parking) before changing durable behavior.

## Verification

- `cargo xtask module validate modules`
- `cargo test -p rustok-modules`
- `cargo check -p rustok-server --lib`
