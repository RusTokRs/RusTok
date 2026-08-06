# Pages / Page Builder Inline Edit Release Composition Packet

Date: 2026-08-06  
Status: `source-ready / execution-browser-rollout-pending`

## Scope

This packet composes the previously source-ready Pages authenticated authoring route, binary-embedded client assets and admin launch control into the deterministic release and production server Docker build paths.

It does not change the Pages document, grant, Fly command, persistence, publish, rollback, route-admission, cache or public storefront owners.

## Single build owner

The deployment source owner is:

```text
scripts/build/build-pages-inline-edit-deployment.sh
```

It runs, in order:

```text
build-embedded-admin.sh --pages-inline-edit-launch
→ build-pages-inline-edit-server.sh --profile release
→ validate embedded admin output
→ validate bootstrap/JS/WASM output
→ validate native server binary
```

The same script is called by:

- the release artifact build job;
- the independent release reproducibility job;
- the production stage of `apps/server/Dockerfile`.

This prevents release, rebuild and Docker from silently composing different feature graphs.

## Embedded admin boundary

`build-embedded-admin.sh` retains its standard default behavior. Standard builds explicitly remove `RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN` from the child process.

The new explicit mode:

```text
--pages-inline-edit-launch
```

uses:

```text
RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true
trunk --no-default-features --features hydrate,pages-inline-edit-launch build --release
```

Only the embedded same-origin deployment composition selects this mode. The standalone admin Dockerfile remains unchanged.

Trunk installation now uses the exact Cargo requirement:

```text
cargo install trunk --version "=0.21.14" --locked
```

## Dedicated authoring client and server boundary

`build-pages-inline-edit-server.sh` continues to:

- derive the exact `wasm-bindgen` version from `Cargo.lock`;
- install or verify exactly that `wasm-bindgen-cli` version;
- build only the dedicated `pages-inline-edit-hydrate` client profile;
- validate the fixed bootstrap, JavaScript and WebAssembly files;
- build `rustok-server --features pages-inline-edit-assets`.

The generated files remain compile-time inputs to the Pages-owned asset router.

## Cross-target Rust flags

Native release linker flags can be invalid for WebAssembly. The composition therefore separates three values:

```text
RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS
RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS
RUSTFLAGS
```

The first applies only to embedded admin WASM. The second applies only to the dedicated authoring WASM. The original `RUSTFLAGS` value is restored for the native server build.

The release workflow sets the two WASM builds to deterministic path remapping without native linker flags, while retaining strip/build-id flags for the native binary.

## Release and reproducibility workflow

Both isolated jobs now invoke the same deployment orchestrator with the same:

- exact Trunk tool root;
- exact Cargo.lock-selected wasm-bindgen tool root;
- admin target directory;
- server target directory;
- source timestamp and native reproducibility flags.

Archive packaging, SBOM generation, digest comparison, attestations, collision checks and immutable publication remain unchanged.

The workflow action references were synchronized to the existing allow-list of full commit SHAs. No tag, branch or floating action reference was added.

## Production Docker boundary

The production builder in `apps/server/Dockerfile` uses the same orchestrator and validates all required outputs before the runtime stage.

The development stage retains the standard embedded admin build and does not enable the same-origin launch feature.

`apps/server/Dockerfile.release` remains runtime-only and unchanged. It receives the already-built binary from the deterministic release archive; no Cargo, npm, Trunk or wasm-bindgen tooling is added to that runtime image.

The standalone admin Dockerfile remains unchanged and does not receive the same-origin launch build acknowledgement.

## Approval and supply-chain policy

The `release-infra-approved` gate now protects:

```text
scripts/build/build-pages-inline-edit-deployment.sh
scripts/build/build-pages-inline-edit-server.sh
apps/storefront/scripts/build-pages-inline-edit-client.mjs
```

alongside the existing release workflows, Dockerfiles and embedded admin builder.

The base-owned supply-chain guard requires:

- one common deployment owner in release build and rebuild;
- the same owner in production Docker;
- exact Trunk and wasm-bindgen contracts;
- explicit same-origin admin launch selection;
- separated cross-target Rust flags;
- current action allow-list and occurrence counts;
- unchanged runtime-only image policy.

The release readiness checklist now requires durable hashes, sizes, HTTP evidence and browser evidence. Source inspection alone is explicitly insufficient.

## Deliberate limits

This source slice does not claim:

- execution of any static verifier;
- Cargo, Trunk, npm, wasm-bindgen or server build success;
- an embedded admin JS/WASM artifact;
- a dedicated authoring JS/WASM artifact;
- a server binary or Docker image;
- release or reproducibility workflow execution;
- HTTP `200`/`304`, MIME, ETag, cache, CORP or CSP behavior;
- launch visibility or navigation;
- browser edit, save, replacement grant, stale revision, replay or expiry behavior;
- anonymous bundle inspection;
- tenant rollout, FFA or FBA promotion.

## Source evidence

- `.github/workflows/release.yml`
- `.github/workflows/release-infrastructure.yml`
- `.github/workflows/hardening-gates.yml`
- `apps/server/Dockerfile`
- `scripts/build/build-embedded-admin.sh`
- `scripts/build/build-pages-inline-edit-deployment.sh`
- `scripts/build/build-pages-inline-edit-server.sh`
- `apps/storefront/scripts/build-pages-inline-edit-client.mjs`
- `scripts/verify/verify-release-infrastructure-approval.mjs`
- `scripts/verify/verify-release-supply-chain-contract.mjs`
- `scripts/verify/verify-release-readiness-contract.mjs`
- `docs/release/RELEASE_READINESS_CHECKLIST.md`
- `crates/rustok-pages/contracts/evidence/pages-inline-edit-release-composition-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-inline-edit-release-composition.mjs`

## Next cursor

1. Apply the required `release-infra-approved` review label to this protected source change.
2. Run the Pages route, asset, launch and release-composition static guards.
3. Run the release supply-chain and readiness guards.
4. Build the composition twice and retain embedded admin JS/WASM, authoring JS/WASM, server binary and archive hashes and sizes.
5. Build the production Docker target and record its digest.
6. Prove asset `200`/`304`, MIME, ETag, cache, CORP and CSP behavior.
7. Prove launch visible/hidden states and exact-locale same-origin navigation.
8. Execute direct-user allowed and anonymous/service/delegated/permission-denied cases.
9. Observe edit, save, replacement grant, stale revision, replay and expiry browser behavior.
10. Re-run anonymous graph and built-artifact exclusion evidence.
11. Record tenant rollout evidence before FFA/FBA promotion.

## Validation status

No tests, static verifiers, formatting, Cargo checks, npm installs, Trunk builds, WASM builds, server builds, Docker builds, HTTP requests, browser navigation, workflows or CI were run by the implementation agent.

Suggested commands, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-release-composition.mjs
node scripts/verify/verify-release-infra-self-test.mjs
node scripts/verify/verify-release-supply-chain-contract.mjs
node scripts/verify/verify-release-readiness-contract.mjs
bash scripts/build/build-pages-inline-edit-deployment.sh
```

Execution evidence remains pending.
