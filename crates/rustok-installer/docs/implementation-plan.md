# Implementation Plan for `rustok-installer`

## Current state

`rustok-installer` owns the neutral install-plan, preflight, state-machine,
secret-reference, receipt, checksum, and seed-workflow contracts. Its
`InstallProfile` expresses frontend/build intent (`dev_local`, `monolith`,
`hybrid_admin`, and `headless_leptos`) and is not deployment
authority. `InstallTopology` records the selected role-to-surface assignment
and an optional host-only `InstallDistributionBinding` containing the exact
public preparation ID, distribution release ID, bundle-root digest, role-set
digest, and the exact role-to-artifact digest list. Every topology is invalid without that binding: a monolith is
the same role-bundle contract with one role, not a deployment shortcut.

Next.js is optional, external, and manually deployed, so it is not installer
topology or apply state. The former Next-specific profile and compatibility
alias have been removed from the canonical executable contract.

The canonical apply sequencing now runs in `rustok-installer`; `apps/server`
provides only HTTP composition for the setup surface. The shared
`rustok-installer-persistence` adapter owns SeaORM database, durable-state,
and bootstrap writer composition rather than server models or duplicated CLI
adapters. The
platform CLI has `install plan`, `install preflight`, `install apply`,
`install status`, and seed providers. The CLI uses the same state machine with
a shared SeaORM adapter; this is monolith bootstrap plumbing, not evidence
that production bundle deployment is implemented.

The independent Axum-to-`rustok-build` per-role activation adapter has been
removed with all repository-owned callers. The shared executor creates one
complete distribution deployment request and accepts one receipt containing
exact per-role observations. The server HTTP adapter now resolves a
host-selected release through the current admitted `rustok-modules` ledger.
Both monolith and distributed apply now report distribution deployment
unavailable until the desired/observed rollout controller is composed. The
standalone CLI fails closed at the same boundary.

The current plan consumes the one canonical instance placement owned by
`rustok-runtime`. It accepts one trusted
operator-selected instance root on any supported operating system, resolves a
relative input against the installer invocation directory, and derives one
portable relative layout. Its normalized physical path is restart/placement
evidence only and never release, module, migration, object, or cross-node
operation identity.

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

The installer must accept one canonical installation manifest that selects one
of the following topologies:

Before topology-specific work, the trusted local adapter binds the selected
instance root and derives `config`, `operations`, `releases`, `sources`,
`storage`, `data`, `state`, `work`, `cache`, `logs`, and `run` subtrees. The
default local/monolith installation is self-contained there. Distributed
adapters may map individual subtrees to external providers without changing
logical keys or lifecycle.
The CLI exposes it as local `--root <path>` install input; a remote wizard can
use only a root selected or allowlisted by its trusted host adapter.

| Topology | Result | Installation rule |
| --- | --- | --- |
| `monolith` | One immutable role bundle contains the monolith role, selected Leptos surfaces, generated registries, browser assets, and selected native module artifacts. | Consume the admitted bundle, pre-stage the monolith role, verify the pre-install recovery boundary, apply schema/seed/admin once, then start and observe the role. |
| `distributed` | One immutable role bundle contains the exact artifacts for selected roles such as `api`, `admin_ssr`, `storefront_ssr`, `worker`, and `registry`. | Consume the same admitted bundle, pre-stage every assignment, verify the pre-install recovery boundary, apply shared schema/seed/admin once, then start and observe every role without repeating database work. |

`rustok-distribution` selects artifacts, `rustok-build` constructs and validates
the canonical role plan, the static-distribution worker alone executes and
publishes it, and `rustok-modules` owns release admission, desired rollout,
observed rollout, and recovery. Production installer apply consumes one
owner-admitted complete bundle or the platform-signed base-distribution receipt
used only to bootstrap a fresh owner ledger; build/publication is a separate
preparation operation and cannot become an apply-time dependency. The trusted
host binds exact distribution release ID, digest-pinned OCI bundle reference
and root digest, and role-set digest before preflight; compile-time composition identity is only a
compatibility check. `rustok-installer` requests one complete distribution
deployment through a typed port; it must not invoke Cargo, compose host routers,
embed deployment-provider logic, or activate a release.
The outside deployment controller and node agent come from a separately
signed, digest-pinned operations-tool release installed by the host
provisioner/service supervisor. Installer preflight binds its exact
package/component/target digests and external protocol revision; candidate
roles and install apply never install or update those tools.
Module selection remains distinct from schema composition: the globally
composed `rustok-migrations::Migrator` is applied once and tenant enablement is
performed afterwards.

## Open results

1. **Complete the canonical placement and topology descriptor.**
   The shared installer input now carries one trusted operator-selected
   instance root and canonical relative layout. The host accepts Windows, Unix,
   and relative paths, normalizes them once, requires an absent/empty root or
   exact resumable marker/pending marker, rejects nonempty unmarked and nested
   roots, and keeps the physical path out of release/module/migration/object
   identity. `InstallTopology` defines
   role identifiers, role-to-surface assignments,
   composition revision/hash, and validation that every selected surface has
   exactly one owner. Map existing `InstallProfile` values into this descriptor
   without treating them as deployment topology aliases.
   External Next.js deployment is never represented as an installer role,
   surface, readiness check, or completion gate.
   Extend the canonical unversioned install plan in place so the trusted host
   binds the exact public `preparation_id`, owner distribution release,
   digest-pinned OCI bundle reference and root, and role-set digest from owner admission or a signed
   fresh-bootstrap receipt. Include
   them in preflight, checksum, deployment request, observations, and terminal
   receipt; a wizard-supplied value is ignored/rejected.
   The placement, descriptor, and trusted selected-distribution revision/hash
   binding are implemented. A plan represents monolith and distributed
   topology, serializes deterministically, and rejects duplicated/missing role
   ownership. The canonical exact-bundle type and role-set validation are
   implemented. HTTP clears any client-supplied binding and now resolves a
   host-selected release ID through the current admitted `rustok-modules`
   ledger, which returns the exact preparation, digest-pinned bundle reference,
   bundle-root, and role-set
   identity. Strict signed fresh-bootstrap receipt resolution is implemented
   for HTTP and CLI hosts: the signature, signer-key digest, validity interval,
   exact bundle identity, and host composition are checked before mutation,
   and ambiguous owner-ledger/receipt configuration is rejected. Import into
   the owner ledger is implemented. Role-bundle deployment convergence and
   bounded failed-install cleanup remain open; failed layout preparation currently preserves its exact marker
   for safe resume and never deletes the selected root.
   **Done when:** a trusted local plan can select any supported instance root,
   derive the same logical layout, represent monolith and distributed
   installations, serialize deterministically, and reject unsafe roots plus
   duplicated/missing role ownership without mixing physical paths into release
   identity. **Verification:** Windows/Unix/relative-root fixtures, independent
   multi-instance and exact-resume fixtures, failed-attempt cleanup proving the
   selected root and unrelated files survive, focused topology tests, and
   deterministic identity fixtures that remain equal across different physical
   roots.

2. **Extract the host-specific apply pipeline behind installer ports.**
   Add narrow ports for trusted instance-layout preparation, database
   readiness, schema application, tenant seed,
   admin provisioning, exact admitted/base-bundle resolution and revalidation,
   bootstrap-receipt import, and owner deployment hand-off. Build submission
   belongs to the separate release-preparation operation and is not an apply
   port. Move orchestration sequencing into this crate; keep SeaORM, Axum,
   background jobs, and credential resolution in adapters.
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

4. **Implement the shared admitted-bundle installation pipeline.**
   Use this pipeline for monolith and distributed topologies. Resolve and
   revalidate an owner-admitted bundle or exact platform-signed fresh-bootstrap
   receipt binding its public `preparation_id`, pre-stage all candidate-only
   role assignments, verify the
   empty/fresh target and pre-install recovery evidence, create the minimal
   owner schema, import/revalidate the bootstrap receipt into the sole
   `rustok-modules` ledger, then run the remaining exact migration plan once.
   Seed tenant/modules/admin once, start the pre-staged roles, observe complete
   convergence, and persist one receipt linking the install session to the
   immutable bundle. The pre-schema bootstrap journal and post-schema owner
   operation hand off idempotently under one fresh
   `install_transition_correlation_id`; the base bundle `preparation_id` is a
   read-only supply reference and is never reused for installer idempotency,
   logging, or correlation authority. Build or publication
   is not part of apply. Before mutation, verify the independently installed
   signed operations-tool release, exact controller/agent/target digests, and
   external protocol revision. Fail closed when it is missing, incompatible,
   or not locally recoverable. Subsequent tool upgrade is the
   `operations_tool_maintenance` class in the same canonical `rustok-modules`
   operation ledger and fleet fence; the host supervisor is only its narrow
   executor and retains the predecessor tools.
   The typed executor, HTTP adapter, CLI adapter, minimal owner migrator, and
   transactionally idempotent signed-receipt import are implemented. The
   remaining work in this item is pre-staging/recovery evidence, the durable
   desired/observed deployment hand-off, and crash-injection verification.
   **Done when:** a resumed apply is idempotent, verifies the observed bundle,
   uses bounded cleanup only before durable state, and otherwise reports the
   common recovery-required outcome with its restore action.
   `FreshInstallCleaned` is limited to pre-durable cleanup. Every failure after
   durable state uses `RecoveryRequired` plus an exact typed recovery action;
   no adapter may translate it into a successful rollback.
   **Verification:** targeted PostgreSQL monolith fixture plus crash injection
   before/after recovery-point, schema, seed, role start, and traffic switch;
   signed-tool digest/protocol mismatch denial; and a candidate-server-down
   resume using the independently installed controller/agent package.

5. **Complete multi-role topology in the shared installation pipeline.**
   The independent per-role build/release hand-off has been removed. The
   shared contract produces one immutable deployment descriptor for the exact
   admitted or fresh-bootstrap role bundle and waits for the owner-controlled
   desired/observed rollout to converge. It records one durable deployment
   receipt with per-role observations and validates exact role/surface coverage,
   artifact digests against the admitted per-role list, and health references. The exact owner release resolver
   is implemented; the controller and per-node convergence remain open. Do not
   expose an independent active-release mutation or release head through
   `rustok-build`, `apps/server`, HTTP, GraphQL, native, or CLI adapters.
   The schema, tenant seed, and admin provisioning stages run once for the
   shared database; node and role retries must not repeat them. Multi-role
   topology validation rejects a role claiming another role's surface. Static
   build inputs retain exact role runtime modes, generated registries, selected
   Leptos artifacts, and browser asset manifests. Standalone CLI remains
   explicitly unavailable until it receives the same deployment control
   adapter.
   **Done when:** an interrupted deployment resumes one fenced operation, all
   selected roles report the same admitted bundle and composition, and searches
   find no executable per-role active-release path.
   **Verification:** CI fixtures for monolith and at least `api`, `admin_ssr`,
   and `worker`, including crash/retry, partial-wave, base-bundle retention,
   candidate-only no-predecessor cleanup/resume, and idempotency assertions.

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
- Building, deploying, observing, or rolling back an optional Next.js host.

## Verification

- `cargo test -p rustok-installer --quiet`
- `node scripts/generate/generate-cli-registry.mjs --check`
- focused CLI, HTTP adapter, and topology-fixture checks in CI

Long workspace compilation is intentionally deferred to CI.
