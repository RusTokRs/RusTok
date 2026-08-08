# Index replay GraphQL transport

Status: `source_complete_owner_execution_pending`.

## Boundary

The server now exposes two bounded GraphQL mutations over the existing guarded `IndexReplayOperatorRuntime`:

- `runIndexReplay(input: ...)` runs one bounded schema-wide replay chunk;
- `cancelIndexReplay(input: ...)` requests cancellation of one replay job in the authenticated tenant.

This is a command transport only. It does not add automatic replay scheduling, background worker ownership, a
new replay algorithm, locale/partition checkpoint dimensions, or a traffic cutover.

## Authority before input parsing

Tenant and actor identities are never accepted from GraphQL input. The transport derives both from
`TenantContext` / `AuthContext`, requires the request-bound effective permission snapshot, and requires
`modules:manage` before parsing untrusted schema identifiers, schema version, or job UUID.

The server-owned `IndexReplayOperatorRuntime` performs the same authorization again before delegating to the
canonical shared replay runtime. A cross-tenant job UUID is therefore only looked up in the authenticated
tenant and cannot widen authority.

## Server-owned replay budget

`IndexReplayRunInput` contains only:

- module name;
- entity name;
- positive schema routing key.

It does not accept tenant, actor, worker identity, page limit, page count, heartbeat cadence, lease duration,
locale, partition, source name, database handle, or scheduler controls.

Each GraphQL invocation constructs a fresh server-owned worker identity and uses one fixed bounded chunk:

- page limit: `100` mutations;
- maximum pages: `8`;
- heartbeat cadence: every page;
- lease duration: `60` seconds.

Those transport caps are intentionally narrower than the generic replay runner bounds. A yielded job remains
resumable by a later authorized invocation through the existing durable job/checkpoint contract.

## Result surface

The run payload exposes only bounded operational counters plus status and job UUID. It does not expose source
payloads, SQL, database errors, request authority, lease owner identity, or raw dependency causes.

The cancellation payload exposes only the normalized outcome: requested, cancelled, already terminal, or not
found. Operational runner failures are mapped to a generic GraphQL error; an unknown source-owned schema is
reported as bad user input.

## Evidence state

Source guards retain:

- authorization-before-parsing ordering;
- absence of caller authority/worker/resource-budget fields;
- fixed server-owned bounds;
- delegation only through `IndexReplayOperatorRuntime`;
- merged GraphQL schema registration;
- no direct database/source/scheduler access from the transport.

GraphQL execution, database replay execution, cancellation races, CI, and retained runtime evidence remain
maintainer-owned and are not claimed by this source slice.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
