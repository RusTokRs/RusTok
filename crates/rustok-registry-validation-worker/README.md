# rustok-registry-validation-worker

This executable is the independent process boundary for durable registry
publish-bundle validation. It polls and conditionally claims owner-managed
validation jobs, reads only the claimed artifact object, verifies its size and
SHA-256, runs the owner validation contract, and persists a typed terminal
result through `rustok-modules`. Artifact reads use the bounded worker retry
policy; after its final failed attempt, the worker persists the failed terminal
result and treats that delivery as complete rather than retrying an already
settled job in the host loop.

It has no HTTP server dependency and does not use a server-local background
task. Configure its database connection, storage configuration JSON, worker ID,
and polling delay through the `RUSTOK_REGISTRY_VALIDATION_*` environment
variables.
