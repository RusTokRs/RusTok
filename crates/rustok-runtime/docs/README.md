# `rustok-runtime` Documentation

`rustok-runtime` is a backend foundation crate for host runtime composition helpers.

The crate is intentionally small. Its first role is to stop new backend adapters from
copying typed shared-handle lookup and DB access patterns while the Loco runtime context is
being removed.

Boundary rules:

- Runtime contracts currently sourced from `rustok-api` may move here only when they are
  executable runtime helpers rather than stable API contracts.
- Domain services do not move here.
- HTTP response mapping belongs in `rustok-web`.
- CLI command contracts belong in `rustok-cli-core`.
- FBA provider/consumer metadata belongs in `rustok-fba`.

