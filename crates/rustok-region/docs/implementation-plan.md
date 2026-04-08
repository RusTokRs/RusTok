# План реализации `rustok-region`

Статус: region boundary выделен; модуль держит country/currency/tax baseline, storefront lookup contract и собственные module-owned admin/storefront UI.

## Область работ

- удерживать `rustok-region` как owner region/country/currency policy baseline;
- удерживать region CRUD/read-side внутри module-owned service и admin/storefront UI packages;
- синхронизировать region runtime contract, manifest metadata и local docs;
- не смешивать region boundary с tenant locale policy или полноценным tax domain.

## Текущее состояние

- `regions` и `RegionService` уже живут в отдельном модуле;
- модуль задаёт базовый lookup по `region_id` или стране;
- tenant locale policy остаётся platform-level concern вне `rustok-region`;
- storefront region transport всё ещё публикуется через `rustok-commerce`;
- admin route для region list/detail/create/update теперь живёт в `rustok-region/admin` и использует native Leptos server functions поверх `RegionService`.
- storefront route для region discovery теперь живёт в `rustok-region/storefront` и использует native Leptos server functions с GraphQL fallback поверх существующего `storefrontRegions` transport.

## Этапы

### 1. Contract stability

- [x] зафиксировать region-owned storage и lookup contract;
- [x] отделить region boundary от tenant locale policy;
- [x] вывести admin UI по ownership boundary модуля;
- [x] вывести storefront UI по ownership boundary модуля;
- [ ] удерживать sync между region runtime contract, commerce orchestration и module metadata.

### 2. Domain expansion

- [ ] развивать richer region/country/currency policy только через module-owned service layer;
- [ ] не превращать плоские tax flags в суррогат полноценного tax domain;
- [ ] покрывать region resolution и policy edge-cases targeted tests.

### 3. Operability

- [x] документировать module-owned admin/storefront routes и manifest wiring одновременно с runtime surface;
- [ ] удерживать local docs, `README.md` и admin package docs синхронизированными;
- [ ] удерживать local docs, `README.md`, `admin/README.md` и `storefront/README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении region/storefront orchestration expectations.

## Проверка

- `cargo xtask module validate region`
- `cargo xtask module test region`
- `cargo check -p rustok-region-admin --lib`
- `cargo check -p rustok-region-storefront --lib`
- targeted tests для region lookup, country/currency policy и tax-baseline semantics

## Правила обновления

1. При изменении region runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `docs/README.md`, `admin/README.md`, `storefront/README.md` и `rustok-module.toml`.
3. При изменении admin wiring синхронизировать `apps/admin` docs и central UI indexes.
4. При изменении region/pricing/tax orchestration обновлять umbrella docs.
