# Forum / Page Builder deployed server-function attestation actualization — 2026-08-08

Status: `source-ready / maintainer-deployment-attestation-execution-pending / browser-execution-pending / runtime-execution-pending / wave-pending`

## Rechecked cursor

PR #3266 retained direct runtime authorization/visibility evidence source for the Forum Page Builder native transports. Its runner is still maintainer-execution-pending. PR #3264 retained the real browser evidence harness, which is also still execution-pending.

The remaining evidence gap between those packets was deployment provenance: a browser URL plus a maintainer-supplied RepoDigest did not independently prove which source revision the live `/api/fn` transport was serving.

This slice makes a bounded deployed server-function attestation source-ready without running it.

## Read-only transport probe

`rustok-forum-admin` now registers:

```text
POST /api/fn/forum/page-builder-transport-attestation
```

The server-function argument is intentionally a single bounded scalar:

```text
challenge=<bounded token>
```

This avoids reimplementing Leptos nested `request` serialization in an external evidence runner.

The probe is not a second Forum data API. Before returning anything it crosses the same production seams as Forum Page Builder preview/property transport:

1. authenticated `AuthContext` extraction;
2. trusted `TenantContext` extraction and exact tenant match;
3. platform effective `forum_topics:read` authorization;
4. exact enabled `forum` tenant-module lookup;
5. `HostRuntimeContext` lookup;
6. the shared `TransactionalEventBus` runtime dependency;
7. the Forum-owned widget contract catalog.

The success body contains only:

- the caller challenge;
- a stable attestation contract id;
- the deployed source commit when the production image supplies a canonical revision;
- generated Forum module/provider/version identity;
- Forum widget catalog/contract versions;
- the three canonical widget types;
- the canonical preview/property server-function paths.

It does **not** read or return topics, replies, categories, tenant ids, actor ids, sessions, permissions or Page Builder document content.

## Source revision binding

Both production server image definitions already carry the OCI revision label:

```text
org.opencontainers.image.revision=${OCI_REVISION}
```

This slice also exports the same immutable build argument into the production container runtime as:

```text
RUSTOK_SOURCE_COMMIT=${OCI_REVISION}
```

The probe returns that value only when it is a canonical 40-character hexadecimal Git commit. Images built with the default `unknown` value therefore cannot produce a successful deployment-attestation packet.

The external runner compares the live response marker to the exact checkout `HEAD`. This gives the live origin an independent source-revision statement instead of merely recording a digest supplied to the browser harness.

## RepoDigest boundary

The capture command also requires a canonical Docker RepoDigest:

```text
REPOSITORY@sha256:<64 hex>
```

That RepoDigest remains the immutable deployment identity used to correlate browser/deployment evidence.

However, this source does **not** claim a cryptographic origin-to-RepoDigest proof. The fact that the supplied RepoDigest belongs to the reviewed running deployment remains a maintainer/infrastructure provenance fact. The retained packet says so explicitly.

The live source revision and the reviewed RepoDigest are complementary:

- live source commit answers “which repository revision does this server report?”;
- RepoDigest answers “which immutable image did deployment review identify?”;
- infrastructure review still owns “this origin is routed to that immutable image.”

## Capture contract

The machine-readable contract is:

```text
crates/rustok-forum/contracts/evidence/forum-page-builder-serverfn-deployment-attestation-contract.json
```

The runner is:

```text
scripts/evidence/capture-forum-page-builder-serverfn-attestation.mjs
```

A successful future capture writes:

```text
format: forum_page_builder_server_fn_deployment_attestation_v1
status: server_fn_deployment_attestation_passed_wave_pending
```

The runner requires:

- a reviewed HTTP(S) origin;
- the reviewed deployment RepoDigest;
- authenticated credentials with effective `forum_topics:read`;
- a separate authenticated no-read credential profile;
- optional non-secret common routing headers such as a reviewed tenant routing header when the environment requires one.

It sends the same challenge to three profiles:

1. anonymous — must not obtain a valid success attestation;
2. authorized — must return the attestation contract, round-tripped challenge, exact checkout source commit, Forum owner identity, all three widget identities and all three canonical production transport paths;
3. no-read — must not obtain a valid success attestation.

The authorized request proves the exact deployed `/api/fn` dispatcher has the Forum attestation server function registered and can cross tenant/RBAC/module/runtime owner gates. The no-read/anonymous checks are deployment smoke evidence only; the exhaustive direct authorization semantics remain owned by the separate #3266 runtime harness.

## Retention and privacy

The retained packet stores only:

- checkout source commit;
- verification that the live response contained that source commit;
- reviewed immutable RepoDigest;
- SHA-256 and byte length of the target origin, never the raw origin;
- SHA-256 and byte length of the random challenge, never the raw challenge;
- required-source SHA-256 hashes;
- scenario status, bounded selected response headers, body size and body SHA-256;
- credential **environment variable names** only.

It never stores:

- raw URLs/origins;
- Authorization values;
- cookies;
- common header values;
- response bodies;
- tenant or actor identifiers;
- Forum content.

The prior output is removed before any network request so a failed attempt cannot leave a stale success packet.

## Source-only guard

The source guard is:

```text
node scripts/verify/verify-forum-page-builder-serverfn-deployment-attestation.mjs
```

It verifies the probe remains read-only and behind the shared transport gates, the production `/api/fn/{*fn_name}` dispatcher remains mounted through `handle_server_fns_with_context`, both production Docker definitions bind `RUSTOK_SOURCE_COMMIT` to the OCI revision argument, and the runner retains hashes/status only.

## Maintainer capture cursor

After deploying the exact source revision with a real OCI revision and preparing reviewed credentials:

```text
node scripts/evidence/capture-forum-page-builder-serverfn-attestation.mjs \
  --base-url <reviewed-origin> \
  --deployment-image-digest <repo@sha256:digest>
```

Credential and optional routing-header environment variable names are declared by the JSON contract.

The browser and direct runtime runners remain separate packets. Acceptance should correlate all three packets to the same source revision and reviewed RepoDigest/deployment provenance before any Forum Page Builder Wave claim.

## Promotion boundary

FORUM-32 remains `in_progress`.

Source is now ready for:

- contribution/Fly identity;
- Forum owner preview;
- Forum owner-backed properties;
- browser evidence;
- direct runtime authorization/visibility evidence;
- deployed native server-function source-revision attestation.

Still open:

1. execute and retain the browser packet;
2. execute and retain the direct runtime authorization packet;
3. execute and retain this deployed server-function attestation packet;
4. retain/approve the external origin-to-RepoDigest infrastructure provenance;
5. satisfy the existing Pages reference-consumer gate;
6. only then evaluate observed Forum Page Builder Wave evidence.

Provider SLO health remains `unobserved`; this probe does not convert missing provider-health observation into a healthy claim.

No HTTP request, browser, Cargo command, Node verifier, Docker build/inspect, database fixture, formatter, build, workflow or CI execution is claimed by this source slice.
