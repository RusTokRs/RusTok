# rustok-worker-transport

This infrastructure crate owns shared mutually authenticated tonic listener and
client configuration for isolated RusToK workers. It loads mounted server/client
identity and trust material, applies bounded listener limits, and never exposes
a plaintext listener.

The crate also owns the process-wide `WorkerAdmission` semaphore and the common
SIGTERM/Ctrl+C shutdown future. Admission is global across authenticated
connections, sheds excess work after a bounded wait, and is applied only to
expensive RPCs so readiness remains available under saturation. Worker hosts use
tonic graceful shutdown; cancellation-safe subprocess adapters retain
`kill_on_drop` for the bounded drain deadline.

`peer_certificate_fingerprint` derives a canonical SHA-256 fingerprint from
the verified mTLS leaf certificate on a tonic request. A protocol adapter can
use that fingerprint with its deployment-owned identity map, but cannot accept
an agent, node, or role identity from request JSON or metadata. This crate does
not own those protocol-specific mappings.

Each worker passes its protocol-specific message ceiling to the listener
constructor. The shared foundation rejects zero or values above its absolute
128 MiB ceiling.

Workers supply a stable uppercase environment prefix. Worker-specific policy,
credentials, tool execution, database access, and request/result contracts do
not belong here.
