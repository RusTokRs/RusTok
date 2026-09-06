# rustok-module-build-dispatcher

This crate owns transport-neutral delivery handling for queued module builds.
`IggyModuleBuildDeliverySource` is the production external adapter for the
dedicated `module-build` topic. It owns one persistent consumer-group cursor
and commits an offset only after the owner persists a terminal result.
Before invoking the remote worker, the dispatcher obtains the owner-issued
durable build claim. A duplicate delivery that finds a live claim elsewhere is
acknowledged without starting another worker; only the claim holder can persist
the terminal result. If that holder is lost, the owner allows a later delivery
to replace the claim only after its lease expires.

The `rustok-module-build-dispatcher` binary is the separately deployable host
for that adapter. It has only a database connection, external Iggy credentials,
and an mTLS client identity for the build worker. It never starts Cargo or
shares a process with `apps/server` or `rustok-module-build-worker`.

It does not run Cargo, access CAS, or execute inside the server or build worker
process.
