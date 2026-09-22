# Forum / Page Builder owner preview actualization — 2026-08-08

Status: `source-ready / owner-preview-host-composed / property-editing-open / execution-pending`

## Rechecked cursor

PR #3239 completed the Forum Fly identity layer: generated contribution metadata, three canonical
component/block ids, renderer/property-editor contract descriptors and a Forum
`ContributionAdapter` that intentionally performs no owner-data reads. The remaining source cursor
was the owner-backed preview/read path plus host composition into the real Pages Page Builder
surface.

That cursor is now source-ready without changing Forum persistence or authorization ownership.

## Forum owner preview boundary

`ForumWidgetPreviewService` is the only new owner-data preview composition service. Every request
first crosses the existing `ForumWidgetContractService::validate_props` normalization contract.
Invalid configuration returns the same normalized validation envelope without executing an owner
read.

Valid configuration is read under the caller's exact tenant and permission snapshot:

- `forum.topic_list` uses a widget-specific bounded Forum topic query. `category_id`, `page`,
  `per_page`, `include_pinned` and `sort` are applied before pagination. `activity`, `newest` and
  `top` therefore have real owner semantics instead of being post-filtered after a generic list
  endpoint. Hidden-category filtering comes from `ForumTopicVisibilityService` before the query.
- `forum.topic_detail` resolves the exact topic through the existing Forum topic facade and, when
  requested, returns at most 20 approved replies through the existing reply owner facade.
- `forum.reply_stream` keeps the existing page/per-page bound. `approved_only=true` returns only
  approved replies. `approved_only=false` requires `forum_replies:moderate` and still excludes
  deleted tombstones from preview output.

Neither tenant id nor actor id is accepted in widget props. Page Builder therefore cannot choose a
Forum security context.

`POST /api/forum/widgets/preview` exposes the owner contract beside the existing catalog and
validation routes. It requires `forum_topics:read` at transport admission and maps downstream Forum
owner authorization/visibility failures through the established Forum error boundary.

## Admin transport boundary

`rustok-forum-admin` adds a native server-function transport for the owner preview service. The
browser-facing request contains only `widget_type + props`; backend Forum types remain behind the
crate's existing SSR-only `rustok-forum` dependency.

The native transport rechecks:

- authenticated tenant scope;
- tenant Forum module enablement;
- `forum_topics:read`;
- the exact server-side permission snapshot used by the Forum owner service.

The transport returns the owner response as JSON only after Forum normalization and authorization.
It does not make Fly or Page Builder depend on the Forum backend crate.

## Provider-neutral Page Builder host

`rustok-page-builder-admin` now exposes a provider-neutral contribution-host context. One optional
domain extension may provide:

- its generated `ModuleContributionManifest`;
- a narrow Fly registry installer;
- an optional async owner-preview port.

The Page Builder package contains no Forum import. External manifests are assembled with the
current effective editor capability profile and exact host-granted permission strings, then merged
into the consumer-owned contribution registry. Registry identity conflicts fail closed as assembly
errors rather than replacing Pages contracts.

Provider registries are installed through one narrow `AdminCanvasController` seam and the current
document is immediately revalidated. Consumer persistence remains untouched.

`preview_off` remains truthful: the Forum `tree + properties` contribution stays admitted when
those capabilities remain enabled, while the separate `preview` renderer contribution is filtered.
The owner preview panel only becomes actionable when an admitted `Presentation::Preview` renderer
and matching owner-preview port both exist.

## Application composition

`apps/admin` is the composition root. On the real Pages admin route it builds optional Page Builder
extensions only from the tenant's already-loaded enabled-module set. Forum is therefore mounted
only when that tenant has Forum enabled.

Contribution discovery does not infer RBAC from the client-visible role. The browser sends only the
permissions declared by enabled contribution manifests to a server function; the server parses
each permission and evaluates it with `rustok_api::has_effective_permission`. This preserves the
platform rule that `resource:manage` satisfies an exact `resource:read` requirement while exposing
no unrelated permission snapshot to the browser.

The Forum extension supplies the generated Forum manifest, the Forum Fly registry installer and a
Forum preview port implemented through `rustok-forum-admin`. Pages remains the document/persistence
consumer and does not import Forum.

## Selected-component preview UI

The generic Page Builder right rail now contains an explicit owner-preview panel. It never performs
an automatic owner read merely because a document is opened or a component is selected.

The Refresh action is enabled only when:

1. the selected Fly component has a provider and component type;
2. the merged contribution registry contains an admitted preview renderer for that exact identity;
3. the host has mounted a preview port for the provider;
4. the Fly component carries an object-shaped `props` payload.

The request forwards only provider identity, component identity/type, presentation and the stored
configuration object. The UI displays a bounded 16 KiB JSON summary and truncates larger results;
owner data is not persisted back into the Fly document.

## Still open

Forum owner-backed property editing remains open. The property-editor metadata still uses
`forum_widget_owner_schema_ref_v1` and `owner_data_state = "owner_preview_transport_open"` because
this slice does not add schema-fetch/form/save UI. A future property editor must fetch/validate
through the Forum owner contracts and save only normalized configuration into Fly `props`; it must
not create a second Forum data store.

Observed runtime/browser evidence also remains open. This source slice does not promote the
synthetic Forum Wave packet to observed evidence and does not claim provider-health observation.

## Lockfile and validation boundary

This slice adds no new Cargo dependency declarations. The existing generated `Cargo.lock` refresh
left from the preceding Forum Fly-adapter dependency change remains maintainer-owned if it has not
already been refreshed externally.

No tests, Node verifiers, Cargo commands, formatters, lock generation, builds, workflows, CI,
database, browser or runtime evidence were executed while preparing this slice.

## Next cursor

1. Implement owner-backed Forum widget property-editor transport/UI using the existing schema ids,
   catalog and validation service; persist only normalized Fly `props`.
2. Retain browser evidence for Forum-enabled Pages authoring, explicit owner preview and
   `preview_off` behavior while confirming Forum-disabled Pages has no Forum extension.
3. Retain runtime authorization evidence for hidden categories, moderator reply preview and
   effective `manage -> read` contribution admission.
4. Replace the synthetic Forum tenant Wave packet only after the existing Pages reference-consumer
   execution gate is satisfied.
