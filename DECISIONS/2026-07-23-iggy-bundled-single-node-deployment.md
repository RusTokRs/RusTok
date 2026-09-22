# Iggy bundled and external deployment

- Date: 2026-07-23
- Status: Accepted

## Context

RusToK previously exposed an `Embedded` Iggy mode, but it did not start or
connect to a broker. It could make lifecycle tests appear successful while no
message had been durably published. The platform needs a Docker-free option
that is suitable for a durable single-node deployment as well as development
and automated testing.

The Iggy server is a separate native executable. The Rust SDK provides the
client protocol, not an in-process durable broker. RusToK therefore treats the
server artifact as an explicit module/deployment asset rather than pretending
that the SDK embeds a broker.

## Decision

Expose exactly `Bundled` and `External`, without compatibility aliases for the
former `Embedded`, `Local`, or `Remote` configuration values.

Bundled mode starts the `iggy-server` artifact packaged by the connector module, with
no shell, a configured persistent data directory, a loopback-only TCP listener,
and explicit startup and shutdown time bounds. It waits for both TCP reachability
and SDK authentication, then delegates publishing and persistent consumer groups
to the same client implementation used for external deployments. Consequently
bundled and external modes use the same real
Iggy SDK path.

Bundled mode is available only on operating systems supported by upstream
`iggy-server`. Upstream Iggy server does not support Windows, so RusToK rejects
bundled mode there with a configuration error instead of launching a process that
cannot exist. Windows hosts must use external Iggy on a supported host.

External mode remains the choice for independently managed, distributed, or
high-availability Iggy deployments. Persistent RusToK consumer groups require
TCP. External mode exposes Iggy SDK TLS settings; bundled mode keeps client traffic
on loopback and rejects a partial TLS bootstrap configuration.

The connector does not silently compile, download, or emulate Iggy at runtime.
A missing bundled artifact, disabled SDK feature, failed authentication, or
readiness timeout is a configuration or connection error. The bundled artifact
is owned by connector-module installation; an external-only distribution does
not install it.

## Consequences

- Bundled mode is valid for durable single-node production deployments, provided
  operators protect the data directory, set strong
  bootstrap credentials before first startup, and select Iggy durability
  settings appropriate for their crash-recovery requirements.
- Bundled mode is not a high-availability or clustering solution; use external
  mode with an independently supervised Iggy deployment for that topology.
- Application shutdown terminates the child process within a bounded timeout.
  Operators must treat Iggy durability configuration and host supervision as
  the source of crash-recovery guarantees.
- Existing `embedded`, `local`, and `remote` values are intentionally rejected.
  Configuration uses `bundled` or `external`.
- Real-broker integration tests remain required for receive, commit, reconnect,
  TLS/authentication, DLQ, and replay semantics.
