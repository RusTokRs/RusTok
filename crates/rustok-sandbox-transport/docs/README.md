# Sandbox worker transport

## Contract

`SandboxWorkerService.Execute` is a bidirectional tonic stream. The host sends
one initial request, zero or more capability results, and an optional
cancellation frame. The worker sends zero or more capability requests followed
by exactly one outcome or error frame.

Every frame carries the exact protocol revision and execution UUID. A revision
mismatch, execution mismatch, duplicate request, empty frame, unknown
capability call ID, malformed payload, unsolicited frame, or premature stream
closure fails the execution. The transport never retries through an in-process
executor.

The independently deployed wire revision is an external boundary only. Both
peers immediately map it to the single current unversioned Rust sandbox model;
there is no parallel internal protocol family.

## Payload Encoding

The initial request separates the JSON metadata from `payload.bytes`. This is
required because the current sandbox model contains `serde_json::Value`, while
artifact bytes can reach the admitted 64 MiB limit and must remain native
protobuf bytes rather than a JSON integer array or base64 value. The worker
rejects metadata that also contains artifact bytes.

Capability calls, responses, outcomes, and typed `SandboxError` values use
bounded JSON inside protobuf byte fields. The common 72 MiB message ceiling
covers the artifact limit plus framing overhead and remains below the shared
128 MiB absolute worker-transport ceiling.

## Capability Boundary

The worker receives no network, database, object-store, secret, MCP, or module
clients. Its neutral `RhaiCapabilityBridge` sends a typed `CapabilityCall` to
the host stream. The worker-side sandbox validates the request policy before
sending it, and the original host `SandboxHost` rechecks execution identity,
grants, constraints, budgets, audit, and cancellation before invoking the
owner-composed broker.

## TLS and Failure Semantics

Production callers can construct `GrpcRhaiExecutor` only through
`connect_with_tls`. The server must perform `check_readiness` before registering
the adapter as `isolated_worker`. Worker execution also rechecks readiness and
the request limits against its deployment isolation envelope. Connection loss,
worker exit, deadline, cancellation, or protocol failure is terminal for the
request and cannot activate a local fallback.

Plaintext channels are available only to crate-local loopback tests.

