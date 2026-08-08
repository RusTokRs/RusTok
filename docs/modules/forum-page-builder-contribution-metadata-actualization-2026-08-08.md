# Forum / Page Builder contribution metadata actualization — 2026-08-08

Status: `source-ready / adapter-open / execution-pending`

## Scope

This slice continues the shared contribution-tooling cursor after PR #3222. Forum is the repository's second production Page Builder FBA consumer: its module manifest already declares Page Builder capability/degraded/rollout policy and its backend already owns a versioned widget catalog for `forum.topic_list`, `forum.topic_detail` and `forum.reply_stream`.

The missing boundary was canonical Fly contribution discovery metadata. Forum had no `fba.builder_consumer.contribution_manifest`, so the shared `rustok-build` normalizer and `xtask module validate forum` publish-readiness gate could not validate Forum contribution identity, provider version, permission admission or capability admission.

## Source result

`crates/rustok-forum/rustok-module.toml` now declares one owner-provider contribution discovery entry:

- module/provider identity remains `forum` / `rustok.forum`;
- owner version continues to derive from `[module].version` through the shared normalizer;
- module-level contribution admission requires `forum_topics:read`, matching the existing widget catalog/validation HTTP authorization boundary;
- the contribution requires only the already-declared `preview` Page Builder capability;
- metadata points at the existing Forum-owned catalog and validation endpoints instead of copying widget schema bodies;
- Forum remains the persistence and authorization owner.

The contribution deliberately declares `blocks = []` and no renderer/property-editor/storefront entries. Forum does not yet register Fly `BlockDefinition`s or a `ContributionAdapter` for its widget types. Advertising those surfaces before they exist would create a false runtime capability. The manifest therefore records `adapter_state = "pending"` and stays discovery-only.

## Widget schema drift correction

The existing `topic_detail` manifest entry incorrectly reused `forum.topic_list.v1` even though `ForumWidgetContractService` validates a distinct topic-detail schema. The canonical metadata now uses `forum.topic_detail.v1`; topic-list and reply-stream retain their existing distinct ids.

## Shared tooling boundary

No Forum-local TOML parser or contribution generator is added. The declaration is consumed by the shared source introduced in PR #3222:

- `crates/rustok-build/src/module_manifest_contribution.rs` validates module/provider/version, exact target provider versions, required permissions, capabilities and reserved identity metadata;
- `xtask module validate forum` invokes the same normalizer as module publish readiness;
- Forum admin receives no `fly-ui` or Page Builder UI dependency in this slice;
- `Cargo.toml` dependency topology and `Cargo.lock` remain unchanged by this slice.

## Ownership retained

Forum continues to own category/topic/reply lifecycle, revisions, visibility, widget source facts, widget validation and authorization. Page Builder/Fly continues to own generic authoring/composition contracts. This slice does not move Forum persistence, moderation, publication, routing or widget runtime data into Page Builder.

## Guardrail

`scripts/verify/verify-forum-page-builder-contribution-metadata.mjs` source-checks:

- exact Forum contribution discovery identity and `forum_topics:read` permission admission;
- distinct topic-list/topic-detail/reply-stream schema ids;
- shared normalizer markers;
- existing Forum widget endpoint RBAC;
- absence of Forum Fly renderer/property-editor/storefront claims while the adapter is pending;
- absence of a new Forum-admin `fly-ui` / Page Builder UI dependency;
- synchronized Forum and Page Builder plan markers.

## Next source cursor

Implement the first real Forum Fly adapter/component-registry slice before adding non-empty contribution blocks/renderers/property editors. That slice must define actual Fly component/block identities, adapter rendering/property behavior, preview data ownership and failure/degraded semantics against Forum-owned widget contracts. Only then should runtime contribution assembly claim those capabilities.

Observed tenant Wave evidence remains separate and still requires maintainer execution after the Pages reference-consumer gate.

No tests, Node verifiers, Cargo checks, formatting, builds, workflows, CI, browser or database evidence were run by the implementation agent.
