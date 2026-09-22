# Pages / Page Builder Contribution Parity Actualization — 2026-08-08

Status: current-source-overlay / pages-repair-lineage-source-ready / provider-degraded-controls-source-ready / contribution-registry-version-parity-source-ready / execution-open

## Why this actualization exists

A fresh source recheck found that the shared Pages/Page Builder continuation cursor still stopped at PR #3063 even though later source slices had already advanced both owners:

- PR #3191 made Pages recovery tolerate repeated physical loss of previously rebuilt immutable artifacts while preserving bounded repair/rollback lineage;
- PR #3196 connected Page Builder provider rollout/degraded controls to the admin capability path and kept absent live health explicitly `unobserved`;
- the current Fly contribution factories already separate admin/storefront assembly and already filter by tenant module, permission, capability, provider policy and provider health with duplicate, missing-dependency, cycle and missing-provider diagnostics.

The remaining Phase 9 source mismatch was version parity. The static contribution guard still described the intended owner-safe version-pinned manifest contract, while the live Rust model had regressed to an unversioned owner/target set.

## Source changes in this slice

The contribution manifest is version-pinned again:

- `ModuleContributionManifest` carries `owner_version`;
- cross-provider targets are `provider -> exact version` mappings;
- manifest normalization rejects empty/conflicting owner and target versions;
- manifest assembly requires every manifest-routed contribution to declare `metadata.providerVersion`;
- missing, non-string and empty provider versions fail closed;
- a provider-name allowlist match with the wrong version produces `contribution_target_provider_version_mismatch` and is not registered;
- provider allow/deny, tenant enablement, capability, permission, health, duplicate, dependency and cycle behavior remains layered after the version boundary.

Pages is the first current consumer of the restored contract:

- owner target: `rustok.pages@<crate version>`;
- Fly built-in target: `fly.builtin@1`;
- landing-block contribution declares `providerVersion = 1`;
- Pages metadata contribution declares `providerVersion = <crate version>`;
- existing executable metadata property-editor contribution remains present and continues to use consumer-owned persistence rather than mutating the Fly document.

No fallback unversioned contribution route is added.

## Rechecked parity state

### Pages

Source-ready through repeated immutable-artifact-loss recovery, including latest-repair-state-per-locale reconstruction and rollback continuity. Execution remains maintainer-owned and pending.

### Page Builder provider controls

Source-ready for rollout flags, fail-closed unavailable state, degraded publish suppression and Pages consumer wiring. There is still no authoritative live Page Builder SLO observation source in the repository, so Pages must continue to expose health as `unobserved`; this slice does not fabricate latency/error/sanitize observations from unrelated telemetry.

### Contribution registry Phase 9

- [x] Separate admin/storefront factories.
- [ ] Generate the complete contribution registry directly from canonical module metadata. Pages still constructs its `ModuleContributionManifest` in module-owned Rust and only cross-checks selected capability metadata against `rustok-module.toml`.
- [x] Filter by tenant, permission, capability, provider policy and health.
- [x] Duplicate, missing-provider, missing-dependency, cycle and provider-version diagnostics.

## Next source cursor

1. Define one canonical module-metadata representation for Fly contribution descriptors/version targets and generate `ModuleContributionManifest` from that authority instead of maintaining a parallel handwritten declaration.
2. Keep provider health `unobserved` until a real Page Builder SLO collector/source exists; then connect that source to `PageBuilderAdminProviderStatus` without allowing health to grant capabilities denied by RBAC/policy.
3. Preserve Pages as persistence/publication/repair owner and Page Builder/Fly as reviewed-document/runtime owner.
4. Retain all execution, browser, database, Cargo and static-verifier evidence as maintainer-owned until explicitly run.

## Validation boundary

Per maintainer instruction, no tests, Node verifiers, Cargo checks, formatting, database scenarios, workflows, CI or `git diff --check` are executed by this implementation slice.
