# Module composition is the manifest

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: Implemented
- Owners: repository tooling, `apps/server`, `rustok-distribution`
- Extends: [Shared Platform Manifest Foundation Library](./2026-10-15-rustok-manifest-foundation-library.md)
- Supersedes: None
- Superseded by: None

## Context

`modules.toml` is the canonical statement of which modules a deployment is composed of; the
registry the server compiles is checked against it at startup (`validate_registry_vs_manifest`), and
the documentation says so in as many words (`docs/architecture/matryoshka.md`). The compiled
registry, however, is decided by something else entirely: the `mod-*` cargo features of
`apps/server/Cargo.toml`. Nothing derived one from the other, and nothing compared them.

They drifted. Fifteen modules the manifest declared — with a package that declares a `[crate]
entry_type`, which is exactly what the contract requires in the registry — were absent from the
server's `default` feature list. `cargo build -p rustok-server` therefore produced a binary that
exited during startup with `MissingInRegistry`. CI compiled the server with `--all-features`, so the
default build was never exercised; it only surfaced in the "Rust-hosted Browser Smoke" job, which
uses the default build, and it failed there before reaching its own spec. The failure looked like a
browser-and-CSP problem and was a composition problem.

The reverse direction was also unguarded: a `mod-*` feature in the default list for a module the
manifest does not declare produces `UnexpectedInRegistry`, and no check would have said so before
startup.

## Decision

1. **The manifest is the input; the feature list is derived.** A check reads the `[modules]` table
   of the manifest, computes the feature set a build of that manifest needs, and fails when the
   server's `default` list disagrees. It runs in CI before every default-feature build.
2. **The requirement is keyed on a runtime entry, not on a manifest line.** A module needs its
   `mod-*` feature exactly when it is compiled from this workspace (`source = "path"`, not
   `runtime = "extension"`), is not `required = true`, and its **package** declares a `[crate]
   entry_type`. That last condition is the one the contract enforces: the registry check reads the
   flag the package's manifest supplies, so a module can be required by its package and not by the
   catalogue, and a catalogue entry without a package entry is not required.
3. **The reverse direction is keyed on manifest slugs.** A compiled `mod-*` feature is unexpected
   only when the manifest names no such module. A compiled extension is consistent, not a defect,
   because the registry contract is about the manifest map and does not require the module to be
   present.
4. **The check is dependency-free and runs before the build.** It parses the inline tables itself
   rather than depending on a TOML library, so it can run in a job that has not installed anything,
   and it reads the manifest through the same `RUSTOK_MODULES_MANIFEST` resolution the server's
   build script uses, so CI and startup cannot disagree about which file they mean.
5. **The check does not edit the list.** It reports the drift with the failure each item would
   cause at startup; the feature list stays a reviewed artifact of the workspace, not something a
   script rewrites.

## Sources of truth and ownership

- Canonical: `modules.toml` for composition, and each module package's `rustok-module.toml` for its
  own `[crate] entry_type`.
- Canonical for the compiled set: `apps/server/Cargo.toml`.
- Derived: `scripts/ci/modules-manifest-features.mjs` computes the required set and the diff; it
  owns no state and is not a second list.
- The server's build script and its startup validation remain the authority on whether a binary may
  serve.

## Invariants

- A default build of the server starts: every module the manifest requires and the package declares
  an entry for is compiled.
- No compiled module is absent from the manifest.
- The check uses the same manifest resolution as the runtime, so a manifest the tool reads is the
  manifest the binary validates.
- A failure names the feature and the startup error it would produce, so the report is actionable
  without reading the server.

### Allowed states

- An extension is compiled and not required.
- A manifest module without a package entry is compiled and not required.
- A `required = true` module is compiled unconditionally and contributes no feature requirement.

### Forbidden states

- A default build that exits on its own manifest.
- `--all-features` as the only build CI ever performs, leaving the default list unexercised.
- A checker that keeps its own copy of the module list.

## Non-goals

- This decision does not choose which modules a deployment should ship; it makes the choice the
  manifest states the one the binary is built with.
- It does not remove `--all-features` from CI; that build still covers modules a deployment may opt
  into.
- It does not introduce the `profile-*` features that `docs/architecture/distribution-profiles.md`
  describes aspirationally — they do not exist as cargo features, and a check written against them
  would fail the build rather than guard it.

## Data, transaction, and concurrency boundary

No data. The check reads two files and exits; it writes nothing, and it holds no cache that could
outlive a manifest change.

## Context dimensions

Tenant, principal, locale, channel and timezone do not participate: this is a build-time property of
the binary, decided before any request exists.

## Events and projections

None. A change to the manifest or to the feature list is a source change, reviewed and built like
any other.

## Failure semantics

- Disagreement fails the job before the build, with one line per drifted feature.
- An unreadable manifest or a missing `default` list exits with the usage/read code (`2`), distinct
  from a disagreement (`1`), so a broken check is never mistaken for a composition defect.
- The check never weakens to a warning: a default build that cannot start is not a warning.

## Migration and cutover

Adopting the check required correcting the `default` list once — the fifteen missing features — and
nothing else. No runtime behaviour changes, no route changes, and no manifest changes.

## Alternatives considered

- **Generate `apps/server/Cargo.toml` from the manifest.** Rejected: it would make the cargo
  feature list unreadable and hand a script the authority to change the build, including for the
  features a deployment opts into.
- **Check at startup only.** Rejected: that is the current state, and it spends a full build to
  learn what two files already say.
- **Key the requirement on the manifest's runtime mode alone.** Rejected: it misses the package
  entry that the contract actually reads, which is how a required module could stay uncompiled.
- **Key the reverse direction on compiled modules.** Rejected: it flags every extension as a
  defect.

## Verification

- `node scripts/ci/modules-manifest-features.mjs --check` passes on the corrected tree and fails,
  naming the feature, when a required `mod-*` is removed from the default list.
- `node scripts/ci/modules-manifest-features.test.mjs` pins the derivation: a required module needs
  its feature, a `required = true` module does not, an extension is not required, a compiled module
  the manifest never declares is unexpected, and the parser ignores comments and section headers.
- Both workflows run the check before their build, so neither a default-feature build nor an
  `--all-features` build can be the only one that runs.

## Consequences

The default build is now the composition the manifest describes, and CI proves it before it spends
the build. The cost is a second place where a new module must be registered — the feature list — but
it is now checked rather than remembered, and a manifest change without the matching feature fails
in seconds instead of after a build and a failed startup.
