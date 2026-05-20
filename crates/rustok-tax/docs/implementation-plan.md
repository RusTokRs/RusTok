# План реализации `rustok-tax`

Статус: foundation phase.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-20T00:00:00Z

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
