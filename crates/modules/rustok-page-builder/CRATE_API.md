# rustok-page-builder / CRATE_API

## Platform Capability Bindings
- `PageBuilderModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `page_builder`.
- Permissions: `Resource::Pages` with `Action::Create`, `Read`, `Update`, `Delete`, `Publish`, `Manage`.

## Domain Collaborations
- FBA-first visual authoring capability and compatibility module for RusToK.
- Integrates framework-neutral `fly` engine with backend page-builder adapters.
- Consumed by `rustok-pages` and other layout/content modules through canonical capability envelopes (`preview`, `tree`, `properties`, `publish`).
- Coordinates interaction capabilities with `rustok-forms` (`FORMS_PROVIDER_ID`).
- Publishes optional `rustok-page-builder-admin` visual composition surface.

## Minimum Contract Set

### Input DTOs/Commands
- `PublishPageBuilderInput`: `page_id`, `revision_id`, `project_data` JSON.
- `PreviewPageBuilderInput`: `page_id`, `project_data`, `PageBuilderPreviewRuntime`.
- `BuilderNodePropertiesInput`: `page_id`, `node_id`, `properties` JSON.
- `PageBuilderCapabilityRequest`: tagged enum covering `Preview`, `Tree`, `Properties`, `Publish`.
- `PageBuilderCapabilityResponse`: capability execution result.

### Domain Invariants
- Visual authoring contracts maintain vendor-neutral `grapesjs` write/read semantics.
- Static publish policy: strict sanitization of CSS, attributes, subresources, and URLs (`validate_static_publish_document`).
- Materialization identity: context, locale, and scenario are deterministic components of the build artifact identity (`compile_materialized_static_landing`).
- Interaction providers: host form runtime verifies registered providers (`forms/submit`) and blocks publication on unregistered interaction forms.
- Rollout toggles: capability flags narrow configured rollout while keeping read paths alive.

### Events / Outbox Side Effects
- Consumer modules own document persistence and outbox event emissions; page builder remains stateless capability engine.
- Emits bounded provider SLO observations and health telemetry.

### Errors / Failure Codes
- `PageBuilderErrorKind::ValidationFailed`: schema or input validation error.
- `PageBuilderErrorKind::FeatureDisabled`: capability disabled by rollout profile (`FEATURE_DISABLED`).
- `PageBuilderErrorKind::SanitizationFailed`: static policy or security rejection.
- `PageBuilderErrorKind::RenderFailed`: error during Fly project rendering or materialization.
