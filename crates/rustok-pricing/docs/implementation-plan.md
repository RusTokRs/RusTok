# План реализации `rustok-pricing`

Статус: pricing boundary выделен как отдельный модуль; модуль держит pricing runtime
baseline и module-owned admin read-side UI, а dedicated pricing write transport и полный
`pricing 2.0` остаются в активном backlog umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-pricing` как owner pricing service boundary;
- синхронизировать pricing runtime contract, module-owned admin UI и local docs;
- не смешивать pricing storage с product catalog, promotions или tax orchestration.

## Текущее состояние

- `PricingModule`, `PricingService` и pricing migrations уже выделены;
- модуль зависит от `product`, не создавая цикла с umbrella `rustok-commerce`;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- `rustok-pricing/admin` уже публикует pricing-owned admin route для price visibility,
  sale markers и currency coverage inspection;
- `rustok-pricing/storefront` уже публикует pricing-owned storefront route для public
  pricing atlas, currency coverage и sale-marker visibility;
- dedicated pricing mutations пока не вынесены: текущий pricing UI честно остаётся
  read-side поверх существующего product GraphQL контракта.

## Этапы

### 1. Contract stability

- [x] закрепить pricing boundary как отдельный модуль;
- [x] удерживать зависимость `pricing -> product` без цикла на umbrella;
- [x] вынести pricing admin UI в module-owned пакет `rustok-pricing/admin`;
- [x] вынести pricing storefront UI в module-owned пакет `rustok-pricing/storefront`;
- [ ] удерживать sync между pricing runtime contract, admin UI, commerce transport
  и module metadata.

### 2. Pricing transport split

- [ ] вынести dedicated pricing read/write transport из umbrella `rustok-commerce`;
- [ ] перевести pricing admin UI с read-only product-backed transport на targeted
  price mutations и operator workflows;
- [ ] покрывать transport parity, money semantics и compare-at invariants targeted tests.

### 3. Pricing 2.0 rollout

- [ ] перейти от базовых цен к rule-driven price resolution;
- [ ] добавить tiers, adjustments и promotion-ready semantics;
- [ ] покрывать deterministic price resolution и rounding targeted tests.

### 4. Operability

- [x] документировать новые pricing guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении pricing/promotion scope.

## Проверка

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- targeted tests для price resolution, pricing transport и money semantics

## Правила обновления

1. При изменении pricing runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `admin/README.md`
   и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении pricing/promotion boundary обновлять umbrella commerce docs.
