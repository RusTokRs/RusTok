---
id: doc://docs/release/RELEASE_READINESS_CHECKLIST.md
kind: operational_checklist
language: markdown
status: active
owner: release engineering
---
# Release Readiness Checklist

Use this checklist for every RusToK release. A release is not complete because a tag exists; it is complete only when source, migration, compatibility, artifact, deployment and rollback evidence are recorded against the exact commit.

## 1. Release identity

- [ ] Select a canonical version `MAJOR.MINOR.PATCH` or an explicit SemVer prerelease. Build metadata is not allowed in the release tag.
- [ ] Update `[workspace.package].version` and regenerate `Cargo.lock`; confirm the locked `rustok-server` version matches.
- [ ] Add exactly one dated `## [VERSION] - YYYY-MM-DD` section to `CHANGELOG.md` with at least one real release bullet and no placeholder text.
- [ ] Confirm the release commit is reachable from protected `main` and all required checks passed for that exact commit.
- [ ] Create a cryptographically verified **annotated** tag `vVERSION`. Do not use a lightweight or unsigned release tag.

## 2. Repository and registry preflight

- [ ] Confirm repository immutable releases are enabled. The tag workflow must fail closed when the setting is unavailable or disabled.
- [ ] Confirm no GitHub Release already exists for `vVERSION`.
- [ ] Confirm no immutable GHCR tag `ghcr.io/rustokrs/rustok:VERSION` already exists.
- [ ] Confirm release workflows and GitHub actions are unchanged or explicitly approved with `release-infra-approved`.
- [ ] Confirm the pinned Debian base digest and Debian Snapshot timestamp in `apps/server/Dockerfile.release` have a reviewed update record if changed.
- [ ] For a private mirror, remove the unauthenticated post-checkout `git fetch origin main` residual or provide narrowly scoped ephemeral authentication before creating the tag.

## 3. Required verification before tagging

Record command output or CI run identifiers; do not rely on source inspection.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo nextest run --workspace --all-targets --all-features

node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-csp-inline-style-exceptions.mjs
node scripts/verify/verify-csp-next-style-boundary.mjs

node scripts/verify/verify-api-compatibility-self-test.mjs
node scripts/verify/verify-api-compatibility-infra-self-test.mjs
node scripts/verify/verify-api-compatibility-contract.mjs

node scripts/verify/verify-migration-plan-self-test.mjs
node scripts/verify/verify-migration-backfill-self-test.mjs
node scripts/verify/verify-migration-infra-self-test.mjs
node scripts/verify/verify-migration-compatibility-contract.mjs

node scripts/verify/verify-release-tooling-self-test.mjs
node scripts/verify/verify-release-infra-self-test.mjs
node scripts/verify/verify-release-supply-chain-contract.mjs
node scripts/verify/verify-release-runtime-image-contract.mjs

cargo tree -i rsa --workspace --all-features
cargo tree -i atomic-polyfill --workspace --all-features
cargo audit
```

- [ ] Run Next admin and frontend Playwright smoke suites.
- [ ] Run Leptos/server-hosted browser smoke for authentication, navigation, storefront and strict CSP.
- [ ] Run Page Builder smoke for custom viewport size, continuous zoom, iframe load, overlay alignment, drag/drop and resize.
- [ ] Execute fresh, incremental and N-1 PostgreSQL migration scenarios against disposable databases.
- [ ] Verify current backup/PITR evidence and identify the restore point immediately preceding deployment.

## 4. Tag workflow evidence

The release workflow must produce and verify all of the following:

- [ ] The tag is annotated, cryptographically verified and points to a commit on `main`.
- [ ] Workspace version, locked server version, tag and changelog section are identical.
- [ ] Two isolated jobs produce the same SHA-256 digest for the Linux server archive.
- [ ] The SPDX 2.3 SBOM contains only the transitive dependency closure reachable from `rustok-server`.
- [ ] The runtime image is built from the pinned Debian base and fixed snapshot package sources, runs as UID/GID `10001`, and is published by digest.
- [ ] Container provenance is attached to the GHCR digest.
- [ ] The finalized GitHub Release contains exactly five assets:
  1. `rustok-server-VERSION-linux-x86_64.tar.gz`
  2. `rustok-server-VERSION.spdx.json`
  3. `container-image.json`
  4. `release-manifest.json`
  5. `SHA256SUMS`
- [ ] `sha256sum --check SHA256SUMS` succeeds against the downloaded release assets.
- [ ] Checksummed asset attestations and the archive SBOM attestation verify against the RusToK repository identity.
- [ ] `container-image.json` and `release-manifest.json` reference the same commit, version, image and canonical digest.

## 5. Deployment and post-release smoke

- [ ] Deploy the image by immutable digest, not by `latest`, major or minor convenience tags.
- [ ] Record environment, region/cell, tenant profile, migration version and image digest.
- [ ] Verify `/health`, readiness, metrics and operator endpoints.
- [ ] Verify REST, GraphQL HTTP and GraphQL WebSocket tenant isolation on the deployed environment.
- [ ] Verify authentication issuance/refresh/revocation and privileged admin access.
- [ ] Verify storefront and admin browser journeys with no unexpected enforced CSP violations.
- [ ] Confirm worker queues, outbox/search lag and build admission metrics remain inside defined operating thresholds.
- [ ] Confirm no migration, panic, authorization or tenant-resolution error spike after deployment.

## 6. Rollback decision

- [ ] Name the incident commander and rollback approver before changing production state.
- [ ] Prefer application rollback by the previously recorded **image digest**.
- [ ] Do not move, recreate or overwrite a published SemVer tag or immutable GitHub Release.
- [ ] Do not use a mutable convenience image tag as rollback evidence.
- [ ] Roll back database schema only when the migration has a verified down path, no incompatible writes occurred, and the rollback smoke passed.
- [ ] Otherwise restore from the pre-release recovery point or deploy a forward-fix migration.
- [ ] Preserve failed release assets, logs, attestations and database evidence for incident review.

## 7. Failed-release recovery

### Failure before the version image tag is pushed

- [ ] Fix the source or infrastructure issue on `main`.
- [ ] Delete and recreate the release tag only if no GitHub Release or immutable version image tag was published, and record the reason.
- [ ] Prefer a new patch/prerelease version when any external consumer could have observed the tag.

### Failure after the version image tag is pushed but before GitHub Release publication

- [ ] Stop blind reruns: the collision guard is expected to reject the existing version tag.
- [ ] Record the existing image digest and verify its repository/commit provenance.
- [ ] Determine whether attestations and finalized assets were produced by the failed run.
- [ ] Do not overwrite the version tag with a different digest.
- [ ] Recovery requires an incident-approved procedure: either complete publication from the already verified digest and exact assets, or abandon the version and release a new patch/prerelease version.
- [ ] Delete a failed registry tag only when it was never announced or consumed, provenance proves it belongs to the failed run, and the deletion is recorded. Never use deletion to replace published evidence silently.

### Failure after GitHub Release publication

- [ ] Treat the release as immutable.
- [ ] Do not mutate assets, tag, checksums, SBOM or manifest.
- [ ] Mark the release as affected in operator communication and create a new patch version with corrected evidence.
- [ ] Roll back production by digest where necessary.

## 8. Evidence record

Attach or link the following to the release record:

```text
Version / tag:
Commit SHA:
Signed-tag verification:
Required-check run IDs:
API compatibility artifact/run:
Migration compatibility artifact/run:
Browser smoke runs:
Server archive SHA-256:
SBOM SHA-256:
GHCR image and digest:
Attestation verification:
Five-asset checksum verification:
Deployment environment and timestamp:
Pre-release restore point:
Post-release smoke owner/result:
Rollback owner/result (if used):
Exceptions and approvals:
```

A checkbox without a durable run, artifact or operator record is not release evidence.
