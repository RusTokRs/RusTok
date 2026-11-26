# rustok-channel-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-channel` module.

## Responsibilities

- Exposes the channel admin root view used by `apps/admin`.
- Stays module-owned: channel-specific admin UI does not live in `apps/admin`.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns the experimental channel-management operator flow: bootstrap, create channel, rename, activate/deactivate and a confirmed delete, attach targets, bind modules, bind OAuth apps.
- Ships package-owned `admin/locales/en.ftl` and `admin/locales/ru.ftl` bundles declared through `[provides.admin_ui.i18n]`.
- Renders only `rustok-ui-leptos` components (`Button`, `Input`, `Select`, `Checkbox`, `Alert`) — **zero** raw `<button>`/`<input>`/`<select>` in this package, which is what `verify-ui-ds-adoption` measures. A new raw control fails the ratchet instead of quietly re-introducing a third visual language.

## Entry Points

- `ChannelAdmin` — root admin page component for the module.
- `rustok-module.toml [provides.admin_ui]` advertises `leptos_crate`, `route_segment`, and `nav_label` for host composition.

## Interactions

- Consumed by `apps/admin` via manifest-driven `build.rs` code generation.
- Mounted by the Leptos admin host under `/modules/channels` through the generic module page route.
- Uses native Leptos `#[server]` functions through `HostRuntimeContext` for bootstrap, channel lifecycle (create/update/set-active/delete/default), target CRUD, module bindings, OAuth app bindings, and policy-set/rule lifecycle operations (create/activate/update/reorder/delete).
- Has exactly **one** transport per build (`transport::dispatch`): server-side render and hydration call the server functions in-process, a browser build calls `/api/channels/*` on the origin that served the bundle. There is no cross-transport retry, so a rejected mutation is never replayed and no API base URL or loopback default exists.
- Shows the lifecycle transition from `core::channel_lifecycle_toggle`: the adapter binds the derived command and operator copy, it does not re-derive activation state.
- Owns no lifecycle write semantics. Rename goes through `core::channel_rename_command` (trim + blank-name rejection + the `UpdateChannelPayload` it becomes + its feedback copy) and delete reports `core::CHANNEL_DELETE_FEEDBACK`; the adapter keeps only view state (which form is open, busy flag) and calls the transport.
- Resolves the bootstrap with a Leptos `Resource` so SSR can render the loaded view. CSR and hydration keep the browser HTTP future on its originating JS thread via a WASM-only checked wrapper; the transport and request scope are unchanged.
- Must keep API assumptions aligned with the `rustok-channel` module and server wiring.
- Reads the effective UI locale from `UiRouteContext.locale`; package-local translations must stay aligned with the host locale contract.

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.
- See [platform docs](../../../../docs/index.md).
