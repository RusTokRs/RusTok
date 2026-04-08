# План реализации `rustok-profiles`

Статус: storage/service/GraphQL foundation уже подняты; модуль находится в
режиме rollout hardening вокруг profile summary, backfill и UI/read-model
дальнейшего развития.

## Область работ

- удерживать `rustok-profiles` как отдельный public profile domain;
- синхронизировать storage, summary contracts, GraphQL surfaces и local docs;
- не допускать схлопывания `profiles`, `users`, `customer` и будущих seller surfaces.

## Текущее состояние

- `ProfilesModule`, `rustok-module.toml` и permission surface `profiles:*` уже существуют;
- `profiles` и `profile_translations` уже живут в module-owned storage;
- `ProfileService`, `ProfilesReader`, batched summary lookup и GraphQL transport уже реализованы;
- `blog` и `forum` уже используют модуль как author presentation boundary;
- taxonomy-backed `profile_tags`, `profile.updated` и explicit backfill path уже входят в live contract.

## Этапы

### 1. Contract stability

- [x] зафиксировать profile boundary поверх `users`;
- [x] поднять module-owned storage, service layer и GraphQL baseline;
- [x] внедрить `ProfilesReader` как downstream integration contract;
- [ ] удерживать sync между runtime contracts, GraphQL surface и module metadata.

### 2. Rollout hardening

- [ ] решить, нужен ли отдельный projection/read-model помимо прямого чтения из `profiles + profile_translations`;
- [ ] довести visibility/media policy и оставшиеся storage решения вокруг handle uniqueness;
- [ ] удерживать profile backfill и `profile.updated` semantics совместимыми с downstream consumers.

### 3. UI and operability

- [ ] добавить module-owned UI packages после фиксации profile-domain contract;
- [ ] развить audit trail, observability и runbook guidance для profile conflicts и rollout effects;
- [ ] документировать новые guarantees одновременно с изменением runtime/API surface.

## Проверка

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- targeted tests для handle policy, locale fallback, summary batching, GraphQL path и backfill/events

## Правила обновления

1. При изменении profile runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении downstream integration expectations обновлять consumer docs у `blog`, `forum` и других модулей.
