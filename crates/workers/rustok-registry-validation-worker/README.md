# rustok-registry-validation-worker

This executable is the independent process boundary for durable, origin-aware
registry artifact validation. It polls and conditionally claims owner-managed
validation jobs, reads only the claimed artifact object, verifies its size and
SHA-256, runs the owner-selected validation contract, and persists a typed terminal
result through `rustok-modules`. Artifact reads use the bounded worker retry
policy; after its final failed attempt, the worker persists the failed terminal
result and treats that delivery as complete rather than retrying an already
settled job in the host loop.

Platform-built and external-prebuilt artifacts use the metadata publish-bundle
contract. Alloy-authored artifacts use the bounded canonical Rhai workspace
contract; their exact checksum is later required to match the reviewed source
revision before owner staging and final promotion.

After an Alloy workspace passes its delivery contract, the worker reloads the
immutable owner stage instead of consulting Alloy storage again. It
re-canonicalizes the workspace, checks the receipt-bound descriptor and declared
capabilities, derives deterministic CycloneDX and SLSA evidence from owner facts,
publishes one digest-pinned OCI manifest, signs it with Cosign, creates the
required signed CycloneDX and SLSA attestations, and resolves immutable
signature manifest by digest. It then fetches the
published package through a fresh short-lived credential lease and asks the
isolated verifier to require the exact request, tenant/script, revision, source,
review, descriptor, and entrypoint SLSA binding. Only a successful independent
decision records the digest of its verified evidence output and platform
admission. This route deliberately has no
build-service-attestation write, so a reviewed Rhai workspace cannot claim an
isolated WASM build it did not run.

For platform-built publication, the worker also owns production evidence
composition. It reloads the exact completed build and current publication stage
through `rustok-modules`, obtains a short-lived registry credential lease from a
deployment-owned broker, fetches and revalidates the digest-pinned OCI package,
and calls the isolated trust verifier through readiness-gated mTLS. Only then
does it record the immutable build-service attestation and platform-admission
contract through owner operations. Registry credentials and trust roots are not
exposed to the server, Alloy, MCP, AI, or module runtime.

It has no HTTP server dependency and does not use a server-local background
task. Configure `RUSTOK_INSTANCE_ROOT`, its database connection, storage driver
JSON, worker ID, and polling delay through the
`RUSTOK_REGISTRY_VALIDATION_*` environment variables. The local driver always
uses `rustok-runtime::InstanceLayout::storage()` at
`<instance-root>/storage`; its JSON does not select another physical root.

Production platform-build evidence additionally requires:

- `RUSTOK_REGISTRY_VALIDATION_VERIFICATION_ENDPOINT`;
- the `RUSTOK_REGISTRY_VALIDATION_VERIFICATION_*` mTLS identity and trust files;
- instance-relative `RUSTOK_REGISTRY_VALIDATION_REGISTRY_CREDENTIAL_BROKER` and its pinned
  `RUSTOK_REGISTRY_VALIDATION_REGISTRY_CREDENTIAL_BROKER_DIGEST`;
- `RUSTOK_REGISTRY_VALIDATION_REGISTRY_ID`;
- `RUSTOK_REGISTRY_VALIDATION_TRUST_POLICY_REVISION` and
  `RUSTOK_REGISTRY_VALIDATION_CAPABILITY_POLICY_REVISION`;
- `RUSTOK_REGISTRY_VALIDATION_BUILD_SERVICE_ISSUER_IDENTITY` and
  `RUSTOK_REGISTRY_VALIDATION_BUILD_SERVICE_POLICY_REVISION`.

Alloy OCI publication additionally requires:

- `RUSTOK_REGISTRY_VALIDATION_ALLOY_PUBLICATION_REGISTRY` and
  `RUSTOK_REGISTRY_VALIDATION_ALLOY_PUBLICATION_REPOSITORY`;
- instance-relative `RUSTOK_REGISTRY_VALIDATION_COSIGN_PROGRAM`, its pinned
  `RUSTOK_REGISTRY_VALIDATION_COSIGN_PROGRAM_DIGEST`, and
  `RUSTOK_REGISTRY_VALIDATION_COSIGN_KEY_REFERENCE`.

The selected verifier policy must allow the Alloy publication builder
`https://rustok.dev/registry-validation-worker`, build type
`https://rustok.dev/build/rhai-workspace/v1`, source URI
`https://rustok.dev/alloy/reviewed-workspace`, and source ref
`owner-receipted`; those static values are checked together with the owner-bound
SLSA fields, not accepted from worker configuration.
