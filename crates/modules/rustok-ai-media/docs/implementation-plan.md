# rustok-ai-media implementation plan

## Current state

`rustok-ai-media` owns the `image_asset` descriptor and image-size validation.
`rustok-ai` composes the descriptor and executes providers; `rustok-media`
owns `MediaAssetReadPort`. The stable API and validation rules are in the crate
README, while the provider contract is in the FBA registry.

## FFA/FBA readiness

- FFA status: `not_started` — this support adapter has no UI boundary.
- FBA status: `boundary_ready` (`no_ui_boundary`).
- Structural shape: `no_ui_boundary`
- Static evidence records `MediaAssetReadPort` / `media.asset_read.v1`, the
  `get_image_descriptor` and `get_asset` operations, and degraded modes
  `skip_asset_enrichment`, `proxy_storage_relative_url`, and
  `summarize_internal_binary`.
- Evidence: `crates/rustok-ai-media/contracts/ai-media-fba-registry.json`,
  `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`,
  `crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json`,
  and `scripts/verify/verify-ai-media-fba.mjs`.

## Completed direct-execution evidence

The `image_asset` direct path is covered through `rustok-ai`: the runtime uses
adapter-owned size validation and normalized provider input, then persists the
provider image and its localized metadata through the Media owner service.

## Next results

1. **Execute the media consumer contract.** Add a composed test for
   `get_image_descriptor` and `get_asset` that proves typed port-error mapping,
   deadline propagation, and each declared degraded behavior. Done when the
   static matrix no longer records runtime evidence as pending.
2. **Specify a remote media adapter only on selection.** Before promoting the
   `remote_adapter_placeholder` profile, define its transport owner, security
   model, error mapping, and compatibility evidence. Done when no hidden
   alternate transport is implied.

## Verification

- `npm run verify:ai-media:fba`
- `npm run verify:ai:domain-verticals`
- `cargo test -p rustok-ai-media --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [AI media FBA registry](../contracts/ai-media-fba-registry.json)
