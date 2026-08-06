# Pages / Page Builder Inline Edit Artifact and HTTP Evidence Harness Packet

Date: 2026-08-06  
Status: `source-ready / maintainer-execution-pending / browser-rollout-pending`

## Scope

This packet adds machine-locked, maintainer-run tooling for the next Pages inline-authoring execution boundary:

- two independent deterministic build snapshots;
- embedded admin, dedicated authoring JS/WASM and server binary hashes and sizes;
- full embedded admin distribution manifests;
- production image digest and runtime identity;
- authoring asset `200` and conditional `304` behavior;
- direct-user and denied authoring route cases;
- reuse of the existing anonymous storefront artifact inspector;
- one same-commit aggregate evidence document.

The tooling is source only. It does not run automatically and no execution result is claimed by this packet.

## Machine contract

The locked contract is:

```text
crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json
```

It owns:

- exact input and output formats;
- the two build profile identifiers `build-a` and `build-b`;
- required critical artifact identities;
- fixed asset paths and MIME types;
- expected authoring admission statuses;
- source-commit binding;
- privacy and non-promotion boundaries.

The final aggregate path is:

```text
target/pages-inline-edit-artifact-http-evidence.json
```

Its passing status is deliberately bounded:

```text
artifact_http_execution_passed_browser_rollout_pending
```

A passing aggregate does not claim browser edit/save/replay/expiry behavior or tenant rollout.

## Build snapshots

After each independent build, the maintainer runs:

```text
scripts/evidence/capture-pages-inline-edit-build-snapshot.mjs
```

The capture requires:

- explicit `build-a` or `build-b` profile;
- the produced server binary;
- exact Trunk and wasm-bindgen binaries;
- the build command log;
- an output evidence path.

It records only:

- source commit;
- Node, Cargo, rustc, Trunk and wasm-bindgen versions;
- SHA-256 and byte size of the command log, not raw output;
- SHA-256 of all locked source files;
- SHA-256 and byte size of the embedded admin index/CSS, authoring bootstrap/JS/WASM and native server binary;
- a sorted full-file manifest for `apps/admin/dist`.

Symlinks, empty files, a non-executable server binary or source-commit mismatch fail closed.

The assembler requires both build profiles to have identical toolchains, source hashes, critical hashes/sizes and full embedded admin manifests.

## Production image capture

The maintainer runs:

```text
scripts/evidence/capture-pages-inline-edit-docker-evidence.mjs
```

It uses `docker image inspect` but persists only a bounded projection:

- canonical image ID;
- immutable RepoDigests;
- image size;
- `linux/amd64` platform;
- runtime user `10001:10001`;
- `/app/rustok-server` entrypoint;
- selected OCI labels.

`org.opencontainers.image.revision` must equal the checked-out source commit. The raw Docker inspect document and environment values are not persisted.

## HTTP capture

The maintainer runs:

```text
scripts/evidence/capture-pages-inline-edit-http-evidence.mjs
```

The target must be an explicit HTTP(S) origin. The script requests:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

For every asset it requires:

- initial `200`;
- exact MIME type;
- `Cache-Control: public, max-age=0, must-revalidate`;
- `Cross-Origin-Resource-Policy: same-origin`;
- a strong full SHA-256 ETag equal to the body hash;
- empty `304` for exact `If-None-Match`;
- empty `304` for weak `If-None-Match`.

The authoring route is exercised for:

```text
anonymous          → 401
direct_user        → 200
service            → 403
delegated          → 403
missing_session    → 401
permission_denied  → 403
```

Every authoring response must carry:

```text
Cache-Control: private, no-store
X-Robots-Tag: noindex, nofollow, noarchive
```

The successful direct-user HTML must contain the bounded authoring root, bootstrap path, canonical page UUID and exact locale. It must not contain known proof, token, session or signing-secret markers.

Credential values are read only from named environment variables. The evidence stores environment variable names, selected response headers, status, body size and body SHA-256. It never stores Authorization/Cookie values, raw HTML, denial bodies, grants or proofs.

An optional environment variable may provide bounded non-secret common headers:

```text
RUSTOK_PAGES_INLINE_EDIT_EVIDENCE_COMMON_HEADERS_JSON
```

It cannot contain Authorization, Cookie or Set-Cookie.

## Anonymous artifact input

The aggregate reuses the existing inspector output:

```text
crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs
```

The input must have:

```text
format: pages_anonymous_storefront_ssr_artifact_execution_v1
status: passed
source_commit: exact current commit
findings: []
```

Every inspected artifact must have an empty `forbidden_markers_found` array.

Absence of an artifact is not treated as passing evidence.

## Aggregate assembly

The maintainer runs:

```text
scripts/evidence/assemble-pages-inline-edit-artifact-http-evidence.mjs
```

Inputs:

```text
build-a snapshot
build-b snapshot
Docker capture
HTTP capture
anonymous artifact inspector packet
```

The assembler requires all inputs to bind the same current git commit. It additionally binds each HTTP asset body hash to the corresponding built authoring asset hash.

The aggregate is written through atomic replacement and records SHA-256/size for every input document. It never edits canonical plans or source evidence automatically.

## Privacy boundary

The following values are forbidden from retained evidence:

- Authorization and Cookie header values;
- bearer or session tokens;
- session IDs;
- inline grants and proofs;
- signing keys;
- raw authoring HTML or denial bodies;
- raw build logs;
- full Docker inspect documents.

Only bounded facts, environment variable names, selected headers, hashes, sizes and source identities are retained.

## Deliberate limits

This packet does not provide or claim:

- browser launch visibility execution;
- same-origin browser navigation execution;
- editable real-DOM interaction execution;
- save and reload execution;
- replacement grant execution;
- stale revision conflict execution;
- replay denial execution;
- expiry denial execution;
- FFA/FBA or tenant rollout.

Those remain the next cursor after artifact and HTTP evidence is actually captured and reviewed.

## Suggested maintainer sequence

Commands are illustrative and intentionally not run by the implementation agent:

```bash
# Build A, tee its output to target/pages-inline-edit-build-a.log, then capture.
node scripts/evidence/capture-pages-inline-edit-build-snapshot.mjs \
  --profile build-a \
  --server-binary target/release-build/release/rustok-server \
  --trunk .tools/trunk-0.21.14/bin/trunk \
  --wasm-bindgen .tools/wasm-bindgen-cli/bin/wasm-bindgen \
  --command-log target/pages-inline-edit-build-a.log \
  --output target/pages-inline-edit-build-a.json

# Repeat from an isolated clean build root/worktree for build-b.
node scripts/evidence/capture-pages-inline-edit-build-snapshot.mjs \
  --profile build-b \
  --server-binary target/release-build/release/rustok-server \
  --trunk .tools/trunk-0.21.14/bin/trunk \
  --wasm-bindgen .tools/wasm-bindgen-cli/bin/wasm-bindgen \
  --command-log target/pages-inline-edit-build-b.log \
  --output target/pages-inline-edit-build-b.json

node scripts/evidence/capture-pages-inline-edit-docker-evidence.mjs \
  --image ghcr.io/rustokrs/rustok@sha256:DIGEST \
  --output target/pages-inline-edit-docker.json

node scripts/evidence/capture-pages-inline-edit-http-evidence.mjs \
  --base-url https://tenant.example \
  --page-id PAGE_UUID \
  --locale en \
  --output target/pages-inline-edit-http.json

node crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs \
  --artifact EXPLICIT_ANONYMOUS_ARTIFACT \
  --output target/pages-anonymous-storefront-ssr-artifact.json

node scripts/evidence/assemble-pages-inline-edit-artifact-http-evidence.mjs \
  --build-a target/pages-inline-edit-build-a.json \
  --build-b target/pages-inline-edit-build-b.json \
  --docker target/pages-inline-edit-docker.json \
  --http target/pages-inline-edit-http.json \
  --anonymous target/pages-anonymous-storefront-ssr-artifact.json \
  --output target/pages-inline-edit-artifact-http-evidence.json
```

## Validation status

No tests, static verifiers, Cargo commands, npm installs, Trunk builds, WASM builds, server builds, Docker commands, HTTP requests, browser scenarios, workflows or CI were run by the implementation agent.

The source harness is ready for maintainer execution. Artifact, HTTP, browser and rollout evidence remain pending.
