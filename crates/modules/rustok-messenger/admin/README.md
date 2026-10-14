# `rustok-messenger-admin`

Leptos admin UI package for the `rustok-messenger` module, providing channel management, real-time fabric observability, and zero-knowledge cryptographic device audits.

## Architecture

This package strictly complies with **FFA (Fluid Frontend Architecture)**:
- [Module UI Package Architecture](../../../../docs/UI/module-package-architecture.md)
- [Module UI Package Implementation Guide](../../../../docs/UI/module-package-implementation.md)

### Layer Separation
1. **`core.rs`:** Framework-agnostic view-models, validation rules, state transitions, and form drafts. Contains zero `leptos::*` imports and is covered by unit tests.
2. **`model.rs`:** Transport-neutral DTOs.
3. **`transport/`:** Transport facade decoupling the UI adapter from execution topology (native SSR vs GraphQL/REST).
4. **`i18n.rs`:** Framework-agnostic Project Fluent localization using `rustok-ui-i18n`.
5. **`ui/leptos.rs`:** Thin Leptos render/bind adapter using canonical `leptos-ui` design primitives.
