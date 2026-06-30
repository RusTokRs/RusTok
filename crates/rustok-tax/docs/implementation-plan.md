# План реализации `rustok-tax`

Статус: FBA provider boundary in progress.

## Execution checkpoint

- Current phase: fba_provider_static_evidence
- Last checkpoint: Tax calculation provider boundary now has a neutral `TaxCalculationPort`, module metadata, machine-readable registry and static evidence verified by `npm run verify:tax:fba`.
- Next step: Replace static contract evidence with runtime contract execution and fallback smoke before any `boundary_ready` promotion.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-18T00:00:00Z


## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Evidence:
  - пакетный no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` и fixture-regression suite проверяют `crates/rustok-tax/contracts/evidence/tax-runtime-contract-smoke.json`: shared read policy предшествует owner `TaxService`, typed error mapping и fallback/degraded registry parity защищены от drift; статус остаётся `in_progress` до live provider execution;
  - FBA provider registry `crates/rustok-tax/contracts/tax-fba-registry.json`, static contract evidence `crates/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json` and neutral `TaxCalculationPort`/`tax.calculation.v1` are locked for cart tax calculation consumers; runtime contract execution/fallback smoke remain pending before `boundary_ready`;
  - `scripts/verify/verify-tax-fba.mjs` checks manifest metadata, local/central plan sync, typed `PortContext`/`PortError`, in-process `TaxService` implementation, serializable tax DTOs and static evidence drift.
- Last verified at (UTC): 2026-06-18T00:00:00Z
- Owner: `rustok-tax` module team

## Цель

- вынести tax calculation из hardcoded cart runtime в отдельный bounded context;
- зафиксировать provider seam до реальных внешних интеграций;
- сделать `provider_id` частью tax snapshot contract.

## Текущее состояние

- default provider `region_default` сохраняет текущую region-based tax policy;
- `rustok-cart` вызывает `TaxService`, а не считает налог напрямую из `region`;
- current provider selection hook lives in `regions.tax_provider_id`;
- cart/order tax lines получают typed `provider_id`.

## Следующие шаги

- tax rules beyond flat region rate;
- provider registry и external engine adapters;
- richer jurisdiction metadata и transport parity tests.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
