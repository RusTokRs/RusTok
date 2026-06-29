# leptos-auth — implementation plan

_Нет запланированных задач._

## Execution checkpoint

- Current phase: transport_module_alignment
- Last checkpoint: Legacy `src/api.rs` file removed; auth native/server-function + GraphQL fallback implementation now lives in `src/transport.rs`, while `leptos_auth::api::*` remains as a compatibility re-export for existing callers.
- Next step: Синхронизировать план с текущим кодом и выбрать первый незавершённый пункт.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-29T00:00:00Z



## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
