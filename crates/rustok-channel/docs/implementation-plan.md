# План реализации `rustok-channel`

Статус: experimental core capability; `v0 baseline complete`, идёт rollout
post-v0 resolution architecture.

## Область работ

- удерживать `rustok-channel` как domain-owned resolution module, а не host middleware bucket;
- синхронизировать channel runtime contract, admin UI и manifest metadata;
- развивать typed resolution policies без возврата к ad-hoc host logic.

## Текущее состояние

- storage, service layer, runtime resolver и admin UI уже подняты;
- explicit default channel и tenant-scoped typed policy layer уже встроены в baseline;
- `pages`, `blog` и `commerce` уже служат живыми channel-aware proof point;
- local docs, root README и manifest contract входят в scoped audit path.

## Этапы

### 1. Contract stability

- [x] зафиксировать финальную resolution-модель `explicit selectors -> built-in target slice -> typed policies -> explicit default -> unresolved`;
- [x] удерживать domain-owned resolver внутри `rustok-channel`;
- [x] удерживать sync между runtime contract, admin UI и server middleware tests.

### 2. Policy rollout

- [x] довести policy trace в admin bootstrap/runtime diagnostics;
- [x] добавить базовые operator flows для policy-set activation и policy-rule authoring/removal;
- [ ] добавить полный operator flow для policy reorder и disable;
- [ ] решить, остаётся ли built-in host slice отдельным fast-path после полного policy rollout.

### 3. Semantic expansion

- [ ] возвращаться к richer target/connector taxonomy только при появлении реального runtime pressure;
- [ ] расширять channel-aware proof points в доменных модулях только вместе с локальной документацией и tests.

## Проверка

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests для resolution order, explicit selectors и default semantics

## Правила обновления

1. При изменении resolution contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata и UI wiring синхронизировать `rustok-module.toml`.
