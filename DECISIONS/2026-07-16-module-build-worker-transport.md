# Module Build Worker Transport

- Date: 2026-07-16
- Amended: 2026-08-09
- Status: Accepted

## Context

`rustok-modules` already owns immutable module build requests and results, a
tenant-RLS durable queue, and transactional outbox facts. It must dispatch an
untrusted Rust source build without allowing `apps/server` or the runtime
sandbox to invoke Cargo. The worker needs a deployment-ready protocol with
authenticated readiness and no accidental in-process fallback.

## Decision

Use a dedicated `rustok-module-build-transport` support crate. It maps the
owner-owned `ModuleBuildWorker` port onto the single current tonic gRPC service
whose
bodies are canonical JSON serializations of `ModuleBuildRequest` and
`ModuleBuildResult`. `rustok-worker-transport` owns the shared mTLS listener
baseline used by both verification and build workers. Production callers use
mTLS and the same authenticated listener exposes readiness. The transport
contains no build policy, source/CAS access, database access, result
persistence, or Cargo execution.

`rustok-module-build-worker` is the separate process/OCI-job boundary. It
accepts only the immutable request, invokes a fixed image-owned job runner with
a cleared environment, enforces the request deadline/output cap, and returns a
validated terminal result. Before becoming ready, the worker must load a
bounded deployment-owned isolation attestation that matches the selected
runtime and digest-pinned image and records the required unprivileged,
host-isolated, resource-limited, ephemeral-job controls. This attestation is
configuration-review evidence; deployment still has to demonstrate that the
launcher enforces the corresponding OCI controls. `rustok-modules` validates
and persists that result against the queued request. Transport or worker
failure has no server-local fallback.

The shared worker transport owns one process-wide bounded admission semaphore,
not merely tonic's per-connection limit. Expensive verification and build RPCs
must acquire it with a bounded wait; readiness remains permit-free so saturation
is observable. Worker hosts use the shared SIGTERM/Ctrl+C future with tonic
graceful shutdown, and every external subprocess uses kill-on-drop semantics so
cancellation cannot orphan tool execution.

The build worker has no attestation-free construction path. Its bounded
isolation-attestation schema rejects unknown fields, and the worker reloads the
deployment-owned file through the readiness gate before each execution. This
prevents a transport caller from bypassing startup evidence or continuing after
the mounted evidence is revoked; it still does not replace runtime deployment
proof from the gVisor/Kata job supervisor.

The source archive carries `module-artifact.json`, an author-owned descriptor
declaration without the build-derived component digest. The worker validates
and binds this declaration to the immutable request before launching author
code. After independently inspecting the fixed Component and its extracted WIT
surface, the worker inserts the verified component digest and creates the final
`module-artifact-descriptor.json` exactly once. Runner-provided final
descriptors are rejected, so untrusted build output cannot select the payload
identity that publication admits.

The control plane is the sole writer of the current deployment-mounted source
CAS. `CasArchivePublisher` strictly scans a deterministic archive, copies and
rehashes it privately, and uses an atomic no-replace commit before the
digest-addressed object becomes visible. Existing objects are rehashed and
rescanned. The authoring owner then constructs the complete immutable build
request with owner-selected limits, dependency egress, validation profiles,
WIT, ABI, and target policy and commits it through the durable queue. The
worker receives the same CAS root as a read-only deployment mount and remains
unable to publish or mutate source objects.

The accepted release-safety target narrows and replaces that current writer
boundary atomically. `rustok-modules` preparation owns the single unversioned
`SourceObjectStore` port, authenticated owner-domain publication, globally
deduplicated create-only `source_digest` blob, separate RLS-scoped
`source_receipt_id` over owner/preparation/media-type/length/manifest,
same-request idempotency, and all-reference retention/collection authority for
platform, native, WASM, and reviewed-Rhai source
objects. `CasArchivePublisher` becomes an archive-specialized client/codec path
and loses direct CAS layout/writer authority; reviewed Rhai bounded-workspace
objects use the same owner without tar wrapping. Every repository caller and
worker mount is cut over together, with no dual writer or old-layout fallback.

`rustok-cli module build` is an owner client for this path. It requires explicit
tenant, actor, project, trace, correlation, and idempotency identity, but it
cannot select worker policy or invoke the worker, registry, or signing service.
Its local source path is packaged into a private deterministic archive and
deleted after the owner accepts or rejects the submission.

`rustok-cli module publish` is the matching owner client for release staging,
not a final publication authority. It constructs a bounded metadata bundle from
the current source descriptor plus `Cargo.toml`. The owner reloads the exact
tenant-scoped completed build, validates and stores that bundle by its own
SHA-256, creates an immutable deterministic publish request, binds the build's
source, Component, and OCI receipt identities, and queues registry validation.
The metadata-bundle digest is never compared with or substituted for the
Component payload digest or OCI manifest digest. Build-service attestation,
platform admission, marketplace approval, and final release creation remain
reserved owner operations outside the author CLI.

Production publication evidence is composed by the independent
`rustok-registry-validation-worker`, not by `apps/server`. The worker reloads the
exact completed build and current publication stage through the owner, obtains a
short-lived registry credential lease from a deployment-owned broker, fetches
and revalidates the digest-pinned OCI package, and calls the isolated trust
verifier through readiness-gated mTLS. It records the build-service attestation
and platform-admission facts only through owner operations. This keeps registry
credentials and trust roots outside the server, Alloy, MCP, AI, and module
runtime while preserving one canonical publication authority.

Rust components use the current native Rust Component Model path: pinned Cargo
builds the SDK-generated guest against `wasm32-wasip2`, and `wasm-tools`
performs post-build inspection. `cargo-component` is not a parallel or fallback
build path. This follows the current Bytecode Alliance guidance that native
Rust tooling emits WASI P2 components directly.

The delivery host is a separate broker consumer. It owns broker
acknowledgement and the database connection required to call the owner delivery
service; it does not execute Cargo or join the worker process. It invokes the
build worker only through a mutually authenticated client and leaves failed
deliveries unacknowledged for broker retry. The Iggy adapter consumes the
dedicated `module-build` topic through one persistent remote consumer-group
cursor and commits an offset only after the owner result persistence succeeds.
Broker topology provisioning and deployment of the delivery host remain
operational requirements.

## Consequences

- The owner protocol remains independent of tonic and worker runtime details.
- The protobuf package has no generation suffix or compatibility service; a
  contract change replaces callers and workers atomically in this initial
  implementation.
- A worker binary can be deployed and supervised independently of the server.
- Author source declares policy and bindings, while only the worker can bind
  the final descriptor to the verified executable digest.
- SDK and template releases remain independently versioned provenance inputs;
  neither duplicates the canonical WIT source.
- Verification and build workers share one mTLS listener implementation rather
  than drifting into separate TLS/limit defaults.
- The delivery host must not compete with the global outbox relay or
  acknowledge another consumer's event stream position.
- The `module-build` topic must be provisioned before the dispatcher starts;
  unexpected or malformed queue payloads remain unacknowledged and require
operator remediation rather than being silently skipped.

The same support crate also maps the owner-owned
`ModuleStaticDistributionExecutor` port onto a separate current-only
`rustok.static_distribution` service. Static-distribution queue ownership,
lease renewal, and terminal persistence remain in `rustok-modules`; the remote
service receives only an already claimed immutable work item and returns one
terminal outcome. Its client exposes only the mTLS connection constructor.
- The legacy server-local `rustok-build` executor cannot be retained as a
  fallback; its removal follows when remote dispatch and the worker deployment
  are wired.
- Build execution and OCI-job isolation remain worker-owned follow-up work;
  readiness attestation is required before deployment evidence can close that
  work.
