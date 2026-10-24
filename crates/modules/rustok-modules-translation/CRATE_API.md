# rustok-modules-translation / CRATE_API

## Public surface

### Primary Public Types & Contracts
- `STATIC_SETTINGS_TRANSLATION_OWNER_SLUG`: Constant `"modules"`.
- `STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND`: Constant `"static_settings"`.
- `StaticSettingsTranslationIdentity`: Bridges `StaticSettingsLocalizationRegistry` to `TranslationResourceIdentity` and provides field descriptors.
- `StaticSettingsTranslationRevisions`: Encapsulates resource, source, and target opaque revisions.
- `StaticSettingsTranslationApplyPlan`: Planned execution sequence of `StaticLocalizedSettingApplyCommand`s.
- `StaticSettingsTranslationPrepareResult`: Discriminated union of `Ready(StaticSettingsTranslationApplyPlan)` and `Rejected(TranslationPatchValidation)`.
- `StaticSettingsTranslationIdentityError`: Structured error enum.

## Platform Capability Bindings
- **Adapter Boundary**: Maps static module settings into neutral `TranslationResourceIdentity` without owning persistence or registering runtime providers.
- **Deterministic Hashing**: Implements SHA-256 revision derivations for source and target settings values.
- **Command Synthesis**: Translates validated patches into `StaticLocalizedSettingApplyCommand` sequences for `rustok-modules`.

## Domain Collaborations
- **`rustok-modules`**: Supplies static settings metadata, registry definitions, and apply command execution.
- **`rustok-translation-targets`**: Neutral target contracts, field descriptors, patch requests, and validation models.
- **`rustok-translation`**: Future translation control plane orchestrating static settings localization.

## Minimum Contract Set

### Input DTOs/Commands
- `StaticSettingsLocalizationRegistry`: Originating static settings registry.
- `StaticSettingsExactLocaleSnapshot`: Source/target snapshot of localized settings fields.
- `TranslationPatchRequest`: Inbound translation patch to be validated and prepared into owner commands.
- `ModuleCommandContext`: Command context containing tenant ID, actor, trace, and idempotency key.

### Domain Invariants
- **Owner Sovereignty**: Contains zero direct database access; delegates all persistence writes to `rustok-modules`.
- **Identity Strictness**: Validates that owner slug is `"modules"`, resource kind is `"static_settings"`, and module slug is a valid static module slug.
- **Revision Monotonicity**: Increments expected owner revision once per patched field in deterministic sorted order.
- **Deterministic Step Idempotency**: Derives unique deterministic UUIDs per field application step to prevent receipt collisions.
- **AI Export Safeguard**: Settings copy is tenant-private; AI export is denied by default until explicitly opted in.

### Events / Outbox Side Effects
- Emits no outbox events directly; all side effects are managed by the `rustok-modules` owner application service.

### Errors / Failure Codes
- `StaticSettingsTranslationIdentityError`:
  - `ForeignIdentity`: Wrong owner slug, resource kind, or subresource ID.
  - `SnapshotIdentityMismatch`: Snapshot module slug does not match identity.
  - `UnadmittedSnapshotField`: Field in snapshot not admitted by registry.
  - `InvalidSnapshot`: Inconsistent or non-positive revisions.
  - `InvalidCommandContext`: Tenant mismatch or missing command metadata.
  - `OwnerRevisionOverflow`: Checked integer overflow protection.
  - `TargetContract`: Forwarded contract errors.
