# План реализации `rustok-region`

Статус: region boundary выделен; модуль держит currency/country/tax baseline,
а locale/currency orchestration над ним остаётся у umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-region` как owner region/country/currency policy baseline;
- синхронизировать region runtime contract и local docs;
- не смешивать region boundary с tenant locale policy или full tax domain.

## Текущее состояние

- `regions` и `RegionService` уже живут в отдельном модуле;
- модуль задаёт базовый lookup по `region_id` или стране;
- tenant locale policy остаётся platform-level concern вне `rustok-region`;
- richer tax domain и pricing orchestration пока живут в backlog umbrella `rustok-commerce`.

## Этапы

### 1. Contract stability

- [x] зафиксировать region-owned storage и lookup contract;
- [x] отделить region boundary от tenant locale policy;
- [ ] удерживать sync между region runtime contract, commerce orchestration и module metadata.

### 2. Domain expansion

- [ ] развивать richer region/country/currency policy только через module-owned service layer;
- [ ] не превращать плоские tax flags в суррогат полноценного tax domain;
- [ ] покрывать region resolution и policy edge-cases targeted tests.

### 3. Operability

- [ ] документировать новые region guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении orchestration expectations.

## Проверка

- `cargo xtask module validate region`
- `cargo xtask module test region`
- targeted tests для region lookup, country/currency policy и tax-baseline semantics

## Правила обновления

1. При изменении region runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении region/pricing/tax orchestration обновлять umbrella docs.
