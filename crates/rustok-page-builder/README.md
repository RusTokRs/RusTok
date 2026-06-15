# rustok-page-builder

## Purpose
`rustok-page-builder` is a standalone FBA-first visual builder reference module.
It defines capability-first contracts and lifecycle seams for visual page composition before any domain consumer binding.

## Responsibilities
- keep vendor-neutral builder contract baseline (`grapesjs_v1` write/read semantics);
- expose module runtime identity and permissions for builder lifecycle operations;
- serve as reference implementation for FBA module rollout sequence.

## Entry points
- `src/lib.rs` — module runtime metadata (`PageBuilderModule`) and permission surface.
- `src/service.rs` — transport-neutral capability service, rollout guard, and authorized handler seam.
- `rustok-module.toml` — module manifest contract.
- `docs/README.md` — module runtime contract in Russian.

## Interactions
- consumed by `rustok-pages` and other potential layout/content modules as builder capability consumers;
- aligned with central rollout plan in `docs/modules/tiptap-page-builder-implementation-plan.md`.
