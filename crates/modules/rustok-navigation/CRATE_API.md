# rustok-navigation / CRATE_API

## Platform Capability Bindings
- `NavigationModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `navigation`.
- Dependencies: `channel`, `outbox`.
- Permissions: `Resource::Navigation` with `Action::Create`, `Read`, `Update`, `Delete`, `List`, `Manage`.

## Domain Collaborations
- Sovereign owner for localized navigation menus, nested items, and channel slot bindings.
- Coordinates channel scoping with `rustok-channel` (`ChannelContext`).
- Implements `NavigationMenuTranslationTargetProvider` for platform translation targets via `rustok-translation-targets`.
- Outbox integration with `rustok-outbox` for durable operation receipts.
- Exposes GraphQL mutations/queries and REST endpoints (`http::axum_router`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreateMenuInput`: name, handle/slug, optional description.
- `UpdateMenuInput`: updated name, handle/slug, optional description.
- `CreateMenuItemInput`: menu_id, title, url, optional parent_id, sort_order, target (`_self`, `_blank`).
- `UpdateMenuItemInput`: title, url, sort_order, parent_id, target.
- `SetMenuBindingInput`: channel_id, location slot (`header`, `footer`, `sidebar`, `mobile`), menu_id.

### Domain Invariants
- Multi-tenant and channel isolation: every menu, item, and binding enforces tenant and optional channel authority.
- Unique constraints on `(tenant_id, handle)` for menus and `(tenant_id, channel_id, location)` for slot bindings.
- Atomic locale aggregates: a menu name and every nested item title apply as one translation transaction.
- Revision CAS guards: `menus` resource revisions and `menu_translations` aggregate revisions enforce concurrency protection.
- Static URL links: items store clean URLs; empty URLs are rejected.

### Events / Outbox Side Effects
- Uses durable owner receipts for idempotency across translation target applies.
- Records translation cursor repairs in `navigation_translation_changes`.

### Errors / Failure Codes
- `NavigationError::NotFound`: menu, item, or binding not found.
- `NavigationError::Validation`: handle format, empty URL, or invalid location slot.
- `NavigationError::DuplicateHandle`: handle collision within tenant.
- `NavigationError::BindingConflict`: multiple menus bound to the same slot.
- `NavigationError::Translation`: translation integrity failure (`MENU_TRANSLATION_INTEGRITY_ERROR_CODE`).
- `NavigationError::Storage`: underlying database failure.
