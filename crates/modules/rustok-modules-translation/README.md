# RusToK Modules Translation

Adapter contracts that expose static module settings as translation targets.

## Responsibilities

- Derive deterministic translation target identities from module settings.
- Bridge module-owned settings metadata to translation target contracts.
- Validate target evidence without owning translation persistence.

## Interactions

The crate consumes `rustok-modules` metadata and implements `rustok-translation-targets` contracts.

## Entry points

- `src/lib.rs` — adapter types, validation, and public exports.

## Documentation

- [CRATE_API.md](CRATE_API.md) — Public API contract, adapter boundary, capability bindings, and domain invariants.
- [Modules Documentation](../rustok-modules/docs/README.md)
- [Translation Documentation](../rustok-translation/docs/README.md)
