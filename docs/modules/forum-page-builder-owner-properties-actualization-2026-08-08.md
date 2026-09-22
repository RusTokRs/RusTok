# Forum / Page Builder owner property editor actualization — 2026-08-08

Status: `source-ready / owner-preview-and-properties-host-composed / execution-pending`

## Rechecked cursor

PR #3247 completed the owner-backed Forum widget preview path and mounted the provider-neutral
contribution host on the real Pages Page Builder admin route. The remaining FORUM-32 source cursor
was owner-backed property editing: load the current Forum schema, edit configuration without
copying Forum authority into Fly, validate through the Forum owner contract, and persist only the
normalized configuration object in the Fly draft.

That source cursor is now implemented. Runtime, browser and observed Wave evidence remain open.

## Owner schema authority

The canonical contribution manifest still stores only `forum_widget_owner_schema_ref_v1` pointers.
It does not embed the actual Forum JSON schemas. The three property editors continue to identify:

- `forum.topic_list.v1`;
- `forum.topic_detail.v1`;
- `forum.reply_stream.v1`.

`rustok-forum-admin` now exposes an SSR property transport that first verifies the exact generated
property-editor descriptor and then calls `ForumWidgetContractService::catalog` for the schema body.
A browser request therefore cannot substitute an arbitrary schema reference or make the Page Builder
manifest a second schema source.

The same transport rechecks authenticated tenant scope, tenant Forum module enablement and effective
`forum_topics:read` before returning schema material. The generic Page Builder package never imports
Forum.

## Owner validation boundary

Apply never writes the browser form directly into the document. The generic host sends the edited
configuration to the provider-neutral property port. The Forum composition adapter calls
`ForumWidgetContractService::validate_props`, returning:

- `valid`;
- owner `normalized_props`;
- validation/sanitize issues.

Only a valid object-shaped `normalized_props` response may advance the Fly draft. Invalid input is
shown to the author and leaves the component unchanged. Sanitize issues may accompany a valid
response; the normalized owner result, not the raw browser input, is the stored configuration.

## Provider-neutral Page Builder property port

`rustok-page-builder-admin` extends the existing contribution host with a provider-neutral property
port. The host contract supports only:

- loading a schema for an already-admitted property-editor descriptor;
- validating a candidate props object;
- receiving normalized props and typed issues.

No Forum DTO, endpoint name, persistence object or RBAC rule exists in the generic host/panel.
Application composition supplies the Forum implementation only when Forum is enabled for the tenant,
using the same effective manifest permission handshake already used by preview contribution
admission.

The generic form intentionally supports the bounded JSON-schema subset used by the current Forum
catalog: object schemas with `additionalProperties=false`, strings/UUID strings/string enums,
bounded unsigned integers and booleans. It rejects unsupported schema shapes instead of guessing a
control or silently accepting arbitrary fields.

## Fly draft mutation

The selected component is resolved from the already capability/permission-filtered contribution
assembly. A property editor is actionable only when:

1. the selected Fly component has provider/component identity;
2. that exact provider/component pair has an admitted property-editor descriptor;
3. the host mounted a property port for that provider;
4. the existing `props` field is object-shaped.

Schema loading is explicit. Before Apply, the panel verifies that the selected component and exact
registered schema reference are still the ones that were loaded. After asynchronous owner
validation it rechecks the selected component again.

A successful Apply executes the ordinary Fly command path:

`EditorCommand::Patch -> ComponentPatch::set_field("props", normalized_props)`.

This preserves Fly history/revision/validation behavior and stores only normalized configuration.
No owner preview result, topic/reply data, tenant identity, actor identity or Forum persistence state
is written to the builder document.

## Capability behavior

The authoring contribution still requires `tree + properties`, while preview renderer admission is a
separate `preview` contribution. Consequently:

- `preview_off` may keep Forum blocks/property editors admitted while hiding preview renderers;
- `properties_off` filters the authoring/property contribution before the panel can resolve it;
- `builder_off` remains read-only/unavailable through the existing provider-status narrowing.

The owner property transport independently rechecks authorization even after contribution discovery.

## Canonical metadata state

The three Forum property schema references now report:

`owner_data_state = "owner_property_editor_ready"`.

The widget-catalog contribution metadata also records:

`property_data_state = "owner_property_editor_ready"`.

Preview remains `owner_preview_transport_ready`. Forum remains the persistence, visibility,
validation and authorization owner.

## Browser/runtime evidence remains open

The browser/runtime evidence remains open; source presence is not execution proof. The next retained
evidence should prove:

1. Forum-enabled Pages can insert each Forum block, load its owner schema, receive invalid-field
   diagnostics, apply a sanitized/normalized valid configuration, undo/redo the Fly patch and save
   the draft through the existing Pages facade;
2. `preview_off` still permits admitted Forum property editing while preview renderer/owner preview
   is unavailable;
3. `properties_off` and Forum-disabled tenants expose no actionable Forum property editor;
4. schema/validation transports reject tenant mismatch, disabled Forum and missing effective
   `forum_topics:read`, while `forum_topics:manage` continues to satisfy read admission;
5. the required `topic_id`, enum, bounds, UUID trimming/defaulting and sanitize behavior match the
   current Forum owner contract.

Observed Forum Wave evidence remains blocked on the existing Pages reference-consumer execution
gate and these retained Forum browser/runtime checks.

## Validation boundary

No new Cargo dependency declaration is introduced by this slice. No tests, Node verifiers, Cargo
commands, formatters, lock generation, builds, workflows, CI, database, browser or runtime evidence
were executed while preparing it.

## Next cursor

FORUM-32 source composition is now complete for canonical metadata, Fly component/block/adapter
identity, owner preview and owner-backed property editing. The next cursor is retained executable
evidence, not another local Forum schema/data authority:

1. add/retain browser evidence for Forum block insertion, schema loading, owner validation,
   normalized `props`, undo/redo and preview/property capability toggles;
2. add/retain runtime authorization evidence for owner property/preview transports and visibility;
3. confirm Forum-disabled Pages has no Forum contribution extension;
4. replace synthetic Forum Wave evidence only after the Pages reference-consumer gate and the Forum
   executable packet are accepted.
