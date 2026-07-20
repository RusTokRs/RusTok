# Implementation Plan for `rustok-installer`

## Current state

`rustok-installer` owns the neutral install-plan, preflight, state-machine,
secret-reference, receipt, checksum, and seed-workflow contracts. Its
`InstallProfile` currently expresses frontend/build intent (`dev_local`,
`monolith`, `hybrid_admin`, `headless_next`, and `headless_leptos`), but it does
not yet describe a deployable distributed topology.

The canonical apply sequencing now runs in `rustok-installer`; `apps/server`
provides only HTTP composition for the setup surface. The shared
`rustok-installer-persistence` adapter owns SeaORM database, durable-state,
and bootstrap writer composition rather than server models or duplicated CLI
adapters. The
platform CLI has `install plan`, `install preflight`, `install apply`,
`install status`, and seed providers. The CLI uses the same state machine with
a shared SeaORM adapter; this is monolith bootstrap plumbing, not evidence
that distributed installation is implemented.

## FFA/FBA boundary

- FFA status: `not_applicable`
- FBA status: `boundary_ready`
- Structural shape: `no_module_owned_ui`
- This crate owns policy and typed orchestration contracts only. `rustok-cli`
  owns terminal parsing and output; `apps/server` owns HTTP request handling;
  a wizard is a client of the same typed executor. Domain modules retain their
  own seed and lifecycle behavior.
- The feature-neutral surface is browser-safe and exposes the shared plan,
  state, receipt, preflight, deployment, secret, and executor contracts. The
  default `seed-runtime` feature owns native seed execution and its platform
  role dependency; browser clients disable it because they cannot execute seed
  owner ports.

## Target topology contract

The installer must accept one versioned installation manifest that selects one
of the following topologies:

| Topology | Result | Installation rule |
| --- | --- | --- |
| `monolith` | One selected server distribution contains API, operator surfaces and selected module artifacts. | Apply the schema once, then enable the selected modules for the bootstrap tenant. |
| `distributed` | A deployment descriptor assigns selected surfaces to independently built roles such as `api`, `admin_ssr`, `storefront_ssr`, `worker`, and `registry`. | Build each role from the same immutable composition revision; apply the shared schema once per database; do not repeat tenant seeding or migrations per role. |

`rustok-distribution` selects artifacts and `rustok-build` owns build/release
execution. `rustok-installer` must request those capabilities through typed
ports; it must not invoke Cargo, compose host routers, or embed deployment
provider logic. Module selection remains distinct from schema composition:
the globally composed `rustok-migrations::Migrator` is applied once and tenant
enablement is performed afterwards.

## Open results

1. **Add a versioned topology descriptor to the install plan.**
   Define `InstallTopology`, role identifiers, role-to-surface assignments,
   composition revision/hash, and validation that every selected surface has
   exactly one owner. Map existing `InstallProfile` values into this descriptor
   without treating them as deployment topology aliases.
   The descriptor and trusted selected-distribution revision/hash binding are
   implemented. A plan represents monolith and distributed topology, serializes
   deterministically, and rejects duplicated/missing role ownership. Distributed
   apply remains explicitly unavailable until a deployment adapter is added.
   **Done when:** a plan can represent monolith and distributed installations,
   serialize deterministically, and rejects duplicated/missing role ownership.
   **Verification:** focused installer contract tests and a deterministic plan
   checksum fixture.

2. **Extract the host-specific apply pipeline behind installer ports.**
   Add narrow ports for database readiness, schema application, tenant seed,
   admin provisioning, distribution planning, build submission, and deployment
   hand-off. Move orchestration sequencing into this crate; keep SeaORM,
   Axum, background jobs, and credential resolution in adapters.
   The durable session/receipt adapter is already outside the HTTP host in
   `rustok-installer-persistence`. Typed database, schema, persistence, seed,
   admin and verification port contracts now live in `rustok-installer`; the
   server HTTP adapter invokes the shared state machine. **Done when:**
   server HTTP and CLI adapters invoke one executor and no server-local install
   state machine remains.
   **Verification:** installer sequencing tests using fakes plus adapter
   contract tests.

3. **Register the platform CLI install provider.**
   `rustok-cli install plan|preflight|apply|status` is registered through the
   generated CLI registry. The provider renders structured output and uses the
   same executor as the HTTP adapter; apply opens the target database itself so
   it can create it when requested.
   **Done when:** no `rustok-server install ...` parser or command path exists.
   **Verification:** CLI registry generation check and focused provider tests.

4. **Implement monolith installation.**
   Resolve the selected distribution revision, prepare one database, run the
   global migrator once, seed the tenant/modules/admin once, and persist a
   receipt linking the install session to the immutable composition revision.
   **Done when:** a resumed monolith apply is idempotent and verifies the
   running distribution against the recorded revision.
   **Verification:** targeted PostgreSQL integration scenario in CI.

5. **Implement distributed installation.**
   Produce one immutable deployment descriptor, submit role-specific builds via
   `rustok-build`, wait for the selected deployment adapter, and record
   per-role receipts. The schema, tenant seed, and admin provisioning stages
   run once for the shared database; role retries must not repeat them.
   `InstallDeploymentPort`, `InstallRoleDeploymentRequest`, and
   `InstallRoleDeployment` establish the neutral hand-off and deterministic
   per-role request ordering. `execute_distributed_role_deployments` now moves
   the session through `deploying`, validates the active release against each
   immutable role request, and records one durable `deploy` receipt per role.
   The server host adapter now maps the hand-off to `rustok-build`, executes
   the selected role plan, waits for an active release, and composes the helper
   into the full state machine when `rustok.build.enabled=true`. Distributed
   topology validation also rejects a role claiming a
   surface owned by a different process role. `rustok-build::BuildRuntimeMode`
   and `RoleBuildPlan` now persist role lifecycle intent with the immutable
   build plan; the server manifest composer derives role-specific compiled
   surfaces, and filesystem, HTTP, and container release hand-offs carry
   `RUSTOK_RUNTIME_HOST_MODE` onward. Standalone CLI remains explicitly
   unavailable until it receives an equivalent deployment adapter.
   **Done when:** an interrupted role deployment resumes from its own receipt
   and all deployed roles report the same composition revision.
   **Verification:** CI topology fixture with at least `api`, `admin_ssr`, and
   `worker`, including retry/idempotency assertions.

6. **Expose the wizard only as a typed client.**
   Keep `/api/install/*` as a thin authenticated/setup-token-protected adapter
   with durable job/receipt reads. It may select a topology but may not define
   install sequencing or deployment policy locally.
   **Done when:** browser and CLI requests yield the same redacted plan and
   receipt semantics for an identical manifest.
   **Verification:** HTTP adapter tests and client contract fixtures.

## Non-goals

- Per-module physical schema exclusion from the globally composed migrator.
- Running Cargo, Docker, Kubernetes, or cloud SDK commands from this crate.
- A separate installer implementation in the server, web wizard, or `xtask`.
- Repeating migrations, seed, or admin provisioning for every distributed role.

## Verification

- `cargo test -p rustok-installer --quiet`
- `node scripts/generate/generate-cli-registry.mjs --check`
- focused CLI, HTTP adapter, and topology-fixture checks in CI

Long workspace compilation is intentionally deferred to CI.
