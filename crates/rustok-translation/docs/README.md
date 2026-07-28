# Translation module contract

## Purpose

The Translation module is the tenant translation control plane. It coordinates
work while each domain owner remains authoritative for localized business data.

## Responsibility Zone

The module owns inventory projections, provider checkpoints, translation jobs,
proposals, review and approval state, assignments, quality evidence, translation
memory, glossaries, interchange operations, and owner-application receipts.

The first implemented slice owns:

- `translation_inventory_resources`;
- `translation_provider_checkpoints`;
- bounded provider change-cursor synchronization with optimistic checkpoint
  revision protection, provider-identity isolation, and cursor-progress
  validation.

It does not copy source or translated field values into inventory rows.

## Integration

Owner modules register `TranslationTargetProvider` implementations through
`ModuleRuntimeExtensions`. The module consumes the resulting
`TranslationTargetRegistry`; missing providers and missing capabilities fail
explicitly.

`rustok-translation-targets` remains a separate Cargo package even if its
physical directory is later moved under `crates/rustok-translation/`. This
preserves the dependency direction: owners may depend on the neutral SPI but
must never depend on the Translation control-plane crate.

## Verification

- `cargo check -p rustok-translation`
- `cargo test -p rustok-translation`
- `cargo xtask module validate translation`
- `cargo xtask validate-manifest`

## Related Documents

- [Implementation plan](implementation-plan.md)
- [Central translation plan](../../../docs/modules/translation-implementation-plan.md)
- [Translation surface registry](../../../docs/modules/translation-surfaces.json)
- [Module authoring guide](../../../docs/modules/module-authoring.md)
