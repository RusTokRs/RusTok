# Forum / Page Builder Fly adapter actualization — 2026-08-08

Status: `source-ready / owner-preview-open / execution-pending`

## Rechecked source cursor

This packet continues the Forum contribution discovery slice merged in PR #3227. The discovery boundary was real, but `FORUM-32` still lacked actual Fly component/block identities, generated runtime contribution descriptors and a `ContributionAdapter` implementation.

The repository also already had a stricter owner boundary than the old cursor implied: `ForumWidgetContractService` owns distinct JSON schemas and normalization for `forum.topic_list`, `forum.topic_detail` and `forum.reply_stream`, and `/api/forum/widgets/catalog` plus `/api/forum/widgets/validate` enforce Forum authorization. Those contracts must not be copied into Fly metadata or replaced by a Page Builder-owned schema authority.

## Source result

Forum Fly component/block/adapter contracts: source-ready.

`crates/rustok-forum/rustok-module.toml` now remains the canonical source for two complementary admin contributions:

- `rustok.forum.widget-catalog` requires `tree + properties`, declares the three canonical Forum widget block/component ids and their owner-backed property editor references;
- `rustok.forum.widget-preview` requires only `preview` and declares the three renderer contracts.

The split is deliberate. The existing Forum `preview_off` profile keeps `builder.properties.enabled=true`; therefore preview admission must not accidentally remove widget identity or property editing. When preview is unavailable, the preview contribution can be filtered while the authoring/property contribution remains eligible.

Canonical component/block ids are exactly:

- `forum.topic_list`;
- `forum.topic_detail`;
- `forum.reply_stream`.

Each Fly block stores only an opaque JSON object under `props`. No topic, reply, category, visibility, authorization or preview result is copied into the Fly document.

## Generated runtime contract

`crates/rustok-forum/admin/build.rs` delegates generic TOML parsing and provider/version/capability admission to the shared `rustok-build/src/module_manifest_contribution.rs` normalizer. It validates the Forum-specific role split and exact block/renderer/property-editor component set, then emits the normalized version-pinned manifest to `OUT_DIR`.

Forum admin runtime does not parse TOML.

`crates/rustok-forum/admin/src/page_builder.rs` now provides:

- the build-generated `ModuleContributionManifest`;
- a policy-aware Forum admin contribution registry;
- `register_forum_fly_widgets` / `forum_fly_registry_set` for real `ComponentDefinition` and `BlockDefinition` registration;
- `ForumContributionAdapter` for renderer/property-editor contract resolution;
- typed render/property models carrying canonical widget identity, opaque `props` and an owner schema reference.

The adapter does not import `ForumWidgetContractService`, `TopicService`, `ReplyService` or database state. It cannot become a second Forum owner.

## Schema and validation ownership

Property-editor metadata uses `forum_widget_owner_schema_ref_v1`. It contains only:

- the stable schema id (`forum.topic_list.v1`, `forum.topic_detail.v1`, `forum.reply_stream.v1`);
- `/api/forum/widgets/catalog`;
- `/api/forum/widgets/validate`;
- `owner_data_state = "owner_preview_transport_open"`.

The actual JSON schemas and normalization remain in the Forum backend owner contract. This prevents schema drift between Fly and Forum.

## Remaining runtime boundary

Forum owner-backed preview transport/host mount: open.

The current Forum topic list HTTP/read model does not implement every widget-contract property such as `sort` and `include_pinned`. Reusing it as a fake preview endpoint would silently weaken the widget contract. The next source slice must therefore add an explicit owner-backed widget preview/read transport that consumes the already-normalized widget configuration and preserves Forum visibility/RBAC, then connect it through the Page Builder admin/Leptos host.

Until that source exists, the Fly adapter resolves contract identity/configuration but does not claim owner-data preview execution.

Observed tenant Wave evidence remains pending after the Pages reference-consumer gate. Provider-health observation remains a separate runtime cursor.

## Lockfile and execution boundary

Cargo.lock refresh: maintainer-owned.

`rustok-forum-admin` now declares existing workspace packages `fly` and `fly-ui` plus build-time TOML parsing. The checked-in lockfile package dependency list is generated state. Per maintainer instruction, this source-authoring slice does not run Cargo and does not hand-edit generated `Cargo.lock`; the lock refresh belongs to the maintainer execution pass before any `--locked` acceptance claim.

No tests, Node verifiers, Cargo checks, formatting, lock generation, builds, workflows, CI, database, browser or runtime evidence were run by the implementation agent.

## Static guard

`scripts/verify/verify-forum-page-builder-contribution-metadata.mjs` now source-checks:

- the split `tree+properties` / `preview` capability admission;
- all three canonical block/component ids;
- renderer and owner-schema-reference property contracts;
- shared build-time normalizer use;
- Fly registry and `ContributionAdapter` source;
- absence of backend owner/data access from the Forum Fly adapter;
- the still-open owner preview transport boundary.

The existing Forum Wave plan/evidence guard remains unchanged; the observed-run gate is not promoted by this source slice.

## Next source cursor

1. Add an explicit Forum-owned widget preview/read port and transport for all three widget contracts, preserving exact visibility/RBAC and normalized props semantics.
2. Connect that owner preview transport into the Forum/Page Builder admin host without moving Forum data ownership into Fly.
3. Keep preview-off property authoring functional while preview rendering degrades explicitly.
4. Retain runtime/browser evidence before replacing the synthetic Forum Wave packet with an observed tenant run.
