# Implementation Plan for `rustok-cli-platform`

## Current state

`rustok-cli-platform` owns platform-level providers that do not belong to a
domain module. `core version` is registered through the generated selected
distribution registry and executes through the typed `CommandProvider` path.
The crate remains independent of `apps/server`, the runner, and domain crates.
`core rebuild` is the platform-owned maintenance command: it
uses `rustok-build` directly and accepts `--build-id` plus the shared
`--dry-run` option.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- Terminal parsing belongs to `rustok-cli`; module maintenance commands belong
  in owner-local `cli/` adapters; this crate is only for true platform commands.

## Open results

1. **Select the next platform-owned command only after an ownership decision.**
   Done when the command cannot belong to an existing module, is implemented as
   a typed provider here, and is selected through generated registry metadata.
   **Depends on:** platform ownership approval. **Verification:**
   `cargo test -p rustok-cli-platform --quiet` and
   `node scripts/generate/generate-cli-registry.mjs --check`.

## Verification

- `cargo test -p rustok-cli-platform --quiet`
- `node scripts/generate/generate-cli-registry.mjs --check`
- `node scripts/verify/verify-api-surface-contract.mjs`

## Change rules

1. Do not add module-specific maintenance commands to this crate.
2. Keep runner output, parsing, and exit policy in `rustok-cli`.
3. Keep selected provider wiring generated through `rustok-cli-registry`.
