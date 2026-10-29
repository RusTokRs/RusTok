# rustok-calls-admin

For full public interface and operational contract definitions, see [CRATE_API.md](CRATE_API.md).

Admin UI package for `calls`, following Fluid Frontend Architecture (FFA).

## Responsibilities
- Pure Rust view-models and search filtering in `core`
- Decoupled backend communication via `transport`
- Leptos UI components and shared `DataGrid` in `ui`
- Localization catalog in `locales/`
