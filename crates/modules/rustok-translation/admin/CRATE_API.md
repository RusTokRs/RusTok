# rustok-translation-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for translation workbench, glossaries, Translation Memory, and reviewer queues.
- Integrates with `rustok-grid-leptos`, `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-translation` for translation jobs, items, memory entries, and glossaries.
- Collaborates with `rustok-translation-targets` and `rustok-storage` for interchange artifacts.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub use ui::leptos::TranslationAdmin`: Root admin view component for translation workbench.

## Minimum Contract Set

### Input DTOs/Commands
- Translation job creation, export, and import commands.
- Glossary, Translation Memory, and reviewer queue query filters and pagination parameters.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Versioned glossary operator workflow binds immutable glossary revision during job creation.
- Translation memory operations require explicit URL-owned `memory_entry_id` selection.

### Events / Outbox Side Effects
- Admin UI adapter package; state mutations delegate event publishing to `rustok-translation`.

### Errors / Failure Codes
- Transport and validation failures produce typed error notices rendered in the translation UI.
