# План реализации `rustok-pricing`

Статус: pricing boundary выделен как отдельный модуль, но полноценный
`pricing 2.0` остаётся в активном backlog umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-pricing` как owner pricing service boundary;
- синхронизировать pricing runtime contract, dependencies и local docs;
- не смешивать pricing storage с product catalog или promotions/tax оркестрацией.

## Текущее состояние

- `PricingModule`, `PricingService` и pricing migrations уже выделены;
- модуль зависит от `product`, не создавая цикла с umbrella `rustok-commerce`;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- richer price lists, rules, tiers и promotions остаются частью следующего слоя развития.

## Этапы

### 1. Contract stability

- [x] закрепить pricing boundary как отдельный модуль;
- [x] удерживать зависимость `pricing -> product` без цикла на umbrella;
- [ ] удерживать sync между pricing runtime contract, commerce transport и module metadata.

### 2. Pricing 2.0 rollout

- [ ] перейти от базовых цен к rule-driven price resolution;
- [ ] добавить tiers, adjustments и promotion-ready semantics;
- [ ] покрывать deterministic price resolution и rounding targeted tests.

### 3. Operability

- [ ] документировать новые pricing guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении pricing/promotion scope.

## Проверка

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- targeted tests для price resolution, dependency contracts и money semantics

## Правила обновления

1. При изменении pricing runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении pricing/promotion boundary обновлять umbrella commerce docs.
